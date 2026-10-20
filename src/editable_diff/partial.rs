use super::*;
use gpui_kit::component::{
    Disableable,
    checkbox::Checkbox,
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
    pub(super) fn partial_header(&self, cx: &Context<Self>) -> AnyElement {
        let Some(selection) = self.partial_selection(cx) else {
            return div().into_any_element();
        };
        let (n, total) = selection.counts();
        h_flex()
            .justify_end()
            .px_2()
            .h(px(25.))
            .child(
                Checkbox::new("diff-include-file")
                    .label(t("Include in commit"))
                    .checked(total > 0 && n == total)
                    .indeterminate(n > 0 && n < total)
                    .disabled(self.documents.locked() || total == 0)
                    .on_click(cx.listener(|s, checked: &bool, _, cx| {
                        let Some(mut selection) = s.partial_selection(cx) else {
                            return;
                        };
                        selection.set_all(*checked);
                        let (scope, id) = s.commit_scope.as_ref().unwrap();
                        scope.update(cx, |s, cx| s.change(id, selection, cx));
                    })),
            )
            .into_any_element()
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
        view.context_menu(move |menu, _, _| {
            let include = owner.clone();
            let exclude = owner.clone();
            menu.item(
                PopupMenuItem::new(t("Include selected lines in commit")).on_click(
                    move |_, _, cx| {
                        let _ =
                            include.update(cx, |s, cx| s.set_selected_lines(original, true, cx));
                    },
                ),
            )
            .item(
                PopupMenuItem::new(t("Exclude selected lines from commit")).on_click(
                    move |_, _, cx| {
                        let _ =
                            exclude.update(cx, |s, cx| s.set_selected_lines(original, false, cx));
                    },
                ),
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
                    window.paint_quad(fill(*rect, rgb(if *checked { 0x3574f0 } else { 0x6f737a })));
                    let inner = Bounds::new(
                        rect.origin + point(px(2.), px(2.)),
                        rect.size - size(px(4.), px(4.)),
                    );
                    window.paint_quad(fill(inner, rgb(if *checked { 0x3574f0 } else { 0x242628 })));
                    if *checked {
                        let text: SharedString = "✓".into();
                        let shaped = window.text_system().shape_line(
                            text.clone(),
                            px(14.),
                            &[TextRun {
                                len: text.len(),
                                font: gpui_kit::font("Segoe UI"),
                                color: rgb(0xffffff).into(),
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            }],
                            None,
                        );
                        let _ =
                            shaped.paint(rect.origin, px(16.), TextAlign::Left, None, window, cx);
                    }
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
