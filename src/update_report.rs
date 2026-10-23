//! Shared result of a preserving merge/rebase update.
//! Update Project and rejected-push recovery use the same completion rules;
//! their consent, execution and retry policies remain in their own modules.
pub enum Report {
    Merge(crate::merge_update::Report),
    Rebase(crate::rebase_update::Report),
}
impl Report {
    pub fn result(&self) -> crate::update_session::UpdateResult {
        match self {
            Self::Merge(r) => r.result,
            Self::Rebase(r) => r.result,
        }
    }
    /// Success includes restoration of saved changes, not merely a successful
    /// merge/rebase command. A restore conflict must remain visible to the UI.
    pub fn complete(&self) -> bool {
        match self {
            Self::Merge(r) => r.ready_to_push(),
            Self::Rebase(r) => r.ready_to_push(),
        }
    }
    pub fn saved_oid(&self) -> Option<&str> {
        match self {
            Self::Merge(r) => r.saved_oid.as_deref(),
            Self::Rebase(r) => r.saved_oid.as_deref(),
        }
    }
    /// The push-recovery spelling of the same restoration-complete condition.
    pub fn ready(&self) -> bool {
        self.complete()
    }
    pub fn cancelled(&self) -> bool {
        self.result() == crate::update_session::UpdateResult::Cancelled
    }
    pub fn has_saved_changes(&self) -> bool {
        self.saved_oid().is_some()
    }
    pub(crate) fn head(&self) -> &Result<String, String> {
        match self {
            Self::Merge(r) => &r.head,
            Self::Rebase(r) => &r.head,
        }
    }
}
