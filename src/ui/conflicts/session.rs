//! Iterative conflict choices keep their guarded write baseline until dialog close.
use super::*;

fn stage_error(error: String) -> String {
    match error.as_str() {
        "Conflict resolution interrupted" => t("Conflict resolution interrupted").into(),
        "The file is still unresolved" => t("The file is still unresolved").into(),
        _ => error,
    }
}

impl Review {
    pub(super) fn owns_conflict_session(&self, view: &Entity<ConflictList>, cx: &App) -> bool {
        self.repo.path == view.read(cx).repository
            && self
                .conflict_session
                .as_ref()
                .is_some_and(|current| current == view)
    }

    pub(super) fn accept_conflict_in_session(
        &mut self,
        view: Entity<ConflictList>,
        entry: Entry,
        side: Side,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_conflict_session(&view, cx) || view.read(cx).busy {
            return;
        }
        let has_progress = view
            .read(cx)
            .drafts
            .get(entry.key())
            .is_some_and(|draft| draft.decisions.progress().0 > 0);
        if has_progress {
            let owner = cx.entity().downgrade();
            let message = t(match side {
                Side::Yours => {
                    "This file already contains merged changes. Accepting Yours will discard and overwrite them."
                }
                Side::Theirs => {
                    "This file already contains merged changes. Accepting Theirs will discard and overwrite them."
                }
            });
            window.open_dialog(cx, move |d, _, _| {
                let owner = owner.clone();
                let view = view.clone();
                let entry = entry.clone();
                d.title(t("Overwrite Changes"))
                    .w(px(480.))
                    .centered(true)
                    .overlay_closable(false)
                    .child(div().whitespace_normal().child(message))
                    .footer(crate::dialog_style::confirmation(
                        Button::new("discard-and-accept")
                            .primary()
                            .label(t("Discard and Accept")),
                    ))
                    .on_ok(move |_, w, cx| {
                        w.close_dialog(cx);
                        let _ = owner.update(cx, |s, cx| {
                            s.accept_conflict_in_session_confirmed(
                                view.clone(),
                                entry.clone(),
                                side,
                                w,
                                cx,
                            )
                        });
                        false
                    })
            });
            return;
        }
        self.accept_conflict_in_session_confirmed(view, entry, side, window, cx);
    }

    pub(super) fn accept_conflict_in_session_confirmed(
        &mut self,
        view: Entity<ConflictList>,
        entry: Entry,
        side: Side,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_conflict_session(&view, cx) || view.read(cx).busy {
            return;
        }
        let snapshot = entry.snapshot().clone();
        let file = entry.file().clone();
        if !view.read(cx).contains(&snapshot, &file) {
            return;
        }
        let saved = view.read(cx).drafts.get(entry.key()).cloned();
        // Deletions and symbolic links have no editable text model in Rebased.
        let iterative = file
            .stages
            .iter()
            .flatten()
            .all(|v| v.mode != "120000" && v.mode != "160000")
            && file.side(Side::Yours, snapshot.reversed).is_some()
            && file.side(Side::Theirs, snapshot.reversed).is_some();
        if !iterative {
            window.close_dialog(cx);
            self.resolve_conflict(snapshot, file, Resolution::Accept(side), window, cx);
            return;
        }
        let Some((id, token)) =
            self.begin_conflict_operation(plico::active_operation::Phase::Executing, cx)
        else {
            return;
        };
        view.update(cx, |s, cx| {
            s.busy = true;
            s.error = None;
            cx.notify();
        });
        let repo = snapshot.repository();
        let repository = repo.clone();
        let path = Key::new(&snapshot.repository(), &file.path);
        let access = id.access.clone();
        self.conflict_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    let prepared = match saved {
                        Some(saved) => saved.prepared,
                        None => access.prepare(&repo, &snapshot, &file, &token)?,
                    };
                    let Some(inputs) = prepared.native_text_inputs()? else {
                        return access
                            .resolve(&repo, &snapshot, &file, Resolution::Accept(side), &token)
                            .map_err(stage_error)
                            .map(|snapshot| (None, Some(snapshot)));
                    };
                    let text = match side {
                        Side::Yours => inputs[0].clone(),
                        Side::Theirs => inputs[2].clone(),
                    };
                    let mut model =
                        plico::text_merge::Merge::new(&inputs[1], &inputs[0], &inputs[2]);
                    for i in 0..model.changes.len() {
                        model.choose(
                            i,
                            match side {
                                Side::Yours => plico::text_merge::Choice::Yours,
                                Side::Theirs => plico::text_merge::Choice::Theirs,
                            },
                        );
                    }
                    prepared.write_text_result(text)?;
                    if let Err(error) = access.apply(&prepared, &repo, &token) {
                        prepared.retain_output();
                        return Err(error);
                    }
                    Ok((
                        Some(SavedResolution {
                            prepared,
                            decisions: model.state(),
                            reviewed: true,
                        }),
                        None,
                    ))
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_conflict_operation(id, cx).is_none() {
                    return;
                }
                if !s.owns_conflict_session(&view, cx) {
                    return;
                }
                view.update(cx, |state, cx| {
                    state.busy = false;
                    match result {
                        Ok((Some(draft), _)) => {
                            state.automatic.invalidate(&path);
                            state.drafts.insert(path, draft);
                        }
                        Ok((_, Some(snapshot))) => {
                            state.replace_snapshot(Arc::new(snapshot));
                        }
                        Ok(_) => {}
                        Err(error) => state.error = Some(error),
                    }
                    cx.notify();
                });
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
                cx.notify();
            });
        });
    }

    pub(super) fn close_conflict_session(
        &mut self,
        view: Entity<ConflictList>,
        finish: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_conflict_session(&view, cx) || view.read(cx).busy {
            return;
        }
        if finish && !view.read(cx).all_resolved() {
            return;
        }
        if view.read(cx).drafts.values().any(SavedResolution::partial) {
            let owner = cx.entity().downgrade();
            window.open_dialog(cx, move |d, _, _| {
                let owner = owner.clone();
                let view = view.clone();
                let continued = view.clone();
                d.title(t("Discard Partial Merge Progress?")).w(px(480.)).centered(true).overlay_closable(false)
                    .child(div().whitespace_normal().child(t("Closing the dialog discards resolution progress for partially resolved files, but keeps their saved contents. Fully resolved files will be staged.")))
                    .footer(gpui_kit::component::dialog::DialogFooter::new()
                        .child(gpui_kit::component::dialog::DialogAction::new().child(Button::new("discard-partial-merge").primary().label(t("Discard Progress"))))
                        .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.label(t("Continue Merge")))))
                    .on_cancel(move |_, _, cx| {
                        continued.update(cx, |s, _| s.after_close = None);
                        true
                    })
                    .on_ok(move |_, w, cx| {
                        w.close_dialog(cx);
                        let _ = owner.update(cx, |s, cx| s.close_conflict_session_confirmed(view.clone(), finish, w, cx));
                        false
                    })
            });
            return;
        }
        self.close_conflict_session_confirmed(view, finish, window, cx);
    }

    pub(super) fn close_conflict_session_confirmed(
        &mut self,
        view: Entity<ConflictList>,
        finish: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_conflict_session(&view, cx) || view.read(cx).busy {
            return;
        }
        if finish && !view.read(cx).all_resolved() {
            return;
        }
        let drafts = view
            .read(cx)
            .drafts
            .values()
            .filter(|draft| draft.resolved())
            .cloned()
            .collect::<Vec<_>>();
        if drafts.is_empty() {
            self.finish_conflict_session(view, finish, window, cx);
            return;
        }
        let Some((id, token)) =
            self.begin_conflict_operation(plico::active_operation::Phase::Executing, cx)
        else {
            return;
        };
        view.update(cx, |s, cx| {
            s.busy = true;
            s.error = None;
            cx.notify();
        });
        let access = id.access.clone();
        self.conflict_task = cx.spawn_in(window, async move |this, wx| {
            let staging = wx
                .background_spawn(async move {
                    let prepared = drafts.into_iter().map(|d| d.prepared).collect::<Vec<_>>();
                    access.stage_resolved(&prepared, &token)
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_conflict_operation(id, cx) else {
                    return;
                };
                if !s.owns_conflict_session(&view, cx) {
                    return;
                }
                let success = staging.error.is_none() && !cancelled;
                view.update(cx, |state, cx| {
                    for completed in &staging.completed {
                        state.drafts.remove(&completed.key);
                    }
                    for snapshot in staging.snapshots.values() {
                        state.replace_snapshot(Arc::new(snapshot.clone()));
                    }
                    state.busy = false;
                    state.error = staging
                        .error
                        .map(stage_error)
                        .or_else(|| cancelled.then(|| t("Conflict resolution interrupted").into()));
                    cx.notify();
                });
                for touched in staging.touched.values() {
                    s.sync_repository_worktree(
                        &touched.path,
                        plico::worktree_events::Batch {
                            all: true,
                            index: true,
                            ..Default::default()
                        },
                        true,
                        w,
                        cx,
                    );
                }
                if success {
                    s.finish_conflict_session(view, finish, w, cx);
                }
                cx.notify();
            });
        });
    }

    pub(super) fn leave_conflict_session(
        &mut self,
        view: Entity<ConflictList>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_conflict_session(&view, cx) || view.read(cx).busy {
            return;
        }
        view.update(cx, |state, _| {
            for draft in state.drafts.values() {
                draft.prepared.retain_output();
            }
            state.after_close = None;
        });
        self.finish_conflict_session(view, false, window, cx);
    }

    fn finish_conflict_session(
        &mut self,
        view: Entity<ConflictList>,
        finish: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let continuation = view.read(cx).continuation.clone();
        let open_file = view.read(cx).after_close.clone();
        if let Some(Continuation::Update(client)) = &continuation {
            let decision = if finish {
                plico::project_update::conflicts::Decision::Continue
            } else {
                plico::project_update::conflicts::Decision::Close
            };
            if !client.finish(decision) && client.active() {
                view.update(cx, |state, cx| {
                    state.error = Some(t("Conflict resolution interrupted").into());
                    cx.notify();
                });
                return;
            }
            window.close_dialog(cx);
            self.conflict_session = None;
            if let Some((root, path)) = open_file {
                self.open_project_file_in(root.path, path, false, window, cx);
            }
            cx.notify();
            return;
        }
        window.close_dialog(cx);
        self.conflict_session = None;
        if finish
            && continuation
                .as_ref()
                .is_some_and(|repo| repo.resume(self, window, cx))
        {
            return;
        }
        if self
            .sequence_conflict
            .as_ref()
            .is_some_and(|(_, root, _, _)| {
                continuation
                    .as_ref()
                    .is_some_and(|repo| root == &repo.repository().path)
            })
        {
            self.sequence_conflict = None;
        }
        if let Some((root, path)) = open_file {
            self.open_project_file_in(root.path, path, false, window, cx);
        }
        // A separately opened conflict dialog must not create a merge commit.
        self.refresh_impl(false, true, None, cx);
    }
}
