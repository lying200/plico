//! Root-owned branch preparation and partial execution results.
//! Rebased 4aec352 GitBranchOperation / GitRepositoryManager (Apache-2.0).
use super::{Choice, Prepared, Report, execute_choice};
use crate::{
    git::{Cancel, Repository},
    operation_process::Progress,
};
use std::collections::HashSet;

struct Root {
    repository: Repository,
    prepared: Result<Prepared, String>,
}

/// Keep failed preparations in their position: one missing start point must not
/// erase earlier successes or turn the operation into an all-or-nothing write.
pub struct Batch {
    roots: Vec<Root>,
}

pub enum Status {
    NotRun,
    PreparationFailed(String),
    Executed(Result<Report, String>),
}
pub struct RootResult {
    pub repository: Repository,
    pub status: Status,
    // Retained for the explicit rollback phase; a different batch cannot be
    // paired with this result by the UI.
    prepared: Result<Prepared, String>,
}
impl RootResult {
    /// Restore an existing non-current branch overwritten without checkout.
    pub fn rollback_overwritten_ref(
        &self,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<super::rollback::RefReport, String> {
        if !self.succeeded() {
            return Err("Only successful roots can be rolled back".into());
        }
        super::rollback::overwritten_ref(
            self.prepared.as_ref().map_err(String::clone)?,
            token,
            progress,
        )
    }

    /// Only after explicit rollback confirmation, for a successful create and checkout.
    pub fn rollback_checkout(
        &self,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<super::rollback::Report, String> {
        if !self.succeeded() {
            return Err("Only successful roots can be rolled back".into());
        }
        super::rollback::checkout_created(
            self.prepared.as_ref().map_err(String::clone)?,
            token,
            progress,
        )
    }

    pub fn succeeded(&self) -> bool {
        matches!(&self.status, Status::Executed(Ok(report)) if report.verified)
    }
    pub fn prepared(&self) -> Option<&Prepared> {
        self.prepared.as_ref().ok()
    }
}
pub struct ResultSet {
    pub roots: Vec<RootResult>,
    pub cancelled: bool,
}
pub struct RollbackResult {
    pub repository: Repository,
    pub result: Result<crate::branch_delete::Report, String>,
}
impl RollbackResult {
    pub fn succeeded(&self) -> bool {
        self.result.as_ref().is_ok_and(|report| {
            report.deleted
                && report.refresh_error.is_none()
                && report
                    .command
                    .as_ref()
                    .is_ok_and(|command| command.status.success() && command.stopped.is_none())
        })
    }
}

impl ResultSet {
    pub fn succeeded(&self) -> bool {
        !self.roots.is_empty() && self.roots.iter().all(RootResult::succeeded)
    }

    /// Only after the user chooses Rollback. This is the create-without-checkout
    /// rollback from GitCreateBranchOperation, not a checkout/rebase undo.
    /// Failed and unexecuted roots are never touched. Report each failure and
    /// continue attempting the other successful roots, as GitCompoundResult does.
    pub fn rollback_creation(&self, token: &Cancel, progress: &Progress) -> Vec<RollbackResult> {
        self.roots
            .iter()
            .filter(|root| root.succeeded())
            .map(|root| {
                let result = (|| {
                    let prepared = root.prepared.as_ref().map_err(String::clone)?;
                    if prepared.checkout || prepared.previous_tip.is_some() {
                        return Err("This operation requires checkout or overwrite rollback".into());
                    }
                    let repo = prepared.repository();
                    if repo.editor_directory(token)? != prepared.directory {
                        return Err("Repository changed; branch rollback was not performed".into());
                    }
                    if super::resolve(&repo, "HEAD", token)? != prepared.head
                        || repo
                            .text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?
                            .trim()
                            != prepared.symbolic_head
                    {
                        return Err("HEAD changed; branch rollback was not performed".into());
                    }
                    let deletion = crate::branch_delete::Prepared::read(
                        &repo,
                        &format!("refs/heads/{}", prepared.name),
                        token,
                    )?;
                    if deletion.tip != prepared.target {
                        return Err("Created branch changed; rollback was not performed".into());
                    }
                    // Reuse deletion's worktree occupancy, operation-state, merged
                    // commit and final ref checks. Upstream passes force=false.
                    crate::branch_delete::execute(&repo, &deletion, false, token, progress)
                })();
                RollbackResult {
                    repository: root.repository.clone(),
                    result,
                }
            })
            .collect()
    }
}
impl Batch {
    pub fn read(
        repositories: &[Repository],
        name: &str,
        source: &str,
        checkout: bool,
        overwrite: bool,
        token: &Cancel,
    ) -> Result<Self, String> {
        if repositories.is_empty() {
            return Err("No repository selected".into());
        }
        let mut seen = HashSet::new();
        let mut repositories = repositories.to_vec();
        // Upstream reverses VirtualFileHierarchicalComparator: descending path
        // length, stable for equal lengths (not reverse alphabetical order).
        repositories.sort_by_cached_key(|repo| {
            std::cmp::Reverse(repo.path.to_string_lossy().encode_utf16().count())
        });
        let mut roots = Vec::with_capacity(repositories.len());
        for repository in repositories {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let identity = repository
                .editor_directory(token)
                .and_then(|directory| directory.canonicalize().map_err(|e| e.to_string()));
            if let Ok(directory) = &identity {
                if !seen.insert(directory.clone()) {
                    continue;
                }
            }
            let prepared = identity.and_then(|_| {
                Prepared::read_options(&repository, name, source, checkout, overwrite, token)
            });
            roots.push(Root {
                repository,
                prepared,
            });
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(Self { roots })
    }

    pub fn preparations(&self) -> impl Iterator<Item = (&Repository, Result<&Prepared, &str>)> {
        self.roots.iter().map(|root| {
            (
                &root.repository,
                root.prepared.as_ref().map_err(String::as_str),
            )
        })
    }

    /// Execute one operation group after its confirmation. Higher-level branch
    /// checkout partitions (existing/new, reset/rebase) must form separate groups
    /// as GitBranchCheckoutOperation does. None uses each root's default choice.
    pub fn execute(self, choice: Option<Choice>, token: &Cancel, progress: &Progress) -> ResultSet {
        let mut stopped = false;
        let roots = self
            .roots
            .into_iter()
            .map(|root| {
                let status = if stopped || token.cancelled() {
                    stopped = true;
                    Status::NotRun
                } else {
                    match &root.prepared {
                        Err(error) => {
                            stopped = true;
                            Status::PreparationFailed(error.clone())
                        }
                        Ok(prepared) => {
                            let result = execute_choice(
                                &root.repository,
                                prepared,
                                choice.unwrap_or_else(|| prepared.default_choice()),
                                token,
                                progress,
                            );
                            stopped = !result.as_ref().is_ok_and(|report| report.verified);
                            Status::Executed(result)
                        }
                    }
                };
                RootResult {
                    repository: root.repository,
                    prepared: root.prepared,
                    status,
                }
            })
            .collect();
        ResultSet {
            roots,
            cancelled: token.cancelled(),
        }
    }
}
