//! Main-window history operations coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn confirm_abort_rebase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() {
            return;
        }
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let path = path.clone();
            dialog
                .title(t("Abort Rebase"))
                .w(px(460.))
                .centered(true)
                .overlay_closable(false)
                .child(div().child(t("Abort the rebase in this repository?")))
                .child(div().truncate().child(path.display().to_string()))
                .footer(crate::dialog_style::confirmation(
                    Button::new("abort-rebase-confirm")
                        .primary()
                        .label(t("Abort")),
                ))
                .on_ok(move |_, window, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path {
                            s.start_rebase_operation(true, window, cx);
                        }
                    });
                    true
                })
        });
    }
    pub(super) fn prepare_squash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.settings_open || self.welcome {
            return;
        }
        let ids = self.commit_ids();
        let selected = self
            .log_selection
            .ordered(&ids)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if selected.len() < 2 {
            return;
        }
        let Some(head) = self.snapshot.as_ref().and_then(|s| s.head_id.clone()) else {
            return;
        };
        let repo = self.repo.clone();
        let path = repo.path.clone();
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Squash,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Preparing Squash…").into());
        self.rebase_diagnostic = None;
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    use plico::{published_commits::Protection, squash_check};
                    let prepared = squash_check::prepare(
                        &repo,
                        &head,
                        &selected,
                        Protection::Defaults,
                        &token,
                    )?;
                    if prepared.state.in_progress() || !prepared.state.conflicts.is_empty() {
                        return Err(
                            "Finish the current Git operation before starting Squash".to_owned()
                        );
                    }
                    let draft = squash_check::message(&repo, &selected, &token)?;
                    Ok((std::sync::Arc::new(prepared), draft))
                })
                .await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    if s.repo.path != path || cancelled {
                        cx.notify();
                        return;
                    }
                    match result {
                        Ok((prepared, draft)) => {
                            s.status_message = None;
                            let owner = cx.entity().downgrade();
                            let path = path.clone();
                            crate::squash_dialog::show(
                                s.repo.clone(),
                                prepared.clone(),
                                draft,
                                window,
                                cx,
                                move |message, window, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        if s.repo.path == path {
                                            s.start_squash(prepared.clone(), message, window, cx);
                                        }
                                    });
                                },
                            );
                        }
                        Err(error) => {
                            s.status_message =
                                Some(t("Could not squash commits; repository refreshed").into());
                            s.rebase_diagnostic = Some(error);
                        }
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn start_squash(
        &mut self,
        prepared: std::sync::Arc<plico::squash_check::Prepared>,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_squash(prepared, message, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.welcome {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let helper = exe.with_file_name(if cfg!(windows) {
            "gitui-editor.exe"
        } else {
            "gitui-editor"
        });
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Squash,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Squashing commits…").into());
        self.rebase_diagnostic = None;
        self.squash_undo = None;
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = plico::squash_operation::execute(
                &repo,
                &prepared,
                &message,
                &helper,
                plico::published_commits::Protection::Defaults,
                &token,
                &Default::default(),
                Some(&worker_requests),
            );
            let undo = result
                .as_ref()
                .ok()
                .filter(|r| r.succeeded())
                .map(|report| {
                    plico::squash_undo::Undo::capture(&repo, report, &Cancel::default())
                        .map(Arc::new)
                });
            let _ = tx.send((result, undo));
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            let directory = wx
                .background_spawn(
                    async move { directory_repo.editor_directory(&Cancel::default()) },
                )
                .await;
            loop {
                if let Some(pending) = requests.take_pending() {
                    if let Ok(dir) = &directory {
                        let dir = dir.clone();
                        if let Ok(document) = wx
                            .background_spawn(async move {
                                plico::editor::Document::open(pending, &dir)
                            })
                            .await
                        {
                            if let Ok(dialog) =
                                wx.update(|w, cx| crate::editor_dialog::show(document, w, cx))
                            {
                                editor = Some(dialog);
                            }
                        }
                    }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some((Err("Squash worker ended".into()), None))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some((result, undo)) = result {
                    let _ = wx.update(|w, cx| {
                        if editor.as_ref().is_some_and(|d| d.read(cx).active()) {
                            w.close_dialog(cx);
                        }
                        let _ = this.update(cx, |s, cx| {
                            if s.finish_operation(operation, cx).is_none() {
                                return;
                            }
                            let success = result.as_ref().is_ok_and(|r| r.succeeded());
                            s.rebase_diagnostic = match result {
                                Err(error) => Some(error),
                                Ok(report) => {
                                    let mut notes = Vec::new();
                                    if let Some(error) = report.editor_error {
                                        notes.push(error);
                                    }
                                    if let Some(error) = report.recovery_error {
                                        notes.push(error);
                                    }
                                    if let Some(oid) = report.saved_oid {
                                        notes.push(format!("Saved local changes: {oid}"));
                                    }
                                    if let Some(Err(error)) = report.restore {
                                        notes.push(error);
                                    }
                                    let diagnostic = plico::rebase_operation::diagnostic(&Ok(
                                        plico::rebase_operation::ResultState {
                                            squash_undo: None,
                                            command: report.command,
                                            state: report.state,
                                            recovery_error: None,
                                        },
                                    ));
                                    if let Some(error) = diagnostic {
                                        notes.push(error);
                                    }
                                    (!notes.is_empty()).then(|| notes.join("\n"))
                                }
                            };
                            let focus = undo
                                .as_ref()
                                .and_then(|r| r.as_ref().ok())
                                .and_then(|u| u.commit_to_focus())
                                .map(str::to_owned);
                            match undo {
                                Some(Ok(undo)) => s.squash_undo = Some(undo),
                                Some(Err(error)) => s.rebase_diagnostic = Some(error),
                                None => {}
                            }
                            s.status_message = Some(
                                t(if success {
                                    "Commits squashed"
                                } else {
                                    "Could not squash commits; repository refreshed"
                                })
                                .into(),
                            );
                            s.operation_state = None;
                            s.user_menu.update(cx, |m, cx| m.refresh(cx));
                            if let Some(target) = focus {
                                s.navigate_commit_impl(
                                    t("Commits squashed").into(),
                                    target,
                                    false,
                                    true,
                                    w,
                                    cx,
                                );
                            } else {
                                s.refresh_impl(false, true, None, cx);
                            }
                            cx.notify();
                        });
                    });
                    break;
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            }
        });
        cx.notify();
    }
    pub(super) fn prepare_reset(
        &mut self,
        target: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() || self.welcome {
            return;
        }
        let repo = self.repo.clone();
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Reset,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Checking…").into());
        self.rebase_diagnostic = None;
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::reset_operation::Prepared::prepare(&repo, target, &token)
                })
                .await;
            let _ = wx.update(|w, cx| {
                let _ = this.update(cx, |s, cx| {
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    s.status_message = None;
                    if cancelled {
                        cx.notify();
                        return;
                    }
                    match result {
                        Err(e) => {
                            s.rebase_diagnostic = Some(e);
                            s.status_message = Some(t("Reset failed").into());
                        }
                        Ok(prepared) => {
                            let prepared = Arc::new(prepared);
                            let owner = cx.entity().downgrade();
                            crate::reset_dialog::show(
                                prepared.clone(),
                                w,
                                cx,
                                move |mode, w, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        s.start_reset(prepared.clone(), mode, w, cx)
                                    });
                                },
                            );
                        }
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn start_reset(
        &mut self,
        prepared: Arc<plico::reset_operation::Prepared>,
        mode: plico::reset_operation::Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_reset(prepared, mode, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.welcome {
            return;
        }
        let repo = self.repo.clone();
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Reset,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };

        self.status_message =
            Some(t("Resetting… This operation cannot be cancelled once started.").into());
        self.rebase_diagnostic = None;
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    prepared.execute(&repo, mode, &token, &Default::default())
                })
                .await;
            let _ = wx.update(|w, cx| {
                let _ = this.update(cx, |s, cx| {
                    if s.finish_operation(operation, cx).is_none() {
                        return;
                    }

                    let success = result.as_ref().is_ok_and(|r| {
                        r.command
                            .as_ref()
                            .is_ok_and(|o| o.status.success() && o.stopped.is_none())
                            && r.recovery_error.is_none()
                            && r.state.is_ok()
                    });
                    s.rebase_diagnostic = plico::rebase_operation::diagnostic(&result);
                    s.status_message = Some(
                        t(if success {
                            "Reset successful"
                        } else {
                            "Reset failed"
                        })
                        .into(),
                    );
                    s.squash_undo = None;
                    s.operation_state = None;
                    for doc in s.documents.all() {
                        doc.update(cx, |d, cx| {
                            if !d.pending(cx) {
                                d.reload(w, cx);
                            }
                        });
                    }
                    if let Some(tree) = &s.project_view {
                        tree.update(cx, |v, cx| v.refresh(cx));
                    }
                    s.user_menu.update(cx, |m, cx| m.refresh(cx));
                    s.refresh_impl(false, true, None, cx);
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn undo_squash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| s.undo_squash(w, cx));
            return;
        }
        if self.operation.busy() || self.welcome {
            return;
        }
        let Some(undo) = self.squash_undo.clone() else {
            return;
        };
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::UndoSquash,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Undoing Squash…").into());
        self.rebase_diagnostic = None;
        let repo = self.repo.clone();
        let target = undo.commit_to_focus_on_undo().to_owned();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    undo.execute(
                        &repo,
                        plico::published_commits::Protection::Defaults,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = wx.update(|w, cx| {
                let _ = this.update(cx, |s, cx| {
                    if s.finish_operation(operation, cx).is_none() {
                        return;
                    }
                    let success = result.as_ref().is_ok_and(|r| r.succeeded());
                    s.rebase_diagnostic = match result {
                        Err(error) => {
                            s.squash_undo = None;
                            Some(error)
                        }
                        Ok(report) => plico::rebase_operation::diagnostic(&Ok(
                            plico::rebase_operation::ResultState {
                                squash_undo: None,
                                command: report.command,
                                state: report.state,
                                recovery_error: report.head.err(),
                            },
                        )),
                    };
                    s.status_message = Some(
                        t(if success {
                            "Squash undone"
                        } else {
                            "Could not undo Squash"
                        })
                        .into(),
                    );
                    s.operation_state = None;
                    s.user_menu.update(cx, |m, cx| m.refresh(cx));
                    if success {
                        s.squash_undo = None;
                        s.navigate_commit_impl(
                            t("Squash undone").into(),
                            target,
                            false,
                            true,
                            w,
                            cx,
                        );
                    } else {
                        s.refresh_impl(false, true, None, cx);
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn start_rebase_operation(
        &mut self,
        abort: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_rebase_operation(abort, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.welcome {
            return;
        }
        self.rebase_diagnostic = None;
        let editor_command = if abort {
            None
        } else {
            let command = std::env::current_exe().and_then(|exe| {
                plico::editor::command(
                    &exe.with_file_name(if cfg!(windows) {
                        "gitui-editor.exe"
                    } else {
                        "gitui-editor"
                    }),
                    false,
                )
            });
            match command {
                Ok(command) => Some(command),
                Err(_) => {
                    self.status_message = Some(t("Git editor helper is unavailable").into());
                    cx.notify();
                    return;
                }
            }
        };
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Rebase,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Updating rebase state…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            use plico::rebase_operation::{self, Action};
            let action = match editor_command.as_ref() {
                Some(editor) => Action::Continue {
                    editor,
                    requests: Some(&worker_requests),
                },
                None => Action::Abort,
            };
            let _ = tx.send(rebase_operation::execute(
                &repo,
                action,
                &token,
                &Default::default(),
            ));
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            let directory = wx
                .background_spawn(
                    async move { directory_repo.editor_directory(&Cancel::default()) },
                )
                .await;
            loop {
                if let Some(pending) = requests.take_pending() {
                    if let Ok(dir) = &directory {
                        let dir = dir.clone();
                        if let Ok(document) = wx
                            .background_spawn(async move {
                                plico::editor::Document::open(pending, &dir)
                            })
                            .await
                        {
                            if let Ok(dialog) =
                                wx.update(|w, cx| crate::editor_dialog::show(document, w, cx))
                            {
                                editor = Some(dialog);
                            }
                        }
                    }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Rebase worker ended".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(mut result) = result {
                    let _ = wx.update(|w, cx| {
                        if editor.as_ref().is_some_and(|d| d.read(cx).active()) {
                            w.close_dialog(cx);
                        }
                        let _ = this.update(cx, |s, cx| {
                            let Some(cancelled) = s.finish_operation(operation, cx) else {
                                return;
                            };
                            s.rebase_diagnostic = plico::rebase_operation::diagnostic(&result);
                            let undo = result.as_mut().ok().and_then(|r| r.squash_undo.take());
                            let focus = undo
                                .as_ref()
                                .and_then(|u| u.commit_to_focus())
                                .map(str::to_owned);
                            if let Some(undo) = undo {
                                s.squash_undo = Some(Arc::new(undo));
                            }
                            let message = if cancelled {
                                "Rebase operation cancelled; repository refreshed"
                            } else {
                                match result {
                                    Ok(result) if result.recovery_error.is_some() => {
                                        "Rebase operation failed"
                                    }
                                    Ok(result) => match (result.command, result.state) {
                                        (Err(_), _) if abort => "Rebase operation failed",
                                        (Ok(outcome), _)
                                            if abort
                                                && (!outcome.status.success()
                                                    || outcome.stopped.is_some()) =>
                                        {
                                            "Rebase operation failed"
                                        }
                                        (_, Ok(state)) if !state.conflicts.is_empty() => {
                                            "Rebase requires conflict resolution"
                                        }
                                        (_, Ok(state)) if state.in_progress() => {
                                            "Rebase is still in progress"
                                        }
                                        (Ok(outcome), Ok(_))
                                            if outcome.status.success()
                                                && outcome.stopped.is_none() =>
                                        {
                                            if abort {
                                                "Rebase aborted"
                                            } else {
                                                "Rebase completed"
                                            }
                                        }
                                        _ => "Rebase operation failed",
                                    },
                                    Err(_) => "Rebase operation failed",
                                }
                            };
                            s.operation_state = None;
                            s.status_message = Some(t(message).into());
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            if let Some(target) = focus {
                                s.navigate_commit_impl(
                                    t("Commits squashed").into(),
                                    target,
                                    false,
                                    true,
                                    w,
                                    cx,
                                );
                            } else {
                                s.refresh_impl(false, true, None, cx);
                            }
                            cx.notify();
                        });
                    });
                    break;
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            }
        });
        cx.notify();
    }
    pub(super) fn start_merge_operation(
        &mut self,
        action: plico::merge_operation::Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_merge_operation(action, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Merge,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Updating merge state…").into());
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::merge_operation::execute(&repo, action, &token, &Default::default())
                })
                .await;
            let _ = wx.update(|_, cx| {
                let _ = this.update(cx, |s, cx| {
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    let message = if cancelled {
                        "Merge operation cancelled"
                    } else {
                        match result {
                            Ok(result) => match (result.command, result.state) {
                                (Ok(outcome), Ok(state))
                                    if outcome.status.success()
                                        && outcome.stopped.is_none()
                                        && !state.in_progress() =>
                                {
                                    match action {
                                        plico::merge_operation::Action::Commit => "Merge completed",
                                        plico::merge_operation::Action::Abort => "Merge aborted",
                                    }
                                }
                                _ => "Merge operation failed",
                            },
                            Err(_) => "Merge operation failed",
                        }
                    };
                    s.operation_state = None;
                    s.status_message = Some(t(message).into());
                    s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                    s.refresh_impl(false, true, None, cx);
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
}
