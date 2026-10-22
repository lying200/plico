//! Visible rows for a ref-name tree. Group keys are full prefixes, so identically
//! named local and remote folders have independent expansion state.
use crate::git::{Branch, Tag};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub key: String,
    pub label: String,
    pub depth: usize,
    pub branch: Option<String>,
    pub expanded: bool,
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, Node>,
    branch: Option<String>,
}

pub fn visible_rows(branches: &[Branch], collapsed: &HashSet<String>) -> Vec<Row> {
    filtered_rows(branches, collapsed, "")
}

/// Search full display paths without changing saved expansion state. Matching
/// descendants remain visible even when their folders were previously closed.
pub fn filtered_rows(branches: &[Branch], collapsed: &HashSet<String>, query: &str) -> Vec<Row> {
    ref_rows(
        branches.iter().map(|r| r.name.as_str()),
        collapsed,
        query,
        None,
    )
}

pub fn filtered_ref_rows(
    branches: &[Branch],
    tags: &[Tag],
    collapsed: &HashSet<String>,
    query: &str,
    current: Option<&str>,
) -> Vec<Row> {
    ref_rows(
        branches
            .iter()
            .map(|r| r.name.as_str())
            .chain(tags.iter().map(|r| r.name.as_str())),
        collapsed,
        query,
        current,
    )
}

fn ref_rows<'a>(
    refs: impl Iterator<Item = &'a str> + Clone,
    collapsed: &HashSet<String>,
    query: &str,
    current: Option<&str>,
) -> Vec<Row> {
    let query = query.trim().to_lowercase();
    let expanded = HashSet::new();
    let collapsed = if query.is_empty() {
        collapsed
    } else {
        &expanded
    };
    let mut rows = Vec::new();
    for (prefix, label) in [
        ("refs/heads", "Local"),
        ("refs/remotes", "Remote"),
        ("refs/tags", "Tags"),
    ] {
        let mut root = Node::default();
        for name in refs.clone() {
            let Some(path) = name.strip_prefix(&format!("{prefix}/")) else {
                continue;
            };
            if !path.to_lowercase().contains(&query) {
                continue;
            }
            let mut node = &mut root;
            for part in path.split('/') {
                node = node.children.entry(part.to_owned()).or_default();
            }
            node.branch = Some(name.to_owned());
        }
        if !root.children.is_empty() {
            flatten(&root, prefix, label, 0, collapsed, current, &mut rows);
        }
    }
    rows
}

fn flatten(
    node: &Node,
    key: &str,
    label: &str,
    depth: usize,
    collapsed: &HashSet<String>,
    current: Option<&str>,
    rows: &mut Vec<Row>,
) {
    let expanded = !collapsed.contains(key);
    rows.push(Row {
        key: key.into(),
        label: label.into(),
        depth,
        branch: node.branch.clone(),
        expanded,
    });
    if expanded {
        let mut children: Vec<_> = node.children.iter().collect();
        // Rebased: current ref, then folders, then other refs; natural names within each group.
        let weight = |node: &Node| {
            if node
                .branch
                .as_deref()
                .is_some_and(|name| Some(name) == current)
            {
                0
            } else if node.branch.is_none() {
                3
            } else {
                4
            }
        };
        children.sort_by(|(a, left), (b, right)| {
            weight(left)
                .cmp(&weight(right))
                .then_with(|| crate::natural_order::compare(a, b))
        });
        for (name, child) in children {
            flatten(
                child,
                &format!("{key}/{name}"),
                name,
                depth + 1,
                collapsed,
                current,
                rows,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_reveals_matching_paths_and_preserves_collapsed_state() {
        let branches = [
            "refs/heads/feature/review",
            "refs/heads/main",
            "refs/remotes/origin/feature/review",
        ]
        .map(|name| Branch {
            name: name.into(),
            id: String::new(),
        });
        let collapsed = HashSet::from(["refs/heads/feature".into(), "refs/remotes".into()]);
        let rows = filtered_rows(&branches, &collapsed, " FEATURE/REVIEW ");
        assert_eq!(
            rows.iter()
                .filter_map(|r| r.branch.as_deref())
                .collect::<Vec<_>>(),
            [
                "refs/heads/feature/review",
                "refs/remotes/origin/feature/review"
            ]
        );
        assert!(filtered_rows(&branches, &collapsed, "missing").is_empty());
        assert_eq!(
            filtered_rows(&branches, &collapsed, "")
                .iter()
                .filter_map(|r| r.branch.as_deref())
                .collect::<Vec<_>>(),
            ["refs/heads/main"]
        );
        assert_eq!(
            filtered_rows(&branches, &collapsed, "origin/")
                .iter()
                .filter(|r| r.branch.is_some())
                .count(),
            1
        );
    }
    #[test]
    fn collapse_does_not_confuse_local_remote_or_prefix_siblings() {
        let branches = [
            "refs/heads/feature/a",
            "refs/heads/feature/b",
            "refs/heads/features/c",
            "refs/remotes/origin/feature/a",
        ]
        .map(|name| Branch {
            name: name.into(),
            id: String::new(),
        });
        let collapsed = HashSet::from(["refs/heads/feature".into()]);
        let rows = visible_rows(&branches, &collapsed);
        let leaves = rows
            .iter()
            .filter_map(|row| row.branch.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(
            leaves,
            ["refs/heads/features/c", "refs/remotes/origin/feature/a"]
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.key == "refs/heads/feature")
                .unwrap()
                .expanded,
            false
        );
        let remote = rows
            .iter()
            .find(|r| r.branch.as_deref() == Some("refs/remotes/origin/feature/a"))
            .unwrap();
        assert_eq!(remote.depth, 3);
        assert_eq!(
            visible_rows(&branches, &HashSet::new())
                .iter()
                .filter(|r| r.branch.is_some())
                .count(),
            4
        );
    }
}

#[cfg(test)]
mod ref_tree_tests {
    use super::*;
    #[test]
    fn tags_and_current_branch_sort_without_empty_groups() {
        let branches = [
            "refs/heads/z-current",
            "refs/heads/v10",
            "refs/heads/v2",
            "refs/heads/release/work",
        ]
        .map(|name| Branch {
            name: name.into(),
            id: String::new(),
        });
        let tags = ["refs/tags/release/v10", "refs/tags/release/v2"].map(|name| Tag {
            name: name.into(),
            id: String::new(),
        });
        let collapsed = HashSet::from(["refs/heads/release".into()]);
        let rows = filtered_ref_rows(
            &branches,
            &tags,
            &collapsed,
            "",
            Some("refs/heads/z-current"),
        );
        assert!(!rows.iter().any(|r| r.key == "refs/remotes"));
        assert_eq!(
            rows.iter()
                .filter_map(|r| r.branch.as_deref())
                .collect::<Vec<_>>(),
            [
                "refs/heads/z-current",
                "refs/heads/v2",
                "refs/heads/v10",
                "refs/tags/release/v2",
                "refs/tags/release/v10"
            ]
        );
        let searched = filtered_ref_rows(&branches, &tags, &collapsed, "release/v10", None);
        assert_eq!(
            searched
                .iter()
                .filter_map(|r| r.branch.as_deref())
                .collect::<Vec<_>>(),
            ["refs/tags/release/v10"]
        );
        assert!(filtered_ref_rows(&[], &[], &HashSet::new(), "", None).is_empty());
    }
}
