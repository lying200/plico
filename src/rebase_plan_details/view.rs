//! Rebased changes browser / commit metadata split presentation.
use super::*;
impl Details {
    fn file_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(row) = self.rows.get(index) else {
            return div().into_any_element();
        };
        let row_index = index;
        match row {
            Row::Folder {
                path,
                label,
                depth,
                count,
                expanded,
            } => {
                let path = path.clone();
                let toggle_path = path.clone();
                let root = path.as_os_str().is_empty();
                h_flex()
                    .id(("plan-folder", index))
                    .w_full()
                    .h(px(22.))
                    .when(
                        self.selection.contains(&super::navigation::Key::of(row)),
                        |v| v.bg(rgb(crate::ui_theme::palette(cx).selection)),
                    )
                    .gap(px(4.))
                    .pl(px(6. + *depth as f32 * 19.))
                    .child(
                        div()
                            .id(("plan-folder-toggle", index))
                            .flex_shrink_0()
                            .child(
                                Icon::default()
                                    .path(if *expanded {
                                        "jb/chevronDown_dark.svg"
                                    } else {
                                        "jb/chevronRight_dark.svg"
                                    })
                                    .size(px(12.)),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |s, _, w, cx| {
                                    s.files_focus.focus(w, cx);
                                    if !s.collapsed.remove(&toggle_path) {
                                        s.collapsed.insert(toggle_path.clone());
                                    }
                                    s.rebuild();
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        Icon::default()
                            .path(crate::ui_style::themed_asset("jb/folder_dark_4x.svg", cx))
                            .size(px(16.)),
                    )
                    .when(!root, |v| v.child(label.to_string_lossy().into_owned()))
                    .child(
                        div()
                            .text_color(rgb(crate::ui_theme::palette(cx).muted))
                            .child(crate::ui_style::file_count(*count)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |s, e: &MouseDownEvent, w, cx| {
                            s.files_focus.focus(w, cx);
                            s.select_row(
                                index,
                                e.modifiers.control || e.modifiers.platform,
                                e.modifiers.shift,
                            );
                            cx.notify();
                            if e.click_count == 2 {
                                s.show_diff(w, cx);
                            }
                        }),
                    )
                    .into_any_element()
            }
            Row::File { index, depth } => {
                let i = *index;
                let file = &self.data.as_ref().unwrap().files[i];
                let path = &file.after.as_ref().or(file.before.as_ref()).unwrap().path;
                let color = if file.before.is_none() {
                    crate::ui_theme::color(cx, 0x6aab73, 0x0a7700)
                } else if file.after.is_none() {
                    crate::ui_theme::color(cx, 0xcf8e6d, 0x616161)
                } else {
                    crate::ui_theme::color(cx, 0x6897bb, 0x0032a0)
                };
                h_flex()
                    .id(("plan-file", i))
                    .w_full()
                    .h(px(22.))
                    .gap(px(4.))
                    .pl(px(24. + *depth as f32 * 19.))
                    .pr(px(6.))
                    .overflow_hidden()
                    .when(
                        self.selection.contains(&super::navigation::Key::File(i)),
                        |v| v.bg(rgb(crate::ui_theme::palette(cx).selection)),
                    )
                    .child(
                        Icon::default()
                            .path(crate::ui_style::file_icon(path, cx))
                            .size(px(16.))
                            .flex_shrink_0(),
                    )
                    .child(
                        div().flex_shrink_0().text_color(rgb(color)).child(
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                        ),
                    )
                    .when(!self.grouped, |v| {
                        v.child(
                            div()
                                .text_color(rgb(crate::ui_theme::palette(cx).muted))
                                .child(
                                    path.parent()
                                        .unwrap_or(std::path::Path::new(""))
                                        .to_string_lossy()
                                        .into_owned(),
                                ),
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |s, e: &MouseDownEvent, w, cx| {
                            s.files_focus.focus(w, cx);
                            s.select_row(
                                row_index,
                                e.modifiers.control || e.modifiers.platform,
                                e.modifiers.shift,
                            );
                            cx.notify();
                            if e.click_count == 2 {
                                s.show_diff(w, cx);
                            }
                        }),
                    )
                    .into_any_element()
            }
        }
    }
}
impl Render for Details {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let toolbar = h_flex()
            .h(px(30.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(crate::ui_theme::palette(cx).border))
            .px(px(4.))
            .gap(px(3.))
            .child(
                Button::new("rebase-show-diff")
                    .ghost()
                    .size(px(24.))
                    .icon(Icon::default().path("jb/diff_dark.svg"))
                    .tooltip(t("Show Diff"))
                    .disabled(self.data.as_ref().is_none_or(|data| data.files.is_empty()))
                    .on_click(cx.listener(|s, _, w, cx| s.show_diff(w, cx))),
            )
            .child(
                Button::new("rebase-group-files")
                    .ghost()
                    .size(px(24.))
                    .icon(Icon::default().path("jb/history-show_dark.svg"))
                    .tooltip(t("View Options"))
                    .dropdown_menu({
                        let owner = cx.entity().downgrade();
                        let checked = self.grouped;
                        move |menu, _, _| {
                            let owner = owner.clone();
                            menu.item(PopupMenuItem::label(t("Group By"))).item(
                                PopupMenuItem::new(t("Directory"))
                                    .checked(checked)
                                    .on_click(move |_, _, cx| {
                                        let _ = owner.update(cx, |s, cx| {
                                            s.toggle_grouping(cx);
                                        });
                                    }),
                            )
                        }
                    }),
            )
            .child(div().flex_1())
            .child(
                Button::new("rebase-expand-files")
                    .ghost()
                    .size(px(24.))
                    .icon(Icon::default().path("jb/expandAll_dark.svg"))
                    .tooltip(t("Expand All"))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.collapsed.clear();
                        s.rebuild();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("rebase-collapse-files")
                    .ghost()
                    .size(px(24.))
                    .icon(Icon::default().path("jb/collapseAll_dark.svg"))
                    .tooltip(t("Collapse All"))
                    .on_click(cx.listener(|s, _, _, cx| {
                        if let Some(data) = &s.data {
                            s.collapsed.insert(PathBuf::new());
                            let root = super::navigation::Key::Folder(PathBuf::new());
                            s.cursor = Some(root.clone());
                            s.anchor = Some(root.clone());
                            s.selection = HashSet::from([root]);
                            for file in &data.files {
                                if let Some(r) = file.after.as_ref().or(file.before.as_ref()) {
                                    for p in r
                                        .path
                                        .ancestors()
                                        .skip(1)
                                        .filter(|p| !p.as_os_str().is_empty())
                                    {
                                        s.collapsed.insert(p.to_path_buf());
                                    }
                                }
                            }
                        }
                        s.rebuild();
                        cx.notify();
                    })),
            );
        v_flex()
            .size_full()
            .min_w_0()
            .bg(rgb(crate::ui_theme::palette(cx).background))
            .text_color(rgb(crate::ui_theme::palette(cx).foreground))
            .child(
                v_resizable("rebase-metadata-split")
                    .with_state(&self.split)
                    .on_resize(cx.listener(|s, state: &Entity<ResizableState>, _, cx| {
                        if let [files, details] = state.read(cx).sizes().as_slice() {
                            let total = files.as_f32() + details.as_f32();
                            if total > 0. {
                                s.fraction = (files.as_f32() / total).clamp(0.1, 0.9);
                                if let Err(error) =
                                    plico::layout_preferences::RebaseSplit::Details.save(s.fraction)
                                {
                                    eprintln!("Cannot save rebase details split: {error}");
                                }
                            }
                        }
                    }))
                    .child(
                        resizable_panel()
                            .flex_basis(relative(self.fraction))
                            .size_range(px(80.)..px(2000.))
                            .child(
                                v_flex()
                                    .size_full()
                                    .min_h_0()
                                    .child(toolbar)
                                    .when(self.loading, |v| {
                                        v.child(
                                            div()
                                                .p_2()
                                                .text_color(rgb(crate::ui_theme::palette(cx).muted))
                                                .child(t("Loading…")),
                                        )
                                    })
                                    .when_some(self.error.clone(), |v, error| {
                                        v.child(crate::error_ui::notice(
                                            "rebase-details-error",
                                            "Could not load commit details",
                                            error,
                                            self.loading,
                                            cx.listener(|s, _, _, cx| s.reload(cx)),
                                        ))
                                    })
                                    .child(
                                        uniform_list(
                                            "rebase-files",
                                            self.rows.len(),
                                            cx.processor(
                                                |s, range: std::ops::Range<usize>, _, cx| {
                                                    range
                                                        .map(|i| s.file_row(i, cx))
                                                        .collect::<Vec<_>>()
                                                },
                                            ),
                                        )
                                        .flex_1()
                                        .min_h_0()
                                        .track_focus(&self.files_focus)
                                        .track_scroll(&self.scroll)
                                        .on_key_down(cx.listener(|s, e: &KeyDownEvent, _, cx| {
                                            let modifiers = e.keystroke.modifiers;
                                            let handled = if modifiers == Modifiers::default() {
                                                s.navigate(&e.keystroke.key)
                                            } else if modifiers.shift
                                                && !modifiers.control
                                                && !modifiers.platform
                                                && !modifiers.alt
                                            {
                                                s.extend_selection(&e.keystroke.key)
                                            } else {
                                                false
                                            };
                                            if handled {
                                                cx.stop_propagation();
                                                cx.notify();
                                            }
                                        }))
                                        .on_action(cx.listener(
                                            |s, _: &gpui_kit::component::dialog::Confirm, w, cx| {
                                                s.confirm_selection(w, cx);
                                                cx.stop_propagation();
                                            },
                                        )),
                                    ),
                            ),
                    )
                    .child(
                        resizable_panel()
                            .flex_basis(relative(1. - self.fraction))
                            .size_range(px(80.)..px(2000.))
                            .child(
                                div().size_full().child(
                                    list(self.cards.clone(), move |index, _, cx| {
                                        owner
                                            .update(cx, |s, _| {
                                                let Some(card) = s
                                                    .data
                                                    .as_ref()
                                                    .and_then(|d| d.cards.get(index))
                                                else {
                                                    return div().into_any_element();
                                                };
                                                v_flex()
                                                    .p(px(14.))
                                                    .gap(px(12.))
                                                    .min_w_0()
                                                    .child(
                                                        TextView::html(
                                                            format!("rebase-message-{}", card.id),
                                                            card.presentation.message.clone(),
                                                        )
                                                        .font_family("JetBrains Mono")
                                                        .text_size(rems(1.))
                                                        .selectable(true),
                                                    )
                                                    .child(
                                                        TextView::html(
                                                            format!("rebase-meta-{}", card.id),
                                                            card.presentation.metadata
                                                                [usize::from(i18n::is_chinese())]
                                                            .clone(),
                                                        )
                                                        .text_size(rems(1.))
                                                        .selectable(true),
                                                    )
                                                    .into_any_element()
                                            })
                                            .unwrap_or_else(|_| div().into_any_element())
                                    })
                                    .size_full(),
                                ),
                            ),
                    ),
            )
    }
}
