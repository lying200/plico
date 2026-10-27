//! Main-window saved changes coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn restore_stash(
        &mut self,
        entry: plico::stash::project::SavedEntry,
        action: plico::stash_operation::Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.restore_stash(entry, action, w, cx)
            });
            return;
        }
        if self.operation.busy() || self.settings_open || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Restore,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Restoring saved changes…").into());
        let repo = entry.repository.clone();
        let target_path = repo.path.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::stash_operation::execute(
                        &repo,
                        &entry,
                        action,
                        false,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(operation, cx) else {
                    return;
                };
                use plico::stash_operation::Status;
                let resolve_conflicts = !cancelled
                    && result
                        .as_ref()
                        .is_ok_and(|report| report.status == Status::Conflicts);
                let (message, diagnostic) = match result {
                    Ok(report) => {
                        let message = match report.status {
                            Status::Success if report.recovery_error.is_some() => {
                                "Saved changes restored; recovery record needs attention"
                            }
                            Status::Success => "Saved changes restored",
                            Status::Conflicts => {
                                "Saved changes have conflicts; review Local Changes"
                            }
                            Status::IndexConflict => {
                                "Could not restore the saved index; review Local Changes"
                            }
                            Status::LocalChanges => {
                                "Local changes would be overwritten; stash kept"
                            }
                            Status::UntrackedFiles => {
                                "Untracked files would be overwritten; stash kept"
                            }
                            Status::Cancelled => {
                                "Restore cancelled; check Local Changes before retrying"
                            }
                            Status::Failed => "Restore failed or incomplete; review Local Changes",
                        };
                        let diagnostic = if report.status == Status::Success {
                            report.recovery_error
                        } else {
                            Some(match report.command {
                                Ok(output) => format!(
                                    "{}\n{}",
                                    String::from_utf8_lossy(&output.stderr.bytes()),
                                    String::from_utf8_lossy(&output.stdout.bytes())
                                )
                                .chars()
                                .take(8000)
                                .collect(),
                                Err(error) => error,
                            })
                        };
                        if s.repo.path == target_path {
                            s.operation_state = report.state.ok();
                        }
                        (message, diagnostic)
                    }
                    Err(error) => ("Could not restore saved changes", Some(error)),
                };
                s.status_message = Some(t(message).into());
                if let Some(view) = &s.stash_view {
                    view.update(cx, |v, cx| {
                        v.result(
                            Repository::new(&target_path),
                            t(message).into(),
                            diagnostic,
                            cx,
                        )
                    });
                }
                s.refresh(false, cx);
                if resolve_conflicts {
                    if s.repo.path == target_path {
                        s.open_conflicts(w, cx);
                    } else {
                        s.choose_project_window(target_path.clone(), w, cx);
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    pub(super) fn confirm_external_recovery(
        &mut self,
        repo: Repository,
        pending: plico::rebase_saved::Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
            return;
        }
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone(); let path = path.clone(); let pending = pending.clone(); let repo = repo.clone();
            dialog.title(t("Finish Rebase recovery")).w(px(510.)).centered(true).overlay_closable(false)
                .child(div().child(t("Confirm only after handling the saved changes externally. This clears the app recovery record; it does not restore or delete any stash or change your files.")))
                .child(div().truncate().child(path.display().to_string()))
                .footer(gpui_kit::component::dialog::DialogFooter::new()
                    .child(Button::new("acknowledge-recovery-confirm").min_w(px(72.)).label(t("Clear recovery record"))
                        .on_click(move |_, w, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                if s.repo.path == path && !s.operation.busy() { s.acknowledge_external_recovery(repo.clone(), pending.clone(), cx); }
                            }); w.close_dialog(cx);
                        }))
                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.primary().min_w(px(72.)).label(t("Cancel")))))
                // The dialog-level Enter action is cancellation; only the explicit button clears a record.
                .on_ok(|_, _, _| true)
        });
    }
    pub(super) fn acknowledge_external_recovery(
        &mut self,
        repo: Repository,
        pending: plico::rebase_saved::Pending,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Clearing recovery record…").into());
        let target_path = repo.path.clone();
        let path = self.repo.path.clone();
        self.operation_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    plico::rebase_saved::acknowledge_external_recovery(&repo, &pending, &token)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.repo.path != path {
                    return;
                }
                if s.finish_operation(operation, cx).is_none() {
                    return;
                }
                let message = t(if result.is_ok() {
                    "Recovery record cleared; saved changes kept"
                } else {
                    "Recovery record was not cleared. Refresh and review the repository state."
                });
                s.status_message = Some(message.into());
                if let Some(view) = &s.stash_view {
                    view.update(cx, |v, cx| {
                        v.result(Repository::new(&target_path), message.into(), None, cx)
                    });
                }
                s.rebase_diagnostic = result.err();
                cx.notify();
            });
        });
        cx.notify();
    }
}

impl Review {
    pub(super) fn confirm_drop_stash(
        &mut self,
        entry: plico::stash::project::SavedEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let path = path.clone();
            let entry = entry.clone();
            dialog
                .title(t("Drop Stash"))
                .w(px(480.))
                .centered(true)
                .overlay_closable(false)
                .child(div().child(t(
                    "Delete this saved stash? Its changes will not be restored.",
                )))
                .child(
                    div()
                        .truncate()
                        .child(entry.repository.path.display().to_string()),
                )
                .child(div().child(format!("{}: {}", entry.selector, entry.message)))
                .footer(crate::dialog_style::confirmation(
                    Button::new("drop-stash-confirm")
                        .primary()
                        .label(t("Drop Stash")),
                ))
                .on_ok(move |_, _, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path {
                            s.drop_stash(entry.clone(), cx);
                        }
                    });
                    true
                })
        });
    }
    fn drop_stash(&mut self, entry: plico::stash::project::SavedEntry, cx: &mut Context<Self>) {
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = entry.repository.clone();
        let target_path = repo.path.clone();
        self.status_message = Some(t("Dropping stash…").into());
        self.operation_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    plico::stash_operation::drop_stash(&repo, &entry, &token, &Default::default())
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.finish_operation(operation, cx).is_none() {
                    return;
                }
                let (message, diagnostic) = match result {
                    Ok(out) if out.status.success() && out.stopped.is_none() => {
                        ("Stash deleted", None)
                    }
                    Ok(out) => (
                        "Could not delete stash",
                        Some(String::from_utf8_lossy(&out.stderr.bytes()).into_owned()),
                    ),
                    Err(error) => ("Could not delete stash", Some(error)),
                };
                s.status_message = Some(t(message).into());
                if let Some(view) = &s.stash_view {
                    view.update(cx, |v, cx| {
                        v.result(
                            Repository::new(&target_path),
                            t(message).into(),
                            diagnostic,
                            cx,
                        )
                    });
                }
                s.refresh(false, cx);
                cx.notify();
            });
        });
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tab_tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;
    #[gpui::test]
    fn commit_and_stash_tabs_reuse_views_and_remember_hidden_selection(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = review.unwrap();
        cx.update(|w, cx| {
            view.update(cx, |s, cx| {
                s.open_local(w, cx);
                let local = s.local_view.as_ref().unwrap().entity_id();
                s.open_stashes(w, cx);
                let stash = s.stash_view.as_ref().unwrap().entity_id();
                assert!(s.stash_open && !s.local_visible && s.commit_stash_selected);
                s.toggle_commit_window(w, cx);
                assert!(!s.local_visible && !s.stash_open);
                s.toggle_commit_window(w, cx);
                assert!(s.stash_open);
                s.open_local(w, cx);
                assert!(s.local_visible && !s.stash_open && !s.commit_stash_selected);
                assert_eq!(s.local_view.as_ref().unwrap().entity_id(), local);
                s.open_stashes(w, cx);
                assert_eq!(s.stash_view.as_ref().unwrap().entity_id(), stash);
                s.toggle_project(w, cx);
                assert!(!s.stash_open && s.project_visible);
                s.toggle_commit_window(w, cx);
                assert!(s.stash_open && !s.project_visible);
            })
        });
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod repository_routing_tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use std::{fs, path::Path, process::Command};

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }
    #[gpui::test]
    fn child_stash_apply_and_drop_do_not_use_the_parent_repository(cx: &mut gpui::TestAppContext) {
        let fixture = Fixture(
            std::env::temp_dir().join(format!("plico-stash-ui-routing-{}", std::process::id())),
        );
        fs::create_dir(&fixture.0).unwrap();
        let child_path = fixture.0.join("child");
        fs::create_dir(&child_path).unwrap();
        for root in [&fixture.0, &child_path] {
            git(root, &["init", "-q", "-b", "main"]);
            for (key, value) in [
                ("user.name", "Test"),
                ("user.email", "test@example.invalid"),
                ("commit.gpgsign", "false"),
                ("core.hooksPath", "disabled-hooks"),
            ] {
                git(root, &["config", key, value]);
            }
            fs::write(root.join("file.txt"), "base\n").unwrap();
            git(root, &["add", "file.txt"]);
            git(root, &["commit", "-qm", "base"]);
        }
        fs::write(fixture.0.join("file.txt"), "parent saved\n").unwrap();
        git(
            &fixture.0,
            &["stash", "push", "-m", "parent", "--", "file.txt"],
        );
        fs::write(child_path.join("file.txt"), "child saved\n").unwrap();
        git(
            &child_path,
            &["stash", "push", "-m", "child", "--", "file.txt"],
        );
        let child = Repository::new(&child_path);
        let saved = plico::stash::project::SavedEntry {
            repository: child.clone(),
            entry: plico::stash::list(&child, 1, &Cancel::default())
                .unwrap()
                .entries
                .remove(0),
        };
        let parent_stash = git(&fixture.0, &["rev-parse", "refs/stash"]);
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(fixture.0.clone(), 100, None, w, cx));
            review = Some(view.clone());
            gpui::component::Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.restore_stash(saved.clone(), plico::stash_operation::Action::Apply, w, cx)
            })
        });
        cx.run_until_parked();
        assert_eq!(
            fs::read(child_path.join("file.txt")).unwrap(),
            b"child saved\n"
        );
        assert_eq!(fs::read(fixture.0.join("file.txt")).unwrap(), b"base\n");
        assert_eq!(git(&fixture.0, &["rev-parse", "refs/stash"]), parent_stash);
        cx.update(|_, cx| review.update(cx, |s, cx| s.drop_stash(saved.clone(), cx)));
        cx.run_until_parked();
        assert!(git(&child_path, &["stash", "list"]).is_empty());
        assert_eq!(git(&fixture.0, &["rev-parse", "refs/stash"]), parent_stash);
        review.read_with(cx, |s, _| assert!(!s.operation.busy()));
    }
}
