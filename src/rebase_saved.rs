//! Worktree-scoped ownership of local changes saved for a rebase. Rebased keeps
//! this identity in GitStashChangesSaver; this bounded record survives restart.
use crate::git::is_oid;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
// Preserve the on-disk format used by released Squash recovery records.
const NAME: &str = "gitui-squash-saved";
const MAGIC: &str = "GitReview squash saved v1";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub head: String,
    pub base: String,
    pub stash: String,
    pub branch: Option<String>,
}
pub fn path(directory: &Path) -> PathBuf {
    directory.join(NAME)
}
fn valid(record: &Record) -> bool {
    [&record.head, &record.base, &record.stash]
        .iter()
        .all(|s| is_oid(s))
        && record.branch.as_ref().is_none_or(|s| {
            s.starts_with("refs/heads/") && s.len() <= 4096 && !s.contains(['\n', '\r', '\0'])
        })
}
pub fn read(directory: &Path) -> Result<Option<Record>, String> {
    let path = path(directory);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() {
        return Err("Rebase saved-changes record is not a regular file".into());
    }
    let mut text = String::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(8193)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let parts = text.split('\n').collect::<Vec<_>>();
    if text.len() > 8192 || parts.len() != 6 || parts[0] != MAGIC || parts[5] != "" {
        return Err("Invalid rebase saved-changes record".into());
    }
    let record = Record {
        head: parts[1].into(),
        base: parts[2].into(),
        stash: parts[3].into(),
        branch: (!parts[4].is_empty()).then(|| parts[4].into()),
    };
    if !valid(&record) {
        return Err("Invalid rebase saved-changes identities".into());
    }
    Ok(Some(record))
}
pub(crate) fn create(directory: &Path, record: &Record) -> Result<(), String> {
    if !valid(record) {
        return Err("Invalid rebase saved-changes identities".into());
    }
    let path = path(directory);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|e| e.to_string())?;
    let text = format!(
        "{MAGIC}\n{}\n{}\n{}\n{}\n",
        record.head,
        record.base,
        record.stash,
        record.branch.as_deref().unwrap_or("")
    );
    let result = file
        .write_all(text.as_bytes())
        .and_then(|_| file.sync_all());
    drop(file);
    if let Err(e) = result {
        let _ = fs::remove_file(path);
        return Err(e.to_string());
    }
    Ok(())
}
pub(crate) fn remove(directory: &Path, record: &Record) -> Result<(), String> {
    if read(directory)?.as_ref() != Some(record) {
        return Err("Rebase saved-changes ownership changed".into());
    }
    fs::remove_file(path(directory)).map_err(|e| e.to_string())
}

/// Capture ownership before Git removes rebase metadata. Caller holds the
/// common-dir operation guard. An unrelated rebase must not load this stash.
pub(crate) fn for_rebase(directory: &Path) -> Result<Option<Record>, String> {
    let Some(record) = read(directory)? else {
        return Ok(None);
    };
    let merge = directory.join("rebase-merge");
    let apply = directory.join("rebase-apply");
    let rebase = match (merge.is_dir(), apply.is_dir()) {
        (true, false) => merge,
        (false, true) => apply,
        _ => return Err("Cannot identify the rebase owning saved changes".into()),
    };
    for (name, expected) in [
        ("orig-head", record.head.as_str()),
        ("onto", record.base.as_str()),
        (
            "head-name",
            record.branch.as_deref().unwrap_or("detached HEAD"),
        ),
    ] {
        let mut value = String::new();
        fs::File::open(rebase.join(name))
            .map_err(|e| e.to_string())?
            .take(8193)
            .read_to_string(&mut value)
            .map_err(|e| e.to_string())?;
        if value.trim_end() != expected {
            return Err("Saved rebase changes belong to a different rebase".into());
        }
    }
    Ok(Some(record))
}

/// Called once after successful Continue/Abort, under the existing guard.
/// Failed or conflicting restoration retains both stash and ownership record.
pub(crate) fn restore(
    repo: &crate::git::Repository,
    directory: &Path,
    record: &Record,
    aborted: bool,
    progress: &crate::operation_process::Progress,
) -> Result<(), String> {
    use crate::{git::Cancel, operation_process};
    let token = Cancel::default();
    if read(directory)?.as_ref() != Some(record) {
        return Err("Saved rebase ownership changed".into());
    }
    let state = repo.operation_state(&token)?;
    if state.in_progress() || !state.conflicts.is_empty() {
        return Err("Finish the Git operation before restoring saved changes".into());
    }
    if crate::squash_check::symbolic_head(repo, &token)? != record.branch {
        return Err("Branch changed; saved rebase changes were not restored".into());
    }
    if aborted && crate::commit_operation::head(repo, &token)?.as_deref() != Some(&record.head) {
        return Err("Abort did not restore the original HEAD; saved changes were retained".into());
    }
    if repo.stash_head(&token)?.as_deref() != Some(&record.stash) {
        return Err("Saved changes are no longer the top stash; automatic restore stopped".into());
    }
    crate::conflicts::clear_saved_context(repo, &token)?;
    let output = operation_process::run(
        repo.operation_command().args(["stash", "pop", "--index"]),
        &token,
        progress,
        None,
    )
    .map_err(|e| e.to_string());
    crate::conflicts::record_saved_context(repo)?;
    let output = output?;
    if !output.status.success() || output.stopped.is_some() || output.input_error.is_some() {
        return Err(
            "Could not restore saved rebase changes; inspect the retained stash and working tree"
                .into(),
        );
    }
    remove(directory, record)
}

/// A bounded snapshot for an explicit "already handled externally" confirmation.
/// Private bytes bind confirmation to the exact worktree record, including a
/// malformed record. This is not evidence that its saved changes were restored.
#[derive(Clone, Debug)]
pub struct Pending {
    directory: PathBuf,
    bytes: Vec<u8>,
}
fn pending_bytes(directory: &Path) -> Result<Option<Vec<u8>>, String> {
    let file = path(directory);
    let metadata = match fs::symlink_metadata(&file) {
        Ok(value) => value,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() {
        return Err("Recovery record is not a regular file".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Recovery record is a reparse point".into());
        }
    }
    let mut bytes = Vec::new();
    fs::File::open(file)
        .map_err(|e| e.to_string())?
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8192 {
        return Err("Recovery record exceeds the size limit".into());
    }
    Ok(Some(bytes))
}
pub fn pending(
    repo: &crate::git::Repository,
    token: &crate::git::Cancel,
) -> Result<Option<Pending>, String> {
    let directory = repo.editor_directory(token)?;
    let bytes = pending_bytes(&directory)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(bytes.map(|bytes| Pending { directory, bytes }))
}
/// Forget only the app's ownership record after explicit user acknowledgement.
/// Never applies/drops a stash or changes HEAD, index, worktree or Git recovery
/// metadata. External Git clients are not serialized by the application guard.
pub fn acknowledge_external_recovery(
    repo: &crate::git::Repository,
    expected: &Pending,
    token: &crate::git::Cancel,
) -> Result<(), String> {
    let _guard = repo.operation_guard(token)?;
    let directory = repo.editor_directory(token)?;
    if directory != expected.directory {
        return Err("Recovery belongs to another worktree".into());
    }
    let state = repo.operation_state(token)?;
    if state.in_progress() || !state.conflicts.is_empty() {
        return Err(
            "Finish the Git operation and resolve conflicts before acknowledging recovery".into(),
        );
    }
    if pending_bytes(&directory)?.as_ref() != Some(&expected.bytes) {
        return Err("Recovery record changed; review it again".into());
    }
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    fs::remove_file(path(&directory)).map_err(|e| e.to_string())
}
