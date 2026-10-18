//! Selected-file Commit / Amend Last, translated from Rebased GitCheckinEnvironment,
//! GitRepositoryCommitter and GitIndexInfoStagingAreaStateManager, 4aec352 (Apache-2.0).
use crate::{commit_index::ExcludedIndex, git::{Cancel, Repository, is_oid},
    operation_process::{self, Outcome, Progress}, operation_state::State};
use std::{collections::HashSet, ffi::OsStr, fs::{self, OpenOptions}, io::Write,
    path::{Component, Path, PathBuf}, time::Duration};

pub struct Request {
    /// Both rename endpoints belong here: old in remove, new in add.
    pub add: Vec<PathBuf>,
    pub remove: Vec<PathBuf>,
    pub message: String,
    pub amend: bool,
    /// None means an unborn branch, not "skip the stale HEAD check".
    pub expected_head: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase { Prepare, Remove, Add, Commit }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status { Success, RestoreFailed, RecoveryRecordFailed, Cancelled, TimedOut, SigningFailed, Empty, Failed }
pub struct Step { pub phase: Phase, pub result: Result<Outcome, String> }
pub struct Report {
    pub steps: Vec<Step>,
    pub cancelled: bool,
    pub restore: Result<(), String>,
    /// Retained only if restoration fails; contains index identities, no bodies.
    /// Caller must retain this for explicit recovery, never silently discard it.
    pub recovery_input: Option<Vec<u8>>,
    pub recovery_record: Option<PathBuf>,
    pub recovery_record_error: Option<String>,
    pub head: Result<Option<String>, String>,
    pub state: Result<State, String>,
}
impl Report {
    pub fn succeeded(&self) -> bool {
        self.status() == Status::Success
    }
    pub fn status(&self) -> Status {
        use operation_process::StopReason;
        if self.restore.is_err() { return Status::RestoreFailed; }
        if self.recovery_record_error.is_some() { return Status::RecoveryRecordFailed; }
        if self.cancelled { return Status::Cancelled; }
        let Some(step) = self.steps.last() else { return Status::Failed; };
        let Ok(out) = &step.result else { return Status::Failed; };
        match out.stopped {
            Some(StopReason::Cancelled) => return Status::Cancelled,
            Some(StopReason::TimedOut) => return Status::TimedOut,
            None => {},
        }
        if step.phase == Phase::Commit && good(&step.result) && self.head.is_ok() && self.state.is_ok() { return Status::Success; }
        if step.phase == Phase::Commit && !out.status.success() {
            if out.commit_signals.signing { return Status::SigningFailed; }
            if out.commit_signals.empty { return Status::Empty; }
        }
        Status::Failed
    }
}
fn good(result: &Result<Outcome, String>) -> bool {
    result.as_ref().is_ok_and(|o| o.status.success() && o.stopped.is_none() && o.input_error.is_none())
}
pub fn head(repo: &Repository, token: &Cancel) -> Result<Option<String>, String> {
    let out = operation_process::run(repo.command().args(["rev-parse", "--verify", "--quiet", "HEAD"]),
        token, &Progress::default(), Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
    if out.stopped.is_some() { return Err("HEAD query interrupted".into()); }
    if out.status.code() == Some(1) { return Ok(None); }
    let bytes = out.stdout.bytes();
    let value = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?.trim();
    if !out.status.success() || !is_oid(value) { return Err("Cannot determine current HEAD".into()); }
    Ok(Some(value.into()))
}

/// Worker-only; caller must refresh UI even on failed/cancelled reports.
/// The queue protects other operations in this app, not other Git clients.
pub fn execute(repo: &Repository, request: &Request, token: &Cancel, progress: &Progress) -> Result<Report, String> {
    if request.message.trim().is_empty() || request.message.contains('\0') || request.message.len() > 1024 * 1024 {
        return Err("Commit message must be nonempty and at most 1 MiB".into());
    }
    if request.add.is_empty() && request.remove.is_empty() && !request.amend { return Err("Select changes to commit".into()); }
    let adds = path_input(&request.add)?;
    let removes = path_input(&request.remove)?;
    let selected = adds.split(|b| *b == 0).chain(removes.split(|b| *b == 0))
        .filter(|p| !p.is_empty()).map(<[u8]>::to_vec).collect::<HashSet<_>>();
    let _guard = repo.operation_guard(token)?;
    let git_directory = repo.editor_directory(token)?;
    if let Some(record) = crate::commit_recovery::pending(&git_directory)? {
        return Err(format!("Pending commit index recovery must be resolved first: {}", record.display()));
    }
    let state = repo.operation_state(token)?;
    if state.in_progress() || !state.conflicts.is_empty() { return Err("Finish the current Git operation before a selected-file commit".into()); }
    let before = head(repo, token)?;
    if before != request.expected_head { return Err("HEAD changed; refresh before committing".into()); }
    if request.amend && before.is_none() { return Err("There is no commit to amend".into()); }
    for path in &request.add {
        let metadata = fs::symlink_metadata(repo.path.join(path)).map_err(|e| e.to_string())?;
        if metadata.is_dir() { return Err("Select individual files; directory/submodule commits are not yet supported".into()); }
    }
    let encoding = repo.text(&["config", "--get", "--default", "UTF-8", "i18n.commitEncoding"], token)?;
    if !matches!(encoding.trim().to_ascii_lowercase().as_str(), "utf-8" | "utf8") {
        return Err("This commit encoding is not yet supported; no files were staged".into());
    }
    let status = repo.read(&["status", "--porcelain=2", "-z", "--no-renames", "--untracked-files=no", "--ignored=no"].map(OsStr::new), token)?;
    let excluded = ExcludedIndex::capture(&status, &selected)?;
    let message = MessageFile::new(&git_directory, request.message.as_bytes())?;
    let restore_input = excluded.restore_input();
    let prepared_input = excluded.prepare_input();
    let mut recovery_record = if excluded.is_empty() { None } else {
        Some(crate::commit_recovery::create(&git_directory, before.as_deref(), &prepared_input, &restore_input)?)
    };
    let mut steps = Vec::new();
    // Once prepare is attempted, restoration is unconditional (including failed
    // start, lock errors, staging errors, hook rejection, and cancellation).
    if !excluded.is_empty() {
        steps.push(Step { phase: Phase::Prepare, result: index(repo, &prepared_input, token, progress) });
    }
    for (phase, input) in [(Phase::Remove, &removes), (Phase::Add, &adds)] {
        if steps.last().is_some_and(|s| !good(&s.result)) { break; }
        if input.is_empty() { continue; }
        let mut command = repo.operation_command();
        if phase == Phase::Remove { command.args(["rm", "--ignore-unmatch", "--cached", "-r"]); }
        else { command.args(["add", "--force"]); }
        command.args(["--pathspec-from-file=-", "--pathspec-file-nul", "--"]);
        let result = operation_process::run_with_input(&mut command, token, progress, Some(Duration::from_secs(30)), input).map_err(|e| e.to_string());
        steps.push(Step { phase, result });
    }
    if steps.last().is_none_or(|s| good(&s.result)) {
        let mut command = repo.operation_command();
        command.arg("commit").arg("-F").arg(&message.0);
        if request.amend { command.arg("--amend"); }
        command.arg("--");
        let result = operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
        steps.push(Step { phase: Phase::Commit, result });
    }
    // Fresh token: cancellation of committing must not cancel restoration.
    let cleanup = Cancel::default();
    let restore = if excluded.is_empty() { Ok(()) } else {
        let result = index(repo, &restore_input, &cleanup, &Progress::default());
        if good(&result) { Ok(()) } else { Err(match result {
            Err(error) => error,
            Ok(out) => format!("Index restoration failed: {}{}", String::from_utf8_lossy(&out.stderr.bytes()),
                out.input_error.map(|e| e.to_string()).unwrap_or_default()),
        }) }
    };
    let mut recovery_record_error = None;
    if restore.is_ok() {
        if let Some(path) = &recovery_record {
            match fs::remove_file(path) {
                Ok(()) => recovery_record = None,
                Err(error) => recovery_record_error = Some(format!("Index restored but recovery record could not be removed: {error}")),
            }
        }
    }
    Ok(Report { recovery_input: restore.is_err().then_some(restore_input), restore, steps, cancelled: token.cancelled(),
        recovery_record, recovery_record_error,
        head: head(repo, &cleanup), state: repo.operation_state(&cleanup) })
}
fn index(repo: &Repository, input: &[u8], token: &Cancel, progress: &Progress) -> Result<Outcome, String> {
    operation_process::run_with_input(repo.operation_command().args(["update-index", "-z", "--index-info"]),
        token, progress, Some(Duration::from_secs(30)), input).map_err(|e| e.to_string())
}
fn path_input(paths: &[PathBuf]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for path in paths {
        if path.as_os_str().is_empty() || path.components().any(|c| !matches!(c, Component::Normal(_)))
            || path.components().any(|c| c.as_os_str().eq_ignore_ascii_case(OsStr::new(".git"))) {
            return Err("Invalid commit path".into());
        }
        #[cfg(unix)]
        let bytes = { use std::os::unix::ffi::OsStrExt; path.as_os_str().as_bytes() };
        #[cfg(windows)]
        let normalized = path.to_str().ok_or("Invalid Windows commit path")?.replace('\\', "/");
        #[cfg(windows)]
        let bytes = normalized.as_bytes();
        if bytes.contains(&0) { return Err("Invalid commit path".into()); }
        if out.len() + bytes.len() + 1 > 8 * 1024 * 1024 { return Err("Commit path list exceeds limit".into()); }
        out.extend_from_slice(bytes); out.push(0);
    }
    Ok(out)
}
struct MessageFile(PathBuf);
impl MessageFile {
    fn new(directory: &Path, bytes: &[u8]) -> Result<Self, String> {
        let mut random = [0u8; 16]; getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let path = directory.join(format!("gitui-commit-message-{name}"));
        let mut options = OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)]
        { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        let mut file = options.open(&path).map_err(|e| e.to_string())?;
        let message = Self(path);
        file.write_all(bytes).map_err(|e| e.to_string())?;
        Ok(message)
    }
}
impl Drop for MessageFile { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }
