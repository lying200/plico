//! Rebased GitUserNameNotDefinedDialog: name, email, explicit global scope,
//! validation, and resume the original commit only after configuration succeeds.
use gpui_kit::component::{
    Disableable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::{Cancel, Repository},
    i18n::t,
    identity_config::{self, Identity, Scope},
};
use std::{cell::RefCell, rc::Rc};
struct Form {
    name: Entity<InputState>,
    email: Entity<InputState>,
    global: bool,
    busy: bool,
    closed: bool,
    error: Option<String>,
    token: Cancel,
    task: Task<()>,
}
impl Drop for Form {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Form {
    fn value(&self, cx: &App) -> Identity {
        Identity {
            name: self.name.read(cx).value().trim().into(),
            email: self.email.read(cx).value().trim().into(),
        }
    }
}
impl Render for Form {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .child(div().w(px(85.)).child(t("Name")))
                    .child(Input::new(&self.name).disabled(self.busy).flex_1()),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(div().w(px(85.)).child(t("Email")))
                    .child(Input::new(&self.email).disabled(self.busy).flex_1()),
            )
            .child(
                Checkbox::new("identity-global")
                    .label(t("Set properties globally"))
                    .checked(self.global)
                    .disabled(self.busy)
                    .on_click(cx.listener(|s, checked, _, cx| {
                        s.global = *checked;
                        cx.notify();
                    })),
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
pub fn show(
    repo: Repository,
    initial: Identity,
    window: &mut Window,
    cx: &mut App,
    resume: impl FnOnce(&mut Window, &mut App) + 'static,
) {
    let form = cx.new(|cx| {
        let name = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(initial.name, window, cx);
            s
        });
        let email = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(initial.email, window, cx);
            s
        });
        Form {
            name,
            email,
            global: false,
            busy: false,
            closed: false,
            error: None,
            token: Cancel::default(),
            task: Task::ready(()),
        }
    });
    let focus = form.read(cx).name.read(cx).focus_handle(cx);
    let resume = Rc::new(RefCell::new(Some(resume)));
    window.open_dialog(cx,move |d,_,cx| {
        let confirm=form.clone();let close=form.clone();let repo=repo.clone();let resume=resume.clone();
        d.title(t("Git name and email are not configured")).w(px(520.)).centered(true).overlay_closable(false).child(form.clone())
            .footer(crate::dialog_style::confirmation(Button::new("identity-save").primary().label(t(if form.read(cx).busy {"Saving…"} else {"Set and Commit"})).disabled(form.read(cx).busy)))
            .on_ok(move |_,w,cx| {
                confirm.update(cx,|s,cx| {
                    if s.busy || s.closed {return;}
                    let identity=s.value(cx);
                    if identity.validate().is_err() {s.error=Some(t("Enter a name and an email address containing @").into());cx.notify();return;}
                    s.busy=true;s.error=None;
                    let scope=if s.global {Scope::Global} else {Scope::Repository};
                    let repo=repo.clone();let token=s.token.clone();let resume=resume.clone();
                    s.task=cx.spawn_in(w,async move |this,wx| {
                        let result=wx.background_spawn(async move {identity_config::set(&repo,&identity,scope,&token,&Default::default())}).await;
                        let _=wx.update(|w,cx| {
                            let ready=this.update(cx,|s,cx| {
                                s.busy=false;
                                if s.closed || s.token.cancelled() {return false;}
                                let ok=result.as_ref().is_ok_and(|r|r.succeeded());
                                if !ok {s.error=Some(t("Could not configure Git identity. Check configuration permissions and repository overrides.").into());}
                                cx.notify();ok
                            }).unwrap_or(false);
                            if ready {w.close_dialog(cx);if let Some(resume)=resume.borrow_mut().take() {resume(w,cx);}}
                        });
                    });cx.notify();
                });false
            })
            .on_close(move |_,_,cx| {close.update(cx,|s,_|{s.closed=true;s.token.cancel();});})
    });
    focus.focus(window, cx);
}
