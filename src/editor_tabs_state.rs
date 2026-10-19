//! Ordered editor identities and one active editor, independent of view storage.
//! Log is the fixed fallback (no active editor). Preview replacement keeps its slot.
#[derive(Debug)]
pub struct Tabs<Id> {
    order: Vec<Id>,
    active: Option<Id>,
}
impl<Id> Default for Tabs<Id> {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            active: None,
        }
    }
}
impl<Id: Copy + Eq> Tabs<Id> {
    pub fn order(&self) -> &[Id] {
        &self.order
    }
    pub fn active(&self) -> Option<Id> {
        self.active
    }
    pub fn activate(&mut self, id: Id) -> bool {
        if !self.order.contains(&id) {
            return false;
        }
        self.active = Some(id);
        true
    }
    pub fn deactivate(&mut self) -> Option<Id> {
        self.active.take()
    }
    pub fn insert(&mut self, id: Id) {
        if self.order.contains(&id) {
            return;
        }
        let index = self
            .active
            .and_then(|active| self.order.iter().position(|p| *p == active))
            .map_or(0, |i| i + 1);
        self.order.insert(index, id);
    }
    pub fn left_neighbor(&self, id: Id) -> Option<Id> {
        self.order
            .iter()
            .position(|p| *p == id)
            .and_then(|i| i.checked_sub(1))
            .map(|i| self.order[i])
    }
    pub fn remove(&mut self, id: Id) {
        self.order.retain(|p| *p != id);
        if self.active == Some(id) {
            self.active = None;
        }
    }
    pub fn replace(&mut self, old: Id, new: Id) -> bool {
        if self.order.contains(&new) {
            return false;
        }
        let Some(index) = self.order.iter().position(|p| *p == old) else {
            return false;
        };
        self.order[index] = new;
        if self.active == Some(old) {
            self.active = Some(new);
        }
        true
    }
    pub fn clear(&mut self) {
        self.order.clear();
        self.active = None;
    }
    /// Snapshot identities for delayed close actions; never retain vector indexes.
    pub fn others(&self, clicked: Option<Id>) -> Vec<Id> {
        self.order
            .iter()
            .copied()
            .filter(|id| Some(*id) != clicked)
            .collect()
    }
    pub fn right_of(&self, clicked: Option<Id>) -> Vec<Id> {
        let start = clicked
            .and_then(|id| self.order.iter().position(|p| *p == id))
            .map_or(0, |i| i + 1);
        self.order[start..].to_vec()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_tabs_keep_active_identity_when_an_earlier_tab_closes() {
        let mut tabs = Tabs::default();
        tabs.insert("source");
        tabs.activate("source");
        tabs.insert("diff");
        tabs.activate("diff");
        tabs.insert("history");
        tabs.activate("history");
        tabs.remove("source");
        assert_eq!(tabs.active(), Some("history"));
        assert_eq!(tabs.order(), &["diff", "history"]);
        assert!(!tabs.activate("missing"));
        assert_eq!(tabs.active(), Some("history"));
    }
    #[test]
    fn preview_replacement_keeps_position_and_removes_old_identity() {
        let mut tabs = Tabs::default();
        tabs.insert(1);
        tabs.activate(1);
        tabs.insert(2);
        tabs.activate(2);
        tabs.insert(3);
        assert!(tabs.replace(2, 4));
        assert_eq!(tabs.order(), &[1, 4, 3]);
        assert_eq!(tabs.active(), Some(4));
        tabs.remove(2);
        assert_eq!(tabs.active(), Some(4));
        assert!(!tabs.replace(4, 1));
    }
    #[test]
    fn close_snapshot_survives_insertion_and_falls_back_to_log() {
        let mut tabs = Tabs::default();
        tabs.insert(1);
        tabs.activate(1);
        tabs.insert(2);
        tabs.activate(2);
        tabs.insert(3);
        let right = tabs.right_of(Some(1));
        tabs.insert(4);
        for id in right {
            tabs.remove(id);
        }
        assert_eq!(tabs.order(), &[1, 4]);
        assert_eq!(tabs.active(), None);
        assert_eq!(tabs.others(Some(1)), vec![4]);
        tabs.activate(4);
        assert_eq!(tabs.left_neighbor(4), Some(1));
        tabs.clear();
        assert!(tabs.order().is_empty());
        assert_eq!(tabs.active(), None);
    }
}
