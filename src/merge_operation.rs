//! Translated from Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd:
//! GitMerger.mergeCommit, GitMergeCommitAction, GitAbortOperationAction.Merge.
//! Upstream copyright JetBrains s.r.o. and contributors, Apache-2.0.
use crate::{git::{Cancel, Repository}, operation_process::{self, Outcome, Progress},
    operation_state::{Operation, State}};

#[derive(Clone, Copy)]
pub enum Action { Commit, Abort }

pub struct ResultState {
    pub command: Result<Outcome, String>,
    pub state: Result<State, String>,
}

/// Worker-only. Abort requires explicit user confirmation in the caller.
/// Commit completes an existing merge; it neither stages files nor starts one.
pub fn execute(repo: &Repository, action: Action, token: &Cancel,
    progress: &Progress) -> Result<ResultState, String> {
    let _guard = repo.operation_guard(token)?;
    let state = repo.operation_state(token)?;
    if state.operation != Operation::Merge {
        return Err("There is no merge to complete or abort".into());
    }
    let mut command = repo.operation_command();
    match action {
        Action::Commit => {
            if !state.conflicts.is_empty() { return Err("Resolve and stage all conflicts before completing the merge".into()); }
            let message = repo.editor_directory(token)?.join("MERGE_MSG");
            command.arg("commit");
            // try_exists preserves access errors instead of treating them as a missing message.
            if message.try_exists().map_err(|e| e.to_string())? {
                command.arg("-F").arg(message);
            } else {
                let branch = repo.text(&["symbolic-ref", "--quiet", "--short", "HEAD"], token);
                if token.cancelled() { return Err("Cancelled".into()); }
                let branch = if state.detached { String::new() } else { branch?.trim_end().to_owned() };
                command.arg("-m").arg(format!("Merge branch '{branch}' of {} with conflicts.", repo.path.display()));
            }
            command.arg("--");
        }
        Action::Abort => { command.args(["merge", "--abort"]); }
    }
    let command = operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    // Failure/cancellation can occur after a write; always inspect actual state.
    let state = repo.operation_state(&Cancel::default());
    Ok(ResultState { command, state })
}
