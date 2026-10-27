//! One save/restore boundary for all roots, following Rebased 4aec352
//! GitPreservingProcess.execute and GitUpdateProcess.autoLoadDecision.
use crate::{
    git::{Cancel, Repository},
    operation_process::{Outcome, Progress},
    operation_queue::Guard,
    update_session::{Session, UpdateResult},
};
use std::path::PathBuf;

/// Each root remains independently recoverable from its own Git stash. Redact
/// command diagnostics before display, as for other operation reports.
pub struct SavedRoot {
    pub path: PathBuf,
    pub save: Result<Outcome, String>,
    pub restore: Option<Result<Option<Outcome>, String>>,
    pub saved_oid: Option<String>,
}

pub(crate) struct SavedChanges<'a> {
    repositories: &'a [Repository],
    sessions: Vec<Session<'a>>,
    reports: Vec<SavedRoot>,
    saved: bool,
    save_succeeded: bool,
    finished: bool,
}
impl<'a> SavedChanges<'a> {
    /// Construct every session before the first mutation, under one reservation.
    pub(crate) fn new(
        repositories: &'a [Repository],
        guard: &'a Guard,
        token: &Cancel,
    ) -> Result<Self, String> {
        let sessions = repositories
            .iter()
            .map(|repo| Session::under_guard(repo, guard, token))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            repositories,
            sessions,
            reports: Vec::new(),
            saved: false,
            save_succeeded: false,
            finished: false,
        })
    }

    /// The whole save section is non-cancellable, including transitions between
    /// repositories. A failure keeps earlier stash IDs and prevents updating.
    pub(crate) fn save_all(
        &mut self,
        title: &str,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        if self.saved {
            return Err("Local changes have already been saved for this update".into());
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        self.saved = true;
        let saving = Cancel::default();
        for (session, repository) in self.sessions.iter_mut().zip(self.repositories) {
            let save = session.save(title, &saving, progress);
            let success = save.as_ref().is_ok_and(|out| {
                out.status.success() && out.stopped.is_none() && out.input_error.is_none()
            });
            self.reports.push(SavedRoot {
                path: repository.path.clone(),
                save,
                restore: None,
                saved_oid: session.saved_oid().map(str::to_owned),
            });
            if !success {
                return Err("Could not save local changes; update was not started".into());
            }
        }
        self.save_succeeded = true;
        Ok(())
    }

    /// Call once with the *whole* update result, never with an individual root's
    /// success. Failed/incomplete/cancelled updates leave every stash untouched.
    pub(crate) fn finish(
        &mut self,
        result: UpdateResult,
        reinstate_index: bool,
        progress: &Progress,
    ) {
        if self.finished {
            return;
        }
        self.finished = true;
        let restore = self.save_succeeded
            && matches!(
                result,
                UpdateResult::Success
                    | UpdateResult::NothingToUpdate
                    | UpdateResult::ResolvedConflicts
            );
        // Linked worktrees share refs/stash. Restore in reverse save order so
        // each session's exact stash is at the top; keep report presentation in
        // original root order. A failed pop remains retained and later sessions
        // still enforce the top-stash identity guard instead of popping it.
        for (session, report) in self.sessions.iter_mut().zip(&mut self.reports).rev() {
            if restore {
                report.restore = Some(session.finish(result, reinstate_index, progress));
            }
            report.saved_oid = session.saved_oid().map(str::to_owned);
        }
    }

    pub(crate) fn into_reports(self) -> Vec<SavedRoot> {
        self.reports
    }
}
