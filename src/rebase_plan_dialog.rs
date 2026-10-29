//! Operation-owned interactive plan dialog. Answering consumes the pending reply;
//! closing cancels it, and no queued response survives its operation generation.
use crate::{rebase_plan_details::Details, rebase_plan_table::PlanTable};
use gpui_kit::component::{
    Disableable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    resizable::{ResizableState, h_resizable, resizable_panel},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::{Repository, is_oid},
    i18n::t,
    interactive_rebase::{
        draft::Draft,
        interaction::{Answer, Pending, Question},
        model::conversion::Item,
    },
};

pub struct PlanDialog {
    pending: Option<Pending>,
    open: bool,
    accepted: bool,
    table: Option<Entity<PlanTable>>,
    details: Option<Entity<Details>>,
    error: Option<String>,
    split: Entity<ResizableState>,
    fraction: f32,
    _subscription: Option<Subscription>,
}
impl PlanDialog {
    fn new(
        repo: Repository,
        project: std::path::PathBuf,
        pending: Pending,
        draft: Option<Draft<Item>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let table = draft.map(|draft| cx.new(|cx| PlanTable::from_draft(draft, cx)));
        let details = table
            .as_ref()
            .map(|_| cx.new(|cx| Details::new(repo, project, cx)));
        let subscription = table.as_ref().map(|table| {
            cx.subscribe(table, |s, _, _: &crate::rebase_plan_table::Changed, cx| {
                s.selection_changed(cx);
                cx.notify();
            })
        });
        let mut state = Self {
            pending: Some(pending),
            open: true,
            accepted: false,
            table,
            details,
            error: None,
            split: cx.new(|_| ResizableState::default()),
            fraction: plico::layout_preferences::RebaseSplit::Plan.load(),
            _subscription: subscription,
        };
        state.selection_changed(cx);
        state
    }
    fn selection_changed(&mut self, cx: &mut Context<Self>) {
        if let Some((table, details)) = self.table.as_ref().zip(self.details.as_ref()) {
            let ids = table
                .read(cx)
                .selected_items()
                .into_iter()
                .map(|i| i.entry.target.clone())
                .filter(|id| is_oid(id))
                .collect();
            details.update(cx, |s, cx| s.select(ids, cx));
        }
    }
    pub fn dismissed(&self) -> bool {
        !self.open && !self.accepted
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn active(&self) -> bool {
        self.pending.as_ref().is_some_and(|p| !p.cancelled())
    }
    fn can_confirm(&self, cx: &App) -> bool {
        self.active()
            && !self
                .table
                .as_ref()
                .is_some_and(|table| table.read(cx).editing())
    }
    fn confirm(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.can_confirm(cx) {
            return false;
        }
        let answer = match &self.table {
            Some(table) => Answer::Plan(table.read(cx).model().clone()),
            None => Answer::Proceed,
        };
        if let (Some(pending), Answer::Plan(model)) = (&self.pending, &answer) {
            if let Question::Plan(_, encoding) = &pending.question {
                for row in model.rows() {
                    if let plico::interactive_rebase::model::Action::Reword(message) = row.action()
                    {
                        if let Err(error) = encoding.validate(message) {
                            let error = error.to_string();
                            self.error = Some(match error.as_str() {
                                "Message cannot be represented in Git editor encoding" => {
                                    t("Message cannot be represented in Git editor encoding").into()
                                }
                                "Git editor encoding would change the message" => {
                                    t("Git editor encoding would change the message").into()
                                }
                                _ => error,
                            });
                            cx.notify();
                            return false;
                        }
                    }
                }
            }
        }
        let Some(pending) = self.pending.take() else {
            return false;
        };
        if let Err(e) = pending.answer(answer) {
            self.error = Some(e.into());
            cx.notify();
            return false;
        }
        self.accepted = true;
        self.close(cx);
        true
    }
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.pending.take();
        if let Some(details) = &self.details {
            details.update(cx, |s, _| s.close());
        }
        cx.notify();
    }
    fn cancel(&mut self, w: &mut Window, cx: &mut Context<Self>) -> bool {
        let modified = self
            .table
            .as_ref()
            .is_some_and(|table| table.read(cx).modified() || table.read(cx).editing());
        if !modified || !self.active() {
            self.close(cx);
            return true;
        }
        let owner = cx.entity().downgrade();
        w.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            dialog
                .title(t("Cancel Rebase"))
                .w(px(440.))
                .centered(true)
                .overlay_closable(false)
                .child(t("Discard changes and cancel rebase?"))
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogAction::new()
                                .child(Button::new("rebase-discard").primary().label(t("Discard"))),
                        )
                        .child(
                            DialogClose::new()
                                .trigger(|button| button.label(t("Continue Rebasing"))),
                        ),
                )
                .on_ok(move |_, w, cx| {
                    if owner.update(cx, |s, cx| s.close(cx)).is_ok() {
                        // Explicitly dismiss this confirmation and its plan. Returning
                        // false prevents the generic handler from popping a third dialog.
                        w.close_dialog(cx);
                        w.close_dialog(cx);
                    }
                    false
                })
        });
        false
    }
}
const NOOP: &str = "There are no commits to rebase because the current branch is directly below the base branch, or they point to the same commit. Continue and reset the current branch to the base branch?";
impl Render for PlanDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .w_full()
            .min_w_0()
            .when_some(
                self.table.clone().zip(self.details.clone()),
                |v, (table, details)| {
                    v.child(
                        div().w_full().h(px(550.)).flex_none().child(
                            h_resizable("rebase-plan-split")
                                .with_state(&self.split)
                                .on_resize(cx.listener(
                                    |s, state: &Entity<ResizableState>, _, cx| {
                                        if let [left, right] = state.read(cx).sizes().as_slice() {
                                            let total = left.as_f32() + right.as_f32();
                                            if total > 0. {
                                                s.fraction =
                                                    (left.as_f32() / total).clamp(0.1, 0.9);
                                                if let Err(error) =
                                                    plico::layout_preferences::RebaseSplit::Plan
                                                        .save(s.fraction)
                                                {
                                                    eprintln!(
                                                        "Cannot save rebase plan split: {error}"
                                                    );
                                                }
                                            }
                                        }
                                    },
                                ))
                                .child(
                                    resizable_panel()
                                        .flex_basis(relative(self.fraction))
                                        .size_range(px(180.)..px(2000.))
                                        .child(table),
                                )
                                .child(
                                    resizable_panel()
                                        .flex_basis(relative(1. - self.fraction))
                                        .size_range(px(180.)..px(2000.))
                                        .child(details),
                                ),
                        ),
                    )
                },
            )
            .when(self.table.is_none(), |v| v.child(t(NOOP)))
            .when_some(self.error.clone(), |v, e| {
                v.child(
                    div()
                        .w_full()
                        .text_color(rgb(crate::ui_theme::color(cx, 0xf2a6a6, 0xc54e58)))
                        .child(e),
                )
            })
    }
}

pub fn show(
    repo: Repository,
    project: std::path::PathBuf,
    pending: Pending,
    w: &mut Window,
    cx: &mut App,
) -> Result<Entity<PlanDialog>, String> {
    let draft = match &pending.question {
        Question::Plan(model, _) => Some(Draft::new(model.clone())?),
        Question::Noop => None,
    };
    let dialog = cx.new(|cx| PlanDialog::new(repo, project, pending, draft, cx));
    let result = dialog.clone();
    w.open_dialog(cx, move |frame, _, cx| {
        let confirm = dialog.clone();
        let cancel = dialog.clone();
        let close = dialog.clone();
        let state = dialog.read(cx);
        let plan = state.table.is_some();
        let enabled = state.can_confirm(cx);
        frame
            .title(t(if plan {
                "Rebasing Commits"
            } else {
                "Git Rebase"
            }))
            .w(px(if plan { 1000. } else { 500. }))
            .centered(true)
            .overlay_closable(false)
            .child(dialog.clone())
            .footer(crate::dialog_style::confirmation(
                Button::new("start-interactive-rebase")
                    .primary()
                    .label(t(if plan { "Start Rebasing" } else { "OK" }))
                    .disabled(!enabled),
            ))
            .on_ok(move |_, _, cx| confirm.update(cx, |s, cx| s.confirm(cx)))
            .on_cancel(move |_, w, cx| cancel.update(cx, |s, cx| s.cancel(w, cx)))
            .on_close(move |_, _, cx| close.update(cx, |s, cx| s.close(cx)))
    });
    if let Some(table) = &result.read(cx).table {
        table.read(cx).focus_handle(cx).focus(w, cx);
    }
    Ok(result)
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests;
