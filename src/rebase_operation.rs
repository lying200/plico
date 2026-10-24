//! Existing-rebase commands translated from Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd:
//! GitAbstractRebaseAction, GitRebaseUtils (single repository), GitRebaser.continueRebase.
//! Upstream copyright JetBrains s.r.o. and contributors, Apache-2.0.
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
}

pub struct ResultState {
    pub recovery_error: Option<String>,
    pub squash_undo: Option<crate::squash_undo::Undo>,
    pub command: Result<Outcome, String>,
    pub state: Result<State, String>,
}

/// Rebased GitRebaser.collectErrorOutputLines: error-prefixed lines first,
/// otherwise stderr, otherwise the last nonblank stdout line. Retain only a
/// bounded, display-safe diagnostic; never write raw command output to a log.
pub fn diagnostic(result: &Result<ResultState, String>) -> Option<String> {
    let text = match result {
        Err(error) => error.clone(),
        Ok(result) => {
            let mut text = match &result.command {
                Err(error) => error.clone(),
                Ok(output) if !output.status.success() || output.stopped.is_some() => {
                    collect_errors(
                        &String::from_utf8_lossy(&output.stdout.bytes()),
                        &String::from_utf8_lossy(&output.stderr.bytes()),
                    )
                }
                Ok(_) => String::new(),
            };
            if let Err(error) = &result.state {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(error);
            }
            if let Some(error) = &result.recovery_error {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(error);
            }
            text
        }
    };
    let text = display_text(&text);
    (!text.trim().is_empty()).then_some(text)
}

pub(crate) fn collect_errors(stdout: &str, stderr: &str) -> String {
    const PREFIXES: &[&str] = &[
        "warning:",
        "error:",
        "fatal:",
        "remote: error",
        "Cannot",
        "Could not",
        "Interactive rebase already started",
        "refusing to pull",
        "cannot rebase:",
        "conflict",
        "unable",
        "The file will have its original",
        "runnerw:",
    ];
    let errors = stdout
        .lines()
        .chain(stderr.lines())
        .filter(|line| {
            PREFIXES
                .iter()
                .any(|prefix| line.trim().starts_with(prefix))
        })
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        errors.join("\n")
    } else if !stderr.trim().is_empty() {
        stderr.to_owned()
    } else {
        stdout
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .to_owned()
    }
}

pub(crate) fn display_text(text: &str) -> String {
    // Strip URL userinfo/query/fragment before truncating, so a truncated URL
    // cannot leave half a credential visible. Hook output remains plain text.
    let mut safe = String::new();
    for part in text.split_inclusive(char::is_whitespace) {
        if let Some(start) = part.find("://") {
            let scheme_start = part[..start]
                .char_indices()
                .rev()
                .find(|(_, c)| !c.is_ascii_alphanumeric() && !matches!(c, '+' | '-' | '.'))
                .map_or(0, |(i, c)| i + c.len_utf8());
            let end = part
                .trim_end()
                .trim_end_matches(['\'', '"', ')', ']', ',', ';'])
                .len();
            if let Ok(mut url) = url::Url::parse(&part[scheme_start..end]) {
                let _ = url.set_username("");
                let _ = url.set_password(None);
                url.set_query(None);
                url.set_fragment(None);
                safe.push_str(&part[..scheme_start]);
                safe.push_str(url.as_str());
                safe.push_str(&part[end..]);
                continue;
            }
        }
        safe.extend(
            part.chars()
                .filter(|c| !c.is_control() || matches!(c, '\n' | '\t')),
        );
    }
    if safe.chars().count() > 8000 {
        safe = safe.chars().take(8000).collect();
        safe.push_str("\n…");
    }
    safe
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_follow_error_precedence_and_keep_original_text() {
        assert_eq!(
            collect_errors("progress\n error: first\n", "hint: next\nfatal: second\n"),
            " error: first\nfatal: second"
        );
        assert_eq!(
            collect_errors("progress", "custom hook declined\n"),
            "custom hook declined\n"
        );
        assert_eq!(collect_errors("first\nlast\n\n", ""), "last");
        assert_eq!(diagnostic(&Err("文件被锁定".into())).unwrap(), "文件被锁定");
        assert!(diagnostic(&Err(String::new())).is_none());
        let report = Ok(ResultState {
            squash_undo: None,
            recovery_error: None,
            command: Err("command unavailable".into()),
            state: Err("state read failed".into()),
        });
        assert_eq!(
            diagnostic(&report).unwrap(),
            "command unavailable\nstate read failed"
        );
    }
    #[test]
    fn diagnostic_redacts_url_credentials_and_bounds_unicode() {
        let text = display_text(
            "fatal: 'https://alice:secret@example.invalid/repo?token=private#secret'\n",
        );
        assert_eq!(text, "fatal: 'https://example.invalid/repo'\n");
        assert_eq!(
            display_text("地址：https://user:secret@example.invalid/repo"),
            "地址：https://example.invalid/repo"
        );
        assert_eq!(display_text("bad\0value\n"), "badvalue\n");
        let text = display_text(&"错".repeat(9000));
        assert_eq!(text.chars().count(), 8002);
    }
}

/// Worker-only. The caller must confirm Abort. Supports an existing rebase even
/// when its original update session has gone away. Does not restore a stash
/// whose ownership cannot be established. No-change recovery follows GitRebaser.
pub fn execute(
    repo: &Repository,
    action: Action<'_>,
    token: &Cancel,
    progress: &Progress,
) -> Result<ResultState, String> {
    let _guard = repo.operation_guard(token)?;
    let state = repo.operation_state(token)?;
    if !matches!(
        state.operation,
        Operation::RebaseApply | Operation::RebaseMerge
    ) {
        return Err("Repository is not rebasing".into());
    }
    let directory = repo.editor_directory(token)?;
    let saved = crate::rebase_saved::for_rebase(&directory);
    let origin = crate::squash_session::read(&directory);
    let aborted = matches!(&action, Action::Abort);
    let command = match action {
        Action::Continue { editor, requests } => {
            if !state.conflicts.is_empty() {
                return Err("Resolve and stage rebase conflicts before continuing".into());
            }
            if editor.is_empty() {
                return Err("No rebase editor available".into());
            }
            continue_with_recovery(repo, editor, requests, token, progress)
        }
        Action::Abort => {
            let mut command = repo.operation_command();
            command.args(["rebase", "--abort"]);
            operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string())
        }
    };
    // Cancellation is not rollback. A failed command can also have written state.
    let state = repo.operation_state(&Cancel::default());
    let mut recovery_error = match saved {
        Err(error) => Some(error),
        Ok(Some(record))
            if !token.cancelled()
                && command.as_ref().is_ok_and(|o| {
                    o.status.success() && o.stopped.is_none() && o.input_error.is_none()
                })
                && state
                    .as_ref()
                    .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty()) =>
        {
            crate::rebase_saved::restore(repo, &directory, &record, aborted, progress).err()
        }
        _ => None,
    };
    let state = repo.operation_state(&Cancel::default());
    let squash_undo = if !aborted
        && !token.cancelled()
        && recovery_error.is_none()
        && command
            .as_ref()
            .is_ok_and(|o| o.status.success() && o.stopped.is_none() && o.input_error.is_none())
        && state
            .as_ref()
            .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
    {
        match origin.and_then(|o| {
            o.map(|o| crate::squash_undo::Undo::after_continue(repo, &o, &Cancel::default()))
                .transpose()
        }) {
            Ok(undo) => undo,
            Err(error) => {
                recovery_error = Some(error);
                None
            }
        }
    } else {
        None
    };
    Ok(ResultState {
        command,
        state,
        recovery_error,
        squash_undo,
    })
}

/// Caller holds the repository operation guard. Shared with an owned update
/// session so recovery never tries to acquire the same guard recursively.
pub(crate) fn continue_with_recovery(
    repo: &Repository,
    editor: &OsStr,
    requests: Option<&Requests>,
    token: &Cancel,
    progress: &Progress,
) -> Result<Outcome, String> {
    let directory = repo.editor_directory(token)?;
    let server = requests
        .map(|requests| {
            crate::rebase_editor::Router::start(&directory, requests.clone(), token.clone())
        })
        .transpose()
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut action = "--continue";
        let mut previous = None;
        loop {
            let mut command = repo.operation_command();
            command
                .args(["-c", "core.commentChar=\u{1}", "rebase", action])
                .env("GIT_EDITOR", editor);
            if let Some(server) = &server {
                server.configure(&mut command);
            }
            let output = operation_process::run(&mut command, token, progress, None)
                .map_err(|e| e.to_string())?;
            if output.status.success()
                || output.stopped.is_some()
                || token.cancelled()
                || !output
                    .events
                    .contains(crate::operation_events::Event::RebaseNoChanges)
            {
                return Ok(output);
            }
            let state = repo.operation_state(token)?;
            // Modern Git also emits generic conflict-resolution advice for an empty
            // patch. Check the actual unmerged index instead of treating that hint
            // as a new conflict when the explicit no-change marker is present.
            if !matches!(
                state.operation,
                Operation::RebaseApply | Operation::RebaseMerge
            ) || !state.conflicts.is_empty()
            {
                return Ok(output);
            }
            let staged = !repo
                .text(&["diff", "--name-only", "--no-renames", "--cached"], token)?
                .trim()
                .is_empty();
            let unstaged = !repo
                .text(&["diff", "--name-only", "--no-renames"], token)?
                .trim()
                .is_empty();
            let position = (
                repo.text(&["rev-parse", "HEAD"], token)?,
                repo.text(&["rev-parse", "--verify", "REBASE_HEAD"], token)?,
                staged,
                unstaged,
            );
            // The upstream recursive retry has no progress bound. Keep only one
            // fingerprint, and stop if Git repeats the same failed recovery state.
            if previous.as_ref() == Some(&position) {
                return Ok(output);
            }
            previous = Some(position);
            if staged {
                action = "--continue";
            } else if unstaged {
                let mut stage = repo.operation_command();
                stage.args(["add", "--update"]);
                let staged = operation_process::run(&mut stage, token, progress, None)
                    .map_err(|e| e.to_string())?;
                if !staged.status.success() || staged.stopped.is_some() || token.cancelled() {
                    return Ok(staged);
                }
                action = "--continue";
            } else {
                action = "--skip";
            }
        }
    })();
    let editor_error = server.and_then(crate::rebase_editor::Router::finish);
    match editor_error {
        Some(error) => Err(error),
        None => result,
    }
}
