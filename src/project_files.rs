//! Bounded, on-demand working-tree reads for the read-only Project tool window.
//! Unlike the path filter chooser, untracked files are included without consulting Git.
use crate::git::Cancel;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

pub const FILE_LIMIT: usize = 5 * 1024 * 1024;
pub const DIRECTORY_LIMIT: usize = 4096;
pub const PAGE_LIMIT: usize = 256;
pub const DISPLAY_NODE_LIMIT: usize = 16384;
const NAME_BYTES_LIMIT: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Directory,
    File,
    Link,
    Other,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: Kind,
}
/// Continuation is scoped to one directory revision, not a filesystem snapshot.
#[derive(Debug, Clone)]
pub struct Cursor {
    root: PathBuf,
    relative: PathBuf,
    modified: std::time::SystemTime,
    last: Entry,
}
#[derive(Debug)]
pub struct Page {
    pub entries: Vec<Entry>,
    pub next: Option<Cursor>,
}
/// Only previously expanded paths and their loaded ordering boundary are retained.
pub type ExpansionState = BTreeMap<PathBuf, Option<Entry>>;
#[derive(Debug, Default)]
pub struct RefreshedTree {
    pub directories: BTreeMap<PathBuf, Vec<Entry>>,
    pub cursors: BTreeMap<PathBuf, Cursor>,
    pub error: Option<Error>,
    pub limited: bool,
}
fn compare_entries(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    (a.kind != Kind::Directory)
        .cmp(&(b.kind != Kind::Directory))
        .then_with(|| {
            crate::natural_order::compare(
                &a.path.file_name().unwrap_or_default().to_string_lossy(),
                &b.path.file_name().unwrap_or_default().to_string_lossy(),
            )
        })
        .then_with(|| a.path.cmp(&b.path))
}
#[derive(Eq, PartialEq)]
struct OrderedEntry(Entry);
impl Ord for OrderedEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        compare_entries(&self.0, &other.0)
    }
}
impl PartialOrd for OrderedEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Content {
    Text(String),
    Binary,
    UnsupportedEncoding,
}
#[derive(Debug)]
pub enum Error {
    Cancelled,
    InvalidPath,
    LinkedPath,
    NotDirectory,
    NotFile,
    DirectoryTooLarge,
    FileTooLarge,
    ChangedDuringRead,
    Io(io::Error),
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Project {
    root: PathBuf,
}
impl Project {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(Error::NotDirectory);
        }
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Re-read expanded directories parent-first, up to their previous loaded
    /// boundary. New/closed subtrees are never recursively scanned. A failed
    /// subtree is omitted; other expanded siblings can still be restored.
    pub fn refresh_expanded(
        &self,
        expanded: &ExpansionState,
        node_budget: usize,
        cancel: &Cancel,
    ) -> Result<RefreshedTree> {
        let budget = node_budget.min(DISPLAY_NODE_LIMIT);
        let mut tree = RefreshedTree::default();
        let mut used = 0;
        for (path, boundary) in expanded {
            check_cancel(cancel)?;
            if !path.as_os_str().is_empty() {
                let visible = path
                    .parent()
                    .and_then(|parent| tree.directories.get(parent))
                    .is_some_and(|entries| {
                        entries
                            .iter()
                            .any(|e| e.path == *path && e.kind == Kind::Directory)
                    });
                if !visible {
                    continue;
                }
            }
            let mut entries = Vec::new();
            let mut cursor = None;
            loop {
                let page = match self.children_page(path, cursor.as_ref(), cancel) {
                    Ok(page) => page,
                    Err(Error::Cancelled) => return Err(Error::Cancelled),
                    Err(error) if path.as_os_str().is_empty() => return Err(error),
                    Err(error) => {
                        if tree.error.is_none() {
                            tree.error = Some(error);
                        }
                        break;
                    }
                };
                if used + entries.len() + page.entries.len() > budget {
                    tree.limited = true;
                    // Keep the previous cursor; an unaccepted page must not be skipped.
                    if !entries.is_empty() {
                        used += entries.len();
                        tree.directories.insert(path.clone(), entries);
                        if let Some(cursor) = cursor {
                            tree.cursors.insert(path.clone(), cursor);
                        }
                    }
                    break;
                }
                let reached = boundary.as_ref().is_none_or(|last| {
                    page.entries.last().is_some_and(|entry| {
                        compare_entries(entry, last) != std::cmp::Ordering::Less
                    })
                });
                entries.extend(page.entries);
                cursor = page.next;
                if reached || cursor.is_none() {
                    used += entries.len();
                    tree.directories.insert(path.clone(), entries);
                    if let Some(cursor) = cursor {
                        tree.cursors.insert(path.clone(), cursor);
                    }
                    break;
                }
            }
        }
        check_cancel(cancel)?;
        Ok(tree)
    }

    /// Read only the ancestor chain needed to reveal a file, including pages
    /// before it. Existing UI expansion is not scanned or mutated here.
    pub fn reveal_file(&self, relative: &Path, cancel: &Cancel) -> Result<RefreshedTree> {
        if !fs::metadata(self.resolve(relative, cancel)?)?.is_file() {
            return Err(Error::NotFile);
        }
        let mut expanded = ExpansionState::new();
        let mut child = relative.to_path_buf();
        let mut kind = Kind::File;
        while let Some(parent) = child.parent() {
            expanded.insert(
                parent.to_path_buf(),
                Some(Entry {
                    path: child.clone(),
                    kind,
                }),
            );
            child = parent.to_path_buf();
            kind = Kind::Directory;
        }
        let mut tree = self.refresh_expanded(&expanded, DISPLAY_NODE_LIMIT, cancel)?;
        if tree.limited {
            return Err(Error::DirectoryTooLarge);
        }
        if let Some(error) = tree.error.take() {
            return Err(error);
        }
        if !relative
            .parent()
            .and_then(|p| tree.directories.get(p))
            .is_some_and(|entries| {
                entries
                    .iter()
                    .any(|e| e.path == relative && e.kind == Kind::File)
            })
        {
            return Err(Error::InvalidPath);
        }
        Ok(tree)
    }

    // Reject traversal, Git administrative paths, and existing linked ancestors.
    // This is a workspace browser, not a filesystem security boundary against a
    // concurrently hostile process replacing directories between OS calls.
    pub(crate) fn resolve(&self, relative: &Path, cancel: &Cancel) -> Result<PathBuf> {
        check_cancel(cancel)?;
        if linked(&fs::symlink_metadata(&self.root)?) {
            return Err(Error::LinkedPath);
        }
        let mut full = self.root.clone();
        for component in relative.components() {
            match component {
                Component::CurDir => (),
                Component::Normal(name) if !git_admin(name) => {
                    full.push(name);
                    if linked(&fs::symlink_metadata(&full)?) {
                        return Err(Error::LinkedPath);
                    }
                }
                _ => return Err(Error::InvalidPath),
            }
            check_cancel(cancel)?;
        }
        Ok(full)
    }

    pub fn children(&self, relative: &Path, cancel: &Cancel) -> Result<Vec<Entry>> {
        let full = self.resolve(relative, cancel)?;
        if !fs::metadata(&full)?.is_dir() {
            return Err(Error::NotDirectory);
        }
        let mut entries = Vec::new();
        let mut name_bytes = 0;
        for entry in fs::read_dir(full)? {
            check_cancel(cancel)?;
            let entry = entry?;
            let name = entry.file_name();
            if hidden_in_project(&name) {
                continue;
            }
            let path = relative.join(&name);
            name_bytes += path.as_os_str().as_encoded_bytes().len();
            if entries.len() == DIRECTORY_LIMIT || name_bytes > NAME_BYTES_LIMIT {
                return Err(Error::DirectoryTooLarge);
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            let kind = if linked(&metadata) {
                Kind::Link
            } else if metadata.is_dir() {
                Kind::Directory
            } else if metadata.is_file() {
                Kind::File
            } else {
                Kind::Other
            };
            entries.push(Entry { path, kind });
        }
        entries.sort_by(compare_entries);
        check_cancel(cancel)?;
        Ok(entries)
    }

    /// Scan only this directory; keep the next smallest bounded page in memory.
    /// Rescanning avoids retaining an unbounded sorted listing or OS iterator.
    /// Directory timestamp checks detect ordinary external changes, not hostile
    /// replacement or mutations hidden by filesystem timestamp granularity.
    pub fn children_page(
        &self,
        relative: &Path,
        after: Option<&Cursor>,
        cancel: &Cancel,
    ) -> Result<Page> {
        let full = self.resolve(relative, cancel)?;
        let before = fs::metadata(&full)?;
        if !before.is_dir() {
            return Err(Error::NotDirectory);
        }
        let modified = before.modified()?;
        if let Some(cursor) = after {
            if cursor.root != self.root || cursor.relative != relative {
                return Err(Error::InvalidPath);
            }
            if cursor.modified != modified {
                return Err(Error::ChangedDuringRead);
            }
        }
        let mut heap = std::collections::BinaryHeap::<OrderedEntry>::new();
        let mut bytes = 0usize;
        let mut more = false;
        for item in fs::read_dir(&full)? {
            check_cancel(cancel)?;
            let item = item?;
            if hidden_in_project(&item.file_name()) {
                continue;
            }
            let metadata = fs::symlink_metadata(item.path())?;
            let kind = if linked(&metadata) {
                Kind::Link
            } else if metadata.is_dir() {
                Kind::Directory
            } else if metadata.is_file() {
                Kind::File
            } else {
                Kind::Other
            };
            let entry = Entry {
                path: relative.join(item.file_name()),
                kind,
            };
            if after.is_some_and(|cursor| {
                compare_entries(&entry, &cursor.last) != std::cmp::Ordering::Greater
            }) {
                continue;
            }
            let size = entry.path.as_os_str().as_encoded_bytes().len();
            if size > NAME_BYTES_LIMIT {
                return Err(Error::DirectoryTooLarge);
            }
            bytes += size;
            heap.push(OrderedEntry(entry));
            while heap.len() > PAGE_LIMIT || bytes > NAME_BYTES_LIMIT {
                let removed = heap.pop().unwrap();
                bytes -= removed.0.path.as_os_str().as_encoded_bytes().len();
                more = true;
            }
        }
        check_cancel(cancel)?;
        if fs::metadata(self.resolve(relative, cancel)?)?.modified()? != modified {
            return Err(Error::ChangedDuringRead);
        }
        let entries: Vec<_> = heap.into_sorted_vec().into_iter().map(|e| e.0).collect();
        let next = if more {
            entries.last().cloned().map(|last| Cursor {
                root: self.root.clone(),
                relative: relative.to_owned(),
                modified,
                last,
            })
        } else {
            None
        };
        Ok(Page { entries, next })
    }

    pub fn read(&self, relative: &Path, cancel: &Cancel) -> Result<Content> {
        let mut bytes = self.read_bytes(relative, cancel)?;
        if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
            return Ok(Content::UnsupportedEncoding);
        }
        if bytes.contains(&0) {
            return Ok(Content::Binary);
        }
        if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            bytes.drain(..3);
        }
        Ok(match String::from_utf8(bytes) {
            Ok(text) => Content::Text(text),
            Err(_) => Content::UnsupportedEncoding,
        })
    }

    /// Includes BOM and exact line endings for a writable document's baseline.
    pub fn read_bytes(&self, relative: &Path, cancel: &Cancel) -> Result<Vec<u8>> {
        let full = self.resolve(relative, cancel)?;
        let before = fs::symlink_metadata(&full)?;
        if !before.is_file() {
            return Err(Error::NotFile);
        }
        if before.len() > FILE_LIMIT as u64 {
            return Err(Error::FileTooLarge);
        }
        let mut file = fs::File::open(&full)?;
        let opened = file.metadata()?;
        if !opened.is_file() {
            return Err(Error::NotFile);
        }
        if opened.len() > FILE_LIMIT as u64 {
            return Err(Error::FileTooLarge);
        }
        let mut bytes = Vec::with_capacity(opened.len() as usize);
        let mut chunk = [0u8; 32 * 1024];
        loop {
            check_cancel(cancel)?;
            let len = file.read(&mut chunk)?;
            if len == 0 {
                break;
            }
            if bytes.len() + len > FILE_LIMIT {
                return Err(Error::FileTooLarge);
            }
            bytes.extend_from_slice(&chunk[..len]);
        }
        let after = file.metadata()?;
        let current_path = self.resolve(relative, cancel)?;
        let current = fs::metadata(current_path)?;
        if before.len() != opened.len()
            || opened.len() != bytes.len() as u64
            || after.len() != opened.len()
            || current.len() != after.len()
            || before.modified()? != opened.modified()?
            || opened.modified()? != after.modified()?
            || after.modified()? != current.modified()?
        {
            return Err(Error::ChangedDuringRead);
        }
        check_cancel(cancel)?;
        Ok(bytes)
    }
}
fn check_cancel(cancel: &Cancel) -> Result<()> {
    if cancel.cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
// Default name rules adapted from JetBrains FileTypeManagerImpl.DEFAULT_IGNORED
// (Apache-2.0); source/version and matching rules: docs/REBASED-REFERENCE.md.
// This is presentation filtering, not Git ignore evaluation or a read restriction.
fn hidden_in_project(name: &std::ffi::OsStr) -> bool {
    if git_admin(name) {
        return true;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    matches!(
        name,
        ".DS_Store"
            | ".hg"
            | ".mypy_cache"
            | ".pytest_cache"
            | ".ruff_cache"
            | ".svn"
            | "CVS"
            | "__pycache__"
            | "_svn"
            | "vssver.scc"
            | "vssver2.scc"
    ) || name.ends_with('~')
        || name.rsplit_once('.').is_some_and(|(_, ext)| {
            ["pyc", "pyo", "rbc", "yarb"]
                .iter()
                .any(|suffix| ext.eq_ignore_ascii_case(suffix))
        })
}
fn git_admin(name: &std::ffi::OsStr) -> bool {
    #[cfg(windows)]
    {
        name.to_string_lossy().eq_ignore_ascii_case(".git")
    }
    #[cfg(not(windows))]
    {
        name == ".git"
    }
}
fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
