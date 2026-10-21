//! Main-window files coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;
use super::files_options::log_file_count;

impl Review {
    pub(super) fn load_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.files.len() { return; }
        self.selected_file = Some(index);
        self.pending_preview_file = None;
        self.preview_error = None;
        if let Some(row) = self.file_rows.iter().position(|row| matches!(row, FileRow::File(i, _) if *i == index)) {
            self.files_scroll.scroll_to_item(row, ScrollStrategy::Top);
        }
        // The visible preview owns its asynchronous reader, just like an editor Diff.
        // Selecting a file must not cancel the commit-details request.
        cx.notify();
    }
    pub(super) fn move_commit(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.local || self.history_loading || self.commits.is_empty() {
            return;
        }
        self.activate_log_preview();
        let ids = self.commit_ids();
        self.log_selection.step(&ids, delta, false);
        let lead = self.log_selection.lead(&ids);
        self.load_log_selection(cx);
        if let Some(i) = lead {
            self.history_scroll.scroll_to_item(i, ScrollStrategy::Top);
        }
    }
    pub(super) fn extend_commit(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.local || self.history_loading {
            return;
        }
        self.activate_log_preview();
        let ids = self.commit_ids();
        self.log_selection.step(&ids, delta, true);
        let lead = self.log_selection.lead(&ids);
        self.load_log_selection(cx);
        if let Some(i) = lead {
            self.history_scroll.scroll_to_item(i, ScrollStrategy::Top);
        }
    }

    pub(super) fn prune_directory_state(&mut self) {
        let root_collapsed = self.collapsed_directories.contains(&(Area::Commit, PathBuf::new()));
        file_tree::retain_collapsed(
            &mut self.collapsed_directories,
            self.files.iter().map(|f| (f.area, f.path.as_path())),
        );
        if root_collapsed { self.collapsed_directories.insert((Area::Commit, PathBuf::new())); }
    }
    pub(super) fn rebuild_files(&mut self) {
        let mut rows = Vec::new();
        if self.local {
            for area in [
                Area::Conflict,
                Area::Unstaged,
                Area::Staged,
                Area::Untracked,
            ] {
                let count = self.files.iter().filter(|f| f.area == area).count();
                if count == 0 {
                    continue;
                }
                let expanded = !self.collapsed_files.contains(&area);
                rows.push(FileRow::Group(area, count, expanded));
                if expanded {
                    rows.extend(self.directory_rows(area));
                }
            }
        } else {
            if self.log_files_options.directory && !self.files.is_empty() {
                let expanded = !self.collapsed_directories.contains(&(Area::Commit, PathBuf::new()));
                rows.push(FileRow::Folder(Area::Commit, PathBuf::new(), 0, self.files.len(), expanded));
                if expanded { rows.extend(self.directory_rows(Area::Commit)); }
            } else {
                rows.extend(self.files.iter().enumerate().map(|(i, _)| FileRow::File(i, 0)));
            }
        }
        self.file_rows = Arc::new(rows);
    }
    pub(super) fn directory_rows(&self, area: Area) -> Vec<FileRow> {
        file_tree::rows(
            self.files
                .iter()
                .enumerate()
                .filter(|(_, f)| f.area == area)
                .map(|(i, f)| (i, f.path.as_path())),
            |path| {
                self.collapsed_directories
                    .contains(&(area, path.to_path_buf()))
            },
        )
        .into_iter()
        .map(|row| match row {
            file_tree::Row::File { index, depth } => FileRow::File(index, depth + usize::from(area == Area::Commit)),
            file_tree::Row::Folder {
                path,
                depth,
                count,
                expanded,
            } => FileRow::Folder(area, path, depth + usize::from(area == Area::Commit), count, expanded),
        })
        .collect()
    }
    pub(super) fn move_file(&mut self, delta: isize, cx: &mut Context<Self>) {
        let visible: Vec<_> = self
            .file_rows
            .iter()
            .enumerate()
            .filter_map(|(row, item)| {
                if let FileRow::File(i, _) = item {
                    Some((row, *i))
                } else {
                    None
                }
            })
            .collect();
        if visible.is_empty() {
            return;
        }
        let position = visible
            .iter()
            .position(|(_, i)| Some(*i) == self.selected_file);
        let next = position
            .map(|p| (p as isize + delta).clamp(0, visible.len() as isize - 1) as usize)
            .unwrap_or(if delta < 0 { visible.len() - 1 } else { 0 });
        let (row, index) = visible[next];
        self.activate_log_preview();
        self.load_file(index, cx);
        self.files_scroll.scroll_to_item(row, ScrollStrategy::Top);
    }
    pub(super) fn set_files_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        self.collapsed_files.clear();
        self.collapsed_directories.clear();
        if !expanded {
            if !self.local && self.log_files_options.directory {
                self.collapsed_directories.insert((Area::Commit, PathBuf::new()));
            }
            for file in self.files.iter() {
                if self.local && !self.collapsed_files.contains(&file.area) {
                    self.collapsed_files.push(file.area);
                }
                for directory in file
                    .path
                    .ancestors()
                    .skip(1)
                    .filter(|p| !p.as_os_str().is_empty())
                {
                    self.collapsed_directories
                        .insert((file.area, directory.to_path_buf()));
                }
            }
        }
        self.rebuild_files();
        // Retain the review target; expanding restores the same selected file.
        cx.notify();
    }
    pub(super) fn branch_filter_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_0()
            .child(self.branch_menu(cx))
            .when(self.branch.is_some(), |v| {
                v.child(
                    Button::new("clear-branch-filter")
                        .ghost()
                        .small()
                        .icon(jb_icon("closeSmall.svg"))
                        .tooltip(t("Clear branch filter"))
                        .on_click(cx.listener(|s, _, _, cx| s.select_branch(None, cx))),
                )
            })
    }
    pub(super) fn date_filter_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_0()
            .child(self.date_menu(cx))
            .when(!self.date_filter.is_empty(), |v| {
                v.child(
                    Button::new("clear-date-filter")
                        .ghost()
                        .small()
                        .icon(jb_icon("closeSmall.svg"))
                        .tooltip(t("Clear date filter"))
                        .on_click(cx.listener(|s, _, _, cx| s.set_date_filter(None, cx))),
                )
            })
    }
    pub(super) fn filter_overflow(
        &self,
        include_branch: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        Button::new("log-filter-overflow")
            .ghost()
            .small()
            .w(px(28.))
            .flex_shrink_0()
            .icon(jb_icon("chevronRight_dark.svg"))
            .tooltip(t("Filters"))
            .accessibility_label(t("Filters"))
            .dropdown_menu(move |mut menu, _, _| {
                let hit_owner = owner.clone();
                menu = menu
                    .min_w(px(180.))
                    .label(t("Filters"))
                    .close_on_confirm(false)
                    .child_popup_hit_test(move |position, cx| {
                        hit_owner.upgrade().is_some_and(|view| {
                            let s = view.read(cx);
                            s.user_menu.read(cx).contains_popup_position(position)
                                || s.paths_menu.read(cx).contains_popup_position(position)
                        })
                    });
                for filter in 0..4 {
                    if filter == 0 && !include_branch {
                        continue;
                    }
                    let owner = owner.clone();
                    menu = menu.item(PopupMenuItem::element(move |_, cx| {
                        owner
                            .update(cx, |s, cx| match filter {
                                0 => s.branch_filter_control(cx).into_any_element(),
                                1 => s.user_menu.clone().into_any_element(),
                                2 => s.date_filter_control(cx).into_any_element(),
                                _ => s.paths_menu.clone().into_any_element(),
                            })
                            .unwrap_or_else(|_| div().into_any_element())
                    }));
                }
                menu
            })
    }
    pub(super) fn files_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_diff = self
            .selected_file
            .is_some_and(|i| self.files.get(i).is_some());
        let has_tree = !self.files.is_empty() && (self.local || self.log_files_options.directory);
        h_flex()
            .h(px(LOG_TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .px(px(4.))
            .gap(px(2.))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                Button::new("files-show-diff")
                    .ghost()
                    .small()
                    .w(px(28.))
                    .h(px(28.))
                    .icon(jb_icon("diff_dark.svg"))
                    .tooltip(t("Show Diff · Ctrl+D"))
                    .disabled(!can_diff)
                    .on_click(cx.listener(|s, _, w, cx| s.open_diff_tab(w, cx))),
            )
            .when(!self.local, |bar| bar.child(self.files_view_options(cx)))
            .child(div().flex_1())
            .child(
                Button::new("files-expand-all")
                    .ghost()
                    .small()
                    .w(px(28.))
                    .h(px(28.))
                    .icon(jb_icon("expandAll_dark.svg"))
                    .tooltip(t("Expand All"))
                    .disabled(!has_tree)
                    .on_click(cx.listener(|s, _, _, cx| s.set_files_expanded(true, cx))),
            )
            .child(
                Button::new("files-collapse-all")
                    .ghost()
                    .small()
                    .w(px(28.))
                    .h(px(28.))
                    .icon(jb_icon("collapseAll_dark.svg"))
                    .tooltip(t("Collapse All"))
                    .disabled(!has_tree)
                    .on_click(cx.listener(|s, _, _, cx| s.set_files_expanded(false, cx))),
            )
    }
    pub(super) fn files(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection_color = row_selection(&self.files_focus, window);
        let row_height = tree_row_height(cx);
        let files = self.files.clone();
        let flat = !self.local && !self.log_files_options.directory;
        let rows = self.file_rows.clone();
        let selected = self.selected_file;
        let owner = cx.entity().downgrade();
        let file_list = v_flex()
            .size_full()
            .min_h_0()
            .bg(rgb(PANEL))
            .child(self.files_toolbar(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .track_focus(&self.files_focus)
                    .key_context("Files")
                    .on_action(cx.listener(|s, _: &ToggleLogDirectory, _, cx| s.toggle_log_directory(cx)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, _, w, cx| s.files_focus.focus(w, cx)),
                    )
                    .child(
                        uniform_list("files", rows.len(), move |range, _, _| {
                            range
                                .map(|row| {
                                    let (i, depth) = match &rows[row] {
                                        FileRow::File(i, depth) => (*i, *depth),
                                        FileRow::Folder(area, path, depth, count, expanded) => {
                                            let area = *area;
                                            let path = path.clone();
                                            let owner = owner.clone();
                                            return h_flex()
                                                .id(row)
                                                .w_full()
                                                .h(row_height)
                                                .pr_2()
                                                .pl(px(TREE_INSET
                                                    + *depth as f32 * TREE_INDENT
                                                    + if area == Area::Commit { 0. } else { 24. }))
                                                .gap(px(4.))
                                                .child(jb_icon(if *expanded {
                                                    "chevronDown_dark.svg"
                                                } else {
                                                    "chevronRight_dark.svg"
                                                }))
                                                .child(img("jb/folder_dark_4x.svg").size(px(16.)))
                                                .child(
                                                    div().truncate().child(
                                                        path.file_name()
                                                            .unwrap_or_default()
                                                            .to_string_lossy()
                                                            .into_owned(),
                                                    ),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(rgb(MUTED))
                                                        .child(if path.as_os_str().is_empty() { log_file_count(*count) } else { count.to_string() }),
                                                )
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        let key = (area, path.clone());
                                                        if !s.collapsed_directories.remove(&key) {
                                                            s.collapsed_directories.insert(key);
                                                        }
                                                        s.rebuild_files();
                                                        cx.notify();
                                                    });
                                                })
                                                .into_any_element();
                                        }
                                        FileRow::Group(area, count, expanded) => {
                                            let (area, count, expanded) =
                                                (*area, *count, *expanded);
                                            let owner = owner.clone();
                                            return h_flex()
                                                .id(row)
                                                .w_full()
                                                .h(row_height)
                                                .px_2()
                                                .gap(px(4.))
                                                .child(
                                                    jb_icon(if expanded {
                                                        "chevronDown_dark.svg"
                                                    } else {
                                                        "chevronRight_dark.svg"
                                                    })
                                                    .size(px(16.)),
                                                )
                                                .child(area.label())
                                                .child(
                                                    div()
                                                        .text_color(rgb(MUTED))
                                                        .child(count.to_string()),
                                                )
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        if s.collapsed_files.contains(&area) {
                                                            s.collapsed_files
                                                                .retain(|a| *a != area);
                                                        } else {
                                                            s.collapsed_files.push(area);
                                                        }
                                                        s.rebuild_files();
                                                        cx.notify();
                                                    });
                                                })
                                                .into_any_element();
                                        }
                                    };
                                    let f = &files[i];
                                    let owner = owner.clone();
                                    h_flex()
                                        .relative()
                                        .id(row)
                                        .w_full()
                                        .h(row_height)
                                        .px_2()
                                        .gap(px(4.))
                                        .pl(px(TREE_INSET + 20.
                                            + depth as f32 * TREE_INDENT
                                            + if f.area == Area::Commit { 0. } else { 24. }))
                                        .when(selected == Some(i), |d| {
                                            d.child(tree_selection(selection_color))
                                        })
                                        .tooltip({
                                            let label = format!(
                                                "{} · {}\n{}",
                                                f.area.label(),
                                                f.status,
                                                f.path.display()
                                            );
                                            move |window, cx| {
                                                Tooltip::new(label.clone()).build(window, cx)
                                            }
                                        })
                                        .child(
                                            img(file_icon(&f.path)).size(px(16.)).flex_shrink_0(),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .flex_1()
                                                .text_color(rgb(change_color(f)))
                                                .child(
                                                    f.path
                                                        .file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy()
                                                        .into_owned(),
                                                ),
                                        )
                                        .when(flat, |row| row.child(div().truncate().text_color(rgb(MUTED)).child(f.path.parent().unwrap_or(std::path::Path::new("")).to_string_lossy().into_owned())))
                                        .on_click(move |event, window, cx| {
                                            let _ = owner.update(cx, |s, cx| {
                                                s.activate_log_preview();
                                                s.load_file(i, cx);
                                                if event.click_count() == 2 {
                                                    s.open_diff_tab(window, cx);
                                                }
                                            });
                                        })
                                        .into_any_element()
                                })
                                .collect()
                        })
                        .size_full()
                        .track_scroll(&self.files_scroll),
                    )
                    .child(Scrollbar::vertical(&self.files_scroll))
                    .when(self.files.is_empty() && !self.busy, |pane| pane.child(div().absolute().inset_0().flex().items_center().justify_center().p_4().overflow_hidden().text_color(rgb(MUTED)).child(div().w_full().min_w_0().whitespace_normal().text_center().child(t(self.empty_diff_message())))))
                    // Selection clears the previous tree immediately; keep visible
                    // feedback in the changes pane even when the Diff lives in a tab.
                    .when(self.busy, |pane| {
                        pane.child(
                            div()
                                .absolute()
                                .inset_0()
                                .p_4()
                                .bg(rgb(PANEL))
                                .text_color(rgb(MUTED))
                                .child(t("Reading Git data…")),
                        )
                    }),
            );
        if self.local || !self.log_files_options.details {
            return file_list.into_any_element();
        }
        let detail_commit = self.selected.and_then(|i| self.commits.get(i)).cloned();
        let details = v_flex()
            .relative()
            .overflow_hidden()
            .bg(rgb(PANEL))
            .size_full()
            .min_h_0()
            .min_w_0()
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                v_flex()
                    .id(("commit-details", self.selected.unwrap_or_default()))
                    .track_scroll(&self.details_scroll)
                    .size_full()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .px(px(14.))
                    .pt(px(14.))
                    .pb(px(14.))
                    .gap_0()
                    .children(self.selection_details.iter().enumerate().map(
                        |(i, (refs, detail, branches))| {
                            v_flex()
                                .id(("selected-commit-detail", i))
                                .w_full()
                                .flex_shrink_0()
                                .gap_2()
                                .pb_3()
                                .mb_3()
                                .border_b_1()
                                .border_color(rgb(BORDER))
                                .child(
                                    TextView::html(
                                        format!("multi-message-{i}"),
                                        detail.message.clone(),
                                    )
                                    .font_family("JetBrains Mono")
                                    .text_size(cx.theme().font_size)
                                    .selectable(true),
                                )
                                .child(
                                    gpui_kit::base::TextView::html(
                                        format!("multi-metadata-{i}"),
                                        detail.metadata[usize::from(i18n::is_chinese())].clone(),
                                    )
                                    .text_size(cx.theme().font_size)
                                    .style(
                                        gpui_kit::base::TextViewStyle::default()
                                            .with_foreground(rgb(FG).into())
                                            .with_link(rgb(0x71a1fe).into())
                                            .with_selection(rgb(SELECT).into()),
                                    )
                                    .selectable(true),
                                )
                                .child(commit_refs(refs))
                                .child(branches.clone())
                        },
                    ))
                    .when(
                        self.log_selection.ordered(&self.commit_ids()).len() > 50,
                        |v| v.child(div().child(t("Showing the first 50 commits"))),
                    )
                    .child(
                        TextView::html("commit-message", self.message.clone())
                            .mb(px(20.))
                            .w_full()
                            .flex_shrink_0()
                            .font_family("JetBrains Mono")
                            .text_size(cx.theme().font_size)
                            .style(TextViewStyle::default().paragraph_gap(rems(0.)))
                            .selectable(true),
                    )
                    .child(
                        gpui_kit::base::TextView::html(
                            "commit-metadata",
                            self.commit_metadata[usize::from(i18n::is_chinese())].clone(),
                        )
                        .mb(px(10.))
                        .w_full()
                        .flex_shrink_0()
                        .font_family(cx.theme().font_family.clone())
                        .text_size(cx.theme().font_size)
                        .style(
                            gpui_kit::base::TextViewStyle::default()
                                .with_foreground(rgb(FG).into())
                                .with_link(rgb(0x71a1fe).into())
                                .with_selection(rgb(SELECT).into())
                                .with_paragraph_gap(rems(0.)),
                        )
                        .selectable(true),
                    )
                    .children(
                        detail_commit
                            .as_ref()
                            .map(|commit| commit_refs(&commit.refs)),
                    )
                    .children(self.containing_branches.clone())
                    .context_menu(move |mut menu, _, _| {
                        if let Some(commit) = &detail_commit {
                            let hash = commit.id.clone();
                            menu = menu
                                .item(PopupMenuItem::new("Copy Commit Hash").on_click(
                                    move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            hash.clone(),
                                        ));
                                    },
                                ))
                                .separator()
                                .label(t("Parents"));
                            if commit.shallow_boundary {
                                menu = menu.label(t("Unavailable (shallow boundary)"));
                            } else if commit.parents.is_empty() {
                                menu = menu.label(t("Root commit"));
                            } else {
                                for parent in &commit.parents {
                                    let parent = parent.clone();
                                    menu = menu.item(PopupMenuItem::new(parent.clone()).on_click(
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                parent.clone(),
                                            ));
                                        },
                                    ));
                                }
                            }
                        }
                        menu
                    }),
            )
            .when(self.selected.is_none() && self.selection_details.is_empty(), |pane| pane.child(div().absolute().inset_0().flex().items_center().justify_center().text_color(rgb(MUTED)).child(t("Commit details"))))
            .child(
                Scrollbar::vertical(&self.details_scroll)
                    .mode(gpui_kit::component::scroll::ScrollbarMode::Always),
            );
        v_resizable("files-details")
            .with_state(&self.details_split)
            .on_resize(cx.listener(|s, state: &Entity<ResizableState>, _, cx| {
                let sizes = state.read(cx).sizes();
                if let [files, details] = sizes.as_slice() {
                    let total = files.as_f32() + details.as_f32();
                    if total > 0. {
                        s.files_fraction = (files.as_f32() / total).clamp(0.1, 0.9);
                        if let Err(error) =
                            layout_preferences::save_files_fraction(s.files_fraction)
                        {
                            eprintln!("Cannot save layout preference: {error}");
                        }
                    }
                }
            }))
            .child(
                resizable_panel()
                    .flex_basis(relative(self.files_fraction))
                    .size_range(px(48.)..px(4000.))
                    .child(file_list),
            )
            .child(
                resizable_panel()
                    .flex_basis(relative(1. - self.files_fraction))
                    .size_range(px(60.)..px(4000.))
                    .child(details),
            )
            .into_any_element()
    }
}
