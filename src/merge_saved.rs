//! Worktree-scoped saved changes for Smart Merge. Rebased GitPreservingProcess
//! and GitStashChangesSaver (4aec352, JetBrains, Apache-2.0); persisted locally so
//! closing the window does not discard ownership of a retained stash.
use crate::{
    git::{Cancel, Repository, is_oid},
    operation_process::{self, Progress},
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};
const NAME: &str = "plico-merge-saved";
const MAGIC: &str = "Plico merge saved v1";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub head: String,
    pub target: String,
    pub branch: String,
    pub stash: String,
}
fn valid(r: &Record) -> bool {
    [&r.head, &r.target, &r.stash].iter().all(|s| is_oid(s))
        && r.branch.starts_with("refs/heads/")
        && r.branch.len() <= 4096
        && !r.branch.contains(['\0', '\r', '\n'])
}
pub fn read(directory: &Path) -> Result<Option<Record>, String> {
    let path = directory.join(NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Invalid merge recovery record file".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Invalid merge recovery record file".into());
        }
    }
    let mut text = String::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(8193)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let parts = text.split('\n').collect::<Vec<_>>();
    if text.len() > 8192 || parts.len() != 6 || parts[0] != MAGIC || parts[5] != "" {
        return Err("Invalid merge recovery record".into());
    }
    let record = Record {
        head: parts[1].into(),
        target: parts[2].into(),
        branch: parts[3].into(),
        stash: parts[4].into(),
    };
    if !valid(&record) {
        return Err("Invalid merge recovery identities".into());
    }
    Ok(Some(record))
}
pub(crate) fn create(directory: &Path, record: &Record) -> Result<(), String> {
    if !valid(record) {
        return Err("Invalid merge recovery identities".into());
    }
    let path = directory.join(NAME);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|e| e.to_string())?;
    let bytes = format!(
        "{MAGIC}\n{}\n{}\n{}\n{}\n",
        record.head, record.target, record.branch, record.stash
    );
    let result = file
        .write_all(bytes.as_bytes())
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
        return Err("Saved merge changes ownership changed".into());
    }
    fs::remove_file(directory.join(NAME)).map_err(|e| e.to_string())
}
/// Must be called under the operation guard, before consuming MERGE_HEAD.
pub(crate) fn for_merge(repo: &Repository, token: &Cancel) -> Result<Option<Record>, String> {
    let Some(record) = read(&repo.editor_directory(token)?)? else {
        return Ok(None);
    };
    if crate::commit_operation::head(repo, token)?.as_ref() != Some(&record.head)
        || crate::squash_check::symbolic_head(repo, token)?.as_ref() != Some(&record.branch)
        || repo
            .text(&["rev-parse", "--verify", "MERGE_HEAD"], token)?
            .trim()
            != record.target
    {
        return Err("Saved changes belong to a different merge; inspect the retained stash".into());
    }
    Ok(Some(record))
}
pub(crate) fn restore(
    repo: &Repository,
    record: &Record,
    aborted: bool,
    progress: &Progress,
) -> Result<(), String> {
    let token = Cancel::default();
    let directory = repo.editor_directory(&token)?;
    if read(&directory)?.as_ref() != Some(record)
        || crate::squash_check::symbolic_head(repo, &token)?.as_ref() != Some(&record.branch)
    {
        return Err("Saved merge changes ownership changed".into());
    }
    let state = repo.operation_state(&token)?;
    if state.in_progress() || !state.conflicts.is_empty() {
        return Err("Finish the merge before restoring saved changes".into());
    }
    if aborted {
        if crate::commit_operation::head(repo, &token)?.as_ref() != Some(&record.head) {
            return Err("Merge abort did not restore the original HEAD".into());
        }
    } else {
        let parents = repo.text(&["show", "-s", "--format=%P", "HEAD"], &token)?;
        if crate::commit_operation::head(repo, &token)?.as_ref() != Some(&record.target)
            && parents.split_whitespace().collect::<Vec<_>>()
                != [record.head.as_str(), record.target.as_str()]
        {
            return Err("Merge result changed; saved changes were retained".into());
        }
    }
    if repo.stash_head(&token)?.as_ref() != Some(&record.stash) {
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
            "Saved merge changes could not be restored completely; inspect the retained stash"
                .into(),
        );
    }
    remove(&directory, record)
}

/// A reviewable recovery decision. Revalidate the repository when the user
/// acts; a stale dialog must not restore into another branch or clear new data.
#[derive(Clone, Debug)]
pub struct Pending {
    pub record: Record,
    pub can_restore: bool,
    directory: std::path::PathBuf,
    head: Option<String>,
    branch: Option<String>,
}
impl Pending {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Option<Self>, String> {
        let directory = repo.editor_directory(token)?;
        let Some(record) = read(&directory)? else {
            return Ok(None);
        };
        let head = crate::commit_operation::head(repo, token)?;
        let branch = crate::squash_check::symbolic_head(repo, token)?;
        let state = repo.operation_state(token)?;
        let identity = branch.as_ref() == Some(&record.branch)
            && (head.as_ref() == Some(&record.head)
                || head.as_ref() == Some(&record.target)
                || repo
                    .text(&["show", "-s", "--format=%P", "HEAD"], token)?
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    == [record.head.as_str(), record.target.as_str()]);
        let can_restore = identity
            && !state.in_progress()
            && state.conflicts.is_empty()
            && repo.stash_head(token)?.as_ref() == Some(&record.stash)
            && repo
                .text(&["status", "--porcelain", "--untracked-files=no"], token)?
                .is_empty();
        Ok(Some(Self {
            record,
            can_restore,
            directory,
            head,
            branch,
        }))
    }
    fn validate(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.directory
            || read(&self.directory)?.as_ref() != Some(&self.record)
            || crate::commit_operation::head(repo, token)? != self.head
            || crate::squash_check::symbolic_head(repo, token)? != self.branch
        {
            return Err("Merge recovery changed; reopen the recovery dialog".into());
        }
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the Git operation and resolve conflicts before clearing or retrying recovery".into());
        }
        Ok(())
    }
    /// Explicit retry only from a clean tracked tree. A previous failed pop may
    /// have applied some files: do not blindly apply that stash a second time.
    pub fn retry(
        &self,
        repo: &Repository,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        let _guard = repo.operation_guard(token)?;
        self.validate(repo, token)?;
        let fresh = Self::read(repo, token)?.ok_or("Merge recovery record disappeared")?;
        if !fresh.can_restore {
            return Err("Review local changes and the saved stash before retrying recovery".into());
        }
        // restore checks all ownership/operation constraints again, then uses a
        // non-cancellable restore section, matching GitPreservingProcess.load.
        restore(
            repo,
            &self.record,
            self.head.as_ref() == Some(&self.record.head),
            progress,
        )
    }
    /// Explicit acknowledgement only; does not alter worktree, index or stash.
    pub fn acknowledge(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let _guard = repo.operation_guard(token)?;
        self.validate(repo, token)?;
        remove(&self.directory, &self.record)
    }
}
