//! Metadata-only workspace tree. Review selection is independent of commit inclusion.
use std::{collections::HashSet, path::PathBuf};
use crate::{file_tree, git::{Area, Change}};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identity {
    pub area: Area,
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
}
impl From<&Change> for Identity {
    fn from(change: &Change) -> Self {
        Self { area: change.area, path: change.path.clone(), old_path: change.old_path.clone() }
    }
}
#[derive(Clone, Debug)]
pub enum Row {
    Group { area: Area, count: usize, expanded: bool },
    Entry { area: Area, row: file_tree::Row },
}
#[derive(Default)]
pub struct Model {
    flat: bool,
    files: Vec<Change>,
    rows: Vec<Row>,
    selected: Option<Identity>,
    collapsed_groups: HashSet<Area>,
    collapsed_directories: HashSet<(Area, PathBuf)>,
}
impl Model {
    pub fn grouped(&self)->bool { !self.flat }
    pub fn set_grouped(&mut self,grouped:bool){self.flat=!grouped;self.rebuild();}
    pub fn files(&self) -> &[Change] { &self.files }
    pub fn rows(&self) -> &[Row] { &self.rows }
    /// Includes collapsed descendants; path components prevent src matching src-other.
    pub fn inclusion_paths(&self, row: usize) -> HashSet<PathBuf> {
        match self.rows.get(row) {
            Some(Row::Group {area,..}) => self.files.iter().filter(|f| f.area == *area).map(|f|f.path.clone()).collect(),
            Some(Row::Entry {area,row:file_tree::Row::Folder{path,..}}) => self.files.iter()
                .filter(|f| f.area == *area && f.path.starts_with(path)).map(|f|f.path.clone()).collect(),
            Some(Row::Entry {row:file_tree::Row::File{index,..},..}) => [self.files[*index].path.clone()].into_iter().collect(),
            _ => HashSet::new(),
        }
    }
    pub fn selected(&self) -> Option<usize> {
        self.selected.as_ref().and_then(|id| self.files.iter().position(|f| Identity::from(f) == *id))
    }
    /// Replace only a completed listing; loading/errors must not clear retained selection.
    pub fn replace(&mut self, files: Vec<Change>) {
        file_tree::retain_collapsed(&mut self.collapsed_directories, files.iter().map(|f| (f.area, f.path.as_path())));
        self.collapsed_groups.retain(|area| files.iter().any(|f| f.area == *area));
        self.files = files;
        // Never silently change a selected index version into its working-tree version.
        if self.selected().is_none() { self.selected = None; }
        self.rebuild();
    }
    pub fn select(&mut self, index: usize) {
        if let Some(file) = self.files.get(index) { self.selected = Some(Identity::from(file)); }
    }
    /// Diff navigation reveals an exact version without changing commit inclusion.
    pub fn reveal(&mut self, change: &Change) -> Option<usize> {
        let index = self.files.iter().position(|f| Identity::from(f) == Identity::from(change))?;
        self.select(index);
        self.collapsed_groups.remove(&change.area);
        for parent in change.path.ancestors().skip(1) {
            self.collapsed_directories.remove(&(change.area, parent.to_path_buf()));
        }
        self.rebuild();
        self.rows.iter().position(|r| matches!(r, Row::Entry {row: file_tree::Row::File {index: i,..},..} if *i == index))
    }
    pub fn toggle(&mut self, row: usize) {
        match self.rows.get(row).cloned() {
            Some(Row::Group { area, .. }) => {
                if !self.collapsed_groups.remove(&area) { self.collapsed_groups.insert(area); }
            }
            Some(Row::Entry { area, row: file_tree::Row::Folder { path, .. } }) => {
                if !self.collapsed_directories.remove(&(area, path.clone())) { self.collapsed_directories.insert((area, path)); }
            }
            _ => return,
        }
        self.rebuild();
    }
    pub fn expand_all(&mut self) {
        self.collapsed_groups.clear(); self.collapsed_directories.clear(); self.rebuild();
    }
    pub fn collapse_all(&mut self) {
        // Keep the area groups visible, like LocalChangesListView's first level.
        self.collapsed_groups.clear();
        for f in &self.files {
            for dir in f.path.ancestors().skip(1).filter(|p| !p.as_os_str().is_empty()) {
                self.collapsed_directories.insert((f.area, dir.to_path_buf()));
            }
        }
        self.rebuild();
    }
    pub fn step(&mut self, forward: bool) {
        let visible: Vec<usize> = self.rows.iter().filter_map(|r| match r {
            Row::Entry { row: file_tree::Row::File { index, .. }, .. } => Some(*index), _ => None,
        }).collect();
        if visible.is_empty() { return; }
        let position = self.selected().and_then(|index| visible.iter().position(|i| *i == index));
        let next = match (position, forward) {
            (Some(i), true) => (i+1).min(visible.len()-1),
            (Some(i), false) => i.saturating_sub(1),
            (None, true) => 0, (None, false) => visible.len()-1,
        };
        self.select(visible[next]);
    }
    fn rebuild(&mut self) {
        self.rows.clear();
        for area in [Area::Conflict, Area::WorkingTree, Area::Unstaged, Area::Staged, Area::Untracked, Area::Ignored] {
            let count = self.files.iter().filter(|f| f.area == area).count();
            if count == 0 { continue; }
            let expanded = !self.collapsed_groups.contains(&area);
            self.rows.push(Row::Group { area, count, expanded });
            if expanded && self.flat {
                let mut files:Vec<_>=self.files.iter().enumerate().filter(|(_,f)|f.area==area).collect();
                files.sort_by_key(|(_,f)|f.path.clone());
                self.rows.extend(files.into_iter().map(|(index,_)|Row::Entry{area,row:file_tree::Row::File{index,depth:0}}));
            } else if expanded {
                self.rows.extend(file_tree::rows(self.files.iter().enumerate().filter(|(_, f)| f.area == area).map(|(i,f)| (i,f.path.as_path())),
                    |p| self.collapsed_directories.contains(&(area,p.to_path_buf())))
                    .into_iter().map(|row| Row::Entry { area, row }));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn file(path: &str, area: Area) -> Change { Change { path: path.into(), old_path: None, status: "M".into(), area } }
    #[test]
    fn grouping_switch_preserves_selected_identity_and_ignored_scope_is_separate() {
        let mut m=Model::default();let target=file("src/a",Area::WorkingTree);
        m.replace(vec![target.clone(),file("build",Area::Ignored),file("src/b",Area::WorkingTree)]);
        m.select(0);m.collapse_all();m.set_grouped(false);
        assert_eq!(m.selected(),Some(0));
        assert!(!m.rows().iter().any(|r|matches!(r,Row::Entry{row:file_tree::Row::Folder{..},..})));
        let group=m.rows().iter().position(|r|matches!(r,Row::Group{area:Area::WorkingTree,..})).unwrap();
        assert_eq!(m.inclusion_paths(group).len(),2);
        m.set_grouped(true);assert!(m.reveal(&target).is_some());assert_eq!(m.selected(),Some(0));
    }
    #[test]
    fn diff_navigation_reveals_collapsed_exact_version() {
        let mut m = Model::default();
        let target = file("src/nested/b.rs", Area::WorkingTree);
        m.replace(vec![file("src/a.rs", Area::WorkingTree), target.clone(), file("src/nested/b.rs", Area::Staged)]);
        m.collapse_all();
        let row = m.reveal(&target).unwrap();
        assert_eq!(m.selected(), Some(1));
        assert!(matches!(m.rows()[row], Row::Entry {row: file_tree::Row::File {index: 1,..},..}));
        assert!(m.reveal(&file("missing", Area::WorkingTree)).is_none());
        assert_eq!(m.selected(), Some(1));
    }
    #[test]
    fn group_scope_includes_collapsed_descendants_but_not_other_areas_or_prefix_siblings() {
        let mut m=Model::default();
        m.replace(vec![file("src/a",Area::Unstaged),file("src/nested/b",Area::Unstaged),
            file("src-other/c",Area::Unstaged),file("src/staged",Area::Staged)]);
        m.select(0); m.collapse_all();
        let folder=m.rows().iter().position(|r|matches!(r,Row::Entry{area:Area::Unstaged,
            row:file_tree::Row::Folder{path,..}} if path==std::path::Path::new("src"))).unwrap();
        assert_eq!(m.inclusion_paths(folder),["src/a","src/nested/b"].map(PathBuf::from).into_iter().collect());
        let group=m.rows().iter().position(|r|matches!(r,Row::Group{area:Area::Unstaged,..})).unwrap();
        m.toggle(group); assert_eq!(m.inclusion_paths(group).len(),3); assert_eq!(m.selected(),Some(0));
    }
    #[test]
    fn refresh_preserves_exact_area_and_path_after_reorder() {
        let mut m = Model::default();
        m.replace(vec![file("src/中文.rs",Area::Staged),file("src/中文.rs",Area::Unstaged)]); m.select(0);
        m.replace(vec![file("new",Area::Untracked),file("src/中文.rs",Area::Unstaged),file("src/中文.rs",Area::Staged)]);
        assert_eq!(m.selected(),Some(2));
        m.replace(vec![file("src/中文.rs",Area::Unstaged)]);
        assert_eq!(m.selected(),None);
    }
    #[test]
    fn rename_origin_is_part_of_review_identity() {
        let mut m = Model::default(); let mut f = file("new.rs",Area::Staged); f.old_path=Some("old.rs".into());
        m.replace(vec![f.clone()]); m.select(0); f.old_path=Some("different.rs".into()); m.replace(vec![f]);
        assert_eq!(m.selected(),None);
    }
    #[test]
    fn collapsing_retains_selection_and_keyboard_skips_hidden_files() {
        let mut m = Model::default();
        m.replace(vec![file("src/a",Area::Unstaged),file("root",Area::Untracked)]); m.select(0);
        m.collapse_all(); assert_eq!(m.selected(),Some(0)); m.step(true); assert_eq!(m.selected(),Some(1));
        m.expand_all(); m.step(false); assert_eq!(m.selected(),Some(0));
    }
    #[test]
    fn empty_or_failed_refresh_does_not_redirect_selection() {
        let mut m = Model::default(); m.replace(vec![file("a",Area::Untracked)]); m.select(0);
        // A failed request does not call replace at all.
        assert_eq!(m.selected(),Some(0)); m.replace(vec![]); m.step(true); assert_eq!(m.selected(),None);
        m.replace(vec![file("b",Area::Untracked)]); assert_eq!(m.selected(),None);
    }
    #[test]
    fn removed_directories_do_not_leak_collapsed_state_to_new_listing() {
        let mut m = Model::default(); m.replace(vec![file("src/a",Area::Unstaged)]); m.collapse_all();
        m.replace(vec![]); m.replace(vec![file("src/b",Area::Unstaged)]);
        assert!(m.rows().iter().any(|r| matches!(r, Row::Entry{row:file_tree::Row::File{..},..})));
    }
}

/// Ignored directories remain collapsed metadata entries; never enumerate build output.
pub fn ignored(repo:&crate::git::Repository,token:&crate::git::Cancel)->Result<Vec<Change>,String>{
    let bytes=repo.read(&["ls-files","--others","--ignored","--exclude-standard","--directory","-z"].map(std::ffi::OsStr::new),token)?;
    bytes.split(|b|*b==0).filter(|b|!b.is_empty()).map(|b|Ok(Change{path:crate::git::path_from_bytes(b)?,old_path:None,status:"!".into(),area:Area::Ignored})).collect()
}
