//! Ordinary commit Revert, translated from Rebased 4aec352 GitRevertAction,
//! GitRevertProcess and GitImpl.revert (JetBrains and contributors, Apache-2.0).
mod recovery_record;
mod save_retry;
pub use recovery_record::Pending;
pub use save_retry::{SavedChanges, SavedPending, SavedReport};

use crate::{
    auth_request::Requests,
    git::{Cancel, Repository, is_oid},
    operation_process::{self, Outcome, Progress},
    operation_state::State,
};
use std::{ffi::OsStr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Prepared {
    pub target: String,
    pub subject: String,
    head: String,
    branch: Option<String>,
    directory: PathBuf,
}
impl Prepared {
    pub fn read(repo: &Repository, target: &str, token: &Cancel) -> Result<Self, String> {
        if !is_oid(target) {
            return Err("Invalid commit identity".into());
        }
        let head =
            crate::commit_operation::head(repo, token)?.ok_or("Cannot revert without HEAD")?;
        let branch = crate::squash_check::symbolic_head(repo, token)?;
        let directory = repo.editor_directory(token)?;
        let metadata = repo.text(
            &[
                "show",
                "-s",
                "--no-show-signature",
                "--encoding=UTF-8",
                "--format=%P%x00%s",
                target,
                "--",
            ],
            token,
        )?;
        let (parents, subject) = metadata.split_once('\0').ok_or("Invalid commit metadata")?;
        if parents.split_whitespace().count() > 1 {
            return Err("Cannot revert a merge commit".into());
        }
        let prepared = Self {
            target: target.into(),
            subject: subject.trim_end_matches('\n').into(),
            head,
            branch,
            directory,
        };
        prepared.validate(repo, token)?;
        Ok(prepared)
    }
    fn validate(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.directory
            || crate::commit_operation::head(repo, token)?.as_ref() != Some(&self.head)
            || crate::squash_check::symbolic_head(repo, token)? != self.branch
        {
            return Err("Repository changed; review the revert again".into());
        }
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before reverting a commit".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    NothingToRevert,
    LocalChanges,
    Conflicts,
    Cancelled,
    Failed,
}
pub struct Report {
    pub command: Result<Outcome, String>,
    pub state: Result<State, String>,
    pub head: Result<Option<String>, String>,
    pub status: Status,
}
/// Does not stage local changes or silently save/retry on overwrite errors.
/// The caller services the normal editor/auth bridge, and always refreshes.
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    editor: &OsStr,
    requests: Option<&Requests>,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let _guard = repo.operation_guard(token)?;
    if SavedChanges::read(repo, token)?.is_some() {
        return Err("Handle saved Revert changes before starting another revert".into());
    }
    execute_locked(repo, prepared, editor, requests, token, progress)
}

fn execute_locked(
    repo: &Repository,
    prepared: &Prepared,
    editor: &OsStr,
    requests: Option<&Requests>,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    prepared.validate(repo, token)?;
    if editor.is_empty() {
        return Err("Commit editor is unavailable".into());
    }
    let server = requests
        .map(|r| crate::askpass::Server::start(r.clone()))
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut command = repo.operation_command();
    command
        .args(["revert", &prepared.target])
        .env("GIT_EDITOR", editor);
    if let Some(server) = &server {
        server.configure(&mut command);
    }
    let command =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    drop(server);
    let state = repo.operation_state(&Cancel::default());
    let head = crate::commit_operation::head(repo, &Cancel::default());
    let status = match (&command, &state, &head) {
        (Ok(output), _, _) if output.stopped.is_some() => Status::Cancelled,
        (_, Ok(state), _) if !state.conflicts.is_empty() => Status::Conflicts,
        (Ok(output), Ok(state), Ok(Some(head)))
            if !state.in_progress() && output.input_error.is_none() =>
        {
            if output.status.success() && head != &prepared.head {
                Status::Success
            } else if !output.status.success()
                && head == &prepared.head
                && output
                    .events
                    .contains(crate::operation_events::Event::ApplyChangesWouldBeOverwritten)
                && !output
                    .events
                    .contains(crate::operation_events::Event::UntrackedFilesWouldBeOverwritten)
            {
                Status::LocalChanges
            } else {
                let stdout = String::from_utf8_lossy(&output.stdout.bytes()).into_owned();
                if !output.status.success()
                    && head == &prepared.head
                    && (stdout.contains("nothing to commit")
                        || stdout.contains("nothing added to commit but untracked files present"))
                {
                    Status::NothingToRevert
                } else {
                    Status::Failed
                }
            }
        }
        _ => Status::Failed,
    };
    Ok(Report {
        command,
        state,
        head,
        status,
    })
}

/// Ordered exactly as the visible Log selection (newest first). Validate the
/// whole selection before the first write, including commits after a conflict.
#[derive(Clone, Debug)]
pub struct Batch {
    commits: Vec<Prepared>,
}
pub struct BatchReport {
    pub successful: Vec<String>,
    pub skipped: Vec<String>,
    /// Includes the stopped commit, followed by untouched commits, in UI order.
    pub remaining: Vec<String>,
    pub last: Option<Report>,
    pub error: Option<String>,
    pub recovery: Option<Recovery>,
    /// Remaining commits bound to the HEAD reached by this attempt. The UI may
    /// offer an explicit save/retry; never reconstruct from the original selection.
    pub retry: Option<Batch>,
}
impl Batch {
    pub fn targets(&self) -> impl Iterator<Item = &str> {
        self.commits.iter().map(|p| p.target.as_str())
    }

    pub fn read(repo: &Repository, targets: &[String], token: &Cancel) -> Result<Self, String> {
        if targets.is_empty() {
            return Err("No commits selected".into());
        }
        // Rebased VcsLogUtil.MAX_SELECTED_COMMITS. Never silently truncate a write selection.
        if targets.len() > 1000 {
            return Err("Select at most 1000 commits to revert".into());
        }
        let mut seen = std::collections::HashSet::new();
        let mut commits: Vec<Prepared> = Vec::with_capacity(targets.len());
        for target in targets {
            if !seen.insert(target) {
                return Err("Duplicate commit selection".into());
            }
            let prepared = Prepared::read(repo, target, token)?;
            if let Some(first) = commits.first() {
                if first.head != prepared.head
                    || first.branch != prepared.branch
                    || first.directory != prepared.directory
                {
                    return Err("Repository changed; review the revert again".into());
                }
            }
            commits.push(prepared);
        }
        Ok(Self { commits })
    }
    pub fn execute(
        &self,
        repo: &Repository,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<BatchReport, String> {
        let _guard = repo.operation_guard(token)?;
        if SavedChanges::read(repo, token)?.is_some() {
            return Err("Handle saved Revert changes before starting another revert".into());
        }
        self.execute_locked(repo, editor, requests, token, progress)
    }
    fn execute_locked(
        &self,
        repo: &Repository,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<BatchReport, String> {
        if Recovery::record_exists(repo, token)? {
            return Err("Review the saved revert queue before starting another revert".into());
        }
        let first = self.commits.first().ok_or("No commits selected")?;
        first.validate(repo, token)?;
        let mut expected_head = first.head.clone();
        let mut report = BatchReport {
            successful: vec![],
            skipped: vec![],
            remaining: self.commits.iter().map(|c| c.target.clone()).collect(),
            last: None,
            error: None,
            recovery: None,
            retry: None,
        };
        for original in &self.commits {
            let mut prepared = original.clone();
            prepared.head = expected_head.clone();
            let result = match execute_locked(repo, &prepared, editor, requests, token, progress) {
                Ok(result) => result,
                Err(error) => {
                    report.error = Some(error);
                    break;
                }
            };
            let advance = match result.status {
                Status::Success => {
                    expected_head = result
                        .head
                        .as_ref()
                        .ok()
                        .and_then(|h| h.clone())
                        .expect("successful revert has a verified HEAD");
                    report.successful.push(original.target.clone());
                    true
                }
                Status::NothingToRevert => {
                    report.skipped.push(original.target.clone());
                    true
                }
                _ => false,
            };
            report.last = Some(result);
            if !advance {
                break;
            }
            // Processed commits must never be included in a retry.
        }
        report
            .remaining
            .drain(..report.successful.len() + report.skipped.len());
        if report
            .last
            .as_ref()
            .is_some_and(|r| r.status == Status::LocalChanges)
        {
            let commits = self
                .commits
                .iter()
                .skip(report.successful.len() + report.skipped.len())
                .cloned()
                .map(|mut p| {
                    p.head = expected_head.clone();
                    p
                })
                .collect();
            report.retry = Some(Self { commits });
        }
        if report.last.as_ref().is_some_and(|r| {
            r.state
                .as_ref()
                .is_ok_and(|s| s.operation == crate::operation_state::Operation::Revert)
        }) {
            if let Some(stopped) = self
                .commits
                .iter()
                .find(|p| report.remaining.first() == Some(&p.target))
            {
                let mut expected = stopped.clone();
                expected.head = expected_head;
                report.recovery = Some(Recovery {
                    expected,
                    remaining: report.remaining.clone(),
                });
            }
        }
        if let Some(recovery) = &report.recovery {
            if let Err(error) = recovery.persist() {
                report.error = Some(format!("Cannot save the remaining revert queue: {error}"));
            }
        }
        Ok(report)
    }
}

/// Application-owned continuation for a sequence executed one commit at a time.
/// Valid only for the exact stopped Git operation, never inferred from a branch name.
#[derive(Clone, Debug)]
pub struct Recovery {
    expected: Prepared,
    remaining: Vec<String>,
}
impl Recovery {
    fn validate_stopped(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.expected.directory
            || crate::commit_operation::head(repo, token)?.as_ref() != Some(&self.expected.head)
            || crate::squash_check::symbolic_head(repo, token)? != self.expected.branch
            || repo
                .text(&["rev-parse", "--verify", "REVERT_HEAD"], token)?
                .trim()
                != self.expected.target
        {
            return Err("The stopped revert changed; refresh before continuing".into());
        }
        Ok(())
    }

    pub fn resume(
        &self,
        repo: &Repository,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<BatchReport, String> {
        let _guard = repo.operation_guard(token)?;
        self.validate_stopped(repo, token)?;
        self.validate_record()?;
        let saved = SavedChanges::for_recovery(repo, &self.expected, token)?;
        let mut report = self.resume_locked(repo, editor, requests, token, progress)?;
        if report.remaining.is_empty() && report.error.is_none() {
            if let Some(saved) = saved {
                let head = report
                    .last
                    .as_ref()
                    .and_then(|r| r.head.as_ref().ok())
                    .and_then(|h| h.as_deref());
                let result = match head {
                    Some(head) => saved.restore_locked(repo, head, progress),
                    None => {
                        Err("Cannot verify Revert result before restoring saved changes".into())
                    }
                };
                if let Err(error) = result {
                    report.error = Some(error);
                }
                if let Some(last) = &mut report.last {
                    last.state = repo.operation_state(&Cancel::default());
                }
            }
        }
        Ok(report)
    }

    pub(crate) fn saved_changes(
        &self,
        repo: &Repository,
        token: &Cancel,
    ) -> Result<Option<SavedChanges>, String> {
        SavedChanges::for_recovery(repo, &self.expected, token)
    }

    pub(crate) fn restore_after_abort(
        &self,
        saved: &SavedChanges,
        repo: &Repository,
        progress: &Progress,
    ) -> Result<(), String> {
        saved.restore_locked(repo, &self.expected.head, progress)
    }

    fn resume_locked(
        &self,
        repo: &Repository,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<BatchReport, String> {
        let continued = crate::sequencer_operation::execute_locked(
            repo,
            crate::operation_state::Operation::Revert,
            crate::sequencer_operation::Action::Continue { editor, requests },
            token,
            progress,
        )?;
        let head = crate::commit_operation::head(repo, &Cancel::default());
        let success =
            continued.command.as_ref().is_ok_and(|o| {
                o.status.success() && o.stopped.is_none() && o.input_error.is_none()
            }) && continued
                .state
                .as_ref()
                .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
                && head
                    .as_ref()
                    .is_ok_and(|h| h.as_ref().is_some_and(|h| h != &self.expected.head));
        let status = if success {
            Status::Success
        } else if continued
            .state
            .as_ref()
            .is_ok_and(|s| !s.conflicts.is_empty())
        {
            Status::Conflicts
        } else if token.cancelled() {
            Status::Cancelled
        } else {
            Status::Failed
        };
        let mut report = BatchReport {
            successful: vec![],
            skipped: vec![],
            remaining: self.remaining.clone(),
            last: Some(Report {
                command: continued.command,
                state: continued.state,
                head,
                status,
            }),
            error: None,
            recovery: Some(self.clone()),
            retry: None,
        };
        if !success {
            return Ok(report);
        }
        report.successful.push(self.expected.target.clone());
        report.remaining.remove(0);
        report.recovery = None;
        if let Err(error) = self.clear_record() {
            report.error = Some(error);
            return Ok(report);
        }
        if !report.remaining.is_empty() {
            // Validate against the resulting HEAD, not the pre-resolution commit.
            match Batch::read(repo, &report.remaining, token)
                .and_then(|batch| batch.execute_locked(repo, editor, requests, token, progress))
            {
                Ok(mut rest) => {
                    rest.successful.insert(0, self.expected.target.clone());
                    return Ok(rest);
                }
                Err(error) => report.error = Some(error),
            }
        }
        Ok(report)
    }
}
