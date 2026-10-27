//! Multi-root update, following GitUpdateProcess's common preservation boundary.
use super::{
    preservation::{SavedChanges, SavedRoot},
    roots::{Branch, Roots, Skipped},
};
use crate::{
    auth_request::Requests,
    fetch,
    git::{Cancel, Repository},
    merge_update,
    operation_process::{Outcome, Progress, StopReason},
    published_commits::Protection,
    submodule_update::Update,
    update_method::Method,
    update_session::{FastForward, MergeStep, RebaseStep, Session, UpdateResult},
};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub struct Group {
    branches: Vec<Branch>,
    detached: Vec<Update>,
    skipped: Vec<Skipped>,
}
pub struct RootResult {
    pub path: PathBuf,
    pub result: UpdateResult,
    pub merge: Option<Result<MergeStep, String>>,
    pub rebase: Option<Result<RebaseStep, String>>,
    pub fast_forward: Option<FastForward>,
    pub submodule: Option<Result<Outcome, String>>,
}
pub struct Report {
    pub roots: Vec<RootResult>,
    pub fetches: Vec<(PathBuf, Result<fetch::BatchResult, String>)>,
    pub saved: Vec<SavedRoot>,
    pub skipped: Vec<Skipped>,
    pub error: Option<String>,
    cancelled: bool,
}
pub fn diagnostic(result: &Result<Report, String>) -> Option<String> {
    match result {
        Ok(report) => report.diagnostic(),
        Err(error) => Some(crate::rebase_operation::display_text(error)),
    }
}
impl Report {
    /// Bounded, credential-redacted diagnostics for the existing error/details
    /// surface. Preserve root names so a child's failure is not blamed on HEAD.
    pub fn diagnostic(&self) -> Option<String> {
        let mut text = String::new();
        let mut add = |path: Option<&Path>, message: &str| {
            let message = crate::rebase_operation::display_text(message);
            if message.trim().is_empty() || text.len() >= 16 * 1024 {
                return;
            }
            let prefix = path
                .map(|p| format!("{}: ", p.display()))
                .unwrap_or_default();
            let line = crate::rebase_operation::display_text(&format!("{prefix}{message}"));
            text.extend(line.chars().take(4096));
            text.push('\n');
        };
        if let Some(error) = &self.error {
            add(None, error);
        }
        for (path, fetch) in &self.fetches {
            match fetch {
                Err(error) => add(Some(path), error),
                Ok(batch) => {
                    for remote in &batch.remotes {
                        if remote.state != fetch::RemoteState::Succeeded {
                            add(Some(path), &String::from_utf8_lossy(&remote.diagnostic));
                        }
                    }
                }
            }
        }
        for root in &self.roots {
            if matches!(
                root.result,
                UpdateResult::Success
                    | UpdateResult::NothingToUpdate
                    | UpdateResult::ResolvedConflicts
            ) {
                continue;
            }
            add(Some(&root.path), &format!("{:?}", root.result));
            for output in [
                root.merge.as_ref().map(|r| r.as_ref().map(|s| &s.outcome)),
                root.rebase.as_ref().map(|r| r.as_ref().map(|s| &s.outcome)),
                root.submodule.as_ref().map(Result::as_ref),
            ]
            .into_iter()
            .flatten()
            {
                match output {
                    Err(error) => add(Some(&root.path), error),
                    Ok(out) => add(
                        Some(&root.path),
                        &crate::rebase_operation::collect_errors(
                            &String::from_utf8_lossy(&out.stdout.bytes()),
                            &String::from_utf8_lossy(&out.stderr.bytes()),
                        ),
                    ),
                }
            }
        }
        for saved in &self.saved {
            match &saved.save {
                Err(error) => add(Some(&saved.path), error),
                Ok(out) if !good(out) => add(
                    Some(&saved.path),
                    &crate::rebase_operation::collect_errors(
                        &String::from_utf8_lossy(&out.stdout.bytes()),
                        &String::from_utf8_lossy(&out.stderr.bytes()),
                    ),
                ),
                _ => {}
            }
            if let Some(restore) = &saved.restore {
                match restore {
                    Err(error) => add(Some(&saved.path), error),
                    Ok(Some(out)) if !good(out) => add(
                        Some(&saved.path),
                        &crate::rebase_operation::collect_errors(
                            &String::from_utf8_lossy(&out.stdout.bytes()),
                            &String::from_utf8_lossy(&out.stderr.bytes()),
                        ),
                    ),
                    _ => {}
                }
            }
        }
        for skipped in &self.skipped {
            match skipped.reason {
                super::roots::SkipReason::NoUpstream => add(
                    Some(&skipped.path),
                    "No tracked remote branch; repository was not updated",
                ),
                super::roots::SkipReason::DetachedHead => add(
                    Some(&skipped.path),
                    "Detached HEAD; repository was not updated",
                ),
                super::roots::SkipReason::Uninitialized => {}
            }
        }
        (!text.is_empty()).then(|| crate::rebase_operation::display_text(&text))
    }
    pub fn has_saved_changes(&self) -> bool {
        self.saved.iter().any(|r| r.saved_oid.is_some())
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled
            || self
                .roots
                .iter()
                .any(|r| r.result == UpdateResult::Cancelled)
    }
    pub fn result(&self) -> UpdateResult {
        if self
            .fetches
            .iter()
            .any(|(_, r)| !r.as_ref().is_ok_and(|b| b.successful()))
        {
            return UpdateResult::NotReady;
        }
        let initial = if self.cancelled() {
            UpdateResult::Cancelled
        } else if self.error.is_some() {
            UpdateResult::Error
        } else {
            UpdateResult::NothingToUpdate
        };
        self.roots
            .iter()
            .fold(initial, |result, root| result.join(root.result))
    }
    pub fn complete(&self) -> bool {
        !self.cancelled()
            && self.error.is_none()
            && self
                .fetches
                .iter()
                .all(|(_, r)| r.as_ref().is_ok_and(|b| b.successful()))
            && self.roots.iter().all(|r| {
                matches!(
                    r.result,
                    UpdateResult::Success
                        | UpdateResult::NothingToUpdate
                        | UpdateResult::ResolvedConflicts
                )
            })
            && self.saved.iter().all(|r| {
                r.saved_oid.is_none()
                    && r.restore.as_ref().is_some_and(|out| {
                        out.as_ref().is_ok_and(|out| out.as_ref().is_none_or(good))
                    })
            })
    }
}
impl Group {
    /// Capture all project mappings, revalidating their actual Git roots before
    /// preparing requests. No primary-root privilege or fallback upstream.
    pub fn prepare_registered(registered: &[Repository], token: &Cancel) -> Result<Self, String> {
        let inventory =
            crate::project_repositories::Repositories::from_registered(registered, token)?;
        let roots = Roots::from_inventory(&inventory, token)?;
        if roots.branches.is_empty() {
            return Err("No project repository has an attached branch with an existing tracked remote branch".into());
        }
        Self::from_roots(roots, token)
    }

    fn from_roots(roots: Roots, token: &Cancel) -> Result<Self, String> {
        let detached = roots
            .detached
            .into_iter()
            .map(|root| Update::prepare(&root.parent, &root.path, token))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            branches: roots.branches,
            detached,
            skipped: roots.skipped,
        })
    }

    pub fn prepare(
        repo: &Repository,
        request: merge_update::Request,
        token: &Cancel,
    ) -> Result<Self, String> {
        merge_update::validate(repo, &request, token)?;
        let Roots {
            branches,
            detached,
            skipped,
        } = Roots::discover(repo, token)?;
        let mut all = vec![Branch {
            repository: repo.clone(),
            request,
        }];
        all.extend(branches);
        Self::from_roots(
            Roots {
                branches: all,
                detached,
                skipped,
            },
            token,
        )
    }

    /// Capture before starting the worker. Detached checkout never needs an
    /// editor; ordinary branch updaters can each request their own Git files.
    pub fn editor_directories(&self, token: &Cancel) -> Result<Vec<PathBuf>, String> {
        self.branches
            .iter()
            .map(|branch| branch.repository.editor_directory(token))
            .collect()
    }

    pub fn execute_merge(
        self,
        editor: &OsStr,
        token: &Cancel,
        progress: &Progress,
        auth: Option<(&Requests, &Path)>,
    ) -> Result<Report, String> {
        self.execute(
            Method::Merge,
            editor,
            token,
            progress,
            auth,
            Protection::Defaults,
            |_| Ok(Some(Method::Merge)),
            |_| false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        self,
        method: Method,
        editor: &OsStr,
        token: &Cancel,
        progress: &Progress,
        auth: Option<(&Requests, &Path)>,
        protected: Protection<'_>,
        before_save: impl FnMut(&Result<bool, String>) -> Result<Option<Method>, String>,
        confirm_published: impl FnMut(&Result<Option<crate::pull::Target>, String>) -> bool,
    ) -> Result<Report, String> {
        self.execute_with_conflicts(
            method,
            editor,
            token,
            progress,
            auth,
            protected,
            before_save,
            confirm_published,
            |_| Ok(super::conflicts::Decision::Close),
        )
    }

    /// The resolver runs inside each updater, before processing the next root.
    /// Closing it retains INCOMPLETE; only whole-group success restores drafts.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_with_conflicts(
        mut self,
        method: Method,
        editor: &OsStr,
        token: &Cancel,
        progress: &Progress,
        auth: Option<(&Requests, &Path)>,
        protected: Protection<'_>,
        mut before_save: impl FnMut(&Result<bool, String>) -> Result<Option<Method>, String>,
        mut confirm_published: impl FnMut(&Result<Option<crate::pull::Target>, String>) -> bool,
        mut resolve: impl FnMut(
            &super::conflicts::Interaction<'_>,
        ) -> Result<super::conflicts::Decision, String>,
    ) -> Result<Report, String> {
        if editor.is_empty() {
            return Err("Merge editor is unavailable".into());
        }
        // Validate every branch before fetching any. Fetches precede the group
        // reservation because the fetch executor owns its own operation guard.
        for branch in &self.branches {
            merge_update::validate(&branch.repository, &branch.request, token)?;
        }
        let mut report = Report {
            roots: Vec::new(),
            fetches: Vec::new(),
            saved: Vec::new(),
            skipped: self.skipped,
            error: None,
            cancelled: false,
        };
        for branch in &self.branches {
            let spec = fetch::Spec::new(branch.request.target.remote.clone())?;
            let result = match auth {
                Some((requests, helper)) => fetch::execute_authenticated(
                    &branch.repository,
                    &[spec],
                    token,
                    progress,
                    requests,
                    helper,
                ),
                None => fetch::execute(&branch.repository, &[spec], token, progress),
            };
            report
                .fetches
                .push((branch.repository.path.clone(), result));
        }
        if !report
            .fetches
            .iter()
            .all(|(_, r)| r.as_ref().is_ok_and(|b| b.successful()))
        {
            report.cancelled = token.cancelled();
            report.error =
                Some("Project fetch did not complete; local update was not started".into());
            return Ok(report);
        }
        let mut keys = Vec::new();
        for branch in &self.branches {
            keys.push(branch.repository.operation_key(token)?);
        }
        for detached in &self.detached {
            keys.extend_from_slice(detached.reserved_keys());
        }
        let guard = crate::operation_queue::acquire_many(keys, token)?;
        for detached in &mut self.detached {
            detached.refresh_worktree_reserved(&guard, token)?;
        }
        let mut save = Vec::new();
        let mut plans = Vec::new();
        // Fast-forward is itself a write. Retain its result even if a later
        // preflight or UI decision fails; never lose evidence of partial work.
        let preparation = (|| -> Result<(), String> {
            for branch in &mut self.branches {
                let repo = &branch.repository;
                merge_update::validate(repo, &branch.request, token)?;
                let references = merge_update::remote_refs(repo, token)?;
                let target = &branch.request.target;
                let range = format!("HEAD..refs/remotes/{}/{}", target.remote, target.branch);
                let mut needed = !repo
                    .text(&["rev-list", "-1", &range, "--"], token)?
                    .trim()
                    .is_empty();
                report.roots.push(RootResult {
                    path: repo.path.clone(),
                    result: if needed {
                        UpdateResult::NotReady
                    } else {
                        UpdateResult::NothingToUpdate
                    },
                    merge: None,
                    rebase: None,
                    fast_forward: None,
                    submodule: None,
                });
                let result = report.roots.last_mut().unwrap();
                if needed
                    && method == Method::Rebase
                    && repo.has_tracked_changes(token).unwrap_or(false)
                {
                    let mut session = Session::under_guard(repo, &guard, token)?;
                    let ff = session.try_fast_forward(target, token, progress)?;
                    let succeeded = good(&ff.outcome);
                    let stopped = ff.outcome.stopped.is_some();
                    result.fast_forward = Some(ff);
                    if succeeded {
                        result.result = UpdateResult::Success;
                        branch.request.head = result
                            .fast_forward
                            .as_ref()
                            .unwrap()
                            .after
                            .as_ref()
                            .map_err(Clone::clone)?
                            .trim()
                            .to_owned();
                        needed = false;
                    } else if stopped {
                        result.result = if token.cancelled() {
                            UpdateResult::Cancelled
                        } else {
                            UpdateResult::Error
                        };
                        return Err("Fast-forward update did not complete".into());
                    }
                }
                let problem = if needed && method == Method::Rebase {
                    repo.rebase_over_merge_problem(target, token)
                } else {
                    Ok(false)
                };
                plans.push((needed, method, references, problem));
            }
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let problem = plans
                .iter()
                .find_map(|p| p.3.as_ref().err().cloned().map(Err))
                .unwrap_or_else(|| Ok(plans.iter().any(|p| matches!(p.3, Ok(true)))));
            if !matches!(problem, Ok(false)) {
                let Some(chosen) = before_save(&problem)? else {
                    report.cancelled = true;
                    return Err("Update cancelled before saving local changes".into());
                };
                for plan in &mut plans {
                    if !matches!(plan.3, Ok(false)) {
                        plan.1 = chosen;
                    }
                }
            }
            for (branch, (needed, method, refs, _)) in self.branches.iter().zip(&plans) {
                merge_update::validate(&branch.repository, &branch.request, token)?;
                merge_update::validate_remote_refs(&branch.repository, refs, token)?;
                if *needed
                    && match method {
                        Method::Merge => branch
                            .repository
                            .merge_update_save_needed(&branch.request.target, token)?,
                        Method::Rebase => {
                            branch.repository.has_tracked_changes(token).unwrap_or(true)
                        }
                    }
                {
                    save.push(branch.repository.clone());
                }
            }
            Ok(())
        })();
        if let Err(error) = preparation {
            report.cancelled |= token.cancelled();
            report.error = Some(error);
            return Ok(report);
        }
        for detached in &self.detached {
            save.extend(detached.dirty_repositories());
        }
        // Save deeper working trees before their parent. Stash does not store
        // nested working files, so each dirty root still needs its own session.
        save.sort_by_key(|repo| std::cmp::Reverse(repo.path.components().count()));
        let mut saved_paths = std::collections::BTreeSet::new();
        save.retain(|repo| saved_paths.insert(repo.path.clone()));
        let mut preservation = SavedChanges::new(&save, &guard, token)?;
        if let Err(error) = preservation.save_all("Plico: before project update", token, progress) {
            report.cancelled |= token.cancelled();
            report.error = Some(error);
            report.saved = preservation.into_reports();
            return Ok(report);
        }
        let update = (|| -> Result<(), String> {
            for ((branch, (needed, method, refs, _)), result) in
                self.branches.iter().zip(plans).zip(&mut report.roots)
            {
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                merge_update::validate(&branch.repository, &branch.request, token)?;
                merge_update::validate_remote_refs(&branch.repository, &refs, token)?;
                if !needed {
                    continue;
                }
                let mut session = Session::under_guard(&branch.repository, &guard, token)?;
                match method {
                    Method::Merge => {
                        let step = session.merge_with_requests(
                            &branch.request.target,
                            editor,
                            token,
                            progress,
                            auth.map(|(requests, _)| requests),
                        );
                        result.result = step
                            .as_ref()
                            .map_or(UpdateResult::Error, |step| step.result());
                        result.merge = Some(step);
                    }
                    Method::Rebase => {
                        let published =
                            protected.check(&branch.repository, &branch.request.target, token);
                        if token.cancelled()
                            || (!matches!(published, Ok(None)) && !confirm_published(&published))
                        {
                            result.result = UpdateResult::Cancelled;
                            return Err(
                                "Rebase cancelled; saved local changes were retained".into()
                            );
                        }
                        merge_update::validate(&branch.repository, &branch.request, token)?;
                        merge_update::validate_remote_refs(&branch.repository, &refs, token)?;
                        let step = session.rebase_with_requests(
                            &branch.request.target,
                            editor,
                            token,
                            progress,
                            auth.map(|(requests, _)| requests),
                        );
                        result.result = step.as_ref().map_or(UpdateResult::Error, rebase_result);
                        result.rebase = Some(step);
                    }
                }
                while result.result == UpdateResult::Incomplete {
                    if token.cancelled() {
                        return Err("Cancelled".into());
                    }
                    let interaction = super::conflicts::Interaction {
                        repository: &branch.repository,
                        guard: &guard,
                        token,
                        progress,
                    };
                    if resolve(&interaction)? == super::conflicts::Decision::Close {
                        break;
                    }
                    if token.cancelled() {
                        return Err("Cancelled".into());
                    }
                    match method {
                        Method::Rebase => {
                            let step = session.continue_rebase_with_requests(
                                editor,
                                token,
                                progress,
                                auth.map(|(requests, _)| requests),
                            );
                            result.result =
                                step.as_ref().map_or(UpdateResult::Error, rebase_result);
                            result.rebase = Some(step);
                        }
                        Method::Merge => {
                            let step = session.continue_merge(token, progress);
                            result.result = step
                                .as_ref()
                                .map_or(UpdateResult::Error, |step| step.result());
                            result.merge = Some(step);
                        }
                    }
                    if result.result == UpdateResult::Success {
                        result.result = UpdateResult::ResolvedConflicts;
                    }
                }
            }
            for detached in &self.detached {
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                let step = detached.execute_reserved(&guard, token, progress, auth);
                let result = match &step {
                    Ok(out) if good(out) => UpdateResult::Success,
                    Ok(out) if out.stopped == Some(StopReason::Cancelled) => {
                        UpdateResult::Cancelled
                    }
                    _ => UpdateResult::Error,
                };
                report.roots.push(RootResult {
                    path: detached.path(),
                    result,
                    merge: None,
                    rebase: None,
                    fast_forward: None,
                    submodule: Some(step),
                });
            }
            Ok(())
        })();
        report.error = update.err();
        report.cancelled |= token.cancelled();
        let successful = report.error.is_none()
            && report.roots.iter().all(|r| {
                matches!(
                    r.result,
                    UpdateResult::Success
                        | UpdateResult::NothingToUpdate
                        | UpdateResult::ResolvedConflicts
                )
            });
        preservation.finish(
            if successful {
                UpdateResult::Success
            } else {
                UpdateResult::Error
            },
            true,
            progress,
        );
        report.saved = preservation.into_reports();
        Ok(report)
    }
}
fn good(out: &Outcome) -> bool {
    out.status.success() && out.stopped.is_none() && out.input_error.is_none()
}

fn rebase_result(step: &RebaseStep) -> UpdateResult {
    if step.outcome.stopped == Some(StopReason::Cancelled) {
        return UpdateResult::Cancelled;
    }
    if step.outcome.stopped.is_some() {
        return UpdateResult::Error;
    }
    match &step.state {
        Err(_) => UpdateResult::Error,
        Ok(state) if state.in_progress() || !state.conflicts.is_empty() => UpdateResult::Incomplete,
        Ok(_) if good(&step.outcome) => UpdateResult::Success,
        Ok(_) => UpdateResult::Error,
    }
}
