//! Define Remote: Rebased GitDefineRemoteDialog, 4aec352 (Apache-2.0).
//! Form validation and URL access checks are read-only; the owner performs the
//! eventual edit and refresh, after this dialog has closed.
use gpui_kit::component::{
    Disableable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::{Cancel, Repository},
    i18n::t,
    remote_management::{self as remotes, Edit, Remote},
};
use std::{cell::RefCell, rc::Rc};
type Accepted = Rc<RefCell<Option<Box<dyn FnOnce(Edit, bool, &mut Window, &mut App)>>>>;
struct Form {
    repo: Repository,
    previous: Option<Remote>,
    name: Entity<InputState>,
    url: Entity<InputState>,
    fetch: bool,
    busy: bool,
    closed: bool,
    error: Option<String>,
    token: Cancel,
    task: Task<()>,
    accepted: Accepted,
}
impl Drop for Form {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Form {
    fn edit(&self, cx: &App) -> Edit {
        let name = self.name.read(cx).value().trim().to_owned();
        let url = self.url.read(cx).value().trim().to_owned();
        match &self.previous {
            Some(previous) => Edit::Change {
                previous: previous.clone(),
                name,
                url,
            },
            None => Edit::Add { name, url },
        }
    }
    fn check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.closed {
            return;
        }
        let edit = self.edit(cx);
        let url = self.url.read(cx).value().trim().to_owned();
        let helper = match std::env::current_exe() {
            Ok(path) => path.with_file_name(if cfg!(windows) {
                "plico-askpass.exe"
            } else {
                "plico-askpass"
            }),
            Err(_) => {
                self.error = Some(t("Authentication helper is unavailable").into());
                cx.notify();
                return;
            }
        };
        let token = self.token.clone();
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.busy = true;
        self.error = None;
        cx.background_spawn(async move {
            let result = remotes::validate_edit(&repo, &edit, &token).and_then(|_| {
                remotes::check_access(
                    &repo,
                    &url,
                    &worker_requests,
                    &helper,
                    &token,
                    &Default::default(),
                )
                .map(|output| {
                    output.status.success()
                        && output.stopped.is_none()
                        && output.input_error.is_none()
                })
            });
            let _ = tx.send((edit, result));
        })
        .detach();
        self.task = cx.spawn_in(window, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            loop {
                if let Some(pending) = requests.take_pending() {
                    if let Ok(dialog) = wx.update(|w,cx| crate::auth_dialog::show(pending,w,cx)) { login=Some(dialog); }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let _ = wx.update(|w, cx| {
                            if login.as_ref().is_some_and(|login| login.read(cx).active()) { w.close_dialog(cx); }
                            let _ = this.update(cx, |s, cx| {
                                s.busy = false;
                                s.error = Some(t("Could not validate the remote configuration").into());
                                cx.notify();
                            });
                        });
                        break;
                    },
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some((edit,result)) = result {
                    let _ = wx.update(|w,cx| {
                        if login.as_ref().is_some_and(|login| login.read(cx).active()) { w.close_dialog(cx); }
                        let mut accept = None;
                        let _ = this.update(cx, |s,cx| {
                            s.busy=false;
                            if s.closed || s.token.cancelled() {
                                // Authentication dismissal cancels only this attempt; the form
                                // can be corrected and tried again while it remains open.
                                if !s.closed { s.token=Cancel::default(); s.error=Some(t("Remote access check cancelled").into()); }
                                cx.notify(); return;
                            }
                            match result {
                                Ok(true) => accept=Some((s.accepted.clone(),s.fetch)),
                                Ok(false) => s.error=Some(t("Could not access the remote. Check the URL and authentication, then retry.").into()),
                                Err(error) => s.error=Some(t_error(&error)),
                            }
                            cx.notify();
                        });
                        if let Some((accepted,fetch))=accept {
                            w.close_dialog(cx);
                            if let Some(callback)=accepted.borrow_mut().take() { callback(edit,fetch,w,cx); }
                        }
                    });
                    break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(30)).await;
            }
        });
        cx.notify();
    }
}
fn t_error(error: &str) -> String {
    // Validation strings contain no URL or raw Git output.
    match error {
        "A remote with that name already exists" => {
            t("A remote with that name already exists").into()
        }
        "Remote changed; reopen the remote configuration" => {
            t("Remote changed; reopen the remote configuration").into()
        }
        "Enter a nonempty remote URL or path" => t("Enter a nonempty remote URL or path").into(),
        "Authentication helper is unavailable" => t("Authentication helper is unavailable").into(),
        _ => t("Could not validate the remote configuration").into(),
    }
}
impl Render for Form {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .items_center()
                    .child(div().w(px(70.)).child(t("Remote name")))
                    .child(Input::new(&self.name).disabled(self.busy).flex_1()),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .items_center()
                    .child(div().w(px(70.)).child("URL"))
                    .child(Input::new(&self.url).disabled(self.busy).flex_1()),
            )
            .when(self.busy, |v| v.child(t("Checking remote access…")))
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
pub fn show(
    repo: Repository,
    previous: Option<Remote>,
    proposed_name: String,
    window: &mut Window,
    cx: &mut App,
    accepted: impl FnOnce(Edit, bool, &mut Window, &mut App) + 'static,
) {
    let form = cx.new(|cx| {
        let name = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(proposed_name, window, cx);
            s
        });
        let url = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(
                previous
                    .as_ref()
                    .and_then(|p| p.urls.first())
                    .cloned()
                    .unwrap_or_default(),
                window,
                cx,
            );
            s
        });
        for input in [&name, &url] {
            cx.subscribe(input, |s: &mut Form, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    s.error = None;
                    cx.notify();
                }
            })
            .detach();
        }
        Form {
            repo,
            previous,
            name,
            url,
            fetch: true,
            busy: false,
            closed: false,
            error: None,
            token: Cancel::default(),
            task: Task::ready(()),
            accepted: Rc::new(RefCell::new(Some(Box::new(accepted)))),
        }
    });
    let handle = window.window_handle();
    cx.observe(&form, move |_, cx| {
        let _ = handle.update(cx, |_, w, _| w.refresh());
    })
    .detach();
    let state = form.read(cx);
    let focus = if state.name.read(cx).value().is_empty() {
        &state.name
    } else {
        &state.url
    };
    let focus = focus.read(cx).focus_handle(cx);
    window.open_dialog(cx, move |d, _, cx| {
        let confirm = form.clone();
        let close = form.clone();
        let fetch = form.clone();
        d.title(t("Define Remote"))
            .w(px(560.))
            .centered(true)
            .overlay_closable(false)
            .child(form.clone())
            .child(
                Checkbox::new("remote-fetch")
                    .label(t("Fetch remote"))
                    .checked(form.read(cx).fetch)
                    .disabled(form.read(cx).busy)
                    .on_click(move |checked, _, cx| {
                        fetch.update(cx, |s, cx| {
                            s.fetch = *checked;
                            cx.notify();
                        })
                    }),
            )
            .footer(crate::dialog_style::confirmation(
                Button::new("define-remote-ok")
                    .primary()
                    .label(t("OK"))
                    .disabled(form.read(cx).busy),
            ))
            .on_ok(move |_, w, cx| {
                confirm.update(cx, |s, cx| s.check(w, cx));
                false
            })
            .on_close(move |_, _, cx| {
                close.update(cx, |s, _| {
                    s.closed = true;
                    s.token.cancel();
                })
            })
    });
    focus.focus(window, cx);
}
