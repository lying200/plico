//! Log tag prompt: Rebased GitCreateTagAction's single-name dialog.
use super::*;
struct TagName {
    input: Entity<InputState>,
    error: Option<String>,
    busy: bool,
    closed: bool,
    token: Option<Cancel>,
    _subscription: Subscription,
}
impl Render for TagName {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .child(t("Enter the name of new tag"))
            .child(
                Input::new(&self.input)
                    .external_focus_ring(false)
                    .disabled(self.busy)
                    .w_full(),
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
    pub(super) fn new_tag_dialog(
        &mut self,
        repository: Repository,
        target: String,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy()
            || self.operation.saving_documents()
            || !plico::git::is_oid(&target)
        {
            return;
        }
        let draft = cx.new(|cx| {
            let input = cx.new(|cx| InputState::new(w, cx));
            let subscription =
                cx.subscribe(&input, |s: &mut TagName, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        s.error = None;
                        cx.notify();
                    }
                });
            TagName {
                input,
                error: None,
                busy: false,
                closed: false,
                token: None,
                _subscription: subscription,
            }
        });
        let focus = draft.read(cx).input.read(cx).focus_handle(cx);
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        w.open_dialog(cx, move |dialog, _, cx| {
            let close = draft.clone();
            let confirm = draft.clone();
            let owner = owner.clone();
            let path = path.clone();
            let target = target.clone();
            let repository = repository.clone();
            let name = draft.read(cx).input.read(cx).value();
            dialog
                .title(format!("{} {}", t("Create New Tag On"), &target[..8]))
                .w(px(460.))
                .centered(true)
                .overlay_closable(false)
                .child(draft.clone())
                .footer(crate::dialog_style::confirmation(
                    Button::new("create-tag").primary().label(t("OK")).disabled(
                        draft.read(cx).busy
                            || name.is_empty()
                            || name.chars().any(char::is_whitespace),
                    ),
                ))
                .on_ok(move |_, w, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path != path || confirm.read(cx).busy || confirm.read(cx).closed {
                            return;
                        }
                        let name = confirm.read(cx).input.read(cx).value().to_string();
                        s.create_tag(
                            repository.clone(),
                            target.clone(),
                            name,
                            confirm.clone(),
                            w,
                            cx,
                        );
                    });
                    false
                })
                .on_close(move |_, _, cx| {
                    close.update(cx, |s, _| {
                        s.closed = true;
                        if let Some(token) = &s.token {
                            token.cancel();
                        }
                    });
                })
        });
        focus.focus(w, cx);
    }
    fn create_tag(
        &mut self,
        repo: Repository,
        target: String,
        name: String,
        draft: Entity<TagName>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Tag,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        draft.update(cx, |s, cx| {
            s.busy = true;
            s.error = None;
            s.token = Some(token.clone());
            cx.notify();
        });
        self.status_message = Some(t("Creating tag…").into());
        // The project path guards this window; the captured repository owns the write.
        let path = self.repo.path.clone();
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    let prepared =
                        plico::tag_create::Prepared::read(&repo, &name, &target, &token)?;
                    let outcome =
                        plico::tag_create::execute(&repo, &prepared, &token, &Default::default())?;
                    if outcome.status.success()
                        && outcome.stopped.is_none()
                        && outcome.input_error.is_none()
                    {
                        Ok(())
                    } else {
                        Err(String::from_utf8_lossy(&outcome.stderr.bytes()).into_owned())
                    }
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if s.repo.path != path {
                    return;
                }
                let error = if cancelled {
                    Some(t("Operation cancelled; repository refreshed").to_owned())
                } else {
                    result.err()
                };
                draft.update(cx, |d, cx| {
                    d.busy = false;
                    d.error = error.clone();
                    cx.notify();
                });
                if error.is_none() {
                    s.status_message = Some(t("Tag created").into());
                    if !draft.read(cx).closed {
                        w.close_dialog(cx);
                    }
                } else {
                    s.status_message = Some(t("Could not create tag").into());
                }
                s.refresh_impl(false, true, None, cx);
                cx.notify();
            });
        });
    }
}

impl Review {
    pub(super) fn tag_push_menu(
        menu: gpui_kit::component::menu::PopupMenu,
        owner: &WeakEntity<Self>,
        reference: &str,
        window: &mut Window,
        cx: &mut Context<gpui_kit::component::menu::PopupMenu>,
    ) -> gpui_kit::component::menu::PopupMenu {
        let Some(view) = owner.upgrade() else {
            return menu;
        };
        let state = view.read(cx);
        let remotes = state.branch_popup.push_remotes.clone();
        let disabled = state.operation.busy() || state.operation.saving_documents();
        let owner = owner.clone();
        let reference = reference.to_owned();
        let items = move |mut menu: gpui_kit::component::menu::PopupMenu| {
            for remote in &remotes {
                let owner = owner.clone();
                let reference = reference.clone();
                let remote = remote.clone();
                menu = menu.item(
                    PopupMenuItem::new(format!("{} {remote}", t("Push to")))
                        .disabled(disabled)
                        .on_click(move |_, w, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                s.push_tag(reference.clone(), remote.clone(), w, cx)
                            });
                        }),
                );
            }
            menu
        };
        if state.branch_popup.push_remotes.len() >= 6 {
            menu.submenu(t("Push Tag"), window, cx, move |menu, _, _| items(menu))
        } else {
            items(menu)
        }
    }

    fn push_tag(
        &mut self,
        reference: String,
        remote: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let helper = exe.with_file_name(if cfg!(windows) {
            "plico-askpass.exe"
        } else {
            "plico-askpass"
        });
        if !helper.is_file() {
            self.status_message = Some(t("Authentication helper is unavailable").into());
            cx.notify();
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Push,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.dismiss_repository_menu(cx);
        self.status_message = Some(t("Pushing…").into());
        let requests = plico::auth_request::Requests::persistent(token.clone());
        requests.set_project(&self.repo.path);
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = plico::tag_push::Prepared::read(&repo, &reference, &remote, &token)
                .and_then(|prepared| {
                    plico::tag_push::execute(
                        &repo,
                        &prepared,
                        &token,
                        &Default::default(),
                        Some((&worker_requests, &helper)),
                    )
                });
            let _ = tx.send(result);
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            loop {
                if let Some(pending) = requests.take_pending() {
                    match wx.update(|w, cx| crate::auth_dialog::show(pending, w, cx)) {
                        Ok(dialog) => login = Some(dialog),
                        Err(_) => break,
                    }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("Push worker ended".into())),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = this.update_in(wx, |s, w, cx| {
                        let Some(cancelled) = s.finish_operation(operation, cx) else { return; };
                        if login.as_ref().is_some_and(|d| d.read(cx).active()) { w.close_dialog(cx); }
                        use plico::push::Status;
                        let message = if cancelled { "Push cancelled; remote changes may have completed" } else {
                            match result {
                                Ok(r) if r.state.is_ok() => match r.status {
                                    Status::Success | Status::NewBranch | Status::Forced => "Push completed",
                                    Status::UpToDate => "Already up to date",
                                    Status::RejectedOther => "Push rejected by remote",
                                    Status::RejectedNonFf => "Push rejected: remote history has changed",
                                    Status::RejectedStale => "Push rejected: stale remote information",
                                    Status::Cancelled => "Push cancelled; remote changes may have completed",
                                    _ => "Push failed or incomplete; some remote updates may have completed",
                                },
                                _ => "Push failed or incomplete; some remote updates may have completed",
                            }
                        };
                        s.status_message = Some(t(message).into());
                        s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                        s.refresh_impl(false, true, None, cx);
                        cx.notify();
                    });
                    break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
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
    fn creating_tag_in_child_preserves_project_and_other_repository(cx: &mut gpui::TestAppContext) {
        use gpui::component::Root;
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn git(path: &std::path::Path, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = format!("plico-child-tag-{}-{nonce}", std::process::id());
        let fixture = Fixture(std::env::temp_dir().join(directory));
        let parent = fixture.0.join("parent");
        let child = fixture.0.join("child");
        std::fs::create_dir_all(&parent).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.hooksPath", "disabled-hooks"],
            vec!["commit", "--allow-empty", "-qm", "shared commit"],
        ] {
            git(&parent, &args);
        }
        git(
            &parent,
            &["clone", "-q", "--no-local", ".", child.to_str().unwrap()],
        );
        // Both repositories have the target object: using the wrong root would
        // succeed, so the test must verify the ref in each repository.
        let target = git(&parent, &["rev-parse", "HEAD"]);
        std::fs::write(child.join("draft.txt"), "unsaved work on disk").unwrap();
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(parent.clone(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        let mut tag_draft = None;
        cx.update(|window, cx| {
            let draft = cx.new(|cx| {
                let input = cx.new(|cx| InputState::new(window, cx));
                let subscription = cx.subscribe(&input, |_: &mut TagName, _, _: &InputEvent, _| {});
                TagName {
                    input,
                    error: None,
                    busy: false,
                    closed: true,
                    token: None,
                    _subscription: subscription,
                }
            });
            tag_draft = Some(draft.clone());
            review.update(cx, |s, cx| {
                s.create_tag(
                    Repository::new(&child),
                    target.clone(),
                    "child-tag".into(),
                    draft,
                    window,
                    cx,
                )
            });
        });
        cx.run_until_parked();
        tag_draft.unwrap().read_with(cx, |draft, _| {
            assert!(!draft.busy);
            assert!(draft.error.is_none(), "{:?}", draft.error);
        });
        review.read_with(cx, |s, _| {
            assert_eq!(s.repo.path, parent);
            assert!(!s.operation.busy());
        });
        assert_eq!(git(&child, &["rev-parse", "refs/tags/child-tag"]), target);
        assert!(git(&parent, &["tag", "--list", "child-tag"]).is_empty());
        assert_eq!(git(&parent, &["rev-parse", "HEAD"]), target);
        assert_eq!(git(&child, &["rev-parse", "HEAD"]), target);
        assert_eq!(
            std::fs::read_to_string(child.join("draft.txt")).unwrap(),
            "unsaved work on disk"
        );
    }
}
