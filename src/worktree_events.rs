//! Bounded filesystem invalidations, not a second repository cache.
mod configuration;
mod git_paths;
mod ignore_rules;
mod project;
pub use project::{Notice, ProjectEvents};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Default, Debug)]
pub struct Batch {
    pub paths: BTreeSet<PathBuf>,
    pub all: bool,
    pub index: bool,
    /// HEAD or references changed; distinct from staging-only updates.
    pub refs: bool,
    /// Shallow boundaries changed without necessarily moving any reference.
    pub shallow: bool,
    /// Object interpretation may have changed (replace refs or packed/reftable
    /// ref storage, where the affected namespace is not available).
    pub interpretation: bool,
    /// Effective Git configuration changed; fixed Diff endpoints stay fixed.
    pub configuration: bool,
    /// Only diff.* settings changed; membership and history rows remain valid.
    pub diff_configuration: bool,
    /// Ignore file contents changed without moving HEAD or altering history.
    pub status_rules: bool,
    /// Repository roots, declarations or gitlinks may have changed.
    pub membership: bool,
}
impl Batch {
    pub fn changed(&self) -> bool {
        self.all
            || self.index
            || self.refs
            || self.interpretation
            || self.configuration
            || self.diff_configuration
            || self.status_rules
            || self.membership
            || !self.paths.is_empty()
    }
    pub fn history_changed(&self) -> bool {
        self.all || self.refs || self.interpretation || self.configuration
    }
    /// Local comparisons depend on both HEAD and the index.
    pub fn baseline_changed(&self) -> bool {
        self.history_changed() || self.index || self.diff_configuration
    }
    pub fn affects(&self, path: &Path) -> bool {
        self.all
            || self
                .paths
                .iter()
                .any(|p| path.starts_with(p) || p.starts_with(path))
    }
    /// UI invalidation only: canonical Windows roots may have a verbatim prefix
    /// while watcher paths do not. Never use these display identities for I/O.
    pub fn affects_root(&self, observed_root: &Path, root: &Path) -> bool {
        let observed = crate::recent_projects::identity(observed_root);
        let root = crate::recent_projects::identity(root);
        Path::new(&root)
            .strip_prefix(Path::new(&observed))
            .is_ok_and(|relative| {
                self.paths.iter().any(|path| {
                    relative.starts_with(Path::new(&crate::recent_projects::identity(path)))
                })
            })
    }
    fn record(&mut self, path: PathBuf) {
        if self.all {
            return;
        }
        self.paths.insert(path);
        if self.paths.len() > 512 {
            self.paths.clear();
            self.all = true;
        }
    }
}
/// Independent deadlines for open-document polling and repository fallback.
/// File events never postpone either deadline. Advance only when writes are idle.
pub struct Schedule {
    next_documents: Instant,
    next_repository: Instant,
    fallback: bool,
    initial: bool,
}
impl Schedule {
    pub fn new(now: Instant, fallback: bool) -> Self {
        Self {
            next_documents: now,
            next_repository: now,
            fallback,
            initial: true,
        }
    }
    pub fn advance(&mut self, batch: &mut Batch, now: Instant) -> bool {
        if std::mem::take(&mut self.initial) || (self.fallback && now >= self.next_repository) {
            batch.all = true;
            batch.index = true;
            self.next_repository = now + Duration::from_secs(30);
        }
        let poll = now >= self.next_documents;
        if poll {
            self.next_documents = now + Duration::from_secs(2);
        }
        poll
    }
}

#[derive(Default)]
struct Pending {
    batch: Batch,
    first: Option<Instant>,
    last: Option<Instant>,
}
impl Pending {
    fn push(&mut self, root: &Path, event: notify::Result<Event>) {
        match event {
            Ok(event) if matches!(event.kind, EventKind::Access(_)) => return,
            Ok(event) => {
                if event.need_rescan() {
                    self.batch.all = true;
                    self.batch.index = true;
                }
                for path in event.paths {
                    let Ok(path) = path.strip_prefix(root) else {
                        continue;
                    };
                    if path
                        .file_name()
                        .is_some_and(|name| name == ".git" || name == ".gitmodules")
                        || (path.file_name().is_some_and(|name| name == "index")
                            && path.components().any(|part| part.as_os_str() == ".git"))
                    {
                        self.batch.membership = true;
                    }
                    if path.starts_with(".git") {
                        if path == Path::new(".git/info/exclude") || path == Path::new(".git/info")
                        {
                            self.batch.status_rules = true;
                        }
                        if path == Path::new(".git/config")
                            || path == Path::new(".git/config.worktree")
                        {
                            self.batch.configuration = true;
                        }
                        if path.starts_with(".git/refs/replace")
                            || path == Path::new(".git/packed-refs")
                            || path.starts_with(".git/reftable")
                        {
                            self.batch.interpretation = true;
                        }
                        if path == Path::new(".git/index") || path == Path::new(".git") {
                            self.batch.index = true;
                        }
                        if path == Path::new(".git/shallow") {
                            self.batch.shallow = true;
                            self.batch.refs = true;
                        }
                        if path == Path::new(".git")
                            || path == Path::new(".git/HEAD")
                            || path == Path::new(".git/packed-refs")
                            || path.starts_with(".git/refs")
                            || path.starts_with(".git/reftable")
                        {
                            self.batch.refs = true;
                        }
                        continue;
                    }
                    // Directory removal may hide a .git entry before the
                    // watcher can report it individually. Rediscovery remains
                    // limited to registered roots and declared submodules.
                    if matches!(
                        event.kind,
                        EventKind::Create(notify::event::CreateKind::Folder)
                            | EventKind::Remove(notify::event::RemoveKind::Folder)
                    ) {
                        self.batch.membership = true;
                    }
                    if path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with(".plico-save-"))
                    {
                        continue;
                    }
                    self.batch.record(path.to_owned());
                }
            }
            Err(_) => {
                self.batch.all = true;
                self.batch.index = true;
            }
        }
        if self.batch.changed() {
            let now = Instant::now();
            self.first.get_or_insert(now);
            self.last = Some(now);
        }
    }
    fn take(&mut self) -> Batch {
        let ready = self
            .last
            .is_some_and(|t| t.elapsed() >= Duration::from_millis(300))
            || self
                .first
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(2));
        if !ready {
            return Batch::default();
        }
        self.first = None;
        self.last = None;
        std::mem::take(&mut self.batch)
    }
}
pub struct Events {
    _watcher: RecommendedWatcher,
    pending: Arc<Mutex<Pending>>,
}
impl Events {
    /// Install off the UI thread: some backends enumerate directories here.
    pub fn new(root: PathBuf) -> notify::Result<Self> {
        Self::install(root, None)
    }
    /// Resolve Git directories off the UI thread. Failure lets callers retain
    /// their existing polling fallback instead of silently missing index events.
    pub fn for_repository(
        repo: &crate::git::Repository,
        token: &crate::git::Cancel,
    ) -> Result<Self, String> {
        let paths = git_paths::GitPaths::read(repo, token)?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Self::install(repo.path.clone(), Some(paths)).map_err(|error| error.to_string())
    }
    fn install(root: PathBuf, paths: Option<git_paths::GitPaths>) -> notify::Result<Self> {
        let mut directories = vec![root.clone()];
        if let Some(paths) = &paths {
            for path in paths.directories() {
                if !directories.iter().any(|parent| path.starts_with(parent)) {
                    directories.push(path.to_owned());
                }
            }
        }
        let pending = Arc::new(Mutex::new(Pending::default()));
        let queue = pending.clone();
        let watched = root.clone();
        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<Event>| {
                let event = event.map(|mut event| {
                    if let Some(paths) = &paths {
                        event.paths = event
                            .paths
                            .iter()
                            .filter(|path| !paths.is_directory_metadata(path, event.kind))
                            .filter_map(|path| paths.route(&watched, path))
                            .collect();
                    }
                    event
                });
                if let Ok(mut pending) = queue.lock() {
                    pending.push(&watched, event);
                }
            },
            notify::Config::default().with_follow_symlinks(false),
        )?;
        for directory in directories {
            watcher.watch(&directory, RecursiveMode::Recursive)?;
        }
        Ok(Self {
            _watcher: watcher,
            pending,
        })
    }
    pub fn take(&self) -> Batch {
        self.pending
            .lock()
            .map(|mut p| p.take())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
}
impl Stamp {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        let m = std::fs::symlink_metadata(path)?;
        if !m.is_file() {
            return Err(std::io::Error::other("Not a regular file"));
        }
        Ok(Self {
            len: m.len(),
            modified: m.modified().ok(),
            created: m.created().ok(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn events_do_not_accelerate_or_starve_document_polling() {
        let now = Instant::now();
        let mut schedule = Schedule::new(now, false);
        let mut initial = Batch::default();
        assert!(schedule.advance(&mut initial, now));
        assert!(initial.all);
        let mut polls = 0;
        for tick in 1..=10 {
            let mut batch = Batch::default();
            batch.paths.insert("unrelated.txt".into());
            polls +=
                usize::from(schedule.advance(&mut batch, now + Duration::from_millis(tick * 400)));
            assert!(!batch.all);
        }
        assert_eq!(polls, 2);
        let mut fallback = Schedule::new(now, true);
        fallback.advance(&mut Batch::default(), now);
        let mut batch = Batch::default();
        fallback.advance(&mut batch, now + Duration::from_secs(29));
        assert!(!batch.changed());
        fallback.advance(&mut batch, now + Duration::from_secs(30));
        assert!(batch.all && batch.index);
    }
    #[test]
    fn staging_and_history_invalidation_have_distinct_consumers() {
        let root = Path::new("repo");
        for path in [".git/info/exclude", ".git/info"] {
            let mut pending = Pending::default();
            pending.push(
                root,
                Ok(Event::new(EventKind::Any).add_path(root.join(path))),
            );
            assert!(pending.batch.changed() && pending.batch.status_rules);
            assert!(!pending.batch.history_changed() && !pending.batch.baseline_changed());
            assert!(crate::changes_scope::Scope::from_batch(&pending.batch).is_full());
        }
        let diff = Batch {
            diff_configuration: true,
            ..Default::default()
        };
        assert!(diff.changed() && diff.baseline_changed());
        assert!(!diff.history_changed() && !diff.membership);
        for path in [".git/config", ".git/config.worktree"] {
            let mut pending = Pending::default();
            pending.push(
                root,
                Ok(Event::new(EventKind::Any).add_path(root.join(path))),
            );
            assert!(pending.batch.configuration && pending.batch.history_changed());
            assert!(!pending.batch.index && !pending.batch.refs && !pending.batch.interpretation);
        }
        for (path, index, refs) in [
            (".git/index", true, false),
            (".git/HEAD", false, true),
            (".git/refs/heads/main", false, true),
            (".git/refs/tags/v1", false, true),
            (".git/refs/remotes/origin/main", false, true),
            (".git/refs/replace/object", false, true),
            (".git/packed-refs", false, true),
            (".git/shallow", false, true),
            (".git/reftable/tables.list", false, true),
        ] {
            let mut pending = Pending::default();
            pending.push(
                root,
                Ok(Event::new(EventKind::Any).add_path(root.join(path))),
            );
            assert_eq!(pending.batch.index, index, "{path}");
            assert_eq!(pending.batch.refs, refs, "{path}");
            assert_eq!(
                pending.batch.interpretation,
                path == ".git/packed-refs"
                    || path.starts_with(".git/refs/replace")
                    || path.starts_with(".git/reftable"),
                "only replacement or opaque ref storage reinterprets existing objects: {path}"
            );
            assert_eq!(pending.batch.history_changed(), refs, "{path}");
            assert!(pending.batch.baseline_changed(), "{path}");
        }
        // Deepening replaces the boundary file; unshallow removes it entirely.
        for kind in [
            EventKind::Create(notify::event::CreateKind::File),
            EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::To,
            )),
            EventKind::Remove(notify::event::RemoveKind::File),
        ] {
            let mut pending = Pending::default();
            pending.push(
                root,
                Ok(Event::new(kind).add_path(root.join(".git/shallow"))),
            );
            assert!(pending.batch.shallow && pending.batch.history_changed());
            assert!(!pending.batch.index);
        }
        let overflow = Batch {
            all: true,
            ..Default::default()
        };
        assert!(overflow.history_changed() && overflow.baseline_changed());
        let mut mixed = Pending::default();
        for path in [".git/index", ".git/refs/heads/main"] {
            mixed.push(
                root,
                Ok(Event::new(EventKind::Any).add_path(root.join(path))),
            );
        }
        assert!(mixed.batch.index && mixed.batch.refs);
    }
    #[test]
    fn repository_membership_events_are_separate_from_ordinary_edits_and_refs() {
        let root = Path::new("repo");
        for path in [
            ".git",
            ".gitmodules",
            ".git/index",
            "child/.git",
            "child/.gitmodules",
            ".git/modules/child/index",
        ] {
            let mut pending = Pending::default();
            pending.push(
                root,
                Ok(Event::new(EventKind::Any).add_path(root.join(path))),
            );
            assert!(pending.batch.membership, "{path}");
            assert!(pending.batch.changed());
        }
        for path in [
            "src/main.rs",
            "child/src/main.rs",
            ".git/HEAD",
            ".git/refs/heads/main",
            ".git/index.lock",
            ".git/objects/ab/cd",
        ] {
            let mut pending = Pending::default();
            pending.push(
                root,
                Ok(Event::new(EventKind::Any).add_path(root.join(path))),
            );
            assert!(!pending.batch.membership, "{path}");
        }
        let mut pending = Pending::default();
        pending.push(
            root,
            Ok(
                Event::new(EventKind::Remove(notify::event::RemoveKind::Folder))
                    .add_path(root.join("child")),
            ),
        );
        assert!(
            pending.batch.membership,
            "deleted directories cannot be inspected for .git"
        );
    }
    #[cfg(windows)]
    #[test]
    fn root_renames_match_verbatim_drive_and_unc_paths() {
        let batch = Batch {
            paths: [PathBuf::from("child")].into(),
            ..Default::default()
        };
        assert!(batch.affects_root(Path::new(r"C:\project"), Path::new(r"\\?\C:\project\child")));
        assert!(batch.affects_root(
            Path::new(r"\\server\share\project"),
            Path::new(r"\\?\UNC\server\share\project\child")
        ));
        assert!(!batch.affects_root(
            Path::new(r"C:\project"),
            Path::new(r"\\?\C:\project-other\child")
        ));
        assert!(!batch.affects_root(
            Path::new(r"C:\project"),
            Path::new(r"\\?\C:\project\child-other")
        ));
    }
    #[test]
    fn coalesces_filters_and_bounds_events() {
        let root = Path::new("repo");
        let mut p = Pending::default();
        for path in [
            ".git/objects/ab/cd",
            ".git/index.lock",
            ".plico-save-123.tmp",
        ] {
            p.push(
                root,
                Ok(Event::new(EventKind::Any).add_path(root.join(path))),
            );
        }
        assert!(!p.batch.changed());
        p.push(
            root,
            Ok(
                Event::new(EventKind::Create(notify::event::CreateKind::Folder))
                    .add_path(root.join(".git/objects/ab")),
            ),
        );
        assert!(
            !p.batch.changed(),
            "new object directories are not repository membership changes"
        );
        p.push(
            root,
            Ok(Event::new(EventKind::Any).add_path(root.join(".git/index"))),
        );
        assert!(p.batch.index);
        p.push(
            root,
            Ok(Event::new(EventKind::Any).add_path(root.join("src"))),
        );
        assert!(p.batch.affects(Path::new("src/a.rs")));
        assert!(!p.take().changed());
        for i in 0..600 {
            p.batch.record(format!("file-{i}").into());
        }
        assert!(p.batch.all);
        assert!(p.batch.paths.is_empty());
        p.first = Some(Instant::now() - Duration::from_secs(3));
        assert!(p.take().all);
        assert!(!p.take().changed());
    }
}
