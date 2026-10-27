//! Guarded write-back and staging for the native merge editor.
use super::*;
use plico::merge_result::Prepared;
type MergeFinished = Box<dyn FnOnce(bool, &mut Window, &mut App)>;
impl Review {
    pub(super) fn apply_merge_result(
        &mut self,
        prepared: Arc<Prepared>,
        text: String,
        decisions: plico::text_merge::State,
        finished: MergeFinished,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) =
            self.begin_conflict_operation(plico::active_operation::Phase::Executing, cx)
        else {
            finished(false, window, cx);
            return;
        };
        let session = self.conflict_result_session(&prepared, cx);
        let iterative = session.is_some();
        let repo = prepared.snapshot().repository();
        let repository = repo.clone();
        let project_path = self.repo.path.clone();
        let access = id.access.clone();
        self.conflict_task = cx.spawn_in(window, async move |this, wx| {
            let worker_prepared = prepared.clone();
            let result = wx
                .background_spawn(async move {
                    let prepared = worker_prepared;
                    let result: Result<(), String> = (|| {
                        prepared.write_text_result(text)?;
                        access.apply(&prepared, &repo, &token)?;
                        if iterative { return Ok(()); }
                        access.stage(&prepared, &repo, &token)?;
                        Ok(())
                    })();
                    // Preserve before returning to the UI: the window may have
                    // closed while Git was running, so no callback is guaranteed.
                    if result.is_err() || token.cancelled() { prepared.retain_output(); }
                    result
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_conflict_operation(id, cx) else {
                    return;
                };
                if s.repo.path != project_path { finished(false, w, cx); return; }
                let error = result.err();
                if error.is_none() {
                    if let Some(session) = &session { s.keep_conflict_resolution(session, prepared.clone(), decisions, cx); }
                }
                finished(error.is_none() && !cancelled, w, cx);
                s.status_message = Some(
                    t(if error.is_none() && !cancelled {
                        if iterative { "Merge result saved" } else { "Conflict resolved" }
                    } else {
                        "Conflict resolution interrupted"
                    })
                    .into(),
                );
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
                if error.is_some() || cancelled {
                    let output = prepared.retain_output();
                    let detail = format!("{}\n\n{}\n{}",
                        error.unwrap_or_else(|| t("Conflict resolution interrupted").into()),
                        t("The merge result has been kept at the following path. Copy it before removing the recovery directory."),
                        output.display());
                    crate::error_ui::show_details(detail, "Could not resolve conflict", w, cx);
                } else {
                    s.reopen_conflicts_in(repository, w, cx);
                }
            });
        });
    }
}
