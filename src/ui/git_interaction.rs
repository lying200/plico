//! Operation-owned Git prompts. Request routing and modal lifetime stay together;
//! operation-specific result interpretation and refresh remain with the caller.
use super::*;
use plico::{
    active_operation::Id,
    auth_request::{Kind, Requests},
};

pub(super) struct Session {
    id: Id,
    owner: WeakEntity<Review>,
    requests: Requests,
    directories: Vec<PathBuf>,
    authentication: bool,
    editor: Option<Entity<crate::editor_dialog::Editor>>,
    login: Option<Entity<crate::auth_dialog::Login>>,
    plan: Option<Entity<crate::rebase_plan_dialog::PlanDialog>>,
    decision: Option<Entity<crate::update_dialog::UpdateDialog>>,
}
impl Session {
    pub fn new(
        id: Id,
        owner: WeakEntity<Review>,
        requests: Requests,
        directories: Vec<PathBuf>,
    ) -> Self {
        Self {
            id,
            owner,
            requests,
            directories,
            authentication: false,
            editor: None,
            login: None,
            plan: None,
            decision: None,
        }
    }
    pub fn with_authentication(mut self) -> Self {
        self.authentication = true;
        self
    }
    fn current(&self, wx: &mut AsyncWindowContext) -> bool {
        self.owner
            .update(wx, |s, _| s.operation.owns(self.id))
            .unwrap_or(false)
    }
    pub async fn poll(&mut self, wx: &mut AsyncWindowContext) {
        let Some(pending) = self.requests.take_pending() else {
            return;
        };
        if pending.is_cancelled() || !self.current(wx) {
            return;
        }
        if matches!(pending.kind, Kind::CommitEditor | Kind::SequenceEditor) {
            let cancel = pending.cancel_token();
            let directories = self.directories.clone();
            let document = wx
                .background_spawn(async move {
                    plico::editor::Document::open_in_roots(pending, &directories)
                })
                .await;
            // Filesystem work can outlive its operation; recheck before showing UI.
            if cancel.cancelled() || !self.current(wx) {
                return;
            }
            if let Ok(document) = document {
                if let Ok(dialog) = wx.update(|w, cx| crate::editor_dialog::show(document, w, cx)) {
                    self.editor = Some(dialog);
                }
            }
        } else if self.authentication {
            if let Ok(dialog) = wx.update(|w, cx| crate::auth_dialog::show(pending, w, cx)) {
                self.login = Some(dialog);
            }
        }
    }
    pub fn show_plan(
        &mut self,
        pending: plico::interactive_rebase::interaction::Pending,
        repo: Repository,
        project: PathBuf,
        wx: &mut AsyncWindowContext,
    ) {
        if !self.current(wx) {
            return;
        }
        if let Ok(Ok(dialog)) =
            wx.update(|w, cx| crate::rebase_plan_dialog::show(repo, project, pending, w, cx))
        {
            self.plan = Some(dialog);
        }
    }
    pub fn show_decision(
        &mut self,
        pending: plico::update_decision::Pending,
        wx: &mut AsyncWindowContext,
    ) {
        if !self.current(wx) {
            return;
        }
        if let Ok(dialog) = wx.update(|w, cx| crate::update_dialog::show(pending, w, cx)) {
            self.decision = Some(dialog);
        }
    }
    pub fn dismissed(&self, cx: &App) -> bool {
        self.plan.as_ref().is_some_and(|p| p.read(cx).dismissed())
    }
    /// Check operation identity before touching dialogs or unlocking documents.
    pub fn finish(
        &mut self,
        review: &mut Review,
        w: &mut Window,
        cx: &mut Context<Review>,
    ) -> Option<bool> {
        let cancelled = review.finish_operation(self.id, cx)?;
        if self.plan.as_ref().is_some_and(|p| p.read(cx).is_open())
            || self.editor.as_ref().is_some_and(|p| p.read(cx).active())
            || self.login.as_ref().is_some_and(|p| p.read(cx).active())
            || self.decision.as_ref().is_some_and(|p| p.read(cx).active())
        {
            if let Some(plan) = &self.plan {
                plan.update(cx, |p, cx| p.close(cx));
                w.close_all_dialogs(cx);
            } else {
                w.close_dialog(cx);
            }
        }
        Some(cancelled)
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;
    use plico::active_operation::{Kind as Operation, Phase};
    use std::{
        thread,
        time::{Duration, Instant},
    };

    fn pending(requests: &Requests) -> (plico::auth_request::Pending, thread::JoinHandle<()>) {
        let worker_requests = requests.clone();
        let worker = thread::spawn(move || {
            let _ = worker_requests.ask(Kind::Ssh, "Fixture confirmation".into());
        });
        let start = Instant::now();
        loop {
            if let Some(pending) = requests.take_pending() {
                return (pending, worker);
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[gpui::test]
    fn stale_completion_cannot_close_replacement_dialog_or_unlock_new_operation(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = review.unwrap();
        cx.update(|w, cx| {
            view.update(cx, |s, cx| {
                let (id, token) = s
                    .begin_operation(Operation::Pull, Phase::Executing, cx)
                    .unwrap();
                let requests = Requests::new(token.clone());
                let (pending, worker) = pending(&requests);
                let mut old =
                    Session::new(id, view.downgrade(), requests, vec![]).with_authentication();
                old.login = Some(crate::auth_dialog::show(pending, w, cx));
                w.close_dialog(cx);
                assert_eq!(s.finish_operation(id, cx), Some(false));
                let (next, next_token) = s
                    .begin_operation(Operation::Rebase, Phase::Executing, cx)
                    .unwrap();
                w.open_dialog(cx, |dialog, _, _| dialog.title("Replacement"));
                assert_eq!(old.finish(s, w, cx), None);
                assert!(w.has_active_dialog(cx));
                assert!(s.operation.owns(next));
                assert!(!next_token.cancelled());
                token.cancel();
                drop(old);
                worker.join().unwrap();
                w.close_dialog(cx);
                assert_eq!(s.finish_operation(next, cx), Some(false));
            })
        });
    }

    #[gpui::test]
    fn cancelled_owner_closes_its_prompt_only_when_worker_completes(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = review.unwrap();
        cx.update(|w, cx| {
            view.update(cx, |s, cx| {
                let (id, token) = s
                    .begin_operation(Operation::Pull, Phase::Executing, cx)
                    .unwrap();
                let requests = Requests::new(token);
                let (pending, worker) = pending(&requests);
                let mut session =
                    Session::new(id, view.downgrade(), requests, vec![]).with_authentication();
                session.login = Some(crate::auth_dialog::show(pending, w, cx));
                s.operation.request_cancel();
                assert!(s.operation.owns(id));
                assert!(w.has_active_dialog(cx));
                assert_eq!(session.finish(s, w, cx), Some(true));
                assert!(!w.has_active_dialog(cx));
                assert!(!s.operation.busy());
                drop(session);
                worker.join().unwrap();
            })
        });
    }
}
