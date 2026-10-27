//! Identity-based Log selection shared by single- and multi-repository views.
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct IdentitySelection<Id> {
    selected: HashSet<Id>,
    selection_order: Vec<Id>,
    anchor: Option<Id>,
    lead: Option<Id>,
}
impl<Id: Clone + Eq + std::hash::Hash> IdentitySelection<Id> {
    pub fn contains(&self, oid: &Id) -> bool {
        self.selected.contains(oid)
    }
    pub fn ordered<'a>(&self, rows: &'a [Id]) -> Vec<&'a Id> {
        rows.iter().filter(|id| self.contains(id)).collect()
    }
    /// Selection-path order, distinct from display order and the last clicked row.
    pub fn first_matching(&self, mut matches: impl FnMut(&Id) -> bool) -> Option<&Id> {
        self.selection_order.iter().find(|id| matches(id))
    }
    fn insert(&mut self, id: &Id) {
        if self.selected.insert(id.clone()) {
            self.selection_order.push(id.clone());
        }
    }
    fn clear(&mut self) {
        self.selected.clear();
        self.selection_order.clear();
    }
    pub fn lead(&self, rows: &[Id]) -> Option<usize> {
        self.lead
            .as_ref()
            .and_then(|id| rows.iter().position(|row| row == id))
    }
    /// Keep only identities present in the replacement graph/window. An empty
    /// result remains empty; default-first selection is an explicit caller choice.
    pub fn reconcile(&mut self, rows: &[Id]) {
        let visible: HashSet<_> = rows.iter().collect();
        self.selected.retain(|id| visible.contains(id));
        self.selection_order.retain(|id| visible.contains(id));
        if self.anchor.as_ref().is_some_and(|id| !visible.contains(id)) {
            self.anchor = None;
        }
        if self.lead.as_ref().is_some_and(|id| !visible.contains(id)) {
            self.lead = None;
        }
    }
    /// JTable's documented toggle/extend conventions, represented with OIDs.
    pub fn select(&mut self, rows: &[Id], row: usize, toggle: bool, extend: bool) {
        let Some(id) = rows.get(row) else {
            return;
        };
        if extend {
            let anchor = self
                .anchor
                .as_ref()
                .and_then(|id| rows.iter().position(|r| r == id))
                .unwrap_or(0);
            let add = !toggle || self.anchor.as_ref().is_some_and(|id| self.contains(id));
            if !toggle {
                self.clear();
            }
            for id in &rows[anchor.min(row)..=anchor.max(row)] {
                if add {
                    self.insert(id);
                } else {
                    self.selected.remove(id);
                }
            }
            self.anchor = Some(rows[anchor].clone());
        } else {
            let remove = toggle && self.contains(id);
            if !toggle {
                self.clear();
            }
            if remove {
                self.selected.remove(id);
            } else {
                self.insert(id);
            }
            self.anchor = Some(id.clone());
        }
        // Prune once per gesture, not once per removed row in a large range.
        self.selection_order.retain(|id| self.selected.contains(id));
        self.lead = Some(id.clone());
    }
    /// A popup on a selected row preserves the collection and its range anchor.
    pub fn context(&mut self, rows: &[Id], row: usize) {
        if rows.get(row).is_some_and(|id| !self.contains(id)) {
            self.select(rows, row, false, false);
        }
    }
    pub fn step(&mut self, rows: &[Id], delta: isize, extend: bool) {
        if rows.is_empty() {
            return;
        }
        let row = self
            .lead(rows)
            .map(|i| i.saturating_add_signed(delta).min(rows.len() - 1))
            .unwrap_or(0);
        self.select(rows, row, false, extend);
    }
}

impl<Id> Default for IdentitySelection<Id> {
    fn default() -> Self {
        Self {
            selected: HashSet::new(),
            selection_order: Vec::new(),
            anchor: None,
            lead: None,
        }
    }
}

/// Compatibility surface for existing single-repository views.
#[derive(Clone, Default, Debug)]
pub struct Selection {
    inner: IdentitySelection<String>,
}
impl Selection {
    pub fn contains(&self, oid: &str) -> bool {
        self.inner.selected.contains(oid)
    }
    pub fn ordered<'a>(&self, rows: &'a [String]) -> Vec<&'a str> {
        self.inner
            .ordered(rows)
            .into_iter()
            .map(String::as_str)
            .collect()
    }
    pub fn first_matching(&self, mut matches: impl FnMut(&str) -> bool) -> Option<&str> {
        self.inner
            .first_matching(|id| matches(id))
            .map(String::as_str)
    }
    pub fn lead(&self, rows: &[String]) -> Option<usize> {
        self.inner.lead(rows)
    }
    pub fn reconcile(&mut self, rows: &[String]) {
        self.inner.reconcile(rows)
    }
    pub fn select(&mut self, rows: &[String], row: usize, toggle: bool, extend: bool) {
        self.inner.select(rows, row, toggle, extend)
    }
    pub fn context(&mut self, rows: &[String], row: usize) {
        self.inner.context(rows, row)
    }
    pub fn step(&mut self, rows: &[String], delta: isize, extend: bool) {
        self.inner.step(rows, delta, extend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows() -> Vec<String> {
        (1..=5).map(|n| format!("{n:040x}")).collect()
    }
    #[test]
    fn selection_path_order_survives_reorder_and_drops_removed_items() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 3, false, false);
        s.select(&rows, 1, true, false);
        assert_eq!(s.first_matching(|_| true), Some(rows[3].as_str()));
        assert_eq!(s.ordered(&rows), vec![rows[1].as_str(), rows[3].as_str()]);
        s.context(&rows, 1);
        assert_eq!(s.first_matching(|_| true), Some(rows[3].as_str()));
        s.select(&rows, 3, true, false);
        assert_eq!(s.first_matching(|_| true), Some(rows[1].as_str()));
        s.select(&rows, 3, true, false);
        s.reconcile(&[rows[3].clone()]);
        assert_eq!(s.first_matching(|_| true), Some(rows[3].as_str()));
        s.reconcile(&[]);
        assert_eq!(s.first_matching(|_| true), None);
    }
    #[test]
    fn toggle_and_range_have_separate_anchor_and_lead() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 1, false, false);
        s.select(&rows, 4, true, false);
        assert_eq!(s.ordered(&rows), [&*rows[1], &*rows[4]]);
        s.select(&rows, 2, false, true);
        assert_eq!(s.ordered(&rows), [&*rows[2], &*rows[3], &*rows[4]]);
        s.step(&rows, 1, true);
        assert_eq!(s.ordered(&rows), [&*rows[3], &*rows[4]]);
        assert_eq!(s.lead(&rows), Some(3));
        s.step(&rows, -1, false);
        assert_eq!(s.ordered(&rows), [&*rows[2]]);
    }
    #[test]
    fn toggle_extend_applies_selected_or_deselected_anchor() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 0, false, false);
        s.select(&rows, 2, true, true);
        assert_eq!(s.ordered(&rows).len(), 3);
        s.select(&rows, 2, true, false); // deselect anchor, keep rows 0/1
        s.select(&rows, 0, true, true); // deselect whole interval
        assert!(s.ordered(&rows).is_empty());
        assert!(s.first_matching(|_| true).is_none());
        assert_eq!(s.lead(&rows), Some(0));
    }
    #[test]
    fn popup_preserves_selected_set_but_replaces_for_unselected_row() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 1, false, false);
        s.select(&rows, 3, true, false);
        s.context(&rows, 1);
        assert_eq!(s.ordered(&rows), [&*rows[1], &*rows[3]]);
        assert_eq!(s.lead(&rows), Some(3));
        s.context(&rows, 4);
        assert_eq!(s.ordered(&rows), [&*rows[4]]);
    }
    #[test]
    fn refresh_reorder_filter_and_page_never_transfer_selection_by_index() {
        let rows = rows();
        let mut s = Selection::default();
        s.select(&rows, 1, false, false);
        s.select(&rows, 3, true, false);
        let changed = vec![rows[3].clone(), rows[0].clone(), rows[1].clone()];
        s.reconcile(&changed);
        assert_eq!(s.ordered(&changed), [&*rows[3], &*rows[1]]);
        assert_eq!(s.lead(&changed), Some(0));
        s.reconcile(&rows[..2]);
        assert_eq!(s.ordered(&rows), [&*rows[1]]);
        assert_eq!(s.lead(&rows), None);
        s.reconcile(&[]);
        assert!(s.ordered(&rows).is_empty());
        s.select(&[], 0, true, true);
        s.step(&[], 1, true);
        s.select(&rows, 999, false, false);
        assert!(s.ordered(&rows).is_empty());
    }
}
