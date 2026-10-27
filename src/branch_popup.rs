//! Top-toolbar branch popup model, separate from Log filtering.
//! Translated from Rebased 4aec352 GitBranchesTreeModel / SingleRepoModel /
//! ModelUtil (JetBrains and contributors, Apache-2.0).
pub mod project;

use crate::git::{Branch, Tag};
use std::collections::HashSet;

/// One repository's explicit overrides. Defaults follow GitBranchManager;
/// exclusion is distinct from never having customized a default favorite.
/// The owner must load/save this value under the repository's identity.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Favorites {
    added: HashSet<String>,
    excluded: HashSet<String>,
}
impl Favorites {
    pub fn predefined(full_ref: &str) -> bool {
        matches!(
            full_ref,
            "refs/heads/main"
                | "refs/heads/master"
                | "refs/remotes/origin/main"
                | "refs/remotes/origin/master"
        )
    }
    pub fn contains(&self, full_ref: &str) -> bool {
        self.added.contains(full_ref)
            || (!self.excluded.contains(full_ref) && Self::predefined(full_ref))
    }
    /// Full refs retain branch/tag/remote identity. UI-only pseudo refs such as
    /// HEAD cannot be favorites and must not become persisted overrides.
    pub fn set(&mut self, full_ref: &str, favorite: bool) -> bool {
        if !["refs/heads/", "refs/remotes/", "refs/tags/"]
            .iter()
            .any(|prefix| {
                full_ref
                    .strip_prefix(prefix)
                    .is_some_and(|name| !name.is_empty())
            })
        {
            return false;
        }
        self.added.remove(full_ref);
        self.excluded.remove(full_ref);
        match (favorite, Self::predefined(full_ref)) {
            (true, false) => {
                self.added.insert(full_ref.into());
            }
            (false, true) => {
                self.excluded.insert(full_ref.into());
            }
            _ => {}
        }
        true
    }
    pub fn toggle(&mut self, full_ref: &str) -> bool {
        self.set(full_ref, !self.contains(full_ref))
    }
    /// Only current refs enter the tree. Overrides for removed refs are kept so
    /// recreating the same branch restores its preference, as upstream does.
    pub fn present(&self, branches: &[Branch], tags: &[Tag]) -> HashSet<String> {
        branches
            .iter()
            .map(|b| &b.name)
            .chain(tags.iter().map(|t| &t.name))
            .filter(|name| self.contains(name))
            .cloned()
            .collect()
    }
    /// Minimal persistence representation: no copies of predefined favorites.
    pub fn overrides(&self) -> impl Iterator<Item = (&str, bool)> {
        self.added
            .iter()
            .map(|r| (r.as_str(), true))
            .chain(self.excluded.iter().map(|r| (r.as_str(), false)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Recent,
    Local,
    Remote,
    Tags,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reference<'a> {
    pub full_name: &'a str,
    pub name: &'a str,
    pub id: &'a str,
    pub current: bool,
    pub favorite: bool,
}
#[derive(Debug)]
pub struct Group<'a> {
    pub section: Section,
    pub references: Vec<Reference<'a>>,
}

/// Builds reference groups only. Actions, directory nodes and fuzzy matching are
/// separate popup concerns. Recent names are supplied in repository recency order.
/// The predicate must be the same matcher for Recent and the other groups.
/// No query, persisted preference, or selection from the Log is modified.
pub fn groups<'a>(
    branches: &'a [Branch],
    tags: &'a [Tag],
    current: Option<&str>,
    favorites: &HashSet<String>,
    recent: &[String],
    directory_grouping: bool,
    matches: Option<&dyn Fn(&str) -> bool>,
) -> Vec<Group<'a>> {
    let make = |full_name: &'a str, id: &'a str, prefix: &str| {
        full_name.strip_prefix(prefix).map(|name| Reference {
            full_name,
            name,
            id,
            current: Some(full_name) == current,
            favorite: favorites.contains(full_name),
        })
    };
    let mut local: Vec<_> = branches
        .iter()
        .filter_map(|b| make(&b.name, &b.id, "refs/heads/"))
        .collect();
    let mut remote: Vec<_> = branches
        .iter()
        .filter_map(|b| make(&b.name, &b.id, "refs/remotes/"))
        .collect();
    let mut tags: Vec<_> = tags
        .iter()
        .filter_map(|t| make(&t.name, &t.id, "refs/tags/"))
        .collect();
    let mut seen = HashSet::new();
    let recent: Vec<_> = recent
        .iter()
        .filter(|name| seen.insert(name.as_str()))
        .filter_map(|name| local.iter().find(|r| r.full_name == name).copied())
        .collect();
    let order = |a: &Reference<'_>, b: &Reference<'_>| {
        (!a.current)
            .cmp(&!b.current)
            .then_with(|| (!a.favorite).cmp(&!b.favorite))
            .then_with(|| {
                (!(directory_grouping && a.name.contains('/')))
                    .cmp(&!(directory_grouping && b.name.contains('/')))
            })
            .then_with(|| crate::natural_order::compare(a.name, b.name))
    };
    local.sort_by(order);
    remote.sort_by(order);
    tags.sort_by(order);
    if matches.is_some() {
        // Upstream exceptRefFilter is evaluated only when a matcher exists.
        local.retain(|r| !recent.iter().any(|recent| recent.full_name == r.full_name));
    }
    [
        (Section::Recent, recent),
        (Section::Local, local),
        (Section::Remote, remote),
        (Section::Tags, tags),
    ]
    .into_iter()
    .filter_map(|(section, mut references)| {
        if let Some(matches) = matches {
            references.retain(|r| matches(r.name));
        }
        (!references.is_empty()).then_some(Group {
            section,
            references,
        })
    })
    .collect()
}

/// Section and full prefix form identity; Recent and Local may contain the same ref.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FolderKey {
    pub section: Section,
    pub prefix: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Node<'a> {
    Folder {
        key: FolderKey,
        label: &'a str,
    },
    Ref {
        reference: Reference<'a>,
        label: &'a str,
    },
}
impl<'a> Group<'a> {
    /// Materialize only the requested level. No recursive tree or per-ref path
    /// copies are retained. Callers own expansion state, independently of Log.
    pub fn children(&self, prefix: &str, directory_grouping: bool) -> Vec<Node<'a>> {
        if !directory_grouping {
            return if prefix.is_empty() {
                self.references
                    .iter()
                    .map(|r| Node::Ref {
                        reference: *r,
                        label: r.name,
                    })
                    .collect()
            } else {
                Vec::new()
            };
        }
        let mut folders = HashSet::new();
        let mut nodes = Vec::new();
        for reference in &self.references {
            let rest = if prefix.is_empty() {
                reference.name
            } else {
                let Some(rest) = reference
                    .name
                    .strip_prefix(prefix)
                    .and_then(|r| r.strip_prefix('/'))
                else {
                    continue;
                };
                rest
            };
            if let Some((label, _)) = rest.split_once('/') {
                if folders.insert(label) {
                    let path = if prefix.is_empty() {
                        label.to_owned()
                    } else {
                        format!("{prefix}/{label}")
                    };
                    nodes.push(Node::Folder {
                        key: FolderKey {
                            section: self.section,
                            prefix: path,
                        },
                        label,
                    });
                }
            } else {
                nodes.push(Node::Ref {
                    reference: *reference,
                    label: rest,
                });
            }
        }
        // SingleRepoModel applies this stable comparator to prefix children,
        // while section-root children retain LinkedHashMap insertion order.
        if !prefix.is_empty() {
            nodes.sort_by_key(|node| match node {
                Node::Ref { reference, .. } if reference.current || reference.favorite => 0,
                Node::Folder { .. } => 1,
                Node::Ref { .. } => 2,
            });
        }
        nodes
    }
}

#[derive(Clone, Default, Debug)]
pub struct Expansion {
    pub expanded: HashSet<FolderKey>,
}
impl Expansion {
    pub fn toggle(&mut self, key: FolderKey) {
        if !self.expanded.remove(&key) {
            self.expanded.insert(key);
        }
    }
    /// Rebased SmartExpander: collapsing removes descendant expansion;
    /// expanding follows a chain with exactly one folder child.
    pub fn toggle_tree(&mut self, key: FolderKey, groups: &[Group<'_>], directory_grouping: bool) {
        if self.expanded.contains(&key) {
            self.expanded.retain(|other| {
                other.section != key.section
                    || !(other.prefix == key.prefix
                        || key.prefix.is_empty()
                        || other
                            .prefix
                            .strip_prefix(&key.prefix)
                            .is_some_and(|rest| rest.starts_with('/')))
            });
            return;
        }
        let Some(group) = groups.iter().find(|group| group.section == key.section) else {
            return;
        };
        let mut next = key;
        loop {
            self.expanded.insert(next.clone());
            let children = group.children(&next.prefix, directory_grouping);
            match children.as_slice() {
                [Node::Folder { key, .. }] => next = key.clone(),
                _ => break,
            }
        }
    }
    /// Make a selected reference visible without expanding unrelated branches.
    pub fn reveal(&mut self, section: Section, name: &str, directory_grouping: bool) {
        self.expanded.insert(FolderKey {
            section,
            prefix: String::new(),
        });
        if directory_grouping {
            for (at, _) in name.match_indices('/') {
                self.expanded.insert(FolderKey {
                    section,
                    prefix: name[..at].into(),
                });
            }
        }
    }
    /// Upstream saves expanded section roots and their immediate children only.
    /// Ignore stale paths and hidden children of collapsed sections.
    pub fn saved_paths(
        &self,
        groups: &[Group<'_>],
        directory_grouping: bool,
    ) -> HashSet<FolderKey> {
        let mut saved = HashSet::new();
        for group in groups {
            let root = FolderKey {
                section: group.section,
                prefix: String::new(),
            };
            if !self.expanded.contains(&root) {
                continue;
            }
            saved.insert(root);
            for node in group.children("", directory_grouping) {
                if let Node::Folder { key, .. } = node {
                    if self.expanded.contains(&key) {
                        saved.insert(key);
                    }
                }
            }
        }
        saved
    }
}

#[derive(Debug)]
pub enum VisibleNode<'a> {
    Section(Section),
    Entry(Node<'a>),
}
#[derive(Debug)]
pub struct VisibleRow<'a> {
    pub section: Section,
    pub depth: usize,
    pub expanded: bool,
    pub node: VisibleNode<'a>,
}

/// Flatten only expanded levels, using an explicit stack rather than recursion
/// (Git ref names can have many slash components). Suitable for a virtual list.
pub fn visible_rows<'a>(
    groups: &[Group<'a>],
    expansion: &Expansion,
    directory_grouping: bool,
) -> Vec<VisibleRow<'a>> {
    let mut result = Vec::new();
    for group in groups {
        let expanded = expansion.expanded.contains(&FolderKey {
            section: group.section,
            prefix: String::new(),
        });
        result.push(VisibleRow {
            section: group.section,
            depth: 0,
            expanded,
            node: VisibleNode::Section(group.section),
        });
        if !expanded {
            continue;
        }
        let mut stack: Vec<_> = group
            .children("", directory_grouping)
            .into_iter()
            .rev()
            .map(|n| (1, n))
            .collect();
        while let Some((depth, node)) = stack.pop() {
            let expanded = if let Node::Folder { key, .. } = &node {
                if expansion.expanded.contains(key) {
                    stack.extend(
                        group
                            .children(&key.prefix, directory_grouping)
                            .into_iter()
                            .rev()
                            .map(|n| (depth + 1, n)),
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            };
            result.push(VisibleRow {
                section: group.section,
                depth,
                expanded,
                node: VisibleNode::Entry(node),
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smart_expansion_follows_single_folder_chains_and_clears_descendants() {
        let branches = [
            "refs/remotes/origin/team/one",
            "refs/remotes/origin/team/two",
            "refs/heads/origin/local",
        ]
        .map(|name| Branch {
            name: name.into(),
            id: "same".into(),
        });
        let groups = groups(&branches, &[], None, &HashSet::new(), &[], true, None);
        let key = FolderKey {
            section: Section::Remote,
            prefix: String::new(),
        };
        let mut expansion = Expansion::default();
        expansion.reveal(Section::Local, "origin/local", true);
        expansion.toggle_tree(key.clone(), &groups, true);
        let rows = visible_rows(&groups, &expansion, true);
        assert!(rows.iter().any(|row| matches!(
            &row.node,
            VisibleNode::Entry(Node::Ref { label: "one", .. })
        ) && row.depth == 3));
        expansion.toggle_tree(key, &groups, true);
        assert!(
            !expansion
                .expanded
                .iter()
                .any(|key| key.section == Section::Remote)
        );
        assert!(
            expansion
                .expanded
                .iter()
                .any(|key| key.section == Section::Local && key.prefix == "origin")
        );
    }
    #[test]
    fn recent_duplicates_only_disappear_when_searching_and_identity_is_full_ref() {
        let branches = [
            "refs/heads/main",
            "refs/heads/topic",
            "refs/remotes/origin/main",
        ]
        .map(|name| Branch {
            name: name.into(),
            id: name.into(),
        });
        let tags = [Tag {
            name: "refs/tags/main".into(),
            id: "tag-commit".into(),
        }];
        let recent = vec![
            "refs/heads/missing".into(),
            "refs/heads/main".into(),
            "refs/heads/main".into(),
        ];
        let favorites = HashSet::new();
        let all = groups(
            &branches,
            &tags,
            Some("refs/heads/main"),
            &favorites,
            &recent,
            true,
            None,
        );
        assert_eq!(
            all.iter().map(|g| g.section).collect::<Vec<_>>(),
            [
                Section::Recent,
                Section::Local,
                Section::Remote,
                Section::Tags
            ]
        );
        assert_eq!(all[0].references.len(), 1);
        assert_eq!(all[1].references[0].full_name, "refs/heads/main");
        let found = groups(
            &branches,
            &tags,
            Some("refs/heads/main"),
            &favorites,
            &recent,
            true,
            Some(&|name| name.contains("main")),
        );
        assert_eq!(
            found.iter().map(|g| g.section).collect::<Vec<_>>(),
            [Section::Recent, Section::Remote, Section::Tags]
        );
        assert!(found[0].references[0].current);
        assert!(!found[2].references[0].current);
        assert_eq!(found[2].references[0].id, "tag-commit");
    }
    #[test]
    fn current_then_favorites_then_directory_refs_and_natural_names() {
        let branches = [
            "refs/heads/v10",
            "refs/heads/feature/a",
            "refs/heads/v2",
            "refs/heads/z",
            "refs/heads/main",
        ]
        .map(|name| Branch {
            name: name.into(),
            id: String::new(),
        });
        let favorites = HashSet::from(["refs/heads/z".into()]);
        let result = groups(
            &branches,
            &[],
            Some("refs/heads/main"),
            &favorites,
            &[],
            true,
            None,
        );
        assert_eq!(
            result[0]
                .references
                .iter()
                .map(|r| r.name)
                .collect::<Vec<_>>(),
            ["main", "z", "feature/a", "v2", "v10"]
        );
        assert!(
            groups(
                &branches,
                &[],
                None,
                &favorites,
                &[],
                true,
                Some(&|_| false)
            )
            .is_empty()
        );
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    #[test]
    fn prefix_levels_preserve_upstream_order_and_do_not_mix_sections() {
        let branches = [
            "refs/heads/feature/current",
            "refs/heads/feature/z",
            "refs/heads/feature/nested/a",
            "refs/heads/feature/b",
            "refs/heads/features/wrong",
        ]
        .map(|name| Branch {
            name: name.into(),
            id: name.into(),
        });
        let recent = vec!["refs/heads/feature/current".into()];
        let favorites = HashSet::from(["refs/heads/feature/z".into()]);
        let groups = groups(
            &branches,
            &[],
            Some("refs/heads/feature/current"),
            &favorites,
            &recent,
            true,
            None,
        );
        let roots = groups[1].children("", true);
        assert!(matches!(
            &roots[0],
            Node::Folder {
                label: "feature",
                ..
            }
        ));
        let children = groups[1].children("feature", true);
        let labels: Vec<_> = children
            .iter()
            .map(|n| match n {
                Node::Folder { label, .. } | Node::Ref { label, .. } => *label,
            })
            .collect();
        assert_eq!(labels, ["current", "z", "nested", "b"]);
        let local_key = match &roots[0] {
            Node::Folder { key, .. } => key,
            _ => panic!(),
        };
        let recent_roots = groups[0].children("", true);
        let recent_key = match &recent_roots[0] {
            Node::Folder { key, .. } => key,
            _ => panic!(),
        };
        assert_ne!(local_key, recent_key);
        assert_eq!(groups[1].children("feature/nested", true).len(), 1);
        assert!(groups[1].children("feat", true).is_empty());
        let flat = groups[1].children("", false);
        assert_eq!(flat.len(), branches.len());
        assert!(flat.iter().all(|n| matches!(n, Node::Ref { .. })));
        assert!(groups[1].children("feature", false).is_empty());
    }
}

#[cfg(test)]
mod expansion_tests {
    use super::*;
    #[test]
    fn reveal_and_persistence_keep_section_identity_and_only_save_shallow_visible_paths() {
        let branches = [
            "refs/heads/feature/nested/current",
            "refs/heads/other/hidden",
        ]
        .map(|name| Branch {
            name: name.into(),
            id: name.into(),
        });
        let recent = vec![branches[0].name.clone()];
        let grouped = groups(&branches, &[], None, &HashSet::new(), &recent, true, None);
        let mut expansion = Expansion::default();
        assert_eq!(visible_rows(&grouped, &expansion, true).len(), 2);
        expansion.reveal(Section::Recent, "feature/nested/current", true);
        let visible = visible_rows(&grouped, &expansion, true);
        assert_eq!(visible.len(), 5);
        assert!(matches!(
            &visible[3].node,
            VisibleNode::Entry(Node::Ref {
                label: "current",
                ..
            })
        ));
        assert_eq!(visible[3].depth, 3);
        let saved = expansion.saved_paths(&grouped, true);
        assert_eq!(saved.len(), 2);
        assert!(!saved.contains(&FolderKey {
            section: Section::Recent,
            prefix: "feature/nested".into()
        }));
        assert!(!saved.iter().any(|key| key.section == Section::Local));
        expansion.toggle(FolderKey {
            section: Section::Recent,
            prefix: String::new(),
        });
        assert!(expansion.saved_paths(&grouped, true).is_empty());
        // Collapsing the parent does not discard its in-session child expansion.
        expansion.toggle(FolderKey {
            section: Section::Recent,
            prefix: String::new(),
        });
        assert_eq!(visible_rows(&grouped, &expansion, true).len(), 5);
        assert!(expansion.saved_paths(&[], true).is_empty());
    }
}

/// Background-only, bounded to upstream's default 50 checkout reflog entries.
/// `saved_recent` is an optional full local ref from application preferences.
/// Read failures remain distinguishable from an empty recent list for the UI.
pub fn recent_branches(
    repo: &crate::git::Repository,
    branches: &[Branch],
    saved_recent: Option<&str>,
    token: &crate::git::Cancel,
) -> Result<Vec<String>, String> {
    let log = repo.text(
        &[
            "reflog",
            "--max-count=50",
            "--grep-reflog=checkout:",
            "--format=%gs",
            "--",
        ],
        token,
    )?;
    let locals: HashSet<_> = branches
        .iter()
        .filter_map(|b| b.name.strip_prefix("refs/heads/"))
        .collect();
    let mut seen = HashSet::new();
    let mut recent = Vec::new();
    for line in log.lines() {
        let Some((_, name)) = line.rsplit_once(" to ") else {
            continue;
        };
        if locals.contains(name) && seen.insert(name) {
            recent.push(format!("refs/heads/{name}"));
            if recent.len() == 5 {
                return Ok(recent);
            }
        }
    }
    if let Some(name) = saved_recent.and_then(|r| r.strip_prefix("refs/heads/")) {
        if locals.contains(name) && seen.insert(name) {
            recent.push(format!("refs/heads/{name}"));
        }
    }
    Ok(recent)
}

#[cfg(test)]
mod favorite_tests {
    use super::*;
    #[test]
    fn defaults_exclusions_and_ref_types_survive_override_roundtrip() {
        let mut prefs = Favorites::default();
        for name in [
            "refs/heads/main",
            "refs/heads/master",
            "refs/remotes/origin/main",
            "refs/remotes/origin/master",
        ] {
            assert!(prefs.contains(name));
        }
        for name in [
            "refs/tags/main",
            "refs/remotes/upstream/main",
            "refs/heads/feature/main",
            "HEAD",
        ] {
            assert!(!prefs.contains(name));
        }
        assert_eq!(prefs.overrides().count(), 0);
        prefs.toggle("refs/heads/main");
        prefs.toggle("refs/tags/main");
        assert!(!prefs.contains("refs/heads/main"));
        assert!(prefs.contains("refs/tags/main"));
        assert!(prefs.contains("refs/remotes/origin/main"));
        let mut restored = Favorites::default();
        for (name, favorite) in prefs.overrides() {
            restored.set(name, favorite);
        }
        assert_eq!(restored, prefs);
        prefs.toggle("refs/heads/main");
        prefs.toggle("refs/tags/main");
        assert_eq!(prefs, Favorites::default());
        for invalid in ["HEAD", "main", "refs/tags/", "refs/notes/review"] {
            assert!(!prefs.set(invalid, true));
        }
        assert_eq!(prefs, Favorites::default());
    }
    #[test]
    fn repository_scopes_and_tree_sorting_keep_favorites_distinct_from_current() {
        let branches =
            ["refs/heads/main", "refs/heads/topic", "refs/heads/current"].map(|name| Branch {
                name: name.into(),
                id: name.into(),
            });
        let mut first = Favorites::default();
        let second = Favorites::default();
        first.set("refs/heads/main", false);
        first.set("refs/heads/topic", true);
        assert!(second.contains("refs/heads/main"));
        assert!(!second.contains("refs/heads/topic"));
        let favorites = first.present(&branches, &[]);
        let groups = groups(
            &branches,
            &[],
            Some("refs/heads/current"),
            &favorites,
            &[],
            true,
            None,
        );
        assert_eq!(
            groups[0]
                .references
                .iter()
                .map(|r| r.name)
                .collect::<Vec<_>>(),
            ["current", "topic", "main"]
        );
        assert!(!groups[0].references[0].favorite);
        assert!(groups[0].references[1].favorite);
        assert!(first.present(&[], &[]).is_empty());
        assert!(first.present(&branches, &[]).contains("refs/heads/topic"));
    }
}
