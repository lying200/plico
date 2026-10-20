//! Local-view rollback lifecycle; Review supplies the shared save/lock gate.
use super::*;
use plico::rollback_operation;
use std::{collections::BTreeSet, sync::Arc};
pub struct RequestRollback;
pub struct ExecuteRollback {
    pub prepared: Arc<rollback_operation::Prepared>,
    pub selected: BTreeSet<usize>,
    pub delete_added: bool,
}
impl EventEmitter<RequestRollback> for LocalView {}
impl EventEmitter<ExecuteRollback> for LocalView {}
impl LocalView {
    pub(super) fn can_rollback(&self, cx: &App) -> bool {
        !self.writing()
            && !self.refresh.read(cx).busy()
            && self.error.is_none()
            && !self.recovery
            && self
                .model
                .files()
                .iter()
                .any(|f| f.area == plico::git::Area::WorkingTree)
    }
    pub(super) fn request_rollback(&self, cx: &mut Context<Self>) {
        if self.can_rollback(cx) {
            cx.emit(RequestRollback);
        }
    }
    pub fn prepare_rollback_saved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.writing() {
            return;
        }
        let scope = self
            .rollback_scope
            .clone()
            .filter(|(area, _)| *area == plico::git::Area::WorkingTree);
        let repo = self.repo.clone();
        let token = Cancel::default();
        self.write_token = Some(token.clone());
        self.notice = Some(t("Checking changes…").into());
        self.write_error = None;
        self.write_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move { rollback_operation::prepare(&repo, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                s.write_token = None;
                s.notice = None;
                match result {
                    Err(details) => {
                        s.write_error = Some(WriteFailure {
                            summary: "Could not prepare rollback",
                            details,
                        })
                    }
                    Ok(prepared) => {
                        let selected = prepared
                            .changes
                            .iter()
                            .enumerate()
                            .filter(|(_, c)| {
                                scope.as_ref().is_none_or(|(area, paths)| {
                                    *area == c.area && paths.contains(&c.path)
                                })
                            })
                            .map(|(i, _)| i)
                            .collect();
                        let prepared = Arc::new(prepared);
                        let submit = prepared.clone();
                        let owner = cx.entity().downgrade();
                        crate::rollback_dialog::show(
                            s.repo.path.clone(),
                            prepared,
                            selected,
                            w,
                            cx,
                            move |selected, delete_added, _, cx| {
                                let _ = owner.update(cx, |_, cx| {
                                    cx.emit(ExecuteRollback {
                                        prepared: submit.clone(),
                                        selected,
                                        delete_added,
                                    })
                                });
                            },
                        );
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    pub fn rollback_saved(
        &mut self,
        event: &ExecuteRollback,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.writing() {
            return;
        }
        let repo = self.repo.clone();
        let prepared = event.prepared.clone();
        let selected = event.selected.clone();
        let delete = event.delete_added;
        // This token only marks ownership. The confirmed Git sequence is non-cancellable.
        self.write_token = Some(Cancel::default());
        self.rolling_back = true;
        self.notice = Some(t("Rolling back changes…").into());
        self.write_error = None;
        self.write_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    rollback_operation::execute(&repo, &prepared, &selected, delete)
                })
                .await;
            let _ = this.update_in(wx, |s, _, cx| {
                s.write_token = None;
                s.rolling_back = false;
                match result {
                    Ok(()) => s.notice = Some(t("Changes rolled back").into()),
                    Err(details) => {
                        s.notice = None;
                        s.write_error = Some(WriteFailure {
                            summary: "Rollback failed",
                            details,
                        });
                    }
                }
                cx.emit(Changed { push: false });
                cx.notify();
            });
        });
        cx.notify();
    }
}
