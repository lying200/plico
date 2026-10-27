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
        let Some(error) = self
            .error
            .clone()
            .or_else(|| self.update_diagnostic.clone())
            .or_else(|| self.sequence_diagnostic.clone())
        else {
            return;
        };
        let title = if self.error.is_none()
            && self.update_diagnostic.is_some()
            && self.update_diagnostic_warning
        {
            "Repositories skipped"
        } else {
            "Git error details"
        };
        crate::error_ui::show_details(error, title, window, cx);
    }
    pub(super) fn empty_diff_message(&self) -> &'static str {
        if self.files.is_empty() {
            if self.local {
                "No local changes. The index and working tree are clean."
            } else if let Some(commit) = self.selected.and_then(|i| self.log.commits().get(i)) {
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
                } else {
                    "Select commits to view changes"
                }
            }
        } else if self.selected_file.is_some() {
            "No patch was returned for this file. Refresh to check for changes made outside this viewer."
        } else {
            "Select a changed file to review its diff"
        }
    }
    pub(super) fn log_preview_target(
        &self,
        cx: &App,
    ) -> Option<(diff_target::Target, diff_target::Chain)> {
        let index = self.selected_file?;
        if let Some(mut chain) = self.aggregate_chain.clone() {
            return chain.select(index).map(|target| (target, chain));
        }
        let change = self.files.get(index)?.clone();
        let commit = if self.local {
            None
        } else {
            Some(self.log.commits().get(self.selected?)?.clone())
        };
        let target = diff_target::Target::new(self.log_files_repository()?, commit, change).ok()?;
        let chain = self
            .local_preview
            .as_ref()
            .and_then(|preview| preview.read(cx).fork_chain(&target))
            .or_else(|| diff_target::Chain::new(&target, &self.files).ok())?;
        Some((target, chain))
    }

    pub(super) fn diff_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let error = self.preview_error.as_ref().or(self.error.as_ref()).cloned();
        v_flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(rgb(BG))
            .child(
                div()
                    .h(px(LOG_TOOLBAR_HEIGHT))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(BORDER)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .w_full()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .p_4()
                    .text_color(rgb(MUTED))
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .whitespace_normal()
                            .text_center()
                            .child(t(if error.is_some() {
                                "Could not load the diff."
                            } else if self.log_query.busy() {
                                "Reading Git data…"
                            } else {
                                self.empty_diff_message()
                            })),
                    )
                    .when_some(error, |panel, error| {
                        panel.child(
                            h_flex()
                                .gap_3()
                                .child(
                                    Button::new("reload-preview-changes")
                                        .link()
                                        .small()
                                        .label(t("Refresh"))
                                        .disabled(
                                            self.log_query.busy()
                                                || self.log_query.history_loading(),
                                        )
                                        .on_click(
                                            cx.listener(|s, _, w, cx| s.refresh_current(w, cx)),
                                        ),
                                )
                                .child(
                                    Button::new("copy-preview-error")
                                        .link()
                                        .small()
                                        .label(t("Copy error details"))
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                error.clone(),
                                            ))
                                        }),
                                ),
                        )
                    }),
            )
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod ownership_tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;
    #[gpui::test]
    fn file_selection_keeps_details_job_and_preview_uses_selected_revision(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        cx.update(|_, cx| {
            review.unwrap().update(cx, |s, cx| {
                s.log = log_display::Display::fixture_single(
                    &s.repo.path,
                    ['a', 'b']
                        .into_iter()
                        .map(|id| git::Commit {
                            id: id.to_string().repeat(40),
                            parents: vec![],
                            shallow_boundary: false,
                            committer_timestamp: 0,
                            author: String::new(),
                            date: String::new(),
                            subject: String::new(),
                            refs: vec![],
                        })
                        .collect(),
                );
                s.local = false;
                s.files = Arc::new(vec![git::Change {
                    path: "same.txt".into(),
                    old_path: None,
                    status: "M".into(),
                    area: Area::Commit,
                }]);
                s.selected = Some(0);
                let generation = s.log_query.stamp();
                s.load_file(0, cx);
                assert_eq!(
                    s.log_query.stamp(),
                    generation,
                    "file selection must not cancel details"
                );
                let (first, _) = s.log_preview_target(cx).unwrap();
                s.selected = Some(1);
                let (second, _) = s.log_preview_target(cx).unwrap();
                assert_ne!(
                    first.identity(),
                    second.identity(),
                    "same filename is not same revision"
                );
                s.selected_file = None;
                assert!(s.log_preview_target(cx).is_none());
            })
        });
    }

    #[gpui::test]
    fn clicking_same_log_commit_reclaims_preview_without_closing_commit_sidebar(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = review.unwrap();
        cx.update(|window, cx| {
            view.update(cx, |s, cx| {
                s.log = log_display::Display::fixture_single(
                    &s.repo.path,
                    vec![git::Commit {
                        id: "a".into(),
                        parents: vec![],
                        shallow_boundary: false,
                        committer_timestamp: 0,
                        author: String::new(),
                        date: String::new(),
                        subject: "commit".into(),
                        refs: vec![],
                    }],
                );
                s.log_selection.select(&s.commit_ids(), 0, false, false);
                s.log_query.finish(s.log_query.stamp());
                s.open_local(window, cx);
                assert!(s.uses_local_changes_preview());
                s.select_log_row(0, false, false, false, cx);
                assert!(
                    s.local_visible,
                    "switching preview must not collapse Commit"
                );
                assert!(
                    !s.uses_local_changes_preview(),
                    "Log must own the preview even if selection did not change"
                );
            })
        });
        cx.update(|_, cx| {
            view.update(cx, |s, cx| {
                s.local_view
                    .as_ref()
                    .unwrap()
                    .update(cx, |_, cx| cx.notify());
            })
        });
        cx.update(|_, cx| {
            view.update(cx, |s, cx| {
                assert!(
                    !s.uses_local_changes_preview(),
                    "ordinary sidebar notifications must not reclaim preview"
                );
                s.local_view
                    .as_ref()
                    .unwrap()
                    .update(cx, |_, cx| cx.emit(crate::local_view::PreviewSelected));
            })
        });
        cx.update(|_, cx| {
            view.update(cx, |s, cx| {
                assert!(
                    s.uses_local_changes_preview(),
                    "explicit local selection must reclaim preview"
                );
                s.select_log_row(0, true, false, false, cx);
                assert!(
                    !s.uses_local_changes_preview(),
                    "empty Log selection must not reveal local diff"
                );
            })
        });
    }
}
