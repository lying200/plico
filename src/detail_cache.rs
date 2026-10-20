//! Window-local, bounded Log detail cache. Refreshed history invalidates the cache;
//! branch containment and write-operation preflight are deliberately not cached.
use crate::git::{Commit, Details};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
};
const MAX_ENTRIES: usize = 32;
const MAX_BYTES: usize = 2 * 1024 * 1024;
struct Entry {
    repository: PathBuf,
    id: String,
    parents: Vec<String>,
    shallow: bool,
    value: Arc<Details>,
    bytes: usize,
}
#[derive(Default)]
pub struct Cache {
    entries: VecDeque<Entry>,
    bytes: usize,
}
impl Cache {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
    pub fn get(&mut self, repository: &Path, commit: &Commit) -> Option<Arc<Details>> {
        let index = self.entries.iter().position(|entry| {
            entry.repository == repository
                && entry.id == commit.id
                && entry.parents == commit.parents
                && entry.shallow == commit.shallow_boundary
        })?;
        let entry = self.entries.remove(index)?;
        let value = entry.value.clone();
        self.entries.push_back(entry);
        Some(value)
    }
    pub fn insert(&mut self, repository: &Path, commit: &Commit, value: Arc<Details>) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.repository == repository && entry.id == commit.id)
        {
            self.bytes -= self.entries.remove(index).unwrap().bytes;
        }
        let bytes = std::mem::size_of::<Entry>()
            + std::mem::size_of::<Details>()
            + repository.as_os_str().len()
            + commit.id.capacity()
            + commit.parents.capacity() * std::mem::size_of::<String>()
            + commit.parents.iter().map(String::capacity).sum::<usize>()
            + [
                &value.message,
                &value.author,
                &value.authored,
                &value.committer,
                &value.committed,
                &value.base,
            ]
            .iter()
            .map(|s| s.capacity())
            .sum::<usize>()
            + value.files.capacity() * std::mem::size_of::<crate::git::Change>()
            + value
                .files
                .iter()
                .map(|f| {
                    f.path.capacity()
                        + f.old_path.as_ref().map_or(0, |p| p.capacity())
                        + f.status.capacity()
                })
                .sum::<usize>();
        if bytes > MAX_BYTES {
            return;
        }
        while self.entries.len() >= MAX_ENTRIES || self.bytes + bytes > MAX_BYTES {
            let Some(old) = self.entries.pop_front() else {
                break;
            };
            self.bytes -= old.bytes;
        }
        self.bytes += bytes;
        self.entries.push_back(Entry {
            repository: repository.into(),
            id: commit.id.clone(),
            parents: commit.parents.clone(),
            shallow: commit.shallow_boundary,
            value,
            bytes,
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn commit(id: usize) -> Commit {
        Commit {
            id: format!("{id:040x}"),
            parents: vec![],
            shallow_boundary: false,
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            refs: vec![],
        }
    }
    fn details(message: String) -> Arc<Details> {
        Arc::new(Details {
            message,
            author: String::new(),
            authored: String::new(),
            committer: String::new(),
            committed: String::new(),
            base: String::new(),
            files: vec![],
        })
    }
    #[test]
    fn hits_are_shared_bounded_and_scoped_to_repository_and_comparison() {
        let mut cache = Cache::default();
        let path = Path::new("repo");
        let value = details("message".into());
        cache.insert(path, &commit(0), value.clone());
        assert!(Arc::ptr_eq(&cache.get(path, &commit(0)).unwrap(), &value));
        assert!(cache.get(Path::new("other"), &commit(0)).is_none());
        let mut shallow = commit(0);
        shallow.shallow_boundary = true;
        assert!(cache.get(path, &shallow).is_none());
        for id in 1..MAX_ENTRIES {
            cache.insert(path, &commit(id), details(String::new()));
        }
        cache.get(path, &commit(0));
        cache.insert(path, &commit(MAX_ENTRIES), details(String::new()));
        assert!(cache.get(path, &commit(1)).is_none());
        assert!(cache.get(path, &commit(0)).is_some());
        cache.insert(path, &commit(99), details("x".repeat(MAX_BYTES + 1)));
        assert!(cache.get(path, &commit(99)).is_none());
        assert!(cache.bytes <= MAX_BYTES);
        cache.clear();
        assert!(cache.get(path, &commit(0)).is_none());
        assert_eq!(cache.bytes, 0);
    }
}
