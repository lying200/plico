//! GitStashTracker keeps readable repositories available even during conflicts.
//! Rebased 4aec352, stash/GitStashTracker.kt (Apache-2.0).
use crate::{
    git::{Cancel, Repository},
    project_repositories::Repositories,
};
use std::path::PathBuf;

#[derive(Clone)]
pub struct SavedEntry {
    pub repository: Repository,
    pub entry: super::Entry,
}
impl std::ops::Deref for SavedEntry {
    type Target = super::Entry;
    fn deref(&self) -> &Self::Target {
        &self.entry
    }
}
impl SavedEntry {
    pub fn identity(&self) -> (PathBuf, String) {
        (self.repository.path.clone(), self.oid.clone())
    }
}

pub struct Stack {
    pub entries: Vec<SavedEntry>,
    pub more: bool,
    pub grouped: bool,
    pub errors: Vec<(PathBuf, String)>,
    pub recovery: Result<Option<crate::rebase_saved::Pending>, String>,
}

/// Bound metadata across the whole project, not 1000 entries for every child.
/// Contents are still loaded only for the selected repository and stash.
pub fn load(repo: &Repository, token: &Cancel) -> Result<Stack, String> {
    let roots = Repositories::discover(repo, token)?;
    load_in(repo, &roots, token)
}

/// Reuse project membership; refresh stash contents without rediscovering roots.
pub fn load_in(
    repo: &Repository,
    inventory: &Repositories,
    token: &Cancel,
) -> Result<Stack, String> {
    let mut roots = inventory.roots.clone();
    let mut errors = inventory.errors.clone();
    roots.sort_by(|a, b| {
        a.path
            .file_name()
            .cmp(&b.path.file_name())
            .then(a.path.cmp(&b.path))
    });
    let grouped = roots.len() > 1;
    let mut entries = Vec::new();
    let mut more = false;
    for root in roots {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let available = super::MAX_ENTRIES - entries.len();
        match super::list(&root, available.max(1), token) {
            Ok(mut stack) => {
                more |= stack.more || stack.entries.len() > available;
                stack.entries.truncate(available);
                entries.extend(stack.entries.into_iter().map(|entry| SavedEntry {
                    repository: root.clone(),
                    entry,
                }));
            }
            Err(error) => errors.push((root.path.clone(), error)),
        }
    }
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(Stack {
        entries,
        more,
        grouped,
        errors,
        recovery: crate::rebase_saved::pending(repo, token),
    })
}
