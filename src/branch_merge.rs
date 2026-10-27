//! Rebased GitMergeOperation (4aec352), Apache-2.0, JetBrains and contributors.
//! Merge a selected branch without fetching or changing its tracking settings.
use crate::{
    auth_request::Requests,
    git::{Cancel, Repository},
    operation_events::Event,
    operation_process::{self, Outcome, Progress},
    operation_state::State,
    update_session::Session,
};
use std::{ffi::OsStr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Prepared {
    root: PathBuf,
    pub reference: String,
    pub target: String,
    pub branch: String,
    head: String,
    directory: PathBuf,
}
impl Prepared {
    pub fn repository(&self) -> Repository {
        Repository::new(&self.root)
    }

    fn validate_repository(&self, repo: &Repository) -> Result<(), String> {
        if crate::recent_projects::identity(&repo.path)
            != crate::recent_projects::identity(&self.root)
        {
            return Err("Branch merge belongs to a different repository".into());
        }
        Ok(())
    }

    pub fn read(repo: &Repository, reference: &str, token: &Cancel) -> Result<Self, String> {
        if !(reference.starts_with("refs/heads/") || reference.starts_with("refs/remotes/"))
            || reference.contains(['\0', '\n', '\r'])
        {
            return Err("Select a local or remote branch to merge".into());
        }
        repo.text(&["check-ref-format", reference], token)?;
        let branch = crate::squash_check::symbolic_head(repo, token)?
            .ok_or("Checkout a branch before merging")?;
        if branch == reference {
            return Err("Cannot merge a branch into itself".into());
        }
        let target = resolve(repo, reference, token)?;
        let head =
            crate::commit_operation::head(repo, token)?.ok_or("Cannot merge without HEAD")?;
        let prepared = Self {
            root: repo.path.clone(),
            reference: reference.into(),
            target,
            branch,
            head,
            directory: repo.editor_directory(token)?,
        };
        prepared.validate(repo, token)?;
        Ok(prepared)
    }
    /// GitBranchPopupActions.MergeAction proposes deletion only for local,
    /// unprotected branches. This uses the same default main/master policy as Push.
    pub fn may_propose_delete(&self) -> bool {
        self.reference
            .strip_prefix("refs/heads/")
            .is_some_and(|name| !matches!(name, "main" | "master"))
    }

    /// Read again when the user accepts the suggestion, never delete a tip that
    /// moved after merge. The regular deletion flow retains worktree/upstream checks.
    pub fn prepare_deletion(
        &self,
        repo: &Repository,
        token: &Cancel,
    ) -> Result<crate::branch_delete::Prepared, String> {
        self.validate_repository(repo)?;
        if !self.may_propose_delete()
            || repo.editor_directory(token)? != self.directory
            || crate::squash_check::symbolic_head(repo, token)?.as_ref() != Some(&self.branch)
            || resolve(repo, &self.reference, token)? != self.target
        {
            return Err("Branches changed; review branch deletion again".into());
        }
        if !repo
            .text(
                &[
                    "rev-list",
                    "--max-count=1",
                    &format!("HEAD..{}", self.target),
                    "--",
                ],
                token,
            )?
            .trim()
            .is_empty()
        {
            return Err("Merged commits are no longer contained in HEAD".into());
        }
        crate::branch_delete::Prepared::read(repo, &self.reference, token)
    }

    /// Bind a later Continue action to the exact merge originally started here.
    pub fn matches_pending_merge(&self, repo: &Repository, token: &Cancel) -> Result<bool, String> {
        self.validate_repository(repo)?;
        Ok(repo.editor_directory(token)? == self.directory
            && crate::squash_check::symbolic_head(repo, token)?.as_ref() == Some(&self.branch)
            && crate::commit_operation::head(repo, token)?.as_ref() == Some(&self.head)
            && resolve(repo, &self.reference, token)? == self.target
            && repo.operation_state(token)?.operation == crate::operation_state::Operation::Merge
            && repo
                .text(&["rev-parse", "--verify", "MERGE_HEAD"], token)?
                .trim()
                == self.target)
    }

    fn validate(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        self.validate_repository(repo)?;
        if repo.editor_directory(token)? != self.directory
            || crate::squash_check::symbolic_head(repo, token)?.as_ref() != Some(&self.branch)
            || crate::commit_operation::head(repo, token)?.as_ref() != Some(&self.head)
            || resolve(repo, &self.reference, token)? != self.target
        {
            return Err("Branches changed; review the merge again".into());
        }
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before merging".into());
        }
        Ok(())
    }
}
fn resolve(repo: &Repository, reference: &str, token: &Cancel) -> Result<String, String> {
    repo.text(
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
        token,
    )
    .map(|s| s.trim().to_owned())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    AlreadyUpToDate,
    LocalChanges,
    UntrackedFiles,
    Conflicts,
    Cancelled,
    Failed,
}
pub struct Report {
    pub command: Option<Result<Outcome, String>>,
    pub save: Option<Result<Outcome, String>>,
    pub restore: Option<Result<Option<Outcome>, String>>,
    pub saved_oid: Option<String>,
    pub error: Option<String>,
    pub state: Result<State, String>,
    pub status: Status,
}
/// `save_changes` is only set after the user accepts Smart Merge. Untracked
/// files are never stashed or removed implicitly. Failed/conflicting restore
/// retains the stash identity in the report for explicit recovery.
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    save_changes: bool,
    editor: &OsStr,
    requests: Option<&Requests>,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    prepared.validate_repository(repo)?;
    let mut session = Session::begin(repo, token)?;
    prepared.validate(repo, token)?;
    if editor.is_empty() {
        return Err("Merge editor is unavailable".into());
    }
    let mut report = Report {
        command: None,
        save: None,
        restore: None,
        saved_oid: None,
        error: None,
        state: Err("Not refreshed".into()),
        status: Status::Failed,
    };
    let mut saved_record = None;
    let run = (|| -> Result<(), String> {
        if save_changes {
            if crate::merge_saved::read(&prepared.directory)?.is_some() {
                return Err("Resolve the previous saved merge changes before saving again".into());
            }
            report.save = Some(session.save("Plico: save changes before merge", token, progress));
            if !report
                .save
                .as_ref()
                .unwrap()
                .as_ref()
                .is_ok_and(|o| o.status.success() && o.stopped.is_none() && o.input_error.is_none())
            {
                return Err("Could not save local changes before merge".into());
            }
            if let Some(stash) = session.saved_oid() {
                let record = crate::merge_saved::Record {
                    head: prepared.head.clone(),
                    target: prepared.target.clone(),
                    branch: prepared.branch.clone(),
                    stash: stash.to_owned(),
                };
                crate::merge_saved::create(&prepared.directory, &record)?;
                saved_record = Some(record);
            }
            prepared.validate(repo, token)?;
        }
        let server = requests
            .map(|r| crate::askpass::Server::start(r.clone()))
            .transpose()
            .map_err(|e| e.to_string())?;
        let mut command = repo.operation_command();
        // Use the full ref immediately; no ambiguous short-name attempt is needed.
        command
            .args(["merge", "--no-stat", "-v", "--", &prepared.reference])
            .env("GIT_EDITOR", editor);
        if let Some(server) = &server {
            server.configure(&mut command);
        }
        report.command = Some(
            operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string()),
        );
        Ok(())
    })();
    report.error = run.err();
    let fresh = Cancel::default();
    report.state = repo.operation_state(&fresh);
    report.status = classify(&report, token.cancelled());
    if save_changes
        && report
            .state
            .as_ref()
            .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
    {
        report.restore = Some(
            if crate::squash_check::symbolic_head(repo, &fresh)
                .as_ref()
                .ok()
                .and_then(|b| b.as_ref())
                != Some(&prepared.branch)
            {
                Err("Branch changed; saved merge changes were retained".into())
            } else {
                session.restore_saved(true, progress)
            },
        );
        report.state = repo.operation_state(&fresh);
        if report.restore.as_ref().is_some_and(|r| {
            !r.as_ref().is_ok_and(|o| {
                o.as_ref().is_none_or(|o| {
                    o.status.success() && o.stopped.is_none() && o.input_error.is_none()
                })
            })
        }) {
            report.error =
                Some("Merge finished but saved changes could not be restored completely".into());
            report.status = if report.state.as_ref().is_ok_and(|s| !s.conflicts.is_empty()) {
                Status::Conflicts
            } else {
                Status::Failed
            };
        }
    }
    if session.saved_oid().is_none() {
        if let Some(record) = saved_record {
            if let Err(error) = crate::merge_saved::remove(&prepared.directory, &record) {
                report.error = Some(error);
                report.status = Status::Failed;
            }
        }
    }
    report.saved_oid = session.saved_oid().map(str::to_owned);
    Ok(report)
}
fn classify(report: &Report, cancelled: bool) -> Status {
    if cancelled {
        return Status::Cancelled;
    }
    if report.state.as_ref().is_ok_and(|s| !s.conflicts.is_empty()) {
        return Status::Conflicts;
    }
    let (Some(Ok(output)), Ok(state)) = (&report.command, &report.state) else {
        return Status::Failed;
    };
    if report.error.is_some() || output.stopped.is_some() || output.input_error.is_some() {
        return Status::Failed;
    }
    if output.status.success() && !state.in_progress() {
        if output.events.contains(Event::AlreadyUpToDate) {
            Status::AlreadyUpToDate
        } else {
            Status::Success
        }
    } else if output
        .events
        .contains(Event::LocalChangesWouldBeOverwritten)
    {
        Status::LocalChanges
    } else if output
        .events
        .contains(Event::UntrackedFilesWouldBeOverwritten)
    {
        Status::UntrackedFiles
    } else {
        Status::Failed
    }
}
