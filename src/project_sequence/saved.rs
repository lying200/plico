//! GitApplyChangesProcess: save every participating root before retrying, and
//! restore only after the whole operation succeeds. Reuse the update saver.
use super::*;
use crate::sequencer_operation::{EmptyChoice, EmptyCommit};
use crate::{
    operation_queue,
    project_update::preservation::{SavedChanges, SavedRoot},
    update_session::{Session, UpdateResult},
};

pub struct SavedResults {
    pub results: Results,
    pub saved: Vec<SavedRoot>,
    pub error: Option<String>,
    directories: Vec<std::path::PathBuf>,
}
impl SavedResults {
    /// Eligibility of the original workflow; on-disk identity and conflict
    /// guards are still checked by resume before any write.
    pub fn resumable(&self) -> bool {
        self.error.is_none() && self.saved.iter().all(|root| root.restore.is_none())
    }

    /// Read on a worker: distinguish restoration conflicts from an unfinished
    /// apply operation, without consuming the exact retained stash reports.
    pub fn restore_conflict(&self, token: &Cancel) -> Option<Repository> {
        self.saved
            .iter()
            .filter(|root| root.restore.is_some())
            .find_map(|root| {
                let repository = Repository::new(&root.path);
                repository
                    .operation_state(token)
                    .is_ok_and(|state| !state.conflicts.is_empty())
                    .then_some(repository)
            })
    }

    /// Continue only the original conflict interaction. Retained drafts belong
    /// to the whole workflow, including roots already completed before it stopped.
    pub fn resume_revert(
        &mut self,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        self.resume(Kind::Revert, None, editor, requests, token, progress)
    }

    pub fn resume_cherry_pick(
        &mut self,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        self.resume(Kind::CherryPick, None, editor, requests, token, progress)
    }

    pub fn resolve_empty(
        &mut self,
        decision: (&EmptyCommit, EmptyChoice),
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        self.resume(
            Kind::CherryPick,
            Some(decision),
            editor,
            requests,
            token,
            progress,
        )
    }

    fn resume(
        &mut self,
        kind: Kind,
        decision: Option<(&EmptyCommit, EmptyChoice)>,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        if !self.resumable() {
            return Err("Review the saved operation before continuing".into());
        }
        self.validate_directories(token)?;
        match (kind, decision) {
            (Kind::CherryPick, Some(decision)) => self
                .results
                .resolve_empty(decision, editor, requests, token, progress)?,
            (Kind::Revert, _) => self
                .results
                .resume_revert(editor, requests, token, progress)?,
            (Kind::CherryPick, None) => self
                .results
                .resume_cherry_pick(editor, requests, token, progress)?,
        }
        if !self.results.complete() {
            return Ok(());
        }
        let repositories = self
            .results
            .roots
            .iter()
            .map(|root| root.repository.clone())
            .collect::<Vec<_>>();
        let keys = repositories
            .iter()
            .map(|repo| repo.operation_key(token))
            .collect::<Result<Vec<_>, _>>()?;
        let guard = operation_queue::acquire_many(keys, token)?;
        self.validate_directories(token)?;
        // Shared worktree stash stacks must be unwound in reverse save order.
        // Each restore attempts once; conflicts retain the stash for explicit recovery.
        for (repo, saved) in repositories.iter().zip(&mut self.saved).rev() {
            let mut session =
                Session::retained_under_guard(repo, &guard, saved.saved_oid.clone(), token)?;
            saved.restore = Some(session.finish(UpdateResult::Success, true, progress));
            saved.saved_oid = session.saved_oid().map(str::to_owned);
        }
        Ok(())
    }

    fn validate_directories(&self, token: &Cancel) -> Result<(), String> {
        if self.directories.len() != self.results.roots.len()
            || self.saved.len() != self.results.roots.len()
        {
            return Err("Saved operation roots changed".into());
        }
        for ((root, saved), directory) in self
            .results
            .roots
            .iter()
            .zip(&self.saved)
            .zip(&self.directories)
        {
            if root.repository.path != saved.path
                || root.repository.editor_directory(token)? != *directory
            {
                return Err("Saved operation repository changed".into());
            }
        }
        Ok(())
    }

    /// Successful Git commands alone do not prove local changes were restored.
    pub fn complete(&self) -> bool {
        self.error.is_none()
            && self.results.complete()
            && self.saved.iter().all(|root| {
                root.saved_oid.is_none()
                    && root.restore.as_ref().is_some_and(|result| {
                        result.as_ref().is_ok_and(|output| {
                            output.as_ref().is_none_or(|output| {
                                output.status.success()
                                    && output.stopped.is_none()
                                    && output.input_error.is_none()
                            })
                        })
                    })
            })
    }
}

impl Plan {
    /// Explicitly authorized save-and-retry. All roots are reserved together;
    /// no nested acquisition may deadlock a linked worktree's common-dir.
    /// Errors after saving are returned with the exact retained stash IDs.
    pub fn save_and_execute(
        mut self,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<SavedResults, String> {
        if editor.is_empty() {
            return Err("Commit editor is unavailable".into());
        }
        let repositories = self
            .roots
            .iter()
            .map(|root| root.repository.clone())
            .collect::<Vec<_>>();
        let keys = repositories
            .iter()
            .map(|repo| repo.operation_key(token))
            .collect::<Result<Vec<_>, _>>()?;
        let guard = operation_queue::acquire_many(keys, token)?;
        // Reject stale preparations before saving any user's working tree.
        for root in &self.roots {
            match &root.prepared {
                Prepared::CherryPick(prepared) => prepared.validate(&root.repository, token),
                Prepared::Revert(prepared) => prepared.validate(&root.repository, token),
            }
            .map_err(|error| format!("{}: {error}", root.repository.path.display()))?;
        }
        let directories = repositories
            .iter()
            .map(|repo| repo.editor_directory(token))
            .collect::<Result<Vec<_>, _>>()?;
        let mut saved = SavedChanges::new(&repositories, &guard, token)?;
        let error = saved
            .save_all("Plico: before applying commits", token, progress)
            .err();
        if error.is_none() {
            for root in &mut self.roots {
                if token.cancelled() {
                    break;
                }
                root.result = Some(match &root.prepared {
                    Prepared::CherryPick(prepared) => cherry_pick::execute_under_guard(
                        &root.repository,
                        prepared,
                        &guard,
                        editor,
                        requests,
                        token,
                        progress,
                    )
                    .map(Report::CherryPick),
                    Prepared::Revert(prepared) => prepared
                        .execute_under_guard(
                            &root.repository,
                            &guard,
                            editor,
                            requests,
                            token,
                            progress,
                        )
                        .map(Report::Revert),
                });
                if !root.complete() {
                    break;
                }
            }
        }
        let results = Results {
            roots: self.roots,
            cancelled: token.cancelled(),
        };
        saved.finish(
            if error.is_none() && results.complete() {
                UpdateResult::Success
            } else {
                UpdateResult::Incomplete
            },
            true,
            progress,
        );
        Ok(SavedResults {
            results,
            saved: saved.into_reports(),
            directories,
            error,
        })
    }
}
