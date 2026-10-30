//! Bounded invalidation ranges for status queries, not commit inclusion ranges.
use crate::git::Change;
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope(Option<BTreeSet<PathBuf>>);

impl Scope {
    pub fn full() -> Self {
        Self(None)
    }
    pub fn empty() -> Self {
        Self(Some(BTreeSet::new()))
    }
    pub fn paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut scope = Self::empty();
        for path in paths {
            scope.insert(path);
            if scope.is_full() {
                break;
            }
        }
        scope
    }
    pub fn from_batch(batch: &crate::worktree_events::Batch) -> Self {
        if batch.baseline_changed() || batch.membership || batch.shallow || batch.status_rules {
            Self::full()
        } else {
            Self::paths(batch.paths.iter().cloned())
        }
    }
    /// Translate repository-local events through the project's declared gitlinks.
    /// Unknown membership cannot establish the boundary of a partial query.
    pub fn for_event(
        repository: &Path,
        observed: &Path,
        batch: &crate::worktree_events::Batch,
        catalog: Option<&crate::project_repositories::Repositories>,
    ) -> Self {
        if !batch.changed() {
            return Self::empty();
        }
        let Some(catalog) = catalog.filter(|c| c.errors.is_empty() && c.submodules.len() <= 4096)
        else {
            return Self::full();
        };
        let key = crate::recent_projects::identity;
        let target = key(repository);
        if key(observed) == target {
            let mut scope = Self::from_batch(batch);
            for module in &catalog.submodules {
                if key(&module.parent) == target
                    && scope
                        .query_paths()
                        .is_some_and(|paths| paths.iter().any(|path| under(path, &module.path)))
                {
                    scope.insert(module.path.clone());
                }
            }
            return scope;
        }
        let mut pending = vec![key(observed)];
        let mut visited = BTreeSet::new();
        let mut scope = Self::empty();
        while let Some(root) = pending.pop() {
            if !visited.insert(root.clone()) {
                continue;
            }
            for module in &catalog.submodules {
                if key(&module.root) == root {
                    let parent = key(&module.parent);
                    if parent == target {
                        scope.insert(module.path.clone());
                    } else if !visited.contains(&parent) {
                        pending.push(parent);
                    }
                }
            }
        }
        scope
    }
    pub fn is_full(&self) -> bool {
        self.0.is_none()
    }
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_some_and(BTreeSet::is_empty)
    }
    pub fn merge(&mut self, other: &Self) {
        match &other.0 {
            None => self.0 = None,
            Some(paths) => {
                for path in paths {
                    self.insert(path.clone());
                }
            }
        }
    }
    fn insert(&mut self, path: PathBuf) {
        let Some(paths) = &mut self.0 else {
            return;
        };
        if path.as_os_str().is_empty()
            || (cfg!(windows) && !path.as_os_str().as_encoded_bytes().is_ascii())
            || path
                .components()
                .any(|p| !matches!(p, Component::Normal(_)))
            || path
                .components()
                .any(|p| component_matches(p.as_os_str(), std::ffi::OsStr::new(".git")))
            || path.file_name().is_some_and(|n| {
                [".gitignore", ".gitattributes", ".gitmodules"]
                    .iter()
                    .any(|rule| component_matches(n, std::ffi::OsStr::new(rule)))
            })
        {
            self.0 = None;
            return;
        }
        if paths.iter().any(|parent| under(&path, parent)) {
            return;
        }
        paths.retain(|child| !under(child, &path));
        paths.insert(path);
        // Avoid splitting rename candidates across commands. Fall back to one
        // full query if the bounded command line cannot contain the whole range.
        if paths.len() > 512
            || paths
                .iter()
                .map(|p| p.as_os_str().len() + 18)
                .sum::<usize>()
                > 6000
        {
            self.0 = None;
        }
    }
    pub(crate) fn expanded(&self, previous: &[Change]) -> Self {
        let mut scope = self.clone();
        if scope.is_full() || scope.is_empty() {
            return scope;
        }
        for file in previous {
            if scope.is_full() {
                break;
            }
            if let Some(old) = &file.old_path {
                scope.insert(old.clone());
                scope.insert(file.path.clone());
            } else if file
                .status
                .bytes()
                .any(|b| matches!(b, b'A' | b'D' | b'R' | b'C'))
            {
                scope.insert(file.path.clone());
            }
        }
        scope
    }
    pub(crate) fn query_paths(&self) -> Option<&BTreeSet<PathBuf>> {
        self.0.as_ref()
    }
    pub(crate) fn contains(&self, path: &Path) -> bool {
        self.0
            .as_ref()
            .is_none_or(|paths| paths.iter().any(|p| under(path, p)))
    }
    pub(crate) fn contains_change(&self, file: &Change) -> bool {
        self.contains(&file.path) || file.old_path.as_ref().is_some_and(|p| self.contains(p))
    }
}

fn component_matches(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    if cfg!(windows) {
        a.as_encoded_bytes()
            .eq_ignore_ascii_case(b.as_encoded_bytes())
    } else {
        a == b
    }
}

fn under(path: &Path, parent: &Path) -> bool {
    let mut parts = path.components();
    parent.components().all(|part| {
        parts
            .next()
            .is_some_and(|candidate| component_matches(candidate.as_os_str(), part.as_os_str()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_scope_uses_the_same_ascii_matching_as_its_query() {
        let scope = Scope::paths(["DIR/File.txt".into(), "dir".into()]);
        assert_eq!(scope.query_paths().unwrap().len(), 1);
        assert!(scope.contains(Path::new("Dir/FILE.TXT")));
        assert!(!scope.contains(Path::new("Directory/File.txt")));
        for path in [".GITIGNORE", "sub/.GitAttributes", "子目录/file"] {
            assert!(Scope::paths([path.into()]).is_full());
        }
    }
    #[test]
    fn ranges_collapse_subtrees_and_unknown_or_excessive_ranges_become_full() {
        let mut scope = Scope::paths(["dir/file".into(), "dir-other/file".into()]);
        scope.merge(&Scope::paths(["dir".into()]));
        assert_eq!(scope.query_paths().unwrap().len(), 2);
        assert!(scope.contains(Path::new("dir/deleted/child")));
        assert!(!scope.contains(Path::new("dir-other/missing")));
        for path in [
            "",
            "../outside",
            "nested/.gitignore",
            ".git/info/exclude",
            ".gitattributes",
        ] {
            assert!(Scope::paths([path.into()]).is_full(), "{path}");
        }
        assert!(Scope::paths((0..513).map(|n| format!("f{n}").into())).is_full());
        assert!(Scope::paths(["x".repeat(6000).into()]).is_full());
        scope.merge(&Scope::full());
        assert!(scope.is_full());
    }
}
