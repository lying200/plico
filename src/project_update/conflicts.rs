//! Conflict writes inside GitUpdateProcess's still-owned preservation session.
//! The worker lends this interaction only while its current updater is paused.
pub mod channel;

use crate::{
    conflicts::{File, Resolution, Snapshot},
    git::{Cancel, Repository},
    merge_result::{self, Prepared},
    operation_process::Progress,
    operation_queue::Guard,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Close the resolver, leaving this root incomplete; update the next root.
    Close,
    /// All selected results were saved/staged. Recheck Git before continuing.
    Continue,
}

/// A scoped capability, not a cloneable repository with locking disabled.
/// It cannot outlive the group's reservation or grant access to another root.
/// UI adapters send commands to this worker; they never receive the guard.
pub struct Interaction<'a> {
    pub(crate) repository: &'a Repository,
    pub(crate) guard: &'a Guard,
    pub(crate) token: &'a Cancel,
    pub(crate) progress: &'a Progress,
}
impl Interaction<'_> {
    pub fn repository(&self) -> &Repository {
        self.repository
    }

    pub fn snapshot(&self) -> Result<Snapshot, String> {
        Snapshot::read(self.repository, self.token)
    }

    pub fn resolve(
        &self,
        snapshot: &Snapshot,
        file: &File,
        resolution: Resolution,
    ) -> Result<Snapshot, String> {
        let report = crate::conflicts::resolve_reserved(
            self.repository,
            self.guard,
            snapshot,
            file,
            resolution,
            self.token,
            self.progress,
            || Ok(()),
        )?;
        merge_result::checked_report(report, &file.path)
    }

    pub fn prepare(&self, snapshot: &Snapshot, file: &File) -> Result<Prepared, String> {
        Prepared::read(self.repository, snapshot, file, self.token)
    }

    /// Save an accepted editor result without prematurely marking it resolved.
    pub fn apply(&self, prepared: &Prepared) -> Result<(), String> {
        prepared.apply_reserved(self.repository, self.guard, self.token)
    }

    pub fn saved_text(&self, prepared: &Prepared) -> Result<String, String> {
        prepared.saved_text_reserved(self.repository, self.guard, self.token)
    }

    pub fn resolve_automatically(
        &self,
        prepared: &Prepared,
        saved: Option<&crate::text_merge::State>,
    ) -> Result<Option<merge_result::AutomaticResult>, String> {
        prepared.resolve_automatically_reserved(self.repository, self.guard, saved, self.token)
    }

    pub fn stage_resolved(&self, drafts: &[std::sync::Arc<Prepared>]) -> merge_result::StageResult {
        merge_result::stage_resolved_with(drafts, self.token, |draft, repository| {
            if repository.path != self.repository.path {
                return Err("Merge result does not belong to this update interaction".into());
            }
            draft.stage_reserved(self.repository, self.guard, self.token, self.progress)
        })
    }

    pub fn stage(&self, prepared: &Prepared) -> Result<Snapshot, String> {
        let report =
            prepared.stage_reserved(self.repository, self.guard, self.token, self.progress)?;
        merge_result::checked_report(report, &prepared.file().path)
    }
}
