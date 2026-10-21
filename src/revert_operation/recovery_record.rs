//! Durable continuation for our one-at-a-time Revert queue. Git owns the
//! current REVERT_HEAD; this record owns only the remaining application queue.
use super::*;
use std::{
    fs,
    io::{Read, Write},
};
const NAME: &str = "plico-revert-queue";
const MAGIC: &str = "Plico revert queue v1";
const LIMIT: u64 = 72_000;

impl Recovery {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Option<Self>, String> {
        let directory = repo.editor_directory(token)?;
        let Some(record) = Self::read_record(&directory)? else {
            return Ok(None);
        };
        record.validate_stopped(repo, token)?;
        Ok(Some(record))
    }

    fn read_record(directory: &std::path::Path) -> Result<Option<Self>, String> {
        let path = directory.join(NAME);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > LIMIT {
            return Err("Invalid revert queue file".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("Invalid revert queue file".into());
            }
        }
        let mut text = String::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(LIMIT + 1)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        let lines = text.lines().collect::<Vec<_>>();
        if text.len() as u64 > LIMIT
            || !text.ends_with('\n')
            || lines.len() < 4
            || lines.len() > 1003
            || lines[0] != MAGIC
            || !is_oid(lines[1])
            || (!lines[2].is_empty()
                && (!lines[2].starts_with("refs/heads/") || lines[2].contains(['\0', '\r'])))
            || !lines[3..].iter().all(|s| is_oid(s))
            || lines[3..]
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != lines.len() - 3
        {
            return Err("Invalid revert queue identities".into());
        }
        Ok(Some(Self {
            expected: Prepared {
                target: lines[3].into(),
                subject: String::new(),
                head: lines[1].into(),
                branch: (!lines[2].is_empty()).then(|| lines[2].into()),
                directory: directory.into(),
            },
            remaining: lines[3..].iter().map(|s| (*s).into()).collect(),
        }))
    }

    fn same_queue(&self, other: &Self) -> bool {
        self.expected.directory == other.expected.directory
            && self.expected.head == other.expected.head
            && self.expected.branch == other.expected.branch
            && self.expected.target == other.expected.target
            && self.remaining == other.remaining
    }

    pub(super) fn validate_record(&self) -> Result<(), String> {
        if let Some(record) = Self::read_record(&self.expected.directory)? {
            if !self.same_queue(&record) {
                return Err("Saved revert queue changed".into());
            }
        }
        Ok(())
    }

    pub(super) fn persist(&self) -> Result<(), String> {
        if let Some(existing) = Self::read_record(&self.expected.directory)? {
            return if self.same_queue(&existing) {
                Ok(())
            } else {
                Err("A different saved revert queue requires attention".into())
            };
        }
        let text = format!(
            "{MAGIC}\n{}\n{}\n{}\n",
            self.expected.head,
            self.expected.branch.as_deref().unwrap_or(""),
            self.remaining.join("\n")
        );
        if text.len() as u64 > LIMIT {
            return Err("Revert queue is too large".into());
        }
        let path = self.expected.directory.join(NAME);
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

    /// Caller holds the repository write guard and has completed/aborted this
    /// exact operation. Do not remove a record replaced by another window.
    pub(crate) fn clear_record(&self) -> Result<(), String> {
        let Some(record) = Self::read_record(&self.expected.directory)? else {
            return Ok(());
        };
        if !self.same_queue(&record) {
            return Err("Saved revert queue changed".into());
        }
        fs::remove_file(self.expected.directory.join(NAME)).map_err(|e| e.to_string())
    }
}

/// Reviewable snapshot of an application queue, including externally ended
/// operations. Clearing it never changes Git's sequencer, index or worktree.
#[derive(Clone, Debug)]
pub struct Pending {
    recovery: Recovery,
    head: Option<String>,
    branch: Option<String>,
    pub can_clear: bool,
}
impl Pending {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Option<Self>, String> {
        let Some(recovery) = Recovery::read_record(&repo.editor_directory(token)?)? else {
            return Ok(None);
        };
        let head = crate::commit_operation::head(repo, token)?;
        let branch = crate::squash_check::symbolic_head(repo, token)?;
        let state = repo.operation_state(token)?;
        Ok(Some(Self {
            recovery,
            head,
            branch,
            can_clear: !state.in_progress() && state.conflicts.is_empty(),
        }))
    }
    pub fn targets(&self) -> &[String] {
        &self.recovery.remaining
    }
    pub fn clear(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let _guard = repo.operation_guard(token)?;
        let current = Self::read(repo, token)?.ok_or("Revert queue no longer exists")?;
        if !self.can_clear
            || !current.can_clear
            || self.head != current.head
            || self.branch != current.branch
            || !self.recovery.same_queue(&current.recovery)
        {
            return Err("Repository or revert queue changed; review it again".into());
        }
        self.recovery.clear_record()
    }
}

impl Recovery {
    pub fn record_exists(repo: &Repository, token: &Cancel) -> Result<bool, String> {
        let path = repo.editor_directory(token)?.join(NAME);
        match fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
}
