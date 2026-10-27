//! Repository operation markers based on Rebased GitRepositoryReader.
//! Conflict paths come from unmerged index entries, independently of operation type.
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    None,
    Merge,
    RebaseApply,
    RebaseMerge,
    CherryPick,
    Revert,
    Sequencer,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub operation: Operation,
    pub detached: bool,
    pub rebase_branch: Option<String>,
    pub conflicts: Vec<PathBuf>,
}

/// Cheap invalidation key for operation chrome, independent of branch refs.
/// Includes the per-worktree index because resolving conflicts need not move HEAD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    directory: PathBuf,
    markers: Vec<Option<MarkerStamp>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct MarkerStamp {
    directory: bool,
    len: u64,
    modified: Option<std::time::SystemTime>,
}
impl Observation {
    pub fn read(repo: &crate::git::Repository, token: &crate::git::Cancel) -> Result<Self, String> {
        let directory = repo.editor_directory(token)?;
        let mut markers = Vec::new();
        for name in [
            "HEAD",
            "index",
            "MERGE_HEAD",
            "rebase-apply",
            "rebase-apply/head-name",
            "rebase-merge",
            "rebase-merge/head-name",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "sequencer",
            "sequencer/todo",
        ] {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let value = match std::fs::symlink_metadata(directory.join(name)) {
                Ok(meta) => Some(MarkerStamp {
                    directory: meta.is_dir(),
                    len: meta.len(),
                    modified: meta.modified().ok(),
                }),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
            };
            markers.push(value);
        }
        Ok(Self { directory, markers })
    }
}

/// Only acknowledge stable reads. Failed or concurrent writes are retried by
/// the caller, never cached as a normal/no-operation state.
pub fn observe(
    repo: &crate::git::Repository,
    previous: Option<&Observation>,
    token: &crate::git::Cancel,
) -> Result<Option<(Observation, State)>, String> {
    let next = Observation::read(repo, token)?;
    if previous == Some(&next) {
        return Ok(None);
    }
    let state = repo.operation_state(token)?;
    if Observation::read(repo, token)? != next {
        return Err("Operation state changed while observing repository".into());
    }
    Ok(Some((next, state)))
}
impl State {
    pub fn in_progress(&self) -> bool {
        self.operation != Operation::None
    }
}
