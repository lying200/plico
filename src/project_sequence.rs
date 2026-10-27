//! Rebased 4aec352 GitApplyChangesProcess: group by root, stop on the first
//! unfinished group, retain partial results. JetBrains, Apache-2.0.
//! The ordinary path does not stash; save/retry is a separate user decision.
mod continuation;
mod saved;
mod workflow;
use crate::{
    auth_request::Requests,
    cherry_pick,
    git::{Cancel, Repository},
    operation_process::Progress,
    project_log::{CommitId, selection},
    revert_operation,
};
pub use saved::SavedResults;
use std::ffi::OsStr;
pub use workflow::Workflow;

#[derive(Clone, Copy)]
pub enum Kind {
    CherryPick,
    Revert,
}

#[derive(Clone)]
enum Prepared {
    CherryPick(cherry_pick::Prepared),
    Revert(revert_operation::Batch),
}

pub enum Report {
    CherryPick(cherry_pick::Report),
    Revert(revert_operation::BatchReport),
}
impl Report {
    pub fn complete(&self) -> bool {
        match self {
            Self::CherryPick(report) => report.status == cherry_pick::Status::Success,
            Self::Revert(report) => {
                report.error.is_none()
                    && report.remaining.is_empty()
                    && report.recovery.is_none()
                    && report.retry.is_none()
                    && report.last.as_ref().is_some_and(|last| {
                        last.state
                            .as_ref()
                            .is_ok_and(|state| !state.in_progress() && state.conflicts.is_empty())
                    })
            }
        }
    }
}

/// A root's original selection and preparation survive an earlier root's failure.
/// Prepared guards still reject changed HEAD/branch/gitdir before a write.
pub struct Root {
    pub repository: Repository,
    pub targets: Vec<String>,
    prepared: Prepared,
    /// None means not attempted, never success.
    pub result: Option<Result<Report, String>>,
}
impl Root {
    pub fn complete(&self) -> bool {
        self.result
            .as_ref()
            .is_some_and(|result| result.as_ref().is_ok_and(Report::complete))
    }
}

pub struct Plan {
    roots: Vec<Root>,
}
pub struct Results {
    pub roots: Vec<Root>,
    pub cancelled: bool,
}
impl Results {
    pub fn complete(&self) -> bool {
        !self.cancelled && !self.roots.is_empty() && self.roots.iter().all(Root::complete)
    }

    /// Build an explicit local-changes retry, excluding all completed work.
    /// Keep the failed root's refreshed guard and untouched roots' original
    /// guards. Never re-read HEAD to make a stale operation appear valid.
    /// Saving/restoring drafts belongs to the enclosing operation session.
    pub fn retry_plan(&self) -> Result<Plan, String> {
        if self.cancelled {
            return Err("Cancelled operation requires review before retry".into());
        }
        let stopped = self
            .roots
            .iter()
            .position(|root| !root.complete())
            .ok_or("No unfinished operation to retry")?;
        let root = &self.roots[stopped];
        let prepared = match root.result.as_ref() {
            Some(Ok(Report::CherryPick(report))) => report.retry.clone().map(Prepared::CherryPick),
            Some(Ok(Report::Revert(report))) if report.error.is_none() => {
                report.retry.clone().map(Prepared::Revert)
            }
            _ => None,
        }
        .ok_or("Operation has no local-changes retry; resolve or review it first")?;
        // Retry preparations already omit successful/skipped commits. Cherry
        // preparations use execution order, whereas Root.targets uses Log order.
        let targets = match &prepared {
            Prepared::CherryPick(prepared) => prepared.targets.iter().rev().cloned().collect(),
            Prepared::Revert(prepared) => prepared.targets().map(str::to_owned).collect(),
        };
        let mut roots = vec![Root {
            repository: root.repository.clone(),
            targets,
            prepared,
            result: None,
        }];
        for root in &self.roots[stopped + 1..] {
            if root.result.is_some() {
                return Err("Operation results changed; review the remaining selection".into());
            }
            roots.push(Root {
                repository: root.repository.clone(),
                targets: root.targets.clone(),
                prepared: root.prepared.clone(),
                result: None,
            });
        }
        Ok(Plan { roots })
    }
}
impl Plan {
    /// Remaining selection in Log order, including repository identity.
    pub fn selection(&self) -> Vec<CommitId> {
        self.roots
            .iter()
            .flat_map(|root| {
                let path = std::sync::Arc::new(root.repository.path.clone());
                root.targets.iter().map(move |hash| CommitId {
                    root: path.clone(),
                    hash: hash.clone(),
                })
            })
            .collect()
    }
    /// Input is Log order. Revert keeps it; CherryPick::Prepared reverses each
    /// root exactly once. Root order is stable first appearance, not dependency order.
    pub fn read(ids: &[CommitId], kind: Kind, token: &Cancel) -> Result<Self, String> {
        if ids.is_empty() {
            return Err("No commits selected".into());
        }
        let groups = selection::group(ids, 1000)?;
        let mut roots = Vec::with_capacity(groups.len());
        for group in groups {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let repository = Repository::new(group.root.as_ref());
            let prepared = match kind {
                Kind::Revert => revert_operation::Batch::read(&repository, &group.hashes, token)
                    .map(Prepared::Revert),
                Kind::CherryPick => (|| {
                    let protected = repository
                        .pull_targets(token)?
                        .branches
                        .into_iter()
                        .filter(crate::published_commits::default_protected)
                        .map(|target| format!("refs/remotes/{}/{}", target.remote, target.branch))
                        .collect::<Vec<_>>();
                    cherry_pick::Prepared::read(&repository, &group.hashes, &protected, token)
                        .map(Prepared::CherryPick)
                })(),
            }
            .map_err(|error| format!("{}: {error}", repository.path.display()))?;
            roots.push(Root {
                repository,
                targets: group.hashes,
                prepared,
                result: None,
            });
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(Self { roots })
    }

    pub fn execute(
        mut self,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Results {
        for root in &mut self.roots {
            if token.cancelled() {
                break;
            }
            root.result = Some(match &root.prepared {
                Prepared::Revert(prepared) => prepared
                    .execute(&root.repository, editor, requests, token, progress)
                    .map(Report::Revert),
                Prepared::CherryPick(prepared) => cherry_pick::execute(
                    &root.repository,
                    prepared,
                    editor,
                    requests,
                    token,
                    progress,
                )
                .map(Report::CherryPick),
            });
            if !root.complete() {
                break;
            }
        }
        Results {
            roots: self.roots,
            cancelled: token.cancelled(),
        }
    }
}
