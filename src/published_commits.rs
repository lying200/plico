//! Rebased GitRebaseProcess.isRebasingPublishedCommit / GitProtectedBranches.kt.
//! Reference 4aec35256d765c217aa0d662b82ac4e860c396fd (Apache-2.0).
use crate::{
    git::{Cancel, Repository, is_oid},
    operation_process::{self, Progress},
    pull::Target,
};
use std::time::Duration;

/// GitSharedSettings defaults. Custom Java regex/provider rules belong to the
/// settings layer and must supply their resulting target set to the check.
pub fn default_protected(target: &Target) -> bool {
    matches!(target.branch.as_str(), "main" | "master")
}

/// Resolve policy against current remote refs at the published-commit gate,
/// after Fetch. Explicit targets support callers with their own settings rules.
#[derive(Clone, Copy)]
pub enum Protection<'a> {
    Defaults,
    Explicit(&'a [Target]),
}
impl Protection<'_> {
    pub fn check(
        self,
        repo: &Repository,
        base: &Target,
        cancel: &Cancel,
    ) -> Result<Option<Target>, String> {
        match self {
            Self::Defaults => {
                let targets = repo
                    .pull_targets(cancel)?
                    .branches
                    .into_iter()
                    .filter(default_protected)
                    .collect::<Vec<_>>();
                repo.published_rebase_target(base, &targets, cancel)
            }
            Self::Explicit(targets) => repo.published_rebase_target(base, targets, cancel),
        }
    }
}

impl Repository {
    /// Returns a protected remote branch containing a commit in base..HEAD.
    /// Errors are unknown, never proof that rebasing published commits is safe.
    /// Caller must obtain confirmation for a match/unknown before rebase.
    /// This is based on local remote refs, just like the upstream fallback query.
    pub fn published_rebase_target(
        &self,
        base: &Target,
        protected: &[Target],
        cancel: &Cancel,
    ) -> Result<Option<Target>, String> {
        let known = self.pull_targets(cancel)?;
        if known.current_branch.is_none() {
            return Err("Rebase requires an attached branch".into());
        }
        let resolve = |target: &Target| -> Result<String, String> {
            if !known.contains(&target.remote, &target.branch) {
                return Err("Remote branch is no longer available".into());
            }
            let reference = format!(
                "refs/remotes/{}/{}^{{commit}}",
                target.remote, target.branch
            );
            self.text(&["rev-parse", "--verify", &reference], cancel)
                .map(|id| id.trim().to_owned())
        };
        let head = self
            .text(&["rev-parse", "--verify", "HEAD^{commit}"], cancel)?
            .trim()
            .to_owned();
        let base = resolve(base)?;
        for target in protected {
            let protected_tip = resolve(target)?;
            // Common ancestors = ancestors of all best merge bases. There is a
            // rewritten published commit iff some base is not reachable from the
            // update target. --all matters for criss-cross histories. This avoids
            // collecting every candidate commit and spawning one query per commit.
            let mut command = self.command();
            command.args(["merge-base", "--all", &head, &protected_tip]);
            let result = operation_process::run(
                &mut command,
                cancel,
                &Progress::default(),
                Some(Duration::from_secs(30)),
            )
            .map_err(|e| e.to_string())?;
            if result.stopped.is_some() {
                return Err("Published-commit query cancelled or timed out".into());
            }
            if result.status.code() == Some(1) && result.stdout.bytes().is_empty() {
                continue;
            }
            if !result.status.success() || result.stdout.truncated {
                return Err("Cannot determine published commits".into());
            }
            let bytes = result.stdout.bytes();
            let common = std::str::from_utf8(&bytes).map_err(|_| "Invalid merge-base output")?;
            for oid in common.lines() {
                if !is_oid(oid) {
                    return Err("Invalid merge-base identity".into());
                }
                let outside = self.text(
                    &["rev-list", "--max-count=1", oid, "--not", &base, "--"],
                    cancel,
                )?;
                if !outside.is_empty() {
                    return Ok(Some(target.clone()));
                }
            }
        }
        if cancel.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(None)
    }
}
