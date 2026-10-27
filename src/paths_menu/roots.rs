//! Root selection stays independent of discovery and loaded commit rows.
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Default)]
pub(super) struct Roots {
    pub entries: Vec<(PathBuf, u32)>,
    pub selected: Option<BTreeSet<PathBuf>>,
}
impl Roots {
    pub fn replace(&mut self, mut entries: Vec<(PathBuf, u32)>) {
        entries.sort_by(|(a, _), (b, _)| a.file_name().cmp(&b.file_name()).then(a.cmp(b)));
        entries.dedup_by(|a, b| a.0 == b.0);
        self.entries = entries;
    }
    pub fn includes(&self, path: &Path) -> bool {
        self.selected
            .as_ref()
            .is_none_or(|roots| roots.contains(path))
    }
    pub fn choose(&mut self, path: PathBuf, only: bool) {
        if !self.entries.iter().any(|(root, _)| *root == path) {
            return;
        }
        if only {
            self.selected = Some([path].into_iter().collect());
        } else {
            let selected = self
                .selected
                .get_or_insert_with(|| self.entries.iter().map(|(p, _)| p.clone()).collect());
            if !selected.remove(&path) {
                selected.insert(path);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_selection_is_not_all_and_discovery_does_not_reselect_roots() {
        let a = PathBuf::from("a");
        let b = PathBuf::from("b");
        let mut roots = Roots::default();
        roots.replace(vec![(a.clone(), 0), (b.clone(), 1)]);
        assert!(roots.includes(&a) && roots.includes(&b));
        roots.choose(a.clone(), false);
        assert!(!roots.includes(&a) && roots.includes(&b));
        roots.choose(b.clone(), false);
        assert_eq!(roots.selected.as_ref().unwrap().len(), 0);
        roots.replace(vec![(a.clone(), 0), (b.clone(), 1), ("c".into(), 2)]);
        assert!(!roots.includes(Path::new("c")));
        roots.choose(b.clone(), true);
        assert!(!roots.includes(&a) && roots.includes(&b));
        roots.selected = None;
        assert!(roots.includes(Path::new("c")));
    }
}
