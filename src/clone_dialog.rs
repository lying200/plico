//! Rebased DvcsCloneDialogComponent: URL, destination chooser, shallow clone.
use gpui_kit::component::{
    Disableable as _, Sizable as _, Icon, WindowExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{clone_repository::Request, git::Cancel, i18n::t};
use std::{path::PathBuf, rc::Rc};
pub struct Form {
    suggestion: plico::clone_destination::Suggestion,
    url: Entity<InputState>,
    directory: Entity<InputState>,
    depth: Entity<InputState>,
    shallow: bool,
    recurse: bool,
    busy: bool,
    closed: bool,
    token: Cancel,
    error: Option<String>,
    progress: String,
}
impl Drop for Form {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Form {
    pub fn begin(&mut self, token: Cancel, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        self.progress.clear();
        self.token = token;
        cx.notify();
    }
    pub fn progress(&mut self, text: String, cx: &mut Context<Self>) {
        if self.progress != text {
            self.progress = text;
            cx.notify();
        }
    }
    pub fn complete(&mut self, error: Option<String>, cx: &mut Context<Self>) -> bool {
        self.busy = false;
        self.error = error;
        self.progress.clear();
        cx.notify();
        !self.closed
    }
    fn request(&mut self, cx: &mut Context<Self>) -> Option<Request> {
        if self.busy || self.closed {
            return None;
        }
        let depth = if self.shallow {
            match self.depth.read(cx).value().parse::<u32>() {
                Ok(n) if n > 0 => Some(n),
                _ => {
                    self.error = Some(t("Clone depth must be greater than zero").into());
                    cx.notify();
                    return None;
                }
            }
        } else {
            None
        };
        Some(Request {
            source: self.url.read(cx).value().trim().into(),
            destination: PathBuf::from(self.directory.read(cx).value().trim()),
            depth,
            recurse_submodules: self.recurse,
        })
    }
    fn browse(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let result = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t("Clone destination directory").into()),
        });
        cx.spawn_in(w, async move |this, wx| {
            if let Some(path) = result
                .await
                .ok()
                .and_then(|v| v.ok())
                .flatten()
                .and_then(|v| v.into_iter().next())
            {
                let _ = this.update_in(wx, |s, w, cx| {
                    if !s.busy && !s.closed {
                        s.suggestion.select_directory();
                        s.directory
                            .update(cx, |i, cx| i.set_value(path.to_string_lossy(), w, cx));
                    }
                });
            }
        })
        .detach();
    }
}
impl Render for Form {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .min_w_0().min_h(px(260.))
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().w(px(70.)).flex_shrink_0().child("URL:"))
                    .child(Input::new(&self.url).disabled(self.busy).flex_1()),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().w(px(70.)).flex_shrink_0().child(format!("{}:", t("Directory"))))
                    .child(Input::new(&self.directory).disabled(self.busy).flex_1().suffix(
                        Button::new("clone-browse")
                            .ghost().small().icon(Icon::default().path("jb/open_dark.svg").size(px(16.)))
                            .tooltip(t("Clone destination directory"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|s, _, w, cx| s.browse(w, cx))),
                    )),
            )
            .child(
                h_flex()
                    .gap_3().items_center().flex_wrap()
                    .child(
                        Checkbox::new("clone-shallow")
                            .label(t("Shallow clone with a history truncated to"))
                            .checked(self.shallow)
                            .disabled(self.busy)
                            .on_click(cx.listener(|s, v, _, cx| {
                                s.shallow = *v;
                                cx.notify();
                            })),
                    )
                    .child(Input::new(&self.depth).disabled(self.busy || !self.shallow).w(px(66.)))
                    .child(t("commits")),
            )
            .child(
                Checkbox::new("clone-submodules")
                    .label(t("Clone submodules"))
                    .checked(self.recurse)
                    .disabled(self.busy)
                    .on_click(cx.listener(|s, v, _, cx| {
                        s.recurse = *v;
                        cx.notify();
                    })),
            )
            .when(self.busy, |v| {
                v.child(
                    h_flex()
                        .gap_2()
                        .child(gpui_kit::component::spinner::Spinner::new().small())
                        .child(t(if self.token.cancelled() {
                            "Cancelling clone…"
                        } else {
                            "Cloning…"
                        })),
                )
                .child(
                    div()
                        .max_h(px(100.))
                        .overflow_hidden()
                        .whitespace_normal()
                        .child(self.progress.clone()),
                )
            })
            .when_some(self.error.clone(), |v, e| {
                v.child(div().whitespace_normal().text_color(rgb(0xf2a6a6)).child(e))
            })
    }
}
pub fn show(
    w: &mut Window,
    cx: &mut App,
    start: impl Fn(Request, Entity<Form>, &mut Window, &mut App) + 'static,
) {
    let form = cx.new(|cx| {
        let suggestion =
            plico::clone_destination::Suggestion::new(plico::clone_destination::parent());
        let directory = cx.new(|cx| {
            let mut input = InputState::new(w, cx);
            input.set_value(suggestion.current().to_string_lossy(), w, cx);
            input
        });
        let url = cx.new(|cx| InputState::new(w, cx));
        cx.subscribe(&directory, |s: &mut Form, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                s.suggestion
                    .edited(std::path::Path::new(input.read(cx).value().as_ref()));
                cx.notify();
            }
        })
        .detach();
        cx.subscribe_in(&url, w, |s: &mut Form, input, event: &InputEvent, w, cx| {
            if matches!(event, InputEvent::Change) && !s.busy {
                if let Some(path) = s.suggestion.from_url(input.read(cx).value().as_ref()) {
                    s.directory
                        .update(cx, |i, cx| i.set_value(path.to_string_lossy(), w, cx));
                }
                cx.notify();
            }
        })
        .detach();
        Form {
            suggestion,
            url,
            directory,

            depth: cx.new(|cx| {
                let mut i = InputState::new(w, cx);
                i.set_value("1", w, cx);
                i
            }),
            shallow: false,
            recurse: false,
            busy: false,
            closed: false,
            token: Cancel::default(),
            error: None,
            progress: String::new(),
        }
    });
    let focus = form.read(cx).url.read(cx).focus_handle(cx);
    let start = Rc::new(start);
    let handle = w.window_handle();
    cx.observe(&form, move |_, cx| {
        let _ = handle.update(cx, |_, w, _| w.refresh());
    })
    .detach();
    w.open_dialog(cx, move |d, _, cx| {
        let confirm = form.clone();
        let close = form.clone();
        let start = start.clone();
        d.title(t("Clone Repository"))
            .w(px(620.))
            .centered(true)
            .overlay_closable(false)
            .child(form.clone())
            .footer(if form.read(cx).busy {
                let cancel = form.clone();
                gpui_kit::component::dialog::DialogFooter::new().child(
                    Button::new("cancel-clone")
                        .label(t("Cancel"))
                        .disabled(form.read(cx).token.cancelled())
                        .on_click(move |_, _, cx| {
                            cancel.update(cx, |s, cx| {
                                s.token.cancel();
                                cx.notify();
                            })
                        }),
                )
            } else {
                crate::dialog_style::confirmation(
                    Button::new("clone-confirm").primary().label(t("Clone"))
                        .disabled(form.read(cx).url.read(cx).value().trim().is_empty() || form.read(cx).directory.read(cx).value().trim().is_empty()),
                )
            })
            .on_ok(move |_, w, cx| {
                let request = confirm.update(cx, |s, cx| s.request(cx));
                if let Some(request) = request {
                    start(request, confirm.clone(), w, cx);
                }
                false
            })
            .on_close(move |_, _, cx| {
                close.update(cx, |s, _| {
                    s.closed = true;
                    s.token.cancel();
                })
            })
    });
    focus.focus(w, cx);
}
