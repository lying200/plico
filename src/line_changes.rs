//! Source-editor Git markers. Rebased LineStatusTracker uses a VCS baseline,
//! independent of the last save; changes are applied through the editor undo stack.
use crate::{
    file_document::Document,
    git::{Cancel, Repository},
};
use std::{
    ffi::OsStr,
    ops::Range,
    path::{Component, Path},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub original: Range<usize>,
    pub current: Range<usize>,
}
impl Block {
    pub fn kind(&self) -> Kind {
        if self.original.is_empty() {
            Kind::Added
        } else if self.current.is_empty() {
            Kind::Deleted
        } else {
            Kind::Modified
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Added,
    Modified,
    Deleted,
}

/// Only committed regular files with a resolved index have an ordinary baseline.
/// Never interpret an IO/Git error as an empty original (which would allow a
/// destructive rollback). New/untracked files deliberately have no markers.
pub fn base_oid(repo: &Repository, path: &Path, stop: &Cancel) -> Result<Option<String>, String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Invalid source path".into());
    }
    let index = repo.read(
        &[
            OsStr::new("--literal-pathspecs"),
            OsStr::new("ls-files"),
            OsStr::new("--stage"),
            OsStr::new("-z"),
            OsStr::new("--"),
            path.as_os_str(),
        ],
        stop,
    )?;
    let records: Vec<_> = index.split(|b| *b == 0).filter(|r| !r.is_empty()).collect();
    if records.len() != 1 {
        return Ok(None);
    }
    let header = records[0].split(|b| *b == b'\t').next().unwrap_or_default();
    if !header.ends_with(b" 0")
        || !(header.starts_with(b"100644 ") || header.starts_with(b"100755 "))
    {
        return Ok(None);
    }
    let heads = repo.read(
        &[
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new("--quiet"),
            OsStr::new("HEAD"),
        ],
        stop,
    )?;
    let head = String::from_utf8(heads)
        .map_err(|_| "Invalid HEAD")?
        .trim()
        .to_owned();
    if !crate::git::is_oid(&head) {
        return Err("Invalid HEAD".into());
    }
    let tree = repo.read(
        &[
            OsStr::new("--literal-pathspecs"),
            OsStr::new("ls-tree"),
            OsStr::new("-z"),
            OsStr::new(&head),
            OsStr::new("--"),
            path.as_os_str(),
        ],
        stop,
    )?;
    let Some(record) = tree.split(|b| *b == 0).find(|r| !r.is_empty()) else {
        return Ok(None);
    };
    let header = record.split(|b| *b == b'\t').next().unwrap_or_default();
    let fields: Vec<_> = header.split(|b| *b == b' ').collect();
    if fields.len() != 3 || fields[1] != b"blob" || !matches!(fields[0], b"100644" | b"100755") {
        return Ok(None);
    }
    let oid = std::str::from_utf8(fields[2]).map_err(|_| "Invalid blob ID")?;
    if !crate::git::is_oid(oid) {
        return Err("Invalid blob ID".into());
    }
    Ok(Some(oid.into()))
}
pub fn base_text(repo: &Repository, oid: &str, stop: &Cancel) -> Result<String, String> {
    if !crate::git::is_oid(oid) {
        return Err("Invalid blob ID".into());
    }
    let bytes = repo.read(
        &[OsStr::new("cat-file"), OsStr::new("blob"), OsStr::new(oid)],
        stop,
    )?;
    Document::new(bytes)
        .map(|d| d.text().to_owned())
        .map_err(|e| format!("Unsupported marker baseline: {e:?}"))
}

pub fn blocks(original: &str, current: &str) -> Vec<Block> {
    similar::TextDiff::configure()
        .timeout(Duration::from_millis(250))
        .diff_lines(original, current)
        .ops()
        .iter()
        .filter(|op| op.tag() != similar::DiffTag::Equal)
        .map(|op| Block {
            original: op.old_range(),
            current: op.new_range(),
        })
        .collect()
}
pub fn fragment(text: &str, lines: Range<usize>) -> String {
    text.split_inclusive('\n')
        .skip(lines.start)
        .take(lines.len())
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_add_modify_delete_and_eof() {
        for (old, new, kind, a, b) in [
            ("a\n", "a\nb\n", Kind::Added, 1..1, 1..2),
            ("a\nb\n", "a\n", Kind::Deleted, 1..2, 1..1),
            ("a\n旧", "a\n新", Kind::Modified, 1..2, 1..2),
            ("a\n", "", Kind::Deleted, 0..1, 0..0),
        ] {
            let changes = blocks(old, new);
            assert_eq!(changes.len(), 1);
            assert_eq!(
                changes[0],
                Block {
                    original: a,
                    current: b
                }
            );
            assert_eq!(changes[0].kind(), kind);
        }
        assert!(blocks("same\n", "same\n").is_empty());
    }
    #[test]
    fn separates_independent_edits_and_keeps_unicode() {
        let changes = blocks("a\n旧\nz\ntail\n", "a\n新\nz\n");
        assert_eq!(changes.len(), 2);
        assert_eq!(
            fragment("a\n旧\nz\ntail\n", changes[0].original.clone()),
            "旧\n"
        );
    }
}
