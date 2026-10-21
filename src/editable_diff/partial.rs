use super::*;
use gpui_kit::component::{
    Disableable, Sizable,
    button::ButtonVariants,
    menu::{ContextMenuExt, PopupMenuItem},
};
use plico::{
    commit_inclusion::Identity,
    i18n::t,
    partial_selection::{Included, Selection},
};
impl EditableDiff {
    pub fn set_commit_scope(
        &mut self,
        scope: Entity<crate::commit_scope::Scope>,
        id: Identity,
        cx: &mut Context<Self>,
    ) {
        cx.observe(&scope, |_, _, cx| cx.notify()).detach();
        self.commit_scope = Some((scope, id));
        self.recompare(cx);
        cx.notify();
    }
    pub(super) fn partial_selection(&self, cx: &App) -> Option<Selection> {
        let (scope, id) = self.commit_scope.as_ref()?;
        Some(
            scope
                .read(cx)
                .selection(id, self.partial_template.as_ref()?),
        )
    }
    fn set_selected_lines(&mut self, original: bool, included: bool, cx: &mut Context<Self>) {
        if self.documents.locked() {
            return;
        }
        let Some(mut selection) = self.partial_selection(cx) else {
            return;
        };
        let editor = if original {
            &self.original
        } else {
            &self.modified
        };
        let e = editor.read(cx);
        let r = e.selected_range();
        let a = e.text().offset_to_position(r.start);
        let b = e.text().offset_to_position(r.end);
        let end = b.line as usize + usize::from(r.is_empty() || b.character > 0);
        selection.set_selected_lines(
            original,
            a.line as usize..end.max(a.line as usize + 1),
            included,
        );
        let (scope, id) = self.commit_scope.as_ref().unwrap();
        scope.update(cx, |s, cx| s.change(id, selection, cx));
    }
    pub fn inclusion_counts(&self, cx: &App) -> Option<(usize, usize)> {
        self.partial_selection(cx).map(|s| s.visible_counts())
    }
    pub fn inclusion_checkbox(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let selection = self.partial_selection(cx)?;
        let (n, total) = selection.counts();
        if total == 0 {
            return None;
        }
        let icon = if n == total {
            "jb/gutterCheckBoxSelected.svg"
        } else if n == 0 {
            "jb/gutterCheckBox.svg"
        } else {
            "jb/gutterCheckBoxIndeterminate.svg"
        };
        Some(
            gpui_kit::component::button::Button::new("diff-include-file")
                .ghost()
                .small()
                .h(px(18.))
                .w(px(18.))
                .icon(
                    gpui_kit::component::Icon::default()
                        .path(icon)
                        .size(px(14.)),
                )
                .tooltip(t(if n == total {
                    "Exclude from commit"
                } else {
                    "Include in commit"
                }))
                .disabled(self.documents.locked())
                .on_click(cx.listener(|s, _, _, cx| {
                    if s.documents.locked() {
                        return;
                    }
                    let Some(mut selection) = s.partial_selection(cx) else {
                        return;
                    };
                    let (n, total) = selection.counts();
                    selection.set_all(n != total);
                    let (scope, id) = s.commit_scope.as_ref().unwrap();
                    scope.update(cx, |s, cx| s.change(id, selection, cx));
                }))
                .into_any_element(),
        )
    }
    fn selected_actions(&self, original: bool, cx: &App) -> (bool, bool) {
        if self.documents.locked() {
            return (false, false);
        }
        let Some(selection) = self.partial_selection(cx) else {
            return (false, false);
        };
        let e = if original {
            &self.original
        } else {
            &self.modified
        }
        .read(cx);
        let r = e.selected_range();
        let a = e.text().offset_to_position(r.start);
        let b = e.text().offset_to_position(r.end);
        let end = b.line as usize + usize::from(r.is_empty() || b.character > 0);
        selection.selected_actions(original, a.line as usize..end.max(a.line as usize + 1))
    }
    pub(super) fn selection_menu(
        &self,
        element: impl IntoElement,
        original: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let enabled = self.commit_scope.is_some() && !self.documents.locked();
        let view = div().size_full().child(element);
        if !enabled {
            return view.into_any_element();
        }
        view.context_menu(move |menu, _, cx| {
            let (can_include, can_exclude) = owner
                .upgrade()
                .map(|v| v.read(cx).selected_actions(original, cx))
                .unwrap_or_default();
            let include = owner.clone();
            let exclude = owner.clone();
            menu.item(
                PopupMenuItem::new(t("Include selected lines in commit"))
                    .disabled(!can_include)
                    .on_click(move |_, _, cx| {
                        let _ =
                            include.update(cx, |s, cx| s.set_selected_lines(original, true, cx));
                    }),
            )
            .item(
                PopupMenuItem::new(t("Exclude selected lines from commit"))
                    .disabled(!can_exclude)
                    .on_click(move |_, _, cx| {
                        let _ =
                            exclude.update(cx, |s, cx| s.set_selected_lines(original, false, cx));
                    }),
            )
        })
        .into_any_element()
    }
    pub(super) fn partial_gutter(&self, cx: &Context<Self>) -> AnyElement {
        let Some(selection) = self.partial_selection(cx) else {
            return div().into_any_element();
        };
        let original = self.original.clone();
        let modified = self.modified.clone();
        let ratio = self.split_ratio;
        let owner = cx.entity().downgrade();
        let locked = self.documents.locked();
        canvas(
            move |bounds, window, cx| {
                let x = bounds.left() + bounds.size.width * ratio;
                let mut items = vec![];
                let mut add =
                    |line: usize, left: bool, block: usize, partial: bool, checked: bool| {
                        let editor = if left { &original } else { &modified };
                        let Some((y, _)) = editor.read(cx).source_line_extent(line..line) else {
                            return;
                        };
                        if y < bounds.top() || y + px(16.) > bounds.bottom() {
                            return;
                        }
                        let rect = Bounds::new(
                            point(x + if left { px(-20.) } else { px(4.) }, y),
                            size(px(16.), px(16.)),
                        );
                        let hit =
                            (!locked).then(|| window.insert_hitbox(rect, HitboxBehavior::Normal));
                        items.push((rect, hit, line, left, block, partial, checked));
                    };
                for (i, (block, state)) in
                    selection.blocks.iter().zip(&selection.included).enumerate()
                {
                    match state {
                        Included::All(checked) => {
                            add(block.current.start, false, i, false, *checked)
                        }
                        Included::Lines { deleted, added } => {
                            for (line, v) in deleted.iter().enumerate() {
                                add(block.original.start + line, true, i, true, *v);
                            }
                            for (line, v) in added.iter().enumerate() {
                                add(block.current.start + line, false, i, true, *v);
                            }
                        }
                    }
                }
                (items, selection.clone())
            },
            move |_, (items, selection), window, cx| {
                for (rect, hit, _, _, _, _, checked) in &items {
                    let icon = if *checked {
                        "jb/gutterCheckBoxSelected.svg"
                    } else {
                        "jb/gutterCheckBox.svg"
                    };
                    let bounds =
                        Bounds::new(rect.origin + point(px(1.), px(1.)), size(px(14.), px(14.)));
                    let _ = window.paint_svg(
                        bounds,
                        icon.into(),
                        None,
                        TransformationMatrix::unit(),
                        rgb(0xced0d6).into(),
                        cx,
                    );
                    if let Some(hit) = hit {
                        window.set_cursor_style(CursorStyle::PointingHand, hit);
                    }
                }
                let owner = owner.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if !phase.bubble() || event.button != MouseButton::Left {
                        return;
                    }
                    for (_, hit, line, left, index, partial, checked) in &items {
                        if hit.as_ref().is_some_and(|h| h.is_hovered(window)) {
                            let _ = owner.update(cx, |s, cx| {
                                if s.documents.locked()
                                    || s.baseline != selection.original
                                    || s.modified.read(cx).text().to_string()
                                        != selection.current.as_ref()
                                {
                                    return;
                                }
                                let Some(mut value) = s.partial_selection(cx) else {
                                    return;
                                };
                                if *partial {
                                    value.set_lines(*left, *line..*line + 1, !checked);
                                } else {
                                    value.toggle_block(*index);
                                }
                                let (scope, id) = s.commit_scope.as_ref().unwrap();
                                scope.update(cx, |s, cx| s.change(id, value, cx));
                            });
                            cx.stop_propagation();
                            break;
                        }
                    }
                });
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::diff::DiffFile;
    struct Host(Entity<EditableDiff>);
    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let checkbox = self.0.update(cx, |v, cx| v.inclusion_checkbox(cx));
            v_flex()
                .size_full()
                .child(div().h(px(24.)).children(checkbox))
                .child(div().flex_1().min_h_0().child(self.0.clone()))
        }
    }
    #[gpui::test]
    fn title_checkbox_click_updates_shared_scope_and_toggles_back(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut scope = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let file = plico::commit_changes::File::Tracked(plico::git::WorkingTreeChange {
                path: "file.txt".into(),
                old_path: None,
                status: "M".into(),
            });
            let id = Identity::of(&file);
            let state = cx.new(|_| crate::commit_scope::Scope::default());
            state.update(cx, |s, cx| {
                s.replace(
                    &plico::commit_changes::Snapshot {
                        head: Some("a".repeat(40)),
                        files: vec![file],
                        review_versions: vec![],
                    },
                    cx,
                )
            });
            let doc =
                DiffFile::from_sources(Some(("file.txt", "old\n")), Some(("file.txt", "new\n")))
                    .unwrap();
            let view = cx.new(|cx| {
                EditableDiff::snapshot(&doc, std::path::Path::new("file.txt"), window, cx)
            });
            view.update(cx, |v, cx| {
                v.partial_template = Some(Selection::new("old\n".into(), "new\n".into(), false));
                v.commit_scope = Some((state.clone(), id));
                cx.observe(&state, |_, _, cx| cx.notify()).detach();
            });
            scope = Some(state);
            let host = cx.new(|cx| {
                cx.observe(&view, |_, _, cx| cx.notify()).detach();
                Host(view)
            });
            gpui::component::Root::new(host, window, cx)
        });
        let scope = scope.unwrap();
        for expected in [1, 0] {
            cx.run_until_parked();
            cx.update(|w, cx| {
                let _ = w.draw(cx);
            });
            cx.simulate_click(point(px(9.), px(9.)), Modifiers::default());
            cx.run_until_parked();
            assert_eq!(scope.read_with(cx, |s, _| s.inclusion.count()), expected);
        }
    }
}
