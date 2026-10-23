//! Selection-scoped availability and explicit message rewrite coordination.
use super::*;
use plico::{
    active_operation::{Kind, Phase},
    reword::Prepared,
};

pub(super) struct Gate {
    pub target: String,
    pub result: Option<Result<Arc<Prepared>, String>>,
    token: Cancel,
    _task: Task<()>,
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Gate {
    pub fn new(repo: Repository, target: String, cx: &mut Context<Self>) -> Self {
        let token = Cancel::default();
        let query = token.clone();
        let id = target.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { Prepared::prepare(&repo, id, &query).map(Arc::new) })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.result = Some(result);
                cx.notify();
            });
        });
        Self {
            target,
            token,
            _task: task,
            result: None,
        }
    }
}
impl Review {
    pub(super) fn refresh_reword_gate(&mut self, cx: &mut Context<Self>) {
        let ids = self.commit_ids();
        let selected = self.log_selection.ordered(&ids);
        self.reword_gate = if selected.len() == 1 {
            Some(cx.new(|cx| Gate::new(self.repo.clone(), selected[0].to_owned(), cx)))
        } else {
            None
        };
    }
    pub(super) fn can_reword(&self, target: &str, cx: &App) -> bool {
        !self.operation.busy()
            && !self.operation.saving_documents()
            && !self.log_query.history_loading()
            && self.log_selection.ordered(&self.commit_ids()).len() == 1
            && self
                .commits
                .iter()
                .find(|c| c.id == target)
                .is_some_and(|c| {
                    c.parents.len() <= 1
                        || self.snapshot.as_ref().and_then(|s| s.head_id.as_deref()) == Some(target)
                })
            && self.reword_gate.as_ref().is_some_and(|g| {
                let g = g.read(cx);
                g.target == target && g.result.as_ref().is_none_or(Result::is_ok)
            })
    }
    pub(super) fn edit_commit_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(gate) = &self.reword_gate else {
            return;
        };
        if !self.can_reword(&gate.read(cx).target, cx) {
            return;
        }
        if let Some(Ok(prepared)) = &gate.read(cx).result {
            self.show_reword(prepared.clone(), window, cx);
            return;
        }
        // Like Rebased, unknown containing-branch information does not make the
        // first context menu inert. Resolve it before showing the editor.
        let target = gate.read(cx).target.clone();
        let repo = self.repo.clone();
        let Some((operation, token)) = self.begin_operation(Kind::Reword, Phase::Preparing, cx)
        else {
            return;
        };
        self.status_message = Some(t("Checking…").into());
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move { Prepared::prepare(&repo, target, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(operation, cx) else {
                    return;
                };
                s.status_message = None;
                if cancelled {
                    return;
                }
                match result {
                    Ok(p) => s.show_reword(Arc::new(p), w, cx),
                    Err(e) => {
                        s.rebase_diagnostic = Some(e);
                        s.status_message = Some(t("Could not edit commit message").into());
                    }
                }
                cx.notify();
            });
        });
    }
    fn show_reword(
        &mut self,
        prepared: Arc<Prepared>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let initial = prepared.initial;
        let owner = cx.entity().downgrade();
        let repo = self.repo.clone();
        let show = move |w: &mut Window, cx: &mut App| {
            let owner = owner.clone();
            let repo = repo.clone();
            let expected = prepared.clone();
            crate::reword_dialog::show(repo, prepared.clone(), w, cx, move |message, w, cx| {
                let _ = owner.update(cx, |s, cx| s.start_reword(expected.clone(), message, w, cx));
            });
        };
        if initial {
            window.open_dialog(cx, move |d, _, _| {
                let show = show.clone();
                d.title(t("Editing Initial Commit")).w(px(470.)).centered(true).overlay_closable(false)
                    .child(t("Editing the initial commit rewrites the history of the current branch. Continue?"))
                    .footer(crate::dialog_style::confirmation(Button::new("reword-root").primary().label(t("Continue"))))
                    .on_ok(move |_, w, cx| { w.close_dialog(cx); show(w, cx); false })
            });
        } else {
            show(window, cx);
        }
    }
    fn start_reword(
        &mut self,
        prepared: Arc<Prepared>,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.welcome || self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        let Some((operation, token)) = self.begin_operation(Kind::Reword, Phase::Executing, cx)
        else {
            return;
        };
        let repo = self.repo.clone();
        self.rebase_diagnostic = None;
        self.status_message = Some(t("Editing commit message…").into());
        let old_target = prepared.target.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    prepared.execute(&repo, &message, &token, &Default::default())
                })
                .await;
            let _ = this.update_in(wx, |s, _, cx| {
                if s.finish_operation(operation, cx).is_none() {
                    return;
                }
                match result {
                    Ok(done) => {
                        s.status_message = Some(
                            t(if done.warning.is_some() {
                                "Commit message edited; follow-up failed"
                            } else {
                                "Commit message edited"
                            })
                            .into(),
                        );
                        s.rebase_diagnostic = done.warning;
                        // Keep the selection on the rewritten target, including older commits.
                        if let Some(i) = s.commits.iter().position(|c| c.id == old_target) {
                            if let Some(c) = Arc::make_mut(&mut s.commits).get_mut(i) {
                                c.id = done.target;
                            }
                            let ids = s.commit_ids();
                            s.log_selection.select(&ids, i, false, false);
                        }
                    }
                    Err(e) => {
                        s.rebase_diagnostic = Some(e);
                        s.status_message = Some(t("Could not edit commit message").into());
                    }
                }
                s.squash_undo = None;
                s.operation_state = None;
                s.refresh_impl(false, true, None, cx);
                cx.notify();
            });
        });
    }
}
