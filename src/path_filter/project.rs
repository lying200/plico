//! Resolve structure filters before issuing per-repository history queries.
//! Mirrors VcsLogUtil.collectRoots/getFilteredFilesForRoot (see reference record).
use crate::git::{LogFilter, Result};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Missing entry means invisible; an empty path list means the entire root.
/// `paths` are relative to the editor/chooser's base, not to every repository.
/// Resolve filesystem directory state once, on the history worker.
pub fn map(
    roots: &[PathBuf],
    base: &Path,
    paths: &[String],
) -> Result<BTreeMap<PathBuf, Vec<String>>> {
    LogFilter {
        paths: paths.to_vec(),
        ..Default::default()
    }
    .validate_paths()?;
    if paths.is_empty() {
        return Ok(roots.iter().cloned().map(|root| (root, vec![])).collect());
    }
    let selected = paths
        .iter()
        .map(|path| {
            let absolute = if path == "." {
                base.to_path_buf()
            } else {
                base.join(path)
            };
            let directory = absolute.is_dir();
            (absolute, directory)
        })
        .collect::<Vec<_>>();
    resolve(roots, &selected)
}

fn relative<'a>(root: &str, path: &'a str) -> Option<&'a str> {
    let prefix = path.get(..root.len())?;
    let equal = if cfg!(windows) {
        prefix.eq_ignore_ascii_case(root)
    } else {
        prefix == root
    };
    if !equal {
        return None;
    }
    let rest = &path[root.len()..];
    if rest.is_empty() {
        Some("")
    } else {
        rest.strip_prefix('/')
    }
}
fn text(path: &Path) -> Result<String> {
    let value = path.to_str().ok_or("Filter path is not UTF-8")?;
    let normalized = if cfg!(windows) {
        value.replace('\\', "/")
    } else {
        value.to_owned()
    };
    Ok(normalized.trim_end_matches('/').to_owned())
}
fn resolve(
    roots: &[PathBuf],
    selected: &[(PathBuf, bool)],
) -> Result<BTreeMap<PathBuf, Vec<String>>> {
    let names = roots
        .iter()
        .map(|root| text(root))
        .collect::<Result<Vec<_>>>()?;
    let selected = selected
        .iter()
        .map(|(path, dir)| Ok((text(path)?, *dir)))
        .collect::<Result<Vec<_>>>()?;
    let mut visible = vec![false; roots.len()];
    for (path, directory) in &selected {
        // The deepest containing root owns a path, including a deleted file.
        if let Some((index, _)) = names
            .iter()
            .enumerate()
            .filter(|(_, root)| relative(root, path).is_some())
            .max_by_key(|(_, root)| root.len())
        {
            visible[index] = true;
        }
        if *directory {
            for (index, root) in names.iter().enumerate() {
                visible[index] |= relative(path, root).is_some();
            }
        }
    }
    let mut result = BTreeMap::new();
    for (index, root) in names.iter().enumerate() {
        if !visible[index] {
            continue;
        }
        let whole = selected.iter().any(|(path, directory)| {
            path == root || (*directory && relative(path, root).is_some())
        });
        let mut paths = if whole {
            vec![]
        } else {
            selected
                .iter()
                .filter_map(|(path, _)| relative(root, path))
                .filter(|path| !path.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        paths.sort();
        paths.dedup();
        LogFilter {
            paths: paths.clone(),
            ..Default::default()
        }
        .validate_paths()?;
        result.insert(roots[index].clone(), paths);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_draft_roundtrips_sibling_paths_and_preserves_deleted_names() {
        let base = std::env::temp_dir().join("plico-project-path-draft");
        let roots = [base.join("main"), base.join("other")];
        let selected = parse("other/deleted.md\nmain/readme.md", &roots).unwrap();
        assert_eq!(
            selected,
            [roots[1].join("deleted.md"), roots[0].join("readme.md")]
        );
        assert_eq!(display(&selected[0], &roots), "other/deleted.md");
        assert!(parse("../outside", &roots).is_err());
        assert!(parse("unregistered/file", &roots).is_err());
        // Changing membership changes presentation, never the selected target.
        assert_eq!(display(&selected[0], &roots[1..]), "deleted.md");
    }
    #[test]
    fn nested_roots_and_same_named_files_have_distinct_scopes() {
        let roots = ["/project", "/project/child", "/other"].map(PathBuf::from);
        let selected = [(PathBuf::from("/project/child/readme.md"), false)];
        let actual = resolve(&roots, &selected).unwrap();
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[&roots[1]], ["readme.md"]);
        let selected = [
            (PathBuf::from("/project/readme.md"), false),
            selected[0].clone(),
        ];
        let actual = resolve(&roots, &selected).unwrap();
        assert_eq!(actual.len(), 2);
        assert_eq!(actual[&roots[0]], ["child/readme.md", "readme.md"]);
        assert_eq!(actual[&roots[1]], ["readme.md"]);
    }
    #[test]
    fn whole_directory_selection_includes_nested_roots_but_not_siblings() {
        let roots = ["/project", "/project/child", "/project-other"].map(PathBuf::from);
        let actual = resolve(&roots, &[(roots[0].clone(), true)]).unwrap();
        assert_eq!(actual.len(), 2);
        assert!(actual.values().all(Vec::is_empty));
        let actual = resolve(&roots, &[(roots[1].clone(), true)]).unwrap();
        assert_eq!(actual.len(), 1);
        assert!(actual[&roots[1]].is_empty());
        assert!(
            resolve(&roots, &[(PathBuf::from("/outside/deleted"), false)])
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn empty_filter_is_all_roots_and_invalid_paths_fail() {
        let roots = [PathBuf::from("/project")];
        assert_eq!(map(&roots, &roots[0], &[]).unwrap().len(), 1);
        assert!(map(&roots, &roots[0], &["../outside".into()]).is_err());
    }
}

/// Stable absolute selections survive changes to the project root list.
pub fn map_absolute(
    roots: &[PathBuf],
    paths: &[PathBuf],
) -> Result<BTreeMap<PathBuf, Vec<String>>> {
    if paths.is_empty() {
        return Ok(roots.iter().cloned().map(|root| (root, vec![])).collect());
    }
    let selected = paths
        .iter()
        .map(|path| (path.clone(), path.is_dir()))
        .collect::<Vec<_>>();
    resolve(roots, &selected)
}

pub fn common_ancestor(roots: &[PathBuf]) -> Option<PathBuf> {
    let mut common = roots.first()?.clone();
    for root in roots.iter().skip(1) {
        while !root.starts_with(&common) {
            if !common.pop() {
                return None;
            }
        }
    }
    (!common.as_os_str().is_empty()).then_some(common)
}

/// Text drafts use the common project ancestor; unrelated volumes need absolute paths.
pub fn parse(draft: &str, roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if draft.len() > 64 * 1024 {
        return Err("Paths draft exceeds 64 KiB".into());
    }
    let base = common_ancestor(roots);
    let mut selected = Vec::new();
    for line in draft.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let normalized = line.replace('\\', "/");
        let path = Path::new(&normalized);
        if path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
            || normalized.contains('\0')
        {
            return Err("Select paths inside the project repositories".into());
        }
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.as_ref()
                .ok_or("Use absolute paths for repositories on different volumes")?
                .join(path)
        };
        // Do not canonicalize: a historical path may no longer exist on disk.
        let path = path.components().collect::<PathBuf>();
        let value = text(&path)?;
        let related = roots.iter().try_fold(false, |related, root| {
            let root = text(root)?;
            Ok::<_, String>(
                related || relative(&root, &value).is_some() || relative(&value, &root).is_some(),
            )
        })?;
        if !related {
            return Err("Select paths inside the project repositories".into());
        }
        if !selected.contains(&path) {
            selected.push(path);
        }
        if selected.len() > 128 {
            return Err("Select at most 128 paths".into());
        }
    }
    Ok(selected)
}

pub fn display(path: &Path, roots: &[PathBuf]) -> String {
    common_ancestor(roots)
        .and_then(|base| path.strip_prefix(base).ok().map(Path::to_path_buf))
        .map(|relative| {
            if relative.as_os_str().is_empty() {
                ".".to_owned()
            } else {
                relative.to_string_lossy().replace('\\', "/")
            }
        })
        .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"))
}
