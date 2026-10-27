//! Read-only gate for GitInteractiveRebaseAction / GitSingleCommitEditingAction.
//! A prepared snapshot is not authorization: validate again under the write queue.
use crate::{
    git::{Cancel, Repository, is_oid},
    operation_process::{self, Progress},
    published_commits::Protection,
    squash_check,
};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, PartialEq, Eq)]
pub struct Prepared {
    repository: PathBuf,
    directory: PathBuf,
    head: String,
    branch: Option<String>,
    target: String,
    /// None means --root; UI must confirm the initial-commit warning first.
    base: Option<String>,
    refs: String,
    shallow: (bool, std::collections::HashSet<String>),
}
impl Prepared {
    pub fn repository(&self) -> Repository {
        Repository::new(&self.repository)
    }
    pub fn head(&self) -> &str {
        &self.head
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub(crate) fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }
    pub fn base(&self) -> Option<&str> {
        self.base.as_deref()
    }
    pub fn initial(&self) -> bool {
        self.base.is_none()
    }
    pub fn prepare(
        repo: &Repository,
        target: &str,
        protection: Protection<'_>,
        token: &Cancel,
    ) -> Result<Self, String> {
        if !is_oid(target) {
            return Err("Invalid rebase commit".into());
        }
        let directory = repo.editor_directory(token)?;
        let head = crate::commit_operation::head(repo, token)?.ok_or("Cannot find HEAD")?;
        let branch = squash_check::symbolic_head(repo, token)?;
        let refs = squash_check::inventory(repo, token)?;
        let shallow = repo.shallow_boundaries(token)?;
        check_idle(repo, &directory, token)?;
        if shallow.1.contains(target) {
            return Err(
                "Cannot rebase from a shallow boundary; fetch complete history first".into(),
            );
        }
        let metadata = repo.text(&["rev-list", "--parents", "-n", "1", target, "--"], token)?;
        let fields: Vec<_> = metadata.split_whitespace().collect();
        if fields.first().copied() != Some(target) || fields.iter().any(|id| !is_oid(id)) {
            return Err("Invalid rebase commit metadata".into());
        }
        if fields.len() > 2 {
            return Err("Cannot start interactive rebase from a merge commit".into());
        }
        let ancestry = operation_process::run(
            repo.command()
                .args(["merge-base", "--is-ancestor", target, &head]),
            token,
            &Progress::default(),
            Some(Duration::from_secs(30)),
        )
        .map_err(|e| e.to_string())?;
        if ancestry.stopped.is_some() {
            return Err("Rebase ancestry check interrupted".into());
        }
        if !ancestry.status.success() {
            return Err("Selected commit does not belong to HEAD history".into());
        }
        if let Some(branch) = squash_check::protected_containing(repo, target, protection, token)? {
            return Err(format!(
                "Cannot rebase commits published to protected branch {branch}"
            ));
        }
        let prepared = Self {
            repository: repo.path.clone(),
            directory,
            head,
            branch,
            target: target.into(),
            base: fields.get(1).map(|s| (*s).into()),
            refs,
            shallow,
        };
        prepared.check_snapshot(repo, token)?;
        Ok(prepared)
    }
    pub fn validate(&self, protection: Protection<'_>, token: &Cancel) -> Result<(), String> {
        let repo = self.repository();
        self.check_snapshot(&repo, token)?;
        let now = Self::prepare(&repo, &self.target, protection, token)?;
        if now != *self {
            return Err("Repository changed; reopen the rebase plan".into());
        }
        Ok(())
    }
    fn check_snapshot(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.directory
            || crate::commit_operation::head(repo, token)?.as_deref() != Some(&self.head)
            || squash_check::symbolic_head(repo, token)? != self.branch
            || squash_check::inventory(repo, token)? != self.refs
            || repo.shallow_boundaries(token)? != self.shallow
        {
            return Err("Repository changed; reopen the rebase plan".into());
        }
        check_idle(repo, &self.directory, token)
    }
}
fn check_idle(
    repo: &Repository,
    directory: &std::path::Path,
    token: &Cancel,
) -> Result<(), String> {
    let state = repo.operation_state(token)?;
    if state.in_progress() || !state.conflicts.is_empty() {
        return Err("Finish the current Git operation before interactive rebase".into());
    }
    if crate::commit_recovery::pending(directory)?.is_some() {
        return Err("Resolve pending index recovery before interactive rebase".into());
    }
    Ok(())
}
