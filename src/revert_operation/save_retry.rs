//! GitApplyChangesProcess save-and-retry: save explicitly, retry only remaining
//! commits, load on complete success. Failures retain the stash for recovery.
use super::*;
use std::{
    fs,
    io::{Read, Write},
};
const NAME: &str = "plico-revert-saved";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedChanges {
    pub stash: String,
    pub head: String,
    pub branch: Option<String>,
    directory: PathBuf,
}
pub struct SavedReport {
    pub batch: Option<BatchReport>,
    pub saved: Option<SavedChanges>,
    pub error: Option<String>,
}
impl SavedChanges {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Option<Self>, String> {
        let directory = repo.editor_directory(token)?;
        let path = directory.join(NAME);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 8192 {
            return Err("Invalid saved Revert changes record".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("Invalid saved Revert changes record".into());
            }
        }
        let mut text = String::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(8193)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        let lines = text.split('\n').collect::<Vec<_>>();
        if text.len() > 8192
            || lines.len() != 5
            || lines[0] != "Plico revert saved v1"
            || lines[4] != ""
            || !is_oid(lines[1])
            || !is_oid(lines[2])
            || (!lines[3].is_empty()
                && (!lines[3].starts_with("refs/heads/") || lines[3].contains(['\0', '\r'])))
        {
            return Err("Invalid saved Revert identities".into());
        }
        Ok(Some(Self {
            stash: lines[1].into(),
            head: lines[2].into(),
            branch: (!lines[3].is_empty()).then(|| lines[3].into()),
            directory,
        }))
    }
    pub(super) fn for_recovery(
        repo: &Repository,
        expected: &Prepared,
        token: &Cancel,
    ) -> Result<Option<Self>, String> {
        let Some(saved) = Self::read(repo, token)? else {
            return Ok(None);
        };
        if saved.directory != expected.directory || saved.branch != expected.branch {
            return Err("Saved changes belong to a different Revert".into());
        }
        repo.text(
            &["merge-base", "--is-ancestor", &saved.head, &expected.head],
            token,
        )
        .map_err(|_| "Saved changes belong to a different Revert history".to_owned())?;
        Ok(Some(saved))
    }

    /// The caller owns the write guard and supplies the verified operation result.
    /// No refresh-and-assume: a changed HEAD or unrelated top stash stops loading.
    pub(super) fn restore_locked(
        &self,
        repo: &Repository,
        expected_head: &str,
        progress: &Progress,
    ) -> Result<(), String> {
        let token = Cancel::default();
        if Self::read(repo, &token)?.as_ref() != Some(self)
            || crate::commit_operation::head(repo, &token)?.as_deref() != Some(expected_head)
            || crate::squash_check::symbolic_head(repo, &token)? != self.branch
        {
            return Err("Repository or saved Revert changes changed; stash retained".into());
        }
        let state = repo.operation_state(&token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the Git operation before restoring saved Revert changes".into());
        }
        if repo.stash_head(&token)?.as_deref() != Some(&self.stash) {
            return Err(
                "Saved changes are no longer the top stash; automatic restore stopped".into(),
            );
        }
        crate::conflicts::clear_saved_context(repo, &token)?;
        let outcome = operation_process::run(
            repo.operation_command().args(["stash", "pop", "--index"]),
            &token,
            progress,
            None,
        )
        .map_err(|e| e.to_string());
        crate::conflicts::record_saved_context(repo)?;
        let outcome = outcome?;
        if !outcome.status.success() || outcome.stopped.is_some() || outcome.input_error.is_some() {
            return Err("Could not restore saved Revert changes; inspect the retained stash and working tree".into());
        }
        self.remove(repo)
    }

    fn create(&self) -> Result<(), String> {
        let text = format!(
            "Plico revert saved v1\n{}\n{}\n{}\n",
            self.stash,
            self.head,
            self.branch.as_deref().unwrap_or("")
        );
        if text.len() > 8192 {
            return Err("Saved Revert record is too large".into());
        }
        let path = self.directory.join(NAME);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|e| e.to_string())?;
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
    fn remove(&self, repo: &Repository) -> Result<(), String> {
        if Self::read(repo, &Cancel::default())?.as_ref() != Some(self) {
            return Err("Saved Revert ownership changed".into());
        }
        fs::remove_file(self.directory.join(NAME)).map_err(|e| e.to_string())
    }
}
impl Batch {
    /// Caller presents the failed batch's explicit Save and Retry choice. Uses
    /// one repository guard for save, all retry commits and immediate restore.
    pub fn save_and_execute(
        &self,
        repo: &Repository,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<SavedReport, String> {
        let mut session = crate::update_session::Session::begin(repo, token)?;
        let first = self.commits.first().ok_or("No commits selected")?;
        first.validate(repo, token)?;
        if editor.is_empty() {
            return Err("Commit editor is unavailable".into());
        }
        if Recovery::record_exists(repo, token)? || SavedChanges::read(repo, token)?.is_some() {
            return Err("Handle the previous Revert recovery before saving again".into());
        }
        let save = session.save("Plico: save changes before Revert", token, progress);
        let mut report = SavedReport {
            batch: None,
            saved: session.saved_oid().map(|stash| SavedChanges {
                stash: stash.into(),
                head: first.head.clone(),
                branch: first.branch.clone(),
                directory: first.directory.clone(),
            }),
            error: None,
        };
        // Record even a failed save if Git produced a stash. Never hide the OID.
        if let Some(saved) = &report.saved {
            if let Err(error) = saved.create() {
                report.error = Some(error);
                return Ok(report);
            }
        }
        match save {
            Ok(output)
                if output.status.success()
                    && output.stopped.is_none()
                    && output.input_error.is_none() => {}
            Ok(output) => {
                report.error = Some(format!(
                    "Could not save local changes: {}",
                    String::from_utf8_lossy(&output.stderr.bytes())
                ));
                return Ok(report);
            }
            Err(error) => {
                report.error = Some(error);
                return Ok(report);
            }
        }
        let batch = match self.execute_locked(repo, editor, requests, token, progress) {
            Ok(batch) => batch,
            Err(error) => {
                report.error = Some(error);
                return Ok(report);
            }
        };
        let complete =
            batch.remaining.is_empty() && batch.error.is_none() && batch.recovery.is_none();
        report.batch = Some(batch);
        if complete {
            match session.restore_saved(true, progress) {
                Ok(outcome)
                    if outcome.as_ref().is_none_or(|o| {
                        o.status.success() && o.stopped.is_none() && o.input_error.is_none()
                    }) =>
                {
                    if let Some(saved) = &report.saved {
                        match saved.remove(repo) {
                            Ok(()) => report.saved = None,
                            Err(error) => report.error = Some(error),
                        }
                    }
                }
                Ok(_) => report.error = Some(
                    "Could not restore saved changes; inspect the retained stash and working tree"
                        .into(),
                ),
                Err(error) => report.error = Some(error),
            }
        }
        Ok(report)
    }
}

/// Snapshot for explicit recovery after an external abort or failed stash pop.
#[derive(Clone, Debug)]
pub struct SavedPending {
    pub record: SavedChanges,
    head: Option<String>,
    branch: Option<String>,
    pub can_restore: bool,
    pub can_clear: bool,
}
impl SavedPending {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Option<Self>, String> {
        let Some(record) = SavedChanges::read(repo, token)? else {
            return Ok(None);
        };
        let head = crate::commit_operation::head(repo, token)?;
        let branch = crate::squash_check::symbolic_head(repo, token)?;
        let state = repo.operation_state(token)?;
        let can_clear = !state.in_progress() && state.conflicts.is_empty();
        let can_restore = can_clear
            && branch == record.branch
            && repo.stash_head(token)?.as_ref() == Some(&record.stash)
            && head.as_ref().is_some_and(|head| {
                repo.text(&["merge-base", "--is-ancestor", &record.head, head], token)
                    .is_ok()
            })
            && repo
                .text(&["status", "--porcelain", "--untracked-files=no"], token)?
                .is_empty();
        Ok(Some(Self {
            record,
            head,
            branch,
            can_restore,
            can_clear,
        }))
    }
    fn validate(&self, repo: &Repository, token: &Cancel) -> Result<Self, String> {
        let current = Self::read(repo, token)?.ok_or("Saved Revert record no longer exists")?;
        if current.record != self.record
            || current.head != self.head
            || current.branch != self.branch
            || !current.can_clear
        {
            return Err("Repository or saved Revert record changed; review it again".into());
        }
        Ok(current)
    }
    pub fn retry(
        &self,
        repo: &Repository,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        let _guard = repo.operation_guard(token)?;
        let current = self.validate(repo, token)?;
        if !self.can_restore || !current.can_restore {
            return Err("Saved Revert changes cannot be restored to this working tree".into());
        }
        self.record.restore_locked(
            repo,
            self.head.as_deref().ok_or("No HEAD for restoration")?,
            progress,
        )
    }
    pub fn acknowledge(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let _guard = repo.operation_guard(token)?;
        self.validate(repo, token)?;
        if !self.can_clear {
            return Err("Finish the current Git operation before clearing saved changes".into());
        }
        self.record.remove(repo)
    }
}
impl SavedChanges {
    pub fn record_exists(repo: &Repository, token: &Cancel) -> Result<bool, String> {
        Self::record_exists_at(&repo.editor_directory(token)?)
    }
    pub(crate) fn record_exists_at(directory: &std::path::Path) -> Result<bool, String> {
        match fs::symlink_metadata(directory.join(NAME)) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
}
