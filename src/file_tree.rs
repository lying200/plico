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
            Pending::Directory(path, depth) => {
                let Some(node) = nodes.get(&path) else {
                    continue;
                };
                let root = path.as_os_str().is_empty();
                if !root {
                    let expanded = !collapsed(&path);
                    output.push(Row::Folder {
                        path: path.clone(),
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
