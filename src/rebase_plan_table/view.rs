//! Rendering and input events for the operation-owned plan draft.
use super::*;

impl PlanTable {
    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let indices = self.indices();
        let busy = self.editing();
        let unite = !busy && self.model().unite_selection(&indices).is_some();
        let reword = !busy && indices.len() == 1 && self.model().can_reword(indices[0]);
        let owner = cx.entity().downgrade();
        h_flex()
            .h(px(30.))
            .gap(px(3.))
            .px(px(4.))
            .border_b_1()
            .border_color(rgb(crate::ui_theme::palette(cx).border))
            .child(
                Button::new("plan-up")
                    .small()
                    .ghost()
                    .icon(Icon::default().path("jb/up_dark.svg"))
                    .tooltip(t("Move up"))
                    .disabled(!self.can_move(-1))
                    .on_click(cx.listener(|s, _, w, cx| s.move_rows(-1, w, cx))),
            )
            .child(
                Button::new("plan-down")
                    .small()
                    .ghost()
                    .icon(Icon::default().path("jb/down_dark.svg"))
                    .tooltip(t("Move down"))
                    .disabled(!self.can_move(1))
                    .on_click(cx.listener(|s, _, w, cx| s.move_rows(1, w, cx))),
            )
            .child(
                Button::new("plan-pick")
                    .small()
                    .ghost()
                    .icon(Icon::default().path("jb/rollback_dark.svg"))
                    .tooltip(t("Pick"))
                    .disabled(busy || !self.model().can_set(&indices, &Action::Pick))
                    .on_click(cx.listener(|s, _, _, cx| s.set_action(Action::Pick, cx))),
            )
            .child(
                Button::new("plan-edit")
                    .small()
                    .ghost()
                    .icon(Icon::default().path("jb/pause_dark.svg"))
                    .tooltip(t("Edit"))
                    .disabled(busy || !self.model().can_set(&indices, &Action::Edit))
                    .on_click(cx.listener(|s, _, _, cx| s.set_action(Action::Edit, cx))),
            )
            .child(
                Button::new("plan-reword")
                    .small()
                    .label(t("Reword"))
                    .disabled(!reword)
                    .on_click(cx.listener(|s, _, w, cx| {
                        if let Some(id) = s.selected.first().copied() {
                            s.reword_action(id, w, cx);
                        }
                    })),
            )
            .child(
                h_flex()
                    .child(
                        Button::new("plan-squash")
                            .small()
                            .label(t("Squash"))
                            .disabled(!unite)
                            .on_click(cx.listener(|s, _, w, cx| s.unite(true, w, cx))),
                    )
                    .child(
                        Button::new("plan-squash-options")
                            .small()
                            .label("▾")
                            .disabled(!unite)
                            .dropdown_menu(move |menu, _, _| {
                                let owner = owner.clone();
                                menu.item(PopupMenuItem::element(|_, _| t("Fixup")).on_click(
                                    move |_, w, cx| {
                                        let _ = owner.update(cx, |s, cx| s.unite(false, w, cx));
                                    },
                                ))
                            }),
                    ),
            )
            .child(
                Button::new("plan-drop")
                    .small()
                    .label(t("Drop"))
                    .disabled(busy || !self.model().can_set(&indices, &Action::Drop))
                    .on_click(cx.listener(|s, _, _, cx| s.set_action(Action::Drop, cx))),
            )
            .child(div().flex_1())
            .when(self.reset_visible, |v| {
                v.child(
                    Button::new("plan-reset")
                        .small()
                        .link()
                        .label(t("Reset"))
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.dismiss_editor(true, w, cx);
                        })),
                )
            })
    }
    fn row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = &self.model().rows()[index];
        let editing = self.editor.as_ref().filter(|e| e.row == row.id);
        let selected = self.selected.contains(&row.id);
        let color = match row.action() {
            Action::Reword(_) => 0x6b9bfa,
            _ => crate::ui_theme::palette(cx).foreground,
        };
        // Git's todo may prefix the subject with "#" (rebase.instructionFormat).
        // GitRebaseCommitsTableModel.getPresentation uses full commit metadata,
        // never the todo's display annotation as the commit message.
        let subject = match row.action() {
            Action::Reword(message) => message.lines().next().unwrap_or("").to_owned(),
            _ => row
                .entry
                .message
                .as_deref()
                .map(|message| message.lines().next().unwrap_or("").to_owned())
                .unwrap_or_else(|| {
                    plico::interactive_rebase::todo::render(std::slice::from_ref(&row.entry.entry))
                        .unwrap_or_else(|_| row.entry.entry.target.clone())
                        .trim_end()
                        .to_owned()
                }),
        };
        let element = h_flex()
            .id(("rebase-row", index))
            .w_full()
            .items_start()
            .min_h(px(22.))
            .pr(px(6.))
            .when(selected && editing.is_none(), |v| {
                v.bg(rgb(crate::ui_theme::palette(cx).selection))
            })
            .relative()
            .pl(px(if row.parent().is_some() { 44. } else { 24. }))
            .child(self.graph(index, selected && editing.is_none(), cx))
            .when_some(self.drag_line(index), |v, below| {
                v.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .h(px(2.))
                        .when(below, |v| v.bottom_0())
                        .when(!below, |v| v.top_0())
                        .bg(rgb(0x3574f0)),
                )
            })
            .child(if let Some(editor) = editing {
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        Textarea::new(&editor.input)
                            .external_focus_ring(false)
                            .h(px(106.)),
                    )
                    .child(
                        div()
                            .px(px(8.))
                            .text_color(rgb(crate::ui_theme::palette(cx).muted))
                            .child(t("Press Ctrl+Enter to apply changes")),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_color(rgb(color))
                    .child({
                        let len = subject.len();
                        StyledText::new(subject).with_highlights(
                            matches!(row.action(), Action::Drop).then_some((
                                0..len,
                                HighlightStyle {
                                    strikethrough: Some(StrikethroughStyle {
                                        thickness: px(1.),
                                        color: None,
                                    }),
                                    ..Default::default()
                                },
                            )),
                        )
                    })
                    .into_any_element()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, e: &MouseDownEvent, w, cx| {
                    if s.editor
                        .as_ref()
                        .is_some_and(|edit| s.model().index(edit.row) == Some(index))
                    {
                        return;
                    }
                    s.select_with_mouse(index, e, w, cx);
                }),
            );
        self.drag_row(index, element, cx).into_any_element()
    }
}
impl Focusable for PlanTable {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for PlanTable {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        v_flex()
            .size_full()
            .relative()
            .min_w_0()
            .bg(rgb(crate::ui_theme::palette(cx).background))
            .track_focus(&self.focus)
            .child(self.toolbar(cx))
            .child(
                list(self.list.clone(), move |index, _, cx| {
                    owner
                        .update(cx, |s, cx| s.row(index, cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .flex_1()
                .min_h_0(),
            )
            .when(self.pending_mouse_selection.is_some(), |v| {
                v.child(self.mouse_selection_after_layout(cx))
            })
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    div()
                        .text_color(rgb(crate::ui_theme::color(cx, 0xf2a6a6, 0xc54e58)))
                        .child(error),
                )
            })
            .capture_action(
                cx.listener(|s, event: &gpui_kit::component::input::Enter, w, cx| {
                    if event.secondary && s.editing() {
                        s.apply_message(w, cx);
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }),
            )
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                let key = &e.keystroke.key;
                let mods = e.keystroke.modifiers;
                if key == "escape" && s.cancel_drag(w, cx) {
                    cx.stop_propagation();
                    return;
                }
                if s.editing() {
                    if key == "enter" && mods.control {
                        s.apply_message(w, cx);
                        cx.stop_propagation();
                    } else if key == "escape" {
                        s.dismiss_editor(false, w, cx);
                        cx.stop_propagation();
                    }
                    return;
                }
                if mods.control && key == "z" {
                    if mods.shift {
                        s.reset_visible |= s.draft.redo();
                    } else {
                        s.reset_visible |= s.draft.undo();
                    }
                    s.changed(cx);
                    cx.stop_propagation();
                } else if mods.control && key == "y" {
                    s.reset_visible |= s.draft.redo();
                    s.changed(cx);
                    cx.stop_propagation();
                } else if (key == "up" || key == "down") && !mods.control && !mods.alt {
                    let indices = s.indices();
                    let current = if key == "up" {
                        indices.first()
                    } else {
                        indices.last()
                    }
                    .copied()
                    .unwrap_or(0);
                    let next = if key == "up" {
                        current.saturating_sub(1)
                    } else {
                        (current + 1).min(s.model().rows().len().saturating_sub(1))
                    };
                    if !s.model().rows().is_empty() {
                        s.select(next, mods, w, cx);
                        s.list.scroll_to_reveal_item(next);
                    }
                    cx.stop_propagation();
                } else if mods.alt {
                    match key.as_str() {
                        "p" => s.set_action(Action::Pick, cx),
                        "e" => s.set_action(Action::Edit, cx),
                        "d" => s.set_action(Action::Drop, cx),
                        "s" => s.unite(true, w, cx),
                        "f" => s.unite(false, w, cx),
                        "r" if s.selected.len() == 1 => s.reword_action(s.selected[0], w, cx),
                        _ => return,
                    }
                    cx.stop_propagation();
                } else if key == "f2" {
                    if let Some(id) = s
                        .selected
                        .first()
                        .copied()
                        .filter(|_| s.selected.len() == 1)
                    {
                        s.reword_action(id, w, cx);
                    }
                    cx.stop_propagation();
                }
            }))
    }
}
