//! Conflict-list adapter. Versions and mutations are owned by plico::conflicts.
use super::*;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use plico::conflicts::{Resolution, Side, Snapshot};

struct ConflictList {
    owner: WeakEntity<Review>,
    repository: PathBuf,
    snapshot: Arc<Snapshot>,
    selected: Option<usize>,
}
impl ConflictList {
    fn merge(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self
            .selected
            .and_then(|i| self.snapshot.files.get(i))
            .cloned()
        else {
            return;
        };
        if file
            .stages
            .iter()
            .flatten()
            .any(|v| v.mode == "120000" || v.mode == "160000")
        {
            return;
        }
        let snapshot = self.snapshot.clone();
        let _ = self.owner.update(cx, |owner, cx| {
            if owner.repo.path == self.repository {
                owner.with_saved_documents(window, cx, move |s, w, cx| {
                    s.open_text_merge(snapshot, file, w, cx)
                });
            }
        });
    }
    fn act(&self, resolution: Option<Resolution>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self
            .selected
            .and_then(|i| self.snapshot.files.get(i))
            .cloned()
        else {
            return;
        };
        let snapshot = self.snapshot.clone();
        window.close_dialog(cx);
        let _ = self.owner.update(cx, |s, cx| {
            if s.repo.path != self.repository {
                return;
            }
            if let Some(resolution) = resolution {
                s.with_saved_documents(window, cx, move |s, w, cx| {
                    s.resolve_conflict(snapshot, file, resolution, w, cx)
                });
            } else {
                s.open_project_file(file.path, false, window, cx);
            }
        });
    }
}
impl Render for ConflictList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.and_then(|i| self.snapshot.files.get(i));
        let disabled =
            selected.is_none_or(|f| f.stages.iter().flatten().any(|v| v.mode == "160000"));
        h_flex()
            .gap_3()
            .items_start()
            .child(
                v_flex()
                    .id("conflict-files")
                    .flex_1()
                    .min_w_0()
                    .h(px(300.))
                    .overflow_y_scroll()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(t("File")))
                            .child(div().w(px(76.)).child(t("Yours")))
                            .child(div().w(px(76.)).child(t("Theirs"))),
                    )
                    .when(self.snapshot.files.is_empty(), |v| {
                        v.child(div().p_3().child(t("All conflicts are resolved")))
                    })
                    .children(self.snapshot.files.iter().enumerate().map(|(i, file)| {
                        let status = |side| {
                            t(if file.side(side, self.snapshot.reversed).is_some() {
                                "Modified"
                            } else {
                                "Deleted"
                            })
                        };
                        h_flex()
                            .id(("conflict-file", i))
                            .gap_2()
                            .px_2()
                            .py_1()
                            .min_w_0()
                            .cursor_pointer()
                            .when(self.selected == Some(i), |row| row.bg(rgb(0x2e4369)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(file.path.to_string_lossy().into_owned()),
                            )
                            .child(div().w(px(76.)).child(status(Side::Yours)))
                            .child(div().w(px(76.)).child(status(Side::Theirs)))
                            .on_click(cx.listener(move |s, event: &ClickEvent, w, cx| {
                                s.selected = Some(i);
                                if event.click_count() == 2 {
                                    s.merge(w, cx);
                                }
                                cx.notify();
                            }))
                    })),
            )
            .child(
                v_flex()
                    .gap_2()
                    .w(px(150.))
                    .flex_shrink_0()
                    .child(
                        Button::new("accept-yours")
                            .label(t("Accept Yours"))
                            .w_full()
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.act(Some(Resolution::Accept(Side::Yours)), w, cx)
                            })),
                    )
                    .child(
                        Button::new("accept-theirs")
                            .label(t("Accept Theirs"))
                            .w_full()
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.act(Some(Resolution::Accept(Side::Theirs)), w, cx)
                            })),
                    )
                    .child(
                        Button::new("native-merge")
                            .label(t("Merge…"))
                            .w_full()
                            .disabled(
                                disabled
                                    || selected.is_some_and(|f| {
                                        f.stages.iter().flatten().any(|v| v.mode == "120000")
                                    }),
                            )
                            .on_click(cx.listener(|s, _, w, cx| s.merge(w, cx))),
                    )
                    .child(
                        Button::new("edit-conflict")
                            .label(t("Edit File"))
                            .w_full()
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.act(None, w, cx))),
                    )
                    .child(
                        Button::new("mark-resolved")
                            .label(t("Mark Resolved"))
                            .w_full()
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.act(Some(Resolution::MarkResolved), w, cx)
                            })),
                    ),
            )
    }
}
impl Review {
    pub(super) fn open_conflicts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                .background_spawn(async move { Snapshot::read(&repo, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if cancelled {
                    return;
                }
                match result {
                    Err(error) => {
                        crate::error_ui::show_details(error, "Could not read conflicts", w, cx)
                    }
                    Ok(snapshot) => {
                        let owner = cx.entity().downgrade();
                        let repository = s.repo.path.clone();
                        let selected = (!snapshot.files.is_empty()).then_some(0);
                        let view = cx.new(|_| ConflictList {
                            owner,
                            repository,
                            snapshot: Arc::new(snapshot),
                            selected,
                        });
                        w.open_dialog(cx, move |d, _, _| {
                            d.title(t("Conflicts"))
                                .w(px(760.))
                                .centered(true)
                                .overlay_closable(false)
                                .child(view.clone())
                                .footer(DialogFooter::new().child(
                                    DialogClose::new().trigger(|button| button.label(t("Close"))),
                                ))
                        });
                    }
                }
            });
        });
    }
    pub(super) fn resolve_conflict(
        &mut self,
        snapshot: Arc<Snapshot>,
        file: plico::conflicts::File,
        resolution: Resolution,
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
        let resolved_path = file.path.clone();
        self.status_message = Some(t("Resolving conflict…").into());
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::conflicts::resolve(
                        &repo,
                        &snapshot,
                        &file,
                        resolution,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                let error = match result {
                    Err(error) => Some(error),
                    Ok(report) => report
                        .commands
                        .into_iter()
                        .find_map(|result| match result {
                            Err(error) => Some(error),
                            Ok(o)
                                if !o.status.success()
                                    || o.stopped.is_some()
                                    || o.input_error.is_some() =>
                            {
                                Some(format!(
                                    "{}\n{:?}",
                                    String::from_utf8_lossy(&o.stderr.bytes()),
                                    o.stopped
                                ))
                            }
                            _ => None,
                        })
                        .or_else(|| match report.snapshot {
                            Err(error) => Some(error),
                            Ok(snapshot)
                                if snapshot.files.iter().any(|f| f.path == resolved_path) =>
                            {
                                Some("The file still has unresolved index entries".into())
                            }
                            Ok(_) => None,
                        }),
                };
                s.status_message = Some(
                    t(if error.is_some() || cancelled {
                        "Conflict resolution interrupted"
                    } else {
                        "Conflict resolved"
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
                if let Some(error) = error {
                    crate::error_ui::show_details(error, "Could not resolve conflict", w, cx);
                } else if !cancelled {
                    s.open_conflicts(w, cx);
                }
            });
        });
    }
}
