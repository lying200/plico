//! Existing cherry-pick/revert recovery. Rebased 4aec352:
//! CherryPickContinueCommand, GitAbortOperationAction, GitRevertProcess.
//! JetBrains and contributors, Apache-2.0.
use crate::{
    auth_request::Requests,
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
    operation_state::{Operation, State},
};
use std::ffi::OsStr;

pub enum Action<'a> {
    Continue {
        editor: &'a OsStr,
        requests: Option<&'a Requests>,
    },
    Abort,
    Empty {
        expected: &'a EmptyCommit,
        choice: EmptyChoice,
    },
}
#[derive(Clone, Copy, Debug)]
pub enum EmptyChoice {
    Skip,
    Keep,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyCommit {
    head: String,
    picked: String,
}
impl EmptyCommit {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Option<Self>, String> {
        let state = repo.operation_state(token)?;
        if state.operation != Operation::CherryPick || !state.conflicts.is_empty() {
            return Ok(None);
        }
        // A remaining sequencer todo without CHERRY_PICK_HEAD is not an empty
        // stopped commit: the user may already have committed it manually.
        let directory = repo.editor_directory(token)?;
        if !directory
            .join("CHERRY_PICK_HEAD")
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            return Ok(None);
        }
        let picked = repo
            .text(&["rev-parse", "--verify", "CHERRY_PICK_HEAD"], token)?
            .trim()
            .to_owned();
        let head = crate::commit_operation::head(repo, token)?.ok_or("No HEAD for cherry-pick")?;
        if !crate::git::is_oid(&picked) {
            return Err("Invalid cherry-pick identity".into());
        }
        if !repo
            .text(
                &[
                    "diff",
                    "--cached",
                    "--raw",
                    "--no-ext-diff",
                    "--no-renames",
                    "HEAD",
                    "--",
                ],
                token,
            )?
            .is_empty()
        {
            return Ok(None);
        }
        Ok(Some(Self { head, picked }))
    }
}
pub struct ResultState {
    pub command: Result<Outcome, String>,
    pub state: Result<State, String>,
    pub empty: Option<EmptyCommit>,
}
/// Operates only on an already-running operation of the expected type.
/// Continue does not stage files, skip an empty commit, or discard changes.
/// Abort must be explicitly confirmed by the caller.
pub fn execute(
    repo: &Repository,
    expected: Operation,
    action: Action<'_>,
    token: &Cancel,
    progress: &Progress,
) -> Result<ResultState, String> {
    let _guard = repo.operation_guard(token)?;
    execute_locked(repo, expected, action, token, progress)
}

pub(crate) fn execute_locked(
    repo: &Repository,
    expected: Operation,
    action: Action<'_>,
    token: &Cancel,
    progress: &Progress,
) -> Result<ResultState, String> {
    let state = repo.operation_state(token)?;
    let name = match expected {
        Operation::CherryPick => "cherry-pick",
        Operation::Revert => "revert",
        _ => return Err("Expected an existing cherry-pick or revert".into()),
    };
    if state.operation != expected {
        return Err("The operation has changed; refresh before continuing or aborting".into());
    }
    let aborted_queue = if expected == Operation::Revert && matches!(&action, Action::Abort) {
        crate::revert_operation::Recovery::read(repo, token)?
    } else {
        None
    };
    let aborted_saved = aborted_queue
        .as_ref()
        .map(|queue| queue.saved_changes(repo, token))
        .transpose()?
        .flatten();
    let mut command = repo.operation_command();
    let server;
    match action {
        Action::Continue { editor, requests } => {
            if !state.conflicts.is_empty() {
                return Err("Resolve and stage all conflicts before continuing".into());
            }
            if editor.is_empty() {
                return Err("No commit editor available".into());
            }
            server = requests
                .map(|r| crate::askpass::Server::start(r.clone()))
                .transpose()
                .map_err(|e| e.to_string())?;
            // MERGE_MSG was generated using the repository's comment marker.
            // Changing it only on Continue commits the previous marker's
            // conflict annotations as literal message text (also without an editor).
            command
                .args([name, "--continue"])
                .env("GIT_EDITOR", editor);
            if let Some(server) = &server {
                server.configure(&mut command);
            }
        }
        Action::Abort => {
            server = None;
            command.args([name, "--abort"]);
        }
        Action::Empty {
            expected: empty,
            choice,
        } => {
            server = None;
            if expected != Operation::CherryPick
                || EmptyCommit::read(repo, token)?.as_ref() != Some(empty)
            {
                return Err(
                    "The empty cherry-pick changed; refresh before choosing a resolution".into(),
                );
            }
            match choice {
                EmptyChoice::Skip => {
                    if !repo
                        .text(
                            &["diff", "--raw", "--no-ext-diff", "--no-renames", "--"],
                            token,
                        )?
                        .is_empty()
                    {
                        return Err("Save or set aside unstaged changes before skipping the empty cherry-pick".into());
                    }
                    command.args(["cherry-pick", "--skip"]);
                }
                EmptyChoice::Keep => {
                    command.args(["commit", "--allow-empty"]);
                    let message = repo.editor_directory(token)?.join("MERGE_MSG");
                    if message.try_exists().map_err(|e| e.to_string())? {
                        command.arg("-F").arg(message);
                    } else {
                        command.args(["-m", "Empty cherry-pick"]);
                    }
                }
            }
        }
    }
    let result =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    drop(server);
    let mut final_state = repo.operation_state(&Cancel::default());
    let mut result = result;
    if result
        .as_ref()
        .is_ok_and(|o| o.status.success() && o.stopped.is_none())
        && final_state
            .as_ref()
            .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
    {
        if let Some(queue) = aborted_queue {
            if let Err(error) = queue.clear_record() {
                result = Err(error);
            }
            if let Some(saved) = aborted_saved {
                if let Err(error) = queue.restore_after_abort(&saved, repo, progress) {
                    result = Err(error);
                }
                final_state = repo.operation_state(&Cancel::default());
            }
        }
    }
    Ok(ResultState {
        command: result,
        state: final_state,
        empty: EmptyCommit::read(repo, &Cancel::default()).ok().flatten(),
    })
}
