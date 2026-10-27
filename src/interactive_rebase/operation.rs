//! Explicit interactive rebase execution under the existing common-dir queue.
use super::{interaction::Requests as PlanRequests, prepare::Prepared};
use crate::{
    auth_request::Requests,
    editor,
    git::Cancel,
    operation_process::{self, Outcome, Progress},
    operation_state::State,
    published_commits::Protection,
    rebase_saved,
    update_session::Session,
};
use std::path::Path;

pub fn progress_text(progress: &Progress) -> String {
    crate::rebase_operation::display_text(&String::from_utf8_lossy(&progress.snapshot().bytes()))
}

pub struct Report {
    pub command: Result<Outcome, String>,
    pub editor_error: Option<String>,
    pub save: Option<Result<Outcome, String>>,
    pub restore: Option<Result<Option<Outcome>, String>>,
    pub saved_oid: Option<String>,
    pub recovery_error: Option<String>,
    pub head: Result<Option<String>, String>,
    pub state: Result<State, String>,
}
fn good(outcome: &Outcome) -> bool {
    outcome.status.success() && outcome.stopped.is_none() && outcome.input_error.is_none()
}
impl Report {
    /// Rebased's GitRebaseProblemDetector recognizes this Git diagnostic.
    /// Require the live rebase state too: output alone must not describe an
    /// already completed or failed operation as waiting for an edit.
    pub fn stopped_for_editing(&self) -> bool {
        self.state.as_ref().is_ok_and(|state| {
            matches!(
                state.operation,
                crate::operation_state::Operation::RebaseApply
                    | crate::operation_state::Operation::RebaseMerge
            ) && state.conflicts.is_empty()
        }) && self.command.as_ref().is_ok_and(|outcome| {
            good(outcome)
                && [&outcome.stdout, &outcome.stderr].iter().any(|output| {
                    String::from_utf8_lossy(&output.bytes())
                        .to_ascii_lowercase()
                        .contains("you can amend the commit now")
                })
        })
    }
    pub fn diagnostic(&self) -> String {
        let mut messages = Vec::new();
        let mut append = |result: &Result<Outcome, String>| match result {
            Err(error) => messages.push(error.clone()),
            Ok(out) => {
                messages.push(crate::rebase_operation::collect_errors(
                    &String::from_utf8_lossy(&out.stdout.bytes()),
                    &String::from_utf8_lossy(&out.stderr.bytes()),
                ));
                if let Some(error) = &out.input_error {
                    messages.push(error.to_string());
                }
            }
        };
        append(&self.command);
        if let Some(save) = &self.save {
            append(save);
        }
        match &self.restore {
            Some(Ok(Some(out))) => messages.push(crate::rebase_operation::collect_errors(
                &String::from_utf8_lossy(&out.stdout.bytes()),
                &String::from_utf8_lossy(&out.stderr.bytes()),
            )),
            Some(Err(e)) => messages.push(e.clone()),
            _ => {}
        }
        messages.extend(
            self.editor_error
                .iter()
                .chain(self.recovery_error.iter())
                .cloned(),
        );
        if let Err(e) = &self.state {
            messages.push(e.clone());
        }
        if let Err(e) = &self.head {
            messages.push(e.clone());
        }
        crate::rebase_operation::display_text(&messages.join("\n"))
    }
    pub fn succeeded(&self) -> bool {
        self.command.as_ref().is_ok_and(good)
            && self.editor_error.is_none()
            && self.recovery_error.is_none()
            && self.saved_oid.is_none()
            && self
                .save
                .as_ref()
                .is_none_or(|s| s.as_ref().is_ok_and(good))
            && self
                .restore
                .as_ref()
                .is_none_or(|s| s.as_ref().is_ok_and(|o| o.as_ref().is_none_or(good)))
            && self.head.as_ref().is_ok_and(Option::is_some)
            && self
                .state
                .as_ref()
                .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
    }
}
/// Worker-only. `initial_confirmed` records the separate root-commit warning,
/// not permission to bypass write-time snapshot/ownership checks.
pub fn execute(
    prepared: &Prepared,
    initial_confirmed: bool,
    helper: &Path,
    protection: Protection<'_>,
    plans: &PlanRequests,
    fallback: &Requests,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    if prepared.initial() && !initial_confirmed {
        return Err("Confirm rewriting the initial commit before rebasing".into());
    }
    let sequence_editor = editor::command(helper, true).map_err(|e| e.to_string())?;
    let commit_editor = editor::command(helper, false).map_err(|e| e.to_string())?;
    let repo = prepared.repository();
    let mut session = Session::begin(&repo, token)?;
    prepared.validate(protection, token)?;
    let directory = repo.editor_directory(token)?;
    if rebase_saved::read(&directory)?.is_some() {
        return Err("Resolve pending saved changes before rebasing".into());
    }
    editor::validate_encodings(&repo, token).map_err(|e| e.to_string())?;
    let dirty = !repo
        .text(&["status", "--porcelain", "--untracked-files=no"], token)?
        .is_empty();
    let save = dirty.then(|| session.save("Plico: before interactive rebase", token, progress));
    let record = session.saved_oid().map(|stash| rebase_saved::Record {
        head: prepared.head().into(),
        base: prepared.base().map(str::to_owned),
        branch: prepared.branch().map(str::to_owned),
        stash: stash.into(),
    });
    let mut recovery_error = record
        .as_ref()
        .and_then(|record| rebase_saved::create(&directory, record).err());
    let saved_ok = save.as_ref().is_none_or(|s| s.as_ref().is_ok_and(good));
    let (command, editor_error) = if !saved_ok {
        (
            Err("Could not save local changes; rebase was not started".into()),
            None,
        )
    } else if recovery_error.is_some() {
        (
            Err("Could not persist saved changes; rebase was not started".into()),
            None,
        )
    } else {
        match prepared.validate(protection, token) {
            Err(error) => (Err(error), None),
            Ok(()) => match crate::rebase_editor::Router::start_interactive(
                repo.clone(),
                prepared.head().into(),
                plans.clone(),
                fallback.clone(),
                token.clone(),
            ) {
                Err(error) => (Err(error.to_string()), None),
                Ok(router) => {
                    let mut command = repo.operation_command();
                    command.args([
                        "-c",
                        "core.commentChar=\u{1}",
                        "-c",
                        "rebase.autoStash=false",
                        "rebase",
                        "--interactive",
                    ]);
                    command.arg(prepared.base().unwrap_or("--root"));
                    command
                        .env("GIT_SEQUENCE_EDITOR", sequence_editor)
                        .env("GIT_EDITOR", commit_editor);
                    router.configure(&mut command);
                    let result = operation_process::run(&mut command, token, progress, None)
                        .map_err(|e| e.to_string());
                    (result, router.finish())
                }
            },
        }
    };
    plans.close();
    let fresh = Cancel::default();
    let before_state = repo.operation_state(&fresh);
    let unchanged = crate::commit_operation::head(&repo, &fresh)
        .is_ok_and(|h| h.as_deref() == Some(prepared.head()));
    let completed = command.as_ref().is_ok_and(good) && editor_error.is_none();
    let can_restore = saved_ok
        && recovery_error.is_none()
        && !token.cancelled()
        && before_state
            .as_ref()
            .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
        && crate::squash_check::symbolic_head(&repo, &fresh)
            .is_ok_and(|b| b.as_deref() == prepared.branch())
        && (completed || unchanged);
    let restore = (can_restore && record.is_some()).then(|| session.restore_saved(true, progress));
    if session.saved_oid().is_none() {
        if let Some(record) = &record {
            if recovery_error.is_none() {
                recovery_error = rebase_saved::remove(&directory, record).err();
            }
        }
    }
    Ok(Report {
        command,
        editor_error,
        save,
        restore,
        saved_oid: session.saved_oid().map(str::to_owned),
        recovery_error,
        head: crate::commit_operation::head(&repo, &fresh),
        state: repo.operation_state(&fresh),
    })
}
