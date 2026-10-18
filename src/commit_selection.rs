//! Whole-file inclusion, independent of review selection. Based on Rebased
//! ChangesViewCommitWorkflowHandler / PartialCommitInclusionModel (4aec352).
//! Partial-line inclusion and named changelists are not implemented here.
use crate::{commit_operation::Request, git::{Area, Change}};
use std::{collections::{BTreeMap, BTreeSet, HashSet}, path::{Path, PathBuf}};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identity { pub path: PathBuf, pub rename_sources: BTreeSet<PathBuf> }
#[derive(Clone, Debug)]
pub struct Item {
    pub identity: Identity,
    pub conflict: bool,
    pub untracked: bool,
    pub deleted: bool,
}
#[derive(Default)]
pub struct Selection { items: Vec<Item>, included: HashSet<Identity> }
impl Selection {
    pub fn items(&self) -> &[Item] { &self.items }
    pub fn count(&self) -> usize { self.included.len() }
    pub fn included(&self, path: &Path) -> bool {
        self.items.iter().find(|i| i.identity.path == path).is_some_and(|i| self.included.contains(&i.identity))
    }
    /// Only completed refreshes reconcile inclusion. Newly appearing files are
    /// not silently added; explicit include actions determine initial policy.
    pub fn replace(&mut self, changes: &[Change]) {
        let mut grouped = BTreeMap::<PathBuf, Vec<&Change>>::new();
        for change in changes.iter().filter(|c| c.area != Area::Commit) {
            grouped.entry(change.path.clone()).or_default().push(change);
        }
        self.items = grouped.into_iter().map(|(path, changes)| {
            let rename_sources = changes.iter().filter(|c| c.status.starts_with('R'))
                .filter_map(|c| c.old_path.clone()).collect();
            // A recreated untracked file wins over a staged deletion. Otherwise
            // the working-tree status wins over the index-only status.
            let final_change = changes.iter().max_by_key(|c| match c.area {
                Area::Untracked => 3, Area::Unstaged => 2, _ => 1,
            }).unwrap();
            Item { identity: Identity { path, rename_sources }, conflict: changes.iter().any(|c| c.area == Area::Conflict),
                untracked: changes.iter().all(|c| c.area == Area::Untracked), deleted: final_change.status.starts_with('D') }
        }).collect();
        let available: HashSet<_> = self.items.iter().filter(|i| !i.conflict).map(|i| &i.identity).collect();
        self.included.retain(|id| available.contains(id));
    }
    pub fn set(&mut self, path: &Path, included: bool) {
        let Some(item) = self.items.iter().find(|i| i.identity.path == path && !i.conflict) else { return; };
        if included { self.included.insert(item.identity.clone()); } else { self.included.remove(&item.identity); }
    }
    pub fn set_all(&mut self, included: bool) {
        self.included.clear();
        if included { self.included.extend(self.items.iter().filter(|i| !i.conflict).map(|i| i.identity.clone())); }
    }
    /// Aggregate logical files, excluding conflicts and deduplicating review versions.
    pub fn scope_counts(&self, paths: &HashSet<PathBuf>) -> (usize, usize) {
        self.items.iter().filter(|i| !i.conflict && paths.contains(&i.identity.path))
            .fold((0,0), |(included,total), i| (included + usize::from(self.included.contains(&i.identity)), total+1))
    }
    pub fn set_scope(&mut self, paths: &HashSet<PathBuf>, included: bool) {
        for item in self.items.iter().filter(|i| !i.conflict && paths.contains(&i.identity.path)) {
            if included { self.included.insert(item.identity.clone()); } else { self.included.remove(&item.identity); }
        }
    }
    pub fn request(&self, message: String, amend: bool, expected_head: Option<String>) -> Result<Request, String> {
        if self.items.iter().any(|i| i.conflict) { return Err("Resolve conflicts before committing".into()); }
        let mut add = BTreeSet::new(); let mut remove = BTreeSet::new();
        for item in self.items.iter().filter(|i| self.included.contains(&i.identity)) {
            remove.extend(item.identity.rename_sources.iter().cloned());
            if item.deleted { remove.insert(item.identity.path.clone()); }
            else { add.insert(item.identity.path.clone()); }
        }
        // A rename source may also be a selected destination/recreated file.
        // Rebased stageAndCommit removes paths that will be added from toRemove.
        remove.retain(|p| !add.contains(p));
        if add.is_empty() && remove.is_empty() && !amend { return Err("Select changes to commit".into()); }
        Ok(Request { add: add.into_iter().collect(), remove: remove.into_iter().collect(), message, amend, expected_head })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn c(path: &str, status: &str, area: Area) -> Change { Change { path: path.into(), old_path: None, status: status.into(), area } }
    #[test]
    fn group_inclusion_is_unique_preserves_other_groups_and_excludes_conflicts() {
        let mut s=Selection::default();
        s.replace(&[c("src/a","M",Area::Staged),c("src/a","M",Area::Unstaged),
            c("src/b","M",Area::Unstaged),c("src/bad","UU",Area::Conflict),c("other","??",Area::Untracked)]);
        let paths=["src/a","src/b","src/bad"].map(PathBuf::from).into_iter().collect();
        s.set(Path::new("other"),true); s.set(Path::new("src/a"),true);
        assert_eq!(s.scope_counts(&paths),(1,2));
        s.set_scope(&paths,true); assert_eq!(s.scope_counts(&paths),(2,2)); assert_eq!(s.count(),3);
        s.set_scope(&paths,false); assert_eq!(s.scope_counts(&paths),(0,2)); assert!(s.included(Path::new("other")));
        assert!(!s.included(Path::new("src/bad")));
    }
    #[test]
    fn review_versions_share_one_checkbox_and_refresh_retains_only_known_identities() {
        let mut s = Selection::default();
        s.replace(&[c("a", "M", Area::Staged), c("a", "M", Area::Unstaged)]);
        assert_eq!(s.items().len(), 1); s.set(Path::new("a"), true);
        s.replace(&[c("new", "??", Area::Untracked), c("a", "M", Area::Unstaged)]);
        assert!(s.included(Path::new("a"))); assert!(!s.included(Path::new("new")));
        s.replace(&[]); s.replace(&[c("a", "M", Area::Unstaged)]); assert_eq!(s.count(), 0);
    }
    #[test]
    fn deletion_recreation_rename_and_copy_build_exact_path_sets() {
        let mut rename = c("renamed", "R", Area::Staged); rename.old_path = Some("old".into());
        let mut copy = c("copy", "C", Area::Staged); copy.old_path = Some("source".into());
        let mut s = Selection::default();
        s.replace(&[c("recreated", "D", Area::Staged), c("recreated", "??", Area::Untracked),
            c("gone", "A", Area::Staged), c("gone", "D", Area::Unstaged), rename, copy]);
        s.set_all(true); let request = s.request("message".into(), false, None).unwrap();
        assert_eq!(request.add, vec![PathBuf::from("copy"), "recreated".into(), "renamed".into()]);
        assert_eq!(request.remove, vec![PathBuf::from("gone"), "old".into()]);
    }
    #[test]
    fn conflicts_and_changed_rename_sources_cannot_inherit_inclusion() {
        let mut s = Selection::default(); let mut rename = c("new", "R", Area::Staged); rename.old_path = Some("old".into());
        s.replace(&[rename.clone()]); s.set_all(true); rename.old_path = Some("different".into()); s.replace(&[rename]);
        assert_eq!(s.count(), 0);
        s.replace(&[c("conflict", "UU", Area::Conflict)]); s.set_all(true); assert_eq!(s.count(), 0);
        assert!(s.request("message".into(), true, None).is_err());
    }
}
