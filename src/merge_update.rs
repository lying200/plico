//! Single-repository GitUpdateProcess(MERGE) sequence, Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd.
//! Apache-2.0, JetBrains s.r.o. and contributors. No push or automatic retry here.
use crate::{git::{Repository, Cancel}, pull::Target, fetch, operation_process::{Outcome, Progress},
    operation_state::State, update_session::{Session, MergeStep, UpdateResult}};

pub struct Request {
    pub branch: String,
    pub head: String,
    pub target: Target,
    pub reinstate_index: bool,
}
pub struct Report {
    pub result: UpdateResult,
    pub fetch: Option<fetch::BatchResult>,
    pub save: Option<Result<Outcome, String>>,
    pub merge: Option<Result<MergeStep, String>>,
    pub restore: Option<Result<Option<Outcome>, String>>,
    pub saved_oid: Option<String>,
    pub error: Option<String>,
    pub head: Result<String, String>,
    pub state: Result<State, String>,
}
impl Report {
    pub fn ready_to_push(&self) -> bool {
        matches!(self.result, UpdateResult::Success | UpdateResult::NothingToUpdate)
            && self.error.is_none() && self.saved_oid.is_none()
            && self.restore.as_ref().is_none_or(|r| r.as_ref().is_ok_and(|r|
                r.as_ref().is_none_or(|out| out.status.success() && out.stopped.is_none())))
            && self.head.is_ok() && self.state.as_ref().is_ok_and(|s| !s.detached && !s.in_progress() && s.conflicts.is_empty())
    }
}
pub(crate) fn validate(repo: &Repository, request: &Request, token: &Cancel) -> Result<(), String> {
    let targets = repo.pull_targets(token)?;
    if targets.current_branch.as_deref() != Some(&request.branch) || targets.tracked.as_ref() != Some(&request.target) {
        return Err("Update branch or tracking target changed".into());
    }
    validate_source(repo, request, token)
}

/// Pull explicitly selects a remote branch; it need not be the upstream.
/// Keep Push's tracking requirement in `validate` above.
pub(crate) fn validate_selected(repo: &Repository, request: &Request, token: &Cancel) -> Result<(), String> {
    let targets = repo.pull_targets(token)?;
    if targets.current_branch.as_deref() != Some(&request.branch)
        || !targets.contains(&request.target.remote, &request.target.branch) {
        return Err("Pull source or selected target changed".into());
    }
    validate_source(repo, request, token)
}

fn validate_source(repo: &Repository, request: &Request, token: &Cancel) -> Result<(), String> {
    if repo.text(&["rev-parse", "--verify", "HEAD"], token)?.trim() != request.head {
        return Err("Update source changed; review again".into());
    }
    let state = repo.operation_state(token)?;
    if state.in_progress() || !state.conflicts.is_empty() { return Err("An unfinished operation prevents update".into()); }
    Ok(())
}

/// Capture the local remote-ref inventory after Fetch. This is a local stale
/// decision guard, not a lock against other Git processes or an ABA detector.
pub(crate) fn remote_refs(repo: &Repository, token: &Cancel) -> Result<String, String> {
    repo.text(&["for-each-ref", "--sort=refname", "--format=%(refname) %(objectname)", "refs/remotes/"], token)
}
pub(crate) fn validate_remote_refs(repo: &Repository, expected: &str, token: &Cancel) -> Result<(), String> {
    if remote_refs(repo, token)? != expected {
        return Err("Remote references changed during update; review again".into());
    }
    Ok(())
}

/// Worker-only, after an explicit Merge decision. Caller services editor/auth
/// requests while this runs and must refresh even on an error result.
pub fn execute(repo: &Repository, request: &Request, editor: &std::ffi::OsStr,
    token: &Cancel, progress: &Progress, auth: Option<(&crate::auth_request::Requests, &std::path::Path)>) -> Result<Report, String> {
    validate(repo, request, token)?;
    if editor.is_empty() { return Err("Merge editor is unavailable".into()); }
    let spec = fetch::Spec::new(request.target.remote.clone())?;
    let fetched = match auth {
        Some((requests, helper)) => fetch::execute_authenticated(repo, &[spec], token, progress, requests, helper),
        None => fetch::execute(repo, &[spec], token, progress),
    };
    let mut report = Report { result: UpdateResult::NotReady, fetch: None, save: None, merge: None,
        restore: None, saved_oid: None, error: None, head: Err("Not refreshed".into()), state: Err("Not refreshed".into()) };
    match fetched {
        Ok(batch) => { let success = batch.successful(); report.fetch = Some(batch);
            if success {
                // Fetch owns its own queue guard. Acquire the update session
                // only after it returns, then revalidate before any local writes.
                let updated = (|| -> Result<(), String> {
                    let mut session = Session::begin(repo, token)?;
                    validate(repo, request, token)?;
                    let references = remote_refs(repo, token)?;
                    let result = (|| -> Result<UpdateResult, String> {
                        let range = format!("HEAD..refs/remotes/{}/{}", request.target.remote, request.target.branch);
                        if repo.text(&["rev-list", "-1", &range, "--"], token)?.trim().is_empty() {
                            return Ok(UpdateResult::NothingToUpdate);
                        }
                        if repo.merge_update_save_needed(&request.target, token)? {
                            report.save = Some(session.save("Git Review: update by merge", token, progress));
                            if !report.save.as_ref().unwrap().as_ref().is_ok_and(|out| out.status.success() && out.stopped.is_none()) {
                                return Err("Could not save local changes".into());
                            }
                        }
                        validate(repo, request, token)?;
                        validate_remote_refs(repo, &references, token)?;
                        report.merge = Some(session.merge_with_requests(&request.target, editor, token, progress,
                            auth.map(|(requests, _)| requests)));
                        match report.merge.as_ref().unwrap() {
                            Ok(step) => Ok(step.result()), Err(error) => Err(error.clone()),
                        }
                    })();
                    report.result = match result { Ok(result) => result, Err(error) => {
                        report.error = Some(error); if token.cancelled() { UpdateResult::Cancelled } else { UpdateResult::Error }
                    } };
                    report.restore = Some(session.finish(report.result, request.reinstate_index, progress));
                    report.saved_oid = session.saved_oid().map(str::to_owned);
                    Ok(())
                })();
                if let Err(error) = updated { report.error = Some(error); }
            }
        }
        Err(error) => report.error = Some(error),
    }
    let refresh = Cancel::default();
    report.head = repo.text(&["rev-parse", "--verify", "HEAD"], &refresh).map(|s| s.trim().to_owned());
    report.state = repo.operation_state(&refresh);
    Ok(report)
}
