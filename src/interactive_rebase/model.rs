// Adapted from JetBrains GitRebaseTodoModel.kt (Apache-2.0).
// Copyright 2000-2024 JetBrains s.r.o. and contributors.
// Source/version and license: docs/THIRD-PARTY.txt.
//! Stable row identities keep selection independent of reorder/group operations.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Pick,
    Edit,
    Reword(std::sync::Arc<str>),
    Drop,
    UpdateRef,
    Fixup,
}
impl Action {
    fn keep(&self) -> bool {
        matches!(self, Self::Pick | Self::Edit | Self::Reword(_))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RowId(usize);
#[derive(Clone, Debug)]
pub struct Row<T> {
    pub id: RowId,
    pub entry: T,
    action: Action,
    parent: Option<RowId>,
}
impl<T> Row<T> {
    pub fn action(&self) -> &Action {
        &self.action
    }
    pub fn parent(&self) -> Option<RowId> {
        self.parent
    }
}
#[derive(Clone)]
pub struct Model<T> {
    rows: Vec<Row<T>>,
}
impl<T> Model<T> {
    /// Initial rows must be independent. Autosquash conversion groups them via
    /// unite after validating the real Git todo rather than inventing parents.
    pub fn new(entries: Vec<(T, Action)>) -> Result<Self, &'static str> {
        if entries.iter().any(|(_, action)| *action == Action::Fixup) {
            return Err("Fixup needs a preceding group");
        }
        Ok(Self {
            rows: entries
                .into_iter()
                .enumerate()
                .map(|(id, (entry, action))| Row {
                    id: RowId(id),
                    entry,
                    action,
                    parent: None,
                })
                .collect(),
        })
    }
    pub fn rows(&self) -> &[Row<T>] {
        &self.rows
    }
    pub fn index(&self, id: RowId) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }
    fn selected(&self, indices: &[usize]) -> Option<Vec<RowId>> {
        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != indices.len() || sorted.last().is_some_and(|i| *i >= self.rows.len()) {
            return None;
        }
        Some(sorted.into_iter().map(|i| self.rows[i].id).collect())
    }
    fn group(&self, root: RowId) -> Vec<RowId> {
        self.rows
            .iter()
            .filter(|r| r.id == root || r.parent == Some(root))
            .map(|r| r.id)
            .collect()
    }
    pub fn can_reword(&self, index: usize) -> bool {
        self.rows
            .get(index)
            .is_some_and(|r| r.parent.is_none() && r.action != Action::UpdateRef)
    }
    pub fn can_set(&self, indices: &[usize], action: &Action) -> bool {
        if !action.keep() && *action != Action::Drop {
            return false;
        }
        self.selected(indices).is_some_and(|ids| {
            ids.iter().any(|id| {
                let r = &self.rows[self.index(*id).unwrap()];
                r.action != Action::UpdateRef && r.action != *action
            })
        })
    }
    /// Reword's UI only accepts one independent row; Pick/Edit can detach children.
    pub fn set(&mut self, indices: &[usize], action: Action) -> bool {
        if matches!(action, Action::Reword(_))
            && (indices.len() != 1 || !self.can_reword(indices[0]))
        {
            return false;
        }
        if !self.can_set(indices, &action) {
            return false;
        }
        let ids = self.selected(indices).unwrap();
        if action == Action::Drop {
            // Dropping a group root drops the whole group before selected children
            // are detached, matching the upstream two-phase operation.
            for id in &ids {
                let i = self.index(*id).unwrap();
                if self.rows[i].parent.is_none() && self.rows[i].action != Action::UpdateRef {
                    for member in self.group(*id) {
                        let i = self.index(member).unwrap();
                        self.rows[i].parent = None;
                        self.rows[i].action = Action::Drop;
                    }
                }
            }
        }
        for id in ids.into_iter().rev() {
            let i = self.index(id).unwrap();
            if self.rows[i].action == Action::UpdateRef {
                continue;
            }
            self.detach(id, action.clone());
        }
        debug_assert!(self.valid());
        true
    }
    fn detach(&mut self, id: RowId, action: Action) {
        let i = self.index(id).unwrap();
        let end = self.rows[i]
            .parent
            .map(|root| self.index(*self.group(root).last().unwrap()).unwrap());
        self.rows[i].parent = None;
        self.rows[i].action = action;
        if let Some(end) = end {
            self.move_rows(&[id], end);
        }
    }
    pub fn can_unite(&self, indices: &[usize]) -> bool {
        let Some(ids) = self.selected(indices) else {
            return false;
        };
        if ids.len() < 2
            || indices
                .iter()
                .any(|i| self.rows[*i].action == Action::UpdateRef)
        {
            return false;
        }
        let first = &self.rows[self.index(ids[0]).unwrap()];
        let root = first.parent.unwrap_or(first.id);
        ids.iter()
            .skip(1)
            .any(|id| self.rows[self.index(*id).unwrap()].parent != Some(root))
    }
    /// A single selection joins the nearest earlier kept commit/group, skipping
    /// dropped rows and update-ref commands. Already joined children are disabled.
    pub fn unite_selection(&self, indices: &[usize]) -> Option<Vec<usize>> {
        if self.can_unite(indices) {
            let mut indices = indices.to_vec();
            indices.sort_unstable();
            return Some(indices);
        }
        let [index] = indices else {
            return None;
        };
        if self.rows.get(*index)?.action == Action::UpdateRef {
            return None;
        }
        let previous = (0..*index)
            .rev()
            .find(|i| self.rows[*i].parent.is_none() && self.rows[*i].action.keep())?;
        let joined = vec![previous, *index];
        self.can_unite(&joined).then_some(joined)
    }
    pub fn unite(&mut self, indices: &[usize]) -> Option<RowId> {
        let indices = self.unite_selection(indices)?;
        let ids = self.selected(&indices)?;
        let first = &self.rows[self.index(ids[0])?];
        let root = first.parent.unwrap_or(first.id);
        let root_index = self.index(root)?;
        if !self.rows[root_index].action.keep() {
            self.rows[root_index].action = Action::Pick;
        }
        let mut children = Vec::new();
        for id in ids.into_iter().skip(1) {
            if self.rows[self.index(id)?].parent == Some(root) {
                continue;
            }
            for child in self.group(id) {
                if !children.contains(&child) {
                    children.push(child);
                }
            }
        }
        let position = self.index(*self.group(root).last()?)? + 1;
        self.move_rows(&children, position);
        for id in children {
            let i = self.index(id)?;
            self.rows[i].parent = Some(root);
            self.rows[i].action = Action::Fixup;
        }
        debug_assert!(self.valid());
        Some(root)
    }
    pub fn exchange(&mut self, old: usize, new: usize) -> bool {
        if old >= self.rows.len() || new >= self.rows.len() || old == new {
            return false;
        }
        let id = self.rows[old].id;
        let moved = if let Some(parent) = self.rows[old].parent {
            self.detach(id, Action::Pick);
            if self.index(id) == Some(new) {
                let i = self.index(id).unwrap();
                self.rows[i].parent = Some(parent);
                self.rows[i].action = Action::Fixup;
                return true;
            }
            vec![id]
        } else {
            self.group(id)
        };
        self.move_rows(&moved, new);
        let last = self.index(*moved.last().unwrap()).unwrap();
        if let Some(root) = self.rows.get(last + 1).and_then(|r| r.parent) {
            for id in moved {
                let i = self.index(id).unwrap();
                self.rows[i].parent = Some(root);
                self.rows[i].action = Action::Fixup;
            }
        }
        debug_assert!(self.valid());
        true
    }
    fn move_rows(&mut self, ids: &[RowId], position: usize) {
        if ids.is_empty() {
            return;
        }
        let mut target = position.min(self.rows.len() - ids.len());
        if ids
            .iter()
            .any(|id| self.rows[self.index(*id).unwrap()].action == Action::UpdateRef)
        {
            let row = &self.rows[target];
            if self.index(ids[0]).unwrap() > position {
                if let Some(root) = row.parent {
                    target = self.index(root).unwrap();
                }
            } else if self.group(row.id).len() > 1 {
                target = self.index(*self.group(row.id).last().unwrap()).unwrap();
            }
        }
        let mut moved = Vec::new();
        let mut remaining = Vec::with_capacity(self.rows.len());
        for row in self.rows.drain(..) {
            if ids.contains(&row.id) {
                moved.push(row);
            } else {
                remaining.push(row);
            }
        }
        moved.sort_by_key(|row| ids.iter().position(|id| *id == row.id).unwrap());
        remaining.splice(target..target, moved);
        self.rows = remaining;
    }
    fn valid(&self) -> bool {
        self.rows.iter().enumerate().all(|(i, row)| {
            if let Some(parent) = row.parent {
                let Some(root) = self.index(parent) else {
                    return false;
                };
                root < i
                    && self.rows[root].parent.is_none()
                    && self.rows[root].action.keep()
                    && row.action == Action::Fixup
                    && self.rows[root + 1..i]
                        .iter()
                        .all(|r| r.parent == Some(parent))
            } else {
                row.action != Action::Fixup
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> Model<usize> {
        Model::new((0..6).map(|id| (id, Action::Pick)).collect()).unwrap()
    }
    fn order(model: &Model<usize>) -> Vec<usize> {
        model.rows().iter().map(|r| r.entry).collect()
    }
    #[test]
    fn fixup_groups_selection_and_pick_detaches_after_the_group() {
        let mut m = plan();
        assert!(m.unite(&[0]).is_none());
        let root = m.unite(&[1, 3, 4]).unwrap();
        assert_eq!(order(&m), [0, 1, 3, 4, 2, 5]);
        assert_eq!(m.rows()[2].parent(), Some(root));
        assert!(!m.can_reword(2));
        assert!(m.unite_selection(&[2]).is_none());
        assert!(m.set(&[2], Action::Pick));
        assert_eq!(order(&m), [0, 1, 4, 3, 2, 5]);
        assert_eq!(m.rows()[3].parent(), None);
        assert!(m.valid());
    }
    #[test]
    fn moving_root_carries_children_and_drop_marks_the_whole_group() {
        let mut m = plan();
        m.unite(&[1, 2]);
        m.exchange(1, 4);
        assert_eq!(order(&m), [0, 3, 4, 5, 1, 2]);
        assert!(m.set(&[4], Action::Drop));
        assert_eq!(m.rows()[4].action(), &Action::Drop);
        assert_eq!(m.rows()[5].action(), &Action::Drop);
        assert_eq!(m.rows()[5].parent(), None);
    }
    #[test]
    fn single_fixup_skips_dropped_and_update_ref_rows() {
        let mut m = Model::new(vec![
            (0, Action::Pick),
            (1, Action::UpdateRef),
            (2, Action::Drop),
            (3, Action::Pick),
        ])
        .unwrap();
        assert_eq!(m.unite_selection(&[3]), Some(vec![0, 3]));
        assert!(m.unite(&[0, 1]).is_none());
        assert!(!m.set(&[1], Action::Drop));
        m.unite(&[3]);
        assert_eq!(order(&m), [0, 3, 1, 2]);
        assert_eq!(m.rows()[2].action(), &Action::UpdateRef);
    }
    #[test]
    fn joining_groups_retains_root_message_and_every_commit() {
        let mut m = plan();
        m.unite(&[0, 1]);
        m.unite(&[3, 4]);
        m.set(&[0], Action::Reword("new\n\nmessage".into()));
        m.unite(&[0, 3]);
        assert_eq!(order(&m), [0, 1, 3, 4, 2, 5]);
        assert_eq!(
            m.rows()[0].action(),
            &Action::Reword("new\n\nmessage".into())
        );
        assert!(
            m.rows()[1..4]
                .iter()
                .all(|r| r.parent() == Some(m.rows()[0].id))
        );
        let before = order(&m);
        assert!(!m.set(&[100], Action::Drop));
        assert!(m.unite(&[0, 0]).is_none());
        assert_eq!(order(&m), before);
    }
    #[test]
    fn all_move_destinations_preserve_contiguous_groups_and_row_identity() {
        for update_ref in [false, true] {
            for old in 0..6 {
                for new in 0..6 {
                    let mut m = plan();
                    if update_ref {
                        m.rows[5].action = Action::UpdateRef;
                    }
                    m.unite(&[1, 2, 3]);
                    m.exchange(old, new);
                    assert!(m.valid(), "{update_ref} {old}->{new}");
                    let mut ids = order(&m);
                    ids.sort_unstable();
                    assert_eq!(ids, [0, 1, 2, 3, 4, 5]);
                    for mask in 1..64 {
                        let selected: Vec<_> = (0..6).filter(|i| mask & (1 << i) != 0).collect();
                        for action in [Action::Pick, Action::Edit, Action::Drop] {
                            let mut copy = m.clone();
                            copy.set(&selected, action);
                            assert!(copy.valid());
                            let mut ids = order(&copy);
                            ids.sort_unstable();
                            assert_eq!(ids, [0, 1, 2, 3, 4, 5]);
                        }
                        let mut copy = m.clone();
                        copy.unite(&selected);
                        assert!(copy.valid());
                    }
                }
            }
        }
    }
}

pub mod conversion;
