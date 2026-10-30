//! Read-only operation chrome shares a freshly resolved worktree directory.
//! Rebased 2450423: GitRepositoryFiles and GitRepositoryImpl::readRepoInfo.
//! These values never authorize a write or survive as a directory cache.
use crate::{
    git::{Cancel, Repository},
    operation_state::State,
};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryPresence {
    pub merge_saved: bool,
    pub revert_queue: bool,
    pub revert_saved: bool,
}
impl RecoveryPresence {
    // Unreadable records must continue to block unsafe actions, as before.
    const UNKNOWN: Self = Self {
        merge_saved: true,
        revert_queue: true,
        revert_saved: true,
    };
    pub fn read(repo: &Repository, token: &Cancel) -> Self {
        match repo.editor_directory(token) {
            Ok(directory) => Self::read_at(&directory, token),
            Err(_) => Self::UNKNOWN,
        }
    }
    fn read_at(directory: &Path, token: &Cancel) -> Self {
        let presence = Self {
            merge_saved: crate::merge_saved::read(directory)
                .map(|r| r.is_some())
                .unwrap_or(true),
            revert_queue: crate::revert_operation::Recovery::record_exists_at(directory)
                .unwrap_or(true),
            revert_saved: crate::revert_operation::SavedChanges::record_exists_at(directory)
                .unwrap_or(true),
        };
        if token.cancelled() {
            Self::UNKNOWN
        } else {
            presence
        }
    }
    pub fn flags(self) -> (bool, bool, bool) {
        (self.merge_saved, self.revert_queue, self.revert_saved)
    }
}

pub struct Inspection {
    pub state: Result<State, String>,
    pub recovery: RecoveryPresence,
}
impl Inspection {
    pub fn read(repo: &Repository, token: &Cancel) -> Self {
        let directory = match repo.editor_directory(token) {
            Ok(directory) => directory,
            Err(error) => return Self::unknown(error),
        };
        let state = repo.operation_state_in(&directory, token);
        let recovery = RecoveryPresence::read_at(&directory, token);
        // Git commands still resolve through the repository root. Reject a
        // changed .git indirection rather than publish mixed worktree inputs.
        match repo.editor_directory(token) {
            Ok(current) if current == directory => Self { state, recovery },
            Ok(_) => {
                Self::unknown("Repository directory changed while inspecting operations".into())
            }
            Err(error) => Self::unknown(error),
        }
    }
    fn unknown(error: String) -> Self {
        Self {
            state: Err(error),
            recovery: RecoveryPresence::UNKNOWN,
        }
    }
}
