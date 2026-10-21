//! GitConfigureRemotesDialog: single-repository name/URL table and nested
//! Define Remote/removal confirmation. Git execution belongs to Review.
use gpui_kit::component::{
    Disableable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogClose, DialogFooter},
    h_flex, v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::Repository,
    i18n::t,
    remote_management::{Edit, Remote},
};
use std::rc::Rc;
type Apply = Rc<dyn Fn(Edit, bool, Entity<Manager>, &mut Window, &mut App)>;
pub struct Manager {
    repo: Repository,
    rows: Vec<Remote>,
    selected: Option<usize>,
    busy: bool,
    error: Option<String>,
    apply: Apply,
}
impl Manager {
    pub fn begin(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
    }
    pub fn complete(
        &mut self,
        rows: Result<Vec<Remote>, String>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let selected = self
            .selected
            .and_then(|i| self.rows.get(i))
            .map(|r| r.name.clone());
        self.busy = false;
        self.error = error;
        match rows {
            Ok(rows) => {
                self.selected = selected.and_then(|name| rows.iter().position(|r| r.name == name));
                self.rows = rows;
            }
            Err(_) => {
                self.rows.clear();
                self.selected = None;
                self.error = Some(t("Could not reload remote configuration").into());
            }
        }
        cx.notify();
    }
    fn define(&mut self, edit: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let previous = if edit {
            self.selected.and_then(|i| self.rows.get(i)).cloned()
        } else {
            None
        };
        if edit && previous.is_none() {
            return;
        }
        let proposed = previous
            .as_ref()
            .map(|r| r.name.clone())
            .unwrap_or_else(|| {
                if self.rows.iter().any(|r| r.name == "origin") {
                    String::new()
                } else {
                    "origin".into()
                }
            });
        let owner = cx.entity().downgrade();
        let apply = self.apply.clone();
        crate::define_remote_dialog::show(
            self.repo.clone(),
            previous,
            proposed,
            w,
            cx,
            move |edit, fetch, w, cx| {
                if let Some(owner) = owner.upgrade() {
                    apply(edit, fetch, owner, w, cx);
                }
            },
        );
    }
    fn remove(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(previous) = self.selected.and_then(|i| self.rows.get(i)).cloned() else {
            return;
        };
        let owner = cx.entity().downgrade();
        let apply = self.apply.clone();
        w.open_dialog(cx, move |d, _, _| {
            let previous = previous.clone();
            let owner = owner.clone();
            let apply = apply.clone();
            d.title(t("Remove Remote"))
                .w(px(440.))
                .centered(true)
                .overlay_closable(false)
                .child(format!("{} ‘{}’?", t("Remove remote"), previous.name))
                .footer(crate::dialog_style::confirmation(
                    Button::new("remove-remote").primary().label(t("Remove")),
                ))
                .on_ok(move |_, w, cx| {
                    if let Some(owner) = owner.upgrade() {
                        apply(
                            Edit::Remove {
                                previous: previous.clone(),
                            },
                            false,
                            owner,
                            w,
                            cx,
                        );
                    }
                    true
                })
        });
    }
}
impl Render for Manager {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .min_w_0()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("remote-add")
                            .ghost()
                            .label("+")
                            .tooltip(t("Add Remote"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|s, _, w, cx| s.define(false, w, cx))),
                    )
                    .child(
                        Button::new("remote-edit")
                            .ghost()
                            .label(t("Edit"))
                            .disabled(self.busy || self.selected.is_none())
                            .on_click(cx.listener(|s, _, w, cx| s.define(true, w, cx))),
                    )
                    .child(
                        Button::new("remote-remove")
                            .ghost()
                            .label("−")
                            .tooltip(t("Remove Remote"))
                            .disabled(self.busy || self.selected.is_none())
                            .on_click(cx.listener(|s, _, w, cx| s.remove(w, cx))),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(div().w(px(130.)).child(t("Remote name")))
                    .child(div().flex_1().child("URL")),
            )
            .child(
                v_flex()
                    .id("remotes-table")
                    .h(px(180.))
                    .overflow_y_scroll()
                    .children(self.rows.iter().enumerate().map(|(i, r)| {
                        h_flex()
                            .id(("remote-row", i))
                            .gap_3()
                            .px_2()
                            .py_1()
                            .min_w_0()
                            .when(self.selected == Some(i), |v| v.bg(rgb(0x2e4369)))
                            .child(
                                div()
                                    .w(px(130.))
                                    .flex_shrink_0()
                                    .truncate()
                                    .child(r.name.clone()),
                            )
                            .child(
                                div().flex_1().min_w_0().truncate().child(
                                    r.urls
                                        .first()
                                        .map(|url| plico::remote_management::display_url(url))
                                        .unwrap_or_default(),
                                ),
                            )
                            .on_click(cx.listener(move |s, event: &ClickEvent, w, cx| {
                                if s.busy {
                                    return;
                                }
                                s.selected = Some(i);
                                cx.notify();
                                if event.click_count() == 2 {
                                    s.define(true, w, cx);
                                }
                            }))
                    })),
            )
            .when(self.busy, |v| v.child(t("Updating remote configuration…")))
            .when_some(self.error.clone(), |v, e| {
                v.child(div().whitespace_normal().text_color(rgb(0xf2a6a6)).child(e))
            })
    }
}
pub fn show(
    repo: Repository,
    rows: Vec<Remote>,
    w: &mut Window,
    cx: &mut App,
    apply: impl Fn(Edit, bool, Entity<Manager>, &mut Window, &mut App) + 'static,
) {
    let manager = cx.new(|_| Manager {
        repo,
        rows,
        selected: None,
        busy: false,
        error: None,
        apply: Rc::new(apply),
    });
    let handle = w.window_handle();
    cx.observe(&manager, move |_, cx| {
        let _ = handle.update(cx, |_, w, _| w.refresh());
    })
    .detach();
    w.open_dialog(cx, move |d, _, _| {
        d.title(t("Git Remotes"))
            .w(px(640.))
            .centered(true)
            .overlay_closable(false)
            .child(manager.clone())
            .footer(DialogFooter::new().child(DialogClose::new().trigger(|b| b.label(t("Close")))))
    });
}
