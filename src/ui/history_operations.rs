//! Main-window history operations coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn confirm_abort_rebase_in(
        &mut self,
        repository: Repository,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
            return;
        }
        let owner = cx.entity().downgrade();
        let path = repository.path.clone();
        let project_path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let path = path.clone();
            let repository = repository.clone();
            let project_path = project_path.clone();
            dialog
                .title(t("Abort Rebase"))
                .w(px(460.))
                .centered(true)
                .overlay_closable(false)
                .child(
                    div()
                        .whitespace_normal()
                        .child(plico::i18n::abort_rebase_question(
                            &path
                                .file_name()
                                .unwrap_or(path.as_os_str())
                                .to_string_lossy(),
                        )),
                )
                .child(div().truncate().child(path.display().to_string()))
                .footer(crate::dialog_style::confirmation(
                    Button::new("abort-rebase-confirm")
                        .primary()
                        .label(t("Abort")),
                ))
                .on_ok(move |_, window, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == project_path {
                            s.start_rebase_operation_in(repository.clone(), true, window, cx);
                        }
                    });
                    true
                })
        });
    }
    /// Squash edits one repository's history; hashes alone are insufficient.
    pub(super) fn squash_selection_is_single_root(&self) -> bool {
        let ids = self.commit_ids();
        let selected = self.log_selection.ordered(&ids);
        selected.len() >= 2 && selected.iter().all(|id| id.root == selected[0].root)
    }
    pub(super) fn prepare_squash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy()
            || self.settings_open
            || self.welcome
            || !self.squash_selection_is_single_root()
        {
            return;
        }
        let ids = self.commit_ids();
        let selection = self.log_selection.ordered(&ids);
        let repo = Repository::new(selection[0].root.as_ref());
        let selected = selection
            .into_iter()
            .map(|id| id.hash.clone())
            .collect::<Vec<_>>();
        // The project guards the dialog lifetime; the selected root owns Git work.
        let path = self.repo.path.clone();
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
                    let head = plico::commit_operation::head(&repo, &token)?
                        .ok_or("There is no HEAD to squash")?;
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
                                prepared.repository(),
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
            "plico-editor.exe"
        } else {
            "plico-editor"
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
        let repo = prepared.repository();
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
                            let focus = undo.as_ref().and_then(|r| r.as_ref().ok()).and_then(|u| {
                                u.commit_to_focus()
                                    .map(|hash| plico::project_log::CommitId {
                                        root: Arc::new(u.repository().path),
                                        hash: hash.to_owned(),
                                    })
                            });
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
                                s.navigate_log_commit(
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
        repo: Repository,
        target: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() || self.welcome {
            return;
        }
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
                            let include_repository = prepared.repository().path != s.repo.path
                                || s.project_roots.read(cx).has_multiple_roots();
                            crate::reset_dialog::show(
                                prepared.clone(),
                                include_repository,
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
        let repo = prepared.repository();
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
        let repo = undo.repository();
        let target = plico::project_log::CommitId {
            root: Arc::new(repo.path.clone()),
            hash: undo.commit_to_focus_on_undo().to_owned(),
        };
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
                        s.navigate_log_commit(
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
    /// Keep the operation owner independent of the project window's primary root.
    pub(super) fn start_rebase_operation_in(
        &mut self,
        repository: Repository,
        abort: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            let project_path = self.repo.path.clone();
            self.with_saved_documents(window, cx, move |s, w, cx| {
                if s.repo.path == project_path {
                    s.start_rebase_operation_in(repository, abort, w, cx);
                }
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
                        "plico-editor.exe"
                    } else {
                        "plico-editor"
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
        let repo = repository.clone();
        let project_path = self.repo.path.clone();
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
                            if s.repo.path != project_path {
                                return;
                            }
                            s.rebase_diagnostic = plico::rebase_operation::diagnostic(&result);
                            let undo = result.as_mut().ok().and_then(|r| r.squash_undo.take());
                            let focus = undo.as_ref().and_then(|u| {
                                u.commit_to_focus()
                                    .map(|hash| plico::project_log::CommitId {
                                        root: Arc::new(u.repository().path),
                                        hash: hash.to_owned(),
                                    })
                            });
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
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            if let Some(target) = focus {
                                s.navigate_log_commit(
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
    pub(super) fn start_merge_operation_in(
        &mut self,
        repository: Repository,
        action: plico::merge_operation::Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_merge_operation_in(repository, action, w, cx)
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
        let repo = repository.clone();
        let project_path = self.repo.path.clone();
        let pending = self.pending_branch_merge.clone().filter(|p| {
            plico::recent_projects::identity(&p.repository().path)
                == plico::recent_projects::identity(&repository.path)
        });
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    let candidate = pending.filter(|p| {
                        matches!(action, plico::merge_operation::Action::Commit)
                            && p.matches_pending_merge(&repo, &token).unwrap_or(false)
                    });
                    let result =
                        plico::merge_operation::execute(&repo, action, &token, &Default::default());
                    let completed = result.as_ref().is_ok_and(|r| {
                        r.command.as_ref().is_ok_and(|o| {
                            o.status.success() && o.stopped.is_none() && o.input_error.is_none()
                        }) && r
                            .state
                            .as_ref()
                            .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
                            && r.restore.as_ref().is_none_or(|r| r.is_ok())
                    });
                    let proposal = candidate.filter(|p| {
                        completed && p.prepare_deletion(&repo, &Cancel::default()).is_ok()
                    });
                    (result, proposal)
                })
                .await;
            let _ = wx.update(|w, cx| {
                let _ = this.update(cx, |s, cx| {
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    if s.repo.path != project_path {
                        return;
                    }
                    let (result, proposal) = result;
                    if !cancelled {
                        s.merged_branch = proposal;
                    }
                    if result
                        .as_ref()
                        .is_ok_and(|r| r.state.as_ref().is_ok_and(|s| !s.in_progress()))
                    {
                        if s.pending_branch_merge.as_ref().is_some_and(|p| {
                            plico::recent_projects::identity(&p.repository().path)
                                == plico::recent_projects::identity(&repository.path)
                        }) {
                            s.pending_branch_merge = None;
                        }
                    }
                    let restore_error = result
                        .as_ref()
                        .ok()
                        .and_then(|r| r.restore.as_ref())
                        .and_then(|r| r.as_ref().err())
                        .cloned();
                    let command_error = plico::merge_operation::diagnostic(&result);
                    let message = if cancelled {
                        "Merge operation cancelled"
                    } else {
                        match result {
                            Ok(_) if restore_error.is_some() => match action {
                                plico::merge_operation::Action::Commit => {
                                    "Merge completed; saved changes require attention"
                                }
                                plico::merge_operation::Action::Abort => {
                                    "Merge aborted; saved changes require attention"
                                }
                            },
                            Ok(result) => match (result.command, result.state) {
                                (Ok(outcome), Ok(state))
                                    if outcome.status.success()
                                        && outcome.stopped.is_none()
                                        && outcome.input_error.is_none()
                                        && !state.in_progress()
                                        && state.conflicts.is_empty() =>
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
                    if let Some(error) = command_error {
                        crate::error_ui::show_details(error, "Merge requires attention", w, cx);
                    }
                    s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                    s.refresh_impl(false, true, None, cx);
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod selection_tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;

    #[gpui::test]
    fn rebase_abort_targets_child_and_preserves_parent(cx: &mut gpui::TestAppContext) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn git(root: &std::path::Path, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-rebase-root-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        let parent = fixture.0.join("parent");
        let child = fixture.0.join("child");
        let mut original = Vec::new();
        for root in [&parent, &child] {
            std::fs::create_dir_all(root).unwrap();
            git(root, &["init", "-q", "-b", "main"]);
            git(root, &["config", "user.name", "Test"]);
            git(root, &["config", "user.email", "test@example.test"]);
            std::fs::write(root.join("same.txt"), "base\n").unwrap();
            git(root, &["add", "."]);
            git(root, &["commit", "-qm", "base"]);
            git(root, &["checkout", "-qb", "topic"]);
            std::fs::write(root.join("same.txt"), "topic\n").unwrap();
            git(root, &["commit", "-qam", "topic"]);
            original.push(git(root, &["rev-parse", "HEAD"]));
            git(root, &["checkout", "main"]);
            std::fs::write(root.join("same.txt"), "main\n").unwrap();
            git(root, &["commit", "-qam", "main"]);
            git(root, &["checkout", "topic"]);
            let result = std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["rebase", "main"])
                .output()
                .unwrap();
            assert!(!result.status.success());
            assert!(root.join(".git/rebase-merge").exists());
        }
        let parent_head = git(&parent, &["rev-parse", "HEAD"]);
        let parent_index = git(&parent, &["ls-files", "-u"]);
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.welcome = false;
                s.repo = Repository::new(&parent);
                s.start_rebase_operation_in(Repository::new(&child), true, window, cx);
                assert!(s.operation.busy());
            })
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(!s.operation.busy());
            assert!(s.rebase_diagnostic.is_none(), "{:?}", s.rebase_diagnostic);
        });
        assert_eq!(git(&child, &["rev-parse", "HEAD"]), original[1]);
        assert!(!child.join(".git/rebase-merge").exists());
        assert!(git(&child, &["ls-files", "-u"]).is_empty());
        assert_eq!(git(&parent, &["rev-parse", "HEAD"]), parent_head);
        assert_eq!(git(&parent, &["ls-files", "-u"]), parent_index);
        assert!(parent.join(".git/rebase-merge").exists());
    }

    #[gpui::test]
    fn squash_prepares_selected_child_without_primary_head(cx: &mut gpui::TestAppContext) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
                "plico-squash-owner-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        let child = fixture.0.join("child");
        std::fs::create_dir_all(&child).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.test"],
            vec!["commit", "--allow-empty", "-m", "base"],
            vec!["commit", "--allow-empty", "-m", "first"],
            vec!["commit", "--allow-empty", "-m", "second"],
        ] {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&child)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        }
        let commits = Repository::new(&child)
            .snapshot(None, 2, None, &Cancel::default())
            .unwrap()
            .commits;
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.welcome = false;
                s.repo = Repository::new(&fixture.0); // No primary Git repository or HEAD.
                s.log = log_display::Display::fixture_single(&s.repo.path, commits);

                let ids = s
                    .log
                    .commits()
                    .iter()
                    .map(|commit| plico::project_log::CommitId {
                        root: Arc::new(child.clone()),
                        hash: commit.id.clone(),
                    })
                    .collect::<Vec<_>>();
                s.log = s.log.fixture_identified(ids.clone());
                s.log_selection.select(&ids, 0, false, false);
                s.log_selection.select(&ids, 1, false, true);
                s.prepare_squash(window, cx);
                assert!(
                    s.operation.busy(),
                    "child selection must begin preparation without primary HEAD"
                );
            })
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(!s.operation.busy());
            assert!(s.status_message.is_none(), "{:?}", s.status_message);
            assert!(s.rebase_diagnostic.is_none(), "{:?}", s.rebase_diagnostic);
        });
    }

    #[gpui::test]
    fn squash_rejects_cross_root_selection_before_starting_work(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.welcome = false;
                s.log = log_display::Display::fixture_single(
                    &s.repo.path,
                    ["a", "b"]
                        .into_iter()
                        .map(|hash| Commit {
                            id: hash.repeat(40),
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
                let child = Arc::new(PathBuf::from("child"));
                let mut ids = s
                    .log
                    .commits()
                    .iter()
                    .map(|commit| plico::project_log::CommitId {
                        root: child.clone(),
                        hash: commit.id.clone(),
                    })
                    .collect::<Vec<_>>();
                s.log = s.log.fixture_identified(ids.clone());
                s.log_selection.select(&ids, 0, false, false);
                assert!(!s.squash_selection_is_single_root());
                s.log_selection.select(&ids, 1, false, true);
                assert!(s.squash_selection_is_single_root());
                ids[1].root = Arc::new(PathBuf::from("parent"));
                s.log = s.log.fixture_identified(ids.clone());
                s.log_selection.select(&ids, 0, false, false);
                s.log_selection.select(&ids, 1, false, true);
                assert!(!s.squash_selection_is_single_root());
                s.prepare_squash(window, cx);
                assert!(!s.operation.busy());
                assert!(s.status_message.is_none());
                s.log = s.log.fixture_identified(vec![ids[0].clone()]);
                assert!(!s.squash_selection_is_single_root());
            })
        });
    }
}
