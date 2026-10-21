//! Recovery for cherry-pick/revert already in progress.
use super::*;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use plico::operation_state::Operation;
use plico::sequencer_operation::{EmptyChoice, EmptyCommit};
#[derive(Clone)]
enum Request {
    Continue,
    Abort,
    Empty(EmptyCommit, EmptyChoice),
}

impl Review {
    pub(super) fn confirm_abort_sequence(
        &self,
        kind: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |d, _, _| {
            let owner = owner.clone(); let path = path.clone();
            d.title(t(if kind == Operation::CherryPick { "Abort Cherry-pick" } else { "Abort Revert" }))
                .w(px(500.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t("Abort the operation and return to its starting state? Conflict resolutions may be lost.")))
                .footer(crate::dialog_style::confirmation(Button::new("abort-sequence").primary().label(t("Abort"))))
                .on_ok(move |_, w, cx| {
                    let _ = owner.update(cx, |s, cx| if s.repo.path == path { s.start_sequence(kind, true, w, cx); }); true
                })
        });
    }
    pub(super) fn start_sequence(
        &mut self,
        kind: Operation,
        abort: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_sequence(
            kind,
            if abort {
                Request::Abort
            } else {
                Request::Continue
            },
            window,
            cx,
        );
    }
    fn run_sequence(
        &mut self,
        kind: Operation,
        request: Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let abort = matches!(request, Request::Abort);
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.run_sequence(kind, request, w, cx)
            });
            return;
        }
        let editor_command = if !matches!(request, Request::Continue) {
            None
        } else {
            match std::env::current_exe().and_then(|exe| {
                plico::editor::command(
                    &exe.with_file_name(if cfg!(windows) {
                        "plico-editor.exe"
                    } else {
                        "plico-editor"
                    }),
                    false,
                )
            }) {
                Ok(command) => Some(command),
                Err(error) => {
                    crate::error_ui::show_details(
                        error.to_string(),
                        "Git editor helper is unavailable",
                        window,
                        cx,
                    );
                    return;
                }
            }
        };
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Updating operation state…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            use plico::sequencer_operation::{self, Action};
            let action = match &request {
                Request::Continue => Action::Continue {
                    editor: editor_command.as_ref().unwrap(),
                    requests: Some(&worker_requests),
                },
                Request::Abort => Action::Abort,
                Request::Empty(expected, choice) => Action::Empty {
                    expected,
                    choice: *choice,
                },
            };
            let _ = tx.send(sequencer_operation::execute(
                &repo,
                kind,
                action,
                &token,
                &Default::default(),
            ));
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let directory = wx
                .background_spawn(
                    async move { directory_repo.editor_directory(&Cancel::default()) },
                )
                .await;
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            loop {
                if let Some(pending) = requests.take_pending() {
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
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Operation worker ended".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = this.update_in(wx, |s, w, cx| {
                        if editor.as_ref().is_some_and(|e| e.read(cx).active()) {
                            w.close_dialog(cx);
                        }
                        let Some(cancelled) = s.finish_operation(id, cx) else {
                            return;
                        };
                        let empty = result.as_ref().ok().and_then(|r| r.empty.clone());
                        let (error, complete, conflicts) = match result {
                            Err(error) => (Some(error), false, false),
                            Ok(report) => {
                                let error = match report.command {
                                    Err(error) => Some(error),
                                    Ok(o) if !o.status.success() || o.stopped.is_some() => Some(
                                        String::from_utf8_lossy(&o.stderr.bytes()).into_owned(),
                                    ),
                                    _ => None,
                                };
                                match report.state {
                                    Ok(state) => {
                                        (error, !state.in_progress(), !state.conflicts.is_empty())
                                    }
                                    Err(e) => (Some(e), false, false),
                                }
                            }
                        };
                        s.status_message = Some(
                            t(if cancelled {
                                "Operation cancelled; repository refreshed"
                            } else if conflicts {
                                "Resolve conflicts to continue"
                            } else if complete && error.is_none() {
                                if abort {
                                    "Operation aborted"
                                } else {
                                    "Operation completed"
                                }
                            } else {
                                "Operation requires attention"
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
                        if let Some(empty) = empty.filter(|_| !cancelled) {
                            s.choose_empty_commit(empty, w, cx);
                        } else if let Some(error) = error {
                            crate::error_ui::show_details(
                                error,
                                "Operation requires attention",
                                w,
                                cx,
                            );
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
    fn choose_empty_commit(&self, empty: EmptyCommit, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |d, _, _| {
            let mut footer = DialogFooter::new();
            for (id, label, choice) in [
                ("skip-empty", "Skip Commit", EmptyChoice::Skip),
                ("keep-empty", "Keep Empty Commit", EmptyChoice::Keep),
            ] {
                let owner = owner.clone();
                let path = path.clone();
                let empty = empty.clone();
                footer = footer.child(Button::new(id).label(t(label)).on_click(move |_, w, cx| {
                    w.close_dialog(cx);
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path {
                            s.run_sequence(
                                Operation::CherryPick,
                                Request::Empty(empty.clone(), choice),
                                w,
                                cx,
                            );
                        }
                    });
                }));
            }
            d.title(t("Empty Cherry-pick"))
                .w(px(560.))
                .centered(true)
                .overlay_closable(false)
                .child(div().whitespace_normal().child(t(
                    "The resolved commit has no staged changes. Skip it or keep an empty commit?",
                )))
                .footer(
                    footer.child(DialogClose::new().trigger(|button| button.label(t("Cancel")))),
                )
        });
    }
}
