//! Tree-row selection, separate from the descendant files used by Diff.
//! Rules: Rebased 4aec352 TreeAction.select and VcsTreeModelData.SelectedData
//! (JetBrains, Apache-2.0). Paths retain native encoding; directories are identities.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Node {
    File(PathBuf),
    Directory(PathBuf),
}
#[derive(Default, Debug)]
pub struct Selection {
    selected: HashSet<Node>,
    order: Vec<Node>,
    anchor: Option<Node>,
    lead: Option<Node>,
}
impl Selection {
    pub fn lead_node(&self) -> Option<&Node> {
        self.lead.as_ref()
    }
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
    pub fn replace(&mut self, nodes: Vec<Node>) {
        self.anchor = nodes.first().cloned();
        self.lead = self.anchor.clone();
        self.selected.clear();
        self.order.clear();
        for node in nodes {
            self.insert(node);
        }
    }

    pub fn ordered(&self) -> &[Node] {
        &self.order
    }
    fn insert(&mut self, node: Node) {
        if self.selected.insert(node.clone()) {
            self.order.push(node);
        }
    }
    pub fn contains(&self, node: &Node) -> bool {
        self.selected.contains(node)
    }
    pub fn includes_file(&self, path: &Path) -> bool {
        self.selected.iter().any(|node| match node {
            Node::File(file) => file == path,
            Node::Directory(folder) => path.starts_with(folder),
        })
    }
    pub fn lead(&self, visible: &[Node]) -> Option<usize> {
        self.lead
            .as_ref()
            .and_then(|lead| visible.iter().position(|node| node == lead))
    }
    pub fn select(&mut self, visible: &[Node], row: usize, toggle: bool, extend: bool) {
        let Some(node) = visible.get(row) else {
            return;
        };
        let anchor = self
            .anchor
            .as_ref()
            .and_then(|anchor| visible.iter().position(|node| node == anchor));
        if extend && let Some(anchor) = anchor {
            if !toggle {
                self.selected.clear();
                self.order.clear();
            }
            for node in &visible[anchor.min(row)..=anchor.max(row)] {
                self.insert(node.clone());
            }
        } else {
            if !toggle {
                self.selected.clear();
                self.order.clear();
            }
            if !toggle || !self.selected.remove(node) {
                self.insert(node.clone());
            }
            self.anchor = Some(node.clone());
        }
        self.order.retain(|node| self.selected.contains(node));
        self.lead = Some(node.clone());
    }
    pub fn step(&mut self, visible: &[Node], delta: isize, extend: bool) {
        if visible.is_empty() {
            return;
        }
        let row = self
            .lead(visible)
            .map(|row| row.saturating_add_signed(delta).min(visible.len() - 1))
            .unwrap_or(0);
        self.select(visible, row, false, extend);
    }
    /// Refresh uses all model nodes, not only expanded rows, so collapsed selections survive.
    pub fn retain(&mut self, mut exists: impl FnMut(&Node) -> bool) {
        self.selected.retain(&mut exists);
        self.order.retain(|node| self.selected.contains(node));
        if self.anchor.as_ref().is_some_and(|node| !exists(node)) {
            self.anchor = None;
        }
        if self.lead.as_ref().is_some_and(|node| !exists(node)) {
            self.lead = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows() -> Vec<Node> {
        vec![
            Node::Directory("nested".into()),
            Node::File("nested/beta".into()),
            Node::File("nested/gamma".into()),
            Node::File("alpha".into()),
        ]
    }
    #[test]
    fn ctrl_order_survives_removal_readdition_and_refresh() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 2, false, false);
        s.select(&rows, 1, true, false);
        assert_eq!(s.ordered(), &[rows[2].clone(), rows[1].clone()]);
        s.select(&rows, 2, true, false);
        s.select(&rows, 2, true, false);
        assert_eq!(s.ordered(), &[rows[1].clone(), rows[2].clone()]);
        s.retain(|node| node != &rows[1]);
        assert_eq!(s.ordered(), &[rows[2].clone()]);
        s.replace(vec![rows[2].clone(), rows[2].clone()]);
        assert_eq!(s.ordered(), &[rows[2].clone()]);
    }
    #[test]
    fn directory_and_file_highlights_differ_from_diff_membership() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 0, false, false);
        s.select(&rows, 3, true, false);
        assert!(s.contains(&rows[0]) && s.contains(&rows[3]));
        assert!(!s.contains(&rows[1]) && !s.contains(&rows[2]));
        for path in ["nested/beta", "nested/gamma", "alpha"] {
            assert!(s.includes_file(Path::new(path)));
        }
        assert!(!s.includes_file(Path::new("nested-other/beta")));
        s.select(&rows, 0, true, false);
        assert!(!s.includes_file(Path::new("nested/beta")));
        assert!(s.includes_file(Path::new("alpha")));
    }
    #[test]
    fn keyboard_range_shrinks_back_to_directory_anchor_without_wrapping() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 0, false, false);
        s.step(&rows, 1, true);
        assert!(s.contains(&rows[0]) && s.contains(&rows[1]));
        s.step(&rows, -1, true);
        s.step(&rows, -1, true);
        assert_eq!(s.selected.len(), 1);
        assert_eq!(s.lead(&rows), Some(0));
        assert!(s.includes_file(Path::new("nested/gamma")));
    }
    #[test]
    fn hidden_anchor_falls_back_to_target_and_deleted_paths_are_removed() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 1, false, false);
        let visible = vec![rows[0].clone(), rows[3].clone()];
        s.select(&visible, 1, false, true);
        assert_eq!(s.selected.len(), 1);
        assert!(s.contains(&rows[3]));
        s.retain(|node| node != &rows[3]);
        assert!(s.selected.is_empty());
        assert_eq!(s.lead(&rows), None);
    }
}
