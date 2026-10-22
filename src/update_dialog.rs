use gpui_kit::component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    i18n::t,
    update_decision::{Action, Answer, Pending, Problem, Prompt},
};

actions!(push_update, [NextControl, PreviousControl, ActivateControl]);
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", NextControl, Some("PushUpdate")),
        KeyBinding::new("shift-tab", PreviousControl, Some("PushUpdate")),
        KeyBinding::new("space", ActivateControl, Some("PushUpdate")),
    ]);
}

pub struct UpdateDialog {
    pending: Option<Pending>,
    prompt: Prompt,
    focus: FocusHandle,
    selected: usize,
    remember: bool,
}
impl UpdateDialog {
    pub fn active(&self) -> bool {
        self.pending.is_some()
    }
    fn actions(&self) -> Vec<Action> {
        if matches!(self.prompt, Prompt::Published { .. }) {
            vec![Action::Rebase, Action::Cancel]
        } else {
            vec![Action::Merge, Action::Rebase, Action::Cancel]
        }
    }
    fn has_remember(&self) -> bool {
        matches!(self.prompt, Prompt::Rejected { .. })
    }
    fn move_control(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let count = self.actions().len() + usize::from(self.has_remember());
        self.selected = (self.selected + if backwards { count - 1 } else { 1 }) % count;
        cx.notify();
    }
    fn confirm_action(&self) -> Action {
        self.actions()
            .get(self.selected)
            .copied()
            .unwrap_or(self.prompt.default_and_focus().0)
    }
    fn answer(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pending) = self.pending.take() {
            pending.answer(Answer {
                action,
                remember: self.remember,
            });
        }
        window.close_dialog(cx);
    }
    fn footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let (default, _) = self.prompt.default_and_focus();
        let mut buttons = h_flex().flex_shrink_0().justify_end().gap_2();
        for (index, action) in self.actions().into_iter().enumerate() {
            let label = match action {
                Action::Merge => "Merge",
                Action::Cancel => "Cancel",
                Action::Rebase
                    if matches!(
                        self.prompt,
                        Prompt::Rejected {
                            problem: Problem::No,
                            ..
                        } | Prompt::OverMerge { .. }
                    ) =>
                {
                    "Rebase"
                }
                Action::Rebase => "Rebase Anyway",
            };
            buttons = buttons.child(
                Button::new(("update-choice", index))
                    .tab_stop(false)
                    .min_w(px(72.))
                    .label(t(label))
                    .when(action == default, |b| b.primary())
                    .when(index == self.selected, |b| {
                        b.border_2().border_color(rgb(0x6b9bfa))
                    })
                    .on_click(cx.listener(move |s, _, w, cx| s.answer(action, w, cx))),
            );
        }
        h_flex().w_full().gap_4()
            .when(self.has_remember(), |v| v.child(
                div().flex_1().min_w_0().rounded(px(3.)).border_2().border_color(if self.selected == self.actions().len() { rgb(0x6b9bfa) } else { rgba(0x00000000) })
                .child(Checkbox::new("remember-update").label(t("Remember this choice for future push rejections (change in Settings)"))
                    .tab_stop(false).checked(self.remember).on_click(cx.listener(|s, value, w, cx| { s.remember = *value; s.selected = s.actions().len(); s.focus.focus(w, cx); cx.notify(); })))))
            .child(div().flex_1().when(self.has_remember(), |v| v.flex_none()))
            .child(buttons).into_any_element()
    }
}
impl Render for UpdateDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let warning = match self.prompt {
            Prompt::Rejected {
                problem: Problem::No,
                ..
            } => "Remote history has changed. Update before pushing again.",
            Prompt::Rejected {
                problem: Problem::Yes,
                ..
            }
            | Prompt::OverMerge {
                problem: Problem::Yes,
            } => {
                "Rebase may discard changes made in merge commits. Merge instead or rebase anyway?"
            }
            Prompt::Rejected { .. } | Prompt::OverMerge { .. } => {
                "Could not check merge commits. Choose how to update or cancel."
            }
            Prompt::Published { unknown: false } => {
                "Rebase will rewrite published commits. Rebase anyway?"
            }
            Prompt::Published { unknown: true } => {
                "Could not check published commits. Rebase anyway?"
            }
        };
        v_flex()
            .gap_4()
            .key_context("PushUpdate")
            .track_focus(&self.focus)
            .on_action(cx.listener(|s, _: &NextControl, _, cx| s.move_control(false, cx)))
            .on_action(cx.listener(|s, _: &PreviousControl, _, cx| s.move_control(true, cx)))
            .on_action(cx.listener(|s, _: &ActivateControl, w, cx| {
                if s.has_remember() && s.selected == s.actions().len() {
                    s.remember = !s.remember;
                    cx.notify();
                } else {
                    s.answer(s.confirm_action(), w, cx);
                }
            }))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(15.))
                    .child(
                        img("jb/warningDialog_dark_4x.svg")
                            .size(px(28.))
                            .flex_shrink_0(),
                    )
                    .child(div().flex_1().min_w_0().child(t(warning))),
            )
            .capture_key_down(cx.listener(|s, event: &KeyDownEvent, w, cx| {
                let actions = s.actions();
                match event.keystroke.key.as_str() {
                    "escape" => s.answer(Action::Cancel, w, cx),
                    "enter" => s.answer(s.confirm_action(), w, cx),
                    "space" if s.has_remember() && s.selected == actions.len() => {
                        s.remember = !s.remember;
                        cx.notify();
                    }
                    "left" if s.selected < actions.len() => {
                        s.selected = (s.selected + actions.len() - 1) % actions.len();
                        cx.notify();
                    }
                    "right" if s.selected < actions.len() => {
                        s.selected = (s.selected + 1) % actions.len();
                        cx.notify();
                    }
                    "tab" => {
                        let count = actions.len() + usize::from(s.has_remember());
                        s.selected = (s.selected
                            + if event.keystroke.modifiers.shift {
                                count - 1
                            } else {
                                1
                            })
                            % count;
                        cx.notify();
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
    }
}
// This view owns no pending decision. Its weak owner and observation live only
// for this dialog, keeping footer state in sync with keyboard selection.
struct UpdateFooter {
    owner: WeakEntity<UpdateDialog>,
}
impl Render for UpdateFooter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.owner
            .update(cx, |state, cx| state.footer(cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}
pub fn show(pending: Pending, window: &mut Window, cx: &mut App) -> Entity<UpdateDialog> {
    let prompt = pending.prompt;
    let dialog = cx.new(|cx| {
        let mut state = UpdateDialog {
            pending: Some(pending),
            prompt,
            focus: cx.focus_handle(),
            selected: 0,
            remember: false,
        };
        state.selected = state
            .actions()
            .iter()
            .position(|a| *a == prompt.default_and_focus().1)
            .unwrap_or(0);
        state
    });
    let footer = cx.new(|cx| {
        cx.observe(&dialog, |_, _, cx| cx.notify()).detach();
        UpdateFooter {
            owner: dialog.downgrade(),
        }
    });
    let retained = dialog.clone();
    window.open_dialog(cx, move |d, _, _| {
        let close = dialog.clone();
        let confirm = dialog.clone();
        d.title(t(match prompt {
            Prompt::Rejected { .. } => "Push Rejected",
            Prompt::OverMerge { .. } => "Rebasing Merge Commits",
            _ => "Update Confirmation",
        }))
        .w(px(600.))
        .centered(true)
        .overlay_closable(false)
        .child(dialog.clone())
        .footer(footer.clone())
        .on_ok(move |_, window, cx| {
            confirm.update(cx, |s, cx| {
                let action = s.confirm_action();
                s.answer(action, window, cx);
            });
            false
        })
        .on_close(move |_, _, cx| {
            close.update(cx, |s, _| {
                s.pending.take();
            })
        })
    });
    retained.read(cx).focus.clone().focus(window, cx);
    retained
}
