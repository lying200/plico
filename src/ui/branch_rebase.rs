//! Rebased GitRebaseBranchAction: rebase the current branch onto a selection.
use super::*;
use plico::branch_rebase::Prepared;

impl Review {
    pub(super) fn branch_rebase_item(
        owner: &WeakEntity<Self>,
        reference: &str,
        cx: &App,
    ) -> PopupMenuItem {
        let current = owner
            .upgrade()
            .and_then(|s| {
                s.read(cx)
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.current_branch.clone())
            })
            .unwrap_or_else(|| "HEAD".into());
        let target = reference
            .strip_prefix("refs/heads/")
            .or_else(|| reference.strip_prefix("refs/remotes/"))
            .unwrap_or(reference);
        let label = t("Rebase {current} onto {target}")
            .replace("{current}", &current)
            .replace("{target}", target);
        let enabled = owner
            .upgrade()
            .is_some_and(|s| s.read(cx).can_rebase_branch(reference));
        let owner = owner.clone();
        let reference = reference.to_owned();
        PopupMenuItem::new(label)
            .disabled(!enabled)
            .on_click(move |_, w, cx| {
                let _ = owner.update(cx, |s, cx| s.rebase_branch(reference.clone(), w, cx));
            })
    }

    pub(super) fn can_rebase_branch(&self, reference: &str) -> bool {
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
    pub(super) fn rebase_branch(
        &mut self,
        reference: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_rebase_branch(&reference) {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(expected_head) = snapshot.head_id.clone() else {
            return;
        };
        let Some(expected_target) = snapshot
            .branches
            .iter()
            .find(|b| b.name == reference)
            .map(|b| b.id.clone())
        else {
            return;
        };
        self.dismiss_repository_menu(cx);
        self.execute_branch_rebase(reference, expected_head, expected_target, window, cx);
    }
    fn execute_branch_rebase(
        &mut self,
        reference: String,
        expected_head: String,
        expected_target: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.execute_branch_rebase(reference, expected_head, expected_target, w, cx)
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
            plico::active_operation::Kind::Rebase,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Rebasing branch…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = Prepared::read(&repo, &reference, &token).and_then(|prepared| {
                if !prepared.matches_displayed(&expected_head, &expected_target) {
                    return Err("Branches changed; review the rebase again".into());
                }
                plico::branch_rebase::execute(
                    &repo,
                    &prepared,
                    &editor,
                    Some(&worker_requests),
                    &token,
                    &Default::default(),
                )
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
                        Some(Err("Rebase worker ended".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = this.update_in(wx, |s, w, cx| {
                        let Some(cancelled) = interaction.finish(s, w, cx) else {
                            return;
                        };
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
                        match result {
                            Err(error) => {
                                s.status_message = Some(t("Rebase requires attention").into());
                                crate::error_ui::show_details(
                                    error,
                                    "Rebase requires attention",
                                    w,
                                    cx,
                                );
                            }
                            Ok(report) => {
                                let conflicts =
                                    report.state.as_ref().is_ok_and(|s| !s.conflicts.is_empty());
                                let success = report.succeeded();
                                s.status_message = Some(
                                    t(if cancelled {
                                        "Operation cancelled; repository refreshed"
                                    } else if conflicts {
                                        "Resolve conflicts to continue"
                                    } else if success {
                                        "Branch rebased"
                                    } else {
                                        "Rebase requires attention"
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
                                if !cancelled && conflicts {
                                    s.open_conflicts(w, cx);
                                } else if !cancelled && !success {
                                    crate::error_ui::show_details(
                                        report.diagnostic(),
                                        "Rebase requires attention",
                                        w,
                                        cx,
                                    );
                                }
                            }
                        }
                        cx.notify();
                    });
                    break;
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(25))
                    .await;
            }
        });
    }
}
