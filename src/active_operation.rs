//! UI operation ownership, separate from Git's on-disk merge/rebase state.
//! Requesting cancellation retains ownership until the worker reports completion.
use crate::git::Cancel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Fetch,
    Push,
    Pull,
    Update,
    Checkout,
    Reset,
    Reword,
    Squash,
    UndoSquash,
    Rebase,
    Merge,
    Restore,
    Recovery,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Preparing,
    Executing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id(u64);
struct Running {
    id: Id,
    kind: Kind,
    phase: Phase,
    token: Cancel,
}
#[derive(Default)]
pub struct Lifecycle {
    next: u64,
    running: Option<Running>,
    saving_documents: bool,
}
impl Lifecycle {
    pub fn is_running(&self, kind: Kind) -> bool {
        self.running.as_ref().is_some_and(|running| running.kind == kind)
    }
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }
    pub fn saving_documents(&self) -> bool {
        self.saving_documents
    }
    pub fn begin_save(&mut self) -> bool {
        if self.saving_documents || self.busy() {
            return false;
        }
        self.saving_documents = true;
        true
    }
    pub fn finish_save(&mut self) {
        self.saving_documents = false;
    }
    pub fn begin(&mut self, kind: Kind, phase: Phase) -> Option<(Id, Cancel)> {
        if self.busy() || self.saving_documents {
            return None;
        }
        self.next += 1;
        let id = Id(self.next);
        let token = Cancel::default();
        self.running = Some(Running {
            id,
            kind,
            phase,
            token: token.clone(),
        });
        Some((id, token))
    }
    pub fn can_cancel(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|r| !(r.kind == Kind::Reset && r.phase == Phase::Executing))
    }
    pub fn request_cancel(&self) {
        if self.can_cancel() {
            self.running.as_ref().unwrap().token.cancel();
        }
    }
    /// None means a stale completion; it must not release another operation.
    /// The bool reports a cancellation request, not a rollback or Git outcome.
    pub fn finish(&mut self, id: Id) -> Option<bool> {
        if self.running.as_ref().map(|r| r.id) != Some(id) {
            return None;
        }
        self.running.take().map(|r| r.token.cancelled())
    }
    pub fn documents_locked(&self, local_write: bool) -> bool {
        self.busy() || self.saving_documents || local_write
    }
}
impl Drop for Lifecycle {
    fn drop(&mut self) {
        self.request_cancel();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_keeps_ownership_and_stale_completion_cannot_unlock_new_work() {
        let mut state = Lifecycle::default();
        let (first, token) = state.begin(Kind::Push, Phase::Executing).unwrap();
        assert!(!state.is_running(Kind::Fetch));
        state.request_cancel();
        assert!(token.cancelled());
        assert!(state.documents_locked(false));
        assert!(state.begin(Kind::Checkout, Phase::Executing).is_none());
        assert_eq!(state.finish(first), Some(true));
        let (second, _) = state.begin(Kind::Checkout, Phase::Executing).unwrap();
        assert_eq!(state.finish(first), None);
        assert!(state.busy());
        assert_eq!(state.finish(second), Some(false));
        assert!(!state.documents_locked(false));
    }
    #[test]
    fn fetch_indicator_survives_cancel_request_until_worker_finishes() {
        let mut state = Lifecycle::default();
        let (id, _) = state.begin(Kind::Fetch, Phase::Executing).unwrap();
        assert!(state.is_running(Kind::Fetch));
        state.request_cancel();
        assert!(state.is_running(Kind::Fetch));
        assert!(state.begin(Kind::Fetch, Phase::Executing).is_none());
        assert_eq!(state.finish(id), Some(true));
        assert!(!state.is_running(Kind::Fetch));
    }
    #[test]
    fn reset_preparation_is_cancellable_execution_is_not() {
        let mut state = Lifecycle::default();
        let (id, token) = state.begin(Kind::Reset, Phase::Preparing).unwrap();
        state.request_cancel();
        assert!(token.cancelled());
        state.finish(id);
        let (_, execution) = state.begin(Kind::Reset, Phase::Executing).unwrap();
        assert!(!state.can_cancel());
        state.request_cancel();
        drop(state);
        assert!(!execution.cancelled());
    }
    #[test]
    fn save_gate_and_local_write_keep_documents_locked() {
        let mut state = Lifecycle::default();
        assert!(state.begin_save());
        assert!(!state.begin_save());
        assert!(state.begin(Kind::Reset, Phase::Executing).is_none());
        assert!(state.documents_locked(false));
        state.finish_save();
        assert!(state.documents_locked(true));
        assert!(!state.documents_locked(false));
        assert!(state.begin(Kind::Reset, Phase::Executing).is_some());
    }
}
