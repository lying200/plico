//! Main-window repository coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn read_primary_recovery(repo: &Repository, token: &Cancel) -> (bool, bool, bool) {
        let merge_recovery = repo
            .editor_directory(token)
            .and_then(|dir| plico::merge_saved::read(&dir))
            .map(|record| record.is_some())
            .unwrap_or(true);
        let revert_recovery =
            plico::revert_operation::Recovery::record_exists(repo, token).unwrap_or(true);
        let revert_saved =
            plico::revert_operation::SavedChanges::record_exists(repo, token).unwrap_or(true);
        (merge_recovery, revert_recovery, revert_saved)
    }
    pub(super) fn record_recent_project(&mut self) {
        if !self.recent_recorded {
            self.recent_recorded = true;
            match plico::recent_projects::opened(&self.repo.path) {
                Ok(paths) => self.recent_projects = paths,
                Err(error) => {
                    self.status_message =
                        Some(format!("{}: {error}", t("Could not save recent projects")))
                }
            }
        }
    }
    pub(super) fn reset_log_presentation(&mut self) {
        self.graph_hover = None;
        self.graph_print_token.cancel();
        self.graph_print_generation += 1;
        self.graph_print_task = Task::ready(());
        self.graph_print_loading = false;
        self.adjacent_token.cancel();
        self.adjacent_menu = None;
        self.adjacent_subscription = None;
        self.error = None;
        self.preview_error = None;
        self.navigation_loading = None;
        self.navigation_notice = None;
    }
    pub(super) fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() {
            return;
        }
        let request = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Git repository".into()),
        });
        cx.spawn_in(window, async move |this, wx| {
            let Some(path) = request
                .await
                .ok()
                .and_then(|r| r.ok())
                .flatten()
                .and_then(|v| v.into_iter().next())
            else {
                return;
            };
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| s.choose_project_window(path, window, cx));
            });
        })
        .detach();
    }
    pub(super) fn choose_project_window(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.settings_open {
            return;
        }
        if self.repo.path.as_os_str().is_empty() {
            if !path.is_dir() {
                self.error = Some(t("Project folder is unavailable").into());
                cx.notify();
                return;
            }
            let old = window.bounds();
            let mut bounds = Bounds::new(
                point(
                    old.origin.x + (old.size.width - px(1440.)) / 2.,
                    old.origin.y + (old.size.height - px(900.)) / 2.,
                ),
                size(px(1440.), px(900.)),
            );
            if let Some(display) = window.display(cx) {
                let screen = display.bounds();
                bounds.size.width = bounds.size.width.min(screen.size.width);
                bounds.size.height = bounds.size.height.min(screen.size.height);
                bounds.origin.x = bounds
                    .origin
                    .x
                    .max(screen.origin.x)
                    .min(screen.origin.x + screen.size.width - bounds.size.width);
                bounds.origin.y = bounds
                    .origin
                    .y
                    .max(screen.origin.y)
                    .min(screen.origin.y + screen.size.height - bounds.size.height);
            }
            match crate::open_project_window_at(path, self.limit, None, Some(bounds), cx) {
                Ok(()) => window.remove_window(),
                Err(error) => {
                    self.error = Some(error.to_string());
                    cx.notify();
                }
            }
            return;
        }
        let owner = cx.entity().downgrade();
        let limit = self.limit;
        window.open_dialog(cx, move |dialog, _, _| {
            let current = owner.clone();
            let new_owner = owner.clone();
            let current_path = path.clone();
            let new_path = path.clone();
            dialog
                .title(t("Open Project"))
                .w(px(480.))
                .centered(true)
                .child(div().child(t("Where would you like to open the project?")))
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .truncate()
                        .child(project_display_path(&path)),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogAction::new().child(
                                Button::new("project-this-window")
                                    .primary()
                                    .label(t("This Window")),
                            ),
                        )
                        .child(
                            Button::new("project-new-window")
                                .label(t("New Window"))
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    if let Err(error) =
                                        crate::open_project_window(new_path.clone(), limit, cx)
                                    {
                                        let _ = new_owner.update(cx, |s, cx| {
                                            s.error = Some(error.to_string());
                                            cx.notify();
                                        });
                                    }
                                }),
                        )
                        .child(
                            gpui_kit::component::dialog::DialogClose::new()
                                .trigger(|b| b.label(t("Cancel"))),
                        ),
                )
                .on_ok(move |_, w, cx| {
                    let _ =
                        current.update(cx, |s, cx| s.open_repository(current_path.clone(), w, cx));
                    true
                })
        });
    }
    pub(super) fn open_repository(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| s.open_repository(path, w, cx));
            return;
        }
        if self.operation.busy() {
            return;
        }
        if !path.is_dir() {
            self.error = Some(t("Project folder is unavailable").into());
            cx.notify();
            return;
        }
        self.source_token.cancel();
        self.source_generation += 1;
        self.source_task = Task::ready(());
        self.recent_recorded = false;
        self.welcome = false;
        self.dismiss_repository_menu(cx);
        self.branch_popup.reset_project(path.clone());
        window.set_window_title(&format!(
            "Plico – {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
        self.details_cache.clear();
        self.repo = Repository::new(path);
        self.project_roots =
            cx.new(|cx| super::project_roots::ProjectRoots::open(self.repo.path.clone(), cx));
        self.bind_project_roots(cx);
        self.show_long_edges = layout_preferences::load_log_long_edges(&self.repo.path);
        self.show_log_root_names = layout_preferences::load_log_root_names(&self.repo.path);
        self.log_selection = Default::default();
        self.aggregate_chain = None;
        self.selection_details.clear();
        self.selection_branches = None;
        self.rebase_diagnostic = None;
        self.update_diagnostic = None;
        self.sequence_diagnostic = None;
        self.sequence_retry = None;
        self.sequence_conflict = None;
        self.squash_undo = None;
        self.operation_state = None;
        self.revert_recoveries.clear();
        self.merge_recovery_available = false;
        self.merged_branch = None;
        self.pending_branch_merge = None;
        self.revert_recovery_available = false;
        self.revert_saved_available = false;
        self.paths_menu
            .update(cx, |menu, cx| menu.reset(self.repo.path.clone(), cx));
        self.user_menu
            .update(cx, |menu, cx| menu.reset(self.repo.clone(), cx));
        self.branch_filter.clear();
        self.selected_tree_ref = None;
        self.branch_selection = Default::default();
        self.branch_rows = Arc::default();
        self.collapsed_branches.clear();
        self.history_count.invalidate();
        self.snapshot = None;
        self.log = Default::default();
        self.pending_project_reload = None;
        self.selected = None;
        self.selected_file = None;
        self.pending_preview_file = None;
        self.local = false;
        self.saved_changes_available = false;
        self.stash_open = false;
        self.stash_view = None;
        self.worktrees = None;
        self.worktree_open_task = Task::ready(());
        self.worktrees_visible = false;
        self.commit_stash_selected = false;
        self.comparison = None;
        self.comparison_visible = false;
        self.comparison_preview = None;
        self.local_preview = None;
        self.local_view = None;
        self.local_visible = false;
        self.project_view = None;
        self.project_visible = false;
        self.file_tabs.clear();
        self.documents = Self::new_documents(cx);
        self.editor_tabs.clear();
        self.tabs.clear();
        self.branch_compare_request = Default::default();
        self.branch_compare_tabs.clear();
        self.close_settings(cx);
        self.collapsed_files.clear();
        self.collapsed_directories.clear();
        self.history_scroll = UniformListScrollHandle::new();
        self.tree_scroll = UniformListScrollHandle::new();
        self.history_horizontal = ScrollHandle::new();
        self.files_scroll = UniformListScrollHandle::new();
        self.filter_search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.text_filter = None;
        self.date_filter = Default::default();
        self.branch_search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.limit = git::HISTORY_PAGE;
        self.refresh(false, cx);
        self.start_auto_refresh(window, cx);
        self.focus.focus(window, cx);
    }
    pub(super) fn refresh(&mut self, more: bool, cx: &mut Context<Self>) {
        self.refresh_impl(more, false, None, cx);
    }
    pub(super) fn refresh_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.active_branch_compare_index() {
            self.branch_compare_tabs[index].update(cx, |view, cx| view.refresh(true, cx));
            return;
        }
        if self.stash_open {
            if let Some(view) = &self.stash_view {
                view.update(cx, |s, cx| s.refresh(cx));
            }
        }
        if let Some(tab) = self.active_diff_index().and_then(|i| self.tabs.get(i)) {
            let callback = self.diff_source_callback(cx);
            tab.update(cx, |tab, cx| {
                tab.source_callback = Some(callback);
                tab.activate(window, cx);
            });
            return;
        }
        // During a refresh the displayed file list is temporarily empty. A
        // second refresh must not replace the in-flight selection with None.
        if self.log_query.busy() || self.log_query.history_loading() {
            return;
        }
        let user_active = self.user_menu.read(cx).active();
        self.user_menu.update(cx, |menu, cx| menu.refresh(cx));
        if user_active {
            return;
        }
        self.refresh_impl(false, true, None, cx);
    }
    pub(super) fn previous_window(&mut self, cx: &mut Context<Self>) {
        if self.log_query.busy() || self.log_query.history_loading() || self.log.offset() == 0 {
            return;
        }
        let offset = self.log.offset().saturating_sub(git::HISTORY_MAX);
        self.reload_history(false, false, Some(offset), cx);
    }
    /// A filter is a new Log query, not a repository-change notification.
    pub(super) fn refresh_log_filter(&mut self, cx: &mut Context<Self>) {
        self.history_count.invalidate();
        self.reload_history(false, false, None, cx);
    }
    pub(super) fn refresh_impl(
        &mut self,
        more: bool,
        preserve: bool,
        requested_offset: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if !more {
            self.project_roots.update(cx, |roots, cx| roots.refresh(cx));
            for view in &self.branch_compare_tabs {
                view.update(cx, |view, cx| view.refresh(false, cx));
            }
            self.history_count.invalidate();
            self.details_cache.clear();
            if let Some(view) = &self.local_view {
                view.update(cx, |s, cx| s.refresh(cx));
            }
        }
        self.reload_history(more, preserve, requested_offset, cx);
    }
    fn reload_history(
        &mut self,
        more: bool,
        preserve: bool,
        requested_offset: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if !self.local
            && (self.log.project_window().is_some()
                || self.project_roots.read(cx).has_multiple_roots())
        {
            self.reload_project_history(more, preserve, requested_offset, cx);
            return;
        }
        let preferred = self.preferred_log_file();
        self.pending_preview_file = preferred.clone();
        self.aggregate_chain = None;
        let initial_history = self.snapshot.is_none();
        self.reset_log_presentation();
        let (generation, token) = self.log_query.begin(log_query::Kind::History);
        self.files = Arc::default();
        self.rebuild_files(cx);
        self.selected_file = None;
        self.message.clear();
        self.commit_metadata = Default::default();
        self.containing_branches = None;
        self.reword_gate = None;
        self.base = None;
        let repo = self.repo.clone();
        let branches = self.branch_filter.clone();
        let text_filter = self.text_filter.clone();
        let local = self.local;
        let show_long_edges = self.show_long_edges;
        let next_window = more && self.limit >= git::HISTORY_MAX;
        let offset = if let Some(offset) = requested_offset {
            offset
        } else if next_window {
            self.log.offset() + self.log.commits().len()
        } else if more || preserve {
            self.log.offset()
        } else {
            0
        };
        let start = if next_window {
            self.log.end().clone()
        } else if more {
            self.log.start().clone()
        } else {
            graph::Layout::default()
        };
        let limit = if requested_offset.is_some() {
            git::HISTORY_MAX
        } else if next_window {
            git::HISTORY_PAGE
        } else if more {
            (self.limit + git::HISTORY_PAGE).min(git::HISTORY_MAX)
        } else {
            self.limit
        };
        let layout_start = if preserve || requested_offset.is_some() {
            None
        } else {
            Some(start.clone())
        };
        let fixed = if more || requested_offset.is_some() {
            self.snapshot.as_ref().map(|s| s.tips.clone())
        } else {
            None
        };
        let filtering = text_filter.is_some();
        self.log_query.track(
            generation,
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_spawn(async move {
                        let state = repo.operation_state(&token);
                        let recovery = Self::read_primary_recovery(&repo, &token);
                        let result = if local {
                            repo.local_changes(&token).map(|f| (None, Some(f)))
                        } else if let Some(filter) = &text_filter {
                            history::read_branches_filtered(
                                &repo,
                                &branches,
                                offset,
                                limit,
                                fixed.as_deref(),
                                filter,
                                &token,
                            )
                            .map(|window| (Some(window), None))
                        } else {
                            history::read_branches(
                                &repo,
                                &branches,
                                offset,
                                limit,
                                fixed.as_deref(),
                                layout_start,
                                &token,
                            )
                            .map(|window| (Some(window), None))
                        };
                        let mut graph_model = None;
                        let result = result.and_then(|(window, local)| {
                            let window = window
                                .map(|mut window: history::Window| {
                                    let model = Arc::new(plico::graph_print::Model::new(
                                        &window.rows,
                                        &token,
                                    )?);
                                    window.rows = model.project(show_long_edges, &token)?;
                                    graph_model = Some(model);
                                    Ok::<_, String>(window)
                                })
                                .transpose()?;
                            Ok((window, local))
                        });
                        (result, state, graph_model, recovery)
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if !this.log_query.finish(generation) {
                        return;
                    }
                    this.operation_state = result.1.ok();
                    this.merge_recovery_available = result.3.0;
                    this.revert_recovery_available = result.3.1;
                    this.revert_saved_available = result.3.2;
                    match result.0 {
                        Ok((
                            Some(history::Window {
                                snapshot: s,
                                rows,
                                start,
                                end,
                                ..
                            }),
                            _,
                        )) => {
                            this.record_recent_project();
                            if offset != this.log.offset() {
                                this.history_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            }
                            if preserve && offset == this.log.offset() {
                                let root = Arc::new(this.repo.path.clone());
                                super::log_viewport::restore(
                                    &this.history_scroll,
                                    &this.commit_ids(),
                                    &s.commits
                                        .iter()
                                        .map(|commit| plico::project_log::CommitId {
                                            root: root.clone(),
                                            hash: commit.id.clone(),
                                        })
                                        .collect::<Vec<_>>(),
                                    offset,
                                    log_row_height(cx),
                                );
                            }
                            this.limit = limit;
                            let query = plico::project_log::Query {
                                repository: this.repo.clone(),
                                branches: this.branch_filter.clone(),
                                filter: this.text_filter.clone(),
                            };
                            let (display, s) = log_display::Display::single(
                                query,
                                history::Window {
                                    snapshot: s,
                                    rows,
                                    start,
                                    end,
                                    projected_parents: None,
                                },
                                offset,
                                result.2,
                            );
                            this.log = display;
                            this.pending_project_reload = None;
                            if this.squash_undo.as_ref().is_some_and(|undo| {
                                s.head_id.as_deref() != Some(undo.result_head())
                            }) {
                                this.squash_undo = None;
                            }
                            this.snapshot = Some(s);
                            this.update_history_count(cx);
                            this.rebuild_branches(cx);
                            this.selected = None;
                            let ids = this.commit_ids();
                            this.log_selection.reconcile(&ids);
                            if this.log_selection.ordered(&ids).is_empty()
                                && !ids.is_empty()
                                && !preserve
                                && initial_history
                            {
                                this.log_selection.select(&ids, 0, false, false);
                            }
                            this.pending_preview_file = preferred;
                            this.load_log_selection(cx);
                        }
                        Ok((_, Some(f))) => {
                            this.files = Arc::new(f);
                            this.prune_directory_state();
                            this.rebuild_files(cx);
                            this.base = Some(i18n::ComparisonTitle::Label(
                                "Local changes · staged and unstaged are separate comparisons",
                            ));
                            this.pending_preview_file = None;
                            if let Some(index) = this.preferred_file_index(preferred.as_ref()) {
                                this.load_file(index, cx);
                            }
                        }
                        Err(e) => {
                            this.error = Some(e);
                            if filtering {
                                this.log = Default::default();
                                this.pending_project_reload = None;
                                this.selected = None;
                                this.snapshot = None;
                            }
                        }
                        _ => {}
                    }
                    cx.notify();
                });
            }),
        );
        cx.notify();
    }
}
