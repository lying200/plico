//! Rebased GitOperationActionBase / GitSelectRootDialog (4aec352).
use super::*;
use plico::{merge_operation::Action, operation_state::Operation};

#[derive(Clone, Copy)]
enum RecoveryAction {
    Merge(Action),
    Rebase { abort: bool },
}
struct RootChoice {
    message: &'static str,
    roots: Vec<Repository>,
    selected: usize,
    focus: FocusHandle,
}
impl Render for RootChoice {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let roots = self.roots.clone();
        v_flex()
            .gap_3()
            .child(div().whitespace_normal().child(t(self.message)))
            .child(
                h_flex().gap_2().child(t("Git Root:")).child(
                    Button::new("merge-root")
                        .focus_handle(self.focus.clone())
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .child(project_path_text(&self.roots[self.selected].path)),
                                )
                                .child(jb_icon("chevronDown_dark.svg")),
                        )
                        .dropdown_menu(move |mut menu, _, _| {
                            for (index, root) in roots.iter().enumerate() {
                                let owner = owner.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(project_path_text(&root.path)).on_click(
                                        move |_, _, cx| {
                                            let _ = owner.update(cx, |s, cx| {
                                                s.selected = index;
                                                cx.notify();
                                            });
                                        },
                                    ),
                                );
                            }
                            menu.min_w(px(700.)).max_w(px(740.))
                        }),
                ),
            )
    }
}
impl Review {
    pub(super) fn merge_roots(&self, cx: &App) -> Vec<Repository> {
        self.project_roots
            .read(cx)
            .operations()
            .iter()
            .filter_map(|entry| {
                entry
                    .state
                    .as_ref()
                    .ok()
                    .filter(|state| state.operation == Operation::Merge)
                    .map(|_| entry.repository.clone())
            })
            .collect()
    }
    pub(super) fn project_has_conflicts(&self, cx: &App) -> bool {
        self.project_roots
            .read(cx)
            .operations()
            .iter()
            .any(|entry| {
                entry
                    .state
                    .as_ref()
                    .is_ok_and(|state| !state.conflicts.is_empty())
            })
    }
    pub(super) fn choose_merge_root(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.choose_recovery_root(RecoveryAction::Merge(action), window, cx);
    }
    pub(super) fn rebase_roots(&self, cx: &App) -> Vec<Repository> {
        self.project_roots
            .read(cx)
            .operations()
            .iter()
            .filter_map(|entry| {
                entry
                    .state
                    .as_ref()
                    .ok()
                    .filter(|state| {
                        matches!(
                            state.operation,
                            Operation::RebaseApply | Operation::RebaseMerge
                        )
                    })
                    .map(|_| entry.repository.clone())
            })
            .collect()
    }
    pub(super) fn choose_rebase_root(
        &mut self,
        abort: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.choose_recovery_root(RecoveryAction::Rebase { abort }, window, cx);
    }
    fn choose_recovery_root(
        &mut self,
        action: RecoveryAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.settings_open || self.welcome {
            return;
        }
        let (roots, title, message) = match action {
            RecoveryAction::Merge(action) => (
                self.merge_roots(cx),
                match action {
                    Action::Commit => "Complete Merge",
                    Action::Abort => "Abort Merge",
                },
                "Multiple Git roots have unfinished merge process, please select root to perform action on.",
            ),
            RecoveryAction::Rebase { abort } => (
                self.rebase_roots(cx),
                if abort {
                    "Abort Rebase"
                } else {
                    "Continue Rebase"
                },
                "Multiple Git roots have unfinished rebase process, please select root to perform action on.",
            ),
        };
        if roots.len() == 1 {
            self.apply_recovery_choice(roots[0].clone(), action, window, cx);
            return;
        }
        if roots.is_empty() {
            return;
        }
        let ids = self.commit_ids();
        let context_root = self
            .log_selection
            .lead(&ids)
            .and_then(|index| ids.get(index))
            .map(|id| id.root.as_path())
            .unwrap_or(&self.repo.path);
        let selected = roots
            .iter()
            .position(|r| r.path == context_root)
            .or_else(|| roots.iter().position(|r| r.path == self.repo.path))
            .unwrap_or(0);
        let choice = cx.new(|cx| RootChoice {
            message,
            roots,
            selected,
            focus: cx.focus_handle(),
        });
        let focus = choice.read(cx).focus.clone();
        let owner = cx.entity().downgrade();
        let project = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let choice_ok = choice.clone();
            let owner = owner.clone();
            let project = project.clone();
            dialog
                .title(t(title))
                .centered(true)
                .w(px(760.))
                .overlay_closable(false)
                .child(choice.clone())
                .footer(crate::dialog_style::confirmation(
                    Button::new("select-merge-root").primary().label(t(title)),
                ))
                .on_ok(move |_, window, cx| {
                    let choice = choice_ok.read(cx);
                    let repository = choice.roots[choice.selected].clone();
                    // Open the abort confirmation after this chooser has closed.
                    let owner = owner.clone();
                    let project = project.clone();
                    window.close_dialog(cx);
                    window.defer(cx, move |window, cx| {
                        let _ = owner.update(cx, |s, cx| {
                            if s.repo.path == project {
                                s.apply_recovery_choice(repository, action, window, cx);
                            }
                        });
                    });
                    false
                })
        });
        focus.focus(window, cx);
    }
    fn apply_recovery_choice(
        &mut self,
        repository: Repository,
        action: RecoveryAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            RecoveryAction::Merge(action) => {
                self.apply_merge_choice(repository, action, window, cx)
            }
            RecoveryAction::Rebase { abort } => {
                if !self
                    .rebase_roots(cx)
                    .iter()
                    .any(|r| r.path == repository.path)
                {
                    return;
                }
                if abort {
                    self.confirm_abort_rebase_in(repository, window, cx);
                } else {
                    // Fresh conflict discovery also handles a concurrently resolved index.
                    self.open_rebase_conflicts_in(repository, window, cx);
                }
            }
        }
    }
    fn apply_merge_choice(
        &mut self,
        repository: Repository,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .merge_roots(cx)
            .iter()
            .any(|r| r.path == repository.path)
        {
            return;
        }
        match action {
            Action::Commit if !self.project_has_conflicts(cx) => {
                self.start_merge_operation_in(repository, action, window, cx)
            }
            Action::Commit => {}
            Action::Abort => self.confirm_abort_merge_in(repository, window, cx),
        }
    }
}
