//! Rebased GitStashUtils: metadata first, contents only when selected.
use crate::git::{Cancel, Commit, Details, Repository, Result, is_oid};

pub const MAX_ENTRIES: usize = 1000;
#[derive(Clone, Debug)]
pub struct Entry {
    pub oid: String,
    /// Full parents: base HEAD, index, and optional untracked-files commit.
    pub parents: Vec<String>,
    pub author_time: i64,
    /// Display only: this selector changes when another stash is pushed/dropped.
    pub selector: String,
    pub branch: Option<String>,
    pub message: String,
}
pub struct Stack {
    pub entries: Vec<Entry>,
    pub more: bool,
}
pub struct Contents {
    pub commit: Commit,
    pub working: Details,
    /// Upstream exposes the index and optional untracked commits separately.
    pub parents: Vec<(Commit, Details)>,
}

/// Stable selection within a saved patch. Working/index groups can contain the
/// same path, so a filename alone cannot restore the user's current comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileIdentity {
    pub group: usize,
    pub revision: String,
    pub path: std::path::PathBuf,
    pub old_path: Option<std::path::PathBuf>,
}
impl Contents {
    pub fn file_identity(&self, group: usize, index: usize) -> Option<FileIdentity> {
        let (commit, details) = if group == 0 {
            (&self.commit, &self.working)
        } else {
            let (c, d) = self.parents.get(group - 1)?;
            (c, d)
        };
        let file = details.files.get(index)?;
        Some(FileIdentity {
            group,
            revision: commit.id.clone(),
            path: file.path.clone(),
            old_path: file.old_path.clone(),
        })
    }
}

pub fn list(repo: &Repository, limit: usize, token: &Cancel) -> Result<Stack> {
    if !(1..=MAX_ENTRIES).contains(&limit) {
        return Err("Invalid stash list limit".into());
    }
    let count = format!("--max-count={}", limit + 1);
    let output = repo.text(
        &[
            "stash",
            "list",
            "-z",
            &count,
            "--encoding=UTF-8",
            "--format=%H%x00%P%x00%at%x00%gd%x00%s",
        ],
        token,
    )?;
    let mut fields = output.split_terminator('\0');
    let mut entries = Vec::new();
    while let Some(oid) = fields.next() {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let parents = fields
            .next()
            .ok_or("Missing stash parents")?
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let author_time = fields
            .next()
            .ok_or("Missing stash time")?
            .parse()
            .map_err(|_| "Invalid stash time")?;
        let selector = fields.next().ok_or("Missing stash selector")?.to_owned();
        let subject = fields.next().ok_or("Missing stash subject")?;
        if !is_oid(oid) || !(2..=3).contains(&parents.len()) || parents.iter().any(|id| !is_oid(id))
        {
            return Err("Invalid stash commit identity or parents".into());
        }
        let (branch, message) = match subject.split_once(':') {
            Some((branch, message)) => (Some(branch.trim().to_owned()), message.trim().to_owned()),
            None => (None, subject.trim().to_owned()),
        };
        entries.push(Entry {
            oid: oid.into(),
            parents,
            author_time,
            selector,
            branch,
            message,
        });
        if entries.len() > limit {
            break;
        }
    }
    let more = entries.len() > limit;
    entries.truncate(limit);
    Ok(Stack { entries, more })
}

/// Use immutable identity, never the position-dependent stash@{n} selector.
pub fn contents(repo: &Repository, entry: &Entry, token: &Cancel) -> Result<Contents> {
    let snapshot = repo.snapshot_commits(
        std::slice::from_ref(&entry.oid),
        std::slice::from_ref(&entry.oid),
        token,
    )?;
    let commit = snapshot
        .commits
        .into_iter()
        .next()
        .ok_or("Saved stash commit is unavailable")?;
    if !(2..=3).contains(&commit.parents.len()) {
        return Err("Not a stash commit".into());
    }
    let working = repo.details(&commit, token)?;
    let parent_ids = &commit.parents[1..];
    let parents = repo
        .snapshot_commits(parent_ids, parent_ids, token)?
        .commits
        .into_iter()
        .map(|parent| {
            repo.details(&parent, token)
                .map(|details| (parent, details))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Contents {
        commit,
        working,
        parents,
    })
}
