// Adapted from GitRebaseCommitsTableModel.kt and ChangeEntryStateActions.kt.
// Copyright 2000-2024 JetBrains s.r.o. and contributors. Apache-2.0.
// See docs/THIRD-PARTY.txt.
//! Bounded edit history and message editing, independent of a window or Git process.
use super::model::{Action, Model, RowId};
use std::{collections::VecDeque, sync::Arc};

const STATES: usize = 10;
pub struct Draft<T> {
    initial: Model<Arc<T>>,
    states: VecDeque<Model<Arc<T>>>,
    current: usize,
    modified: bool,
}
impl<T> Draft<T> {
    pub fn new(initial: Model<Arc<T>>) -> Result<Self, String> {
        if initial.rows().len() > crate::git::HISTORY_MAX {
            return Err("Rebase plan exceeds history limit".into());
        }
        Ok(Self {
            states: VecDeque::from([initial.clone()]),
            initial,
            current: 0,
            modified: false,
        })
    }
    pub fn model(&self) -> &Model<Arc<T>> {
        &self.states[self.current]
    }
    pub fn modified(&self) -> bool {
        self.modified
    }
    pub fn can_undo(&self) -> bool {
        self.current > 0
    }
    pub fn can_redo(&self) -> bool {
        self.current + 1 < self.states.len()
    }
    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.current -= 1;
        true
    }
    pub fn redo(&mut self) -> bool {
        if !self.can_redo() {
            return false;
        }
        self.current += 1;
        true
    }
    pub fn reset(&mut self) {
        self.record(self.initial.clone());
    }
    fn record(&mut self, model: Model<Arc<T>>) {
        self.states.truncate(self.current + 1);
        if self.states.len() == STATES {
            self.states.pop_front();
        }
        self.states.push_back(model);
        self.current = self.states.len() - 1;
        self.modified = true;
    }
    pub fn set(&mut self, indices: &[usize], action: Action) -> bool {
        // Message edits must use reword so validation and restoration-to-Pick
        // cannot be bypassed by a toolbar callback.
        if matches!(action, Action::Reword(_)) {
            return false;
        }
        let mut next = self.model().clone();
        if !next.set(indices, action) {
            return false;
        }
        self.record(next);
        true
    }
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        let mut next = self.model().clone();
        if !next.exchange(from, to) {
            return false;
        }
        self.record(next);
        true
    }
    pub fn fixup(&mut self, indices: &[usize]) -> Option<RowId> {
        let mut next = self.model().clone();
        let root = next.unite(indices)?;
        self.record(next);
        Some(root)
    }
    pub fn reword(
        &mut self,
        index: usize,
        message: String,
        original: impl Fn(&T) -> &str,
    ) -> Result<bool, String> {
        validate_message(&message)?;
        if !self.model().can_reword(index) {
            return Ok(false);
        }
        let row = &self.model().rows()[index];
        let action = if message == original(&row.entry) {
            Action::Pick
        } else {
            Action::Reword(message.into())
        };
        let mut next = self.model().clone();
        if !next.set(&[index], action) {
            return Ok(false);
        }
        self.record(next);
        Ok(true)
    }
    /// Squash is a single undoable operation: group commits and prepare the root
    /// message for inline editing. Failure leaves both the plan and history intact.
    pub fn squash(
        &mut self,
        indices: &[usize],
        original: impl Fn(&T) -> &str,
    ) -> Result<Option<RowId>, String> {
        let Some(indices) = self.model().unite_selection(indices) else {
            return Ok(None);
        };
        let first = &self.model().rows()[indices[0]];
        let old_root = first.parent().unwrap_or(first.id);
        let old_children: Vec<_> = self
            .model()
            .rows()
            .iter()
            .filter(|row| row.parent() == Some(old_root))
            .map(|row| row.id)
            .collect();
        let existing_group = !old_children.is_empty();
        let mut next = self.model().clone();
        let Some(root) = next.unite(&indices) else {
            return Ok(None);
        };
        let messages: Vec<String> = next
            .rows()
            .iter()
            .filter(|row| {
                row.id == root || (row.parent() == Some(root) && !old_children.contains(&row.id))
            })
            .map(|row| {
                if existing_group {
                    if let Action::Reword(message) = row.action() {
                        return message.to_string();
                    }
                }
                original(&row.entry).to_owned()
            })
            .collect();
        let message = crate::squash_plan::draft(&messages)?;
        validate_message(&message)?;
        let root_index = next.index(root).unwrap();
        next.set(&[root_index], Action::Reword(message.into()));
        self.record(next);
        Ok(Some(root))
    }
}
fn validate_message(message: &str) -> Result<(), String> {
    if message.len() > crate::git::OUTPUT_LIMIT || message.contains('\0') {
        return Err("Rebase message exceeds preview limit or contains NUL".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn draft() -> Draft<String> {
        Draft::new(
            Model::new(
                ["base", "next", "last"]
                    .into_iter()
                    .map(|s| (Arc::new(s.to_owned()), Action::Pick))
                    .collect(),
            )
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn undo_redo_and_reset_keep_groups_and_stable_selection() {
        let mut d = draft();
        let id = d.model().rows()[1].id;
        d.fixup(&[1]).unwrap();
        assert!(d.modified());
        assert_eq!(d.model().rows()[1].action(), &Action::Fixup);
        assert!(d.undo());
        assert_eq!(d.model().rows()[1].action(), &Action::Pick);
        assert!(d.redo());
        assert_eq!(d.model().index(id), Some(1));
        d.reset();
        assert_eq!(d.model().rows()[1].action(), &Action::Pick);
        assert!(d.undo());
        assert_eq!(d.model().rows()[1].action(), &Action::Fixup);
        d.set(&[2], Action::Drop);
        assert!(!d.can_redo());
    }
    #[test]
    fn history_is_bounded_and_shares_commit_content() {
        let mut d = draft();
        let entry = d.model().rows()[0].entry.clone();
        for i in 0..30 {
            d.reword(0, format!("message {i}"), String::as_str).unwrap();
        }
        assert_eq!(d.states.len(), STATES);
        for state in &d.states {
            assert!(Arc::ptr_eq(&state.rows()[0].entry, &entry));
        }
        for _ in 0..9 {
            assert!(d.undo());
        }
        assert!(!d.undo());
        assert_eq!(
            d.model().rows()[0].action(),
            &Action::Reword("message 20".into())
        );
        d.reset();
        assert_eq!(d.model().rows()[0].action(), &Action::Pick);
    }
    #[test]
    fn squash_is_atomic_and_does_not_repeat_existing_group_messages() {
        let mut d = draft();
        d.squash(&[0, 1], String::as_str).unwrap().unwrap();
        assert_eq!(
            d.model().rows()[0].action(),
            &Action::Reword("base\n\n\nnext".into())
        );
        d.squash(&[2], String::as_str).unwrap().unwrap();
        assert_eq!(
            d.model().rows()[0].action(),
            &Action::Reword("base\n\n\nnext\n\n\nlast".into())
        );
        assert!(d.undo());
        assert_eq!(d.model().rows()[2].action(), &Action::Pick);
        let before = d.states.len();
        assert!(d.squash(&[2], |_| "invalid\0message").is_err());
        assert_eq!(d.states.len(), before);
        assert!(d.can_redo());
        assert_eq!(d.model().rows()[2].action(), &Action::Pick);
    }
    #[test]
    fn restoring_original_message_returns_to_pick_without_detaching_children() {
        let mut d = draft();
        d.squash(&[0, 1], String::as_str).unwrap();
        assert!(d.reword(0, "base".into(), String::as_str).unwrap());
        assert_eq!(d.model().rows()[0].action(), &Action::Pick);
        assert_eq!(d.model().rows()[1].action(), &Action::Fixup);
        assert!(!d.reword(1, "child".into(), String::as_str).unwrap());
    }
}
