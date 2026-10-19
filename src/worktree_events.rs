//! Bounded filesystem invalidations, not a second repository cache.
use std::{collections::BTreeSet, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::{Duration, Instant}};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

#[derive(Default, Debug)]
pub struct Batch {
    pub paths: BTreeSet<PathBuf>,
    pub all: bool,
    pub index: bool,
}
impl Batch {
    pub fn changed(&self) -> bool { self.all || self.index || !self.paths.is_empty() }
    pub fn affects(&self, path: &Path) -> bool {
        self.all || self.paths.iter().any(|p| path.starts_with(p) || p.starts_with(path))
    }
    fn record(&mut self, path: PathBuf) {
        if self.all { return; }
        self.paths.insert(path);
        if self.paths.len() > 512 { self.paths.clear(); self.all = true; }
    }
}
#[derive(Default)]
struct Pending { batch: Batch, first: Option<Instant>, last: Option<Instant> }
impl Pending {
    fn push(&mut self, root: &Path, event: notify::Result<Event>) {
        match event {
            Ok(event) if matches!(event.kind, EventKind::Access(_)) => return,
            Ok(event) => {
                if event.need_rescan() { self.batch.all = true; self.batch.index = true; }
                for path in event.paths {
                    let Ok(path) = path.strip_prefix(root) else { continue; };
                    if path.starts_with(".git") {
                        if path == Path::new(".git/index") || path == Path::new(".git")
                            || path == Path::new(".git/HEAD") || path == Path::new(".git/packed-refs") || path.starts_with(".git/refs") {
                            self.batch.index = true;
                        }
                        continue;
                    }
                    if path.file_name().is_some_and(|n| n.to_string_lossy().starts_with(".plico-save-")) { continue; }
                    self.batch.record(path.to_owned());
                }
            }
            Err(_) => { self.batch.all = true; self.batch.index = true; }
        }
        if self.batch.changed() {
            let now = Instant::now();
            self.first.get_or_insert(now); self.last = Some(now);
        }
    }
    fn take(&mut self) -> Batch {
        let ready = self.last.is_some_and(|t| t.elapsed() >= Duration::from_millis(300))
            || self.first.is_some_and(|t| t.elapsed() >= Duration::from_secs(2));
        if !ready { return Batch::default(); }
        self.first = None; self.last = None;
        std::mem::take(&mut self.batch)
    }
}
pub struct Events { _watcher: RecommendedWatcher, pending: Arc<Mutex<Pending>> }
impl Events {
    /// Install off the UI thread: some backends enumerate directories here.
    pub fn new(root: PathBuf) -> notify::Result<Self> {
        let pending = Arc::new(Mutex::new(Pending::default()));
        let queue = pending.clone(); let watched = root.clone();
        let mut watcher = RecommendedWatcher::new(move |event| {
            if let Ok(mut pending) = queue.lock() { pending.push(&watched, event); }
        }, notify::Config::default().with_follow_symlinks(false))?;
        watcher.watch(&root, RecursiveMode::Recursive)?;
        Ok(Self { _watcher: watcher, pending })
    }
    pub fn take(&self) -> Batch { self.pending.lock().map(|mut p| p.take()).unwrap_or_default() }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp { len: u64, modified: Option<std::time::SystemTime>, created: Option<std::time::SystemTime> }
impl Stamp {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        let m = std::fs::symlink_metadata(path)?;
        if !m.is_file() { return Err(std::io::Error::other("Not a regular file")); }
        Ok(Self { len: m.len(), modified: m.modified().ok(), created: m.created().ok() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coalesces_filters_and_bounds_events() {
        let root = Path::new("repo"); let mut p = Pending::default();
        for path in [".git/objects/ab/cd", ".git/index.lock", ".plico-save-123.tmp"] {
            p.push(root, Ok(Event::new(EventKind::Any).add_path(root.join(path))));
        }
        assert!(!p.batch.changed());
        p.push(root, Ok(Event::new(EventKind::Any).add_path(root.join(".git/index"))));
        assert!(p.batch.index);
        p.push(root, Ok(Event::new(EventKind::Any).add_path(root.join("src"))));
        assert!(p.batch.affects(Path::new("src/a.rs")));
        assert!(!p.take().changed());
        for i in 0..600 { p.batch.record(format!("file-{i}").into()); }
        assert!(p.batch.all); assert!(p.batch.paths.is_empty());
        p.first = Some(Instant::now() - Duration::from_secs(3));
        assert!(p.take().all); assert!(!p.take().changed());
    }
}
