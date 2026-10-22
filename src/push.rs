//! Single ordinary or explicitly requested safe force push attempt, translated from Rebased
//! 4aec35256d765c217aa0d662b82ac4e860c396fd GitPushOperation.doPush/createPushSpec,
//! GitImpl.doPush and GitPushRepoResult.convertType. Apache-2.0, JetBrains et al.
//! Non-fast-forward update/retry belongs to the caller. HTTP authentication may
//! retry only before any successful ref update and while the reviewed source holds.
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
    operation_state::State,
    push_target::Target,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagMode {
    All,
    CurrentBranch,
}
impl TagMode {
    pub fn argument(self) -> &'static str {
        match self {
            Self::All => "--tags",
            Self::CurrentBranch => "--follow-tags",
        }
    }
}
pub struct Spec {
    pub tags: Option<TagMode>,
    /// Explicit user selection. Uses Rebased default safe force-with-lease.
    pub force: bool,
    pub source_branch: String,
    /// Reviewed revision. Reject if the source moved before this attempt.
    pub source_oid: String,
    pub target: Target,
    pub set_upstream: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Forced,
    NewBranch,
    UpToDate,
    RejectedNonFf,
    RejectedStale,
    RejectedOther,
    Failed,
    Cancelled,
    Unknown,
}
pub struct ResultState {
    pub command: Result<Outcome, String>,
    pub status: Status,
    pub state: Result<State, String>,
}

pub fn execute(
    repo: &Repository,
    spec: &Spec,
    token: &Cancel,
    progress: &Progress,
    auth: Option<(&crate::auth_request::Requests, &std::path::Path)>,
) -> Result<ResultState, String> {
    let _guard = repo.operation_guard(token)?;
    let snapshot = repo
        .push_snapshot(token)?
        .ok_or("Attached push source is unavailable")?;
    if snapshot.source_branch != spec.source_branch || snapshot.source_oid != spec.source_oid {
        return Err("Push source changed; review the outgoing commits again".into());
    }
    if !snapshot.targets.remotes.contains(&spec.target.remote) {
        return Err("Push remote is unavailable".into());
    }
    if spec.force && !force_allowed(&spec.target) {
        return Err("Force push is prohibited for this protected branch".into());
    }
    let source = format!("refs/heads/{}", spec.source_branch);
    let destination = spec.target.destination();
    repo.text(&["check-ref-format", &source], token)?;
    repo.text(&["check-ref-format", &destination], token)?;
    // Preserve upstream's branch-ref versus fully qualified target construction.
    let target_ref = if spec.set_upstream {
        destination.as_str()
    } else {
        spec.target.branch.as_str()
    };
    let refspec = format!("{source}:{target_ref}");
    let mut command = repo.operation_command();
    command.args(["push", "--progress", "--porcelain"]);
    if spec.set_upstream {
        command.arg("--set-upstream");
    }
    if let Some(tags) = spec.tags {
        command.arg(tags.argument());
    }
    if spec.force {
        // GitPushOperation.doPush reads the expected tip from local remote refs;
        // do not fetch implicitly or weaken an absent tip into unconditional force.
        let reference = format!("refs/remotes/{}/{}", spec.target.remote, spec.target.branch);
        let refs = repo.text(
            &[
                "for-each-ref",
                "--format=%(refname)%09%(objectname)",
                &reference,
            ],
            token,
        )?;
        let expected = refs
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .find_map(|(name, oid)| (name == reference).then_some(oid))
            .unwrap_or("");
        if !expected.is_empty() && !crate::git::is_oid(expected) {
            return Err("Invalid force-push lease identity".into());
        }
        command.arg(format!("--force-with-lease={destination}:{expected}"));
    }
    // Options before '--' is a local argv-hardening adaptation of GitImpl.doPush.
    command.args(["--", &spec.target.remote, &refspec]);
    let result =
        crate::authenticated_command::run(&mut command, auth, token, progress, None, |outcome| {
            crate::authenticated_command::push_can_retry(outcome)
                && repo.push_snapshot(token).is_ok_and(|current| {
                    current.is_some_and(|current| {
                        current.source_branch == spec.source_branch
                            && current.source_oid == spec.source_oid
                    })
                })
        });
    let status = match &result {
        Ok(outcome) => classify(outcome, &source, &destination, spec.force),
        Err(_) if token.cancelled() => Status::Cancelled,
        Err(_) => Status::Failed,
    };
    let state = repo.operation_state(&Cancel::default());
    Ok(ResultState {
        command: result,
        status,
        state,
    })
}
/// GitSharedSettings default protected branch policy, shared by UI and executor.
pub fn force_allowed(target: &Target) -> bool {
    !matches!(target.branch.as_str(), "main" | "master")
}
fn classify(outcome: &Outcome, source: &str, target: &str, force: bool) -> Status {
    use crate::push_result::{Kind, parse};
    if outcome.stopped == Some(operation_process::StopReason::Cancelled) {
        return Status::Cancelled;
    }
    if outcome.stopped.is_some() || outcome.stdout.truncated {
        return Status::Unknown;
    }
    let bytes = outcome.stdout.bytes();
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Status::Unknown;
    };
    let mut found = false;
    let mut status = Status::UpToDate;
    let mut rejection = None;
    // A remote can have multiple push URLs. A success for one must not hide a
    // rejection at another. Keep the bounded raw output for future diagnostics.
    for result in parse(text).filter(|result| result.source == source && result.target == target) {
        found = true;
        match result.kind {
            Kind::Rejected => {
                let next = if result.is_non_ff_update() {
                    Status::RejectedNonFf
                } else if result.is_stale_info() {
                    Status::RejectedStale
                } else {
                    Status::RejectedOther
                };
                // A later custom rejection must suppress update/retry even if
                // an earlier URL only needed a fast-forward update.
                rejection = Some(match (rejection, next) {
                    (Some(Status::RejectedOther), _) | (_, Status::RejectedOther) => {
                        Status::RejectedOther
                    }
                    (Some(Status::RejectedStale), _) | (_, Status::RejectedStale) => {
                        Status::RejectedStale
                    }
                    _ => Status::RejectedNonFf,
                });
            }
            Kind::NewRef => status = Status::NewBranch,
            Kind::Success if !matches!(status, Status::NewBranch | Status::Forced) => {
                status = Status::Success
            }
            Kind::UpToDate | Kind::Success => {}
            // Only an explicitly requested force operation accepts ForcedUpdate.
            Kind::ForcedUpdate if force => status = Status::Forced,
            Kind::ForcedUpdate | Kind::Deleted => return Status::Unknown,
        }
    }
    if let Some(rejection) = rejection {
        rejection
    } else if !outcome.status.success() {
        Status::Failed
    } else if !found {
        Status::Unknown
    } else if status == Status::UpToDate
        && parse(text).any(|r| {
            r.source.starts_with("refs/tags/") && matches!(r.kind, Kind::NewRef | Kind::Success)
        })
    {
        Status::Success
    } else {
        status
    }
}
