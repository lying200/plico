//! Rebased GitPreservingProcess / GitStashChangesSaver / GitUpdateProcess behavior.
//! Reference 4aec35256d765c217aa0d662b82ac4e860c396fd, Apache-2.0.
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
    operation_queue::Guard,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateResult {
    NothingToUpdate,
    Success,
    ResolvedConflicts,
    Incomplete,
    Cancelled,
    Error,
    NotReady,
}
impl UpdateResult {
    /// Actual GitUpdateProcess autoLoadDecision, not its older method comment.
    fn restore(self) -> bool {
        matches!(
            self,
            Self::NothingToUpdate | Self::Success | Self::ResolvedConflicts
        )
    }
}

pub struct Session<'a> {
    repo: &'a Repository,
    _guard: Guard,
    saved: Option<String>,
    save_attempted: bool,
    save_succeeded: bool,
    load_attempted: bool,
    rebase_started: bool,
    merge_started: bool,
}
pub struct FastForward {
    pub outcome: Outcome,
    pub before: String,
    /// A metadata-read failure must not turn an already completed merge into a
    /// reason to repeat the command. Callers refresh separately and report it.
    pub after: Result<String, String>,
}
pub struct RebaseStep {
    pub outcome: Outcome,
    pub state: Result<crate::operation_state::State, String>,
    pub cancellation_abort: Option<Result<Outcome, String>>,
}
pub struct MergeStep {
    pub outcome: Outcome,
    pub state: Result<crate::operation_state::State, String>,
    pub cancellation_reset: Option<Result<Outcome, String>>,
}
impl MergeStep {
    pub fn result(&self) -> UpdateResult {
        if self.outcome.stopped == Some(operation_process::StopReason::Cancelled) {
            return UpdateResult::Cancelled;
        }
        if self.outcome.stopped.is_some() {
            return UpdateResult::Error;
        }
        let Ok(state) = &self.state else {
            return UpdateResult::Error;
        };
        if !state.conflicts.is_empty()
            || self
                .outcome
                .events
                .contains(crate::operation_events::Event::MergeConflict)
        {
            return UpdateResult::Incomplete;
        }
        if self.outcome.status.success() {
            if state.in_progress() {
                UpdateResult::Incomplete
            } else {
                UpdateResult::Success
            }
        } else {
            UpdateResult::Error
        }
    }
}
impl<'a> Session<'a> {
    /// Owns the common-dir queue through save, update and restoration.
    /// Callers must not call another locking operation (e.g. fetch) inside it.
    pub fn begin(repo: &'a Repository, cancel: &Cancel) -> Result<Self, String> {
        Ok(Self {
            repo,
            _guard: repo.operation_guard(cancel)?,
            saved: None,
            save_attempted: false,
            save_succeeded: false,
            load_attempted: false,
            rebase_started: false,
            merge_started: false,
        })
    }
    pub fn saved_oid(&self) -> Option<&str> {
        self.saved.as_deref()
    }

    /// Low-level GitRebaser start, after the updater's published-commit decision.
    /// `editor` must be an application-owned Git editor command, never repository
    /// content. The UI editor/confirmation integration is a separate required step.
    pub fn rebase(
        &mut self,
        target: &crate::pull::Target,
        editor: &std::ffi::OsStr,
        cancel: &Cancel,
        progress: &Progress,
    ) -> Result<RebaseStep, String> {
        self.rebase_with_requests(target, editor, cancel, progress, None)
    }

    pub fn rebase_with_requests(
        &mut self,
        target: &crate::pull::Target,
        editor: &std::ffi::OsStr,
        cancel: &Cancel,
        progress: &Progress,
        requests: Option<&crate::auth_request::Requests>,
    ) -> Result<RebaseStep, String> {
        let targets = self.repo.pull_targets(cancel)?;
        if targets.current_branch.is_none() || !targets.contains(&target.remote, &target.branch) {
            return Err("Update requires an attached branch and an existing remote branch".into());
        }
        let reference = format!("refs/remotes/{}/{}", target.remote, target.branch);
        self.start_rebase(&reference, editor, cancel, progress, requests)
    }

    /// Start an independent branch rebase using the commit captured by the
    /// caller's validated branch selection. Does not fetch or change tracking.
    /// The caller must revalidate its displayed HEAD and target before saving
    /// changes; this session holds the operation guard through execution.
    pub fn rebase_onto_commit(
        &mut self,
        commit: &str,
        editor: &std::ffi::OsStr,
        cancel: &Cancel,
        progress: &Progress,
        requests: Option<&crate::auth_request::Requests>,
    ) -> Result<RebaseStep, String> {
        if !crate::git::is_oid(commit) {
            return Err("Rebase requires a full target commit ID".into());
        }
        if crate::squash_check::symbolic_head(self.repo, cancel)?.is_none() {
            return Err("Rebase requires an attached branch".into());
        }
        self.start_rebase(commit, editor, cancel, progress, requests)
    }

    fn start_rebase(
        &mut self,
        reference: &str,
        editor: &std::ffi::OsStr,
        cancel: &Cancel,
        progress: &Progress,
        requests: Option<&crate::auth_request::Requests>,
    ) -> Result<RebaseStep, String> {
        if self.rebase_started
            || self.merge_started
            || (self.save_attempted && !self.save_succeeded)
        {
            return Err("Update cannot start after a previous rebase or failed save".into());
        }
        let state = self.repo.operation_state(cancel)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("An unfinished Git operation prevents update".into());
        }
        if editor.is_empty() {
            return Err("Rebase editor is unavailable".into());
        }
        let mut command = self.repo.operation_command();
        command
            .args(["-c", "core.commentChar=\u{1}", "rebase", "--", reference])
            .env("GIT_EDITOR", editor);
        let server = requests
            .map(|requests| crate::askpass::Server::start(requests.clone()))
            .transpose()
            .map_err(|e| e.to_string())?;
        if let Some(server) = &server {
            server.configure(&mut command);
        }
        self.rebase_started = true;
        let outcome = operation_process::run(&mut command, cancel, progress, None)
            .map_err(|e| e.to_string())?;
        drop(server);
        let mut state = self.repo.operation_state(&Cancel::default());
        let cancellation_abort =
            if outcome.stopped == Some(operation_process::StopReason::Cancelled) {
                if state.as_ref().is_ok_and(is_rebasing) {
                    let aborted = self.abort_rebase(progress);
                    state = self.repo.operation_state(&Cancel::default());
                    Some(aborted)
                } else {
                    None
                }
            } else {
                None
            };
        Ok(RebaseStep {
            outcome,
            state,
            cancellation_abort,
        })
    }

    /// GitMergeUpdater.doUpdate after fetch/save decisions. Never use Pull here:
    /// merge the existing remote-tracking ref with --no-stat -v.
    pub fn merge(
        &mut self,
        target: &crate::pull::Target,
        editor: &std::ffi::OsStr,
        cancel: &Cancel,
        progress: &Progress,
    ) -> Result<MergeStep, String> {
        self.merge_with_requests(target, editor, cancel, progress, None)
    }

    pub fn merge_with_requests(
        &mut self,
        target: &crate::pull::Target,
        editor: &std::ffi::OsStr,
        cancel: &Cancel,
        progress: &Progress,
        requests: Option<&crate::auth_request::Requests>,
    ) -> Result<MergeStep, String> {
        if self.merge_started
            || self.rebase_started
            || (self.save_attempted && !self.save_succeeded)
        {
            return Err("Update already started or local changes could not be saved".into());
        }
        let targets = self.repo.pull_targets(cancel)?;
        if targets.current_branch.is_none() || !targets.contains(&target.remote, &target.branch) {
            return Err("Update requires an attached branch and an existing remote branch".into());
        }
        let state = self.repo.operation_state(cancel)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("An unfinished Git operation prevents update".into());
        }
        if editor.is_empty() {
            return Err("Merge editor is unavailable".into());
        }
        let reference = format!("refs/remotes/{}/{}", target.remote, target.branch);
        let mut command = self.repo.operation_command();
        command
            .args(["merge", "--no-stat", "-v", "--", &reference])
            .env("GIT_EDITOR", editor)
            .env("LC_ALL", "C")
            .env("LANG", "C");
        let server = requests
            .map(|requests| crate::askpass::Server::start(requests.clone()))
            .transpose()
            .map_err(|e| e.to_string())?;
        if let Some(server) = &server {
            server.configure(&mut command);
        }
        self.merge_started = true;
        let outcome = operation_process::run(&mut command, cancel, progress, None)
            .map_err(|e| e.to_string())?;
        drop(server);
        // GitMergeUpdater.cancel uses reset --merge, not reset --hard. Preserve
        // the reset outcome because cancellation can itself fail to unwind.
        let refresh = Cancel::default();
        let cancellation_reset =
            if outcome.stopped == Some(operation_process::StopReason::Cancelled) {
                Some(
                    operation_process::run(
                        self.repo.operation_command().args(["reset", "--merge"]),
                        &refresh,
                        progress,
                        None,
                    )
                    .map_err(|e| e.to_string()),
                )
            } else {
                None
            };
        let state = self.repo.operation_state(&refresh);
        Ok(MergeStep {
            outcome,
            state,
            cancellation_reset,
        })
    }

    /// GitRebaseUpdater cancels by aborting its rebase; this does not restore stash.
    pub fn abort_rebase(&mut self, progress: &Progress) -> Result<Outcome, String> {
        if !self.rebase_started {
            return Err("No rebase was started by this session".into());
        }
        let token = Cancel::default();
        if !is_rebasing(&self.repo.operation_state(&token)?) {
            return Err("Repository is not rebasing".into());
        }
        operation_process::run(
            self.repo.operation_command().args(["rebase", "--abort"]),
            &token,
            progress,
            None,
        )
        .map_err(|e| e.to_string())
    }

    /// GitRebaser.continueRebase command step. Unlike initial update cancellation,
    /// cancelled continuation leaves the rebase available to the user.
    /// No-change recovery follows GitRebaser (stage tracked files / skip).
    pub fn continue_rebase(
        &mut self,
        editor: &std::ffi::OsStr,
        cancel: &Cancel,
        progress: &Progress,
    ) -> Result<RebaseStep, String> {
        self.continue_rebase_with_requests(editor, cancel, progress, None)
    }

    pub fn continue_rebase_with_requests(
        &mut self,
        editor: &std::ffi::OsStr,
        cancel: &Cancel,
        progress: &Progress,
        requests: Option<&crate::auth_request::Requests>,
    ) -> Result<RebaseStep, String> {
        if !self.rebase_started || editor.is_empty() {
            return Err("No session rebase/editor available".into());
        }
        let state = self.repo.operation_state(cancel)?;
        if !is_rebasing(&state) || !state.conflicts.is_empty() {
            return Err("Resolve and stage rebase conflicts before continuing".into());
        }
        let outcome = crate::rebase_operation::continue_with_recovery(
            self.repo, editor, requests, cancel, progress,
        )?;
        let state = self.repo.operation_state(&Cancel::default());
        Ok(RebaseStep {
            outcome,
            state,
            cancellation_abort: None,
        })
    }

    /// GitRebaseUpdater.fastForwardMerge. The updater invokes this only for local
    /// tracked changes, before saving; it is not the general Pull strategy.
    /// Failed Git exit permits the regular update path, cancellation does not.
    pub fn try_fast_forward(
        &mut self,
        target: &crate::pull::Target,
        cancel: &Cancel,
        progress: &Progress,
    ) -> Result<FastForward, String> {
        if self.save_attempted {
            return Err("Fast-forward optimization must precede saving changes".into());
        }
        let targets = self.repo.pull_targets(cancel)?;
        if targets.current_branch.is_none() || !targets.contains(&target.remote, &target.branch) {
            return Err("Update requires an attached branch and an existing remote branch".into());
        }
        let state = self.repo.operation_state(cancel)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("An unfinished Git operation prevents update".into());
        }
        let before = self.repo.text(&["rev-parse", "--verify", "HEAD"], cancel)?;
        let reference = format!("refs/remotes/{}/{}", target.remote, target.branch);
        let mut command = self.repo.operation_command();
        // Full ref + option delimiter avoid ambiguity with a local branch name.
        command.args(["merge", "--ff-only", "--", &reference]);
        let outcome = operation_process::run(&mut command, cancel, progress, None)
            .map_err(|e| e.to_string())?;
        let after = self
            .repo
            .text(&["rev-parse", "--verify", "HEAD"], &Cancel::default());
        Ok(FastForward {
            outcome,
            before,
            after,
        })
    }

    /// Caller decides whether saving is needed, after the fast-forward optimization.
    /// Once started, the save section is non-cancellable like GitPreservingProcess.
    pub fn save(
        &mut self,
        message: &str,
        cancel: &Cancel,
        progress: &Progress,
    ) -> Result<Outcome, String> {
        if self.save_attempted {
            return Err("Local changes save already attempted".into());
        }
        if cancel.cancelled() {
            return Err("Cancelled".into());
        }
        if message.contains(['\0', '\r', '\n']) {
            return Err("Invalid save message".into());
        }
        let token = Cancel::default();
        let before = self.repo.stash_head(&token)?;
        self.save_attempted = true;
        let outcome = operation_process::run(
            self.repo
                .operation_command()
                .args(["stash", "push", "-m", message]),
            &token,
            progress,
            None,
        )
        .map_err(|e| e.to_string());
        // Inspect even a failed save: Git may already have written a recoverable stash.
        let after = self.repo.stash_head(&token)?;
        if after != before {
            self.saved = after;
        }
        let outcome = outcome?;
        self.save_succeeded = outcome.status.success();
        Ok(outcome)
    }

    /// Only successful update outcomes auto-load. Failure/unfinished update keeps
    /// the stash available. Drop never runs commands or silently rolls back.
    /// This performs one restoration attempt even when it conflicts, as upstream.
    pub fn finish(
        &mut self,
        result: UpdateResult,
        reinstate_index: bool,
        progress: &Progress,
    ) -> Result<Option<Outcome>, String> {
        if !result.restore() {
            return Ok(None);
        }
        self.restore_saved(reinstate_index, progress)
    }

    /// GitPreservingProcess without an autoLoadDecision (Smart Checkout):
    /// restore after the attempted operation, even when it failed/cancelled.
    pub(crate) fn restore_saved(
        &mut self,
        reinstate_index: bool,
        progress: &Progress,
    ) -> Result<Option<Outcome>, String> {
        if !self.save_succeeded || self.saved.is_none() || self.load_attempted {
            return Ok(None);
        }
        self.load_attempted = true;
        let token = Cancel::default();
        // Local adaptation: do not pop an unrelated stash created by another app.
        // This is a preflight check, not a cross-process atomic lock.
        if self.repo.stash_head(&token)?.as_deref() != self.saved_oid() {
            return Err(
                "Saved changes are no longer the top stash; automatic restore stopped".into(),
            );
        }
        let state = self.repo.operation_state(&token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the Git operation before restoring saved changes".into());
        }
        crate::conflicts::clear_saved_context(self.repo, &token)?;
        let mut command = self.repo.operation_command();
        command.args(["stash", "pop"]);
        if reinstate_index {
            command.arg("--index");
        }
        let outcome =
            operation_process::run(&mut command, &token, progress, None).map_err(|e| e.to_string());
        crate::conflicts::record_saved_context(self.repo)?;
        let outcome = outcome?;
        if outcome.status.success() {
            self.saved = None;
        }
        Ok(Some(outcome))
    }
}

fn is_rebasing(state: &crate::operation_state::State) -> bool {
    matches!(
        state.operation,
        crate::operation_state::Operation::RebaseApply
            | crate::operation_state::Operation::RebaseMerge
    )
}
