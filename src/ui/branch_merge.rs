//! Selected branch merge and GitSmartOperationDialog-style explicit retry.
use super::*;
use plico::branch_merge::{Prepared, Report, Status};

impl Review {
    pub(super) fn can_merge_branch(&self, reference: &str) -> bool {
        !self.operation.busy()
            && !self.operation.saving_documents()
            && (reference.starts_with("refs/heads/") || reference.starts_with("refs/remotes/"))
            && self.snapshot.as_ref().is_some_and(|s| {
                s.current_branch
                    .as_ref()
                    .is_some_and(|b| reference != format!("refs/heads/{b}"))
                    && s.branches.iter().any(|b| b.name == reference)
            })
            && self
                .operation_state
                .as_ref()
                .is_some_and(|s| !s.in_progress() && s.conflicts.is_empty())
    }
    pub(super) fn merge_branch(
        &mut self,
        reference: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_merge_branch(&reference) {
            return;
        }
        self.merge_branch_in(self.repo.clone(), reference, window, cx);
    }
    pub(super) fn merge_branch_in(
        &mut self,
        repository: Repository,
        reference: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_repository_menu(cx);
        self.execute_branch_merge(repository, reference, None, window, cx);
    }
    fn execute_branch_merge(
        &mut self,
        repository: Repository,
        reference: String,
        smart: Option<Prepared>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.execute_branch_merge(repository, reference, smart, w, cx)
            });
            return;
        }
        let editor = match std::env::current_exe().and_then(|p| {
            plico::editor::command(
                &p.with_file_name(if cfg!(windows) {
                    "plico-editor.exe"
                } else {
                    "plico-editor"
                }),
                false,
            )
        }) {
            Ok(editor) => editor,
            Err(e) => {
                crate::error_ui::show_details(
                    e.to_string(),
                    "Git editor helper is unavailable",
                    window,
                    cx,
                );
                return;
            }
        };
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Merge,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.merged_branch = None;
        self.pending_branch_merge = None;
        self.status_message = Some(t("Merging branch…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let repo = repository.clone();
        let project_path = self.repo.path.clone();
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let save = smart.is_some();
            let result = smart
                .map(Ok)
                .unwrap_or_else(|| Prepared::read(&repo, &reference, &token))
                .and_then(|prepared| {
                    plico::branch_merge::execute(
                        &repo,
                        &prepared,
                        save,
                        &editor,
                        Some(&worker_requests),
                        &token,
                        &Default::default(),
                    )
                    .map(|report| (prepared, report))
                });
            let _ = tx.send(result);
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let directory = wx
                .background_spawn(
                    async move { directory_repo.editor_directory(&Cancel::default()) },
                )
                .await;
            let mut interaction = super::git_interaction::Session::new(
                id,
                this.clone(),
                requests,
                directory.as_ref().ok().cloned().into_iter().collect(),
            );
            loop {
                interaction.poll(wx).await;
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Merge worker ended".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = this.update_in(wx, |s, w, cx| {
                        let Some(cancelled) = interaction.finish(s, w, cx) else {
                            return;
                        };
                        if s.repo.path != project_path {
                            return;
                        }
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
                        match result {
                            Err(error) => {
                                s.status_message = Some(t("Merge requires attention").into());
                                crate::error_ui::show_details(
                                    error,
                                    "Merge requires attention",
                                    w,
                                    cx,
                                );
                            }
                            Ok((prepared, report)) => {
                                s.status_message = Some(
                                    t(if cancelled {
                                        "Operation cancelled; repository refreshed"
                                    } else {
                                        match report.status {
                                            Status::Success => "Branch merged",
                                            Status::AlreadyUpToDate => "Already up to date",
                                            Status::Conflicts => "Resolve conflicts to continue",
                                            _ => "Merge requires attention",
                                        }
                                    })
                                    .into(),
                                );
                                if let Some(oid) = &report.saved_oid {
                                    s.status_message = Some(format!(
                                        "{} — {}: {}",
                                        s.status_message.as_deref().unwrap_or_default(),
                                        t("Saved changes retained"),
                                        &oid[..8]
                                    ));
                                }
                                if !cancelled
                                    && matches!(
                                        report.status,
                                        Status::Success | Status::AlreadyUpToDate
                                    )
                                    && report.saved_oid.is_none()
                                    && prepared.may_propose_delete()
                                {
                                    s.merged_branch = Some(prepared.clone());
                                }
                                if report.status == Status::Conflicts {
                                    s.pending_branch_merge = Some(prepared.clone());
                                }
                                if !cancelled && report.status == Status::LocalChanges {
                                    let files = report
                                        .command
                                        .as_ref()
                                        .and_then(|r| r.as_ref().ok())
                                        .map(|o| {
                                            plico::checkout_files::parse(
                                                &o.stderr.bytes(),
                                                o.stderr.truncated,
                                                false,
                                            )
                                        })
                                        .unwrap_or_default();
                                    s.smart_merge_dialog(prepared, files, w, cx);
                                } else if !cancelled && report.status == Status::Conflicts {
                                    s.open_conflicts_in(repository.clone(), w, cx);
                                } else if !cancelled
                                    && !matches!(
                                        report.status,
                                        Status::Success | Status::AlreadyUpToDate
                                    )
                                {
                                    crate::error_ui::show_details(
                                        merge_details(&report),
                                        "Merge requires attention",
                                        w,
                                        cx,
                                    );
                                }
                                cx.notify();
                            }
                        }
                    });
                    break;
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(25))
                    .await;
            }
        });
    }
    fn smart_merge_dialog(
        &self,
        prepared: Prepared,
        files: plico::checkout_files::Files,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        let cancel_focus = cx.focus_handle();
        let initial = std::cell::Cell::new(true);
        window.open_dialog(cx, move |d,w,cx| {
            if initial.replace(false) {cancel_focus.focus(w,cx);}
            let cancel_focus=cancel_focus.clone();
            let button_owner=owner.clone();let button_path=path.clone();let button_prepared=prepared.clone();
            let owner=owner.clone();let path=path.clone();let prepared=prepared.clone();
            d.title(t("Git Merge Problem")).w(px(620.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t("Local changes would be overwritten. Smart Merge saves them, merges the branch, and restores them when the merge is complete.")))
                .child(div().id("merge-local-changes").h(px(220.)).overflow_y_scroll()
                    .children(files.paths.iter().map(|path|div().whitespace_normal().child(path.clone()))))
                .when(!files.complete,|d|d.child(t("The affected file list may be incomplete")))
                .footer(h_flex().w_full().justify_end().gap_2()
                    .child(Button::new("smart-merge").primary().label(t("Smart Merge")).on_click(move |_,w,cx| {
                        w.close_dialog(cx);let _=button_owner.update(cx,|s,cx|if s.repo.path==button_path {
                            s.execute_branch_merge(button_prepared.repository(),button_prepared.reference.clone(),Some(button_prepared.clone()),w,cx);
                        });
                    }))
                    .child(Button::new("cancel-smart-merge").focus_handle(cancel_focus.clone()).label(t("Don't Merge"))
                        .on_click(|_,w,cx|w.close_dialog(cx))))
                .on_ok(move |_,w,cx| {
                    if cancel_focus.is_focused(w) {return true;}
                    let _=owner.update(cx,|s,cx|if s.repo.path==path {s.execute_branch_merge(prepared.repository(),prepared.reference.clone(),Some(prepared.clone()),w,cx);});true
                })
        });
    }
}
fn merge_details(report: &Report) -> String {
    let mut details = report.error.clone().unwrap_or_default();
    if let Some(command) = &report.command {
        match command {
            Err(error) => {
                details.push('\n');
                details.push_str(error);
            }
            Ok(output) => {
                details.push('\n');
                details.push_str(&String::from_utf8_lossy(&output.stderr.bytes()));
                details.push('\n');
                details.push_str(&String::from_utf8_lossy(&output.stdout.bytes()));
            }
        }
    }
    if let Err(error) = &report.state {
        details.push('\n');
        details.push_str(error);
    }
    details.trim().to_owned()
}
