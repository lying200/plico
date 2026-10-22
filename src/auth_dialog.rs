use gpui_kit::component::{Disableable, Selectable as _};
use gpui_kit::component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputContentType, InputEvent, InputState},
    radio::Radio,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    auth_request::{Kind, Pending},
    i18n::t,
};
const LABEL_WIDTH: f32 = 125.;
pub struct Login {
    pending: Option<Pending>,
    ssh: Option<plico::ssh_auth::Prompt>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    needs_username: bool,
    username_error: bool,
    password_error: bool,
    remember: bool,
    use_helper: bool,
    helper_available: bool,
    storage_error: bool,
    accounts: Vec<plico::hosting::Account>,
    selected_account: usize,
    hosted: bool,
    project_default: bool,
    busy: bool,
    task: Task<()>,
    login_panel: Option<Entity<crate::hosting_panel::Panel>>,
    hosted_error: Option<String>,
}
impl Login {
    pub fn active(&self) -> bool {
        self.pending.is_some()
    }
    fn new(pending: Pending, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ssh =
            (pending.kind == Kind::Ssh).then(|| plico::ssh_auth::Prompt::parse(&pending.prompt));
        let needs_username = pending.kind == Kind::HttpUsername;
        let suggested = pending.suggested_username().unwrap_or_default();
        let helper_available = pending.helper_available();
        let storage_error = pending.storage_error();
        let remember = pending.remember_default();
        let accounts = pending.hosted_accounts();
        let hosted = !accounts.is_empty();
        let username = cx.new(|cx| InputState::new(window, cx).default_value(suggested));
        let password = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_masked(true, window, cx);
            state
        });
        cx.subscribe(&username, |s, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) && s.username_error {
                s.username_error = false;
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&password, |s, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) && s.password_error {
                s.password_error = false;
                cx.notify();
            }
        })
        .detach();
        Self {
            pending: Some(pending),
            ssh,
            username,
            password,
            needs_username,
            username_error: false,
            password_error: false,
            remember,
            use_helper: false,
            helper_available,
            storage_error,
            accounts,
            selected_account: 0,
            hosted,
            project_default: false,
            busy: false,
            task: Task::ready(()),
            login_panel: None,
            hosted_error: None,
        }
    }
    fn finish_hosted(
        &mut self,
        account: plico::hosting::Account,
        credentials: plico::hosting::Credentials,
        save: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending.as_ref().is_some_and(|p| {
            p.prepare_hosted(&account, &credentials, self.project_default, save)
                .is_err()
        }) {
            self.hosted_error = Some(t("Credential storage is unavailable").into());
            cx.notify();
            return;
        }
        if let Some(pending) = self.pending.take() {
            let _ = pending.answer_hosted(account, credentials);
        }
        self.login_panel = None;
        w.close_dialog(cx);
    }
    fn open_hosted_login(
        &mut self,
        provider: plico::hosting::Provider,
        account: Option<plico::hosting::Account>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hosted = true;
        let panel = crate::hosting_panel::Panel::account_login(provider, account, false, w, cx);
        cx.subscribe_in(
            &panel,
            w,
            |s, panel, _: &crate::hosting_panel::LoggedIn, w, cx| {
                if let Some((account, credentials)) = panel.update(cx, |p, _| p.take_login()) {
                    s.finish_hosted(account, credentials, true, w, cx);
                }
            },
        )
        .detach();
        self.login_panel = Some(panel);
        cx.notify();
    }
    fn start_hosted(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.login_panel.is_some() {
            return;
        }
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        let Some(account) = self.accounts.get(self.selected_account).cloned() else {
            return;
        };
        let Some(target) = pending.http_target() else {
            return;
        };
        let cancel = pending.cancel_token();
        self.busy = true;
        let worker_account = account.clone();
        let job = cx.background_spawn(async move {
            plico::hosting::git_account_credentials(&worker_account, &target, &cancel)
        });
        self.task = cx.spawn_in(w, async move |this, wx| {
            let result = job.await;
            let _ = this.update_in(wx, |s, w, cx| {
                s.busy = false;
                if s.pending.as_ref().is_none_or(|p| p.is_cancelled()) {
                    return;
                }
                match result {
                    Ok(credentials) => s.finish_hosted(account, credentials, false, w, cx),
                    Err(_) => {
                        s.open_hosted_login(account.provider, Some(account), w, cx);
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.hosted {
            self.start_hosted(window, cx);
            return false;
        }
        if self.ssh.as_ref().is_some_and(|p| p.confirmation()) {
            if let Some(pending) = self.pending.take() {
                let _ = pending.answer("yes".into());
            }
            return true;
        }
        if self.use_helper {
            if self
                .pending
                .as_ref()
                .is_some_and(|p| p.select_helper().is_err())
            {
                self.storage_error = true;
                cx.notify();
                return false;
            }
            if let Some(pending) = self.pending.take() {
                let _ = pending.answer(String::new());
            }
            self.password
                .update(cx, |s, cx| s.set_value("", window, cx));
            return true;
        }
        let username = self.username.read(cx).value().to_string();
        let password = self.password.read(cx).value().to_string();
        self.username_error = self.needs_username && username.trim().is_empty();
        // SSH keyboard-interactive prompts may legitimately request an empty
        // answer. Let SSH validate it, matching the native password dialog.
        self.password_error = self.ssh.is_none() && password.trim().is_empty();
        if self.username_error || self.password_error {
            cx.notify();
            return false;
        }
        if let Some(pending) = self.pending.as_ref() {
            if pending
                .save_credentials(&username, &password, self.remember)
                .is_err()
            {
                self.storage_error = true;
                cx.notify();
                return false;
            }
        }
        if let Some(pending) = self.pending.take() {
            let _ = if self.needs_username {
                pending.answer_credentials(username, password)
            } else {
                pending.answer(password)
            };
        }
        self.username
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.password
            .update(cx, |s, cx| s.set_value("", window, cx));
        true
    }
}
impl Render for Login {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.hosted {
            if let Some(panel) = &self.login_panel {
                return v_flex()
                    .min_h(px(230.))
                    .child(panel.clone())
                    .child(
                        Button::new("hosted-other-method")
                            .ghost()
                            .label(t("Use other credentials…"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.login_panel = None;
                                s.hosted = false;
                                cx.notify();
                            })),
                    )
                    .when_some(self.hosted_error.clone(), |v, error| v.child(error))
                    .into_any_element();
            }
            return v_flex()
                .gap_3()
                .child(v_flex().border_1().border_color(rgb(0x34363a)).children(
                    self.accounts.iter().enumerate().map(|(index, account)| {
                        Button::new(("git-account", index))
                            .ghost()
                            .w_full()
                            .selected(index == self.selected_account)
                            .label(account.login.clone())
                            .disabled(self.busy)
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.selected_account = index;
                                cx.notify();
                            }))
                    }),
                ))
                .when(
                    self.pending.as_ref().is_some_and(|p| p.project().is_some()),
                    |v| {
                        v.child(
                            crate::ui_style::checkbox("git-default-account")
                                .label(t("Set as default account for current project"))
                                .checked(self.project_default)
                                .disabled(self.busy)
                                .on_click(cx.listener(|s, checked, _, cx| {
                                    s.project_default = *checked;
                                    cx.notify();
                                })),
                        )
                    },
                )
                .child(
                    Button::new("git-manual-login")
                        .ghost()
                        .label(t("Use other credentials…"))
                        .disabled(self.busy)
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.hosted = false;
                            cx.notify();
                        })),
                )
                .when_some(self.hosted_error.clone(), |v, error| v.child(error))
                .into_any_element();
        }
        if let Some(prompt) = &self.ssh {
            return v_flex().gap_3().min_w_0()
                .child(div().whitespace_normal().child(prompt.message()))
                .when(!prompt.confirmation(), |v| v.child(Input::new(&self.password).external_focus_ring(false).content_type(InputContentType::Password)))
                .when(prompt.rememberable(), |v| v.child(crate::ui_style::checkbox("ssh-remember")
                    .label(t(if cfg!(windows) { "Remember" } else { "Remember for this session" })).checked(self.remember)
                    .on_click(cx.listener(|s, checked, _, cx| { s.remember = *checked; cx.notify(); }))))
                .when(self.storage_error, |v| v.child(t("Credential storage is unavailable. Clear Remember to continue without saving.")))
                .when(self.password_error, |v| v.child(t("Password must not be empty")))
                .into_any_element();
        }
        let field = |label: &'static str, input: Input, error: bool, message: &'static str| {
            h_flex()
                .items_start()
                .gap_3()
                .child(
                    div()
                        .w(px(LABEL_WIDTH))
                        .flex_shrink_0()
                        .pt(px(6.))
                        .child(t(label)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(input)
                        .when(error, |v| {
                            v.child(div().text_color(rgb(0xf2a6a6)).child(t(message)))
                        }),
                )
        };
        v_flex()
            .id("http-auth-fields")
            .capture_key_down(cx.listener(|s, event: &KeyDownEvent, w, cx| {
                if event.keystroke.key == "tab" && s.needs_username && !s.use_helper {
                    let username = s.username.read(cx).focus_handle(cx);
                    let password = s.password.read(cx).focus_handle(cx);
                    if username.is_focused(w) && !event.keystroke.modifiers.shift {
                        password.focus(w, cx);
                        cx.stop_propagation();
                        w.prevent_default();
                    } else if password.is_focused(w) && event.keystroke.modifiers.shift {
                        username.focus(w, cx);
                        cx.stop_propagation();
                        w.prevent_default();
                    }
                }
            }))
            .gap_3()
            .when(!self.helper_available, |v| {
                v.child(t("Enter your credentials"))
            })
            .when(self.helper_available, |v| {
                v.child(
                    Radio::new("auth-enter-credentials")
                        .label(t("Enter your credentials"))
                        .checked(!self.use_helper)
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.use_helper = false;
                            cx.notify();
                        })),
                )
            })
            .child(field(
                "Username",
                Input::new(&self.username)
                    .disabled(!self.needs_username || self.use_helper)
                    .external_focus_ring(false),
                self.username_error,
                "Username must not be empty",
            ))
            .child(field(
                "Password or token",
                Input::new(&self.password)
                    .disabled(self.use_helper)
                    .external_focus_ring(false)
                    .content_type(InputContentType::Password),
                self.password_error,
                "Password must not be empty",
            ))
            .child(
                div().pl(px(LABEL_WIDTH + 12.)).child(
                    crate::ui_style::checkbox("auth-remember")
                        .label(t(if cfg!(windows) {
                            "Remember"
                        } else {
                            "Remember for this session"
                        }))
                        .checked(self.remember)
                        .disabled(self.use_helper)
                        .on_click(cx.listener(|s, checked, _, cx| {
                            s.remember = *checked;
                            cx.notify();
                        })),
                ),
            )
            .when(self.helper_available, |v| {
                v.child(
                    Radio::new("auth-use-helper")
                        .label(t("Use credentials helper"))
                        .checked(self.use_helper)
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.use_helper = true;
                            cx.notify();
                        })),
                )
            })
            .when(self.storage_error, |v| {
                v.child(div().text_color(rgb(0xf2a6a6)).child(t(
                    "Credential storage is unavailable. Clear Remember to continue without saving.",
                )))
            })
            .into_any_element()
    }
}
pub fn show(pending: Pending, window: &mut Window, cx: &mut App) -> Entity<Login> {
    // Raw prompts may contain URL credentials; only show a sanitized target.
    let target = pending
        .http_target()
        .unwrap_or_else(|| t("Git authentication").into());
    let ssh = (pending.kind == Kind::Ssh).then(|| plico::ssh_auth::Prompt::parse(&pending.prompt));
    let accounts = pending.hosted_accounts();
    let title = ssh
        .as_ref()
        .map(|s| t(s.title()).to_owned())
        .unwrap_or_else(|| {
            accounts
                .first()
                .map(|a| format!("{} {}", t("Choose account:"), a.provider.name()))
                .unwrap_or_else(|| format!("{} {target}", t("Log in to")))
        });
    let action = if ssh.as_ref().is_some_and(|s| s.confirmation()) {
        "Yes"
    } else if ssh.is_some() {
        "OK"
    } else {
        "Log In"
    };
    let provider = pending.hosted_provider();
    let expected = provider.and_then(|provider| {
        pending
            .known_username()
            .filter(|login| {
                !login.is_empty()
                    && !(provider == plico::hosting::Provider::GitLab && login == "oauth2")
            })
            .map(|login| plico::hosting::Account {
                provider,
                server: provider.server().into(),
                login,
            })
    });
    let login = cx.new(|cx| Login::new(pending, window, cx));
    if accounts.is_empty() {
        if let Some(provider) = provider {
            login.update(cx, |s, cx| {
                s.open_hosted_login(provider, expected, window, cx)
            });
        }
    }
    let retained = login.clone();
    let confirmation = ssh.as_ref().is_some_and(|s| s.confirmation());
    window.open_dialog(cx, move |dialog, _, cx| {
        let confirm = login.clone();
        let close = login.clone();
        dialog
            .title(
                if ssh.is_none() && (!login.read(cx).hosted || login.read(cx).login_panel.is_some())
                {
                    format!("{} {target}", t("Log in to"))
                } else {
                    title.clone()
                },
            )
            .w(px(460.))
            .overlay_closable(false)
            .centered(true)
            .child(login.clone())
            .footer(if login.read(cx).login_panel.is_some() {
                use gpui_kit::component::dialog::{DialogClose, DialogFooter};
                DialogFooter::new().child(DialogClose::new().trigger(|b| b.label(t("Cancel"))))
            } else if confirmation {
                use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
                DialogFooter::new()
                    .child(
                        DialogAction::new().child(
                            Button::new("auth-login")
                                .primary()
                                .label(t("Yes"))
                                .min_w(px(72.)),
                        ),
                    )
                    .child(
                        DialogClose::new().trigger(|button| button.label(t("No")).min_w(px(72.))),
                    )
            } else {
                crate::dialog_style::confirmation(
                    Button::new("auth-login")
                        .primary()
                        .label(t(action))
                        .loading(login.read(cx).busy),
                )
            })
            .on_ok(move |_, window, cx| confirm.update(cx, |s, cx| s.confirm(window, cx)))
            .on_close(move |_, _, cx| {
                close.update(cx, |s, _| {
                    s.login_panel = None;
                    if let Some(pending) = s.pending.take() {
                        if confirmation {
                            let _ = pending.answer("no".into());
                        }
                    }
                });
            })
    });
    let focus = retained.read(cx);
    if focus.needs_username && focus.username.read(cx).value().is_empty() {
        focus.username.read(cx).focus_handle(cx).focus(window, cx);
    } else {
        focus.password.read(cx).focus_handle(cx).focus(window, cx);
    }
    retained
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::TestAppContext;
    use gpui::component::Root;
    use std::{
        thread,
        time::{Duration, Instant},
    };
    fn request(
        kind: Kind,
        prompt: &str,
    ) -> (
        Pending,
        thread::JoinHandle<Result<String, plico::auth_request::Error>>,
    ) {
        let requests = plico::auth_request::Requests::new(Default::default());
        let worker_requests = requests.clone();
        let prompt = prompt.to_owned();
        let worker = thread::spawn(move || worker_requests.ask(kind, prompt));
        let start = Instant::now();
        loop {
            if let Some(pending) = requests.take_pending() {
                return (pending, worker);
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            thread::sleep(Duration::from_millis(5));
        }
    }
    #[gpui::test]
    fn empty_credentials_keep_request_live_and_submit_clears_fields(cx: &mut TestAppContext) {
        cx.update(gpui::component::init);
        let (pending, worker) = request(Kind::HttpUsername, "Username for 'https://host/repo': ");
        let mut login = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Login::new(pending, w, cx));
            login = Some(view.clone());
            Root::new(view, w, cx)
        });
        let login = login.unwrap();
        cx.update(|w, cx| {
            login.update(cx, |s, cx| {
                assert!(!s.confirm(w, cx));
                assert!(s.active());
                assert!(s.username_error && s.password_error);
                s.username
                    .update(cx, |v, cx| v.set_value("fixture-user", w, cx));
                s.password
                    .update(cx, |v, cx| v.set_value("fixture-token", w, cx));
                assert!(s.confirm(w, cx));
                assert!(!s.active());
                assert!(s.username.read(cx).value().is_empty());
                assert!(s.password.read(cx).value().is_empty());
            })
        });
        assert_eq!(worker.join().unwrap().unwrap(), "fixture-user");
    }
    #[gpui::test]
    fn ssh_interactive_accepts_empty_response_without_http_validation(cx: &mut TestAppContext) {
        cx.update(gpui::component::init);
        let (pending, worker) = request(Kind::Ssh, "Press Enter to continue:");
        let mut login = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Login::new(pending, w, cx));
            login = Some(view.clone());
            Root::new(view, w, cx)
        });
        cx.update(|w, cx| {
            login.unwrap().update(cx, |s, cx| {
                assert!(s.confirm(w, cx));
                assert!(!s.password_error);
            })
        });
        assert_eq!(worker.join().unwrap().unwrap(), "");
    }
    #[gpui::test]
    fn known_login_is_preserved_and_dismissal_cancels_the_worker(cx: &mut TestAppContext) {
        cx.update(gpui::component::init);
        let (pending, worker) = request(
            Kind::HttpPassword,
            "Password for 'https://fixture-user@host/repo': ",
        );
        let mut login = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Login::new(pending, w, cx));
            login = Some(view.clone());
            Root::new(view, w, cx)
        });
        let login = login.unwrap();
        cx.update(|_, cx| {
            login.update(cx, |s, cx| {
                assert!(!s.needs_username);
                assert_eq!(s.username.read(cx).value().as_ref(), "fixture-user");
                s.pending.take();
                assert!(!s.active());
            })
        });
        assert_eq!(
            worker.join().unwrap(),
            Err(plico::auth_request::Error::Cancelled)
        );
    }
}
