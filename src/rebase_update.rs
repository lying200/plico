//! GitUpdateProcess / GitRebaseUpdater, Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd.
//! Apache-2.0, JetBrains s.r.o. and contributors. Single-repository worker.
use crate::{git::{Cancel, Repository}, merge_update::{Request, validate, remote_refs, validate_remote_refs}, pull::Target,
    fetch, operation_process::{Outcome, Progress, StopReason}, operation_state::State,
    update_session::{Session, FastForward, RebaseStep, MergeStep, UpdateResult}};

use crate::update_method::Method;

pub struct Report {
    pub result: UpdateResult,
    pub fetch: Option<fetch::BatchResult>,
    pub fast_forward: Option<FastForward>,
    pub save: Option<Result<Outcome, String>>,
    pub rebase: Option<RebaseStep>,
    pub merge: Option<MergeStep>,
    pub restore: Option<Result<Option<Outcome>, String>>,
    pub saved_oid: Option<String>,
    pub published: Option<Result<Option<Target>, String>>,
    pub over_merge: Option<Result<bool, String>>,
    pub error: Option<String>,
    pub head: Result<String, String>,
    pub state: Result<State, String>,
}
impl Report {
    pub fn ready_to_push(&self) -> bool {
        matches!(self.result, UpdateResult::Success | UpdateResult::NothingToUpdate)
            && self.error.is_none() && self.saved_oid.is_none() && self.head.is_ok()
            && self.state.as_ref().is_ok_and(|s| !s.detached && !s.in_progress() && s.conflicts.is_empty())
            && self.restore.as_ref().is_none_or(|r| r.as_ref().is_ok_and(|r|
                r.as_ref().is_none_or(|out| out.status.success() && out.stopped.is_none())))
    }
}

/// Worker after explicit Rebase selection. `before_save` is a REQUIRED caller
/// gate receiving the rebase-over-merge query (including unknown errors).
/// None cancels; Merge switches the updater in this session. It runs after
/// fetch/fast-forward and before any stash, as GitUpdateProcess does.
/// `confirm_published` services the separate warning AFTER saving, as upstream.
/// Neither callback may invoke another queue-owning operation; UI decisions
/// must remain cancellation-aware. Protection rules are resolved after Fetch at the published-commit gate.
pub fn execute(repo: &Repository, request: &Request, editor: &std::ffi::OsStr,
    token: &Cancel, progress: &Progress, auth: Option<(&crate::auth_request::Requests, &std::path::Path)>,
    protected: crate::published_commits::Protection<'_>, before_save: impl FnMut(&Result<bool, String>) -> Result<Option<Method>, String>,
    confirm_published: impl FnMut(&Result<Option<Target>, String>) -> bool) -> Result<Report, String> {
    execute_validated(repo, request, editor, token, progress, auth, protected, before_save, confirm_published, validate)
}

/// Explicit GitPull REBASE route: preserve the selected remote branch even
/// when upstream is absent or points elsewhere. Does not change tracking config.
pub fn execute_selected(repo: &Repository, request: &Request, editor: &std::ffi::OsStr,
    token: &Cancel, progress: &Progress, auth: Option<(&crate::auth_request::Requests, &std::path::Path)>,
    protected: crate::published_commits::Protection<'_>, before_save: impl FnMut(&Result<bool, String>) -> Result<Option<Method>, String>,
    confirm_published: impl FnMut(&Result<Option<Target>, String>) -> bool) -> Result<Report, String> {
    execute_validated(repo, request, editor, token, progress, auth, protected, before_save, confirm_published, crate::merge_update::validate_selected)
}

fn execute_validated(repo: &Repository, request: &Request, editor: &std::ffi::OsStr,
    token: &Cancel, progress: &Progress, auth: Option<(&crate::auth_request::Requests, &std::path::Path)>,
    protected: crate::published_commits::Protection<'_>, mut before_save: impl FnMut(&Result<bool, String>) -> Result<Option<Method>, String>,
    mut confirm_published: impl FnMut(&Result<Option<Target>, String>) -> bool,
    validate: fn(&Repository, &Request, &Cancel) -> Result<(), String>) -> Result<Report, String> {
    validate(repo, request, token)?;
    if editor.is_empty() { return Err("Rebase editor is unavailable".into()); }
    let spec = fetch::Spec::new(request.target.remote.clone())?;
    let fetched = match auth {
        Some((requests, helper)) => fetch::execute_authenticated(repo, &[spec], token, progress, requests, helper),
        None => fetch::execute(repo, &[spec], token, progress),
    };
    let mut report = Report { result: UpdateResult::NotReady, fetch: None, fast_forward: None,
        save: None, rebase: None, merge: None, restore: None, saved_oid: None, published: None, over_merge: None, error: None,
        head: Err("Not refreshed".into()), state: Err("Not refreshed".into()) };
    match fetched {
        Ok(batch) => {
            let success = batch.successful(); report.fetch = Some(batch);
            if success {
                let updated = (|| -> Result<(), String> {
                    let mut session = Session::begin(repo, token)?;
                    validate(repo, request, token)?;
                    let references = remote_refs(repo, token)?;
                    let result = (|| -> Result<UpdateResult, String> {
                        let range = format!("HEAD..refs/remotes/{}/{}", request.target.remote, request.target.branch);
                        if repo.text(&["rev-list", "-1", &range, "--"], token)?.trim().is_empty() {
                            return Ok(UpdateResult::NothingToUpdate);
                        }
                        // If the change query fails, leave optimization alone and
                        // conservatively attempt saving below (upstream isSaveNeeded).
                        if repo.has_tracked_changes(token).unwrap_or(false) {
                            let ff = session.try_fast_forward(&request.target, token, progress)?;
                            let cancelled = ff.outcome.stopped == Some(StopReason::Cancelled);
                            let stopped = ff.outcome.stopped.is_some();
                            let success = ff.outcome.status.success();
                            report.fast_forward = Some(ff);
                            if cancelled { return Ok(UpdateResult::Cancelled); }
                            if stopped { return Ok(UpdateResult::Error); }
                            if success { return Ok(UpdateResult::Success); }
                        }
                        if token.cancelled() { return Ok(UpdateResult::Cancelled); }
                        report.over_merge = Some(repo.rebase_over_merge_problem(&request.target, token));
                        if token.cancelled() { return Ok(UpdateResult::Cancelled); }
                        let Some(method) = before_save(report.over_merge.as_ref().unwrap())? else { return Ok(UpdateResult::Cancelled); };
                        // A UI decision may have waited while another application
                        // changed HEAD/tracking; do not save/rebase a new selection.
                        validate(repo, request, token)?;
                        validate_remote_refs(repo, &references, token)?;
                        let needs_save = match method {
                            Method::Merge => repo.merge_update_save_needed(&request.target, token)?,
                            Method::Rebase => repo.has_tracked_changes(token).unwrap_or(true),
                        };
                        if token.cancelled() { return Ok(UpdateResult::Cancelled); }
                        if needs_save {
                            report.save = Some(session.save(match method { Method::Merge => "Git Review: update by merge", Method::Rebase => "Git Review: update by rebase" }, token, progress));
                            if !report.save.as_ref().unwrap().as_ref().is_ok_and(|out| out.status.success() && out.stopped.is_none()) {
                                return Err("Could not save local changes".into());
                            }
                        }
                        if method == Method::Merge {
                            validate(repo, request, token)?;
                            validate_remote_refs(repo, &references, token)?;
                            let step = session.merge_with_requests(&request.target, editor, token, progress,
                                auth.map(|(requests, _)| requests))?;
                            let result = step.result();
                            report.merge = Some(step);
                            return Ok(result);
                        }
                        report.published = Some(protected.check(repo, &request.target, token));
                        if token.cancelled() { return Ok(UpdateResult::Cancelled); }
                        let published = report.published.as_ref().unwrap();
                        if !matches!(published, Ok(None)) && !confirm_published(published) {
                            return Ok(UpdateResult::Cancelled);
                        }
                        validate(repo, request, token)?;
                        validate_remote_refs(repo, &references, token)?;
                        let step = session.rebase_with_requests(&request.target, editor, token, progress,
                            auth.map(|(requests, _)| requests))?;
                        let result = if step.outcome.stopped == Some(StopReason::Cancelled) { UpdateResult::Cancelled }
                            else if step.outcome.stopped.is_some() { UpdateResult::Error }
                            else { match &step.state {
                                Err(_) => UpdateResult::Error,
                                Ok(s) if s.in_progress() || !s.conflicts.is_empty() => UpdateResult::Incomplete,
                                Ok(_) if step.outcome.status.success() => UpdateResult::Success,
                                Ok(_) => UpdateResult::Error,
                            } };
                        report.rebase = Some(step);
                        Ok(result)
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
