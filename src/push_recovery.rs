//! Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd GitPushOperation.execute,
//! pushingToNotTrackedBranch/pushingNotCurrentBranch, GitUpdateResult.
//! Apache-2.0, JetBrains s.r.o. and contributors.
//! Policy only: no command runs and no user consent is implied by a decision.
use crate::{
    pull::Targets,
    push::{Spec, Status},
    update_session::UpdateResult,
};

pub const MAX_PUSH_ATTEMPTS: usize = 10;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Stop,
    AskToUpdate,
    Update,
}

/// Only an ordinary non-fast-forward rejection needs fresh recovery targets.
/// Keep this gate shared with the policy so terminal results do not launch queries.
pub(crate) fn needs_targets(status: Status, force: bool, attempt: usize) -> bool {
    attempt < MAX_PUSH_ATTEMPTS && status == Status::RejectedNonFf && !force
}

/// `attempt` is zero-based. Subsequent attempts must belong to the same
/// user-authorized recovery loop, not a newly opened Push dialog.
pub fn after_push(
    status: Status,
    force: bool,
    spec: &Spec,
    current: &Targets,
    attempt: usize,
    auto_update: bool,
) -> Decision {
    if !needs_targets(status, force, attempt) {
        return Decision::Stop;
    }
    if current.current_branch.as_deref() != Some(&spec.source_branch) {
        return Decision::Stop;
    }
    let Some(tracked) = &current.tracked else {
        return Decision::Stop;
    };
    // Compare identities, not remote/branch labels joined with '/': both may
    // themselves contain slashes. Special refs aren't a tracked remote branch.
    if spec.target.special
        || tracked.remote != spec.target.remote
        || tracked.branch != spec.target.branch
    {
        return Decision::Stop;
    }
    if attempt == 0 && !auto_update {
        Decision::AskToUpdate
    } else {
        Decision::Update
    }
}

/// Resolved conflicts intentionally stop auto-push, even though GitUpdateResult
/// calls that update successful. The user must review the resolved content.
pub fn retry_after_update(attempt: usize, result: UpdateResult) -> bool {
    attempt < MAX_PUSH_ATTEMPTS - 1
        && matches!(
            result,
            UpdateResult::Success | UpdateResult::NothingToUpdate
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pull,
        push_target::{Origin, Target},
    };
    fn fixture() -> (Spec, Targets) {
        (
            Spec {
                tags: None,
                force: false,
                source_branch: "topic".into(),
                source_oid: "a".repeat(40),
                set_upstream: false,
                target: Target {
                    remote: "origin".into(),
                    branch: "topic".into(),
                    special: false,
                    new_branch: false,
                    origin: Origin::TrackingBranch,
                },
            },
            Targets {
                remotes: vec!["origin".into()],
                branches: vec![],
                current_branch: Some("topic".into()),
                tracked: Some(pull::Target {
                    remote: "origin".into(),
                    branch: "topic".into(),
                }),
            },
        )
    }
    #[test]
    fn tracked_rejection_asks_once_and_force_custom_detached_stop() {
        let (mut spec, mut current) = fixture();
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 0, false),
            Decision::AskToUpdate
        );
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 0, true),
            Decision::Update
        );
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 1, false),
            Decision::Update
        );
        for status in [
            Status::RejectedOther,
            Status::RejectedStale,
            Status::Failed,
            Status::Cancelled,
            Status::Unknown,
            Status::Success,
            Status::Forced,
            Status::NewBranch,
            Status::UpToDate,
        ] {
            assert_eq!(
                after_push(status, false, &spec, &current, 0, true),
                Decision::Stop
            );
        }
        assert_eq!(
            after_push(Status::RejectedNonFf, true, &spec, &current, 0, true),
            Decision::Stop
        );
        spec.target.branch = "other".into();
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 0, true),
            Decision::Stop
        );
        spec.target.branch = "topic".into();
        current.current_branch = Some("other".into());
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 0, true),
            Decision::Stop
        );
        current.current_branch = None;
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 0, true),
            Decision::Stop
        );
        current.current_branch = Some("topic".into());
        current.tracked = None;
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 0, true),
            Decision::Stop
        );
    }
    #[test]
    fn conflicts_stop_and_attempt_limit_matches_upstream_loop() {
        let (spec, current) = fixture();
        for result in [
            UpdateResult::ResolvedConflicts,
            UpdateResult::Incomplete,
            UpdateResult::Cancelled,
            UpdateResult::Error,
            UpdateResult::NotReady,
        ] {
            assert!(!retry_after_update(0, result));
        }
        let mut attempts = 0;
        loop {
            assert_ne!(
                after_push(
                    Status::RejectedNonFf,
                    false,
                    &spec,
                    &current,
                    attempts,
                    true
                ),
                Decision::Stop
            );
            let retry = retry_after_update(attempts, UpdateResult::Success);
            attempts += 1;
            if !retry {
                break;
            }
        }
        assert_eq!(attempts, 10);
        // Upstream still updates after the final rejection but does not push again.
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 9, true),
            Decision::Update
        );
        assert_eq!(
            after_push(Status::RejectedNonFf, false, &spec, &current, 10, true),
            Decision::Stop
        );
        assert!(retry_after_update(0, UpdateResult::NothingToUpdate));
        assert!(!retry_after_update(usize::MAX, UpdateResult::Success));
    }
}
