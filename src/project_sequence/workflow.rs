//! Original multi-root apply workflow, including retained drafts.
use super::*;
use crate::sequencer_operation::{EmptyChoice, EmptyCommit};

/// The original conflict interaction owns the entire workflow, including drafts.
/// Closing it drops continuation authority, but never removes retained Git stashes.
pub enum Workflow {
    Ordinary(Results),
    Saved(SavedResults),
}
impl Workflow {
    pub fn operation(&self) -> Option<Kind> {
        let stopped = self.results().roots.iter().find(|root| !root.complete())?;
        match stopped.result.as_ref()?.as_ref().ok()? {
            Report::Revert(_) => Some(Kind::Revert),
            Report::CherryPick(_) => Some(Kind::CherryPick),
        }
    }
    pub fn results(&self) -> &Results {
        match self {
            Self::Ordinary(results) => results,
            Self::Saved(saved) => &saved.results,
        }
    }
    pub fn results_mut(&mut self) -> &mut Results {
        match self {
            Self::Ordinary(results) => results,
            Self::Saved(saved) => &mut saved.results,
        }
    }
    /// In-memory eligibility only. Resume revalidates Git state under the
    /// operation guards; merely displaying this predicate grants no authority.
    pub fn resumable(&self) -> bool {
        if self.results().cancelled || self.operation().is_none() {
            return false;
        }
        match self {
            Self::Ordinary(_) => true,
            Self::Saved(saved) => saved.resumable(),
        }
    }
    pub fn complete(&self) -> bool {
        match self {
            Self::Ordinary(results) => results.complete(),
            Self::Saved(saved) => saved.complete(),
        }
    }
    pub fn resume(
        &mut self,
        editor: &std::ffi::OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        let operation = self.operation().ok_or("No stopped operation to continue")?;
        match (operation, self) {
            (Kind::Revert, Self::Ordinary(results)) => {
                results.resume_revert(editor, requests, token, progress)
            }
            (Kind::Revert, Self::Saved(saved)) => {
                saved.resume_revert(editor, requests, token, progress)
            }
            (Kind::CherryPick, Self::Ordinary(results)) => {
                results.resume_cherry_pick(editor, requests, token, progress)
            }
            (Kind::CherryPick, Self::Saved(saved)) => {
                saved.resume_cherry_pick(editor, requests, token, progress)
            }
        }
    }
    pub fn resolve_empty(
        &mut self,
        empty: &EmptyCommit,
        choice: EmptyChoice,
        editor: &std::ffi::OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        match self {
            Self::Ordinary(results) => {
                results.resolve_empty((empty, choice), editor, requests, token, progress)
            }
            Self::Saved(saved) => {
                saved.resolve_empty((empty, choice), editor, requests, token, progress)
            }
        }
    }
}
