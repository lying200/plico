//! Selection-bound Worktrees popup actions; all writes use the existing confirmation flow.
use super::*;
use gpui_kit::component::menu::PopupMenu;

impl Browser {
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
            PopupMenuItem::new(t("Open as a Project"))
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
            PopupMenuItem::new(t("Delete…"))
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
