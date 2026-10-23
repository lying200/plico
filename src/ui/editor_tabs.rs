//! Main-window editor tabs coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn subscribe_diff_navigation(&self, tab: &Entity<DiffTab>, cx: &mut Context<Self>) {
        cx.subscribe(tab, |s, _, event: &crate::diff_tab::FileChanged, cx| {
            let target = &event.0;
            if let Some(view) = &s.local_view {
                view.update(cx, |v, cx| v.select_target(target, cx));
            }
            if let Some(view) = &s.stash_view {
                view.update(cx, |v, cx| v.select_target(target, cx));
            }
            let index = s
                .aggregate_chain
                .as_ref()
                .and_then(|chain| chain.fork_for(target))
                .map(|chain| chain.index())
                .or_else(|| {
                    if s.aggregate_chain.is_some() || target.identity().combined.is_some() {
                        return None;
                    }
                    let commit = if s.local {
                        None
                    } else {
                        s.selected.and_then(|i| s.commits.get(i)).cloned()
                    };
                    s.files.iter().position(|change| {
                        diff_target::Target::new(s.repo.clone(), commit.clone(), change.clone())
                            .is_ok_and(|candidate| candidate.identity() == target.identity())
                    })
                });
            if let Some(index) = index {
                s.selected_file = Some(index);
                for parent in target.change().path.ancestors().skip(1) {
                    s.collapsed_directories
                        .remove(&(target.change().area, parent.to_path_buf()));
                }
                s.rebuild_files();
                if let Some(row) = s
                    .file_rows
                    .iter()
                    .position(|r| matches!(r, FileRow::File(i, _) if *i == index))
                {
                    s.files_scroll.scroll_to_item(row, ScrollStrategy::Top);
                }
            }
            cx.notify();
        })
        .detach();
    }
    pub(super) fn compare_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids = self.commit_ids();
        let selected = self.log_selection.ordered(&ids);
        if selected.len() != 2 {
            return;
        }
        let before = selected[1].to_owned();
        let after = selected[0].to_owned();
        self.close_comparison_preview(cx);
        let view = cx.new(|cx| {
            crate::compare_view::CompareView::new(self.repo.clone(), before, after, window, cx)
        });
        cx.subscribe_in(
            &view,
            window,
            |s, _, event: &crate::compare_view::OpenDiff, w, cx| {
                let index = s.comparison_preview_index(cx).unwrap_or_else(|| {
                    let tab = cx.new(|_| {
                        let mut tab = DiffTab::new(event.0.clone(), event.1.clone());
                        tab.documents = s.documents.clone();
                        tab
                    });
                    s.comparison_preview = Some(tab.downgrade());
                    cx.subscribe(&tab, |s, tab, event: &crate::diff_tab::FileChanged, cx| {
                        if s.comparison_preview
                            .as_ref()
                            .and_then(|p| p.upgrade())
                            .is_some_and(|p| p.entity_id() == tab.entity_id())
                        {
                            if let Some(view) = &s.comparison {
                                view.update(cx, |v, cx| v.select_target(&event.0, cx));
                            }
                        }
                    })
                    .detach();
                    s.subscribe_diff_navigation(&tab, cx);
                    cx.observe(&tab, |_, _, cx| cx.notify()).detach();
                    s.insert_editor_tab(tab.entity_id());
                    s.tabs.push(tab);
                    s.tabs.len() - 1
                });
                s.tabs[index].update(cx, |tab, cx| {
                    tab.replace_target(event.0.clone(), event.1.clone(), cx)
                });
                s.activate_diff_tab(index, w, cx);
            },
        )
        .detach();
        cx.subscribe_in(
            &view,
            window,
            |s, _, event: &crate::compare_view::SelectionChanged, w, cx| {
                let Some(index) = s.comparison_preview_index(cx) else {
                    return;
                };
                if s.tabs[index].read(cx).target.identity() == event.0.identity() {
                    return;
                }
                let active = s.active_diff_index() == Some(index);
                s.tabs[index].update(cx, |tab, cx| {
                    tab.replace_target(event.0.clone(), event.1.clone(), cx);
                    if active {
                        tab.activate_with_focus(false, w, cx);
                    }
                });
            },
        )
        .detach();
        cx.subscribe_in(
            &view,
            window,
            |s, _, event: &crate::compare_view::Reloaded, w, cx| {
                let Some(index) = s.comparison_preview_index(cx) else {
                    return;
                };
                let active = s.active_diff_index() == Some(index);
                s.tabs[index].update(cx, |tab, cx| match &event.0 {
                    Some((target, chain)) => {
                        tab.replace_target(target.clone(), chain.clone(), cx);
                        if active {
                            tab.activate_with_focus(false, w, cx);
                        }
                    }
                    None => tab.invalidate_preview(
                        "Comparison unavailable. Refresh the Changes panel to retry.",
                        cx,
                    ),
                });
            },
        )
        .detach();
        cx.subscribe(&view, |s, _, _: &crate::compare_view::Close, cx| {
            s.close_comparison_preview(cx);
            s.comparison = None;
            s.comparison_visible = false;
            cx.notify();
        })
        .detach();
        self.comparison = Some(view);
        self.comparison_visible = true;
        self.cancel_project_preview(cx);
        self.local_visible = false;
        self.project_visible = false;
        self.stash_open = false;
        cx.notify();
    }
    pub(super) fn comparison_preview_index(&self, _cx: &App) -> Option<usize> {
        let preview = self.comparison_preview.as_ref()?.upgrade()?;
        self.tabs
            .iter()
            .position(|tab| tab.entity_id() == preview.entity_id())
    }
    pub(super) fn close_comparison_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.comparison_preview_index(cx) {
            self.close_diff_tab(index, cx);
        }
        self.comparison_preview = None;
    }
    pub(super) fn open_local(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        if self.local_view.is_none() {
            let repo = self.repo.clone();
            let view = cx.new(|cx| crate::local_view::LocalView::new(repo, window, cx));
            cx.subscribe(&view, |s, _, _: &crate::local_view::PreviewSelected, cx| {
                if !s.local_preview_requested {
                    s.local_preview = None;
                    s.local_preview_requested = true;
                }
                cx.notify();
            })
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, local, event: &crate::local_view::OpenDiff, w, cx| {
                    let index = s
                        .tabs
                        .iter()
                        .position(|t| t.read(cx).target.identity() == event.0.identity())
                        .unwrap_or_else(|| {
                            let scope = local.read(cx).scope.clone();
                            let tab = cx.new(|_| {
                                let mut tab = DiffTab::new(event.0.clone(), event.1.clone());
                                tab.documents = s.documents.clone();
                                tab.commit_scope = Some(scope);
                                tab
                            });
                            s.subscribe_diff_navigation(&tab, cx);
                            cx.observe(&tab, |_, _, cx| cx.notify()).detach();
                            s.insert_editor_tab(tab.entity_id());
                            s.tabs.push(tab);
                            s.tabs.len() - 1
                        });
                    s.activate_diff_tab(index, w, cx);
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, view, event: &crate::local_view::Submit, w, cx| {
                    let view = view.clone();
                    let (push, recover) = (event.0, event.1);
                    let submit = move |s: &mut Self, w: &mut Window, cx: &mut Context<Self>| {
                        s.with_saved_documents(w, cx, move |s, w, cx| {
                            view.update(cx, |v, cx| v.submit_saved(push, recover, w, cx));
                            s.sync_document_lock(cx);
                        });
                    };
                    if recover {
                        submit(s, w, cx);
                    } else {
                        s.with_commit_identity(w, cx, submit);
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, view, _: &crate::local_view::RequestRollback, w, cx| {
                    let view = view.clone();
                    s.with_saved_documents(w, cx, move |s, w, cx| {
                        view.update(cx, |v, cx| v.prepare_rollback_saved(w, cx));
                        s.sync_document_lock(cx);
                    });
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, view, event: &crate::local_view::ExecuteRollback, w, cx| {
                    let view = view.clone();
                    let event = crate::local_view::ExecuteRollback {
                        prepared: event.prepared.clone(),
                        selected: event.selected.clone(),
                        delete_added: event.delete_added,
                    };
                    s.with_saved_documents(w, cx, move |s, w, cx| {
                        view.update(cx, |v, cx| v.rollback_saved(&event, w, cx));
                        s.sync_document_lock(cx);
                    });
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, _, event: &crate::local_view::Changed, w, cx| {
                    s.sync_document_lock(cx);
                    s.sync_worktree(
                        plico::worktree_events::Batch {
                            all: true,
                            index: true,
                            ..Default::default()
                        },
                        true,
                        w,
                        cx,
                    );
                    s.refresh_impl(false, true, None, cx);
                    if event.push {
                        s.open_push(w, cx);
                    }
                },
            )
            .detach();
            cx.observe(&view, |s, _, cx| {
                s.sync_document_lock(cx);
                cx.notify();
            })
            .detach();
            self.local_view = Some(view);
        }
        self.comparison_visible = false;
        self.commit_stash_selected = false;
        self.local_visible = true;
        self.local_preview_requested = true;
        self.project_visible = false;
        self.stash_open = false;
        cx.notify();
    }
    pub(super) fn cancel_project_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = &self.project_view {
            view.update(cx, |v, _| v.cancel_preview());
        }
    }
    pub(super) fn toggle_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        if self.project_view.is_none() {
            let view =
                cx.new(|cx| crate::project_view::ProjectView::new(self.repo.path.clone(), cx));
            cx.subscribe_in(
                &view,
                window,
                |s, source, event: &crate::project_view::OpenRepository, w, cx| {
                    if s.project_visible
                        && s.project_view
                            .as_ref()
                            .is_some_and(|v| v.entity_id() == source.entity_id())
                    {
                        s.choose_project_window(event.0.clone(), w, cx);
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, source, event: &crate::project_view::Open, w, cx| {
                    let current = s.project_view.as_ref().is_some_and(|view| {
                        view.entity_id() == source.entity_id()
                            && (!event.1 || view.read(cx).is_preview_current(event.2))
                    });
                    if s.project_visible && current {
                        s.open_project_file(event.0.clone(), event.1, w, cx);
                    }
                },
            )
            .detach();
            self.project_view = Some(view);
        }
        self.project_visible = !self.project_visible;
        if self.project_visible {
            self.comparison_visible = false;
            self.stash_open = false;
            self.local_visible = false;
        }
        cx.notify();
    }
    pub(super) fn jump_to_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_file_index().and_then(|i| self.file_tabs.get(i)) {
            if let Some(request) = tab.read(cx).history_source_request(cx) {
                self.jump_history_source(request, window, cx);
            }
            return;
        }
        let request = if let Some(tab) = self.active_diff_index().and_then(|i| self.tabs.get(i)) {
            tab.read(cx).source_request(cx)
        } else if let Some(tab) = &self.local_preview {
            tab.read(cx).source_request(cx)
        } else {
            self.selected_file
                .and_then(|i| self.files.get(i))
                .and_then(|change| {
                    let target = if let Some(chain) = &self.aggregate_chain {
                        let mut c = chain.clone();
                        c.select(self.selected_file?)
                    } else {
                        diff_target::Target::new(
                            self.repo.clone(),
                            self.selected.and_then(|i| self.commits.get(i)).cloned(),
                            change.clone(),
                        )
                        .ok()
                    }?;
                    let (original, line) = (change.status.starts_with('D'), 0);
                    Some((target, original, line))
                })
        };
        let Some((target, original, line)) = request else {
            return;
        };
        self.jump_target_source(target, original, line, window, cx);
    }
    pub(super) fn diff_source_callback(
        &self,
        cx: &Context<Self>,
    ) -> crate::diff_tab::SourceCallback {
        let owner = cx.entity().downgrade();
        std::rc::Rc::new(move |(target, original, line), window, cx| {
            let _ = owner.update(cx, |s, cx| {
                if s.repo.path == target.identity().repository {
                    window.close_all_dialogs(cx);
                    s.jump_target_source(target, original, line, window, cx);
                }
            });
        })
    }
    pub(super) fn jump_target_source(
        &mut self,
        target: diff_target::Target,
        original: bool,
        line: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !original
            && target.working_tree_document().is_some()
            && self.documents.get(&target.identity().path).is_some()
        {
            self.source_token.cancel();
            self.source_generation += 1;
            self.open_project_file(target.identity().path.clone(), false, window, cx);
            if let Some(tab) = self.active_file_index().and_then(|i| self.file_tabs.get(i)) {
                tab.update(cx, |t, cx| t.jump_to_line(line, cx));
            }
            return;
        }
        self.source_token.cancel();
        self.source_token = Cancel::default();
        self.source_generation += 1;
        let generation = self.source_generation;
        let token = self.source_token.clone();
        let root = self.repo.path.clone();
        let expected_root = root.clone();
        let path = target.identity().path.clone();
        self.source_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    use gpui_kit::component::diff::{DiffFile, DiffSide};
                    let current = match plico::project_files::Project::new(root)
                        .map_err(|e| format!("{e:?}"))?
                        .read(&path, &token)
                        .map_err(|e| format!("{e:?}"))?
                    {
                        plico::project_files::Content::Text(text) => text,
                        _ => return Err("Source file cannot be displayed as text".to_owned()),
                    };
                    let source = if let Some(contents) = target.read_combined(&token) {
                        let c = contents?;
                        if original { c.before } else { c.after }.unwrap_or_default()
                    } else {
                        let text = target.read_full(&token)?;
                        let files = if target.change().area == git::Area::Untracked {
                            vec![DiffFile::added(target.change().label(), &text)]
                        } else {
                            DiffFile::parse(&text).map_err(|e| e.to_string())?
                        };
                        files
                            .first()
                            .map(|f| {
                                f.source(if original {
                                    DiffSide::Original
                                } else {
                                    DiffSide::Modified
                                })
                                .to_owned()
                            })
                            .unwrap_or_default()
                    };
                    if token.cancelled() {
                        return Err("Cancelled".into());
                    }
                    let mapped =
                        plico::file_history::transfer_range(&source, &current, line..line + 1)?
                            .start;
                    Ok((path, mapped))
                })
                .await;
            let _ = this.update_in(wx, |s, window, cx| {
                if s.source_generation != generation || s.repo.path != expected_root {
                    return;
                }
                match result {
                    Ok((path, line)) => {
                        s.open_project_file(path, false, window, cx);
                        if let Some(tab) = s.active_file_index().and_then(|i| s.file_tabs.get(i)) {
                            tab.update(cx, |t, cx| t.jump_to_line(line, cx));
                        }
                    }
                    Err(error) => {
                        s.error = Some(error);
                        s.status_message = Some(t("Could not jump to source").into());
                    }
                }
                cx.notify();
            });
        });
    }
    pub(super) fn jump_history_source(
        &mut self,
        request: crate::history_view::SourceRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.repository != self.repo.path {
            self.error = Some(t("History belongs to another repository; reopen its file").into());
            cx.notify();
            return;
        }
        self.source_token.cancel();
        self.source_token = Cancel::default();
        self.source_generation += 1;
        let token = self.source_token.clone();
        let generation = self.source_generation;
        let repo = self.repo.clone();
        let expected_root = repo.path.clone();
        self.source_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    let current = match plico::project_files::Project::new(repo.path.clone())
                        .map_err(|e| format!("{e:?}"))?
                        .read(&request.path, &token)
                        .map_err(|e| format!("{e:?}"))?
                    {
                        plico::project_files::Content::Text(text) => text,
                        _ => return Err("Source file cannot be displayed as text".to_owned()),
                    };
                    let line = if let Some(revision) = request.revision {
                        let source = plico::file_history::contents(&repo, &revision, &token)?;
                        plico::file_history::transfer_range(
                            &source,
                            &current,
                            request.line..request.line + 1,
                        )?
                        .start
                    } else {
                        request.line
                    };
                    if token.cancelled() {
                        return Err("Cancelled".into());
                    }
                    Ok((request.path, line))
                })
                .await;
            let _ = this.update_in(wx, |s, window, cx| {
                if generation != s.source_generation || s.repo.path != expected_root {
                    return;
                }
                match result {
                    Ok((path, line)) => {
                        s.open_project_file(path, false, window, cx);
                        window.activate_window();
                        if let Some(tab) = s.active_file_index().and_then(|i| s.file_tabs.get(i)) {
                            tab.update(cx, |t, cx| t.jump_to_line(line, cx));
                        }
                    }
                    Err(e) => {
                        s.error = Some(e);
                        s.status_message = Some(t("Could not jump to source").into());
                    }
                }
                cx.notify();
            });
        });
    }
    pub(super) fn new_documents(cx: &mut Context<Self>) -> crate::editing::Documents {
        let documents = crate::editing::Documents::default();
        let owner = cx.entity().downgrade();
        documents.on_saved(move |cx| {
            let _ = owner.update(cx, |s, cx| {
                if let Some(local) = &s.local_view {
                    local.update(cx, |v, cx| v.refresh(cx));
                }
                cx.notify();
            });
        });
        documents
    }
    pub(super) fn subscribe_file_history(
        &self,
        tab: &Entity<crate::project_view::FileTab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe_in(
            tab,
            window,
            |s, _, request: &crate::history_view::GetRevision, w, cx| {
                s.get_file_revision(request.clone(), w, cx);
            },
        )
        .detach();
        cx.subscribe_in(
            tab,
            window,
            |s, _, event: &crate::markdown_preview::OpenLink, window, cx| {
                s.open_project_file(event.0.clone(), false, window, cx);
            },
        )
        .detach();
        cx.subscribe_in(
            tab,
            window,
            |s, _, event: &crate::source_changes::CommitChunk, w, cx| {
                let event = event.clone();
                s.with_saved_documents(w, cx, move |s, w, cx| s.commit_chunk(event, w, cx));
            },
        )
        .detach();
        cx.subscribe_in(
            tab,
            window,
            |s, _, event: &crate::project_view::HistoryRequested, window, cx| {
                if let Some(selection) = event.selection.clone() {
                    let owner = cx.entity().downgrade();
                    let main_window = window.window_handle();
                    let source_callback = std::rc::Rc::new(
                        move |request: crate::history_view::SourceRequest, cx: &mut App| {
                            let _ = main_window.update(cx, |_, window, cx| {
                                let _ = owner
                                    .update(cx, |s, cx| s.jump_history_source(request, window, cx));
                            });
                        },
                    );
                    if let Err(error) = crate::history_view::open_selection(
                        s.repo.clone(),
                        event.path.clone(),
                        selection,
                        source_callback,
                        cx,
                    ) {
                        s.error = Some(error);
                        cx.notify();
                    }
                    return;
                }
                if let Some(index) = s
                    .file_tabs
                    .iter()
                    .position(|t| t.read(cx).history && t.read(cx).path == event.path)
                {
                    s.activate_editor_neighbor(Some(s.file_tabs[index].entity_id()), window, cx);
                    return;
                }
                let tab = cx.new(|_| {
                    crate::project_view::FileTab::history(s.repo.path.clone(), event.path.clone())
                });
                s.subscribe_file_history(&tab, window, cx);
                s.insert_editor_tab(tab.entity_id());
                s.leave_diff_tab(cx);
                s.file_tabs.push(tab);
                s.editor_tabs
                    .activate(s.file_tabs.last().unwrap().entity_id());
                s.file_tabs
                    .last()
                    .unwrap()
                    .update(cx, |t, cx| t.activate(cx));
                cx.notify();
            },
        )
        .detach();
    }
    pub(super) fn open_project_file(
        &mut self,
        path: PathBuf,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if preview && !self.file_preview {
            return;
        }
        if let Some(index) = self
            .file_tabs
            .iter()
            .position(|tab| !tab.read(cx).history && tab.read(cx).path == path)
        {
            if !preview {
                self.file_tabs[index].update(cx, |tab, _| tab.preview = false);
            }
            if self.active_file_index() == Some(index) {
                if !preview {
                    self.file_tabs[index].update(cx, |tab, cx| tab.focus_viewer(window, cx));
                }
                cx.notify();
                return;
            }
            self.leave_diff_tab(cx);
            self.editor_tabs.activate(self.file_tabs[index].entity_id());
            self.file_tabs[index].update(cx, |tab, cx| tab.activate_with_focus(!preview, cx));
        } else {
            let tab = cx.new(|_| {
                let mut tab = crate::project_view::FileTab::new(self.repo.path.clone(), path);
                tab.documents = self.documents.clone();
                tab.preview = preview;
                tab
            });
            self.subscribe_file_history(&tab, window, cx);
            cx.observe(&tab, |_, _, cx| cx.notify()).detach();

            let replacement = self
                .file_tabs
                .iter()
                .position(|tab| tab.read(cx).preview && !tab.read(cx).dirty(cx));
            let index = if let Some(index) = replacement {
                let old = self.file_tabs[index].entity_id();
                if preview {
                    // Deactivate the old entity before transferring the active identity.
                    self.leave_diff_tab(cx);
                    self.editor_tabs.replace(old, tab.entity_id());
                } else {
                    self.insert_editor_tab(tab.entity_id());
                    self.leave_diff_tab(cx);
                    self.editor_tabs.remove(old);
                }
                self.file_tabs[index] = tab;
                index
            } else {
                self.insert_editor_tab(tab.entity_id());
                self.leave_diff_tab(cx);
                self.file_tabs.push(tab);
                self.file_tabs.len() - 1
            };
            self.editor_tabs.activate(self.file_tabs[index].entity_id());
            self.file_tabs[index].update(cx, |tab, cx| tab.activate_with_focus(!preview, cx));
        }
        cx.notify();
    }
    pub(super) fn leave_diff_tab(&mut self, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        let active_diff = self.active_diff_index();
        let active_file = self.active_file_index();
        self.editor_tabs.deactivate();
        if let Some(index) = active_file {
            if let Some(tab) = self.file_tabs.get(index) {
                tab.update(cx, |t, cx| t.deactivate(cx));
            }
        }
        if let Some(index) = active_diff {
            if let Some(tab) = self.tabs.get(index) {
                tab.update(cx, |tab, cx| tab.deactivate(cx));
            }
            if self.bottom_preview {
                if let Some(file) = self.selected_file {
                    self.load_file(file, cx);
                }
            }
        }
        cx.notify();
    }
    pub(super) fn active_diff_index(&self) -> Option<usize> {
        let id = self.editor_tabs.active()?;
        self.tabs.iter().position(|tab| tab.entity_id() == id)
    }
    pub(super) fn active_file_index(&self) -> Option<usize> {
        let id = self.editor_tabs.active()?;
        self.file_tabs.iter().position(|tab| tab.entity_id() == id)
    }
    pub(super) fn insert_editor_tab(&mut self, id: EntityId) {
        self.editor_tabs.insert(id);
    }
    pub(super) fn editor_left_neighbor(&self, id: EntityId) -> Option<EntityId> {
        self.editor_tabs.left_neighbor(id)
    }
    pub(super) fn activate_editor_neighbor(
        &mut self,
        id: Option<EntityId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) =
            id.and_then(|id| self.file_tabs.iter().position(|tab| tab.entity_id() == id))
        {
            self.leave_diff_tab(cx);
            self.editor_tabs.activate(self.file_tabs[index].entity_id());
            self.file_tabs[index].update(cx, |tab, cx| tab.activate(cx));
        } else if let Some(index) =
            id.and_then(|id| self.tabs.iter().position(|tab| tab.entity_id() == id))
        {
            self.activate_diff_tab(index, window, cx);
        } else {
            self.focus_after_diff(window, cx);
        }
    }
    pub(super) fn close_file_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_project_preview(cx);
        let Some(tab) = self.file_tabs.get(index) else {
            return;
        };
        let id = tab.entity_id();
        if tab.read(cx).dirty(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                if let Some(index) = s.file_tabs.iter().position(|t| t.entity_id() == id) {
                    s.close_file_tab(index, w, cx);
                }
            });
            return;
        }
        let previous = self.editor_left_neighbor(id);
        let was_active = self.active_file_index() == Some(index);
        if was_active {
            self.leave_diff_tab(cx);
        }
        self.file_tabs.remove(index);
        self.editor_tabs.remove(id);
        if was_active {
            self.activate_editor_neighbor(previous, window, cx);
        }
        cx.notify();
    }
    pub(super) fn close_diff_tab_from_header(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            let Some(id) = self.tabs.get(index).map(|t| t.entity_id()) else {
                return;
            };
            self.with_saved_documents(window, cx, move |s, w, cx| {
                if let Some(index) = s.tabs.iter().position(|t| t.entity_id() == id) {
                    s.close_diff_tab_from_header(index, w, cx);
                }
            });
            return;
        }
        self.cancel_project_preview(cx);
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        let previous = self.editor_left_neighbor(tab.entity_id());
        let was_active = self.active_diff_index() == Some(index);
        self.close_diff_tab(index, cx);
        if was_active {
            self.activate_editor_neighbor(previous, window, cx);
        }
    }
    pub(super) fn activate_diff_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_settings(cx);
        self.leave_diff_tab(cx);
        self.reset_log_presentation();
        self.log_query.cancel();
        if let Some(tab) = self.tabs.get(index) {
            self.editor_tabs.activate(tab.entity_id());
            let callback = self.diff_source_callback(cx);
            tab.update(cx, |tab, cx| {
                tab.source_callback = Some(callback);
                tab.activate(window, cx);
            });
        }
    }
    pub(super) fn focus_after_diff(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self
            .active_diff_index()
            .and_then(|index| self.tabs.get(index))
        {
            tab.update(cx, |tab, cx| tab.focus_viewer(window, cx));
        } else if self.stash_open {
            if let Some(view) = &self.stash_view {
                view.update(cx, |s, cx| s.focus_files(window, cx));
            }
        } else {
            self.history_focus.focus(window, cx);
        }
    }
    pub(super) fn close_diff_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.active_diff_index() == Some(index) {
            self.leave_diff_tab(cx);
        }
        let removed = self.tabs.remove(index);
        self.editor_tabs.remove(removed.entity_id());

        cx.notify();
    }
    pub(super) fn toggle_commit_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.local_visible || self.stash_open {
            self.local_visible = false;
            self.stash_open = false;
            cx.notify();
        } else if self.commit_stash_selected {
            self.open_stashes(window, cx);
        } else {
            self.open_local(window, cx);
        }
    }
    pub(super) fn open_stashes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        self.close_settings(cx);
        if self.stash_view.is_none() {
            let repo = self.repo.clone();
            let view = cx.new(|cx| crate::stash_view::StashView::new(repo, cx));
            cx.subscribe_in(
                &view,
                window,
                |s, _, event: &crate::stash_view::OpenDiff, w, cx| {
                    let index = s
                        .tabs
                        .iter()
                        .position(|t| t.read(cx).target.identity() == event.0.identity())
                        .unwrap_or_else(|| {
                            let tab = cx.new(|_| {
                                let mut tab = DiffTab::new(event.0.clone(), event.1.clone());
                                tab.documents = s.documents.clone();
                                tab
                            });
                            s.subscribe_diff_navigation(&tab, cx);
                            cx.observe(&tab, |_, _, cx| cx.notify()).detach();
                            s.insert_editor_tab(tab.entity_id());
                            s.tabs.push(tab);
                            s.tabs.len() - 1
                        });
                    s.tabs[index].update(cx, |tab, _| tab.stash_origin = Some(event.2.clone()));
                    s.activate_diff_tab(index, w, cx);
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, _, event: &crate::stash_view::Restore, w, cx| {
                    s.restore_stash(event.0.clone(), event.1, w, cx);
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, _, event: &crate::stash_view::DropStash, w, cx| {
                    s.confirm_drop_stash(event.0.clone(), w, cx);
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, _, event: &crate::stash_view::Acknowledge, w, cx| {
                    s.confirm_external_recovery(event.0.clone(), w, cx);
                },
            )
            .detach();
            cx.subscribe_in(
                &view,
                window,
                |s, _, _: &crate::stash_view::ViewLocal, w, cx| {
                    s.open_local(w, cx);
                },
            )
            .detach();
            cx.observe(&view, |_, _, cx| cx.notify()).detach();
            self.stash_view = Some(view);
        }
        self.comparison_visible = false;
        self.project_visible = false;
        self.local_visible = false;
        self.commit_stash_selected = true;
        self.stash_open = true;
        cx.notify();
    }
    pub(super) fn open_diff_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(change) = self.selected_file.and_then(|i| self.files.get(i)).cloned() else {
            return;
        };
        let commit = if self.local {
            None
        } else {
            self.selected.and_then(|i| self.commits.get(i)).cloned()
        };
        let aggregate = self.aggregate_chain.clone().and_then(|mut chain| {
            chain
                .select(self.selected_file?)
                .map(|target| (target, chain))
        });
        let target = if let Some((target, _)) = &aggregate {
            target.clone()
        } else {
            let Ok(target) = diff_target::Target::new(self.repo.clone(), commit, change) else {
                return;
            };
            target
        };
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.read(cx).target.identity() == target.identity())
            .unwrap_or_else(|| {
                let chain = aggregate
                    .map(|(_, chain)| chain)
                    .or_else(|| {
                        self.tabs
                            .iter()
                            .find_map(|tab| tab.read(cx).fork_chain(&target))
                    })
                    .unwrap_or_else(|| {
                        diff_target::Chain::new(&target, &self.files)
                            .expect("Selected file belongs to the current file list")
                    });
                let tab = cx.new(|_| {
                    let mut tab = DiffTab::new(target, chain);
                    tab.documents = self.documents.clone();
                    tab
                });
                self.subscribe_diff_navigation(&tab, cx);
                cx.observe(&tab, |_, _, cx| cx.notify()).detach();
                self.insert_editor_tab(tab.entity_id());
                self.tabs.push(tab);
                self.tabs.len() - 1
            });
        self.activate_diff_tab(index, window, cx);
    }
    pub(super) fn close_editor_ids(
        &mut self,
        mut ids: Vec<EntityId>,
        clicked: Option<EntityId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.close_editor_ids(ids, clicked, w, cx)
            });
            return;
        }
        let active = self.editor_tabs.active();
        ids.sort_by_key(|id| Some(*id) == active);
        let closes_active = active.is_some_and(|id| ids.contains(&id));
        for id in ids {
            if let Some(i) = self.file_tabs.iter().position(|t| t.entity_id() == id) {
                self.close_file_tab(i, window, cx);
            } else if let Some(i) = self.tabs.iter().position(|t| t.entity_id() == id) {
                self.close_diff_tab_from_header(i, window, cx);
            }
        }
        if closes_active && self.active_file_index().is_none() && self.active_diff_index().is_none()
        {
            let remaining = clicked
                .filter(|id| self.editor_tabs.order().contains(id))
                .or_else(|| self.editor_tabs.order().last().copied());
            if remaining.is_some() {
                self.activate_editor_neighbor(remaining, window, cx);
            }
        }
    }
    pub(super) fn editor_tab_menu(
        &self,
        mut menu: gpui_kit::component::menu::PopupMenu,
        clicked: Option<EntityId>,
        cx: &mut Context<Self>,
    ) -> gpui_kit::component::menu::PopupMenu {
        for (label, ids) in [
            ("Close", clicked.into_iter().collect::<Vec<_>>()),
            ("Close Other Tabs", self.editor_tabs.others(clicked)),
            ("Close All Tabs", self.editor_tabs.order().to_vec()),
            (
                "Close Tabs to the Right",
                self.editor_tabs.right_of(clicked),
            ),
            (
                "Close All Read-Only",
                self.editor_tabs
                    .order()
                    .iter()
                    .copied()
                    .filter(|id| {
                        !self
                            .file_tabs
                            .iter()
                            .any(|tab| tab.entity_id() == *id && !tab.read(cx).history)
                    })
                    .collect(),
            ),
        ] {
            let owner = cx.entity().downgrade();
            menu = menu.item(
                PopupMenuItem::new(t(label))
                    .disabled(ids.is_empty())
                    .on_click(move |_, window, cx| {
                        let _ = owner.update(cx, |s, cx| {
                            s.close_editor_ids(ids.clone(), clicked, window, cx);
                        });
                    }),
            );
        }
        menu
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;
    #[gpui::test]
    fn commit_event_starts_log_refresh_even_from_welcome_state(cx: &mut gpui::TestAppContext) {
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
                s.open_local(window, cx);
                let local = s.local_view.clone().unwrap();
                let generation = s.log_query.stamp();
                local.update(cx, |_, cx| {
                    cx.emit(crate::local_view::Changed { push: false })
                });
                // Emission is dispatched after the entity update, below.
                assert_eq!(s.log_query.stamp(), generation);
            });
        });
        cx.update(|_, cx| {
            view.update(cx, |s, _| {
                assert!(s.log_query.history_loading());
            })
        });
    }
    #[gpui::test]
    fn disk_read_error_does_not_trap_clean_diff_tabs(cx: &mut gpui::TestAppContext) {
        use gpui::component::input::EditorState;
        use plico::{
            diff_target::{Chain, Target},
            file_document::Document,
        };
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
                let document = s.documents.get_or_create(
                    PathBuf::new(),
                    "deleted.txt".into(),
                    Document::new(b"unchanged".to_vec()).unwrap(),
                    |text, cx| {
                        cx.new(|cx| EditorState::new(window, cx).default_value(text.to_owned()))
                    },
                    cx,
                );
                // The external disk observer retains this diagnostic when a file disappears.
                document.update(cx, |d, cx| {
                    d.error = Some("File is unavailable on disk".into());
                    cx.notify();
                });
                for from_menu in [false, true] {
                    let change = git::Change {
                        area: git::Area::Untracked,
                        path: "deleted.txt".into(),
                        old_path: None,
                        status: "?".into(),
                    };
                    let target = Target::new(s.repo.clone(), None, change.clone()).unwrap();
                    let chain = Chain::new(&target, &[change]).unwrap();
                    let tab = cx.new(|_| DiffTab::new(target, chain));
                    let id = tab.entity_id();
                    s.insert_editor_tab(id);
                    s.tabs.push(tab);
                    if from_menu {
                        s.close_editor_ids(vec![id], Some(id), window, cx);
                    } else {
                        s.close_diff_tab_from_header(0, window, cx);
                    }
                    assert!(
                        s.tabs.is_empty(),
                        "clean deleted-file diff must close through either entry point"
                    );
                    assert!(!s.editor_tabs.order().contains(&id));
                }
                assert_eq!(
                    document.read(cx).editor.read(cx).text().to_string(),
                    "unchanged"
                );
            })
        });
    }
    #[gpui::test]
    fn closing_inactive_tab_keeps_active_entity_and_pending_close_uses_identity(
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
                for name in ["a.txt", "b.txt", "c.txt"] {
                    let tab =
                        cx.new(|_| crate::project_view::FileTab::new(PathBuf::new(), name.into()));
                    s.insert_editor_tab(tab.entity_id());
                    s.editor_tabs.activate(tab.entity_id());
                    s.file_tabs.push(tab);
                }
                let last = s.file_tabs[2].entity_id();
                let close_later = s.file_tabs[1].entity_id();
                s.close_file_tab(0, window, cx);
                assert_eq!(s.editor_tabs.active(), Some(last));
                assert_eq!(s.active_file_index(), Some(1));
                // This menu was prepared before the earlier removal shifted storage indexes.
                s.close_editor_ids(vec![close_later], None, window, cx);
                assert_eq!(s.editor_tabs.active(), Some(last));
                assert_eq!(s.active_file_index(), Some(0));
                assert_eq!(s.editor_tabs.order(), &[last]);
                s.close_file_tab(0, window, cx);
                assert_eq!(s.editor_tabs.active(), None);
                assert!(s.editor_tabs.order().is_empty());
            })
        });
    }
}
