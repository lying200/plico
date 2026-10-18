//! Repository selection in Rebased PushLog: reverse outgoing rows, then zipChanges.
//! Only revision/path metadata is retained; file contents are loaded on demand.
use crate::{combined_changes::{self, Change}, git::{Cancel, Repository, Result}, push_target::Target};
use std::sync::Arc;

pub struct Snapshot {
    pub files: Vec<Change>,
    pub commit_count: usize,
    /// Rebased limits a new branch's outgoing history to 1000 commits.
    pub capped: bool,
    pub references: Arc<str>,
}

pub fn load(repo: &Repository, source: &str, target: &Target, expected: Option<&str>,
    max_commits: usize, token: &Cancel) -> Result<Snapshot> {
    let shallow = repo.shallow_boundaries(token)?;
    let outgoing = repo.outgoing_preview_commits(source, target, expected, max_commits, token)?;
    let count = outgoing.commits.len();
    let mut changes = Vec::new();
    for mut commit in outgoing.commits.into_iter().rev() {
        if token.cancelled() { return Err("Cancelled".into()); }
        commit.shallow_boundary = shallow.1.contains(&commit.id);
        let files = repo.push_commit_changes(&commit, token)?;
        if files.len() > combined_changes::MAX_INPUT_CHANGES.saturating_sub(changes.len()) {
            return Err("Combined preview exceeds the change limit".into());
        }
        changes.extend(files);
    }
    if repo.shallow_boundaries(token)? != shallow {
        return Err("Shallow history changed; reopen the push dialog".into());
    }
    // Changes are immutable objects, but their membership depends on the remote
    // refs. Revalidate after collecting files as well as between history pages.
    repo.outgoing_commits_checked(source, target, 0, 1, Some(&outgoing.references), token)?;
    let files = combined_changes::zip_changes(changes)?;
    if token.cancelled() { return Err("Cancelled".into()); }
    Ok(Snapshot { files, commit_count: count, capped: outgoing.capped, references: outgoing.references })
}
