//! Guarded write-back and staging for the native merge editor.
use super::*;
use plico::merge_result::Prepared;
type MergeFinished = Box<dyn FnOnce(bool, &mut Window, &mut App)>;
impl Review {
    pub(super) fn apply_merge_result(
        &mut self,
        prepared: Arc<Prepared>,
        text: String,
        finished: MergeFinished,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            finished(false, window, cx);
            return;
        };
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let worker_prepared = prepared.clone();
            let result = wx
                .background_spawn(async move {
                    let prepared = worker_prepared;
                    let result = (|| {
                        prepared.write_text_result(text)?;
                        prepared.apply(&repo, &token)?;
                        let report = plico::conflicts::resolve(
                            &repo, prepared.snapshot(), prepared.file(),
                            plico::conflicts::Resolution::MarkResolved, &token, &Default::default(),
                        )?;
                        for command in report.commands {
                            let output = command?;
                            if !output.status.success() || output.stopped.is_some() || output.input_error.is_some() {
                                return Err(String::from_utf8_lossy(&output.stderr.bytes()).into_owned());
                            }
                        }
                        let snapshot = report.snapshot?;
                        if snapshot.files.iter().any(|file| file.path == prepared.file().path) {
                            return Err(t("The file is still unresolved").into());
                        }
                        Ok(())
                    })();
                    // Preserve before returning to the UI: the window may have
                    // closed while Git was running, so no callback is guaranteed.
                    if result.is_err() || token.cancelled() { prepared.retain_output(); }
                    result
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                let error = result.err();
                finished(error.is_none() && !cancelled, w, cx);
                s.status_message = Some(
                    t(if error.is_none() && !cancelled {
                        "Conflict resolved"
                    } else {
                        "Conflict resolution interrupted"
                    })
                    .into(),
                );
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
                if error.is_some() || cancelled {
                    let output = prepared.retain_output();
                    let detail = format!("{}\n\n{}\n{}",
                        error.unwrap_or_else(|| t("Conflict resolution interrupted").into()),
                        t("The merge result has been kept at the following path. Copy it before removing the recovery directory."),
                        output.display());
                    crate::error_ui::show_details(detail, "Could not resolve conflict", w, cx);
                } else {
                    s.open_conflicts(w, cx);
                }
            });
        });
    }
}
