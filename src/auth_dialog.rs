use gpui_kit::component::Disableable;
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
    username: Entity<InputState>,
    password: Entity<InputState>,
    needs_username: bool,
    username_error: bool,
    password_error: bool,
    remember: bool,
    use_helper: bool,
    helper_available: bool,
    storage_error: bool,
}
impl Login {
    pub fn active(&self) -> bool {
        self.pending.is_some()
    }
    fn new(pending: Pending, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let needs_username = pending.kind == Kind::HttpUsername;
        let suggested = pending.suggested_username().unwrap_or_default();
        let helper_available = pending.helper_available();
        let storage_error = pending.storage_error();
        let remember = pending.remember_default();
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
            username,
            password,
            needs_username,
            username_error: false,
            password_error: false,
            remember,
            use_helper: false,
            helper_available,
            storage_error,
        }
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
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
        self.password_error = password.trim().is_empty();
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
    }
}
pub fn show(pending: Pending, window: &mut Window, cx: &mut App) -> Entity<Login> {
    // Raw prompts may contain URL credentials; only show a sanitized target.
    let target = pending
        .http_target()
        .unwrap_or_else(|| t("Git authentication").into());
    let login = cx.new(|cx| Login::new(pending, window, cx));
    let retained = login.clone();
    window.open_dialog(cx, move |dialog, _, _| {
        let confirm = login.clone();
        let close = login.clone();
        dialog
            .title(format!("{} {target}", t("Log in to")))
            .w(px(460.))
            .overlay_closable(false)
            .centered(true)
            .child(login.clone())
            .footer(crate::dialog_style::confirmation(
                Button::new("auth-login").primary().label(t("Log In")),
            ))
            .on_ok(move |_, window, cx| confirm.update(cx, |s, cx| s.confirm(window, cx)))
            .on_close(move |_, _, cx| {
                close.update(cx, |s, _| {
                    s.pending.take();
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
