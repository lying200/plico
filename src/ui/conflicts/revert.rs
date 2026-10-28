//! Reset iterative decisions to the common base without resolving the index.
use super::*;

impl Review {
    pub(super) fn revert_conflict_resolution(
        &mut self,
        view: Entity<ConflictList>,
        path: Key,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_conflict_session(&view, cx) || view.read(cx).busy {
            return;
        }
        if !view
            .read(cx)
            .drafts
            .get(&path)
            .is_some_and(|d| d.decisions.progress().0 > 0)
        {
            return;
        }
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |d, _, _| {
            let owner = owner.clone();
            let view = view.clone();
            let path = path.clone();
            d.title(t("Confirm Revert")).w(px(480.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t("Discard this file's conflict resolutions and restore the base version? The file will remain unresolved.")))
                .footer(crate::dialog_style::confirmation(Button::new("revert-conflict-resolution").primary().label(t("Revert"))))
                .on_ok(move |_, w, cx| {
                    w.close_dialog(cx);
                    let view = view.clone();
                    let path = path.clone();
                    let _ = owner.update(cx, |s, cx| s.with_conflict_documents(w, cx, move |s, w, cx| s.revert_conflict_resolution_confirmed(view, path, w, cx)));
                    false
                })
        });
    }

    pub(super) fn revert_conflict_resolution_confirmed(
        &mut self,
        view: Entity<ConflictList>,
        path: Key,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_conflict_session(&view, cx) || view.read(cx).busy {
            return;
        }
        let Some(draft) = view
            .read(cx)
            .drafts
            .get(&path)
            .cloned()
            .filter(|d| d.decisions.progress().0 > 0)
        else {
            return;
        };
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
        let repository = draft.prepared.snapshot().repository();
        let access = id.access.clone();
        self.conflict_task = cx.spawn_in(window, async move |this, wx| {
            let repo = repository.clone();
            let result = wx
                .background_spawn(async move {
                    let result = (|| {
                        let [_, base, _] = draft.prepared.text_inputs()?;
                        draft.prepared.write_text_result(base)?;
                        access.apply(&draft.prepared, &repo, &token)
                    })();
                    if result.is_err() {
                        draft.prepared.retain_output();
                    }
                    result
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_conflict_operation(id, cx).is_none()
                    || !s.owns_conflict_session(&view, cx)
                {
                    return;
                }
                view.update(cx, |state, cx| {
                    state.busy = false;
                    match result {
                        Ok(()) => {
                            state.automatic.invalidate(&path);
                            state.drafts.remove(&path);
                        }
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
}
