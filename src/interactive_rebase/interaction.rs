//! Operation-owned handoff between the Git editor worker and the plan dialog.
//! The worker retains the document; closing a pending question cancels its reply.
use super::model::{Model, conversion::Item};
use crate::git::Cancel;
use std::{
    sync::{Arc, Mutex, Weak, mpsc},
    time::Duration,
};

type Plan = Model<Arc<Item>>;
pub enum Question {
    Plan(Plan, crate::editor::MessageEncoding),
    Noop,
}
pub enum Answer {
    Plan(Plan),
    Proceed,
}
struct State {
    pending: Option<Pending>,
    waiting: bool,
    closed: bool,
    generation: u64,
}
#[derive(Clone)]
pub struct Requests {
    state: Arc<Mutex<State>>,
    cancel: Cancel,
}
pub struct Pending {
    pub question: Question,
    reply: Option<mpsc::SyncSender<Answer>>,
    state: Weak<Mutex<State>>,
    generation: u64,
    cancel: Cancel,
    stop: Cancel,
}
impl Pending {
    pub fn cancelled(&self) -> bool {
        if self.cancel.cancelled() || self.stop.cancelled() {
            return true;
        }
        let Some(owner) = self.state.upgrade() else {
            return true;
        };
        let state = owner.lock().unwrap_or_else(|e| e.into_inner());
        state.closed || !state.waiting || state.generation != self.generation
    }
    pub fn answer(mut self, answer: Answer) -> Result<(), &'static str> {
        if self.cancelled() {
            return Err("Rebase question has ended");
        }
        if !matches!(
            (&self.question, &answer),
            (Question::Plan(..), Answer::Plan(_)) | (Question::Noop, Answer::Proceed)
        ) {
            return Err("Unexpected rebase answer");
        }
        self.reply
            .take()
            .unwrap()
            .send(answer)
            .map_err(|_| "Rebase question has ended")
    }
}
impl Requests {
    pub fn new(cancel: Cancel) -> Self {
        Self {
            cancel,
            state: Arc::new(Mutex::new(State {
                pending: None,
                waiting: false,
                closed: false,
                generation: 0,
            })),
        }
    }
    pub fn take_pending(&self) -> Option<Pending> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            .take()
    }
    /// Worker-only. None means the dialog was dismissed or its owner shut down.
    pub fn ask(&self, question: Question, stop: &Cancel) -> Result<Option<Answer>, &'static str> {
        if self.cancel.cancelled() || stop.cancelled() {
            return Ok(None);
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let generation = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed {
                return Ok(None);
            }
            if state.waiting {
                return Err("Another rebase question is still active");
            }
            state.generation = state.generation.wrapping_add(1);
            state.waiting = true;
            let generation = state.generation;
            state.pending = Some(Pending {
                question,
                reply: Some(sender),
                state: Arc::downgrade(&self.state),
                generation,
                cancel: self.cancel.clone(),
                stop: stop.clone(),
            });
            generation
        };
        let answer = loop {
            if self.cancel.cancelled() || stop.cancelled() {
                break None;
            }
            if self.state.lock().unwrap_or_else(|e| e.into_inner()).closed {
                break None;
            }
            match receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(answer) => break Some(answer),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.generation == generation {
            state.pending.take();
            state.waiting = false;
        }
        Ok(answer)
    }
    /// Called when the owning operation/window ends, including while a dialog
    /// has already taken its Pending. Its late answer will be rejected.
    pub fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        state.pending.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Instant};
    fn pending(requests: &Requests) -> Pending {
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(pending) = requests.take_pending() {
                return pending;
            }
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn answer_and_dialog_dismissal_release_worker_without_cancelling_operation() {
        let token = Cancel::default();
        let requests = Requests::new(token.clone());
        thread::scope(|s| {
            let worker = s.spawn(|| requests.ask(Question::Noop, &Cancel::default()).unwrap());
            pending(&requests).answer(Answer::Proceed).unwrap();
            assert!(matches!(worker.join().unwrap(), Some(Answer::Proceed)));
            let worker = s.spawn(|| requests.ask(Question::Noop, &Cancel::default()).unwrap());
            drop(pending(&requests));
            assert!(worker.join().unwrap().is_none());
        });
        assert!(!token.cancelled());
    }
    #[test]
    fn closing_owner_or_cancelling_rejects_late_dialog_answers() {
        for close in [false, true] {
            let token = Cancel::default();
            let requests = Requests::new(token.clone());
            thread::scope(|s| {
                let worker = s.spawn(|| requests.ask(Question::Noop, &Cancel::default()).unwrap());
                let dialog = pending(&requests);
                if close {
                    requests.close();
                } else {
                    token.cancel();
                }
                assert!(worker.join().unwrap().is_none());
                assert!(dialog.cancelled());
                assert!(dialog.answer(Answer::Proceed).is_err());
            });
        }
    }
    #[test]
    fn concurrent_questions_do_not_replace_dialog_and_stop_is_respected() {
        let requests = Requests::new(Cancel::default());
        let stop = Cancel::default();
        thread::scope(|s| {
            let worker = s.spawn(|| requests.ask(Question::Noop, &stop).unwrap());
            let dialog = pending(&requests);
            assert!(requests.ask(Question::Noop, &Cancel::default()).is_err());
            stop.cancel();
            assert!(worker.join().unwrap().is_none());
            assert!(dialog.cancelled());
        });
    }
}
