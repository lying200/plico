//! External merge tool setup, execution and explicit result acceptance.
use super::*;
use plico::{
    conflicts::{File, Snapshot},
    external_merge::{Prepared, Tool},
};
struct ToolDraft {
    program: Entity<InputState>,
    arguments: Entity<InputState>,
    error: Option<String>,
}
impl Render for ToolDraft {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(div().child(t("Program")))
            .child(Input::new(&self.program).w_full())
            .child(div().child(t("Arguments")))
            .child(Input::new(&self.arguments).w_full())
            .child(div().whitespace_normal().child(t(
                "%1: yours, %2: theirs, %3: base, %4: result. Quote arguments containing spaces.",
            )))
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    div()
                        .whitespace_normal()
                        .text_color(rgb(0xf2a6a6))
                        .child(error),
                )
            })
    }
}
impl Review {
    pub(super) fn configure_external_merge(
        &mut self,
        snapshot: Arc<Snapshot>,
        file: File,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (program, pattern) = plico::external_merge::load_configuration();
        let program = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(program, window, cx);
            s
        });
        let arguments = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(pattern, window, cx);
            s
        });
        let draft = cx.new(|_| ToolDraft {
            program,
            arguments,
            error: None,
        });
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |d, _, _| {
            let confirm = draft.clone();
            let owner = owner.clone();
            let path = path.clone();
            let snapshot = snapshot.clone();
            let file = file.clone();
            d.title(t("External Merge Tool"))
                .w(px(620.))
                .centered(true)
                .overlay_closable(false)
                .child(draft.clone())
                .footer(crate::dialog_style::confirmation(
                    Button::new("run-merge-tool").primary().label(t("Run")),
                ))
                .on_ok(move |_, w, cx| {
                    let tool =
                        confirm.update(
                            cx,
                            |s, cx| match plico::external_merge::save_configuration(
                                s.program.read(cx).value().as_ref(),
                                s.arguments.read(cx).value().as_ref(),
                            ) {
                                Ok(tool) => Some(tool),
                                Err(e) => {
                                    s.error = Some(e);
                                    cx.notify();
                                    None
                                }
                            },
                        );
                    let Some(tool) = tool else {
                        return false;
                    };
                    let snapshot = snapshot.clone();
                    let file = file.clone();
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path {
                            s.with_saved_documents(w, cx, move |s, w, cx| {
                                s.run_external_merge(snapshot, file, tool, w, cx)
                            });
                        }
                    });
                    true
                })
        });
    }
    fn run_external_merge(
        &mut self,
        snapshot: Arc<Snapshot>,
        file: File,
        tool: Tool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Waiting for external merge tool…").into());
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    let prepared = Prepared::read(&repo, &snapshot, &file, &token)?;
                    let output = prepared.run(&tool, &token, &Default::default());
                    let error = match output {
                        Err(error) => Some(error),
                        Ok(output) if !output.status.success() || output.stopped.is_some() =>
                            Some(format!("External tool did not complete successfully.\n{}",
                                String::from_utf8_lossy(&output.stderr.bytes()))),
                        Ok(_) => None,
                    };
                    if let Some(error) = error {
                        return Err(format!("{error}\n\n{}\n{}",
                            t("The external merge result has been kept at the following path. Copy it before removing the recovery directory."),
                            prepared.retain_output().display()));
                    }
                    Ok::<_, String>(Arc::new(prepared))
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                s.status_message = Some(t("External merge tool finished").into());
                match result {
                    Ok(prepared) if cancelled => crate::error_ui::show_details(
                        format!("{}\n{}",
                            t("The external merge result has been kept at the following path. Copy it before removing the recovery directory."),
                            prepared.retain_output().display()),
                        "Conflict resolution interrupted", w, cx),
                    Ok(prepared) => s.confirm_external_merge(prepared, w, cx),
                    Err(error) => crate::error_ui::show_details(
                        error,
                        "Could not run external merge tool",
                        w,
                        cx,
                    ),
                }
            });
        });
    }
    fn confirm_external_merge(
        &self,
        prepared: Arc<Prepared>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |d, _, _| {
            let prepared = prepared.clone(); let owner = owner.clone(); let path = path.clone();
            d.title(t("Merge in External Tool")).w(px(560.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t("Apply the external tool result and mark this file as resolved? Cancel keeps the working file unchanged.")))
                .footer(crate::dialog_style::confirmation(Button::new("apply-external-merge").primary().label(t("Mark Resolved"))))
                .on_ok(move |_, w, cx| {
                    let prepared = prepared.clone();
                    let _ = owner.update(cx, |s, cx| if s.repo.path == path {
                        s.with_saved_documents(w, cx, move |s, w, cx| s.apply_external_merge(prepared, w, cx));
                    }); true
                })
        });
    }
    fn apply_external_merge(
        &mut self,
        prepared: Arc<Prepared>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let worker_prepared = prepared.clone();
            let result = wx
                .background_spawn(async move {
                    let prepared = worker_prepared;
                    let result = (|| {
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
                        t("The external merge result has been kept at the following path. Copy it before removing the recovery directory."),
                        output.display());
                    crate::error_ui::show_details(detail, "Could not resolve conflict", w, cx);
                } else {
                    s.open_conflicts(w, cx);
                }
            });
        });
    }
}
