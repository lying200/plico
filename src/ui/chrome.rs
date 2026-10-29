//! Main-window chrome coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn main_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let disabled = self.operation.busy() || self.settings_open;
        titlebar_menu_button("main-menu")
            .ghost()
            .icon(jb_icon("windowsMenu@20x20_dark.svg"))
            .icon_size(px(16.))
            .tooltip(t("Main menu"))
            .accessibility_label(t("Main menu"))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, _, window, cx| {
                    s.main_menu_unshallow = s.unshallow_repository(window, cx);
                }),
            )
            .on_key_down(cx.listener(|s, _, window, cx| {
                s.main_menu_unshallow = s.unshallow_repository(window, cx);
            }))
            .dropdown_menu(move |menu, window, cx| {
                let owner = owner.clone();
                let unshallow = owner
                    .upgrade()
                    .and_then(|owner| owner.read(cx).main_menu_unshallow.clone());
                menu.submenu("Git", window, cx, move |mut menu, _, _| {
                    for (label, action) in [
                        ("Push", 0),
                        ("Update Project", 3),
                        ("Pull", 1),
                        ("Fetch", 2),
                        ("Unshallow repository", 10),
                        ("Git Remotes", 4),
                        ("Reflog", 6),
                        ("Worktrees", 9),
                        ("New Tag…", 7),
                        ("Clone Repository…", 5),
                        ("Create Git Repository…", 8),
                    ] {
                        if action == 10 && unshallow.is_none() {
                            continue;
                        }
                        let owner = owner.clone();
                        let unshallow = unshallow.clone();
                        menu = menu.item(PopupMenuItem::new(t(label)).disabled(disabled).on_click(
                            move |_, window, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    // Recheck after menu creation: background state can change.
                                    if s.operation.busy() || s.settings_open {
                                        return;
                                    }
                                    match action {
                                        0 => s.open_push(window, cx),
                                        1 => s.open_pull(window, cx),
                                        3 => s.open_project_update(window, cx),
                                        4 => s.open_remotes(window, cx),
                                        5 => s.open_clone(window, cx),
                                        6 => s.open_reflog(window, cx),
                                        7 => s.open_tag(window, cx),
                                        8 => s.open_init(window, cx),
                                        9 => s.open_worktrees(window, cx),
                                        10 => {
                                            if let Some(repo) = unshallow.clone() {
                                                s.start_unshallow(repo, window, cx);
                                            }
                                        }
                                        _ => s.start_fetch(window, cx),
                                    }
                                });
                            },
                        ));
                    }
                    menu
                })
            })
            .map(titlebar_menu)
    }
    pub(super) fn project_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let name = self
            .repo
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        titlebar_menu_button("project-menu")
            .ghost()
            .min_w_0()
            .flex_shrink_1()
            .max_w(px(340.))
            .disabled(self.operation.busy() || self.settings_open)
            .tooltip(
                self.repo
                    .path
                    .to_string_lossy()
                    .trim_start_matches("\\\\?\\")
                    .to_owned(),
            )
            .accessibility_label(format!("{}: {}", t("Project"), name))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    .child(project_avatar(&self.repo.path, 20.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(cx.theme().font_size)
                            .font_weight(FontWeight::NORMAL)
                            .child(name),
                    )
                    .child(
                        jb_icon("chevronDown_dark.svg")
                            .size(px(10.))
                            .flex_shrink_0(),
                    ),
            )
            .dropdown_menu(move |mut menu, _, cx| {
                let Some(view) = owner.upgrade() else {
                    return menu;
                };
                let state = view.read(cx);
                let disabled = state.operation.busy() || state.settings_open;
                let projects = state.recent_projects.clone();
                let current = state.repo.path.clone();
                let open_owner = owner.clone();
                menu = menu.min_w(px(280.)).max_w(px(360.)).item(
                    PopupMenuItem::new(t("Open…"))
                        .icon(jb_icon("open_dark.svg"))
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            let _ = open_owner.update(cx, |s, cx| {
                                if !s.operation.busy() && !s.settings_open {
                                    s.open(window, cx);
                                }
                            });
                        }),
                );
                let mut seen = HashSet::from([project_identity(&current)]);
                let recent = projects
                    .into_iter()
                    .filter(|path| seen.insert(project_identity(path)))
                    .collect::<Vec<_>>();
                for (i, path) in std::iter::once(current).chain(recent).enumerate() {
                    if i == 0 {
                        menu = menu.separator().label(t("Open Projects"));
                    }
                    if i == 1 {
                        menu = menu.separator().label(t("Recent Projects"));
                    }
                    let selected = i == 0;
                    let item_path = path.clone();
                    let select_owner = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, cx| {
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .gap_2()
                                .py(px(4.))
                                .items_start()
                                .child(project_avatar(&item_path, 20.))
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .flex_1()
                                        .font_weight(FontWeight::NORMAL)
                                        .child(
                                            div()
                                                .text_size(cx.theme().font_size)
                                                .line_height(px(18.))
                                                .truncate()
                                                .child(
                                                    item_path
                                                        .file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy()
                                                        .into_owned(),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .line_height(px(16.))
                                                .text_color(rgb(crate::ui_theme::palette(cx).muted))
                                                .truncate()
                                                .child(project_display_path(&item_path)),
                                        ),
                                )
                        })
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            let _ = select_owner.update(cx, |s, cx| {
                                if !selected && !s.operation.busy() && !s.settings_open {
                                    s.choose_project_window(path.clone(), window, cx);
                                }
                            });
                        }),
                    );
                }
                menu
            })
            .map(titlebar_menu)
    }
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h_full()
            .items_center()
            .flex_shrink_0()
            .text_size(cx.theme().font_size)
            .w_full()
            .pr_3()
            .gap_2()
            .bg(rgb(crate::ui_theme::palette(cx).shell))
            .child(
                div()
                    .w(px(48.))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(gpui_kit::img("branding/plico.svg").size(px(20.))),
            )
            .child(self.main_menu(cx))
            .child(self.project_menu(cx))
            .child(
                titlebar_button("update-project")
                    .ghost()
                    .icon(jb_icon("update_dark.svg"))
                    .icon_size(px(16.))
                    .tooltip(t("Update Project"))
                    .accessibility_label(t("Update Project"))
                    .disabled(self.operation.busy() || self.settings_open)
                    .on_click(cx.listener(|s, _, w, cx| s.open_project_update(w, cx))),
            )
            .child(
                titlebar_button("push")
                    .ghost()
                    .icon(jb_icon("push_dark.svg").size(px(16.)))
                    .icon_size(px(16.))
                    .tooltip(t("Push"))
                    .accessibility_label(t("Push"))
                    .disabled(self.operation.busy() || self.settings_open)
                    .on_click(cx.listener(|s, _, w, cx| s.open_push(w, cx))),
            )
            .child(self.repository_branch_menu(cx))
            .when(self.revert_saved_available, |v| {
                v.child(
                    titlebar_button("revert-saved-recovery")
                        .label(t("Saved Revert Changes"))
                        .disabled(self.operation.busy() || self.settings_open)
                        .on_click(cx.listener(|s, _, w, cx| s.open_revert_saved(w, cx))),
                )
            })
            .when(self.revert_recovery_available, |v| {
                v.child(
                    titlebar_button("saved-revert-queue")
                        .label(t("Saved Revert Queue"))
                        .disabled(self.operation.busy() || self.settings_open)
                        .on_click(cx.listener(|s, _, w, cx| s.open_revert_queue(w, cx))),
                )
            })
            .when(self.merge_recovery_available, |v| {
                v.child(
                    titlebar_button("merge-saved-recovery")
                        .label(t("Saved Merge Changes"))
                        .disabled(self.operation.busy() || self.settings_open)
                        .on_click(cx.listener(|s, _, w, cx| s.open_merge_recovery(w, cx))),
                )
            })
            .when_some(
                self.operation_state
                    .as_ref()
                    .filter(|s| {
                        matches!(
                            s.operation,
                            plico::operation_state::Operation::CherryPick
                                | plico::operation_state::Operation::Revert
                        )
                    })
                    .map(|s| (s.operation, !s.conflicts.is_empty())),
                |v, (kind, conflicts)| {
                    let pick = kind == plico::operation_state::Operation::CherryPick;
                    v.child(
                        titlebar_button("continue-sequence")
                            .primary()
                            .label(t(if pick {
                                "Continue Cherry-pick"
                            } else {
                                "Continue Revert"
                            }))
                            .disabled(conflicts || self.operation.busy() || self.settings_open)
                            .on_click(
                                cx.listener(move |s, _, w, cx| {
                                    s.start_sequence(kind, false, w, cx)
                                }),
                            ),
                    )
                    .child(
                        titlebar_button("abort-sequence")
                            .label(t(if pick {
                                "Abort Cherry-pick"
                            } else {
                                "Abort Revert"
                            }))
                            .disabled(self.operation.busy() || self.settings_open)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.confirm_abort_sequence(kind, w, cx)
                            })),
                    )
                },
            )
            .when(self.project_has_conflicts(cx), |v| {
                v.child(
                    titlebar_button("resolve-conflicts")
                        .label(t("Resolve Conflicts…"))
                        .disabled(self.operation.busy() || self.settings_open)
                        .on_click(cx.listener(|s, _, w, cx| s.open_conflicts(w, cx))),
                )
            })
            .when(!self.merge_roots(cx).is_empty(), |v| {
                v.when(!self.project_has_conflicts(cx), |v| {
                    v.child(
                        titlebar_button("complete-merge")
                            .primary()
                            .label(t("Complete Merge"))
                            .disabled(self.operation.busy() || self.settings_open)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.choose_merge_root(plico::merge_operation::Action::Commit, w, cx)
                            })),
                    )
                })
                .child(
                    titlebar_button("abort-merge")
                        .label(t("Abort Merge"))
                        .disabled(self.operation.busy() || self.settings_open)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.choose_merge_root(plico::merge_operation::Action::Abort, w, cx)
                        })),
                )
            })
            .when(!self.rebase_roots(cx).is_empty(), |v| {
                v.child(
                    titlebar_button("continue-rebase")
                        .primary()
                        .label(t("Continue Rebase"))
                        .disabled(self.operation.busy() || self.settings_open)
                        .on_click(cx.listener(|s, _, w, cx| s.choose_rebase_root(false, w, cx))),
                )
                .child(
                    titlebar_button("abort-rebase")
                        .label(t("Abort Rebase"))
                        .disabled(self.operation.busy() || self.settings_open)
                        .on_click(cx.listener(|s, _, w, cx| s.choose_rebase_root(true, w, cx))),
                )
            })
            .child(div().flex_1())
            .child(
                titlebar_button("settings")
                    .ghost()
                    .icon(jb_icon("settings_dark.svg").size(px(14.)))
                    .icon_size(px(14.))
                    .tooltip(format!("{} (Ctrl+Alt+S)", t("Settings")))
                    .accessibility_label(t("Settings"))
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.open_settings(w, cx);
                    })),
            )
    }
    pub(super) fn left_tool_windows(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(view) = &self.project_view {
            view.update(cx, |v, _| {
                v.set_project_showing(self.project_visible && !self.comparison_visible)
            });
        }
        if self.comparison_visible {
            return div()
                .size_full()
                .children(self.comparison.clone())
                .into_any_element();
        }
        if self.project_visible {
            let source = self
                .active_file_index()
                .and_then(|i| self.file_tabs.get(i))
                .map(|tab| tab.read(cx).path.clone())
                .or_else(|| {
                    self.active_diff_index()
                        .and_then(|i| self.tabs.get(i))
                        .map(|tab| tab.read(cx).target.change().path.clone())
                });
            if let Some(view) = &self.project_view {
                view.update(cx, |v, cx| v.set_current_source(source, cx));
            }
            return div()
                .size_full()
                .children(self.project_view.clone())
                .into_any_element();
        }
        v_flex()
            .size_full()
            .min_h_0()
            .bg(rgb(crate::ui_theme::palette(cx).panel))
            .child(
                h_flex()
                    .h(tab_height(cx) + px(8.))
                    .flex_shrink_0()
                    .px_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(crate::ui_theme::palette(cx).border))
                    .child(
                        islands_tab("commit-content-tab", self.local_visible, cx)
                            .child(t("Commit"))
                            .on_click(cx.listener(|s, _, w, cx| s.open_local(w, cx))),
                    )
                    .child(
                        islands_tab("stash-content-tab", self.stash_open, cx)
                            .child(t("Stash"))
                            .on_click(cx.listener(|s, _, w, cx| s.open_stashes(w, cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("commit-window-hide")
                            .ghost()
                            .small()
                            .icon(jb_icon("hideToolWindow_dark.svg"))
                            .tooltip(t("Hide"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.local_visible = false;
                                s.stash_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().flex_1().min_h_0().child(if self.local_visible {
                div()
                    .size_full()
                    .children(self.local_view.clone())
                    .into_any_element()
            } else {
                div()
                    .size_full()
                    .children(self.stash_view.clone())
                    .into_any_element()
            }))
            .into_any_element()
    }
    pub(super) fn git_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let log_menu_owner = cx.entity().downgrade();
        h_flex()
            .h(tab_height(cx) + px(8.))
            .flex_shrink_0()
            .px(px(11.))
            .gap(px(8.))
            .bg(rgb(crate::ui_theme::palette(cx).panel))
            .rounded_t(px(10.))
            .border_b_1()
            .border_color(rgb(crate::ui_theme::palette(cx).border))
            .child(
                islands_tab(
                    "log-tab",
                    !self.local
                        && self.active_diff_index().is_none()
                        && self.active_file_index().is_none()
                        && self.active_branch_compare_index().is_none(),
                    cx,
                )
                .child(jb_icon("vcs_dark.svg"))
                .child(t("Log"))
                .on_click(cx.listener(|s, _, w, cx| {
                    s.close_settings(cx);
                    s.leave_diff_tab(cx);
                    s.focus.focus(w, cx);
                    if s.local {
                        s.local = false;
                        s.refresh(false, cx);
                    }
                }))
                .context_menu(move |menu, _, cx| {
                    log_menu_owner
                        .update(cx, |s, cx| s.editor_tab_menu(menu, None, cx))
                        .unwrap()
                }),
            )
            .children(self.editor_tabs.order().iter().filter_map(|id| {
                if let Some((index, tab)) = self
                    .file_tabs
                    .iter()
                    .enumerate()
                    .find(|(_, tab)| tab.entity_id() == *id)
                {
                    let preview = tab.read(cx).preview;
                    let menu_owner = cx.entity().downgrade();
                    let tab_id = tab.entity_id();
                    let name = tab
                        .read(cx)
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    let name = if tab.read(cx).dirty(cx) {
                        format!("{name} •")
                    } else {
                        name
                    };
                    let name = if tab.read(cx).history {
                        format!("{}: {name}", t("History"))
                    } else {
                        name
                    };
                    Some(
                        islands_tab(
                            ("file-tab", index),
                            self.active_file_index() == Some(index),
                            cx,
                        )
                        .child(
                            img(file_icon(&tab.read(cx).path, cx))
                                .size(px(16.))
                                .flex_shrink_0(),
                        )
                        .child(
                            div()
                                .max_w(px(200.))
                                .truncate()
                                .when(preview, |d| d.italic())
                                .child(name),
                        )
                        .on_click(cx.listener(move |s, event: &ClickEvent, w, cx| {
                            let Some(index) =
                                s.file_tabs.iter().position(|tab| tab.entity_id() == tab_id)
                            else {
                                return;
                            };
                            s.cancel_project_preview(cx);
                            if event.click_count() == 2 {
                                s.file_tabs[index].update(cx, |tab, _| tab.preview = false);
                            }
                            if s.active_file_index() == Some(index) {
                                s.file_tabs[index].update(cx, |tab, cx| tab.focus_viewer(w, cx));
                            } else {
                                s.leave_diff_tab(cx);
                                s.editor_tabs.activate(s.file_tabs[index].entity_id());
                                s.file_tabs[index].update(cx, |tab, cx| tab.activate(cx));
                            }
                            cx.notify();
                        }))
                        .child(
                            Button::new(("close-file", index))
                                .ghost()
                                .small()
                                .w(px(16.))
                                .h(px(16.))
                                .icon(jb_icon("closeSmall.svg"))
                                .tooltip(t("Close file"))
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    cx.stop_propagation();
                                    if let Some(index) =
                                        s.file_tabs.iter().position(|tab| tab.entity_id() == tab_id)
                                    {
                                        s.close_file_tab(index, w, cx);
                                    }
                                })),
                        )
                        .context_menu(move |menu, _, cx| {
                            menu_owner
                                .update(cx, |s, cx| s.editor_tab_menu(menu, Some(tab_id), cx))
                                .unwrap()
                        }),
                    )
                } else if let Some((index, tab)) = self
                    .tabs
                    .iter()
                    .enumerate()
                    .find(|(_, tab)| tab.entity_id() == *id)
                {
                    let caption = tab.read(cx).caption();
                    let menu_owner = cx.entity().downgrade();
                    let tab_id = tab.entity_id();
                    Some(
                        islands_tab(
                            ("diff-tab", index),
                            self.active_diff_index() == Some(index),
                            cx,
                        )
                        .child(jb_icon("diff_dark.svg"))
                        .child(div().max_w(px(280.)).truncate().child(caption))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.activate_editor_neighbor(Some(tab_id), w, cx)
                        }))
                        .child(
                            Button::new(("close-diff", index))
                                .ghost()
                                .small()
                                .w(px(16.))
                                .h(px(16.))
                                .icon(jb_icon("closeSmall.svg"))
                                .tooltip(t("Close diff · Esc"))
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    cx.stop_propagation();
                                    if let Some(index) =
                                        s.tabs.iter().position(|tab| tab.entity_id() == tab_id)
                                    {
                                        s.close_diff_tab_from_header(index, w, cx);
                                    }
                                })),
                        )
                        .context_menu(move |menu, _, cx| {
                            menu_owner
                                .update(cx, |s, cx| s.editor_tab_menu(menu, Some(tab_id), cx))
                                .unwrap()
                        }),
                    )
                } else if let Some((index, tab)) = self
                    .branch_compare_tabs
                    .iter()
                    .enumerate()
                    .find(|(_, tab)| tab.entity_id() == *id)
                {
                    let tab_id = tab.entity_id();
                    let owner = cx.entity().downgrade();
                    let pair = &tab.read(cx).pair;
                    let caption = format!(
                        "{}: {} / {}",
                        if i18n::is_chinese() {
                            "比较"
                        } else {
                            "Compare"
                        },
                        pair.first_ref
                            .trim_start_matches("refs/heads/")
                            .trim_start_matches("refs/remotes/"),
                        pair.second_ref
                            .trim_start_matches("refs/heads/")
                            .trim_start_matches("refs/remotes/")
                    );
                    Some(
                        islands_tab(
                            ("branch-compare-tab", index),
                            self.active_branch_compare_index() == Some(index),
                            cx,
                        )
                        .child(jb_icon("vcs_dark.svg"))
                        .child(div().max_w(px(280.)).truncate().child(caption))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.activate_editor_neighbor(Some(tab_id), w, cx)
                        }))
                        .child(
                            Button::new(("close-branch-compare", index))
                                .ghost()
                                .small()
                                .w(px(16.))
                                .h(px(16.))
                                .icon(jb_icon("closeSmall.svg"))
                                .tooltip(t("Close"))
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    cx.stop_propagation();
                                    s.close_editor_ids(vec![tab_id], Some(tab_id), w, cx);
                                })),
                        )
                        .context_menu(move |menu, _, cx| {
                            owner
                                .update(cx, |s, cx| s.editor_tab_menu(menu, Some(tab_id), cx))
                                .unwrap()
                        }),
                    )
                } else {
                    None
                }
            }))
            .child(div().flex_1())
            .children(
                self.active_file_index()
                    .and_then(|i| self.file_tabs.get(i))
                    .and_then(|tab| tab.update(cx, |tab, cx| tab.markdown_toolbar(cx))),
            )
    }
}
