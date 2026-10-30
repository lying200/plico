//! GitPushOperation retry sequence, Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd.
//! Apache-2.0, JetBrains s.r.o. and contributors. Single-repository adaptation.
use crate::{
    git::{Cancel, Repository},
    merge_update,
    operation_process::Progress,
    pull::Target,
    push::{self, ResultState, Spec},
    push_recovery::{self, Decision},
    rebase_update,
    update_method::Method,
};

/// Blocking worker callbacks; the caller presents decisions on its UI thread.
/// No callback may silently choose an affirmative answer on cancellation.
pub trait Decisions {
    fn rejected(&mut self, default: Method, over_merge: &Result<bool, String>) -> Option<Method>;
    fn rebase_over_merge(
        &mut self,
        problem: &Result<bool, String>,
    ) -> Result<Option<Method>, String>;
    fn published(&mut self, problem: &Result<Option<Target>, String>) -> bool;
}
pub use crate::update_report::Report as Update;

pub struct Report {
    pub attempts: usize,
    pub push: Option<ResultState>,
    /// Keep only the latest bounded output, not ten sets of subprocess buffers.
    pub update: Option<Update>,
    pub cancelled_decision: bool,
    pub error: Option<String>,
    pub preferences_error: Option<String>,
}

pub fn execute(
    repo: &Repository,
    mut spec: Spec,
    editor: &std::ffi::OsStr,
    token: &Cancel,
    progress: &Progress,
    auth: Option<(&crate::auth_request::Requests, &std::path::Path)>,
    preferences: crate::update_preferences::Preferences,
    protected: crate::published_commits::Protection<'_>,
    decisions: &mut impl Decisions,
) -> Report {
    let mut report = Report {
        attempts: 0,
        push: None,
        update: None,
        cancelled_decision: false,
        error: None,
        preferences_error: None,
    };
    let run = (|| -> Result<(), String> {
        let default_method = || -> Result<Method, String> {
            match preferences.strategy {
                crate::update_preferences::Strategy::BranchDefault => {
                    repo.default_update_method(token)
                }
                _ => Ok(preferences.resolve(Method::Merge)),
            }
        };
        let mut method = None;
        let mut checked_over_merge = false;
        for attempt in 0..push_recovery::MAX_PUSH_ATTEMPTS {
            if token.cancelled() {
                break;
            }
            let result = push::execute(repo, &spec, token, progress, auth)?;
            let status = result.status;
            report.attempts += 1;
            report.push = Some(result);
            if !push_recovery::needs_targets(status, spec.force, attempt) {
                break;
            }
            let current = repo.pull_targets(token)?;
            match push_recovery::after_push(
                status,
                spec.force,
                &spec,
                &current,
                attempt,
                preferences.auto_update,
            ) {
                Decision::Stop => break,
                Decision::AskToUpdate => {
                    let target = current
                        .tracked
                        .as_ref()
                        .ok_or("Push tracking target disappeared")?;
                    let over_merge = repo.rebase_over_merge_problem(target, token);
                    if token.cancelled() {
                        break;
                    }
                    method = decisions.rejected(default_method()?, &over_merge);
                    if method.is_none() {
                        report.cancelled_decision = true;
                        break;
                    }
                    // GitPushOperation folds this warning into its first dialog.
                    checked_over_merge = true;
                }
                Decision::Update => {}
            }
            let selected = match method {
                Some(m) => m,
                None => default_method()?,
            };
            method = Some(selected);
            let request = merge_update::Request {
                branch: spec.source_branch.clone(),
                head: spec.source_oid.clone(),
                target: Target {
                    remote: spec.target.remote.clone(),
                    branch: spec.target.branch.clone(),
                },
                reinstate_index: true,
            };
            let update = match selected {
                Method::Merge => Update::Merge(merge_update::execute(
                    repo, &request, editor, token, progress, auth,
                )?),
                Method::Rebase => {
                    // The callbacks run sequentially within the worker. RefCell
                    // allows both closures to borrow the same UI decision bridge.
                    let decisions = std::cell::RefCell::new(&mut *decisions);
                    Update::Rebase(rebase_update::execute(
                        repo,
                        &request,
                        editor,
                        token,
                        progress,
                        auth,
                        protected,
                        |problem| {
                            if checked_over_merge || matches!(problem, Ok(false)) {
                                Ok(Some(Method::Rebase))
                            } else {
                                decisions.borrow_mut().rebase_over_merge(problem)
                            }
                        },
                        |problem| decisions.borrow_mut().published(problem),
                    )?)
                }
            };
            let retry =
                update.ready() && push_recovery::retry_after_update(attempt, update.result());
            let next_head = update.head().clone();
            report.update = Some(update);
            if !retry || token.cancelled() {
                break;
            }
            // Only an observed successful update authorizes advancing the source
            // identity. The next ordinary push still checks it against the repo.
            spec.source_oid = next_head?;
        }
        Ok(())
    })();
    if let Err(error) = run {
        report.error = Some(error);
    }
    report
}
