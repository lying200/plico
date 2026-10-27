//! Continuation inside the original conflict-resolution workflow, following
//! GitApplyChangesProcess. A separately opened Continue action must not use this
//! to revive a workflow whose conflict window was cancelled.
use super::*;
use crate::sequencer_operation::{EmptyChoice, EmptyCommit};

impl Results {
    /// Complete the exact stopped Revert, then visit untouched roots. The caller
    /// owns the active interaction lifetime; this does not discover pending work
    /// from a repository or reconstruct the original selection after cancellation.
    /// A failed validation leaves these results intact for reporting/review.
    pub fn resume_revert(
        &mut self,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        self.resume(Kind::Revert, None, editor, requests, token, progress)
    }

    pub fn resume_cherry_pick(
        &mut self,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        self.resume(Kind::CherryPick, None, editor, requests, token, progress)
    }

    pub fn resolve_empty(
        &mut self,
        decision: (&EmptyCommit, EmptyChoice),
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        self.resume(
            Kind::CherryPick,
            Some(decision),
            editor,
            requests,
            token,
            progress,
        )
    }

    fn resume(
        &mut self,
        kind: Kind,
        decision: Option<(&EmptyCommit, EmptyChoice)>,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<(), String> {
        if self.cancelled || token.cancelled() {
            return Err("Cancelled operation requires review before continuing".into());
        }
        let index = self
            .roots
            .iter()
            .position(|root| !root.complete())
            .ok_or("No stopped operation to continue")?;
        if self.roots[index + 1..]
            .iter()
            .any(|root| root.result.is_some())
        {
            return Err("Operation results changed; review before continuing".into());
        }
        let root = &mut self.roots[index];
        let resumed = match (kind, root.result.as_ref()) {
            (Kind::Revert, Some(Ok(Report::Revert(previous)))) => {
                if previous.error.is_some() {
                    return Err("Review the failed Revert before continuing".into());
                }
                let recovery = previous
                    .recovery
                    .as_ref()
                    .ok_or("No stopped Revert to continue")?;
                recovery
                    .resume(&root.repository, editor, requests, token, progress)
                    .map(|mut report| {
                        report
                            .successful
                            .splice(0..0, previous.successful.iter().cloned());
                        report
                            .skipped
                            .splice(0..0, previous.skipped.iter().cloned());
                        Report::Revert(report)
                    })
            }
            (Kind::CherryPick, Some(Ok(Report::CherryPick(previous)))) => {
                let recovery = previous
                    .recovery
                    .as_ref()
                    .ok_or("No stopped cherry-pick to continue")?;
                match decision {
                    Some(decision) => recovery.resolve_empty(
                        &root.repository,
                        decision,
                        editor,
                        requests,
                        token,
                        progress,
                    ),
                    None => recovery.resume(&root.repository, editor, requests, token, progress),
                }
                .map(Report::CherryPick)
            }
            _ => return Err("The stopped operation is not resumable by this workflow".into()),
        };
        self.cancelled = token.cancelled();
        root.result = Some(Ok(resumed?));
        self.cancelled = token.cancelled();
        if !root.complete() || self.cancelled {
            return Ok(());
        }
        for root in &mut self.roots[index + 1..] {
            if token.cancelled() {
                break;
            }
            root.result = Some(match (kind, &root.prepared) {
                (Kind::Revert, Prepared::Revert(prepared)) => prepared
                    .execute(&root.repository, editor, requests, token, progress)
                    .map(Report::Revert),
                (Kind::CherryPick, Prepared::CherryPick(prepared)) => cherry_pick::execute(
                    &root.repository,
                    prepared,
                    editor,
                    requests,
                    token,
                    progress,
                )
                .map(Report::CherryPick),
                _ => Err("Operation kind changed; review the selection".into()),
            });
            if !root.complete() {
                break;
            }
        }
        self.cancelled = token.cancelled();
        Ok(())
    }
}
