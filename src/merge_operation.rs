//! Translated from Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd:
//! GitMerger.mergeCommit, GitMergeCommitAction, GitAbortOperationAction.Merge.
//! Upstream copyright JetBrains s.r.o. and contributors, Apache-2.0.
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
    operation_state::{Operation, State},
};

#[derive(Clone, Copy)]
pub enum Action {
    Commit,
    Abort,
}

pub struct ResultState {
    pub command: Result<Outcome, String>,
    pub state: Result<State, String>,
    pub restore: Option<Result<(), String>>,
}

/// Diagnostic shown by the merge recovery UI.
pub fn diagnostic(result: &Result<ResultState, String>) -> Option<String> {
    let mut messages = Vec::new();
    match result {
        Err(error) => messages.push(error.clone()),
        Ok(result) => {
            match &result.command {
                Err(error) => messages.push(error.clone()),
                Ok(output) => {
                    if !output.status.success() || output.stopped.is_some() || output.input_error.is_some() {
                        messages.push(crate::rebase_operation::collect_errors(
                            &String::from_utf8_lossy(&output.stdout.bytes()),
                            &String::from_utf8_lossy(&output.stderr.bytes()),
                        ));
                    }
                    if let Some(error) = &output.input_error { messages.push(error.to_string()); }
                }
            }
            if let Err(error) = &result.state { messages.push(error.clone()); }
            if let Some(Err(error)) = &result.restore { messages.push(error.clone()); }
        }
    }
    let text = crate::rebase_operation::display_text(&messages.join("\n"));
    (!text.trim().is_empty()).then_some(text)
}

/// Worker-only. Abort requires explicit user confirmation in the caller.
/// Commit completes an existing merge; it neither stages files nor starts one.
pub fn execute(
    repo: &Repository,
    action: Action,
    token: &Cancel,
    progress: &Progress,
) -> Result<ResultState, String> {
    let _guard = repo.operation_guard(token)?;
    let state = repo.operation_state(token)?;
    if state.operation != Operation::Merge {
        return Err("There is no merge to complete or abort".into());
    }
    let saved = crate::merge_saved::for_merge(repo, token)?;
    let mut command = repo.operation_command();
    match action {
        Action::Commit => {
            if !state.conflicts.is_empty() {
                return Err("Resolve and stage all conflicts before completing the merge".into());
            }
            let message = repo.editor_directory(token)?.join("MERGE_MSG");
            command.arg("commit");
            // try_exists preserves access errors instead of treating them as a missing message.
            if message.try_exists().map_err(|e| e.to_string())? {
                command.arg("-F").arg(message);
            } else {
                let branch = repo.text(&["symbolic-ref", "--quiet", "--short", "HEAD"], token);
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                let branch = if state.detached {
                    String::new()
                } else {
                    branch?.trim_end().to_owned()
                };
                command.arg("-m").arg(format!(
                    "Merge branch '{branch}' of {} with conflicts.",
                    repo.path.display()
                ));
            }
            command.arg("--");
        }
        Action::Abort => {
            command.args(["merge", "--abort"]);
        }
    }
    let command =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    // Failure/cancellation can occur after a write; always inspect actual state.
    let restore = if command
        .as_ref()
        .is_ok_and(|o| o.status.success() && o.stopped.is_none() && o.input_error.is_none())
    {
        saved.as_ref().map(|record| {
            crate::merge_saved::restore(repo, record, matches!(action, Action::Abort), progress)
        })
    } else {
        None
    };
    let state = repo.operation_state(&Cancel::default());
    Ok(ResultState {
        command,
        state,
        restore,
    })
}
