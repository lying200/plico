//! Branch actions and dialog lifecycle; Git validation/execution lives in branch_create.
use super::*;
use gpui_kit::component::checkbox::Checkbox;
use plico::branch_create::{Choice, Prepared};
struct NewBranch {
    name: Entity<InputState>,
    checkout: bool,
    overwrite: bool,
    existing_names: Vec<String>,
    busy: bool,
    closed: bool,
    error: Option<String>,
    token: Cancel,
    task: Task<()>,
    _subscription: Subscription,
}
impl Drop for NewBranch {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Render for NewBranch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let existing = self
            .existing_names
            .iter()
            .any(|name| name == self.name.read(cx).value().as_ref());
        v_flex()
            .gap_3()
            .min_w_0()
            .child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .child(
                        div()
                            .pt_1()
                            .w(px(90.))
                            .flex_shrink_0()
                            .child(t("Branch name:")),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(Input::new(&self.name).disabled(self.busy).w_full())
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(
                                        Checkbox::new("checkout-new-branch")
                                            .label(t("Checkout branch"))
                                            .checked(self.checkout)
                                            .disabled(self.busy)
                                            .on_click(cx.listener(|s, checked, _, cx| {
                                                s.checkout = *checked;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Checkbox::new("overwrite-existing-branch")
                                            .label(t("Overwrite existing branch"))
                                            .checked(self.overwrite && existing)
                                            .disabled(self.busy || !existing)
                                            .on_click(cx.listener(|s, checked, _, cx| {
                                                s.overwrite = *checked;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
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
    pub(super) fn new_branch_dialog(
        &mut self,
        source: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() || self.welcome {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(displayed_head) = snapshot.head_id.clone() else {
            return;
        };
        let displayed_target = if source == "HEAD" {
            Some(displayed_head.clone())
        } else {
            snapshot
                .branches
                .iter()
                .map(|r| (&r.name, &r.id))
                .chain(snapshot.tags.iter().map(|r| (&r.name, &r.id)))
                .find(|(name, _)| **name == source)
                .map(|(_, id)| id.clone())
                .or_else(|| {
                    self.commits
                        .iter()
                        .find(|c| c.id == source)
                        .map(|c| c.id.clone())
                })
                // Reflog commits can be absent from the visible Log. Prepared
                // still resolves the full OID and revalidates before writing.
                .or_else(|| plico::git::is_oid(&source).then(|| source.clone()))
        };
        let Some(displayed_target) = displayed_target else {
            return;
        };
        let existing_names: Vec<String> = snapshot
            .branches
            .iter()
            .filter_map(|r| r.name.strip_prefix("refs/heads/").map(str::to_owned))
            .collect();
        let initial_name = if source == "HEAD" {
            snapshot.current_branch.clone().unwrap_or_default()
        } else {
            source.strip_prefix("refs/heads/").unwrap_or("").to_owned()
        };
        self.dismiss_repository_menu(cx);
        let options = cx.new(|cx| {
            let name = cx.new(|cx| InputState::new(window, cx).default_value(initial_name));
            name.update(cx, |input, cx| input.select_all(window, cx));
            let subscription = cx.subscribe_in(
                &name,
                window,
                |s: &mut NewBranch, input, event: &InputEvent, w, cx| {
                    if !matches!(event, InputEvent::Change | InputEvent::Blur) {
                        return;
                    }
                    s.error = None;
                    if matches!(event, InputEvent::Change) {
                        s.overwrite = false;
                    }
                    input.update(cx, |input, cx| {
                        // Do not rewrite an IME preedit. Blur also handles a composition
                        // committed without a subsequent Change event.
                        if gpui::EntityInputHandler::marked_text_range(input, w, cx).is_some() {
                            return;
                        }
                        let original = input.value();
                        let cleaned = plico::branch_create::clean_name_on_typing(&original);
                        if cleaned == original.as_ref() {
                            return;
                        }
                        let caret = input.cursor();
                        let new_caret =
                            plico::branch_create::clean_name_on_typing(&original[..caret])
                                .len()
                                .min(cleaned.len());
                        input.set_value(cleaned, w, cx);
                        input.set_selected_range(new_caret..new_caret, cx);
                    });
                    cx.notify();
                },
            );
            NewBranch {
                name,
                checkout: true,
                overwrite: false,
                existing_names,
                busy: false,
                closed: false,
                error: None,
                token: Cancel::default(),
                task: Task::ready(()),
                _subscription: subscription,
            }
        });
        let focus = options.read(cx).name.read(cx).focus_handle(cx);
        let owner = cx.entity().downgrade();
        let repo = self.repo.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let confirm = options.clone(); let close = options.clone();
            let owner = owner.clone(); let repo = repo.clone(); let source = source.clone();
            let expected_head = displayed_head.clone(); let expected_target = displayed_target.clone();
            dialog.title(h_flex().gap_2().child(gpui_kit::img("branding/plico.svg").size(px(16.))).child(div().font_weight(FontWeight::NORMAL).child(t("Create New Branch")))).w(px(540.)).centered(true).overlay_closable(false)
                .child(options.clone())
                .footer(crate::dialog_style::confirmation(Button::new("create-branch").primary()
                    .label(t(if options.read(cx).busy { "Checking…" } else { "Create" }))
                    .disabled(options.read(cx).busy || options.read(cx).name.read(cx).value().trim().is_empty())))
                .on_ok(move |_, window, cx| {
                    let owner = owner.clone(); let repo = repo.clone(); let source = source.clone();
                    let expected_head=expected_head.clone();let expected_target=expected_target.clone();
                    confirm.update(cx, |s, cx| {
                        if s.busy || s.closed { return; }
                        s.busy = true; s.error = None;
                        let name = plico::branch_create::clean_name(&s.name.read(cx).value());
                        s.name.update(cx, |input, cx| input.set_value(name.clone(), window, cx));
                        let checkout = s.checkout;
                        let overwrite = s.overwrite && s.existing_names.contains(&name);
                        let token = s.token.clone(); let path = repo.path.clone();
                        s.task = cx.spawn_in(window, async move |this, wx| {
                            let result = wx.background_spawn(async move { Prepared::read_options(&repo, &name, &source, checkout, overwrite, &token).and_then(|p| {
                                if p.matches_displayed(&expected_head,&expected_target) { Ok(p) }
                                else { Err("Repository changed; close this dialog and review the starting point again".into()) }
                            }) }).await;
                            let _ = wx.update(|window, cx| {
                                let ready = this.update(cx, |s, cx| {
                                    if s.closed || s.token.cancelled() { return None; }
                                    s.busy = false;
                                    match result { Ok(prepared) => Some(prepared), Err(error) => { s.error = Some(error); cx.notify(); None } }
                                }).ok().flatten();
                                if let Some(prepared) = ready {
                                    window.close_dialog(cx);
                                    let _ = owner.update(cx, |s, cx| if s.repo.path == path { s.confirm_create_branch(Arc::new(prepared), window, cx); });
                                }
                            });
                        });
                        cx.notify();
                    });
                    false
                })
                .on_close(move |_, _, cx| close.update(cx, |s, _| { s.closed = true; s.token.cancel(); }))
        });
        focus.focus(window, cx);
    }
    fn confirm_create_branch(
        &mut self,
        prepared: Arc<Prepared>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if prepared.displaced_commits.is_empty() {
            let choice = prepared.default_choice();
            self.create_branch(prepared, choice, window, cx);
            return;
        }
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let mut footer = h_flex().gap_2().w_full().justify_end();
            let mut choices = Vec::new();
            if prepared.can_rebase() {
                choices.push((Choice::Rebase, "Checkout and Rebase"));
            }
            choices.push(if prepared.overwrites() {
                (Choice::Discard, "Drop Local Commits")
            } else {
                (Choice::KeepLocal, "Keep Local Branch")
            });
            for (choice, label) in choices {
                let owner = owner.clone();
                let prepared = prepared.clone();
                let path = path.clone();
                footer = footer.child(
                    Button::new(label)
                        .label(t(label))
                        .when(choice == Choice::Rebase, |b| b.primary())
                        .on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = owner.update(cx, |s, cx| {
                                if s.repo.path == path {
                                    s.create_branch(prepared.clone(), choice, w, cx);
                                }
                            });
                        }),
                );
            }
            footer = footer.child(
                Button::new("branch-choice-cancel")
                    .label(t("Cancel"))
                    .on_click(|_, w, cx| w.close_dialog(cx)),
            );
            dialog
                .title(t("Existing Branch Commits"))
                .w(px(600.))
                .centered(true)
                .overlay_closable(false)
                .child(div().whitespace_normal().child(t(
                    "The existing branch has commits not included in the starting point.",
                )))
                .child(
                    v_flex()
                        .id("displaced-branch-commits")
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .children(
                            prepared
                                .displaced_commits
                                .iter()
                                .take(100)
                                .map(|line| div().truncate().child(line.clone())),
                        )
                        .when(prepared.displaced_commits.len() > 100, |v| {
                            v.child(t("More commits are not shown"))
                        }),
                )
                .footer(footer)
        });
    }
    fn create_branch(
        &mut self,
        prepared: Arc<Prepared>,
        choice: Choice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.create_branch(prepared, choice, w, cx)
            });
            return;
        }
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.status_message = Some(t("Creating branch…").into());
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::branch_create::execute_choice(
                        &repo,
                        &prepared,
                        choice,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = this.update_in(wx, |s, window, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                let mut error = None;
                let success = match result {
                    Ok(report) => {
                        if let Some(state) = report.state {
                            s.operation_state = Some(state);
                        }
                        if !report.verified {
                            error = Some(match report.command {
                                Ok(command) => format!(
                                    "{}\n{}",
                                    String::from_utf8_lossy(&command.stderr.bytes()),
                                    report.refresh_error.unwrap_or_default()
                                ),
                                Err(error) => error,
                            });
                        }
                        report.verified
                    }
                    Err(e) => {
                        error = Some(e);
                        false
                    }
                };
                s.status_message = Some(
                    t(if cancelled {
                        "Branch operation cancelled; repository refreshed"
                    } else if success {
                        "Branch operation completed"
                    } else {
                        "Could not create branch"
                    })
                    .into(),
                );
                if let Some(error) = error {
                    crate::error_ui::show_details(error, "Could not create branch", window, cx);
                }
                s.sync_worktree(
                    plico::worktree_events::Batch {
                        all: true,
                        index: true,
                        ..Default::default()
                    },
                    true,
                    window,
                    cx,
                );
                s.refresh_impl(false, true, None, cx);
                cx.notify();
            });
        });
    }
}

impl Review {
    pub(super) fn delete_merged_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        let Some(merged) = self.merged_branch.clone() else {
            return;
        };
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move { merged.prepare_deletion(&repo, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if cancelled {
                    return;
                }
                s.merged_branch = None;
                match result {
                    Ok(prepared) => s.confirm_delete_branch(Arc::new(prepared), w, cx),
                    Err(error) => {
                        crate::error_ui::show_details(error, "Could not delete branch", w, cx)
                    }
                }
            });
        });
    }

    pub(super) fn prepare_delete_branch(
        &mut self,
        reference: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        self.dismiss_repository_menu(cx);
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::branch_delete::Prepared::read(&repo, &reference, &token)
                })
                .await;
            let _ = this.update_in(wx, |s, window, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if cancelled {
                    return;
                }
                match result {
                    Err(error) => {
                        crate::error_ui::show_details(error, "Could not delete branch", window, cx)
                    }
                    Ok(prepared) => s.confirm_delete_branch(Arc::new(prepared), window, cx),
                }
            });
        });
    }
    fn confirm_delete_branch(
        &self,
        prepared: Arc<plico::branch_delete::Prepared>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let confirm = prepared.clone(); let owner = owner.clone(); let path = path.clone();
            let unmerged = !prepared.unmerged.is_empty();
            dialog.title(t(if unmerged { "Branch Is Not Fully Merged" } else { "Delete Branch" }))
                .w(px(560.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(format!("{} ‘{}’", t("Delete branch"), prepared.name)))
                .when(unmerged, |d| d.child(div().whitespace_normal().child(t("These commits are not merged into the branch's upstream or HEAD. Delete the branch anyway?")))
                    .child(v_flex().id("unmerged-branch-commits").max_h(px(240.)).overflow_y_scroll()
                        .children(prepared.unmerged.iter().take(100).map(|commit| div().truncate().child(commit.clone())))
                        .when(prepared.unmerged.len()>100,|v|v.child(t("More commits are not shown")))))
                .footer(crate::dialog_style::confirmation(Button::new("delete-branch-confirm").primary().label(t("Delete"))))
                .on_ok(move |_, w, cx| {
                    let _=owner.update(cx,|s,cx|if s.repo.path==path {s.delete_branch(confirm.clone(),false,w,cx);}); true
                })
        });
    }
    pub(super) fn restore_deleted_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((path, prepared)) = self.deleted_branch.clone() {
            if path == self.repo.path {
                self.delete_branch(prepared, true, window, cx);
            }
        }
    }
    fn delete_branch(
        &mut self,
        prepared: Arc<plico::branch_delete::Prepared>,
        restore: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        let recovery = prepared.clone();
        self.status_message = Some(
            t(if restore {
                "Restoring branch…"
            } else {
                "Deleting branch…"
            })
            .into(),
        );
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    if restore {
                        return plico::branch_delete::restore(
                            &repo,
                            &prepared,
                            &token,
                            &Default::default(),
                        )
                        .map(|_| (false, None));
                    }
                    let report = plico::branch_delete::execute(
                        &repo,
                        &prepared,
                        !prepared.unmerged.is_empty(),
                        &token,
                        &Default::default(),
                    )?;
                    let error = match report.command {
                        Ok(outcome) if outcome.status.success() && outcome.stopped.is_none() => {
                            report.refresh_error
                        }
                        Ok(outcome) => Some(format!(
                            "{}\n{}",
                            String::from_utf8_lossy(&outcome.stderr.bytes()),
                            report.refresh_error.unwrap_or_default()
                        )),
                        Err(error) => Some(error),
                    };
                    Ok((report.deleted, error))
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                let error = match result {
                    Ok((deleted, error)) => {
                        if deleted {
                            s.deleted_branch = Some((s.repo.path.clone(), recovery));
                        }
                        if restore && error.is_none() {
                            s.deleted_branch = None;
                        }
                        if !restore && !deleted && error.is_none() {
                            Some("Branch still exists; refresh and retry".into())
                        } else {
                            error
                        }
                    }
                    Err(error) => Some(error),
                };
                s.status_message = Some(
                    t(if cancelled {
                        "Branch operation cancelled; repository refreshed"
                    } else if error.is_some() {
                        "Branch operation failed"
                    } else if restore {
                        "Branch restored"
                    } else {
                        "Branch deleted"
                    })
                    .into(),
                );
                if let Some(error) = error {
                    crate::error_ui::show_details(error, "Branch operation failed", w, cx);
                }
                s.refresh_impl(false, true, None, cx);
                cx.notify();
            });
        });
    }
}
