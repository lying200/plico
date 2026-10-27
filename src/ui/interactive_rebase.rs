//! Log selection -> read-only preparation -> saved documents -> operation-owned plan.
//! Repository identity is captured before asynchronous work, never inferred from HEAD.
use super::*;
use plico::{
    active_operation::{Kind, Phase},
    interactive_rebase::{interaction::Requests as Plans, operation, prepare::Prepared},
    published_commits::Protection,
};

impl Review {
    pub(super) fn can_interactive_rebase(&self, index: usize, cx: &App) -> bool {
        !self.operation.busy()
            && !self.operation.saving_documents()
            && !self.log_query.history_loading()
            && self.log_selection.ordered(&self.commit_ids()).len() == 1
            && self
                .commits
                .get(index)
                .is_some_and(|c| c.parents.len() < 2 && !c.shallow_boundary)
            && self.log_repository(index).is_some_and(|repo| {
                let identity = plico::recent_projects::identity(&repo.path);
                let roots = self.project_roots.read(cx);
                if roots.operations().iter().any(|entry| {
                    plico::recent_projects::identity(&entry.repository.path) == identity
                        && entry.state.as_ref().is_ok_and(|state| state.in_progress())
                }) {
                    return false;
                }
                self.containing_branches.as_ref().is_none_or(|query| {
                    let query = query.read(cx);
                    let hash = &self.commits[index].id;
                    query.contains_head(&repo.path, hash) != Some(false)
                        && query.is_protected(&repo.path, hash) != Some(true)
                })
            })
    }
    pub(super) fn prepare_interactive_rebase(
        &mut self,
        repo: Repository,
        target: String,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(Kind::Rebase, Phase::Preparing, cx) else {
            return;
        };
        let project = self.repo.path.clone();
        self.status_message = Some(t("Checking…").into());
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    Prepared::prepare(&repo, &target, Protection::Defaults, &token).map(Arc::new)
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if cancelled || s.repo.path != project {
                    return;
                }
                s.status_message = None;
                match result {
                    Err(e) => crate::error_ui::show_details(e, "Rebase requires attention", w, cx),
                    Ok(prepared) if prepared.initial() => {
                        let owner = cx.entity().downgrade();
                        w.open_dialog(cx, move |dialog, _, _| {
                            let owner = owner.clone();
                            let prepared = prepared.clone();
                            let project = project.clone();
                            dialog
                                .title(t("Editing Initial Commit"))
                                .w(px(450.))
                                .centered(true)
                                .overlay_closable(false)
                                .child(t("Are you sure you want to edit the initial commit?"))
                                .footer(crate::dialog_style::confirmation(
                                    Button::new("confirm-initial-rebase")
                                        .primary()
                                        .label(t("Continue")),
                                ))
                                .on_ok(move |_, w, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        if s.repo.path == project {
                                            s.execute_interactive_rebase(prepared.clone(), w, cx);
                                        }
                                    });
                                    true
                                })
                        });
                    }
                    Ok(prepared) => s.execute_interactive_rebase(prepared, w, cx),
                }
            });
        });
    }
    fn execute_interactive_rebase(
        &mut self,
        prepared: Arc<Prepared>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(w, cx, move |s, w, cx| {
                s.execute_interactive_rebase(prepared, w, cx)
            });
            return;
        }
        let helper = match std::env::current_exe().map(|p| {
            p.with_file_name(if cfg!(windows) {
                "plico-editor.exe"
            } else {
                "plico-editor"
            })
        }) {
            Ok(path) => path,
            Err(e) => {
                crate::error_ui::show_details(
                    e.to_string(),
                    "Git editor helper is unavailable",
                    w,
                    cx,
                );
                return;
            }
        };
        if let Err(e) = plico::editor::command(&helper, true) {
            crate::error_ui::show_details(e.to_string(), "Git editor helper is unavailable", w, cx);
            return;
        }
        let Some((id, token)) = self.begin_operation(Kind::Rebase, Phase::Executing, cx) else {
            return;
        };
        let project = self.repo.path.clone();
        let repo = prepared.repository();
        let directory_repo = repo.clone();
        let plans = Plans::new(token.clone());
        let worker_plans = plans.clone();
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let progress = plico::operation_process::Progress::default();
        let worker_progress = progress.clone();
        self.status_message = Some(t("Rebasing…").into());
        self.rebase_diagnostic = None;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = operation::execute(
                &prepared,
                true,
                &helper,
                Protection::Defaults,
                &worker_plans,
                &worker_requests,
                &token,
                &worker_progress,
            );
            worker_plans.close();
            let _ = tx.send(result);
        })
        .detach();
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let directory = wx
                .background_spawn(
                    async move { directory_repo.editor_directory(&Cancel::default()) },
                )
                .await;
            let mut plan: Option<Entity<crate::rebase_plan_dialog::PlanDialog>> = None;
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            let mut auth: Option<Entity<crate::auth_dialog::Login>> = None;
            loop {
                if let Some(pending) = plans.take_pending() {
                    let repository = repo.clone();
                    if let Ok(Ok(dialog)) = wx.update(|w, cx| {
                        crate::rebase_plan_dialog::show(repository, project.clone(), pending, w, cx)
                    }) {
                        plan = Some(dialog);
                    }
                }
                if let Some(pending) = requests.take_pending() {
                    if matches!(
                        pending.kind,
                        plico::auth_request::Kind::CommitEditor
                            | plico::auth_request::Kind::SequenceEditor
                    ) {
                        if let Ok(directory) = &directory {
                            let directory = directory.clone();
                            if let Ok(document) = wx
                                .background_spawn(async move {
                                    plico::editor::Document::open(pending, &directory)
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
                    } else if let Ok(dialog) =
                        wx.update(|w, cx| crate::auth_dialog::show(pending, w, cx))
                    {
                        auth = Some(dialog);
                    }
                }
                let result = match rx.try_recv() {
                    Ok(r) => Some(r),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Rebase worker ended".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = this.update_in(wx, |s, w, cx| {
                        let dismissed = plan.as_ref().is_some_and(|p| p.read(cx).dismissed());
                        // The Log action creates a root modal. Only while one of its
                        // owned dialogs remains open can nested descendants be closed.
                        if plan.as_ref().is_some_and(|p| p.read(cx).is_open())
                            || editor.as_ref().is_some_and(|e| e.read(cx).active())
                            || auth.as_ref().is_some_and(|a| a.read(cx).active())
                        {
                            if let Some(plan) = &plan {
                                plan.update(cx, |p, cx| p.close(cx));
                            }
                            w.close_all_dialogs(cx);
                        }
                        let Some(cancelled) = s.finish_operation(id, cx) else {
                            return;
                        };
                        if s.repo.path != project {
                            return;
                        }
                        s.finish_interactive_rebase(
                            repo.clone(),
                            result,
                            cancelled || dismissed,
                            w,
                            cx,
                        );
                    });
                    break;
                }
                let output = operation::progress_text(&progress);
                if !output.is_empty() {
                    let _ = this.update(wx, |s, cx| {
                        if s.repo.path == project
                            && s.rebase_diagnostic.as_deref() != Some(output.as_str())
                        {
                            s.rebase_diagnostic = Some(output.clone());
                            cx.notify();
                        }
                    });
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            }
        });
        cx.notify();
    }
    fn finish_interactive_rebase(
        &mut self,
        repo: Repository,
        result: Result<operation::Report, String>,
        cancelled: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut conflicts = false;
        let mut success = false;
        let mut paused = false;
        let mut editing = false;
        let diagnostic = match &result {
            Err(e) => e.clone(),
            Ok(report) => {
                success = report.succeeded();
                conflicts = report.state.as_ref().is_ok_and(|s| !s.conflicts.is_empty());
                paused = report.state.as_ref().is_ok_and(|s| s.in_progress());
                editing = report.stopped_for_editing();
                report.diagnostic()
            }
        };
        self.rebase_diagnostic = (!diagnostic.is_empty()).then_some(diagnostic.clone());
        self.status_message = Some(
            t(if cancelled {
                "Rebase operation cancelled; repository refreshed"
            } else if conflicts {
                "Rebase requires conflict resolution"
            } else if success {
                "Rebase completed"
            } else if editing {
                "Rebase stopped for editing"
            } else if paused {
                "Rebase is still in progress"
            } else {
                "Rebase requires attention"
            })
            .into(),
        );
        if let Ok(report) = &result {
            if let Some(oid) = &report.saved_oid {
                self.status_message = Some(format!(
                    "{} — {}: {}",
                    self.status_message.as_deref().unwrap_or_default(),
                    t("Saved changes retained"),
                    &oid[..oid.len().min(8)]
                ));
            }
        }
        self.sync_repository_worktree(
            &repo.path,
            plico::worktree_events::Batch {
                all: true,
                index: true,
                ..Default::default()
            },
            true,
            w,
            cx,
        );
        self.user_menu.update(cx, |menu, cx| menu.refresh(cx));
        self.refresh_impl(false, true, None, cx);
        if !cancelled && conflicts {
            self.open_conflicts_in(repo, w, cx);
        } else if !cancelled && !success && !paused {
            crate::error_ui::show_details(diagnostic, "Rebase requires attention", w, cx);
        }
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests;
