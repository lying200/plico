//! Main-window preview coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn uses_local_changes_preview(&self) -> bool {
        self.local_visible && self.local_preview_requested
    }
    pub(super) fn activate_log_preview(&mut self) {
        if self.local_preview_requested {
            self.local_preview_requested = false;
            self.local_preview = None;
        }
    }

    pub(super) fn show_error_details(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(error) = self.error.clone() else {
            return;
        };
        crate::error_ui::show_details(error, "Git error details", window, cx);
    }
    pub(super) fn empty_diff_message(&self) -> &'static str {
        if self.files.is_empty() {
            if !self.local && self.text_filter.is_some() && self.commits.is_empty() {
                return "No commits match the message filter. Clear the filter field and press Enter to show all history.";
            }
            if self.local {
                "No local changes. The index and working tree are clean."
            } else if let Some(commit) = self.selected.and_then(|i| self.commits.get(i)) {
                if commit.shallow_boundary {
                    "Parent history is unavailable at this shallow boundary. Deepen the clone with your Git tool, then refresh to review its changes."
                } else if commit.parents.len() > 1 {
                    "No file changes compared with the first parent. Other merge parents may differ."
                } else {
                    "This commit has no file changes compared with its parent or the empty tree."
                }
            } else {
                if !self.selection_details.is_empty() {
                    "No changes"
                } else if !self.commits.is_empty() {
                    "Select commits to view changes"
                } else {
                    "No commits in this history. Use Local Changes to review files before the first commit."
                }
            }
        } else if self.selected_file.is_some() {
            "No patch was returned for this file. Refresh to check for changes made outside this viewer."
        } else {
            "Select a changed file to review its diff"
        }
    }
    pub(super) fn preview_place(&self) -> layout_preferences::DiffPlace {
        if self.local {
            layout_preferences::DiffPlace::Changes
        } else {
            layout_preferences::DiffPlace::Log
        }
    }
    pub(super) fn set_preview_appearance(
        &mut self,
        value: layout_preferences::DiffAppearance,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = layout_preferences::save_diff_appearance(self.preview_place(), value) {
            self.preview_preference_error =
                Some(format!("{}: {error}", t("Cannot save Diff settings")));
            cx.notify();
            return;
        }
        self.preview_preference_error = None;
        self.preview_appearance = value;
        self.diff.update(cx, |d, cx| {
            d.set_inline_unit(crate::diff_settings::inline_unit(value.emphasis), cx)
        });
        cx.notify();
    }
    pub(super) fn set_preview_mode(&mut self, mode: DiffMode, cx: &mut Context<Self>) {
        if let Err(error) =
            layout_preferences::save_diff_unified(self.preview_place(), mode == DiffMode::Unified)
        {
            self.preview_preference_error =
                Some(format!("{}: {error}", t("Cannot save Diff view")));
            cx.notify();
            return;
        }
        self.preview_preference_error = None;
        self.preview_mode = mode;
        self.diff.update(cx, |d, cx| {
            d.set_mode(crate::diff_settings::effective_mode(d.files(), mode), cx)
        });
        cx.notify();
    }
    pub(super) fn preview_settings_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        crate::diff_settings::appearance_button(self.preview_appearance, move |value, cx| {
            let _ = owner.update(cx, |s, cx| s.set_preview_appearance(value, cx));
        })
    }
    pub(super) fn diff_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let side_titles = self
            .selected_file
            .and_then(|index| self.files.get(index))
            .and_then(|file| {
                let revisions = match file.area {
                    Area::Commit => {
                        let commit = self.selected.and_then(|index| self.commits.get(index))?;
                        let before = commit
                            .parents
                            .first()
                            .map(|id| {
                                let short = &id[..8.min(id.len())];
                                if commit.parents.len() > 1 {
                                    format!("{} {short}", t("First parent"))
                                } else {
                                    short.to_owned()
                                }
                            })
                            .unwrap_or_else(|| t("Empty tree").into());
                        (before, commit.id[..8.min(commit.id.len())].to_owned())
                    }
                    Area::WorkingTree => ("HEAD".into(), t("Working tree").into()),
                    Area::Staged => ("HEAD".into(), t("Staged").into()),
                    Area::Unstaged => (t("Staged").into(), t("Working tree").into()),
                    Area::Untracked => (t("Empty file").into(), t("Untracked working file").into()),
                    Area::Conflict | Area::Ignored => return None,
                };
                let before = file
                    .old_path
                    .as_ref()
                    .unwrap_or(&file.path)
                    .to_string_lossy();
                Some((
                    format!("{before} · {}", revisions.0),
                    format!("{} · {}", file.path.display(), revisions.1),
                ))
            });
        let content = if let Some(error) = self.preview_error.clone() {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .p_4()
                .child(t("Could not load the diff."))
                .child(
                    h_flex()
                        .gap_3()
                        .child(
                            Button::new("reload-preview")
                                .link()
                                .small()
                                .label(t("Reload diff"))
                                .on_click(cx.listener(|s, _, _, cx| {
                                    if let Some(index) = s.selected_file {
                                        s.load_file(index, cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new("copy-preview-error")
                                .link()
                                .small()
                                .label(t("Copy error details"))
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(error.clone()))
                                }),
                        ),
                )
                .into_any_element()
        } else if self.error.is_some() {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .p_4()
                .child(t("Could not load changes. Refresh to retry."))
                .child(
                    Button::new("reload-changes")
                        .link()
                        .small()
                        .label(t("Refresh"))
                        .disabled(self.busy || self.history_loading)
                        .on_click(cx.listener(|s, _, w, cx| s.refresh_current(w, cx))),
                )
                .into_any_element()
        } else if self.busy || self.loading_patch {
            div()
                .p_5()
                .text_color(rgb(MUTED))
                .child(t("Reading Git data…"))
                .into_any_element()
        } else if self.diff.read(cx).files().is_empty() {
            div()
                .p_5()
                .text_color(rgb(MUTED))
                .child(t(self.empty_diff_message()))
                .into_any_element()
        } else {
            Diff::new(&self.diff)
                .size_full()
                .soft_wrap(self.preview_appearance.soft_wrap)
                .line_number(self.preview_appearance.line_numbers)
                .change_indicator(DiffChangeIndicator::None)
                .into_any_element()
        };
        v_flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .bg(rgb(BG))
            .rounded(px(10.))
            .child(
                h_flex()
                    .h(px(40.)).flex_shrink_0().mx(px(6.)).mt(px(2.))
                    .px_3()
                    .gap_2()
                    .bg(rgb(0x212326))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .truncate()
                            .flex_1()
                            .text_size(px(12.))
                            .child(self.base.as_ref().map(|base| base.text()).unwrap_or_default()),
                    )
                    .child(
                        Button::new("prev-change")
                            .ghost()
                            .small()
                            .icon(jb_icon("up_dark.svg"))
                            .tooltip(t("Previous change (Shift+F7)"))
                            .disabled(self.preview_error.is_some() || self.error.is_some() || self.loading_patch || self.busy)
                            .on_click(cx.listener(|s, _, w, cx| {
                                if let Some(tab) = s.active_diff_index().and_then(|i| s.tabs.get(i)) { tab.update(cx, |t, cx| t.navigate(false, w, cx)); } else { if let Some(p) = &s.local_preview { p.update(cx, |p, cx| p.navigate(false, w, cx)); } else { s.diff.update(cx, |d, cx| d.previous_change(cx)); } }
                            })),
                    )
                    .child(
                        Button::new("next-change")
                            .ghost()
                            .small()
                            .icon(jb_icon("down_dark.svg"))
                            .tooltip(t("Next change (F7)"))
                            .disabled(self.preview_error.is_some() || self.error.is_some() || self.loading_patch || self.busy)
                            .on_click(cx.listener(|s, _, w, cx| {
                                if let Some(tab) = s.active_diff_index().and_then(|i| s.tabs.get(i)) { tab.update(cx, |t, cx| t.navigate(true, w, cx)); } else { if let Some(p) = &s.local_preview { p.update(cx, |p, cx| p.navigate(true, w, cx)); } else { s.diff.update(cx, |d, cx| d.next_change(cx)); } }
                            })),
                    )
                    .child(
                        Button::new("preview-jump-source").ghost().small().icon(jb_icon("edit_dark.svg"))
                            .tooltip(t("Jump to Source · F4")).disabled(self.loading_patch || self.busy || self.selected_file.is_none())
                            .on_click(cx.listener(|s,_,w,cx|s.jump_to_source(w,cx)))
                    )
                    .child(
                        Button::new("expand")
                            .ghost()
                            .small()
                            .label(t(if self.expanded { "Restore" } else { "Expand" }))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.expanded = !s.expanded;
                                cx.notify();
                            })),
                    )
                    .child({
                        let owner = cx.entity().downgrade();
                        crate::diff_settings::mode_selector(self.preview_mode, move |mode, cx| {
                            let _ = owner.update(cx, |s, cx| s.set_preview_mode(mode, cx));
                        })
                    })
                    .child(self.preview_settings_button(cx)),
            )
            .when_some(self.preview_preference_error.clone(), |panel, error| panel.child(
                h_flex().px_3().py_1().gap_2()
                    .child(div().text_color(rgb(0xf2a6a6)).child(t("Could not save Diff preferences. Try the setting again.")))
                    .child(Button::new("copy-preview-preference-error").link().small().label(t("Copy error details"))
                        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(error.clone()))))))
            .when(
                self.error.is_none()
                    && self.diff.read(cx).files().iter().any(|file| {
                        file.extended_headers().iter().any(|header| {
                            header.split_whitespace().last() == Some("160000")
                        })
                    }),
                |panel| panel.child(
                    div().px_3().py_2().text_size(px(12.)).text_color(rgb(MUTED))
                        .child(t("Submodule reference changed · The diff shows commit IDs, not the files inside the submodule.")),
                ),
            )
            .when_some(side_titles.filter(|_| self.preview_error.is_none() && self.error.is_none()), |panel, (before, after)| {
                let status = self.diff.read(cx).files().first().map(|file| file.status());
                let unified = self.preview_mode == DiffMode::Unified;
                let titles = [(status != Some(DiffFileStatus::Added)).then_some(before),
                    (status != Some(DiffFileStatus::Deleted)).then_some(after)];
                panel.child(
                    div().flex().when(unified, |row| row.flex_col()).w_full().flex_shrink_0()
                        .border_b_1().border_color(rgb(BORDER))
                        .text_size(px(12.)).text_color(rgb(MUTED))
                        .children(titles.into_iter().flatten().enumerate().map(|(side, title)| {
                            div().id(("diff-version", side)).when(unified, |cell| cell.w_full())
                                .when(!unified, |cell| cell.flex_1()).min_w_0().px_3().py_1().truncate()
                                .tooltip({ let title = title.clone(); move |window, cx| Tooltip::new(title.clone()).build(window, cx) })
                                .child(title)
                        })),
                )
            })
            .child(div().flex_1().w_full().min_h_0().min_w_0().child(content))
            .relative().children(island_corners(!self.expanded))
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod ownership_tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;
    #[gpui::test]
    fn clicking_same_log_commit_reclaims_preview_without_closing_commit_sidebar(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone()); Root::new(view, window, cx)
        });
        let view = review.unwrap();
        cx.update(|window, cx| view.update(cx, |s, cx| {
            s.commits = Arc::new(vec![git::Commit { id: "a".into(), parents: vec![], shallow_boundary: false,
                author: String::new(), date: String::new(), subject: "commit".into(), refs: vec![] }]);
            s.log_selection.select(&s.commit_ids(), 0, false, false);
            s.history_loading = false;
            s.open_local(window, cx);
            assert!(s.uses_local_changes_preview());
            s.select_log_row(0, false, false, false, cx);
            assert!(s.local_visible, "switching preview must not collapse Commit");
            assert!(!s.uses_local_changes_preview(), "Log must own the preview even if selection did not change");
        }));
        cx.update(|_, cx| view.update(cx, |s, cx| {
            s.local_view.as_ref().unwrap().update(cx, |_, cx| cx.notify());
        }));
        cx.update(|_, cx| view.update(cx, |s, cx| {
            assert!(!s.uses_local_changes_preview(), "ordinary sidebar notifications must not reclaim preview");
            s.local_view.as_ref().unwrap().update(cx, |_, cx| cx.emit(crate::local_view::PreviewSelected));
        }));
        cx.update(|_, cx| view.update(cx, |s, cx| {
            assert!(s.uses_local_changes_preview(), "explicit local selection must reclaim preview");
            s.select_log_row(0, true, false, false, cx);
            assert!(!s.uses_local_changes_preview(), "empty Log selection must not reveal local diff");
        }));
    }
}
