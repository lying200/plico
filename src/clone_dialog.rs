//! Rebased DvcsCloneDialogComponent: URL, destination chooser, shallow clone.
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{clone_repository::Request, git::Cancel, i18n::t};
use std::{path::PathBuf, rc::Rc};
pub struct Form {
    page: usize,
    host: Entity<crate::hosting_panel::Panel>,
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
        self.host.update(cx, |s, cx| s.cancel(cx));
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
impl Form {
    fn fields(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .min_w_0()
            .when(self.page == 0, |v| {
                v.child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(div().w(px(70.)).flex_shrink_0().child("URL:"))
                        .child(
                            Input::new(&self.url)
                                .external_focus_ring(false)
                                .rounded(px(4.))
                                .disabled(self.busy)
                                .flex_1(),
                        ),
                )
            })
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .w(px(70.))
                            .flex_shrink_0()
                            .child(format!("{}:", t("Directory"))),
                    )
                    .child(
                        Input::new(&self.directory)
                            .external_focus_ring(false)
                            .rounded(px(4.))
                            .disabled(self.busy)
                            .flex_1()
                            .suffix(
                                Button::new("clone-browse")
                                    .ghost()
                                    .small()
                                    .icon(Icon::default().path("jb/open_dark.svg").size(px(16.)))
                                    .tooltip(t("Clone destination directory"))
                                    .disabled(self.busy)
                                    .on_click(cx.listener(|s, _, w, cx| s.browse(w, cx))),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .child(
                        crate::ui_style::checkbox("clone-shallow")
                            .label(t("Shallow clone with a history truncated to"))
                            .checked(self.shallow)
                            .disabled(self.busy)
                            .on_click(cx.listener(|s, v, _, cx| {
                                s.shallow = *v;
                                cx.notify();
                            })),
                    )
                    .child(
                        Input::new(&self.depth)
                            .external_focus_ring(false)
                            .rounded(px(4.))
                            .disabled(self.busy || !self.shallow)
                            .w(px(66.)),
                    )
                    .child(t("commits")),
            )
            .child(
                crate::ui_style::checkbox("clone-submodules")
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
impl Render for Form {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut sidebar = v_flex()
            .w(px(190.))
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(rgb(crate::ui_style::BORDER));
        for (page, label) in [(0, "Repository URL"), (1, "GitHub"), (2, "GitLab")] {
            sidebar = sidebar.child(
                div()
                    .id(("clone-provider", page))
                    .px_3()
                    .py_3()
                    .cursor_pointer()
                    .when(self.page == page, |v| v.bg(rgb(crate::ui_style::SELECT)))
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                img(match page {
                                    1 => "jb/github_dark.svg",
                                    2 => "jb/gitlab.svg",
                                    _ => "jb/vcs_dark.svg",
                                })
                                .size(px(20.)),
                            )
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(t(label)))
                                    .when(page > 0, |v| {
                                        v.child(
                                            div().text_color(rgb(crate::ui_style::MUTED)).child(
                                                self.host.read(cx).account_label(if page == 1 {
                                                    plico::hosting::Provider::GitHub
                                                } else {
                                                    plico::hosting::Provider::GitLab
                                                }),
                                            ),
                                        )
                                    }),
                            ),
                    )
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if s.busy {
                            return;
                        }
                        s.page = page;
                        s.error = None;
                        s.url.update(cx, |i, cx| i.set_value("", w, cx));
                        if page == 0 {
                            s.host.update(cx, |s, cx| s.cancel(cx));
                            s.url.read(cx).focus_handle(cx).focus(w, cx);
                        } else {
                            s.host.update(cx, |s, cx| {
                                s.switch(
                                    if page == 1 {
                                        plico::hosting::Provider::GitHub
                                    } else {
                                        plico::hosting::Provider::GitLab
                                    },
                                    w,
                                    cx,
                                )
                            });
                        }
                        cx.notify();
                    })),
            );
        }
        let mut content = v_flex().flex_1().min_w_0().h_full().gap_3().p_3();
        if self.page > 0 && !self.busy {
            content = content.child(div().h(px(0.)).flex_1().min_h_0().child(self.host.clone()));
        }
        if self.page == 0 || self.host.read(cx).shows_repositories() {
            content = content.child(self.fields(cx));
        }
        h_flex()
            .w_full()
            .h((window.viewport_size().height - px(150.))
                .max(px(180.))
                .min(px(520.)))
            .items_stretch()
            .child(sidebar)
            .child(content)
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
        let host = crate::hosting_panel::Panel::new(w, cx);
        cx.subscribe_in(
            &host,
            w,
            |s: &mut Form, _, selection: &crate::hosting_panel::Selection, w, cx| {
                if !s.busy && !s.closed {
                    s.url.update(cx, |i, cx| {
                        i.set_value(selection.0.clone().unwrap_or_default(), w, cx)
                    });
                    cx.notify();
                }
            },
        )
        .detach();
        cx.observe(&host, |_, _, cx| cx.notify()).detach();
        Form {
            page: 0,
            host,
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
        d.font_family(cx.theme().font_family.clone())
            .text_size(cx.theme().font_size)
            .title(t("Clone Repository"))
            .w(px(900.))
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
                    Button::new("clone-confirm")
                        .primary()
                        .label(t("Clone"))
                        .disabled(
                            form.read(cx).url.read(cx).value().trim().is_empty()
                                || form.read(cx).directory.read(cx).value().trim().is_empty(),
                        ),
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
                close.update(cx, |s, cx| {
                    s.host.update(cx, |s, cx| s.cancel(cx));
                    s.closed = true;
                    s.token.cancel();
                })
            })
    });
    focus.focus(w, cx);
}
