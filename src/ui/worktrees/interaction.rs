//! Selection-bound Worktrees popup actions; all writes use the existing confirmation flow.
use super::*;
use gpui_kit::component::menu::PopupMenu;

impl Browser {
    pub(super) fn move_selection(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.trees.is_empty() {
            return;
        }
        let current = self
            .trees
            .iter()
            .position(|tree| Some(&tree.path) == self.selected.as_ref());
        let index = match current {
            Some(index) if forward => (index + 1).min(self.trees.len() - 1),
            Some(index) => index.saturating_sub(1),
            None => 0,
        };
        self.selected = Some(self.trees[index].path.clone());
        self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        cx.notify();
    }

    pub(super) fn can_remove(&self, cx: &App) -> bool {
        self.owner
            .upgrade()
            .is_some_and(|owner| !owner.read(cx).operation.busy())
            && self
                .selected()
                .is_some_and(|tree| !tree.main && !tree.current && !tree.bare)
    }

    pub(super) fn remove(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !self.can_remove(cx) {
            return;
        }
        if let Some(tree) = self.selected().cloned() {
            let _ = self
                .owner
                .update(cx, |s, cx| s.confirm_remove_worktree(tree, w, cx));
        }
    }

    pub(super) fn menu(
        owner: &WeakEntity<Self>,
        path: &std::path::Path,
        menu: PopupMenu,
        w: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let enabled = owner
            .update(cx, |s, cx| {
                if !s.trees.iter().any(|tree| tree.path == path) {
                    return (false, false);
                }
                s.selected = Some(path.to_owned());
                s.focus.focus(w, cx);
                cx.notify();
                let busy = s
                    .owner
                    .upgrade()
                    .is_none_or(|owner| owner.read(cx).operation.busy());
                (
                    !busy
                        && s.selected()
                            .is_some_and(|tree| !tree.bare && !tree.current && !tree.prunable),
                    s.can_remove(cx),
                )
            })
            .unwrap_or((false, false));
        let open_owner = owner.clone();
        let open_path = path.to_owned();
        let remove_owner = owner.clone();
        let remove_path = path.to_owned();
        menu.item(
            PopupMenuItem::element(move |_, cx| {
                h_flex()
                    .w_full()
                    .gap_6()
                    .child(t("Open as a Project"))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_color(rgb(crate::ui_theme::palette(cx).muted))
                            .child(t("Double Click")),
                    )
            })
            .disabled(!enabled.0)
            .on_click(move |_, w, cx| {
                let _ = open_owner.update(cx, |s, cx| {
                    if s.trees.iter().any(|tree| tree.path == open_path) {
                        s.selected = Some(open_path.clone());
                        s.open(w, cx);
                    }
                });
            }),
        )
        .separator()
        .item(
            PopupMenuItem::element(move |_, cx| {
                h_flex()
                    .w_full()
                    .gap_6()
                    .child(t("Delete…"))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_color(rgb(crate::ui_theme::palette(cx).muted))
                            .child("Delete"),
                    )
            })
            .icon(jb_icon("delete_dark.svg"))
            .disabled(!enabled.1)
            .on_click(move |_, w, cx| {
                let _ = remove_owner.update(cx, |s, cx| {
                    if s.trees.iter().any(|tree| tree.path == remove_path) {
                        s.selected = Some(remove_path.clone());
                        s.remove(w, cx);
                    }
                });
            }),
        )
    }
}
