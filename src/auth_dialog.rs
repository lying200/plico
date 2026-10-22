use gpui_kit::component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputContentType, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    auth_request::{Kind, Pending},
    i18n::t,
};
pub struct Login {
    pending: Option<Pending>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    needs_username: bool,
    username_error: bool,
    password_error: bool,
}
impl Login {
    pub fn active(&self) -> bool {
        self.pending.is_some()
    }
    fn new(pending: Pending, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let needs_username = pending.kind == Kind::HttpUsername;
        let username = cx.new(|cx| InputState::new(window, cx));
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
        }
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let username = self.username.read(cx).value().to_string();
        let password = self.password.read(cx).value().to_string();
        self.username_error = self.needs_username && username.trim().is_empty();
        self.password_error = password.trim().is_empty();
        if self.username_error || self.password_error {
            cx.notify();
            return false;
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
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let field = |label: &'static str, input: Input, error: bool, message: &'static str| {
            h_flex()
                .items_start()
                .gap_3()
                .child(div().w(px(90.)).flex_shrink_0().pt(px(6.)).child(t(label)))
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
            .gap_3()
            .child(t("Enter your credentials"))
            .when(self.needs_username, |v| {
                v.child(field(
                    "Username",
                    Input::new(&self.username),
                    self.username_error,
                    "Username must not be empty",
                ))
            })
            .child(field(
                "Password",
                Input::new(&self.password).content_type(InputContentType::Password),
                self.password_error,
                "Password must not be empty",
            ))
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
    if focus.needs_username {
        focus.username.read(cx).focus_handle(cx).focus(window, cx);
    } else {
        focus.password.read(cx).focus_handle(cx).focus(window, cx);
    }
    retained
}
