//! Operation-scoped worker/UI decisions for rejected Push updates.
//! UI behavior: Rebased GitRejectedPushUpdateDialog.kt, reference
//! 4aec35256d765c217aa0d662b82ac4e860c396fd (Apache-2.0).
use crate::{git::Cancel, push_flow::Decisions, pull::Target, update_method::Method};
use std::{sync::mpsc::{self, Receiver, SyncSender, RecvTimeoutError}, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem { No, Yes, Unknown }
impl From<&Result<bool, String>> for Problem {
    fn from(value: &Result<bool, String>) -> Self {
        match value { Ok(false) => Self::No, Ok(true) => Self::Yes, Err(_) => Self::Unknown }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Merge, Rebase, Cancel }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Answer { pub action: Action, pub remember: bool }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    Rejected { default: Method, problem: Problem },
    OverMerge { problem: Problem },
    Published { unknown: bool },
}
impl Prompt {
    /// Order, default action and focus are separate in JetBrains dialogs.
    pub fn default_and_focus(self) -> (Action, Action) {
        match self {
            Self::Rejected { problem: Problem::Yes | Problem::Unknown, .. } => (Action::Merge, Action::Cancel),
            Self::Rejected { default: Method::Rebase, .. } => (Action::Rebase, Action::Merge),
            Self::Rejected { .. } => (Action::Merge, Action::Rebase),
            Self::OverMerge { .. } => (Action::Merge, Action::Rebase),
            Self::Published { .. } => (Action::Cancel, Action::Cancel),
        }
    }
    fn accepts(self, answer: Answer) -> bool {
        (!answer.remember || matches!(self, Self::Rejected { .. }))
            && (!matches!(self, Self::Published { .. }) || answer.action != Action::Merge)
    }
}

pub struct Pending {
    pub prompt: Prompt,
    reply: SyncSender<Answer>,
    cancel: Cancel,
    ended: Cancel,
}
impl Pending {
    pub fn active(&self) -> bool { !self.cancel.cancelled() && !self.ended.cancelled() }
    /// Consumed on submit. Dropping without an answer dismisses the decision.
    /// Invalid/stale answers cannot authorize an operation.
    pub fn answer(self, answer: Answer) -> bool {
        self.active() && self.prompt.accepts(answer) && self.reply.send(answer).is_ok()
    }
}
pub struct Ui { incoming: Receiver<Pending> }
impl Ui {
    pub fn take_pending(&self) -> Option<Pending> {
        self.incoming.try_recv().ok().filter(Pending::active)
    }
}
pub struct Worker {
    outgoing: SyncSender<Pending>, cancel: Cancel, ended: Cancel,
    /// Only a confirmed first choice is exposed for the settings layer.
    pub accepted: Option<Answer>,
    preferences: Option<(std::path::PathBuf, crate::update_preferences::Preferences)>,
    pub persistence_error: Option<String>,
}
pub fn channel(cancel: Cancel) -> (Worker, Ui) {
    let (outgoing, incoming) = mpsc::sync_channel(1);
    (Worker { outgoing, cancel, ended: Cancel::default(), accepted: None, preferences: None, persistence_error: None }, Ui { incoming })
}
impl Drop for Worker { fn drop(&mut self) { self.ended.cancel(); } }
impl Worker {
    /// Configure workspace persistence on the background worker before Push.
    pub fn set_preferences(&mut self, path: std::path::PathBuf, preferences: crate::update_preferences::Preferences) {
        self.preferences = Some((path, preferences));
        self.persistence_error = None;
    }
    fn ask(&mut self, prompt: Prompt) -> Option<Answer> {
        if self.cancel.cancelled() { return None; }
        let (reply, response) = mpsc::sync_channel(1);
        self.outgoing.send(Pending { prompt, reply, cancel: self.cancel.clone(), ended: self.ended.clone() }).ok()?;
        loop {
            if self.cancel.cancelled() { return None; }
            match response.recv_timeout(Duration::from_millis(50)) {
                Ok(answer) => return (!self.cancel.cancelled() && prompt.accepts(answer)).then_some(answer),
                Err(RecvTimeoutError::Disconnected) => return None,
                Err(RecvTimeoutError::Timeout) => {},
            }
        }
    }
}
impl Decisions for Worker {
    fn rejected(&mut self, default: Method, problem: &Result<bool, String>) -> Option<Method> {
        self.accepted = None;
        let answer = self.ask(Prompt::Rejected { default, problem: problem.into() })?;
        let method = match answer.action { Action::Merge => Method::Merge, Action::Rebase => Method::Rebase, Action::Cancel => return None };
        self.accepted = Some(answer);
        if let Some((path, previous)) = &mut self.preferences {
            let next = previous.accepted(answer, problem.into());
            match crate::update_preferences::save(path, next) {
                Ok(()) => { *previous = next; self.persistence_error = None; }
                Err(error) => self.persistence_error = Some(error.to_string()),
            }
        }
        Some(method)
    }
    fn rebase_over_merge(&mut self, problem: &Result<bool, String>) -> Result<Option<Method>, String> {
        match self.ask(Prompt::OverMerge { problem: problem.into() }).map(|a| a.action) {
            Some(Action::Rebase) => Ok(Some(Method::Rebase)),
            Some(Action::Merge) => Ok(Some(Method::Merge)),
            _ => Ok(None),
        }
    }
    fn published(&mut self, problem: &Result<Option<Target>, String>) -> bool {
        self.ask(Prompt::Published { unknown: problem.is_err() }).is_some_and(|a| a.action == Action::Rebase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Instant};
    fn pending(ui: &Ui) -> Pending {
        let start = Instant::now();
        loop {
            if let Some(p) = ui.take_pending() { return p; }
            assert!(start.elapsed() < Duration::from_secs(3));
            thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn default_focus_and_unknown_are_explicit() {
        assert_eq!(Prompt::Rejected { default: Method::Rebase, problem: Problem::No }.default_and_focus(), (Action::Rebase, Action::Merge));
        assert_eq!(Prompt::Rejected { default: Method::Merge, problem: Problem::No }.default_and_focus(), (Action::Merge, Action::Rebase));
        for problem in [Problem::Yes, Problem::Unknown] {
            assert_eq!(Prompt::Rejected { default: Method::Rebase, problem }.default_and_focus(), (Action::Merge, Action::Cancel));
        }
        assert_eq!(Prompt::Published { unknown: false }.default_and_focus(), (Action::Cancel, Action::Cancel));
    }
    #[test]
    fn affirmative_choice_and_remember_return_only_after_answer() {
        let (mut worker, ui) = channel(Cancel::default());
        let task = thread::spawn(move || {
            let result = worker.rejected(Method::Merge, &Ok(false));
            (result, worker.accepted)
        });
        assert!(pending(&ui).answer(Answer { action: Action::Rebase, remember: true }));
        assert_eq!(task.join().unwrap(), (Some(Method::Rebase), Some(Answer { action: Action::Rebase, remember: true })));
    }
    #[test]
    fn dismissal_and_task_cancellation_unblock_worker_and_reject_stale_clicks() {
        for cancelled in [false, true] {
            let token = Cancel::default(); let (mut worker, ui) = channel(token.clone());
            let task = thread::spawn(move || {
                let answer = worker.rejected(Method::Merge, &Ok(false));
                assert!(worker.accepted.is_none()); answer
            });
            let request = pending(&ui);
            if cancelled {
                token.cancel(); assert_eq!(task.join().unwrap(), None);
                assert!(!request.active());
                assert!(!request.answer(Answer { action: Action::Merge, remember: true }));
            } else { drop(request); assert_eq!(task.join().unwrap(), None); }
        }
    }
    #[test]
    fn persistence_runs_only_for_confirmed_choices_and_failure_preserves_action() {
        use crate::update_preferences::{self, Preferences, Strategy};
        let root = std::env::temp_dir().join(format!("gitui-decision-save-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for (index, action) in [Action::Cancel, Action::Merge, Action::Rebase].into_iter().enumerate() {
            let path = root.join(index.to_string());
            let original = Preferences { strategy: Strategy::Rebase, auto_update: false };
            update_preferences::save(&path, original).unwrap();
            let (mut worker, ui) = channel(Cancel::default());
            worker.set_preferences(path.clone(), original);
            let task = thread::spawn(move || {
                let chosen = worker.rejected(Method::Rebase, &Ok(true));
                (chosen, worker.persistence_error.clone())
            });
            assert!(pending(&ui).answer(Answer { action, remember: true }));
            let (chosen, error) = task.join().unwrap(); assert!(error.is_none());
            assert_eq!(chosen, match action { Action::Cancel => None, Action::Merge => Some(Method::Merge), Action::Rebase => Some(Method::Rebase) });
            let actual = update_preferences::load(&path);
            assert_eq!(actual.strategy, Strategy::Rebase); // temporary merge must not replace it
            assert_eq!(actual.auto_update, action != Action::Cancel);
        }
        // An existing directory cannot be overwritten as a preference file.
        let (mut worker, ui) = channel(Cancel::default());
        worker.set_preferences(root.clone(), Preferences::default());
        let task = thread::spawn(move || {
            let chosen = worker.rejected(Method::Merge, &Ok(false));
            (chosen, worker.persistence_error.clone())
        });
        assert!(pending(&ui).answer(Answer { action: Action::Rebase, remember: true }));
        let (chosen, error) = task.join().unwrap();
        assert_eq!(chosen, Some(Method::Rebase)); assert!(error.is_some());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn over_merge_answers_preserve_all_three_choices() {
        for (action, expected) in [(Action::Merge, Some(Method::Merge)),
            (Action::Rebase, Some(Method::Rebase)), (Action::Cancel, None)] {
            let (mut worker, ui) = channel(Cancel::default());
            let task = thread::spawn(move || worker.rebase_over_merge(&Ok(true)));
            assert!(pending(&ui).answer(Answer { action, remember: false }));
            assert_eq!(task.join().unwrap().unwrap(), expected);
        }
    }
    #[test]
    fn ui_disconnect_and_invalid_published_answer_do_not_authorize() {
        let (mut worker, ui) = channel(Cancel::default()); drop(ui);
        assert_eq!(worker.rejected(Method::Merge, &Ok(false)), None);
        let (mut worker, ui) = channel(Cancel::default());
        let task = thread::spawn(move || worker.published(&Err("unknown".into())));
        let request = pending(&ui); assert_eq!(request.prompt, Prompt::Published { unknown: true });
        assert!(!request.answer(Answer { action: Action::Merge, remember: false }));
        assert!(!task.join().unwrap());
    }
}
