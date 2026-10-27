//! Rebased GitInteractiveRebaseDialog.loadChanges: selected rows in display order,
//! immutable commit metadata and zipChanges. No working-tree reads or file bodies.
use crate::{
    combined_changes::{self, Change},
    commit_presentation::{self, Presentation},
    git::{Cancel, OUTPUT_LIMIT, Repository},
};
use std::collections::{HashMap, HashSet};

pub struct Card {
    pub id: String,
    pub presentation: Presentation,
}
pub struct Selection {
    pub cards: Vec<Card>,
    pub files: Vec<Change>,
}
pub fn load(repo: &Repository, ids: &[String], token: &Cancel) -> Result<Selection, String> {
    if ids.is_empty() {
        return Ok(Selection {
            cards: vec![],
            files: vec![],
        });
    }
    if ids.iter().collect::<HashSet<_>>().len() != ids.len() {
        return Err("Duplicate rebase selection".into());
    }
    let snapshot = repo.snapshot_commits(ids, ids, token)?;
    let mut commits: HashMap<_, _> = snapshot
        .commits
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
    let mut cards = Vec::new();
    let mut changes = Vec::new();
    let mut bytes = 0usize;
    // GitLogUtil uses --no-walk=unsorted. Do not sort by date or the edited plan's
    // original chronology: the upstream dialog passes the current row order.
    for id in ids {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let commit = commits
            .remove(id)
            .ok_or("Selected rebase commit is unavailable")?;
        let details = repo.commit_metadata(&commit, token)?;
        let presentation = commit_presentation::build(&details, Some(&commit));
        let files = repo.log_commit_changes(&commit, token)?;
        bytes = bytes
            .saturating_add(presentation.message.len())
            .saturating_add(presentation.metadata.iter().map(String::len).sum::<usize>());
        for revision in files
            .iter()
            .flat_map(|f| f.before.iter().chain(f.after.iter()))
        {
            bytes = bytes
                .saturating_add(revision.path.as_os_str().len())
                .saturating_add(revision.revision.len());
        }
        if bytes > OUTPUT_LIMIT
            || files.len() > combined_changes::MAX_INPUT_CHANGES.saturating_sub(changes.len())
        {
            return Err("Combined preview exceeds the change limit".into());
        }
        cards.push(Card {
            id: id.clone(),
            presentation,
        });
        changes.extend(files);
    }
    let files = combined_changes::zip_changes(changes)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(Selection { cards, files })
}
