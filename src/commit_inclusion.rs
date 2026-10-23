//! Inclusion for the Rebased-style non-staging Commit tree. Before/after paths
//! drive staging; excluded index paths are preserved by commit_operation.
use crate::{
    commit_changes::{File, Snapshot},
    commit_operation::Request,
};
use std::{
    collections::{BTreeSet, HashSet},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Tracked,
    Unversioned,
    Conflict,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identity {
    pub kind: Kind,
    pub path: PathBuf,
    pub before: Option<PathBuf>,
    pub after: Option<PathBuf>,
}
impl Identity {
    pub fn of(file: &File) -> Self {
        match file {
            File::Tracked(c) => Self {
                kind: Kind::Tracked,
                path: c.path.clone(),
                before: if c.status.starts_with('A') {
                    None
                } else {
                    Some(c.old_path.clone().unwrap_or_else(|| c.path.clone()))
                },
                after: if c.status.starts_with('D') {
                    None
                } else {
                    Some(c.path.clone())
                },
            },
            File::Unversioned(c) => Self {
                kind: Kind::Unversioned,
                path: c.path.clone(),
                before: None,
                after: Some(c.path.clone()),
            },
            File::Conflict(c) => Self {
                kind: Kind::Conflict,
                path: c.path.clone(),
                before: None,
                after: None,
            },
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub added: usize,
    pub unversioned: usize,
    pub modified: usize,
    pub deleted: usize,
}
#[derive(Default)]
pub struct Inclusion {
    available: HashSet<Identity>,
    included: HashSet<Identity>,
    head: Option<String>,
}
impl Inclusion {
    pub fn replace(&mut self, snapshot: &Snapshot) {
        let head_changed = self.head != snapshot.head;
        self.head = snapshot.head.clone();
        self.available = snapshot.files.iter().map(Identity::of).collect();
        if head_changed {
            self.included.clear();
        } else {
            self.included
                .retain(|id| self.available.contains(id) && id.kind != Kind::Conflict);
        }
    }
    pub fn clear(&mut self) {
        self.included.clear();
    }
    pub fn count(&self) -> usize {
        self.included.len()
    }
    /// Counts included file identities, so a partial hunk selection is one file.
    pub fn summary(&self) -> Summary {
        let mut summary = Summary::default();
        for id in &self.included {
            match (id.kind, id.before.is_some(), id.after.is_some()) {
                (Kind::Unversioned, _, _) => summary.unversioned += 1,
                (Kind::Tracked, false, true) => summary.added += 1,
                (Kind::Tracked, true, false) => summary.deleted += 1,
                (Kind::Tracked, true, true) => summary.modified += 1,
                _ => {}
            }
        }
        summary
    }
    pub fn included(&self, id: &Identity) -> bool {
        self.included.contains(id)
    }
    pub fn set(&mut self, id: &Identity, checked: bool) {
        if id.kind == Kind::Conflict || !self.available.contains(id) {
            return;
        }
        if checked {
            self.included.insert(id.clone());
        } else {
            self.included.remove(id);
        }
    }
    pub fn set_scope(&mut self, ids: &HashSet<Identity>, checked: bool) {
        for id in ids {
            self.set(id, checked);
        }
    }
    pub fn scope_counts(&self, ids: &HashSet<Identity>) -> (usize, usize) {
        ids.iter()
            .filter(|id| id.kind != Kind::Conflict && self.available.contains(*id))
            .fold((0, 0), |(included, total), id| {
                (
                    included + usize::from(self.included.contains(id)),
                    total + 1,
                )
            })
    }
    pub fn request(&self, message: String, amend: bool) -> Result<Request, String> {
        if self.available.iter().any(|id| id.kind == Kind::Conflict) {
            return Err("Resolve conflicts before committing".into());
        }
        let mut add = BTreeSet::new();
        let mut remove = BTreeSet::new();
        for id in &self.included {
            add.extend(id.after.iter().cloned());
            remove.extend(id.before.iter().cloned());
        }
        // Same rule as GitCheckinEnvironment.stageAndCommit: added paths win.
        remove.retain(|path| !add.contains(path));
        if add.is_empty() && remove.is_empty() && !amend {
            return Err("Select changes to commit".into());
        }
        Ok(Request {
            add: add.into_iter().collect(),
            remove: remove.into_iter().collect(),
            message,
            amend,
            expected_head: self.head.clone(),
        })
    }
}

#[cfg(test)]
mod summary_tests {
    use super::*;
    #[test]
    fn summary_counts_included_identities_and_moves_only_once() {
        let identity = |kind, before: Option<&str>, after: Option<&str>| Identity {
            kind,
            path: after.or(before).unwrap().into(),
            before: before.map(Into::into),
            after: after.map(Into::into),
        };
        let ids = [
            identity(Kind::Tracked, None, Some("added")),
            identity(Kind::Unversioned, None, Some("new")),
            identity(Kind::Tracked, Some("old"), Some("renamed")),
            identity(Kind::Tracked, Some("deleted"), None),
            identity(Kind::Tracked, Some("excluded"), Some("excluded")),
        ];
        let mut inclusion = Inclusion {
            available: ids.iter().cloned().collect(),
            ..Default::default()
        };
        for id in &ids[..4] {
            inclusion.set(id, true);
        }
        inclusion.set(&ids[2], true);
        assert_eq!(
            inclusion.summary(),
            Summary {
                added: 1,
                unversioned: 1,
                modified: 1,
                deleted: 1
            }
        );
        inclusion.set(&ids[2], false);
        assert_eq!(inclusion.summary().modified, 0);
        inclusion.clear();
        assert_eq!(inclusion.summary(), Summary::default());
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::git::{Area, Change};
    fn file(path: &str, status: &str, area: Area, old: Option<&str>) -> File {
        File::from_change(&Change {
            path: path.into(),
            old_path: old.map(Into::into),
            status: status.into(),
            area,
        })
        .unwrap()
    }
    fn snapshot(files: Vec<File>) -> Snapshot {
        Snapshot {
            head: Some("a".repeat(40)),
            files,
            review_versions: vec![],
        }
    }
    #[test]
    fn group_changes_leave_outside_inclusion_and_conflicts_untouched() {
        let snapshot = snapshot(vec![
            file("src/a", "M", Area::WorkingTree, None),
            file("src/b", "M", Area::WorkingTree, None),
            file("src/conflict", "UU", Area::Conflict, None),
            file("outside", "?", Area::Untracked, None),
        ]);
        let ids: Vec<_> = snapshot.files.iter().map(Identity::of).collect();
        let scope: HashSet<_> = ids[..3].iter().cloned().collect();
        let mut inclusion = Inclusion::default();
        inclusion.replace(&snapshot);
        inclusion.set(&ids[3], true);
        inclusion.set(&ids[0], true);
        assert_eq!(inclusion.scope_counts(&scope), (1, 2));
        inclusion.set_scope(&scope, true);
        assert_eq!(inclusion.scope_counts(&scope), (2, 2));
        assert_eq!(inclusion.count(), 3);
        inclusion.set_scope(&scope, false);
        assert_eq!(inclusion.scope_counts(&scope), (0, 2));
        assert!(inclusion.included(&ids[3]));
        assert!(!inclusion.included(&ids[2]));
        assert!(inclusion.request("message".into(), true).is_err());
    }
    #[test]
    fn refresh_does_not_inherit_inclusion_for_new_files_or_rename_sources() {
        let first = snapshot(vec![file("new", "R", Area::WorkingTree, Some("old"))]);
        let id = Identity::of(&first.files[0]);
        let mut inclusion = Inclusion::default();
        inclusion.replace(&first);
        inclusion.set(&id, true);
        let mut next = first.clone();
        next.files
            .push(file("untracked", "?", Area::Untracked, None));
        inclusion.replace(&next);
        assert!(inclusion.included(&id));
        assert!(!inclusion.included(&Identity::of(&next.files[1])));
        next.files[0] = file("new", "R", Area::WorkingTree, Some("different"));
        inclusion.replace(&next);
        assert_eq!(inclusion.count(), 0);
        inclusion.set(&Identity::of(&next.files[0]), true);
        inclusion.replace(&snapshot(vec![]));
        inclusion.replace(&next);
        assert_eq!(inclusion.count(), 0);
    }
    #[test]
    fn rename_and_deletion_remove_only_original_paths() {
        let snapshot = snapshot(vec![
            file("renamed", "R", Area::WorkingTree, Some("old")),
            file("deleted", "D", Area::WorkingTree, None),
        ]);
        let mut inclusion = Inclusion::default();
        inclusion.replace(&snapshot);
        for file in &snapshot.files {
            inclusion.set(&Identity::of(file), true);
        }
        let request = inclusion.request("chosen files".into(), false).unwrap();
        assert_eq!(request.add, vec![PathBuf::from("renamed")]);
        assert_eq!(request.remove, vec![PathBuf::from("deleted"), "old".into()]);
    }
}
