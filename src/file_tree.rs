//! Visible file rows retain original Git change indices; directories never read disk.
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    ffi::OsString,
    hash::Hash,
    path::{Path, PathBuf},
};

/// Keep expansion choices only for directories in the completed file listing.
/// Callers must not pass the temporary empty listing used while loading.
pub fn retain_collapsed<'a, K: Copy + Eq + Hash>(
    collapsed: &mut HashSet<(K, PathBuf)>,
    files: impl IntoIterator<Item = (K, &'a Path)>,
) {
    if collapsed.is_empty() {
        return;
    }
    let mut previous = std::mem::take(collapsed);
    for (area, path) in files {
        for directory in path.ancestors().skip(1) {
            if directory.as_os_str().is_empty() {
                continue;
            }
            if let Some(key) = previous.take(&(area, directory.to_path_buf())) {
                collapsed.insert(key);
            }
        }
    }
    // Dropping the old set also releases capacity from larger past listings.
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Folder {
        path: PathBuf,
        label: PathBuf,
        depth: usize,
        count: usize,
        expanded: bool,
    },
    File {
        index: usize,
        depth: usize,
    },
}
#[derive(Default)]
struct Node {
    directories: BTreeSet<PathBuf>,
    files: Vec<(OsString, usize)>,
    count: usize,
}

pub fn rows<'a>(
    files: impl IntoIterator<Item = (usize, &'a Path)>,
    collapsed: impl Fn(&Path) -> bool,
) -> Vec<Row> {
    build_rows(files, collapsed, false)
}

/// Collapse directory-only chains, retaining the terminal real path as identity.
pub fn compact_rows<'a>(
    files: impl IntoIterator<Item = (usize, &'a Path)>,
    collapsed: impl Fn(&Path) -> bool,
) -> Vec<Row> {
    build_rows(files, collapsed, true)
}

fn build_rows<'a>(
    files: impl IntoIterator<Item = (usize, &'a Path)>,
    collapsed: impl Fn(&Path) -> bool,
    compact: bool,
) -> Vec<Row> {
    let mut nodes = BTreeMap::<PathBuf, Node>::new();
    for (index, path) in files {
        let directory = path.parent().unwrap_or(Path::new(""));
        nodes
            .entry(directory.into())
            .or_default()
            .files
            .push((path.file_name().unwrap_or_default().into(), index));
        let mut current = directory;
        loop {
            nodes.entry(current.into()).or_default().count += 1;
            let Some(parent) = current.parent() else {
                break;
            };
            nodes
                .entry(parent.into())
                .or_default()
                .directories
                .insert(current.into());
            current = parent;
        }
    }
    for node in nodes.values_mut() {
        node.files.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    }
    enum Pending {
        Directory(PathBuf, usize),
        File(usize, usize),
    }
    let mut pending = vec![Pending::Directory(PathBuf::new(), 0)];
    let mut output = Vec::new();
    while let Some(item) = pending.pop() {
        match item {
            Pending::File(index, depth) => output.push(Row::File { index, depth }),
            Pending::Directory(mut path, depth) => {
                let parent = path.parent().unwrap_or(Path::new("")).to_path_buf();
                let Some(mut node) = nodes.get(&path) else {
                    continue;
                };
                let root = path.as_os_str().is_empty();
                if !root {
                    while compact && node.files.is_empty() && node.directories.len() == 1 {
                        path = node.directories.first().unwrap().clone();
                        node = &nodes[&path];
                    }
                    let expanded = !collapsed(&path);
                    output.push(Row::Folder {
                        path: path.clone(),
                        label: path.strip_prefix(&parent).unwrap_or(&path).to_path_buf(),
                        depth,
                        count: node.count,
                        expanded,
                    });
                    if !expanded {
                        continue;
                    }
                }
                let child_depth = depth + usize::from(!root);
                pending.extend(
                    node.files
                        .iter()
                        .rev()
                        .map(|(_, i)| Pending::File(*i, child_depth)),
                );
                pending.extend(
                    node.directories
                        .iter()
                        .rev()
                        .map(|p| Pending::Directory(p.clone(), child_depth)),
                );
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_chains_stop_at_branches_and_files_and_keep_indices() {
        let paths = [
            "spring-core/src/main/java/org/env/Property.java",
            "spring-core/src/test/java/org/env/CompositeTests.java",
            "spring-core/src/test/java/org/env/PropertyTests.java",
            "spring-core/src/test/resources/log.xml",
        ];
        let build = |hide: &Path| {
            compact_rows(
                paths.iter().enumerate().map(|(i, p)| (i, Path::new(p))),
                |p| p == hide,
            )
        };
        let all = build(Path::new("never"));
        let folders: Vec<_> = all
            .iter()
            .filter_map(|r| match r {
                Row::Folder {
                    path,
                    label,
                    depth,
                    count,
                    ..
                } => Some((path.as_path(), label.as_path(), *depth, *count)),
                _ => None,
            })
            .collect();
        assert_eq!(
            folders,
            vec![
                (
                    Path::new("spring-core/src"),
                    Path::new("spring-core/src"),
                    0,
                    4
                ),
                (
                    Path::new("spring-core/src/main/java/org/env"),
                    Path::new("main/java/org/env"),
                    1,
                    1
                ),
                (Path::new("spring-core/src/test"), Path::new("test"), 1, 3),
                (
                    Path::new("spring-core/src/test/java/org/env"),
                    Path::new("java/org/env"),
                    2,
                    2
                ),
                (
                    Path::new("spring-core/src/test/resources"),
                    Path::new("resources"),
                    2,
                    1
                ),
            ]
        );
        assert!(all.contains(&Row::File { index: 0, depth: 2 }));
        assert!(all.contains(&Row::File { index: 2, depth: 3 }));
        let hidden = build(Path::new("spring-core/src/test/java/org/env"));
        assert!(hidden.contains(&Row::File { index: 3, depth: 3 }));
        assert!(
            !hidden
                .iter()
                .any(|r| matches!(r, Row::File { index: 1 | 2, .. }))
        );
        assert_eq!(build(Path::new("spring-core/src")).len(), 1);
        assert_eq!(build(Path::new("never")), all);
    }

    #[test]
    fn compact_projection_keeps_files_and_multiple_roots_separate() {
        let paths = ["a/b/file", "a/b/c/nested", "a-other/b/file", "README"];
        let rows = compact_rows(
            paths.iter().enumerate().map(|(i, p)| (i, Path::new(p))),
            |_| false,
        );
        assert!(
            rows.iter()
                .any(|r| matches!(r,Row::Folder{path,depth:0,..} if path==Path::new("a/b")))
        );
        assert!(
            rows.iter()
                .any(|r| matches!(r,Row::Folder{path,depth:1,..} if path==Path::new("a/b/c")))
        );
        assert!(rows.contains(&Row::File { index: 3, depth: 0 }));
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r, Row::File { .. }))
                .count(),
            4
        );
    }

    #[test]
    fn expansion_state_tracks_current_directories_and_releases_old_capacity() {
        let mut state: HashSet<_> = (0..1000)
            .map(|i| (0_u8, PathBuf::from(format!("old/{i}"))))
            .collect();
        state.extend([
            (0, "src".into()),
            (0, "src/nested".into()),
            (1, "src".into()),
        ]);
        retain_collapsed(&mut state, [(0, Path::new("src/nested/a.rs"))]);
        assert_eq!(
            state,
            HashSet::from([(0, "src".into()), (0, "src/nested".into())])
        );
        assert!(state.capacity() < 16);
        retain_collapsed(&mut state, [(0, Path::new("src-other/a.rs"))]);
        assert!(state.is_empty());
        assert_eq!(state.capacity(), 0);
    }
    #[test]
    fn folder_collapse_keeps_exact_file_identity_and_counts() {
        let paths = [
            "src/cache.rs",
            "src/nested/cache.rs",
            "src-old/cache.rs",
            "README.md",
            "src/中文 [x].rs",
        ];
        let build = |hide| {
            rows(
                paths.iter().enumerate().map(|(i, p)| (i, Path::new(p))),
                |p| hide && p == Path::new("src"),
            )
        };
        let all = build(false);
        assert!(all.contains(&Row::Folder {
            path: "src".into(),
            label: "src".into(),
            depth: 0,
            count: 3,
            expanded: true
        }));
        assert!(all.contains(&Row::File { index: 1, depth: 2 }));
        let visible = build(true);
        let indices: Vec<_> = visible
            .iter()
            .filter_map(|r| {
                if let Row::File { index, .. } = r {
                    Some(*index)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(indices, vec![2, 3]);
        assert_eq!(build(false), all);
    }
    #[test]
    fn separate_inputs_do_not_mix_staged_and_unstaged_paths() {
        let paths = [(7, Path::new("src/a")), (12, Path::new("src/a"))];
        let result = rows(paths, |_| false);
        assert!(result.contains(&Row::File { index: 7, depth: 1 }));
        assert!(result.contains(&Row::File {
            index: 12,
            depth: 1
        }));
        assert!(rows(std::iter::empty(), |_| false).is_empty());
    }
}
