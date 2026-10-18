//! GitNewCommitMessageActionDialog (Rebased 4aec352, JetBrains, Apache-2.0):
//! labelled message editor, initial focus and nonblank validation.
use plico::{i18n::t, git::{Repository, Cancel}, squash_check::{self, Prepared}, published_commits::Protection};
use std::{rc::Rc, sync::Arc};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit::component::{WindowExt as _, Disableable as _, button::{Button, ButtonVariants as _},
    input::{Textarea, TextareaState}, v_flex};
struct Message {
    input: Entity<TextareaState>,
    _subscription: Subscription,
    busy: bool,
    closed: bool,
    error: Option<String>,
    token: Cancel,
    task: Task<()>,
}
impl Message {
    fn valid(&self, cx: &App) -> bool {
        let value = self.input.read(cx).value();
        !self.busy && !self.closed && self.error.is_none() && !value.trim().is_empty() && value.len() <= 1024 * 1024 && !value.contains('\0')
    }
    fn confirm(&mut self, repo: Repository, prepared: Arc<Prepared>, submit: Rc<dyn Fn(String, &mut Window, &mut App)>,
        window: &mut Window, cx: &mut Context<Self>) {
        if !self.valid(cx) { return; }
        let value = self.input.read(cx).value().to_string();
        self.busy = true; self.error = None;
        let token = self.token.clone();
        self.task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                squash_check::revalidate(&repo, &prepared, Protection::Defaults, &token).map(|_| ())
            }).await;
            let _ = wx.update(|window, cx| {
                let ready = this.update(cx, |s, cx| {
                    if s.closed || s.token.cancelled() { return false; }
                    s.busy = false;
                    match result {
                        Ok(()) => { cx.notify(); true }
                        Err(error) => { s.error = Some(validation_message(&error)); cx.notify(); false }
                    }
                }).unwrap_or(false);
                if ready { window.close_dialog(cx); submit(value, window, cx); }
            });
        });
        cx.notify();
    }
}
impl Drop for Message {
    fn drop(&mut self) { self.token.cancel(); }
}
impl Render for Message {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(t("Message for the squashed commit:"))
            .child(Textarea::new(&self.input).disabled(self.busy).external_focus_ring(false).h(px(280.)).aria_label(t("Commit message")))
            .when_some(self.error.clone(), |v, error| v.child(div().pt(px(4.)).text_color(rgb(0xf2a6a6)).child(error)))
    }
}
pub fn show(repo: Repository, prepared: Arc<Prepared>, draft: String, window: &mut Window, cx: &mut App,
    submit: impl Fn(String, &mut Window, &mut App) + 'static) {
    let message = cx.new(|cx| {
        let input = cx.new(|cx| { let mut s = TextareaState::new(window, cx); s.set_value(draft, window, cx); s });
        let subscription = cx.observe(&input, |_, _, cx| cx.notify());
        Message { input, _subscription: subscription, busy: false, closed: false, error: None, token: Cancel::default(), task: Task::ready(()) }
    });
    let focus = message.read(cx).input.read(cx).focus_handle(cx);
    let submit: Rc<dyn Fn(String, &mut Window, &mut App)> = Rc::new(submit);
    window.open_dialog(cx, move |dialog, _, cx| {
        let confirm = message.clone(); let close = message.clone(); let submit = submit.clone();
        let repo = repo.clone(); let prepared = prepared.clone();
        dialog.title(t("Squash Commits")).w(px(640.)).centered(true).overlay_closable(false)
            .child(message.clone())
            .footer(crate::dialog_style::confirmation(Button::new("squash-ok").primary().label(t(if message.read(cx).busy { "Checking…" } else { "OK" }))
                .disabled(!message.read(cx).valid(cx))))
            .on_ok(move |_, window, cx| {
                confirm.update(cx, |s, cx| s.confirm(repo.clone(), prepared.clone(), submit.clone(), window, cx));
                false
            })
            .on_close(move |_, _, cx| { close.update(cx, |s, _| { s.closed = true; s.token.cancel(); }); })
    });
    focus.focus(window, cx);
}

// DialogWrapper renders component-less ValidationInfo as one error-colored
// message above the buttons. Keep backend diagnostics out of this label.
fn validation_message(error: &str) -> String {
    if let Some(branch) = error.strip_prefix("Selected commits are published to protected branch ") {
        return format!("{} ‘{}’", t("Cannot edit selected commits: pushed to protected branch"), branch);
    }
    if error.contains("HEAD changed") || error.contains("selection changed") || error.contains("references changed") {
        return t("Cannot edit selected commits: repository state was changed").into();
    }
    if error.contains("current Git operation") {
        return t("Cannot edit selected commits: another Git operation is in progress").into();
    }
    if error.contains("recovery") || error.contains("previously saved Squash") {
        return t("Cannot edit selected commits: recover the previous operation first").into();
    }
    t("Cannot validate selected commits. Close this dialog and refresh the log.").into()
}
