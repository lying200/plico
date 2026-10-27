//! Multi-root Log selection. Never zip repository-relative paths across roots.
use super::{CommitId, Entry};
use crate::git::{self, Cancel, Repository, Result};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub struct Changes {
    pub root: Arc<PathBuf>,
    pub files: Vec<crate::combined_changes::Change>,
}
pub struct Selection {
    /// Original displayed selection order, including identical OIDs in different roots.
    pub commits: Vec<Entry>,
    pub changes: Vec<Changes>,
}

/// Repository-local order is preserved; equal OIDs in different roots are distinct.
/// Root order follows first appearance rather than a submodule dependency sort.
#[derive(Debug, PartialEq, Eq)]
pub struct Group {
    pub root: Arc<PathBuf>,
    pub hashes: Vec<String>,
}

pub fn group(ids: &[CommitId], limit: usize) -> Result<Vec<Group>> {
    if ids.len() > limit {
        return Err("Log selection exceeds the commit limit".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut groups: Vec<Group> = Vec::new();
    let mut roots = HashMap::new();
    for id in ids {
        if id.root.as_os_str().is_empty() || !git::is_oid(&id.hash) || !seen.insert(id) {
            return Err("Invalid Log selection".into());
        }
        let index = *roots.entry(id.root.clone()).or_insert_with(|| {
            groups.push(Group {
                root: id.root.clone(),
                hashes: vec![],
            });
            groups.len() - 1
        });
        groups[index].hashes.push(id.hash.clone());
    }
    Ok(groups)
}

/// Load selected commits, not a revision range. Each root retains its own row
/// order for the existing oldest-to-newest change composition algorithm.
/// Any failed root fails the selection: partial changes must not look complete.
pub fn load(ids: &[CommitId], token: &Cancel) -> Result<Selection> {
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    let groups = group(ids, git::HISTORY_MAX)?;
    let positions: HashMap<_, _> = ids
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();
    let mut commits = Vec::new();
    let mut changes = Vec::new();
    let mut file_count = 0usize;
    for Group { root, hashes } in groups {
        let repo = Repository::new(root.as_ref());
        let snapshot = crate::log_changes::load(&repo, &hashes, token)
            .map_err(|error| format!("{}: {error}", root.display()))?;
        file_count = file_count.saturating_add(snapshot.files.len());
        if file_count > crate::combined_changes::MAX_INPUT_CHANGES {
            return Err("Log preview exceeds the change limit".into());
        }
        commits.extend(snapshot.commits.into_iter().map(|commit| Entry {
            root: root.clone(),
            commit,
            display_parents: None,
        }));
        changes.push(Changes {
            root,
            files: snapshot.files,
        });
    }
    commits.sort_by_key(|entry| positions[&entry.identity()]);
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(Selection { commits, changes })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(root: &str, digit: char) -> CommitId {
        CommitId {
            root: Arc::new(PathBuf::from(root)),
            hash: digit.to_string().repeat(40),
        }
    }
    #[test]
    fn grouping_preserves_per_root_order_and_identical_hashes_across_roots() {
        let ids = vec![
            id("parent", '3'),
            id("parent/child", '3'),
            id("other", '2'),
            id("parent", '1'),
            id("parent/child", '1'),
        ];
        let groups = group(&ids, 1000).unwrap();
        assert_eq!(
            groups
                .iter()
                .map(|g| g.root.as_ref().as_path())
                .collect::<Vec<_>>(),
            vec![
                std::path::Path::new("parent"),
                std::path::Path::new("parent/child"),
                std::path::Path::new("other")
            ]
        );
        assert_eq!(
            groups[0].hashes,
            vec![ids[0].hash.clone(), ids[3].hash.clone()]
        );
        assert_eq!(
            groups[1].hashes,
            vec![ids[1].hash.clone(), ids[4].hash.clone()]
        );
        assert_eq!(groups[2].hashes, vec![ids[2].hash.clone()]);
    }
    #[test]
    fn rejects_duplicate_identity_invalid_targets_and_over_limit_without_partial_groups() {
        let first = id("parent", '1');
        assert!(group(&[first.clone(), first.clone()], 1000).is_err());
        assert!(group(&[first.clone(), id("", '2')], 1000).is_err());
        assert!(group(&[first.clone(), id("child", 'z')], 1000).is_err());
        assert!(group(&[first, id("child", '2')], 1).is_err());
        assert!(group(&[], 1000).unwrap().is_empty());
    }
}
