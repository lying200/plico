//! Actual Git administrative paths, distinct from the worktree being observed.
use crate::git::{Cancel, Repository};
use std::path::{Path, PathBuf};

pub(super) struct GitPaths {
    directory: PathBuf,
    common: PathBuf,
}
impl GitPaths {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        Ok(Self {
            directory: repo.editor_directory(token)?,
            common: repo.operation_key(token)?,
        })
    }
    pub fn directories(&self) -> [&Path; 2] {
        [&self.directory, &self.common]
    }
    /// Windows reports the administrative directory's timestamp when Git
    /// replaces index.lock. This is not HEAD/ref movement. Keep removal/rename
    /// events and pointer-file edits, which may change the repository layout.
    pub fn is_directory_metadata(&self, path: &Path, kind: notify::EventKind) -> bool {
        matches!(
            kind,
            notify::EventKind::Modify(
                notify::event::ModifyKind::Any | notify::event::ModifyKind::Metadata(_)
            )
        ) && self.directories().iter().any(|directory| {
            relative(path, directory).is_some_and(|suffix| suffix.as_os_str().is_empty())
        })
    }
    /// Normalize only the event address. The synthetic .git path never reaches
    /// a Git command or filesystem write.
    pub fn route(&self, root: &Path, path: &Path) -> Option<PathBuf> {
        if let Some(relative) = relative(path, &self.directory) {
            return Some(root.join(".git").join(relative));
        }
        if let Some(relative) = relative(path, &self.common) {
            // A linked worktree must not observe the main/another worktree's
            // HEAD, index or sequencer as its own. Config, refs and shallow are shared.
            if relative == Path::new("packed-refs")
                || relative == Path::new("config")
                || relative == Path::new("shallow")
                || relative.starts_with("refs")
                || relative.starts_with("reftable")
            {
                return Some(root.join(".git").join(relative));
            }
            return None;
        }
        Some(path.to_owned())
    }
}

fn relative(path: &Path, root: &Path) -> Option<PathBuf> {
    if let Ok(relative) = path.strip_prefix(root) {
        return Some(relative.to_owned());
    }
    // Windows Git may return an ordinary path while the watcher reports a
    // verbatim one. Preserve the suffix spelling (notably HEAD and ref names).
    let path_key = crate::recent_projects::identity(path);
    let root_key = crate::recent_projects::identity(root);
    let suffix = Path::new(&path_key).strip_prefix(&root_key).ok()?;
    let count = suffix.components().count();
    let components: Vec<_> = path.components().collect();
    Some(
        components[components.len().checked_sub(count)?..]
            .iter()
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn administrative_directory_timestamp_does_not_mean_refs_changed() {
        use notify::{
            EventKind,
            event::{ModifyKind, RemoveKind},
        };
        let paths = GitPaths {
            directory: "/main/.git/worktrees/topic".into(),
            common: "/main/.git".into(),
        };
        for directory in paths.directories() {
            assert!(paths.is_directory_metadata(directory, EventKind::Modify(ModifyKind::Any)));
            assert!(!paths.is_directory_metadata(directory, EventKind::Remove(RemoveKind::Any)));
        }
        for file in [
            "/topic/.git",
            "/main/.git/worktrees/topic/HEAD",
            "/main/.git/refs/heads/main",
        ] {
            assert!(
                !paths.is_directory_metadata(Path::new(file), EventKind::Modify(ModifyKind::Any))
            );
        }
    }
    #[test]
    fn linked_worktree_routes_only_its_index_and_shared_refs() {
        let paths = GitPaths {
            directory: "/main/.git/worktrees/topic".into(),
            common: "/main/.git".into(),
        };
        let root = Path::new("/topic");
        assert_eq!(
            paths.route(root, Path::new("/main/.git/worktrees/topic/index")),
            Some(root.join(".git/index"))
        );
        assert_eq!(
            paths.route(root, Path::new("/main/.git/refs/heads/feature")),
            Some(root.join(".git/refs/heads/feature"))
        );
        assert_eq!(
            paths.route(root, Path::new("/main/.git/shallow")),
            Some(root.join(".git/shallow"))
        );
        assert_eq!(
            paths.route(root, Path::new("/main/.git/config")),
            Some(root.join(".git/config"))
        );
        assert_eq!(
            paths.route(
                root,
                Path::new("/main/.git/worktrees/topic/config.worktree")
            ),
            Some(root.join(".git/config.worktree"))
        );
        for path in [
            "/main/.git/index",
            "/main/.git/config.worktree",
            "/main/.git/HEAD",
            "/main/.git/worktrees/other/index",
            "/main/.git/objects/ab/cd",
        ] {
            assert_eq!(paths.route(root, Path::new(path)), None, "{path}");
        }
        assert_eq!(
            paths.route(root, Path::new("/topic/file.txt")),
            Some(root.join("file.txt"))
        );
    }
}
