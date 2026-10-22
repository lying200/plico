//! Rebased 4aec352 GitConfigureRemotesDialog/GitDefineRemoteDialog and
//! GitImpl remote commands (JetBrains and contributors, Apache-2.0).
//! Configuration edits are explicit; fetch is a separate, user-selected operation.
use crate::{
    auth_request::Requests,
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
};

/// URLs may contain credentials: intentionally not Debug and never logged.
#[derive(Clone, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub urls: Vec<String>,
    pub push_urls: Vec<String>,
    settings: Vec<(String, String)>,
    directory: std::path::PathBuf,
}

pub fn read(repo: &Repository, token: &Cancel) -> Result<Vec<Remote>, String> {
    let directory = repo.editor_directory(token)?;
    let names = repo.fetch_specs(token)?;
    let config = repo.text(&["config", "--null", "--list"], token)?;
    let mut remotes = Vec::new();
    for spec in names {
        let prefix = format!("remote.{}", spec.remote());
        let mut remote = Remote {
            name: spec.remote().to_owned(),
            urls: vec![],
            push_urls: vec![],
            settings: vec![],
            directory: directory.clone(),
        };
        for entry in config.split_terminator('\0') {
            let (key, value) = entry.split_once('\n').unwrap_or((entry, ""));
            if let Some((section, key)) = key.rsplit_once('.') {
                if section != prefix {
                    continue;
                }
                if key == "url" {
                    remote.urls.push(value.to_owned());
                }
                if key == "pushurl" {
                    remote.push_urls.push(value.to_owned());
                }
                remote.settings.push((key.to_owned(), value.to_owned()));
            }
        }
        remotes.push(remote);
    }
    remotes.sort_by(|a, b| (a.name != "origin", &a.name).cmp(&(b.name != "origin", &b.name)));
    Ok(remotes)
}

pub enum Edit {
    Add {
        name: String,
        url: String,
    },
    Change {
        previous: Remote,
        name: String,
        url: String,
    },
    Remove {
        previous: Remote,
    },
}

pub struct Report {
    /// Rename and URL replacement are separate Git operations. A later failure
    /// does not undo an earlier success; callers must refresh the actual config.
    pub commands: Vec<Outcome>,
}
impl Report {
    pub fn succeeded(&self) -> bool {
        self.commands.iter().all(success)
    }
}
fn success(outcome: &Outcome) -> bool {
    outcome.status.success() && outcome.stopped.is_none() && outcome.input_error.is_none()
}
fn validate_url(url: &str) -> Result<(), String> {
    if url.trim().is_empty() || url.starts_with('-') || url.chars().any(char::is_control) {
        return Err("Enter a nonempty remote URL or path".into());
    }
    Ok(())
}
fn validate_name(repo: &Repository, name: &str, token: &Cancel) -> Result<(), String> {
    if name.is_empty() || name.starts_with('-') || name.chars().any(char::is_control) {
        return Err("Invalid remote name".into());
    }
    repo.text(
        &["check-ref-format", &format!("refs/remotes/{name}/branch")],
        token,
    )?;
    Ok(())
}

/// Read-only form preflight. Execution repeats this under the operation guard:
/// UI validation cannot authorize overwriting a subsequently changed remote.
pub fn validate_edit(repo: &Repository, edit: &Edit, token: &Cancel) -> Result<(), String> {
    let remotes = read(repo, token)?;
    let (previous, desired) = match edit {
        Edit::Add { name, url } => (None, Some((name, url))),
        Edit::Change {
            previous,
            name,
            url,
        } => (Some(previous), Some((name, url))),
        Edit::Remove { previous } => (Some(previous), None),
    };
    if let Some(previous) = previous {
        if !remotes.iter().any(|r| r == previous) {
            return Err("Remote changed; reopen the remote configuration".into());
        }
    }
    if let Some((name, url)) = desired {
        validate_name(repo, name, token)?;
        validate_url(url)?;
        if remotes
            .iter()
            .any(|r| r.name == *name && previous.is_none_or(|p| p.name != *name))
        {
            return Err("A remote with that name already exists".into());
        }
    }
    Ok(())
}

/// Caller must confirm removal. An old dialog cannot remove or replace a remote
/// edited by another tool while it was open. Never overwrites a duplicate name.
pub fn execute(
    repo: &Repository,
    edit: &Edit,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let _guard = repo.operation_guard(token)?;
    validate_edit(repo, edit, token)?;
    let mut report = Report { commands: vec![] };
    let mut run = |args: &[&str]| -> Result<bool, String> {
        let result =
            operation_process::run(repo.operation_command().args(args), token, progress, None)
                .map_err(|e| e.to_string())?;
        let ok = success(&result);
        report.commands.push(result);
        Ok(ok)
    };
    match edit {
        Edit::Add { name, url } => {
            run(&["remote", "add", "--", name, url])?;
        }
        Edit::Remove { previous } => {
            run(&["remote", "remove", "--", &previous.name])?;
        }
        Edit::Change {
            previous,
            name,
            url,
        } => {
            if previous.name != *name && !run(&["remote", "rename", "--", &previous.name, name])? {
                return Ok(report);
            }
            if previous.urls.first().map(String::as_str).unwrap_or("") != url {
                run(&["remote", "set-url", "--", name, url])?;
            }
        }
    }
    Ok(report)
}

/// Mirrors Define Remote's asynchronous access check. A failed check is a
/// warning that the dialog may explicitly override, not permission to write.
pub fn check_access(
    repo: &Repository,
    url: &str,
    requests: &Requests,
    helper: &std::path::Path,
    token: &Cancel,
    progress: &Progress,
) -> Result<Outcome, String> {
    validate_url(url)?;
    if !helper.is_absolute() || !helper.is_file() {
        return Err("Authentication helper is unavailable".into());
    }
    let mut command = repo.operation_command();
    command.args(["ls-remote", "--", url]);
    crate::authenticated_command::run(
        &mut command,
        Some((requests, helper)),
        token,
        progress,
        None,
        |_| true,
    )
}

/// Redact credentials before showing a remote URL in the manager table.
pub fn display_url(url: &str) -> String {
    crate::rebase_operation::display_text(url)
}
