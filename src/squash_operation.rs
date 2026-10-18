//! Git fallback execution for Rebased GitSquashOperation/GitCommitEditingOperation
//! (4aec352, JetBrains, Apache-2.0). Worker-only, explicit caller action.
use crate::{askpass::Server, auth_request::{Kind, Requests}, editor::{self, Document}, git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress}, operation_state::State, published_commits::Protection,
    squash_check::{self, Prepared}, squash_message, squash_saved, update_session::Session};
use std::{path::Path, thread, time::Duration};

pub struct Report {
    pub command: Result<Outcome, String>,
    pub editor_error: Option<String>,
    pub save: Option<Result<Outcome, String>>,
    pub restore: Option<Result<Option<Outcome>, String>>,
    pub saved_oid: Option<String>,
    pub recovery_error: Option<String>,
    pub old_head: String,
    pub base: String,
    pub branch: Option<String>,
    pub head: Result<Option<String>, String>,
    pub state: Result<State, String>,
}
impl Report {
    pub fn succeeded(&self) -> bool {
        self.saved_oid.is_none() && self.recovery_error.is_none()
            && self.save.as_ref().is_none_or(|r| r.as_ref().is_ok_and(good))
            && self.restore.as_ref().is_none_or(|r| r.as_ref().is_ok_and(|o| o.as_ref().is_none_or(good)))
            && self.editor_error.is_none() && self.command.as_ref().is_ok_and(|o| o.status.success() && o.stopped.is_none())
            && self.head.as_ref().is_ok_and(Option::is_some)
            && self.state.as_ref().is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
    }
}

/// The dialog's snapshot is checked again while holding the common-dir queue.
/// No implicit rollback: conflicts/cancellation retain Git's actual state.
/// Tracked local changes are saved first. Completed/not-started operations may
/// restore their owned top stash; incomplete/cancelled operations retain it.
pub fn execute(repo: &Repository, expected: &Prepared, message: &str, helper: &Path,
    protection: Protection<'_>, token: &Cancel, progress: &Progress, fallback: Option<&Requests>) -> Result<Report, String> {
    if message.trim().is_empty() || message.len() > 1024 * 1024 || message.contains('\0') {
        return Err("Squash message must be nonempty and at most 1 MiB".into());
    }
    let sequence_editor = editor::command(helper, true).map_err(|e| e.to_string())?;
    let commit_editor = editor::command(helper, false).map_err(|e| e.to_string())?;
    let mut session = Session::begin(repo, token)?;
    let checked = squash_check::revalidate(repo, expected, protection, token)?;
    let gitdir = repo.editor_directory(token)?;
    let oldest = &checked.selected_oldest_first[0];
    let original = repo.text(&["show", "--no-patch", "--no-show-signature", "--encoding=UTF-8", "--format=%B", oldest, "--"], token)?;
    // Remove the formatting newline, retaining the original commit's own one.
    let original = original.strip_suffix('\n').unwrap_or(&original);
    let encoding = repo.text(&["config", "--get", "--default", "UTF-8", "i18n.commitEncoding"], token)?;
    if !matches!(encoding.trim().to_ascii_lowercase().as_str(), "utf-8" | "utf8") {
        return Err("Squash message encoding is not yet supported".into());
    }
    let dirty = !repo.text(&["status", "--porcelain", "--untracked-files=no"], token)?.is_empty();
    let save = dirty.then(|| session.save("GitReview: before Squash", token, progress));
    let record = session.saved_oid().map(|stash| squash_saved::Record { head: checked.head.clone(), base: checked.base.clone(),
        branch: checked.branch.clone(), stash: stash.to_owned() });
    let mut recovery_error = record.as_ref().and_then(|r| squash_saved::create(&gitdir, r).err());
    let saved_ok = save.as_ref().is_none_or(|r| r.as_ref().is_ok_and(good));
    let (command, editor_error) = if !saved_ok {
        (Err("Could not save local changes; Squash was not started".into()), None)
    } else if recovery_error.is_some() {
        (Err("Could not persist saved-changes ownership; Squash was not started".into()), None)
    } else {
        // Saving and user cancellation may take time; recheck the same immutable
        // selection and branch before allowing the history rewrite to start.
        let recheck = squash_check::prepare(repo, &expected.head, &expected.selected_oldest_first, protection, token)
            .and_then(|now| if now.base == checked.base && now.branch == checked.branch && now.plan.entries() == checked.plan.entries()
                && !now.state.in_progress() && now.state.conflicts.is_empty() { Ok(()) }
                else { Err("Squash selection changed after saving local changes".into()) });
        match recheck {
            Ok(()) => run_rebase(repo, &checked, original, message, &gitdir, sequence_editor, commit_editor, token, progress, fallback),
            Err(error) => (Err(error), None),
        }
    };
    let fresh = Cancel::default();
    let before_restore_head = crate::commit_operation::head(repo, &fresh);
    let before_restore_state = repo.operation_state(&fresh);
    let completed = command.as_ref().is_ok_and(good) && editor_error.is_none();
    let not_started = before_restore_head.as_ref().is_ok_and(|h| h.as_deref() == Some(&checked.head));
    let can_restore = saved_ok && recovery_error.is_none() && !token.cancelled()
        && before_restore_state.as_ref().is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
        && squash_check::symbolic_head(repo, &fresh).is_ok_and(|b| b == checked.branch)
        && (completed || not_started);
    let restore = (can_restore && record.is_some()).then(|| session.restore_saved(true, progress));
    if session.saved_oid().is_none() {
        if let Some(record) = &record {
            if recovery_error.is_none() { recovery_error = squash_saved::remove(&gitdir, record).err(); }
        }
    }
    // A cancelled command can already have changed HEAD/index. Use fresh read
    // cancellation state rather than suppressing the resulting recovery report.
    let refresh = Cancel::default();
    Ok(Report { command, editor_error, save, restore, saved_oid: session.saved_oid().map(str::to_owned), recovery_error, old_head: checked.head, base: checked.base, branch: checked.branch,
        head: crate::commit_operation::head(repo, &refresh), state: repo.operation_state(&refresh) })
}

fn good(outcome: &Outcome) -> bool { outcome.status.success() && outcome.stopped.is_none() && outcome.input_error.is_none() }
fn run_rebase(repo: &Repository, checked: &Prepared, original: &str, message: &str, gitdir: &Path,
    sequence_editor: std::ffi::OsString, commit_editor: std::ffi::OsString, token: &Cancel,
    progress: &Progress, fallback: Option<&Requests>) -> (Result<Outcome, String>, Option<String>) {
    let requests = Requests::new(token.clone());
    let server = match Server::start(requests.clone()) {
        Ok(server) => server, Err(error) => return (Err(error.to_string()), None),
    };
    let stop = Cancel::default();
    let (command, editor_error) = thread::scope(|scope| {
        let worker = scope.spawn(|| {
            let mut sequence_seen = false;
            let mut error = None;
            while !stop.cancelled() && !token.cancelled() {
                let Some(pending) = requests.take_pending() else { thread::sleep(Duration::from_millis(10)); continue; };
                let result = (|| -> Result<(), String> {
                    if !matches!(pending.kind, Kind::SequenceEditor | Kind::CommitEditor) {
                        let reply = fallback.and_then(|ui| ui.ask_until(pending.kind, pending.prompt.clone(), &stop).ok());
                        match reply { Some(reply) => pending.answer(reply).map_err(|_| "Prompt cancelled".to_owned()), None => Err("Unexpected non-editor prompt".to_owned()) }?;
                        return Ok(());
                    }
                    let mut document = Document::open(pending, &gitdir).map_err(|e| e.to_string())?;
                    if document.sequence {
                        if sequence_seen { return Err("Unexpected second Squash sequence editor request".into()); }
                        sequence_seen = true;
                        let rewritten = checked.plan.rewrite_todo(document.text(), "\u{1}")?;
                        squash_message::install(&gitdir, &checked.head, &checked.base, original, message).map_err(|e| e.to_string())?;
                        crate::squash_session::install(gitdir, &checked.head, &checked.base, checked.branch.as_deref())?;
                        document.save(&rewritten).map_err(|e| e.to_string())
                    } else if squash_message::apply(&mut document).map_err(|e| e.to_string())? {
                        Ok(())
                    } else if let Some(ui) = fallback {
                        document.forward(ui, &stop).map_err(|e| e.to_string())
                    } else { Err("Squash requires the ordinary message editor for this request".into()) }
                })();
                if let Err(message) = result { if error.is_none() { error = Some(message); } }
            }
            error
        });
        let mut command = repo.operation_command();
        command.args(["-c", "core.commentChar=\u{1}", "rebase", "--interactive", &checked.base])
            .env("GIT_SEQUENCE_EDITOR", sequence_editor).env("GIT_EDITOR", commit_editor);
        server.configure(&mut command);
        let outcome = operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
        stop.cancel();
        (outcome, worker.join().unwrap_or_else(|_| Some("Squash editor worker failed".into())))
    });
    drop(server);
    (command, editor_error)

}
