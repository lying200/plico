//! Rebased 4aec352: GitCloneUtils, CloneDvcsValidationUtils and GitImpl.clone.
//! No shell or recursive cleanup: Git owns clone cleanup; any residual directory
//! is reported and never mistaken for a successfully opened project.
use crate::{
    auth_request::Requests,
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

// Deliberately no Debug: source URLs can contain credentials.
pub struct Request {
    pub source: String,
    pub destination: PathBuf,
    pub depth: Option<u32>,
    pub recurse_submodules: bool,
}
pub struct Report {
    pub outcome: Outcome,
    pub destination: PathBuf,
    pub destination_exists: bool,
}
impl Report {
    pub fn succeeded(&self) -> bool {
        self.outcome.status.success()
            && self.outcome.stopped.is_none()
            && self.outcome.input_error.is_none()
    }
    pub fn diagnostic(&self) -> String {
        crate::rebase_operation::display_text(&crate::rebase_operation::collect_errors(
            &String::from_utf8_lossy(&self.outcome.stdout.bytes()),
            &String::from_utf8_lossy(&self.outcome.stderr.bytes()),
        ))
    }
}
pub fn source_url(value: &str) -> Result<String, String> {
    let value = value.trim();
    let value = value.strip_prefix("git clone ").unwrap_or(value).trim();
    if value.is_empty() || value.starts_with('-') || value.chars().any(char::is_control) {
        return Err("Enter a repository URL or directory".into());
    }
    let local = Path::new(value);
    if local.exists() {
        if !local.is_dir() {
            return Err("Source is not a directory".into());
        }
        return local
            .canonicalize()
            .map(|p| git_local_path(&p))
            .map_err(|e| e.to_string());
    }
    if !value.chars().any(char::is_whitespace) {
        if url::Url::parse(value).is_ok() {
            return Ok(value.into());
        }
        if let Some((host, path)) = value.split_once(':') {
            if !host.is_empty() && !host.contains(['/', '\\']) && !path.is_empty() {
                return Ok(value.into());
            }
        }
    }
    Err("Enter a repository URL or directory".into())
}
pub fn validate(request: &Request) -> Result<(), String> {
    source_url(&request.source)?;
    if request.depth == Some(0) {
        return Err("Clone depth must be greater than zero".into());
    }
    if !request.destination.is_absolute() || request.destination.file_name().is_none() {
        return Err("Choose an absolute destination directory".into());
    }
    match fs::symlink_metadata(&request.destination) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("Destination is not a directory".into());
            }
            if fs::read_dir(&request.destination)
                .map_err(|e| e.to_string())?
                .next()
                .transpose()
                .map_err(|e| e.to_string())?
                .is_some()
            {
                return Err("Destination directory is not empty".into());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    Ok(())
}
pub fn execute(
    request: &Request,
    token: &Cancel,
    progress: &Progress,
    auth: Option<(&Requests, &Path)>,
) -> Result<Report, String> {
    validate(request)?;
    if token.cancelled() {
        return Err("Clone cancelled".into());
    }
    if let Some((_, helper)) = auth {
        if !helper.is_absolute() || !helper.is_file() {
            return Err("Authentication helper is unavailable".into());
        }
    }
    let source = source_url(&request.source)?;
    let parent = request
        .destination
        .parent()
        .ok_or("Choose an absolute destination directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // Recheck after creating parents; another process may have populated the target.
    validate(request)?;
    let repo = Repository::new(parent);
    let mut command = repo.operation_command();
    if cfg!(windows) {
        command.args(["-c", "core.longpaths=true"]);
    }
    command.args(["clone", "--progress"]);
    if request.recurse_submodules {
        command.arg("--recurse-submodules");
    }
    if let Some(depth) = request.depth {
        command.arg(format!("--depth={depth}"));
    }
    command
        .arg("--")
        .arg(source)
        .arg(request.destination.file_name().unwrap());
    let _server = if let Some((requests, helper)) = auth {
        let server = crate::askpass::Server::start(requests.clone()).map_err(|e| e.to_string())?;
        server.configure(&mut command);
        command
            .env("GIT_ASKPASS", helper)
            .env_remove("GITUI_ASKPASS_SSH");
        Some(server)
    } else {
        None
    };
    let outcome =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string())?;
    Ok(Report {
        outcome,
        destination: request.destination.clone(),
        destination_exists: request.destination.exists(),
    })
}

// Git for Windows interprets verbatim drive paths as SCP-style remote URLs.
// Keep the canonical filesystem identity but express it as an ordinary drive
// or UNC path; core.longpaths=true still enables long-path access in Git.
fn git_local_path(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        if let Some(unc) = text.strip_prefix(r"\\?\UNC\") { return format!(r"\\{unc}"); }
        if let Some(drive) = text.strip_prefix(r"\\?\") { return drive.to_owned(); }
    }
    text
}
