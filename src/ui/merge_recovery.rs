//! Explicit recovery actions for retained Smart Merge changes.
use super::*;
use plico::merge_saved::Pending;
impl Review {
    pub(super) fn open_merge_recovery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() {
            return;
        }
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move { Pending::read(&repo, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if cancelled {
                    return;
                }
                match result {
                    Ok(Some(pending)) => s.merge_recovery_dialog(pending, w, cx),
                    Ok(None) => {
                        s.merge_recovery_available = false;
                        cx.notify();
                    }
                    Err(error) => {
                        crate::error_ui::show_details(error, "Merge requires attention", w, cx)
                    }
                }
            });
        });
    }
    fn merge_recovery_dialog(&self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx,move |d,_,_| {
            let retry_owner=owner.clone();let retry_path=path.clone();let retry_pending=pending.clone();
            let clear_owner=owner.clone();let clear_path=path.clone();let clear_pending=pending.clone();
            let show_owner=owner.clone();let show_path=path.clone();
            d.title(t("Saved Merge Changes")).w(px(580.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(format!("{}: {}",t("Branch"),pending.record.branch.strip_prefix("refs/heads/").unwrap_or(&pending.record.branch))))
                .child(div().whitespace_normal().child(format!("Stash: {}",pending.record.stash)))
                .child(div().whitespace_normal().child(t("Review the retained stash before retrying. Retry requires a clean tracked working tree and the original merge result. Clearing the record does not restore or delete the stash.")))
                .footer(gpui_kit::component::dialog::DialogFooter::new().flex_wrap().gap_2()
                    .child(Button::new("show-merge-stash").label(t("Show Stashes")).on_click(move |_,w,cx| {
                        w.close_dialog(cx);let _=show_owner.update(cx,|s,cx|if s.repo.path==show_path {s.open_stashes(w,cx);});
                    }))
                    .child(Button::new("retry-merge-restore").label(t("Retry Restore")).disabled(!pending.can_restore).on_click(move |_,w,cx| {
                        w.close_dialog(cx);let _=retry_owner.update(cx,|s,cx|if s.repo.path==retry_path {s.run_merge_recovery(retry_pending.clone(),false,w,cx);});
                    }))
                    .child(Button::new("clear-merge-recovery").label(t("Clear recovery record")).on_click(move |_,w,cx| {
                        w.close_dialog(cx);let _=clear_owner.update(cx,|s,cx|if s.repo.path==clear_path {s.confirm_clear_merge_recovery(clear_pending.clone(),w,cx);});
                    }))
                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b|b.label(t("Close")))))
                .on_ok(|_,_,_|true)
        });
    }
    fn confirm_clear_merge_recovery(
        &self,
        pending: Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx,move |d,_,_| {
            let owner=owner.clone();let path=path.clone();let pending=pending.clone();
            d.title(t("Clear recovery record")).w(px(530.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t("Confirm only after handling the saved changes externally. This clears the app recovery record; it does not restore or delete any stash or change your files.")))
                .footer(gpui_kit::component::dialog::DialogFooter::new()
                    .child(Button::new("confirm-clear-merge-record").label(t("Clear recovery record")).on_click(move |_,w,cx| {
                        w.close_dialog(cx);let _=owner.update(cx,|s,cx|if s.repo.path==path {s.run_merge_recovery(pending.clone(),true,w,cx);});
                    }))
                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b|b.primary().label(t("Cancel")))))
                .on_ok(|_,_,_|true)
        });
    }
    fn run_merge_recovery(
        &mut self,
        pending: Pending,
        clear: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if !clear && self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.run_merge_recovery(pending, clear, w, cx)
            });
            return;
        }
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    if clear {
                        pending.acknowledge(&repo, &token)
                    } else {
                        pending.retry(&repo, &token, &Default::default())
                    }
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_operation(id, cx).is_none() {
                    return;
                }
                match result {
                    Ok(()) => {
                        s.status_message = Some(
                            t(if clear {
                                "Recovery record cleared; saved changes kept"
                            } else {
                                "Saved changes restored"
                            })
                            .into(),
                        )
                    }
                    Err(error) => {
                        s.status_message = Some(t("Merge requires attention").into());
                        crate::error_ui::show_details(error, "Merge requires attention", w, cx);
                    }
                }
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
            });
        });
    }
}
