//! Main-window checkout coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn open_checkout_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy()
            || self.settings_open
            || self.welcome
            || !self.snapshot.as_ref().is_some_and(|s| s.head_id.is_some())
        {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let input = cx.new(|cx| crate::revision_input::RevisionInput::new(snapshot, window, cx));
        let focus = input.clone();
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let input = input.clone();
            let confirm = input.clone();
            let owner = owner.clone();
            let path = path.clone();
            dialog
                .title(t("Checkout Tag or Revision"))
                .w(px(480.))
                .overlay_closable(false)
                .centered(true)
                .child(input.clone())
                .footer(crate::dialog_style::confirmation(
                    Button::new("checkout-input-ok").primary().label(t("OK")),
                ))
                .on_ok(move |_, window, cx| {
                    let text = confirm.read(cx).value(cx);
                    if text.is_empty() {
                        return false;
                    }
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path {
                            s.resolve_checkout_input(text, window, cx);
                        }
                    });
                    true
                })
        });
        focus.update(cx, |s, cx| s.focus(window, cx));
    }
    pub(super) fn resolve_checkout_input(
        &mut self,
        input: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Resolving revision…").into());
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(
                    async move { plico::checkout::resolve_input(&repo, &input, &token) },
                )
                .await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    if cancelled {
                        s.status_message = Some(t("Checkout cancelled; repository refreshed").into());
                    } else {
                        match result {
                            Ok(resolved) => s.start_checkout(
                                resolved.target,
                                resolved.expected_head,
                                window,
                                cx,
                            ),
                            Err(error) => {
                                s.status_message =
                                    Some(format!("{}: {}", t("Could not resolve revision"), error))
                            }
                        }
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn start_local_checkout(
        &mut self,
        reference: String,
        target: String,
        head: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_local_checkout(reference, target, head, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Checking out branch…").into());
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let checked_ref = reference.clone();
            let checked_target = target.clone();
            let recovery_head = head.clone();
            let result = wx
                .background_spawn(async move {
                    plico::checkout::execute_local(
                        &repo,
                        &reference,
                        &target,
                        &head,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    use plico::checkout::Status;
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    let status = result
                        .as_ref()
                        .map(|r| r.status(&checked_ref, &checked_target));
                    let message = if cancelled {
                        "Checkout cancelled; repository refreshed"
                    } else {
                        match status {
                            Ok(Status::Success) => "Branch checked out",
                            Ok(Status::Cancelled) => "Checkout cancelled; repository refreshed",
                            Ok(Status::TimedOut) => "Checkout timed out; repository refreshed",
                            Ok(Status::LocalChanges) => {
                                "Local changes would be overwritten by checkout"
                            }
                            Ok(Status::UntrackedFiles) => {
                                "Untracked files would be overwritten by checkout"
                            }
                            Ok(Status::Unmerged) => {
                                "Resolve existing index conflicts before checkout"
                            }
                            _ => "Could not check out branch; repository refreshed",
                        }
                    };
                    if !cancelled && status == Ok(Status::LocalChanges) {
                        let files = result
                            .as_ref()
                            .unwrap()
                            .affected_files(&checked_ref, &checked_target);
                        s.show_checkout_recovery(
                            checked_target,
                            recovery_head,
                            Some(checked_ref),
                            files,
                            window,
                            cx,
                        );
                    }

                    s.operation_state = None;
                    s.status_message = Some(t(message).into());
                    s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                    s.refresh_impl(false, true, None, cx);
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn start_checkout(
        &mut self,
        target: String,
        expected_head: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_checkout(target, expected_head, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Checking out commit…").into());
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let checked_target = target.clone();
            let recovery_head = expected_head.clone();
            let result = wx
                .background_spawn(async move {
                    plico::checkout::execute(
                        &repo,
                        &target,
                        &expected_head,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    let message = if cancelled {
                        "Checkout cancelled; repository refreshed"
                    } else {
                        use plico::checkout::Status;
                        match result.as_ref().map(|result| result.status(&checked_target)) {
                            Ok(Status::Success) => "Commit checked out (detached HEAD)",
                            Ok(Status::Cancelled) => "Checkout cancelled; repository refreshed",
                            Ok(Status::TimedOut) => "Checkout timed out; repository refreshed",
                            Ok(Status::Unmerged) => {
                                "Resolve existing index conflicts before checkout"
                            }
                            Ok(Status::LocalChanges) => {
                                "Local changes would be overwritten by checkout"
                            }
                            Ok(Status::UntrackedFiles) => {
                                "Untracked files would be overwritten by checkout"
                            }
                            _ => "Could not check out commit; repository refreshed",
                        }
                    };
                    let recovery = if !cancelled {
                        result
                            .as_ref()
                            .ok()
                            .filter(|r| {
                                r.status(&checked_target) == plico::checkout::Status::LocalChanges
                            })
                            .map(|r| r.affected_files(&checked_target))
                    } else {
                        None
                    };
                    if let Some(files) = recovery {
                        s.show_checkout_recovery(
                            checked_target.clone(),
                            recovery_head,
                            None,
                            files,
                            window,
                            cx,
                        );
                    }
                    s.operation_state = None;
                    s.status_message = Some(t(message).into());
                    s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                    s.refresh_impl(false, true, None, cx);
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn show_checkout_recovery(
        &mut self,
        target: String,
        head: String,
        reference: Option<String>,
        files: plico::checkout_files::Files,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        let cancel_focus = cx.focus_handle();
        let force_focus = cx.focus_handle();
        let initial_focus = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                cancel_focus.focus(window, cx);
            }
            let mut footer = h_flex().w_full().gap_2();
            for smart in [false, true] {
                let owner = owner.clone();
                let target = target.clone();
                let head = head.clone();
                let path = path.clone();
                let reference = reference.clone();
                let button = Button::new(if smart {
                    "smart-checkout"
                } else {
                    "force-checkout"
                })
                .when(smart, |button| button.primary())
                .when(!smart, |button| button.focus_handle(force_focus.clone()))
                .min_w(px(72.))
                .label(t(if smart {
                    "Smart Checkout"
                } else {
                    "Force Checkout"
                }))
                .tooltip(t(if smart {
                    "Save local changes, check out, then restore them"
                } else {
                    "Check out and overwrite local changes"
                }))
                .on_click(move |_, window, cx| {
                    window.close_dialog(cx);
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path {
                            s.start_checkout_recovery(
                                target.clone(),
                                head.clone(),
                                reference.clone(),
                                smart,
                                window,
                                cx,
                            );
                        }
                    });
                });
                footer = footer.child(button);
                if !smart {
                    footer = footer.child(div().flex_1());
                }
            }
            footer = footer.child(
                Button::new("cancel-checkout")
                    .focus_handle(cancel_focus.clone())
                    .min_w(px(72.))
                    .label(t("Don't Checkout"))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            );
            let owner = owner.clone();
            let target = target.clone();
            let head = head.clone();
            let reference = reference.clone();
            let path = path.clone();
            let cancel_focus = cancel_focus.clone();
            let force_focus = force_focus.clone();
            dialog
                .title(t("Checkout Problem"))
                .w(px(640.))
                .centered(true)
                .overlay_closable(false)
                .child(div().child(t(
                    "Local changes would be overwritten. Smart Checkout saves and restores them.",
                )))
                .child(
                    div()
                        .id("checkout-affected-files")
                        .h(px(220.))
                        .overflow_y_scroll()
                        .children(files.paths.iter().map(|path| div().child(path.clone()))),
                )
                .when(!files.complete, |d| {
                    d.child(t("The affected file list may be incomplete"))
                })
                .footer(footer)
                .on_ok(move |_, window, cx| {
                    if cancel_focus.is_focused(window) {
                        return true;
                    }
                    let smart = !force_focus.is_focused(window);
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path {
                            s.start_checkout_recovery(
                                target.clone(),
                                head.clone(),
                                reference.clone(),
                                smart,
                                window,
                                cx,
                            );
                        }
                    });
                    true
                })
        });
    }
    pub(super) fn start_checkout_recovery(
        &mut self,
        target: String,
        head: String,
        reference: Option<String>,
        smart: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_checkout_recovery(target, head, reference, smart, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(
            t(if reference.is_some() {
                "Checking out branch…"
            } else {
                "Checking out commit…"
            })
            .into(),
        );
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let (message, saved) = wx.background_spawn(async move {
                if let Some(reference) = reference {
                    use plico::checkout;
                    if !smart {
                        let result = checkout::force_execute_local(&repo, &reference, &target, &head, &token, &Default::default());
                        return (if result.as_ref().is_ok_and(|r| r.checked_out(&reference, &target)) { "Branch checked out" }
                            else { "Could not check out branch; repository refreshed" }, None);
                    }
                    return match checkout::smart_execute_local(&repo, &reference, &target, &head, true, &token, &Default::default()) {
                        Ok(result) => {
                            let restored = result.restore.as_ref().is_ok_and(|r| r.as_ref().is_none_or(|out| out.status.success() && out.stopped.is_none()));
                            let checked = result.checkout.as_ref().is_some_and(|r| r.as_ref().is_ok_and(|r| r.checked_out(&reference, &target)));
                            let clean = result.state.as_ref().is_ok_and(|s| s.conflicts.is_empty());
                            let message = if !restored || result.saved_oid.is_some() || !clean { "Local changes were not fully restored; review conflicts and saved stash" }
                                else if checked && result.head.as_ref().is_ok_and(|id| id == &target)
                                    && result.branch.as_ref().is_ok_and(|name| name == &reference) { "Branch checked out" }
                                else { "Could not check out branch; repository refreshed" };
                            (message, result.saved_oid)
                        }
                        Err(_) => ("Could not check out branch; repository refreshed", None),
                    };
                }
                if !smart {
                    let result = plico::checkout::force_execute(&repo, &target, &head, &token, &Default::default());
                    return (if result.as_ref().is_ok_and(|r| r.checked_out(&target)) { "Commit checked out (detached HEAD)" }
                        else { "Could not check out commit; repository refreshed" }, None);
                }
                match plico::checkout::smart_execute(&repo, &target, &head, true, &token, &Default::default()) {
                    Ok(result) => {
                        let restored = result.restore.as_ref().is_ok_and(|r| r.as_ref().is_none_or(|out| out.status.success() && out.stopped.is_none()));
                        let checked = result.checkout.as_ref().is_some_and(|r| r.as_ref().is_ok_and(|r| r.checked_out(&target)));
                        let clean_state = result.state.as_ref().is_ok_and(|s| s.conflicts.is_empty());
                        let message = if !restored || result.saved_oid.is_some() || !clean_state { "Local changes were not fully restored; review conflicts and saved stash" }
                            else if checked && result.head.as_ref().is_ok_and(|id| id == &target) { "Commit checked out; local changes restored" }
                            else { "Could not check out commit; repository refreshed" };
                        (message, result.saved_oid)
                    }
                    Err(_) => ("Could not check out commit; repository refreshed", None),
                }
            }).await;
            let _ = wx.update(|_, cx| { let _ = this.update(cx, |s, cx| {
                if s.finish_operation(operation, cx).is_none() { return; } s.operation_state = None;
                s.saved_changes_available |= saved.is_some();
                s.status_message = Some(match saved { Some(oid) => format!("{} (stash: {})", t(message), oid), None => t(message).into() });
                s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                s.refresh_impl(false, true, None, cx); cx.notify();
            }); });
        });
        cx.notify();
    }
}
