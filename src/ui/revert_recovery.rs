//! Inspect and explicitly dismiss an externally ended Revert queue.
use super::*;
use plico::revert_operation::Pending;
impl Review {
    pub(super) fn open_revert_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                    Ok(Some(pending)) => s.revert_queue_dialog(pending, w, cx),
                    Ok(None) => {
                        s.revert_recovery_available = false;
                        s.revert_recoveries.remove(&s.repo.path);
                        cx.notify();
                    }
                    Err(error) => {
                        crate::error_ui::show_details(error, "Operation requires attention", w, cx)
                    }
                }
            });
        });
    }
    fn revert_queue_dialog(&self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |d, _, _| {
            let owner = owner.clone(); let path = path.clone(); let pending = pending.clone();
            let rows = pending.targets().iter().map(|target| div().child(target.clone())).collect::<Vec<_>>();
            d.title(t("Saved Revert Queue")).w(px(600.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t("These commits were pending when Revert stopped. If you finished or aborted outside Plico, clear the queue after reviewing the Log. This does not revert commits or change files.")))
                .child(div().id("saved-revert-targets").max_h(px(220.)).overflow_y_scroll().children(rows))
                .when(!pending.can_clear, |d| d.child(div().whitespace_normal().child(t("Finish or abort the current Git operation before clearing this queue."))))
                .footer(gpui_kit::component::dialog::DialogFooter::new()
                    .child(Button::new("clear-revert-queue").label(t("Clear saved queue")).disabled(!pending.can_clear)
                        .on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = owner.update(cx, |s, cx| if s.repo.path == path { s.clear_revert_queue(pending.clone(), w, cx); });
                        }))
                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.primary().label(t("Cancel")))))
                .on_ok(|_, _, _| true)
        });
    }
    fn clear_revert_queue(
        &mut self,
        pending: Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
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
                .background_spawn(async move { pending.clear(&repo, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_operation(id, cx).is_none() {
                    return;
                }
                match result {
                    Ok(()) => {
                        s.revert_recoveries.remove(&s.repo.path);
                        s.status_message =
                            Some(t("Saved queue cleared; commits and files unchanged").into());
                    }
                    Err(error) => {
                        crate::error_ui::show_details(error, "Operation requires attention", w, cx)
                    }
                }
                s.refresh_impl(false, true, None, cx);
            });
        });
    }
}
