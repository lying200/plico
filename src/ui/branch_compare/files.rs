//! Branch comparison ChangesBrowser presentation; indices always refer to the Diff chain.
use super::*;
use plico::file_tree::{self, Row};

impl Pane {
    fn set_files_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        if !self.group_files {
            return;
        }
        self.collapsed_files.clear();
        if !expanded {
            // Build from the complete chain: hidden descendants must also collapse.
            if let Some(chain) = &self.chain {
                for row in file_tree::compact_rows(
                    chain
                        .changes()
                        .iter()
                        .enumerate()
                        .map(|(i, c)| (i, c.path.as_path())),
                    |_| false,
                ) {
                    if let Row::Folder { path, .. } = row {
                        self.collapsed_files.insert(path);
                    }
                }
                if !chain.changes().is_empty() {
                    self.collapsed_files.insert(PathBuf::new());
                    if self.selected_file.is_some() || self.selected_folder.is_some() {
                        self.selected_file = None;
                        self.selected_folder = Some(PathBuf::new());
                    }
                }
            }
        }
        self.rebuild_file_rows();
        cx.notify();
    }

    pub(super) fn active_file_row(&self) -> Option<usize> {
        self.file_rows.iter().position(|row| match row {
            Row::File { index, .. } => self.selected_file == Some(*index),
            Row::Folder { path, .. } => self.selected_folder.as_ref() == Some(path),
        })
    }

    fn choose_file_row(&mut self, index: usize) {
        match self.file_rows.get(index) {
            Some(Row::File { index, .. }) => {
                self.selected_file = Some(*index);
                self.selected_folder = None;
            }
            Some(Row::Folder { path, .. }) => {
                self.selected_folder = Some(path.clone());
                self.selected_file = None;
            }
            None => return,
        }
        self.files_scroll.scroll_to_item(index, ScrollStrategy::Top);
    }

    pub(super) fn step_file_row(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.file_rows.is_empty() {
            return;
        }
        let next = self
            .active_file_row()
            .map(|i| i.saturating_add_signed(delta).min(self.file_rows.len() - 1))
            .unwrap_or(0);
        self.choose_file_row(next);
        cx.notify();
    }

    pub(super) fn expand_file_row(&mut self, expand: bool, cx: &mut Context<Self>) {
        let Some(index) = self.active_file_row() else {
            return;
        };
        let row = self.file_rows[index].clone();
        let depth = match &row {
            Row::File { depth, .. } | Row::Folder { depth, .. } => *depth,
        };
        if let Row::Folder { path, expanded, .. } = &row {
            if *expanded != expand {
                if expand {
                    self.collapsed_files.remove(path);
                } else {
                    self.collapsed_files.insert(path.clone());
                }
                self.rebuild_file_rows();
                cx.notify();
                return;
            }
            if expand {
                if index + 1 < self.file_rows.len() {
                    self.choose_file_row(index + 1);
                }
                cx.notify();
                return;
            }
        }
        if !expand {
            if let Some(parent) = (0..index)
                .rev()
                .find(|&i| matches!(&self.file_rows[i], Row::Folder { depth: d, .. } if *d < depth))
            {
                self.choose_file_row(parent);
            }
        }
        cx.notify();
    }

    pub(super) fn file_navigation_order(&self) -> Vec<usize> {
        let Some(chain) = &self.chain else {
            return Vec::new();
        };
        let changes = chain.changes();
        if self.group_files {
            file_tree::compact_rows(
                changes
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.path.as_path())),
                |_| false,
            )
            .into_iter()
            .filter_map(|row| match row {
                Row::File { index, .. } => Some(index),
                _ => None,
            })
            .collect()
        } else {
            let mut indices: Vec<_> = (0..changes.len()).collect();
            indices.sort_by_key(|&i| (changes[i].path.file_name(), &changes[i].path));
            indices
        }
    }

    pub(super) fn rebuild_file_rows(&mut self) {
        let Some(chain) = &self.chain else {
            self.file_rows.clear();
            return;
        };
        let changes = chain.changes();
        self.file_rows = if self.group_files {
            let mut rows = file_tree::compact_rows(
                changes
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.path.as_path())),
                |path| self.collapsed_files.contains(path),
            );
            let expanded = !self.collapsed_files.contains(&PathBuf::new());
            if !expanded {
                rows.clear();
            }
            for row in &mut rows {
                match row {
                    Row::Folder { depth, .. } | Row::File { depth, .. } => *depth += 1,
                }
            }
            if !changes.is_empty() {
                rows.insert(
                    0,
                    Row::Folder {
                        path: PathBuf::new(),
                        label: PathBuf::new(),
                        depth: 0,
                        count: changes.len(),
                        expanded,
                    },
                );
            }
            rows
        } else {
            let mut indices: Vec<_> = (0..changes.len()).collect();
            indices.sort_by_key(|&i| (changes[i].path.file_name(), &changes[i].path));
            indices
                .into_iter()
                .map(|index| Row::File { index, depth: 0 })
                .collect()
        };
    }

    pub(super) fn files_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let grouped = self.group_files;
        let details = self.show_details;
        let preview = self.show_preview;
        h_flex()
            .h(px(28.))
            .gap_1()
            .child(
                Button::new("compare-file-diff")
                    .ghost()
                    .small()
                    .icon(jb_icon("diff_dark.svg"))
                    .tooltip(t("Show Diff"))
                    .disabled(self.selected_file.is_none())
                    .on_click(cx.listener(|s, _, _, cx| {
                        if let Some(i) = s.selected_file {
                            s.open_file(i, cx);
                        }
                    })),
            )
            .child(
                Button::new("compare-file-options")
                    .ghost()
                    .small()
                    .icon(jb_icon("history-show_dark.svg"))
                    .tooltip(t("View Options"))
                    .dropdown_menu(move |menu, _, _| {
                        let directory = owner.clone();
                        let details_owner = owner.clone();
                        let preview_owner = owner.clone();
                        menu.label(t("Group By"))
                            .item(
                                PopupMenuItem::new(t("Directory"))
                                    .checked(grouped)
                                    .on_click(move |_, _, cx| {
                                        let _ = directory.update(cx, |s, cx| {
                                            s.group_files = !s.group_files;
                                            s.selected_folder = None;
                                            s.rebuild_file_rows();
                                            cx.notify();
                                        });
                                    }),
                            )
                            .separator()
                            .label(t("Layout"))
                            .item(
                                PopupMenuItem::new(t("Show Details"))
                                    .checked(details)
                                    .on_click(move |_, _, cx| {
                                        let _ = details_owner.update(cx, |s, cx| {
                                            s.show_details = !s.show_details;
                                            cx.notify();
                                        });
                                    }),
                            )
                            .item(
                                PopupMenuItem::new(t("Show Diff Preview"))
                                    .checked(preview)
                                    .on_click(move |_, window, cx| {
                                        let _ = preview_owner.update(cx, |s, cx| {
                                            let shown = !s.show_preview;
                                            if let Err(error) =
                                                layout_preferences::save_branch_preview(
                                                    &s.repo.path,
                                                    s.side == Side::Second,
                                                    shown,
                                                )
                                            {
                                                s.detail_error = Some(error.to_string());
                                                cx.notify();
                                                return;
                                            }
                                            s.show_preview = shown;
                                            cx.emit(PreviewChanged(s.side, shown));
                                            s.sync_preview(window, cx);
                                            cx.notify();
                                        });
                                    }),
                            )
                    }),
            )
            .child(div().flex_1())
            .when(self.group_files, |bar| {
                bar.child(
                    Button::new("compare-files-expand")
                        .ghost()
                        .small()
                        .icon(jb_icon("expandAll_dark.svg"))
                        .tooltip(t("Expand All"))
                        .disabled(self.file_rows.is_empty())
                        .on_click(cx.listener(|s, _, _, cx| s.set_files_expanded(true, cx))),
                )
                .child(
                    Button::new("compare-files-collapse")
                        .ghost()
                        .small()
                        .icon(jb_icon("collapseAll_dark.svg"))
                        .tooltip(t("Collapse All"))
                        .disabled(self.file_rows.is_empty())
                        .on_click(cx.listener(|s, _, _, cx| s.set_files_expanded(false, cx))),
                )
            })
    }

    pub(super) fn file_row(&self, row_index: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = self.file_rows[row_index].clone();
        let (depth, label, icon, color, selected) = match &row {
            Row::Folder { label, depth, .. } => (
                *depth,
                label.to_string_lossy().into_owned(),
                crate::ui_style::themed_asset("jb/folder_dark.svg", cx).to_owned(),
                crate::ui_theme::palette(cx).foreground,
                matches!(&row, Row::Folder { path, .. } if self.selected_folder.as_ref() == Some(path)),
            ),
            Row::File { index, depth } => {
                let change = &self.chain.as_ref().unwrap().changes()[*index];
                (
                    *depth,
                    change
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    file_icon(&change.path, cx).to_owned(),
                    crate::ui_style::change_color(change, cx),
                    self.selected_file == Some(*index),
                )
            }
        };
        let name_end = label.len();
        let mut label = label;
        if let Row::File { index, .. } = &row {
            if !self.group_files {
                if let Some(parent) = self.chain.as_ref().unwrap().changes()[*index].path.parent() {
                    if !parent.as_os_str().is_empty() {
                        label.push_str("  ");
                        label.push_str(&parent.to_string_lossy());
                    }
                }
            }
        }
        let label_end = label.len();
        h_flex()
            .id(("compare-file", row_index))
            .h(px(28.))
            .w_full()
            .min_w_0()
            .rounded(px(4.))
            .pl(px(8. + depth as f32 * 16.))
            .pr_2()
            .gap_2()
            .when(selected, |v| {
                v.bg(rgb(crate::ui_theme::color(cx, 0x2e4773, 0xd0dffe)))
            })
            .when_some(
                match &row {
                    Row::Folder { expanded, .. } => Some(*expanded),
                    _ => None,
                },
                |v, expanded| {
                    v.child(jb_icon(if expanded {
                        "chevronDown_dark.svg"
                    } else {
                        "chevronRight_dark.svg"
                    }))
                },
            )
            .child(img(icon).size(px(16.)).flex_shrink_0())
            .child(div().min_w_0().text_color(rgb(color)).truncate().child(
                StyledText::new(label).with_highlights([(
                    name_end..label_end,
                    HighlightStyle {
                        color: Some(rgb(crate::ui_theme::palette(cx).muted).into()),
                        ..Default::default()
                    },
                )]),
            ))
            .when_some(
                match &row {
                    Row::Folder { count, .. } => Some(*count),
                    _ => None,
                },
                |v, count| {
                    v.child(
                        div()
                            .text_color(rgb(crate::ui_theme::palette(cx).muted))
                            .child(super::super::files_options::log_file_count(count)),
                    )
                },
            )
            .on_click(cx.listener(move |s, event: &ClickEvent, window, cx| {
                s.files_focus.focus(window, cx);
                match &row {
                    Row::Folder { path, .. } => {
                        s.selected_file = None;
                        s.selected_folder = Some(path.clone());
                        if !s.collapsed_files.remove(path) {
                            s.collapsed_files.insert(path.clone());
                        }
                        s.rebuild_file_rows();
                    }
                    Row::File { index, .. } => {
                        s.selected_file = Some(*index);
                        s.selected_folder = None;
                        if event.click_count() == 2 {
                            s.open_file(*index, cx);
                        }
                    }
                }
                cx.notify();
            }))
            .into_any_element()
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;

    #[gpui::test]
    fn grouping_keeps_chain_indices_and_selection(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut pane = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let pair = Pair {
                first_ref: "topic".into(),
                second_ref: "main".into(),
                first: "a".repeat(40),
                second: "b".repeat(40),
            };
            let view = cx.new(|cx| {
                Pane::new(
                    Repository::new("missing-fixture"),
                    pair,
                    Side::First,
                    window,
                    cx,
                )
            });
            pane = Some(view.clone());
            Root::new(view, window, cx)
        });
        pane.unwrap().update(cx, |s, cx| {
            use plico::combined_changes::{Change, Revision};
            let changes = ["src/main/z.txt", "src/main/a.txt", "tests/b.txt"].map(|path| Change {
                before: None,
                after: Some(Revision { revision: "a".repeat(40), path: path.into() }),
            });
            s.chain = Some(diff_target::Chain::combined(Repository::new("fixture"), changes.to_vec(), 0).unwrap().1);
            s.selected_file = Some(0);
            s.rebuild_file_rows();
            assert!(s.file_rows.iter().any(|r| matches!(r, Row::Folder { label, .. } if label == std::path::Path::new("src/main"))));
            s.choose_file_row(3);
            assert_eq!(s.selected_file, Some(0));
            s.expand_file_row(false, cx);
            assert_eq!(s.selected_folder.as_deref(), Some(std::path::Path::new("src/main")));
            assert!(s.selected_file.is_none());
            s.expand_file_row(false, cx);
            assert!(s.collapsed_files.contains(std::path::Path::new("src/main")));
            s.expand_file_row(true, cx);
            s.step_file_row(1, cx);
            assert_eq!(s.selected_file, Some(1));
            s.step_file_row(1, cx);
            assert_eq!(s.selected_file, Some(0));
            s.collapsed_files.insert("src/main".into());
            s.rebuild_file_rows();
            assert!(!s.file_rows.iter().any(|r| matches!(r, Row::File { index: 0 | 1, .. })));
            let order = s.file_navigation_order();
            assert_eq!(order, vec![1, 0, 2]);
            let mut reordered = s.chain.as_ref().unwrap().reordered(&order).unwrap();
            assert_eq!(reordered.index(), 1);
            assert_eq!(reordered.select(0).unwrap().change().path, PathBuf::from("src/main/a.txt"));
            assert_eq!(reordered.step(true).unwrap().change().path, PathBuf::from("src/main/z.txt"));
            assert!(s.chain.as_ref().unwrap().reordered(&[0, 0, 2]).is_none());
            assert!(s.chain.as_ref().unwrap().reordered(&[0, 1, 3]).is_none());
            assert!(s.chain.as_ref().unwrap().reordered(&[0, 1]).is_none());
            s.set_files_expanded(false, cx);
            assert_eq!(s.file_navigation_order(), order);
            assert_eq!(s.file_rows.len(), 1);
            assert!(s.collapsed_files.contains(std::path::Path::new("src/main")));
            assert!(s.collapsed_files.contains(std::path::Path::new("tests")));
            s.set_files_expanded(true, cx);
            assert_eq!(s.file_rows.iter().filter(|r| matches!(r, Row::File { .. })).count(), 3);
            assert!(s.collapsed_files.is_empty());
            s.selected_file = Some(0);
            s.selected_folder = None;
            s.group_files = false;
            s.rebuild_file_rows();
            assert_eq!(s.file_navigation_order(), vec![1, 2, 0]);
            let flat = s.file_rows.clone();
            s.set_files_expanded(false, cx);
            assert_eq!(s.file_rows, flat);
            assert_eq!(s.file_rows, vec![Row::File { index: 1, depth: 0 }, Row::File { index: 2, depth: 0 }, Row::File { index: 0, depth: 0 }]);
            assert_eq!(s.selected_file, Some(0));
            assert_eq!(s.chain.as_ref().unwrap().changes()[0].path, PathBuf::from("src/main/z.txt"));
            s.clear_details();
            assert!(s.file_rows.is_empty());
            assert!(s.selected_file.is_none());
        });
    }
}
