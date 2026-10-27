//! Main-window checkout coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

#[derive(Clone)]
pub(super) enum Recovery {
    Checkout {
        repository: Repository,
        target: String,
        head: String,
        reference: Option<String>,
    },
    Reference {
        prepared: plico::reference_checkout::Prepared,
        choice: plico::reference_checkout::Choice,
    },
}

impl Review {
    /// Only local branches from this exact Log row are checkout candidates.
    pub(super) fn log_checkout_branches(&self, index: usize) -> Vec<String> {
        let Some(repository) = self.log_repository(index) else {
            return vec![];
        };
        let Some(commit) = self.log.commits().get(index) else {
            return vec![];
        };
        let scope = self.current_log_scope();
        let current = scope
            .as_ref()
            .and_then(|scope| {
                scope
                    .roots
                    .iter()
                    .find(|root| root.query.repository.path == repository.path)
            })
            .and_then(|root| root.current_branch.as_deref());
        let mut references: Vec<_> = commit
            .refs
            .iter()
            .filter(|reference| {
                reference.kind == plico::git::RefKind::Local
                    && Some(reference.name.as_str()) != current
            })
            .collect();
        references.sort_by(|a, b| {
            plico::ref_labels::rank(a, current)
                .cmp(&plico::ref_labels::rank(b, current))
                .then_with(|| plico::natural_order::compare(&a.name, &b.name))
        });
        references
            .into_iter()
            .map(|reference| format!("refs/heads/{}", reference.name))
            .collect()
    }

    pub(super) fn start_log_checkout(
        &mut self,
        repository: Repository,
        target: String,
        reference: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.welcome || self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Checking checkout target…").into());
        let project_path = self.repo.path.clone();
        let repo = repository.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(
                    async move { plico::checkout::resolve_input(&repo, &target, &token) },
                )
                .await;
            let _ = this.update_in(wx, |s, window, cx| {
                let Some(cancelled) = s.finish_operation(operation, cx) else {
                    return;
                };
                if s.repo.path != project_path {
                    return;
                }
                if cancelled {
                    s.status_message = Some(t("Checkout cancelled; repository refreshed").into());
                    s.refresh_impl(false, true, None, cx);
                    return;
                }
                match result {
                    Ok(resolved) => {
                        if let Some(reference) = reference {
                            s.start_local_checkout_in(
                                repository,
                                reference,
                                resolved.target,
                                resolved.expected_head,
                                window,
                                cx,
                            );
                        } else {
                            s.start_checkout_in(
                                repository,
                                resolved.target,
                                resolved.expected_head,
                                window,
                                cx,
                            );
                        }
                    }
                    Err(error) => {
                        s.status_message =
                            Some(format!("{}: {error}", t("Could not resolve revision")));
                        s.refresh_impl(false, true, None, cx);
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub(super) fn open_checkout_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        self.open_checkout_input_in(self.repo.clone(), &snapshot, window, cx);
    }
    pub(super) fn open_checkout_input_in(
        &mut self,
        repository: Repository,
        snapshot: &Snapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.settings_open || self.welcome || snapshot.head_id.is_none()
        {
            return;
        }
        let input = cx.new(|cx| crate::revision_input::RevisionInput::new(snapshot, window, cx));
        let focus = input.clone();
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let input = input.clone();
            let confirm = input.clone();
            let owner = owner.clone();
            let path = path.clone();
            let repository = repository.clone();
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
                            s.resolve_checkout_input_in(repository.clone(), text, window, cx);
                        }
                    });
                    true
                })
        });
        focus.update(cx, |s, cx| s.focus(window, cx));
    }
    pub(super) fn resolve_checkout_input_in(
        &mut self,
        repository: Repository,
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
        let repo = repository.clone();
        let project_path = self.repo.path.clone();
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
                    if s.repo.path != project_path {
                        return;
                    }
                    if cancelled {
                        s.status_message =
                            Some(t("Checkout cancelled; repository refreshed").into());
                    } else {
                        match result {
                            Ok(resolved) => s.start_checkout_in(
                                repository,
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
        self.start_local_checkout_in(self.repo.clone(), reference, target, head, window, cx);
    }
    pub(super) fn start_local_checkout_in(
        &mut self,
        repository: Repository,
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
                s.start_local_checkout_in(repository, reference, target, head, w, cx)
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
        let repo = repository.clone();
        let project_path = self.repo.path.clone();
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
                    if s.repo.path != project_path {
                        return;
                    }
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
                            repository.clone(),
                            checked_target,
                            recovery_head,
                            Some(checked_ref),
                            files,
                            window,
                            cx,
                        );
                    }

                    if s.repo.path == repository.path {
                        s.operation_state = None;
                    }
                    s.status_message = Some(t(message).into());
                    s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                    s.refresh_impl(false, true, None, cx);
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn start_checkout_in(
        &mut self,
        repository: Repository,
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
                s.start_checkout_in(repository, target, expected_head, w, cx)
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
        let repo = repository.clone();
        let project_path = self.repo.path.clone();
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
                    if s.repo.path != project_path {
                        return;
                    }
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
                            repository.clone(),
                            checked_target.clone(),
                            recovery_head,
                            None,
                            files,
                            window,
                            cx,
                        );
                    }
                    if s.repo.path == repository.path {
                        s.operation_state = None;
                    }
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
        repository: Repository,
        target: String,
        head: String,
        reference: Option<String>,
        files: plico::checkout_files::Files,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.checkout_recovery_dialog(
            Recovery::Checkout {
                repository,
                target,
                head,
                reference,
            },
            files,
            window,
            cx,
        );
    }
    pub(super) fn checkout_recovery_dialog(
        &mut self,
        recovery: Recovery,
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
                let recovery = recovery.clone();
                let path = path.clone();
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
                            s.start_checkout_request(recovery.clone(), smart, window, cx);
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
            let recovery = recovery.clone();
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
                            s.start_checkout_request(recovery.clone(), smart, window, cx);
                        }
                    });
                    true
                })
        });
    }
    fn start_checkout_request(
        &mut self,
        recovery: Recovery,
        smart: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match recovery {
            Recovery::Checkout {
                repository,
                target,
                head,
                reference,
            } => self.start_checkout_recovery(repository, target, head, reference, smart, w, cx),
            Recovery::Reference { prepared, choice } => {
                self.recover_reference_checkout(prepared, choice, smart, w, cx)
            }
        }
    }
    pub(super) fn start_checkout_recovery(
        &mut self,
        repository: Repository,
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
                s.start_checkout_recovery(repository, target, head, reference, smart, w, cx)
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
        let repo = repository.clone();
        let project_path = self.repo.path.clone();
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
                if s.finish_operation(operation, cx).is_none() || s.repo.path != project_path { return; }
                if s.repo.path == repository.path { s.operation_state = None; }
                s.saved_changes_available |= saved.is_some();
                s.status_message = Some(match saved { Some(oid) => format!("{} (stash: {})", t(message), oid), None => t(message).into() });
                s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                s.refresh_impl(false, true, None, cx); cx.notify();
            }); });
        });
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;

    #[gpui::test]
    fn child_checkout_and_smart_recovery_keep_repository_identity(cx: &mut gpui::TestAppContext) {
        use gpui::component::Root;
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn git(path: &std::path::Path, args: &[&str]) -> String {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "{args:?}: {out:?}");
            String::from_utf8(out.stdout).unwrap().trim().to_owned()
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-checkout-roots-{}-{nonce}",
            std::process::id()
        )));
        let parent = fixture.0.join("parent");
        let child = fixture.0.join("child");
        std::fs::create_dir_all(&parent).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.hooksPath", "disabled-hooks"],
        ] {
            git(&parent, &args);
        }
        std::fs::write(parent.join("file"), "base\n").unwrap();
        git(&parent, &["add", "."]);
        git(&parent, &["commit", "-qm", "base"]);
        let base = git(&parent, &["rev-parse", "HEAD"]);
        git(
            &parent,
            &["clone", "-q", "--no-local", ".", child.to_str().unwrap()],
        );
        for args in [
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            git(&child, &args);
        }
        std::fs::write(child.join("file"), "child\n").unwrap();
        git(&child, &["commit", "-qam", "child"]);
        let head = git(&child, &["rev-parse", "HEAD"]);
        git(&child, &["branch", "base", &base]);
        git(&child, &["branch", "-m", "child-active"]);
        git(&child, &["branch", "main"]);
        git(&child, &["branch", "feature10"]);
        git(&child, &["branch", "feature2"]);
        git(&child, &["tag", "not-a-branch"]);
        let loaded = plico::project_log::read(
            &[plico::project_log::Query {
                repository: Repository::new(&child),
                branches: vec![],
                filter: None,
            }],
            100,
            &Cancel::default(),
        )
        .unwrap();

        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(parent.clone(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        review.update(cx, |s, _| {
            let previous = std::mem::replace(
                &mut s.log,
                log_display::Display::fixture(loaded.page.entries, Some(loaded.scope)),
            );
            let index = s
                .log
                .commits()
                .iter()
                .position(|commit| commit.id == head)
                .unwrap();
            assert_eq!(
                s.log_checkout_branches(index),
                vec![
                    "refs/heads/main",
                    "refs/heads/feature2",
                    "refs/heads/feature10"
                ],
                "exclude the child's current branch and tags, not the parent's current branch"
            );
            s.log = previous;
        });
        git(&child, &["tag", "base-tag", &base]);
        git(
            &child,
            &["update-ref", "refs/remotes/origin/scoped-topic", &head],
        );
        for (reference, target, expected_head) in [
            ("refs/tags/base-tag", &base, &head),
            ("refs/remotes/origin/scoped-topic", &head, &base),
        ] {
            cx.update(|window, cx| {
                review.update(cx, |s, cx| {
                    s.prepare_reference_checkout_in(
                        Repository::new(&child),
                        reference.into(),
                        target.clone(),
                        expected_head.clone(),
                        None,
                        window,
                        cx,
                    );
                })
            });
            cx.run_until_parked();
            assert_eq!(git(&child, &["rev-parse", "HEAD"]), *target);
            assert_eq!(git(&parent, &["rev-parse", "HEAD"]), base);
            assert_eq!(git(&parent, &["branch", "--show-current"]), "main");
        }
        assert_eq!(git(&child, &["branch", "--show-current"]), "scoped-topic");
        // The project branch popup loads both roots without changing the
        // main repository, then keeps the repository node in keyboard order.
        review.update(cx, |s, cx| {
            s.project_roots = cx.new(|cx| {
                super::super::project_roots::ProjectRoots::new(
                    vec![Repository::new(&parent), Repository::new(&child)],
                    cx,
                )
            });
            s.bind_project_roots(cx);
            s.branch_popup.open = true;
        });
        cx.run_until_parked();
        review.update(cx, |s, cx| s.load_repository_recents(cx));
        cx.run_until_parked();
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                assert_eq!(s.branch_popup.project.as_ref().unwrap().roots.len(), 2);
                assert_eq!(s.repo.path, parent);
                s.branch_popup.selection =
                    Some(plico::branch_search::Selection::Action("revision".into()));
                s.repository_visible_move(plico::branch_navigation::Move::Next, cx);
                assert!(matches!(
                    s.branch_popup.selection,
                    Some(plico::branch_search::Selection::Repository(_))
                ));
                let selected = plico::branch_search::Selection::Repository(child.clone());
                s.branch_popup.selection = Some(selected.clone());
                s.branch_popup
                    .input
                    .update(cx, |input, cx| input.focus(window, cx));
                s.repository_activate_selection(false, window, cx);
                assert_eq!(s.branch_popup.child_selection, Some(selected));
            })
        });
        for _ in 0..3 {
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
            cx.run_until_parked();
        }
        // Use the same hit test as the parent popover's outside-click handler.
        // This tests painted coordinates rather than directly invoking an action.
        let viewport = cx.update(|window, _| window.bounds().size);
        let origin = review.read_with(cx, |s, cx| {
            let menu = s
                .branch_popup
                .child_menu
                .as_ref()
                .and_then(|m| m.upgrade())
                .expect("child menu exists");
            let menu = menu.read(cx);
            (0..f32::from(viewport.height) as i32)
                .flat_map(|y| {
                    (0..f32::from(viewport.width) as i32)
                        .map(move |x| gpui::point(px(x as f32), px(y as f32)))
                })
                .find(|point| menu.contains_position(point))
                .expect("child menu has painted bounds")
        });
        review.read_with(cx, |s, cx| {
            let menu = s
                .branch_popup
                .child_menu
                .as_ref()
                .unwrap()
                .upgrade()
                .unwrap();
            let menu = menu.read(cx);
            assert!(
                menu.contains_position(&(origin + gpui::point(px(373.), px(310.)))),
                "repository navigation keeps the Rebased minimum tree area"
            );
        });
        // Group activation stays in this menu. Horizontal navigation restores
        // the reference rows without losing the repository popup or focus.
        cx.simulate_click(origin + gpui::point(px(50.), px(72.)), Default::default());
        cx.run_until_parked();
        for key in ["right", "enter", "right"] {
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
            review.read_with(cx, |s, _| {
                assert!(
                    s.branch_popup.open,
                    "group navigation keeps the parent open"
                );
                assert!(s.branch_popup.child_selection.is_some());
            });
        }
        // Rebased overrides Left in nested repository popups: close the
        // whole child even when an expanded group is selected.
        cx.simulate_keystrokes("left");
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.branch_popup.open);
            assert!(s.branch_popup.child_selection.is_none());
        });
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.branch_popup.selection =
                    Some(plico::branch_search::Selection::Repository(child.clone()));
                s.branch_popup
                    .input
                    .update(cx, |input, cx| input.focus(window, cx));
                s.repository_activate_selection(false, window, cx);
            });
        });
        for _ in 0..3 {
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
            cx.run_until_parked();
        }
        // The first reference row owns a submenu, not an action. A click on
        // it must not bubble into the parent repository dropdown's trigger.
        let reference = origin + gpui::point(px(50.), px(98.));
        cx.simulate_click(reference, Default::default());
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        review.read_with(cx, |s, cx| {
            let menu = s
                .branch_popup
                .child_menu
                .as_ref()
                .and_then(|m| m.upgrade())
                .unwrap();
            let menu = menu.read(cx);
            assert!(
                (0..f32::from(viewport.width) as i32).any(|x| {
                    let point = gpui::point(px(x as f32), reference.y);
                    !menu.contains_position(&point) && menu.contains_menu_position(&point, cx)
                }),
                "a reference click must expand its submenu without requiring a prior hover"
            );
        });
        cx.update(|window, cx| {
            review.read_with(cx, |s, cx| {
                let menu = s
                    .branch_popup
                    .child_menu
                    .as_ref()
                    .and_then(|m| m.upgrade())
                    .unwrap();
                assert!(
                    !menu.focus_handle(cx).is_focused(window),
                    "clicking a submenu must transfer keyboard focus to its actions"
                );
            });
        });
        cx.simulate_click(reference, Default::default());
        review.read_with(cx, |s, _| {
            assert!(
                s.branch_popup.open,
                "clicking a reference submenu must keep the branch popup open"
            );
            assert!(
                s.branch_popup.child_selection.is_some(),
                "reference click must retain the repository menu"
            );
        });
        let revision = origin + gpui::point(px(50.), px(44.));
        cx.simulate_mouse_move(revision, None, Default::default());
        cx.simulate_click(revision, Default::default());
        cx.update(|window, cx| {
            assert!(
                window.has_active_dialog(cx),
                "child revision menu click must open input dialog"
            );
            window.close_dialog(cx);
            review.update(cx, |s, cx| {
                s.dismiss_repository_menu(cx);
                assert!(s.branch_popup.project.is_none());
                s.branch_popup.open = true;
                s.load_repository_recents(cx);
                s.dismiss_repository_menu(cx);
            })
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(
                s.branch_popup.project.is_none(),
                "dismissed popup must reject late metadata"
            );
            assert_eq!(s.repo.path, parent);
        });
        // Ordinary local checkout routes to the child, despite a shared base OID.
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.start_log_checkout(
                    Repository::new(&child),
                    base.clone(),
                    Some("refs/heads/base".into()),
                    window,
                    cx,
                )
            })
        });
        cx.run_until_parked();
        assert_eq!(git(&child, &["branch", "--show-current"]), "base");
        // A local edit blocks revision checkout and opens the existing recovery UI.
        std::fs::write(child.join("file"), "draft\n").unwrap();
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.start_log_checkout(Repository::new(&child), head.clone(), None, window, cx)
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
            window.close_dialog(cx);
            review.update(cx, |s, cx| {
                s.start_checkout_request(
                    Recovery::Checkout {
                        repository: Repository::new(&child),
                        target: head.clone(),
                        head: base.clone(),
                        reference: None,
                    },
                    true,
                    window,
                    cx,
                )
            });
        });
        cx.run_until_parked();
        assert_eq!(git(&child, &["rev-parse", "HEAD"]), head);
        assert_eq!(git(&child, &["branch", "--show-current"]), "");
        // The conflicting draft remains recoverable in the child's stash only.
        assert!(!git(&child, &["stash", "list"]).is_empty());
        assert!(!git(&child, &["ls-files", "-u"]).is_empty());
        assert_eq!(git(&parent, &["rev-parse", "HEAD"]), base);
        assert_eq!(git(&parent, &["branch", "--show-current"]), "main");
        assert!(git(&parent, &["status", "--porcelain"]).is_empty());
        assert!(git(&parent, &["stash", "list"]).is_empty());
        review.read_with(cx, |s, _| {
            assert_eq!(s.repo.path, parent);
            assert!(!s.operation.busy());
            assert!(!s.operation_state.as_ref().unwrap().detached);
        });
        // Repeat the overwrite recovery through the reference checkout path.
        git(&child, &["reset", "--hard", &base]);
        git(&child, &["stash", "clear"]);
        std::fs::write(child.join("file"), "reference draft\n").unwrap();
        let prepared = plico::reference_checkout::Prepared::read(
            &Repository::new(&child),
            "refs/tags/not-a-branch",
            &head,
            &base,
            None,
            &Cancel::default(),
        )
        .unwrap();
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.prepare_reference_checkout_in(
                    Repository::new(&child),
                    prepared.reference.clone(),
                    head.clone(),
                    base.clone(),
                    None,
                    window,
                    cx,
                );
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(window.has_active_dialog(cx));
            window.close_dialog(cx);
            review.update(cx, |s, cx| {
                s.start_checkout_request(
                    Recovery::Reference {
                        prepared,
                        choice: plico::reference_checkout::Choice::Checkout,
                    },
                    true,
                    window,
                    cx,
                )
            });
        });
        cx.run_until_parked();
        assert_eq!(git(&child, &["rev-parse", "HEAD"]), head);
        assert!(!git(&child, &["stash", "list"]).is_empty());
        assert!(!git(&child, &["ls-files", "-u"]).is_empty());
        assert!(git(&parent, &["status", "--porcelain"]).is_empty());
        assert!(git(&parent, &["stash", "list"]).is_empty());
        review.read_with(cx, |s, _| {
            assert_eq!(s.repo.path, parent);
            assert!(!s.operation_state.as_ref().unwrap().detached);
        });
    }
}
