//! On-demand working-tree nodes and selection rules for VcsStructureChooser.
use crate::git::{Cancel, Repository, Result};
use std::{
    collections::BTreeSet,
    io::{self, BufRead, BufReader, Read},
    path::Path,
};
const NODE_LIMIT: usize = 4096;
const TEXT_LIMIT: usize = 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub path: String,
    pub directory: bool,
}

// ls-files can stream an arbitrarily deep subtree. Retain only direct file names.
pub(crate) fn index_children(input: impl Read, prefix: &str) -> io::Result<BTreeSet<String>> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    let mut input = BufReader::new(input);
    let mut field = Vec::new();
    let mut files = BTreeSet::new();
    let mut bytes = 0;
    loop {
        field.clear();
        let size = (&mut input).take(32769).read_until(0, &mut field)?;
        if size == 0 {
            break;
        }
        if size > 32768 || field.pop() != Some(0) {
            return Err(invalid("Invalid or oversized index path"));
        }
        let path = std::str::from_utf8(&field).map_err(|_| invalid("Index path is not UTF-8"))?;
        let Some(name) = path.strip_prefix(prefix) else {
            continue;
        };
        if name.is_empty() || name.contains('/') {
            continue;
        }
        if !files.contains(name) {
            bytes += name.len();
            if files.len() == NODE_LIMIT || bytes > TEXT_LIMIT {
                return Err(invalid("Directory exceeds the path chooser budget"));
            }
            files.insert(name.to_owned());
        }
    }
    Ok(files)
}

fn linked(metadata: &std::fs::Metadata) -> bool {
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

pub fn children(repo: &Repository, directory: &str, token: &Cancel) -> Result<Vec<Node>> {
    crate::git::LogFilter {
        paths: vec![directory.into()],
        ..Default::default()
    }
    .validate_paths()?;
    let mut full_path = repo.path.clone();
    if directory != "." {
        for part in Path::new(directory).components() {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            full_path.push(part);
            let metadata = std::fs::symlink_metadata(&full_path).map_err(|e| e.to_string())?;
            if linked(&metadata) {
                return Err("Linked directories cannot be expanded in the path chooser".into());
            }
        }
    }
    let prefix = if directory == "." {
        String::new()
    } else {
        format!("{directory}/")
    };
    let tracked = repo.chooser_index_children(&prefix, token)?;
    let entries = std::fs::read_dir(&full_path).map_err(|e| e.to_string())?;
    let mut nodes = Vec::new();
    let mut bytes = 0;
    for (scanned, entry) in entries.enumerate() {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if scanned == 8192 {
            return Err("Directory exceeds the 8192-entry scan budget".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Directory entry is not UTF-8")?;
        if name == ".git" {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        let directory = metadata.is_dir() && !linked(&metadata);
        if !directory && !tracked.contains(&name) {
            continue;
        }
        let path = format!("{prefix}{name}");
        bytes += path.len();
        if nodes.len() == NODE_LIMIT || bytes > TEXT_LIMIT {
            return Err("Directory exceeds the 4096-node / 512 KiB path budget".into());
        }
        nodes.push(Node { path, directory });
    }
    nodes.sort_by_cached_key(|node| (!node.directory, node.path.to_lowercase(), node.path.clone()));
    Ok(nodes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Clear,
    Selected,
    CoveredByAncestor,
    ContainsSelected,
}
#[derive(Default)]
pub struct Selection {
    paths: BTreeSet<String>,
}
fn ancestor(parent: &str, child: &str) -> bool {
    parent == "." && child != "."
        || child
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}
impl Selection {
    pub fn state(&self, path: &str) -> State {
        if self.paths.contains(path) {
            State::Selected
        } else if self.paths.iter().any(|p| ancestor(p, path)) {
            State::CoveredByAncestor
        } else if self.paths.iter().any(|p| ancestor(path, p)) {
            State::ContainsSelected
        } else {
            State::Clear
        }
    }
    pub fn toggle(&mut self, path: &str) -> Result<()> {
        match self.state(path) {
            State::CoveredByAncestor => return Ok(()),
            State::Selected => {
                self.paths.remove(path);
                return Ok(());
            }
            State::Clear if self.paths.len() >= 100 => {
                return Err("Select at most 100 files or folders".into());
            }
            _ => {}
        }
        self.paths.retain(|p| !ancestor(path, p));
        self.paths.insert(path.into());
        Ok(())
    }
    pub fn paths(&self) -> Vec<String> {
        self.paths.iter().cloned().collect()
    }
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selecting_parent_replaces_children_and_disables_covered_descendants() {
        let mut selection = Selection::default();
        selection.toggle("src/a").unwrap();
        selection.toggle("src/b").unwrap();
        assert_eq!(selection.state("src"), State::ContainsSelected);
        assert_eq!(selection.state("src-old"), State::Clear);
        selection.toggle("src").unwrap();
        assert_eq!(selection.paths(), vec!["src"]);
        assert_eq!(selection.state("src/a"), State::CoveredByAncestor);
        selection.toggle("src/a").unwrap();
        assert_eq!(selection.paths(), vec!["src"]);
        selection.toggle("src").unwrap();
        assert!(selection.is_empty());
        for i in 0..100 {
            selection.toggle(&format!("src/{i}")).unwrap();
        }
        assert!(selection.toggle("extra").is_err());
        selection.toggle("src").unwrap();
        assert_eq!(selection.paths(), vec!["src"]);
        selection.toggle(".").unwrap();
        assert_eq!(selection.paths(), vec!["."]);
    }
    #[test]
    fn index_stream_retains_direct_names_without_collecting_subtrees() {
        let input = b"src/a\0src/deep/b\0src/a\0src-old/c\0src/.hidden\0";
        assert_eq!(
            index_children(&input[..], "src/")
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            vec![".hidden", "a"]
        );
        assert!(index_children(&b"unterminated"[..], "").is_err());
        assert!(index_children(&b"\xff\0"[..], "").is_err());
        let huge = (0..4097).map(|i| format!("{i}\0")).collect::<String>();
        assert!(index_children(huge.as_bytes(), "").is_err());
    }
}
