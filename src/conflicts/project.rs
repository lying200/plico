//! Project-wide merge-dialog input (Rebased GitResolveConflictsAction, 4aec352).
//! Each row retains its own operation snapshot; relative paths alone are not keys.
use super::{File, Snapshot};
use crate::{
    git::{Cancel, Repository},
    recent_projects,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    root: String,
    path: PathBuf,
}
impl Key {
    pub fn new(repository: &Repository, path: &Path) -> Self {
        Self {
            root: recent_projects::identity(&repository.path),
            path: path.to_owned(),
        }
    }
}
#[derive(Clone)]
pub struct Entry {
    snapshot: Arc<Snapshot>,
    index: usize,
    key: Key,
}
impl Entry {
    pub fn key(&self) -> &Key {
        &self.key
    }
    pub fn snapshot(&self) -> &Arc<Snapshot> {
        &self.snapshot
    }
    pub fn file(&self) -> &File {
        &self.snapshot.files[self.index]
    }
    pub fn full_path(&self) -> PathBuf {
        self.snapshot.repository().path.join(&self.file().path)
    }
}

/// Retain each file's own operation snapshot, including per-root side orientation.
pub fn entries(snapshot: Arc<Snapshot>) -> Vec<Entry> {
    (0..snapshot.files.len())
        .map(|index| Entry {
            key: Key::new(&snapshot.repository(), &snapshot.files[index].path),
            snapshot: snapshot.clone(),
            index,
        })
        .collect()
}

/// Read all roots without publishing a partial list on failure or cancellation.
/// File versions remain lazy: only index identities and operation metadata are read.
pub fn read(roots: &[Repository], token: &Cancel) -> Result<Vec<Entry>, String> {
    let mut entries = read_snapshots(roots, token)?
        .into_iter()
        .flat_map(self::entries)
        .collect::<Vec<_>>();
    entries.sort_by_cached_key(Entry::full_path);
    Ok(entries)
}

pub fn read_snapshots(roots: &[Repository], token: &Cancel) -> Result<Vec<Arc<Snapshot>>, String> {
    if roots.len() > 4096 {
        return Err("Project repository count exceeds the supported size".into());
    }
    let mut seen = BTreeSet::new();
    let mut snapshots = Vec::new();
    for repository in roots {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if !seen.insert(recent_projects::identity(&repository.path)) {
            continue;
        }
        let snapshot = Arc::new(
            Snapshot::read(repository, token)
                .map_err(|error| format!("{}: {error}", repository.path.display()))?,
        );
        snapshots.push(snapshot);
    }
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(snapshots)
}
