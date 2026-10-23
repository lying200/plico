//! Start and recover cherry-pick/revert through one editor and refresh lifecycle.
use super::*;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use plico::operation_state::Operation;
use plico::sequencer_operation::{EmptyChoice, EmptyCommit};
#[derive(Clone)]
enum Request {
    Continue,
    Revert(Vec<String>),
    CherryPick(Vec<String>),
    SaveCherryPick(plico::cherry_pick::Prepared),
    SaveRevert(plico::revert_operation::Batch),
    ResumeRevert(plico::revert_operation::Recovery),
    Abort,
    Empty(EmptyCommit, EmptyChoice),
}

impl Review {
    pub(super) fn cherry_pick_commits(
        &mut self,
        targets: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_sequence(
            Operation::CherryPick,
            Request::CherryPick(targets),
            window,
            cx,
        );
    }

    pub(super) fn revert_commits(
        &mut self,
        target: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_sequence(Operation::Revert, Request::Revert(target), window, cx);
    }

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
            } else if kind == Operation::Revert {
                self.revert_recovery
                    .clone()
                    .map(Request::ResumeRevert)
                    .unwrap_or(Request::Continue)
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
        self.run_sequence_with_editor(kind, request, None, window, cx);
    }
    fn run_sequence_with_editor(
        &mut self,
        kind: Operation,
        request: Request,
        editor_override: Option<std::ffi::OsString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let abort = matches!(request, Request::Abort);
        // Reopened queues enter via Continue, not ResumeRevert. Their next
        // conflict needs the same resolver as a freshly started batch.
        let resolving_changes = !abort;
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.run_sequence_with_editor(kind, request, editor_override, w, cx)
            });
            return;
        }
        let editor_command = if !matches!(
            request,
            Request::Continue
                | Request::CherryPick(_)
                | Request::SaveCherryPick(_)
                | Request::Revert(_)
                | Request::SaveRevert(_)
                | Request::ResumeRevert(_)
        ) {
            None
        } else if editor_override.is_some() {
            editor_override
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
        self.status_message = Some(
            t(if matches!(request, Request::CherryPick(_)) {
                "Cherry-picking commits…"
            } else {
                "Updating operation state…"
            })
            .into(),
        );
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            use plico::sequencer_operation::{self, Action};
            let request = if kind == Operation::Revert && matches!(&request, Request::Continue) {
                match plico::revert_operation::Recovery::read(&repo, &token) {
                    Ok(Some(recovery)) => Request::ResumeRevert(recovery),
                    Ok(None) => request,
                    Err(error) => {
                        let _ = tx.send(Err(error));
                        return;
                    }
                }
            } else {
                request
            };
            if let Request::SaveCherryPick(prepared) = &request {
                let result = prepared
                    .save_and_execute(
                        &repo,
                        editor_command.as_ref().unwrap(),
                        Some(&worker_requests),
                        &token,
                        &Default::default(),
                    )
                    .map(|saved| {
                        let mut completion = match saved.report {
                            Some(report) => SequenceCompletion::cherry(report),
                            None => SequenceCompletion::ordinary(
                                plico::sequencer_operation::ResultState {
                                    command: Err("Cherry-pick did not start".into()),
                                    state: repo.operation_state(&Cancel::default()),
                                    empty: None,
                                },
                            ),
                        };
                        if let Some(error) = saved.error {
                            completion.report.command = Err(error);
                        }
                        completion.cherry_retry = None;
                        completion.saved_cherry = saved.saved;
                        completion
                    });
                let _ = tx.send(result);
                return;
            }
            if let Request::CherryPick(targets) = &request {
                let result = (|| {
                    let protected = repo
                        .pull_targets(&token)?
                        .branches
                        .into_iter()
                        .filter(plico::published_commits::default_protected)
                        .map(|target| format!("refs/remotes/{}/{}", target.remote, target.branch))
                        .collect::<Vec<_>>();
                    let prepared =
                        plico::cherry_pick::Prepared::read(&repo, targets, &protected, &token)?;
                    let result = plico::cherry_pick::execute(
                        &repo,
                        &prepared,
                        editor_command.as_ref().unwrap(),
                        Some(&worker_requests),
                        &token,
                        &Default::default(),
                    )?;
                    Ok(SequenceCompletion::cherry(result))
                })();
                let _ = tx.send(result);
                return;
            }
            if let Request::SaveRevert(batch) = &request {
                let result = batch
                    .save_and_execute(
                        &repo,
                        editor_command.as_ref().unwrap(),
                        Some(&worker_requests),
                        &token,
                        &Default::default(),
                    )
                    .map(|saved| saved_completion(&repo, saved));
                let _ = tx.send(result);
                return;
            }
            let batch = match &request {
                Request::Revert(targets) => Some(
                    plico::revert_operation::Batch::read(&repo, targets, &token).and_then(
                        |batch| {
                            batch.execute(
                                &repo,
                                editor_command.as_ref().unwrap(),
                                Some(&worker_requests),
                                &token,
                                &Default::default(),
                            )
                        },
                    ),
                ),
                Request::ResumeRevert(recovery) => Some(recovery.resume(
                    &repo,
                    editor_command.as_ref().unwrap(),
                    Some(&worker_requests),
                    &token,
                    &Default::default(),
                )),
                _ => None,
            };
            if let Some(result) = batch {
                let _ = tx.send(result.map(|batch| batch_completion(&repo, batch)));
                return;
            }
            let action = match &request {
                Request::Continue => Action::Continue {
                    editor: editor_command.as_ref().unwrap(),
                    requests: Some(&worker_requests),
                },
                Request::Abort => Action::Abort,
                Request::CherryPick(_)
                | Request::SaveCherryPick(_)
                | Request::Revert(_)
                | Request::SaveRevert(_)
                | Request::ResumeRevert(_) => {
                    unreachable!()
                }
                Request::Empty(expected, choice) => Action::Empty {
                    expected,
                    choice: *choice,
                },
            };
            let _ = tx.send(
                sequencer_operation::execute(&repo, kind, action, &token, &Default::default())
                    .map(SequenceCompletion::ordinary),
            );
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
                        let empty = result.as_ref().ok().and_then(|r| r.report.empty.clone());
                        let nothing = result.as_ref().is_ok_and(|r| r.nothing);
                        let ignore_empty_exit = result.as_ref().is_ok_and(|r| r.ignore_empty_exit);
                        let summary = result.as_ref().ok().and_then(|r| r.summary.clone());
                        let retry = result.as_ref().ok().and_then(|r| r.retry.clone());
                        let cherry_retry =
                            result.as_ref().ok().and_then(|r| r.cherry_retry.clone());
                        let saved_cherry =
                            result.as_ref().ok().and_then(|r| r.saved_cherry.clone());
                        if kind == Operation::Revert {
                            if let Ok(completion) = &result {
                                s.revert_recovery = completion.recovery.clone();
                            }
                        }
                        let (error, complete, conflicts) = match result {
                            Err(error) => (Some(error), false, false),
                            Ok(completion) => {
                                let report = completion.report;
                                let error = match report.command {
                                    Err(error) => Some(error),
                                    Ok(o) if o.input_error.is_some() => {
                                        o.input_error.map(|e| e.to_string())
                                    }
                                    Ok(o)
                                        if (!o.status.success() && !ignore_empty_exit)
                                            || o.stopped.is_some() =>
                                    {
                                        Some(
                                            format!(
                                                "{}\n{}",
                                                String::from_utf8_lossy(&o.stderr.bytes()),
                                                String::from_utf8_lossy(&o.stdout.bytes())
                                            )
                                            .trim()
                                            .to_owned(),
                                        )
                                    }
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
                            } else if nothing {
                                "Nothing to revert"
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
                        if let Some(summary) = summary {
                            s.status_message = Some(format!(
                                "{} — {}",
                                s.status_message.as_deref().unwrap_or_default(),
                                summary
                            ));
                        }
                        if let Some(stash) = saved_cherry {
                            s.status_message = Some(format!(
                                "{} — {}: {}",
                                s.status_message.as_deref().unwrap_or_default(),
                                t("Saved changes retained"),
                                stash
                            ));
                            s.open_stashes(w, cx);
                        }
                        if abort && complete && error.is_none() {
                            s.revert_recovery = None;
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
                        if let Some(empty) = empty.filter(|_| !cancelled) {
                            s.choose_empty_commit(empty, w, cx);
                        } else if resolving_changes && conflicts && !cancelled {
                            s.open_conflicts(w, cx);
                        } else if let Some(retry) = cherry_retry.filter(|_| !cancelled) {
                            s.cherry_retry_dialog(retry, w, cx);
                        } else if let Some(retry) = retry.filter(|_| !cancelled) {
                            s.confirm_save_revert(retry, w, cx);
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

fn batch_completion(
    repo: &plico::git::Repository,
    batch: plico::revert_operation::BatchReport,
) -> SequenceCompletion {
    let nothing =
        batch.successful.is_empty() && batch.remaining.is_empty() && !batch.skipped.is_empty();
    let summary = format!(
        "{}: {}, {}: {}, {}: {}",
        t("Reverted"),
        batch.successful.len(),
        t("Skipped"),
        batch.skipped.len(),
        t("Remaining"),
        batch.remaining.len()
    );
    let (command, state) = if let Some(report) = batch.last {
        let command = if let Some(error) = batch.error {
            Err(error)
        } else if let Err(error) = report.head {
            Err(error)
        } else if report.status == plico::revert_operation::Status::NothingToRevert {
            // Last item can be empty even when earlier items succeeded.
            // The UI suppresses this Git nonzero outcome via the second flag.
            report.command
        } else if report.status == plico::revert_operation::Status::Failed
            && report.command.as_ref().is_ok_and(|o| o.status.success())
        {
            Err("Revert did not reach the expected repository state".into())
        } else {
            report.command
        };
        (command, report.state)
    } else {
        (
            Err(batch
                .error
                .unwrap_or_else(|| "No revert result available".into())),
            repo.operation_state(&Cancel::default()),
        )
    };
    // An empty final command is expected for a mixed successful/skipped batch too.
    let ignore_empty_exit = batch.remaining.is_empty() && !batch.skipped.is_empty();
    SequenceCompletion {
        report: plico::sequencer_operation::ResultState {
            command,
            state,
            empty: None,
        },
        nothing,
        recovery: batch.recovery,
        summary: Some(summary),
        ignore_empty_exit,
        retry: batch.retry,
        cherry_retry: None,
        saved_cherry: None,
    }
}

struct SequenceCompletion {
    report: plico::sequencer_operation::ResultState,
    nothing: bool,
    recovery: Option<plico::revert_operation::Recovery>,
    summary: Option<String>,
    ignore_empty_exit: bool,
    retry: Option<plico::revert_operation::Batch>,
    cherry_retry: Option<plico::cherry_pick::Prepared>,
    saved_cherry: Option<String>,
}
impl SequenceCompletion {
    fn cherry(report: plico::cherry_pick::Report) -> Self {
        let mut completion = Self::ordinary(plico::sequencer_operation::ResultState {
            command: report.command,
            state: report.state,
            empty: report.empty,
        });
        completion.cherry_retry = report.retry;
        completion
    }

    fn ordinary(report: plico::sequencer_operation::ResultState) -> Self {
        Self {
            report,
            nothing: false,
            recovery: None,
            summary: None,
            ignore_empty_exit: false,
            retry: None,
            cherry_retry: None,
            saved_cherry: None,
        }
    }
}
fn saved_completion(
    repo: &plico::git::Repository,
    saved: plico::revert_operation::SavedReport,
) -> SequenceCompletion {
    let mut completion = match saved.batch {
        Some(batch) => batch_completion(repo, batch),
        None => SequenceCompletion::ordinary(plico::sequencer_operation::ResultState {
            command: Err("Revert did not start".into()),
            state: repo.operation_state(&Cancel::default()),
            empty: None,
        }),
    };
    // Restoration can introduce conflicts after the final Revert command.
    completion.report.state = repo.operation_state(&Cancel::default());
    if let Some(error) = saved.error {
        completion.report.command = Err(error);
    }
    if let Some(record) = saved.saved {
        completion.retry = None;
        completion.summary = Some(format!(
            "{} — {}: {}",
            completion.summary.unwrap_or_default(),
            t("Saved changes retained"),
            record.stash
        ));
    }
    completion
}

impl Review {
    fn confirm_save_revert(
        &self,
        retry: plico::revert_operation::Batch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.revert_retry_dialog(retry, None, window, cx);
    }
    fn show_revert_local_files(
        &mut self,
        retry: plico::revert_operation::Batch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                .background_spawn(async move {
                    repo.local_changes(&token).map(|changes| {
                        changes
                            .into_iter()
                            .filter(|c| c.area != plico::git::Area::Untracked)
                            .map(|c| c.path.to_string_lossy().into_owned())
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>()
                    })
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if cancelled {
                    return;
                }
                match result {
                    Ok(files) => s.revert_retry_dialog(retry, Some(files), w, cx),
                    Err(error) => {
                        crate::error_ui::show_details(error, "Operation requires attention", w, cx)
                    }
                }
            });
        });
    }
    fn revert_retry_dialog(
        &self,
        retry: plico::revert_operation::Batch,
        files: Option<Vec<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx,move |d,_,_| {
            let file_owner=owner.clone();let file_path=path.clone();let file_retry=retry.clone();
            let owner=owner.clone();let path=path.clone();let retry=retry.clone();
            let rows=retry.targets().map(|target|div().child(target.to_owned())).collect::<Vec<_>>();
            d.title(t("Local Changes Prevent Revert")).w(px(600.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t("Save local changes in a stash and retry only the remaining commits below? Changes will be restored after completion; a conflict keeps them saved until you finish or abort.")))
                .child(div().id("revert-retry-commits").max_h(px(220.)).overflow_y_scroll().children(rows))
                .when_some(files.as_ref(), |d, files| d
                    .child(div().child(t("Local Changes")))
                    .child(div().id("revert-local-files").max_h(px(180.)).overflow_y_scroll()
                        .children(files.iter().map(|path| div().whitespace_normal().child(path.clone())))))
                .footer(DialogFooter::new().flex_wrap().gap_2()
                    .child(Button::new("show-revert-local-files").label(t("Show Files")).on_click(move |_,w,cx| {
                        w.close_dialog(cx);let _=file_owner.update(cx,|s,cx|if s.repo.path==file_path {
                            s.show_revert_local_files(file_retry.clone(),w,cx);
                        });
                    }))
                    .child(Button::new("save-and-retry-revert").label(t("Save and Retry")).on_click(move |_,w,cx| {
                        w.close_dialog(cx);let _=owner.update(cx,|s,cx|if s.repo.path==path {
                            s.run_sequence(Operation::Revert,Request::SaveRevert(retry.clone()),w,cx);
                        });
                    }))
                    .child(DialogClose::new().trigger(|b|b.primary().label(t("Cancel")))))
                .on_ok(|_,_,_|true)
        });
    }
}

impl Review {
    fn cherry_retry_dialog(
        &self,
        prepared: plico::cherry_pick::Prepared,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        w.open_dialog(cx, move |d, _, _| {
            let files_owner = owner.clone(); let files_path = path.clone();
            let owner = owner.clone(); let path = path.clone(); let prepared = prepared.clone();
            d.title(t("Local Changes Prevent Cherry-pick")).w(px(560.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t("Save local changes in a stash and retry? Changes will be restored after success. If the operation stops, they remain available in Stashes.")))
                .footer(DialogFooter::new().gap_2()
                    .child(Button::new("show-cherry-local-files").label(t("Show Files")).on_click(move |_, w, cx| {
                        w.close_dialog(cx);
                        let _ = files_owner.update(cx, |s, cx| if s.repo.path == files_path { s.open_local(w, cx); });
                    }))
                    .child(Button::new("save-and-retry-cherry").primary().label(t("Save and Retry")).on_click(move |_, w, cx| {
                        w.close_dialog(cx);
                        let _ = owner.update(cx, |s, cx| if s.repo.path == path {
                            s.run_sequence(Operation::CherryPick, Request::SaveCherryPick(prepared.clone()), w, cx);
                        });
                    }))
                    .child(DialogClose::new().trigger(|b| b.label(t("Cancel")))))
                .on_ok(|_, _, _| true)
        });
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use gpui::component::Root;

    #[gpui::test]
    fn start_cherry_pick_releases_barrier_and_refreshes_head(cx: &mut gpui::TestAppContext) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(
            std::env::temp_dir().join(format!("plico-cherry-ui-{}-{nonce}", std::process::id())),
        );
        std::fs::create_dir(&fixture.0).unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        };
        git(&["init", "-q"]);
        git(&["config", "user.name", "Test"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.0.join("file.txt"), "base").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        let base = git(&["rev-parse", "HEAD"]);
        git(&["checkout", "-b", "feature"]);
        std::fs::write(fixture.0.join("file.txt"), "picked").unwrap();
        git(&["commit", "-am", "picked"]);
        let target = git(&["rev-parse", "HEAD"]);
        git(&["checkout", "--detach", &base]);
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(fixture.0.clone(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.run_sequence_with_editor(
                    Operation::CherryPick,
                    Request::CherryPick(vec![target.clone()]),
                    Some("true".into()),
                    w,
                    cx,
                )
            })
        });
        cx.run_until_parked();
        assert_eq!(
            std::fs::read(fixture.0.join("file.txt")).unwrap(),
            b"picked"
        );
        assert_eq!(git(&["log", "-1", "--format=%s"]), "picked");
        assert!(git(&["diff", "--cached"]).is_empty());
        review.read_with(cx, |s, _| {
            assert!(!s.operation.busy());
            assert!(!s.documents.locked());
        });
    }
}
