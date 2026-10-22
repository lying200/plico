//! Rebased GitStashActions / GitStashUtils.createUnstashHandler and unstash.
//! Reference 4aec35256d765c217aa0d662b82ac4e860c396fd, JetBrains, Apache-2.0.
use crate::{
    git::{Cancel, Repository, is_oid},
    operation_events::Event,
    operation_process::{self, Outcome, Progress, StopReason},
    operation_state::State,
    stash::Entry,
};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Apply,
    Pop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    Conflicts,
    IndexConflict,
    LocalChanges,
    UntrackedFiles,
    Cancelled,
    Failed,
}
pub struct Report {
    pub recovery_error: Option<String>,
    pub command: Result<Outcome, String>,
    pub state: Result<State, String>,
    pub status: Status,
}

/// Explicit caller action only. The standard upstream Apply/Pop actions pass
/// reinstate_index=false; the Unstash As dialog offers this option separately.
pub fn execute(
    repo: &Repository,
    entry: &Entry,
    action: Action,
    reinstate_index: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let number = entry
        .selector
        .strip_prefix("stash@{")
        .and_then(|s| s.strip_suffix('}'))
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
    if number.is_none() || !is_oid(&entry.oid) {
        return Err("Invalid saved stash identity".into());
    }
    let _guard = repo.operation_guard(token)?;
    // Local adaptation: never knowingly use a stale position-dependent selector.
    // The application lock does not prevent another Git client racing this check.
    let current = repo.text(
        &["rev-parse", "--verify", "--end-of-options", &entry.selector],
        token,
    )?;
    if current.trim() != entry.oid {
        return Err("Saved changes have moved; refresh the stash list".into());
    }
    // Manual restoration can complete a Squash saver left by external Git.
    // Snapshot ownership before executing; clear only that exact record on success.
    let saved = repo.editor_directory(token).and_then(|directory| {
        crate::squash_saved::read(&directory).map(|record| (directory, record))
    });
    if repo.operation_state(token)?.conflicts.is_empty() {
        crate::conflicts::clear_saved_context(repo, token)?;
    }
    let mut command = repo.operation_command();
    command.args([
        "stash",
        match action {
            Action::Apply => "apply",
            Action::Pop => "pop",
        },
    ]);
    if reinstate_index {
        command.arg("--index");
    }
    command.arg(&entry.selector);
    let command =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    // Even a cancelled/failed command can modify files. Never infer rollback.
    let state = repo.operation_state(&Cancel::default());
    let status = classify(&command, &state);
    let recovery_error = if status == Status::Success {
        match saved {
            Ok((directory, Some(record))) if record.stash == entry.oid => {
                crate::squash_saved::remove(&directory, &record).err()
            }
            Err(error) => Some(error),
            _ => None,
        }
    } else {
        None
    };
    Ok(Report {
        command,
        state,
        status,
        recovery_error,
    })
}
fn classify(command: &Result<Outcome, String>, state: &Result<State, String>) -> Status {
    let Ok(command) = command else {
        return Status::Failed;
    };
    if command.stopped == Some(StopReason::Cancelled) {
        return Status::Cancelled;
    }
    if command.events.contains(Event::UnstashIndexConflict) {
        return Status::IndexConflict;
    }
    if state.as_ref().is_ok_and(|s| !s.conflicts.is_empty()) {
        return Status::Conflicts;
    }
    if command
        .events
        .contains(Event::UntrackedFilesWouldBeOverwritten)
    {
        return Status::UntrackedFiles;
    }
    if command
        .events
        .contains(Event::LocalChangesWouldBeOverwritten)
    {
        return Status::LocalChanges;
    }
    if command.stopped.is_none()
        && command.input_error.is_none()
        && command.status.success()
        && state.is_ok()
    {
        Status::Success
    } else {
        Status::Failed
    }
}

/// Drop only the stash that the user confirmed, never a newly shifted selector.
/// Unlike Apply/Pop this must not clear Squash recovery ownership: deleting the
/// saved data does not mean the working changes were restored.
pub fn drop_stash(
    repo: &Repository,
    entry: &Entry,
    token: &Cancel,
    progress: &Progress,
) -> Result<Outcome, String> {
    let number = entry
        .selector
        .strip_prefix("stash@{")
        .and_then(|s| s.strip_suffix('}'))
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
    if number.is_none() || !is_oid(&entry.oid) {
        return Err("Invalid saved stash identity".into());
    }
    let _guard = repo.operation_guard(token)?;
    let current = repo.text(
        &["rev-parse", "--verify", "--end-of-options", &entry.selector],
        token,
    )?;
    if current.trim() != entry.oid {
        return Err("Saved changes have moved; refresh the stash list".into());
    }
    let mut command = repo.operation_command();
    command.args(["stash", "drop", &entry.selector]);
    operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string())
}
