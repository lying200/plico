//! Plan-table interaction adapted from Rebased 4aec352 GitInteractiveRebaseDialog,
//! ChangeEntryStateActions and MoveTableItemRunnable (JetBrains, Apache-2.0).
//! Git/editor ownership stays in the operation; this view owns only its draft.
use crate::ui_style::{BG, BORDER, FG, MUTED, SELECT};
use gpui_kit::component::{
    Disableable as _, Icon, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Textarea, TextareaState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    i18n::t,
    interactive_rebase::{
        draft::Draft,
        model::{Action, Model, RowId, conversion::Item},
    },
};
use std::sync::Arc;

pub struct Changed;
impl EventEmitter<Changed> for PlanTable {}
struct MessageEditor {
    row: RowId,
    input: Entity<TextareaState>,
}
pub struct PlanTable {
    draft: Draft<Item>,
    selected: Vec<RowId>,
    anchor: Option<RowId>,
    editor: Option<MessageEditor>,
    list: ListState,
    focus: FocusHandle,
    error: Option<String>,
    reset_visible: bool,
    reword_invoked: bool,
    drag: Option<drag::RowDrag>,
    pending_mouse_selection: Option<(u64, MouseDownEvent)>,
    mouse_selection_serial: u64,
}
impl PlanTable {
    pub fn from_draft(draft: Draft<Item>, cx: &mut Context<Self>) -> Self {
        let count = draft.model().rows().len();
        let selected: Vec<_> = draft
            .model()
            .rows()
            .first()
            .map(|r| r.id)
            .into_iter()
            .collect();
        Self {
            draft,
            anchor: selected.first().copied(),
            selected,
            editor: None,
            list: ListState::new(count, ListAlignment::Top, px(200.)),
            focus: cx.focus_handle(),
            error: None,
            reset_visible: false,
            reword_invoked: false,
            drag: None,
            pending_mouse_selection: None,
            mouse_selection_serial: 0,
        }
    }
    pub fn model(&self) -> &Model<Arc<Item>> {
        self.draft.model()
    }
    pub fn modified(&self) -> bool {
        self.draft.modified() || self.reword_invoked
    }
    pub fn editing(&self) -> bool {
        self.editor.is_some()
    }
    pub fn selected_items(&self) -> Vec<Arc<Item>> {
        self.indices()
            .into_iter()
            .map(|i| self.model().rows()[i].entry.clone())
            .collect()
    }
    fn indices(&self) -> Vec<usize> {
        self.model()
            .rows()
            .iter()
            .enumerate()
            .filter_map(|(i, r)| self.selected.contains(&r.id).then_some(i))
            .collect()
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.pending_mouse_selection = None;
        // Invalidate measured heights as editing can add/remove an inline row.
        self.list.splice_focusable(
            0..self.model().rows().len(),
            self.model().rows().iter().map(|row| {
                self.editor
                    .as_ref()
                    .filter(|editor| editor.row == row.id)
                    .map(|editor| editor.input.read(cx).focus_handle(cx))
            }),
        );
        if let Some(i) = self.indices().first() {
            self.list.scroll_to_reveal_item(*i);
        }
        cx.emit(Changed);
        cx.notify();
    }
    fn select(
        &mut self,
        index: usize,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_mouse_selection = None;
        if !self.apply_message(window, cx) {
            return;
        }
        let id = self.model().rows()[index].id;
        if modifiers.shift {
            let start = self
                .anchor
                .and_then(|id| self.model().index(id))
                .unwrap_or(index);
            self.selected = self.model().rows()[start.min(index)..=start.max(index)]
                .iter()
                .map(|r| r.id)
                .collect();
        } else if modifiers.control || modifiers.platform {
            if self.selected.contains(&id) {
                self.selected.retain(|x| *x != id);
            } else {
                self.selected.push(id);
            }
            self.anchor = Some(id);
        } else {
            self.selected = vec![id];
            self.anchor = Some(id);
        }
        self.focus.focus(window, cx);
        cx.emit(Changed);
        cx.notify();
    }
    fn set_action(&mut self, action: Action, cx: &mut Context<Self>) {
        if !self.editing() && self.draft.set(&self.indices(), action) {
            self.reset_visible = true;
            self.changed(cx);
        }
    }
    // Rebased's action emits a model notification even before editing. Keep its
    // cancel/reset semantics, but retain actual message snapshots for redo.
    fn reword_action(&mut self, row: RowId, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing()
            || !self
                .model()
                .index(row)
                .is_some_and(|i| self.model().can_reword(i))
        {
            return;
        }
        self.reword_invoked = true;
        self.reset_visible = true;
        self.begin_message(row, window, cx);
    }
    fn begin_message(&mut self, row: RowId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.model().index(row) else {
            return;
        };
        if self.editing() || !self.model().can_reword(index) {
            return;
        }
        let item = &self.model().rows()[index];
        let text = match item.action() {
            Action::Reword(text) => text.as_ref(),
            _ => original(&item.entry),
        }
        .to_owned();
        let input = cx.new(|cx| {
            let mut s = TextareaState::new(window, cx);
            s.set_value(text, window, cx);
            s
        });
        input.read(cx).focus_handle(cx).focus(window, cx);
        self.editor = Some(MessageEditor { row, input });
        self.selected = vec![row];
        self.anchor = Some(row);
        self.changed(cx);
    }
    fn apply_message(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(editor) = &self.editor else {
            return true;
        };
        let Some(index) = self.model().index(editor.row) else {
            return false;
        };
        match self
            .draft
            .reword(index, editor.input.read(cx).value().to_string(), original)
        {
            Ok(changed) => {
                self.reset_visible |= changed;
                self.editor = None;
                self.error = None;
                self.focus.focus(window, cx);
                self.changed(cx);
                true
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                false
            }
        }
    }
    fn dismiss_editor(&mut self, reset: bool, window: &mut Window, cx: &mut Context<Self>) {
        let dirty = self.editor.as_ref().is_some_and(|editor| {
            self.model().index(editor.row).is_some_and(|index| {
                let row = &self.model().rows()[index];
                let current = match row.action() {
                    Action::Reword(text) => text.as_ref(),
                    _ => original(&row.entry),
                };
                editor.input.read(cx).value().as_str() != current
            })
        });
        if !dirty {
            self.discard_editor(reset, window, cx);
            return;
        }
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            dialog
                .title(t("Cancel Reword"))
                .centered(true)
                .w(px(420.))
                .overlay_closable(false)
                .child(t("Entered commit message will be lost"))
                .footer(
                    gpui_kit::component::dialog::DialogFooter::new()
                        .child(
                            gpui_kit::component::dialog::DialogAction::new().child(
                                Button::new("discard-reword")
                                    .primary()
                                    .label(t("Cancel Reword")),
                            ),
                        )
                        .child(
                            gpui_kit::component::dialog::DialogClose::new()
                                .trigger(|button| button.label(t("Keep Editing"))),
                        ),
                )
                .on_ok(move |_, w, cx| {
                    let _ = owner.update(cx, |s, cx| s.discard_editor(reset, w, cx));
                    true
                })
        });
    }
    fn discard_editor(&mut self, reset: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.editor = None;
        self.error = None;
        if reset {
            self.draft.reset();
            self.reset_visible = false;
        }
        self.focus.focus(window, cx);
        self.changed(cx);
    }
    fn unite(&mut self, squash: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing() {
            return;
        }
        let indices = self.indices();
        let root = if squash {
            match self.draft.squash(&indices, original) {
                Ok(root) => root,
                Err(e) => {
                    self.error = Some(e);
                    cx.notify();
                    return;
                }
            }
        } else {
            self.draft.fixup(&indices)
        };
        if let Some(root) = root {
            self.reset_visible = true;
            self.changed(cx);
            if squash {
                self.begin_message(root, window, cx);
            }
        }
    }
    fn can_move(&self, delta: isize) -> bool {
        let indices = self.indices();
        !self.editing()
            && !indices.is_empty()
            && indices.iter().all(|i| {
                i.checked_add_signed(delta)
                    .is_some_and(|j| j < self.model().rows().len())
            })
    }
    fn move_rows(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_move(delta) {
            return;
        }
        let mut indices = self.indices();
        if delta > 0 {
            indices.reverse();
        }
        for index in indices {
            self.reset_visible |= self
                .draft
                .reorder(index, index.checked_add_signed(delta).unwrap());
        }
        self.focus.focus(window, cx);
        self.changed(cx);
    }
}
fn original(item: &Item) -> &str {
    item.message.as_deref().unwrap_or(&item.entry.subject)
}

mod drag;
mod graph;
mod pointer;
#[cfg(all(test, feature = "desktop-tests"))]
mod tests;
mod view;
