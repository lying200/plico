//! Conflict-list adapter. Versions and mutations are owned by plico::conflicts.
use super::*;
use gpui_kit::component::dialog::DialogFooter;
mod access;
mod automatic;
#[cfg(all(test, feature = "desktop-tests"))]
mod automatic_tests;
#[cfg(all(test, feature = "desktop-tests"))]
mod project_tests;
mod revert;
mod session;
#[cfg(all(test, feature = "desktop-tests"))]
mod update_tests;
use plico::conflicts::{
    Resolution, Side, Snapshot,
    project::{Entry, Key},
};

#[derive(Clone)]
pub(super) struct SavedResolution {
    pub prepared: Arc<plico::merge_result::Prepared>,
    pub decisions: plico::text_merge::State,
    pub reviewed: bool,
}
impl SavedResolution {
    fn resolved(&self) -> bool {
        let (done, total) = self.decisions.progress();
        done == total
    }
    fn partial(&self) -> bool {
        let (done, total) = self.decisions.progress();
        done > 0 && done < total
    }
}
#[derive(Clone)]
enum Continuation {
    Sequence(Repository),
    Rebase(Repository),
    Update(plico::project_update::conflicts::channel::Client),
}
impl Continuation {
    fn repository(&self) -> &Repository {
        match self {
            Self::Sequence(repo) | Self::Rebase(repo) => repo,
            Self::Update(client) => client.repository(),
        }
    }
    fn resume(&self, review: &mut Review, window: &mut Window, cx: &mut Context<Review>) -> bool {
        match self {
            Self::Sequence(repo) => review.resume_project_sequence(repo, window, cx),
            Self::Update(client) => {
                client.finish(plico::project_update::conflicts::Decision::Continue)
            }
            Self::Rebase(repo) => {
                review.start_rebase_operation_in(repo.clone(), false, window, cx);
                true
            }
        }
    }
}
pub(super) struct ConflictList {
    owner: WeakEntity<Review>,
    repository: PathBuf,
    roots: Vec<Repository>,
    continuation: Option<Continuation>,
    entries: Vec<Entry>,
    selected: Option<usize>,
    drafts: std::collections::BTreeMap<Key, SavedResolution>,
    busy: bool,
    automatic: automatic::Automatic,
    after_close: Option<(Repository, PathBuf)>,
    error: Option<String>,
}
impl ConflictList {
    #[cfg(all(test, feature = "desktop-tests"))]
    fn key(&self, path: &std::path::Path) -> Key {
        Key::new(&self.roots[0], path)
    }

    fn all_resolved(&self) -> bool {
        self.entries.iter().all(|entry| {
            self.drafts
                .get(entry.key())
                .is_some_and(SavedResolution::resolved)
        })
    }
    fn all_reviewed(&self) -> bool {
        !self.entries.is_empty()
            && self.entries.iter().all(|entry| {
                self.drafts
                    .get(entry.key())
                    .is_some_and(|draft| draft.resolved() && draft.reviewed)
            })
    }
    fn contains(&self, snapshot: &Snapshot, file: &plico::conflicts::File) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.snapshot().same_operation(snapshot) && entry.file() == file)
    }
    fn replace_snapshot(&mut self, snapshot: Arc<Snapshot>) {
        let root = snapshot.repository().path;
        self.entries
            .retain(|entry| entry.snapshot().repository().path != root);
        self.entries
            .extend(plico::conflicts::project::entries(snapshot.clone()));
        self.entries.sort_by_cached_key(Entry::full_path);
        self.selected = None;
    }
    fn merge(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(entry) = self.selected.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        let file = entry.file().clone();
        if file
            .stages
            .iter()
            .flatten()
            .any(|v| v.mode == "120000" || v.mode == "160000")
        {
            return;
        }
        let snapshot = entry.snapshot().clone();
        let owner = self.owner.clone();
        let repository = self.repository.clone();
        window.defer(cx, move |window, cx| {
            let _ = owner.update(cx, |owner, cx| {
                if owner.repo.path == repository {
                    owner.with_conflict_documents(window, cx, move |s, w, cx| {
                        s.open_text_merge(snapshot, file, w, cx)
                    });
                }
            });
        });
    }

    fn act(&self, resolution: Option<Resolution>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.selected.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        if self.busy {
            return;
        }
        let snapshot = entry.snapshot().clone();
        let file = entry.file().clone();
        if let Some(Resolution::Accept(side)) = resolution {
            let view = cx.entity();
            let owner = self.owner.clone();
            let repository = self.repository.clone();
            window.defer(cx, move |window, cx| {
                let _ = owner.update(cx, |s, cx| {
                    if s.repo.path == repository {
                        s.with_conflict_documents(window, cx, move |s, w, cx| {
                            s.accept_conflict_in_session(view, entry, side, w, cx)
                        });
                    }
                });
            });
            return;
        }
        if resolution.is_none() && !self.drafts.is_empty() {
            let view = cx.entity();
            let owner = self.owner.clone();
            window.defer(cx, move |window, cx| {
                view.update(cx, |state, _| {
                    state.after_close = Some((snapshot.repository(), file.path))
                });
                let _ = owner.update(cx, |s, cx| {
                    s.close_conflict_session(view, false, window, cx)
                });
            });
            return;
        }
        let owner = self.owner.clone();
        let repository = self.repository.clone();
        let view = cx.entity();
        window.defer(cx, move |window, cx| {
            let _ = owner.update(cx, |s, cx| {
                if s.repo.path != repository {
                    return;
                }
                if let Some(resolution) = resolution {
                    window.close_dialog(cx);
                    s.with_conflict_documents(window, cx, move |s, w, cx| {
                        s.resolve_conflict(snapshot, file, resolution, w, cx)
                    });
                } else {
                    view.update(cx, |state, _| {
                        state.after_close = Some((snapshot.repository(), file.path));
                    });
                    s.close_conflict_session(view, false, window, cx);
                }
            });
        });
    }
}
impl Render for ConflictList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex()
            .id("conflict-files")
            .w_full()
            .min_w_0()
            .h(px(300.))
            .overflow_y_scroll()
            .child(
                h_flex()
                    .gap_2()
                    .px_2()
                    .child(div().flex_1().child(t("File")))
                    .child(div().w(px(160.)).child(t("Yours")))
                    .child(div().w(px(160.)).child(t("Theirs"))),
            );
        for resolved in [false, true] {
            let rows = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    self.drafts
                        .get(entry.key())
                        .is_some_and(SavedResolution::resolved)
                        == resolved
                })
                .collect::<Vec<_>>();
            if rows.is_empty() {
                continue;
            }
            list = list.child(
                h_flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .child(jb_icon(if resolved {
                        "checked_dark.svg"
                    } else {
                        "close_dark.svg"
                    }))
                    .child(t(if resolved { "Resolved" } else { "Unresolved" }))
                    .child(rows.len().to_string()),
            );
            for (i, entry) in rows {
                let file = entry.file();
                let disabled =
                    self.busy || file.stages.iter().flatten().any(|v| v.mode == "160000");
                let status = |side| {
                    t(if file.side(side, entry.snapshot().reversed).is_some() {
                        "Modified"
                    } else {
                        "Deleted"
                    })
                };
                let full_path = PathBuf::from(project_path_text(&entry.full_path()));
                let display_root = PathBuf::from(project_path_text(&self.repository));
                let parent = full_path.parent().unwrap_or(&full_path);
                let location = parent
                    .strip_prefix(&display_root)
                    .ok()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(project_path_text)
                    .unwrap_or_else(|| {
                        if parent == display_root {
                            self.repository
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                        } else {
                            project_path_text(parent)
                        }
                    });
                let mut row = h_flex()
                    .id(("conflict-file", i))
                    .gap_2()
                    .px_2()
                    .h(px(28.))
                    .min_w_0()
                    .cursor_pointer()
                    .when(self.selected == Some(i), |row| row.bg(rgb(0x2e4369)))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .child(
                                div().min_w_0().truncate().flex_shrink_0().child(
                                    file.path
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned(),
                                ),
                            )
                            .child(
                                div()
                                    .ml_2()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(location),
                            )
                            .when_some(
                                self.drafts
                                    .get(entry.key())
                                    .map(|s| s.decisions.progress())
                                    .or_else(|| self.automatic.counts.get(entry.key()).copied()),
                                |row, (done, total)| {
                                    row.child(
                                        div()
                                            .ml_2()
                                            .px_1()
                                            .rounded(px(8.))
                                            .bg(rgb(0x2e4369))
                                            .child(format!("{done}/{total}")),
                                    )
                                },
                            ),
                    );
                for (side, id) in [(Side::Yours, "ours"), (Side::Theirs, "theirs")] {
                    row = row.child(h_flex().w(px(160.)).gap_2().child(status(side)).when(
                        !resolved,
                        |cell| {
                            cell.child(
                                Button::new((id, i))
                                    .label(t("Accept"))
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        cx.stop_propagation();
                                        s.selected = Some(i);
                                        s.act(Some(Resolution::Accept(side)), w, cx);
                                    })),
                            )
                        },
                    ));
                }
                let edit = cx.entity();
                let mark = edit.clone();
                let can_mark = !disabled && !resolved;
                let busy = self.busy;
                let revert_view = cx.entity();
                let revert_owner = self.owner.clone();
                let revert_path = entry.key().clone();
                let can_revert = !busy
                    && self
                        .drafts
                        .get(entry.key())
                        .is_some_and(|d| d.decisions.progress().0 > 0);
                list = list.child(
                    row.on_click(cx.listener(move |s, event: &ClickEvent, w, cx| {
                        s.selected = Some(i);
                        if event.click_count() == 2 && !s.busy {
                            s.merge(w, cx);
                        }
                        cx.notify();
                    }))
                    .context_menu(move |menu, _, _| {
                        let edit = edit.clone();
                        let mark = mark.clone();
                        let revert_view = revert_view.clone();
                        let revert_owner = revert_owner.clone();
                        let revert_path = revert_path.clone();
                        menu.item(PopupMenuItem::new(t("Edit File")).disabled(busy).on_click(
                            move |_, w, cx| {
                                edit.update(cx, |s, cx| {
                                    s.selected = Some(i);
                                    s.act(None, w, cx);
                                });
                            },
                        ))
                        .item(
                            PopupMenuItem::new(t("Mark Resolved"))
                                .disabled(!can_mark)
                                .on_click(move |_, w, cx| {
                                    mark.update(cx, |s, cx| {
                                        s.selected = Some(i);
                                        s.act(Some(Resolution::MarkResolved), w, cx);
                                    });
                                }),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new(t("Revert Conflict Resolution"))
                                .disabled(!can_revert)
                                .on_click(move |_, w, cx| {
                                    let _ = revert_owner.update(cx, |s, cx| {
                                        s.revert_conflict_resolution(
                                            revert_view.clone(),
                                            revert_path.clone(),
                                            w,
                                            cx,
                                        )
                                    });
                                }),
                        )
                    }),
                );
            }
        }
        let auto_view = cx.entity();
        let auto_owner = self.owner.clone();
        let cancel_owner = self.owner.clone();
        let running = self.automatic.progress.is_some();
        let unresolved = !self.all_resolved();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_3()
                    .min_w_0()
                    .child(
                        Button::new("auto-resolve-conflicts")
                            .icon(jb_icon("magicResolve_dark.svg"))
                            .label(t(if running {
                                "Resolving Conflicts…"
                            } else {
                                "Resolve All Simple Conflicts"
                            }))
                            .loading(running)
                            .disabled(self.busy || self.automatic.attempted || !unresolved)
                            .on_click(move |_, w, cx| {
                                let view = auto_view.clone();
                                let _ = auto_owner.update(cx, |s, cx| {
                                    s.with_conflict_documents(w, cx, move |s, w, cx| {
                                        s.resolve_conflicts_automatically(view, w, cx)
                                    })
                                });
                            }),
                    )
                    .when_some(self.automatic.progress, |row, (current, total)| {
                        row.child(plico::i18n::automatic_merge_progress(current, total))
                            .child(
                                Button::new("cancel-auto-resolve")
                                    .ghost()
                                    .label(t("Cancel"))
                                    .on_click(move |_, _, cx| {
                                        let _ = cancel_owner
                                            .update(cx, |s, _| s.operation.request_cancel());
                                    }),
                            )
                    })
                    .when_some(self.automatic.message.clone(), |row, text| {
                        row.child(div().flex_1().min_w_0().whitespace_normal().child(text))
                    }),
            )
            .when(self.all_reviewed(), |v| {
                v.child(
                    h_flex()
                        .gap_2()
                        .child(jb_icon("checked_dark.svg"))
                        .child(t("All conflicts have been resolved")),
                )
            })
            .child(list)
            .when_some(self.error.clone(), |v, error| {
                let owner = self.owner.clone();
                let view = cx.entity();
                v.child(
                    div()
                        .max_h(px(120.))
                        .id("conflict-error")
                        .overflow_y_scroll()
                        .whitespace_normal()
                        .text_color(rgb(0xe57373))
                        .child(error),
                )
                .child(
                    Button::new("leave-unresolved")
                        .link()
                        .label(t("Close Without Staging"))
                        .disabled(self.busy)
                        .on_click(move |_, w, cx| {
                            let _ = owner
                                .update(cx, |s, cx| s.leave_conflict_session(view.clone(), w, cx));
                        }),
                )
            })
            .when(self.busy && self.automatic.progress.is_none(), |v| {
                v.child(t("Resolving conflict…"))
            })
    }
}
impl Review {
    pub(super) fn saved_conflict_resolution(
        &self,
        snapshot: &Snapshot,
        path: &std::path::Path,
        cx: &App,
    ) -> Option<SavedResolution> {
        let view = self.conflict_session.as_ref()?.read(cx);
        (view.repository == self.repo.path
            && view
                .entries
                .iter()
                .any(|entry| entry.snapshot().same_operation(snapshot)))
        .then(|| {
            view.drafts
                .get(&Key::new(&snapshot.repository(), path))
                .cloned()
        })
        .flatten()
    }
    pub(super) fn conflict_result_session(
        &self,
        prepared: &plico::merge_result::Prepared,
        cx: &App,
    ) -> Option<Entity<ConflictList>> {
        self.conflict_session.clone().filter(|view| {
            let state = view.read(cx);
            state.repository == self.repo.path
                && state.contains(prepared.snapshot(), prepared.file())
        })
    }
    pub(super) fn mark_conflict_reviewed(
        &mut self,
        prepared: &plico::merge_result::Prepared,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self.conflict_result_session(prepared, cx) {
            view.update(cx, |state, cx| {
                if let Some(draft) = state.drafts.get_mut(&Key::new(
                    &prepared.snapshot().repository(),
                    &prepared.file().path,
                )) {
                    draft.reviewed = true;
                }
                cx.notify();
            });
            cx.notify();
        }
    }
    pub(super) fn keep_conflict_resolution(
        &mut self,
        view: &Entity<ConflictList>,
        prepared: Arc<plico::merge_result::Prepared>,
        decisions: plico::text_merge::State,
        cx: &mut Context<Self>,
    ) {
        if self.conflict_result_session(&prepared, cx).as_ref() != Some(view) {
            return;
        }
        view.update(cx, |state, cx| {
            state.automatic.invalidate(&Key::new(
                &prepared.snapshot().repository(),
                &prepared.file().path,
            ));
            state.drafts.insert(
                Key::new(&prepared.snapshot().repository(), &prepared.file().path),
                SavedResolution {
                    prepared,
                    decisions,
                    reviewed: true,
                },
            );
            state.error = None;
            cx.notify();
        });
    }

    pub(super) fn reopen_conflicts_in(
        &mut self,
        repository: Repository,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scope = self.conflict_session.as_ref().and_then(|view| {
            let state = view.read(cx);
            matches!(&state.continuation, Some(Continuation::Update(_)))
                .then(|| (state.roots.clone(), state.continuation.clone()))
        });
        if let Some((roots, continuation)) = scope {
            self.open_conflict_roots(roots, continuation, None, window, cx);
        } else {
            self.open_conflicts_in(repository, window, cx);
        }
    }

    pub(super) fn clear_update_conflicts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let updating = self.conflict_session.as_ref().is_some_and(|view| {
            matches!(&view.read(cx).continuation, Some(Continuation::Update(_)))
        });
        if updating {
            if let Some(view) = self.conflict_session.take() {
                for draft in view.read(cx).drafts.values() {
                    draft.prepared.retain_output();
                }
            }
            window.close_all_dialogs(cx);
        }
    }

    pub(super) fn open_conflicts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let roots = self.project_roots.read(cx).observation_roots();
        self.open_conflict_roots(roots.as_ref().clone(), None, Some(roots), window, cx);
    }
    pub(super) fn open_conflicts_in(
        &mut self,
        repo: Repository,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_conflict_roots(
            vec![repo.clone()],
            Some(Continuation::Sequence(repo)),
            None,
            window,
            cx,
        );
    }
    pub(super) fn open_rebase_conflicts_in(
        &mut self,
        repo: Repository,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_conflict_roots(
            vec![repo.clone()],
            Some(Continuation::Rebase(repo)),
            None,
            window,
            cx,
        );
    }
    fn open_conflict_roots(
        &mut self,
        roots: Vec<Repository>,
        continuation: Option<Continuation>,
        membership: Option<Arc<Vec<Repository>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if roots.is_empty() {
            return;
        }
        let Some((id, token)) =
            self.begin_conflict_operation(plico::active_operation::Phase::Preparing, cx)
        else {
            return;
        };
        let project_path = self.repo.path.clone();
        let interaction = self
            .sequence_conflict
            .as_ref()
            .filter(|(_, root, _, _)| {
                continuation
                    .as_ref()
                    .is_some_and(|r| root == &r.repository().path)
            })
            .map(|(identity, _, _, _)| identity.clone());
        self.conflict_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    let snapshots = plico::conflicts::project::read_snapshots(&roots, &token)?;
                    let roots = snapshots.iter().map(|s| s.repository()).collect::<Vec<_>>();
                    let mut entries = snapshots
                        .into_iter()
                        .flat_map(plico::conflicts::project::entries)
                        .collect::<Vec<_>>();
                    entries.sort_by_cached_key(Entry::full_path);
                    Ok((roots, entries))
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_conflict_operation(id, cx) else {
                    return;
                };
                if cancelled
                    || s.repo.path != project_path
                    || membership.as_ref().is_some_and(|roots| {
                        !Arc::ptr_eq(roots, &s.project_roots.read(cx).observation_roots())
                    })
                {
                    if interaction.as_ref().is_some_and(|identity| {
                        s.sequence_conflict
                            .as_ref()
                            .is_some_and(|(current, _, _, _)| Arc::ptr_eq(identity, current))
                    }) {
                        s.sequence_conflict = None;
                    }
                    return;
                }
                match result {
                    Err(error) => {
                        if interaction.as_ref().is_some_and(|identity| {
                            s.sequence_conflict
                                .as_ref()
                                .is_some_and(|(current, _, _, _)| Arc::ptr_eq(identity, current))
                        }) {
                            s.sequence_conflict = None;
                        }
                        crate::error_ui::show_details(error, "Could not read conflicts", w, cx)
                    }
                    Ok((roots, entries)) => {
                        s.show_conflict_list(roots, entries, continuation, w, cx);
                    }
                }
            });
        });
    }
    fn show_conflict_list(
        &mut self,
        roots: Vec<Repository>,
        entries: Vec<Entry>,
        continuation: Option<Continuation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let s = self;
        let w = window;

        if entries.is_empty()
            && continuation
                .as_ref()
                .is_some_and(|repo| repo.resume(s, w, cx))
        {
            return;
        }
        let interaction = s
            .sequence_conflict
            .as_ref()
            .filter(|(_, root, _, _)| {
                continuation
                    .as_ref()
                    .is_some_and(|r| root == &r.repository().path)
            })
            .map(|(identity, _, _, _)| identity.clone());
        let owner = cx.entity().downgrade();
        let cancel_owner = owner.clone();
        let repository = s.repo.path.clone();
        let selected = (!entries.is_empty()).then_some(0);
        let existing = s.conflict_session.clone().filter(|view| {
            let view = view.read(cx);
            view.repository == repository
        });
        let view = if let Some(view) = existing {
            view.update(cx, |view, cx| {
                view.drafts.retain(|key, draft| {
                    let keep = entries.iter().any(|entry| {
                        entry.key() == key
                            && entry.snapshot().same_operation(draft.prepared.snapshot())
                            && entry.file() == draft.prepared.file()
                    });
                    if !keep {
                        draft.prepared.retain_output();
                    }
                    keep
                });
                view.automatic.counts.clear();
                view.automatic.attempted = false;
                view.automatic.message = None;
                view.roots = roots;
                view.entries = entries;
                view.continuation = continuation;
                view.selected = selected;
                view.busy = false;
                cx.notify();
            });
            view
        } else {
            cx.new(|_| ConflictList {
                owner,
                repository,
                entries,
                roots,
                continuation,
                selected,
                drafts: Default::default(),
                after_close: None,
                busy: false,
                automatic: Default::default(),
                error: None,
            })
        };
        s.conflict_session = Some(view.clone());
        w.open_dialog(cx, move |d, _, cx| {
            let close_owner = cancel_owner.clone();
            let close_view = view.clone();
            let finish_owner = cancel_owner.clone();
            let finish_view = view.clone();
            let cancel_owner = cancel_owner.clone();
            let cancel_view = view.clone();
            let manual_view = view.clone();
            let state = view.read(cx);
            let busy = state.busy;
            let all_reviewed = state.all_reviewed();
            let all_resolved = state.all_resolved();
            let selected_entry = state.selected.and_then(|i| state.entries.get(i));
            let selected_file = selected_entry.map(Entry::file);
            let can_merge = selected_file.is_some_and(|f| {
                !f.stages
                    .iter()
                    .flatten()
                    .any(|v| v.mode == "160000" || v.mode == "120000")
            });
            let reviewing = selected_entry.is_some_and(|entry| {
                state
                    .drafts
                    .get(entry.key())
                    .is_some_and(SavedResolution::resolved)
            });
            let _ = &interaction;
            d.title(t("Conflicts"))
                .w(px(850.))
                .centered(true)
                .overlay_closable(false)
                .child(view.clone())
                .footer(
                    DialogFooter::new()
                        .gap_2()
                        .child(
                            Button::new("close-conflicts")
                                .label(t("Close"))
                                .disabled(busy)
                                .on_click(move |_, w, cx| {
                                    let _ = close_owner.update(cx, |s, cx| {
                                        s.close_conflict_session(close_view.clone(), false, w, cx)
                                    });
                                }),
                        )
                        .child(
                            Button::new("finish-conflicts")
                                .primary()
                                .label(t("Accept and Finish"))
                                .disabled(busy || !all_resolved)
                                .on_click(move |_, w, cx| {
                                    let _ = finish_owner.update(cx, |s, cx| {
                                        s.close_conflict_session(finish_view.clone(), true, w, cx)
                                    });
                                }),
                        )
                        .when(!all_reviewed, |footer| {
                            footer.child(
                                Button::new("manual-conflicts")
                                    .label(t(if reviewing {
                                        "Review Changes"
                                    } else {
                                        "Resolve Manually"
                                    }))
                                    .disabled(busy || !can_merge)
                                    .on_click(move |_, w, cx| {
                                        manual_view.update(cx, |s, cx| s.merge(w, cx));
                                    }),
                            )
                        }),
                )
                .on_cancel(move |_, w, cx| {
                    let _ = cancel_owner.update(cx, |s, cx| {
                        s.close_conflict_session(cancel_view.clone(), false, w, cx)
                    });
                    false
                })
        });
    }

    pub(super) fn open_update_conflicts(
        &mut self,
        pending: plico::project_update::conflicts::channel::Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .operation
            .is_running(plico::active_operation::Kind::Update)
            || !pending.client.active()
        {
            return;
        }
        let roots = vec![pending.client.repository().clone()];
        let entries = plico::conflicts::project::entries(pending.snapshot);
        self.show_conflict_list(
            roots,
            entries,
            Some(Continuation::Update(pending.client)),
            window,
            cx,
        );
        // Unlike standalone recovery, the outer Update remains active: there
        // is no finish_operation notification to invalidate the main view.
        cx.notify();
        window.refresh();
    }

    pub(super) fn resolve_conflict(
        &mut self,
        snapshot: Arc<Snapshot>,
        file: plico::conflicts::File,
        resolution: Resolution,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) =
            self.begin_conflict_operation(plico::active_operation::Phase::Executing, cx)
        else {
            return;
        };
        let session_scope = self.conflict_session.as_ref().and_then(|view| {
            let state = view.read(cx);
            state
                .contains(&snapshot, &file)
                .then(|| (state.roots.clone(), state.continuation.clone()))
        });
        let repo = snapshot.repository();
        let repository = repo.clone();
        let project_path = self.repo.path.clone();
        self.status_message = Some(t("Resolving conflict…").into());
        let access = id.access.clone();
        self.conflict_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    access.resolve(&repo, &snapshot, &file, resolution, &token)
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_conflict_operation(id, cx) else {
                    return;
                };
                if s.repo.path != project_path {
                    return;
                }
                let error = result.err();
                s.status_message = Some(
                    t(if error.is_some() || cancelled {
                        "Conflict resolution interrupted"
                    } else {
                        "Conflict resolved"
                    })
                    .into(),
                );
                s.sync_repository_worktree(
                    &repository.path,
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
                if let Some(error) = error {
                    crate::error_ui::show_details(error, "Could not resolve conflict", w, cx);
                } else if !cancelled {
                    if let Some((roots, continuation)) = session_scope {
                        s.open_conflict_roots(roots, continuation, None, w, cx);
                    } else {
                        s.open_conflicts_in(repository, w, cx);
                    }
                }
            });
        });
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    #[gpui::test]
    fn child_conflict_resolution_and_file_tabs_keep_their_repository(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::component::Root;
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn git(path: &std::path::Path, args: &[&str], success: bool) -> String {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(out.status.success(), success, "{args:?}: {out:?}");
            String::from_utf8(out.stdout).unwrap().trim().into()
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-conflict-roots-{}-{nonce}",
            std::process::id()
        )));
        let parent = fixture.0.join("parent");
        let child = fixture.0.join("child");
        std::fs::create_dir_all(&parent).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            git(&parent, &args, true);
        }
        std::fs::write(parent.join("a"), "base\n").unwrap();
        git(&parent, &["add", "."], true);
        git(&parent, &["commit", "-qm", "base"], true);
        let parent_head = git(&parent, &["rev-parse", "HEAD"], true);
        git(
            &parent,
            &["clone", "-q", "--no-local", ".", child.to_str().unwrap()],
            true,
        );
        for args in [
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["checkout", "-qb", "topic"],
        ] {
            git(&child, &args, true);
        }
        std::fs::write(child.join("a"), "topic\n").unwrap();
        git(&child, &["commit", "-qam", "topic"], true);
        git(&child, &["checkout", "main"], true);
        std::fs::write(child.join("a"), "main\n").unwrap();
        git(&child, &["commit", "-qam", "main"], true);
        let child_head = git(&child, &["rev-parse", "HEAD"], true);
        let child_repo = Repository::new(&child);
        let prepared = plico::branch_merge::Prepared::read(
            &child_repo,
            "refs/heads/topic",
            &Cancel::default(),
        )
        .unwrap();
        git(&child, &["merge", "topic"], false);
        let snapshot =
            Arc::new(Snapshot::read(&Repository::new(&child), &Cancel::default()).unwrap());
        let file = snapshot.files[0].clone();
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(parent.clone(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.open_project_file("a".into(), false, w, cx);
                s.open_project_file_in(child.clone(), "a".into(), false, w, cx);
            });
        });
        cx.run_until_parked();
        let child_document = review.read_with(cx, |s, _| {
            s.documents
                .get(&child, std::path::Path::new("a"))
                .expect("child source document loaded")
        });
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.pending_branch_merge = Some(prepared.clone());
                let owner = cx.entity().downgrade();
                let repository = s.repo.path.clone();
                let view = cx.new(|_| ConflictList {
                    owner,
                    repository,
                    entries: plico::conflicts::project::entries(snapshot.clone()),
                    continuation: None,
                    roots: vec![snapshot.repository()],
                    selected: Some(0),
                    drafts: Default::default(),
                    after_close: None,
                    busy: false,
                    automatic: Default::default(),
                    error: None,
                });
                s.conflict_session = Some(view.clone());
                let entry = view
                    .read(cx)
                    .entries
                    .iter()
                    .find(|e| e.file() == &file)
                    .unwrap()
                    .clone();
                s.accept_conflict_in_session(view, entry, Side::Theirs, w, cx);
            })
        });
        cx.run_until_parked();
        assert_eq!(std::fs::read(child.join("a")).unwrap(), b"topic\n");
        assert_eq!(
            child_document.read_with(cx, |s, cx| s.comparison_text(cx)),
            "topic\n",
            "resolution must refresh the already-open child document, not the primary repository"
        );
        assert_eq!(
            git(&child, &["ls-files", "-u"], true).lines().count(),
            3,
            "inline acceptance saves the working file without staging it"
        );
        let session = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
        assert_eq!(session.read_with(cx, |s, _| s.drafts.len()), 1);
        assert!(
            session.read_with(cx, |s, _| s.all_reviewed()),
            "explicit whole-side acceptance counts as reviewed"
        );
        session.update(cx, |s, _| {
            s.drafts.values_mut().next().unwrap().reviewed = false
        });
        assert!(
            !session.read_with(cx, |s, _| s.all_reviewed()),
            "resolved alone is not reviewed"
        );
        let prepared_for_review = session.read_with(cx, |s, _| {
            s.drafts.values().next().unwrap().prepared.clone()
        });
        review.update(cx, |s, cx| {
            s.mark_conflict_reviewed(&prepared_for_review, cx)
        });
        assert!(session.read_with(cx, |s, _| s.all_reviewed()));
        // Applying the manual editor result stays in the same iterative session.
        let saved = session.read_with(cx, |s, _| s.drafts.values().next().unwrap().clone());
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.apply_merge_result(
                    saved.prepared.clone(),
                    "topic\n".into(),
                    saved.decisions.clone(),
                    Box::new(|success, _, _| assert!(success)),
                    w,
                    cx,
                );
            })
        });
        cx.run_until_parked();
        let session = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
        assert_eq!(git(&child, &["ls-files", "-u"], true).lines().count(), 3);
        let saved = session.read_with(cx, |s, _| s.drafts.values().next().unwrap().clone());
        let texts = saved.prepared.native_text_inputs().unwrap().unwrap();
        let mut model = plico::text_merge::Merge::new(&texts[1], &texts[0], &texts[2]);
        let text = saved
            .prepared
            .saved_text(&child_repo, &Cancel::default())
            .unwrap();
        model.restore(&saved.decisions, &text);
        assert_eq!(model.text(), "topic\n");
        assert_eq!(
            model.remaining(),
            0,
            "reopening must retain resolved block decisions"
        );

        std::fs::write(child.join(".git/index.lock"), "held by test").unwrap();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.close_conflict_session(session.clone(), false, w, cx)
            })
        });
        cx.run_until_parked();
        session.read_with(cx, |s, _| {
            assert!(!s.busy);
            assert!(s.error.is_some());
            assert_eq!(s.drafts.len(), 1, "failed close retains the accepted draft");
        });
        std::fs::remove_file(child.join(".git/index.lock")).unwrap();
        // An external editor must not silently change what this close stages.
        std::fs::write(child.join("a"), "external edit\n").unwrap();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.close_conflict_session(session.clone(), false, w, cx)
            })
        });
        cx.run_until_parked();
        assert_eq!(git(&child, &["ls-files", "-u"], true).lines().count(), 3);
        assert_eq!(std::fs::read(child.join("a")).unwrap(), b"external edit\n");
        std::fs::write(child.join("a"), "topic\n").unwrap();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.close_conflict_session(session.clone(), false, w, cx)
            })
        });
        cx.run_until_parked();
        assert!(git(&child, &["ls-files", "-u"], true).is_empty());
        assert_eq!(
            git(&child, &["rev-parse", "HEAD"], true),
            child_head,
            "Close does not create a merge commit"
        );
        assert!(child.join(".git/MERGE_HEAD").exists());
        review.read_with(cx, |s, _| assert!(s.conflict_session.is_none()));
        assert_eq!(git(&parent, &["rev-parse", "HEAD"], true), parent_head);
        assert!(git(&parent, &["status", "--porcelain"], true).is_empty());
        cx.update(|w, cx| {
            if w.has_active_dialog(cx) {
                w.close_dialog(cx);
            }
            review.update(cx, |s, cx| {
                s.open_project_file("a".into(), false, w, cx);
                s.open_project_file_in(child.clone(), "a".into(), false, w, cx);
                assert_eq!(s.file_tabs.len(), 2);
                assert_eq!(s.file_tabs[0].read(cx).root(), parent);
                assert_eq!(s.file_tabs[1].read(cx).root(), child);
                assert_eq!(s.repo.path, parent);
            });
        });
        cx.run_until_parked();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.start_merge_operation_in(
                    child_repo.clone(),
                    plico::merge_operation::Action::Commit,
                    w,
                    cx,
                );
            })
        });
        cx.run_until_parked();
        assert!(
            !child_repo
                .operation_state(&Cancel::default())
                .unwrap()
                .in_progress()
        );
        assert_eq!(git(&child, &["rev-parse", "HEAD^1"], true), child_head);
        assert_eq!(git(&child, &["rev-parse", "HEAD^2"], true), prepared.target);
        assert_eq!(git(&parent, &["rev-parse", "HEAD"], true), parent_head);
        review.read_with(cx, |s, _| {
            assert!(s.pending_branch_merge.is_none());
            assert_eq!(s.merged_branch.as_ref().unwrap().repository().path, child);
        });
        // Abort also belongs to the child; the primary HEAD must never move.
        git(&child, &["reset", "--hard", &child_head], true);
        git(&child, &["merge", "topic"], false);
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.pending_branch_merge = Some(prepared.clone());
                s.start_merge_operation_in(
                    child_repo.clone(),
                    plico::merge_operation::Action::Abort,
                    w,
                    cx,
                );
            })
        });
        cx.run_until_parked();
        assert!(
            !child_repo
                .operation_state(&Cancel::default())
                .unwrap()
                .in_progress()
        );
        assert_eq!(git(&child, &["rev-parse", "HEAD"], true), child_head);
        assert_eq!(git(&parent, &["rev-parse", "HEAD"], true), parent_head);
        assert!(git(&parent, &["status", "--porcelain"], true).is_empty());
        review.read_with(cx, |s, _| {
            assert!(s.pending_branch_merge.is_none());
            assert!(s.merged_branch.is_none());
        });
        // A failed finish can be left explicitly without staging or losing disk edits.
        git(&child, &["merge", "topic"], false);
        cx.update(|w, cx| {
            review.update(cx, |s, cx| s.open_conflicts_in(child_repo.clone(), w, cx))
        });
        cx.run_until_parked();
        let session = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
        let file = session.read_with(cx, |s, _| s.entries[0].file().clone());
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.accept_conflict_in_session(
                    session.clone(),
                    session
                        .read(cx)
                        .entries
                        .iter()
                        .find(|e| e.file() == &file)
                        .unwrap()
                        .clone(),
                    Side::Theirs,
                    w,
                    cx,
                )
            })
        });
        cx.run_until_parked();
        std::fs::write(child.join("a"), "new external edit\n").unwrap();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.close_conflict_session(session.clone(), false, w, cx)
            })
        });
        cx.run_until_parked();
        assert!(session.read_with(cx, |s, _| s.error.is_some()));
        cx.update(|w, cx| {
            review.update(cx, |s, cx| s.leave_conflict_session(session.clone(), w, cx))
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| assert!(s.conflict_session.is_none()));
        assert_eq!(
            std::fs::read(child.join("a")).unwrap(),
            b"new external edit\n"
        );
        assert_eq!(git(&child, &["ls-files", "-u"], true).lines().count(), 3);
        assert_eq!(git(&parent, &["rev-parse", "HEAD"], true), parent_head);
    }

    #[gpui::test]
    fn partial_merge_progress_survives_review_and_close_only_stages_complete_files(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::component::Root;
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = Fixture(
            std::env::temp_dir().join(format!("plico-partial-{}-{nonce}", std::process::id())),
        );
        let root = dir.0.clone();
        std::fs::create_dir_all(&root).unwrap();
        let git = |args: &[&str], success: bool| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(out.status.success(), success, "{args:?}: {out:?}");
            String::from_utf8(out.stdout).unwrap()
        };
        git(&["init", "-q", "-b", "main"], true);
        git(&["config", "user.name", "Test"], true);
        git(&["config", "user.email", "test@example.invalid"], true);
        git(&["config", "commit.gpgsign", "false"], true);
        let content = |side: &str| {
            format!(
                "{side} one\n{}\n{side} two\n",
                (0..8)
                    .map(|i| format!("unchanged {i}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        for path in ["a", "b"] {
            std::fs::write(root.join(path), content("base")).unwrap();
        }
        git(&["add", "."], true);
        git(&["commit", "-qm", "base"], true);
        git(&["checkout", "-qb", "topic"], true);
        for path in ["a", "b"] {
            std::fs::write(root.join(path), content("topic")).unwrap();
        }
        git(&["commit", "-qam", "topic"], true);
        git(&["checkout", "main"], true);
        for path in ["a", "b"] {
            std::fs::write(root.join(path), content("main")).unwrap();
        }
        git(&["commit", "-qam", "main"], true);
        let head = git(&["rev-parse", "HEAD"], true);
        git(&["merge", "topic"], false);
        let repo = Repository::new(&root);
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(root.clone(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        cx.update(|w, cx| review.update(cx, |s, cx| s.open_conflicts_in(repo.clone(), w, cx)));
        cx.run_until_parked();
        let session = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
        let snapshot = session.read_with(cx, |s, _| s.entries[0].snapshot().clone());
        let mut partial_text = String::new();
        for (index, file) in snapshot.files.iter().enumerate() {
            let prepared = Arc::new(
                plico::merge_result::Prepared::read(&repo, &snapshot, file, &Cancel::default())
                    .unwrap(),
            );
            let inputs = prepared.native_text_inputs().unwrap().unwrap();
            let mut model = plico::text_merge::Merge::new(&inputs[1], &inputs[0], &inputs[2]);
            assert_eq!(model.changes.len(), 2);
            model.choose(0, plico::text_merge::Choice::Yours);
            if index == 1 {
                model.choose(1, plico::text_merge::Choice::Theirs);
            }
            let text = model.text().to_owned();
            if index == 0 {
                partial_text = text.clone();
            }
            cx.update(|w, cx| {
                review.update(cx, |s, cx| {
                    s.apply_merge_result(
                        prepared.clone(),
                        text.clone(),
                        model.state(),
                        Box::new(|success, _, _| assert!(success)),
                        w,
                        cx,
                    )
                })
            });
            cx.run_until_parked();
        }
        assert_eq!(git(&["ls-files", "-u"], true).lines().count(), 6);
        let saved = session.read_with(cx, |s, _| {
            assert!(s.drafts[&s.key(std::path::Path::new("a"))].partial());
            assert!(!s.drafts[&s.key(std::path::Path::new("a"))].resolved());
            assert!(s.drafts[&s.key(std::path::Path::new("b"))].resolved());
            s.drafts[&s.key(std::path::Path::new("a"))].clone()
        });
        let inputs = saved.prepared.native_text_inputs().unwrap().unwrap();
        let mut reopened = plico::text_merge::Merge::new(&inputs[1], &inputs[0], &inputs[2]);
        reopened.restore(
            &saved.decisions,
            &saved
                .prepared
                .saved_text(&repo, &Cancel::default())
                .unwrap(),
        );
        assert_eq!(reopened.state().progress(), (1, 2));
        assert!(
            !session.read_with(cx, |s, _| s.all_reviewed()),
            "reviewed partial files cannot complete the session"
        );
        assert_eq!(reopened.text(), partial_text);
        // Whole-side acceptance asks before overwriting either partial or complete progress.
        for file in &snapshot.files {
            let before = std::fs::read(root.join(&file.path)).unwrap();
            let decisions =
                session.read_with(cx, |s, _| s.drafts[&s.key(&file.path)].decisions.clone());
            cx.update(|w, cx| {
                review.update(cx, |s, cx| {
                    s.accept_conflict_in_session(
                        session.clone(),
                        session
                            .read(cx)
                            .entries
                            .iter()
                            .find(|e| e.file() == file)
                            .unwrap()
                            .clone(),
                        Side::Theirs,
                        w,
                        cx,
                    )
                })
            });
            cx.run_until_parked();
            assert_eq!(std::fs::read(root.join(&file.path)).unwrap(), before);
            assert_eq!(
                session.read_with(cx, |s, _| s.drafts[&s.key(&file.path)].decisions.clone()),
                decisions
            );
            assert_eq!(git(&["ls-files", "-u"], true).lines().count(), 6);
            cx.update(|w, cx| w.close_dialog(cx)); // Cancel
        }
        // Confirmation cannot authorize overwriting a later external disk edit.
        std::fs::write(root.join("a"), "external edit\n").unwrap();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.accept_conflict_in_session_confirmed(
                    session.clone(),
                    session.read(cx).entries[0].clone(),
                    Side::Theirs,
                    w,
                    cx,
                )
            })
        });
        cx.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(root.join("a")).unwrap(),
            "external edit\n"
        );
        assert!(session.read_with(cx, |s, _| s.error.is_some()));
        assert_eq!(
            session.read_with(cx, |s, _| s.drafts[&s.key(std::path::Path::new("a"))]
                .decisions
                .progress()),
            (1, 2)
        );
        std::fs::write(root.join("a"), &partial_text).unwrap();
        // Successful confirmation replaces all decisions but still defers staging.
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.accept_conflict_in_session_confirmed(
                    session.clone(),
                    session.read(cx).entries[1].clone(),
                    Side::Yours,
                    w,
                    cx,
                )
            })
        });
        cx.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(root.join("b")).unwrap(),
            content("main")
        );
        assert_eq!(
            session.read_with(cx, |s, _| s.drafts[&s.key(std::path::Path::new("b"))]
                .decisions
                .progress()),
            (2, 2)
        );
        assert_eq!(git(&["ls-files", "-u"], true).lines().count(), 6);
        // Revert cancellation preserves both text and decisions; external edits block reset.
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.revert_conflict_resolution(
                    session.clone(),
                    session.read(cx).key(std::path::Path::new("b")),
                    w,
                    cx,
                )
            })
        });
        cx.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(root.join("b")).unwrap(),
            content("main")
        );
        assert!(session.read_with(cx, |s, _| {
            s.drafts[&s.key(std::path::Path::new("b"))].resolved()
        }));
        cx.update(|w, cx| w.close_dialog(cx));
        std::fs::write(root.join("b"), "external reset edit\n").unwrap();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.revert_conflict_resolution_confirmed(
                    session.clone(),
                    session.read(cx).key(std::path::Path::new("b")),
                    w,
                    cx,
                )
            })
        });
        cx.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(root.join("b")).unwrap(),
            "external reset edit\n"
        );
        assert!(session.read_with(cx, |s, _| {
            s.drafts.contains_key(&s.key(std::path::Path::new("b"))) && s.error.is_some()
        }));
        std::fs::write(root.join("b"), content("main")).unwrap();
        // Fully and partially resolved files both reset to base, never to conflict markers.
        for file in &snapshot.files {
            let old = session.read_with(cx, |s, _| s.drafts[&s.key(&file.path)].clone());
            let old_text = std::fs::read_to_string(root.join(&file.path)).unwrap();
            cx.update(|w, cx| {
                review.update(cx, |s, cx| {
                    s.revert_conflict_resolution_confirmed(
                        session.clone(),
                        session.read(cx).key(&file.path),
                        w,
                        cx,
                    )
                })
            });
            cx.run_until_parked();
            assert_eq!(
                std::fs::read_to_string(root.join(&file.path)).unwrap(),
                content("base")
            );
            assert!(!session.read_with(cx, |s, _| s.drafts.contains_key(&s.key(&file.path))));
            assert_eq!(git(&["ls-files", "-u"], true).lines().count(), 6);
            // Resume using the same guarded baseline after reset; preserve later close assertions.
            cx.update(|w, cx| {
                review.update(cx, |s, cx| {
                    s.apply_merge_result(
                        old.prepared.clone(),
                        old_text.clone(),
                        old.decisions.clone(),
                        Box::new(|success, _, _| assert!(success)),
                        w,
                        cx,
                    )
                })
            });
            cx.run_until_parked();
        }
        // Finish must not bypass unresolved blocks. Close only asks for confirmation.
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.close_conflict_session(session.clone(), true, w, cx)
            })
        });
        cx.run_until_parked();
        assert_eq!(git(&["ls-files", "-u"], true).lines().count(), 6);
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.close_conflict_session(session.clone(), false, w, cx)
            })
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| assert!(s.conflict_session.is_some()));
        assert_eq!(git(&["ls-files", "-u"], true).lines().count(), 6);
        // Continue Merge dismisses only the confirmation and retains both drafts.
        cx.update(|w, cx| w.close_dialog(cx));
        assert_eq!(session.read_with(cx, |s, _| s.drafts.len()), 2);
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.close_conflict_session_confirmed(session.clone(), false, w, cx)
            })
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| assert!(s.conflict_session.is_none()));
        let unmerged = git(&["ls-files", "-u"], true);
        assert_eq!(unmerged.lines().count(), 3);
        assert!(unmerged.lines().all(|line| line.ends_with("\ta")));
        assert_eq!(
            std::fs::read_to_string(root.join("a")).unwrap(),
            partial_text
        );
        assert_eq!(git(&["rev-parse", "HEAD"], true), head);
        git(&["rev-parse", "MERGE_HEAD"], true);
    }
}
