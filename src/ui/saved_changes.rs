//! Main-window saved changes coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn restore_stash(
        &mut self,
        entry: plico::stash::Entry,
        action: plico::stash_operation::Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.restore_stash(entry, action, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.settings_open || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Restore,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Restoring saved changes…").into());
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::stash_operation::execute(
                        &repo,
                        &entry,
                        action,
                        false,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = this.update(wx, |s, cx| {
                if s.finish_operation(operation, cx).is_none() {
                    return;
                }
                use plico::stash_operation::Status;
                let (message, diagnostic) = match result {
                    Ok(report) => {
                        let message = match report.status {
                            Status::Success if report.recovery_error.is_some() => {
                                "Saved changes restored; recovery record needs attention"
                            }
                            Status::Success => "Saved changes restored",
                            Status::Conflicts => {
                                "Saved changes have conflicts; review Local Changes"
                            }
                            Status::IndexConflict => {
                                "Could not restore the saved index; review Local Changes"
                            }
                            Status::LocalChanges => {
                                "Local changes would be overwritten; stash kept"
                            }
                            Status::UntrackedFiles => {
                                "Untracked files would be overwritten; stash kept"
                            }
                            Status::Cancelled => {
                                "Restore cancelled; check Local Changes before retrying"
                            }
                            Status::Failed => "Restore failed or incomplete; review Local Changes",
                        };
                        let diagnostic = if report.status == Status::Success {
                            report.recovery_error
                        } else {
                            Some(match report.command {
                                Ok(output) => format!(
                                    "{}\n{}",
                                    String::from_utf8_lossy(&output.stderr.bytes()),
                                    String::from_utf8_lossy(&output.stdout.bytes())
                                )
                                .chars()
                                .take(8000)
                                .collect(),
                                Err(error) => error,
                            })
                        };
                        s.operation_state = report.state.ok();
                        (message, diagnostic)
                    }
                    Err(error) => ("Could not restore saved changes", Some(error)),
                };
                s.status_message = Some(t(message).into());
                if let Some(view) = &s.stash_view {
                    view.update(cx, |v, cx| v.result(t(message).into(), diagnostic, cx));
                }
                s.refresh(false, cx);
                cx.notify();
            });
        });
        cx.notify();
    }
    pub(super) fn confirm_external_recovery(
        &mut self,
        pending: plico::squash_saved::Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
            return;
        }
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone(); let path = path.clone(); let pending = pending.clone();
            dialog.title(t("Finish Squash recovery")).w(px(510.)).centered(true).overlay_closable(false)
                .child(div().child(t("Confirm only after handling the saved changes externally. This clears the app recovery record; it does not restore or delete any stash or change your files.")))
                .child(div().truncate().child(path.display().to_string()))
                .footer(gpui_kit::component::dialog::DialogFooter::new()
                    .child(Button::new("acknowledge-recovery-confirm").min_w(px(72.)).label(t("Clear recovery record"))
                        .on_click(move |_, w, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                if s.repo.path == path && !s.operation.busy() { s.acknowledge_external_recovery(pending.clone(), cx); }
                            }); w.close_dialog(cx);
                        }))
                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.primary().min_w(px(72.)).label(t("Cancel")))))
                // The dialog-level Enter action is cancellation; only the explicit button clears a record.
                .on_ok(|_, _, _| true)
        });
    }
    pub(super) fn acknowledge_external_recovery(
        &mut self,
        pending: plico::squash_saved::Pending,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Clearing recovery record…").into());
        let repo = self.repo.clone();
        let path = repo.path.clone();
        self.operation_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    plico::squash_saved::acknowledge_external_recovery(&repo, &pending, &token)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.repo.path != path {
                    return;
                }
                if s.finish_operation(operation, cx).is_none() {
                    return;
                }
                let message = t(if result.is_ok() {
                    "Recovery record cleared; saved changes kept"
                } else {
                    "Recovery record was not cleared. Refresh and review the repository state."
                });
                s.status_message = Some(message.into());
                if let Some(view) = &s.stash_view {
                    view.update(cx, |v, cx| v.result(message.into(), None, cx));
                }
                s.rebase_diagnostic = result.err();
                cx.notify();
            });
        });
        cx.notify();
    }
}
