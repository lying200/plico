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
#[derive(Debug)]
pub struct State {
    pub operation: Operation,
    pub detached: bool,
    pub rebase_branch: Option<String>,
    pub conflicts: Vec<PathBuf>,
}
impl State {
    pub fn in_progress(&self) -> bool {
        self.operation != Operation::None
    }
}
