//! Presentation controls for the Log changes browser; selection is not layout state.
use super::*;

pub(super) fn log_file_count(count: usize) -> String {
    if i18n::is_chinese() {
        format!("{count} 个文件")
    } else {
        format!("{count} {}", if count == 1 { "file" } else { "files" })
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;

    #[gpui::test]
    fn log_grouping_keeps_selection_and_collapsed_directory_state(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        cx.update(|_, cx| {
            review.unwrap().update(cx, |s, _| {
                s.files = Arc::new(
                    ["src/a.rs", "other/b.rs"]
                        .into_iter()
                        .map(|p| Change {
                            path: p.into(),
                            old_path: None,
                            status: "M".into(),
                            area: Area::Commit,
                        })
                        .collect(),
                );
                s.local = false;
                s.selected_file = Some(1);
                s.collapsed_directories.insert((Area::Commit, "src".into()));
                s.log_files_options.directory = true;
                s.rebuild_files();
                assert!(!s.file_rows.iter().any(|r| matches!(r, FileRow::File(0, _))));
                s.log_files_options.directory = false;
                s.rebuild_files();
                assert_eq!(s.file_rows.len(), 2);
                assert!(matches!(s.file_rows[1], FileRow::File(1, 0)));
                assert_eq!(s.selected_file, Some(1));
                s.log_files_options.directory = true;
                s.rebuild_files();
                assert!(!s.file_rows.iter().any(|r| matches!(r, FileRow::File(0, _))));
                assert_eq!(s.selected_file, Some(1));
            })
        });
    }
}

impl Review {
    pub(super) fn set_bottom_preview(&mut self, bottom: bool, cx: &mut Context<Self>) -> bool {
        if let Err(error) = layout_preferences::save_bottom_preview(bottom) {
            self.error = Some(format!("{}: {error}", t("Settings could not be saved")));
            cx.notify();
            return false;
        }
        self.bottom_preview = bottom;
        // The single-panel hidden layout must not overwrite the remembered two-panel sizes.
        self.preview_split = cx.new(|_| ResizableState::default());
        if !self.log_query.busy() {
            if let Some(index) = self.selected_file {
                self.load_file(index, cx);
            }
        }
        cx.notify();
        true
    }

    fn save_log_files_options(
        &mut self,
        options: layout_preferences::LogFilesOptions,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Err(error) = layout_preferences::save_log_files_options(options) {
            self.error = Some(format!("{}: {error}", t("Settings could not be saved")));
            cx.notify();
            return false;
        }
        self.log_files_options = options;
        cx.notify();
        true
    }

    pub(super) fn toggle_log_directory(&mut self, cx: &mut Context<Self>) {
        if self.local {
            return;
        }
        let mut options = self.log_files_options;
        options.directory = !options.directory;
        if !self.save_log_files_options(options, cx) {
            return;
        }
        self.rebuild_files();
        if let Some(row) = self
            .file_rows
            .iter()
            .position(|row| matches!(row, FileRow::File(i, _) if Some(*i) == self.selected_file))
        {
            self.files_scroll.scroll_to_item(row, ScrollStrategy::Top);
        }
    }

    pub(super) fn files_view_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let options = self.log_files_options;
        let preview = self.bottom_preview;
        let focus = self.files_focus.clone();
        Button::new("log-files-view-options")
            .ghost()
            .small()
            .w(px(28.))
            .h(px(28.))
            .icon(jb_icon("history-show_dark.svg"))
            .tooltip(t("View Options"))
            .dropdown_menu(move |menu, _, _| {
                let directory = owner.clone();
                let details = owner.clone();
                let preview_owner = owner.clone();
                menu.action_context(focus.clone())
                    .label(t("Group By"))
                    .item(
                        PopupMenuItem::new(t("Directory"))
                            .action(Box::new(ToggleLogDirectory))
                            .checked(options.directory)
                            .on_click(move |_, _, cx| {
                                let _ = directory.update(cx, |s, cx| s.toggle_log_directory(cx));
                            }),
                    )
                    .separator()
                    .label(t("Layout"))
                    .item(
                        PopupMenuItem::new(t("Show Details"))
                            .checked(options.details)
                            .on_click(move |_, _, cx| {
                                let _ = details.update(cx, |s, cx| {
                                    let mut options = s.log_files_options;
                                    options.details = !options.details;
                                    s.save_log_files_options(options, cx);
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new(t("Show Diff Preview"))
                            .checked(preview)
                            .on_click(move |_, _, cx| {
                                let _ = preview_owner.update(cx, |s, cx| {
                                    s.activate_log_preview();
                                    s.set_bottom_preview(!s.bottom_preview, cx);
                                });
                            }),
                    )
            })
    }
}
