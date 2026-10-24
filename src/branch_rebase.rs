//! Independent branch rebase: Rebased GitRebaseBranchAction/GitRebaseProcess
//! at 4aec352 (JetBrains and contributors, Apache-2.0).
//! A prepared selection binds the operation to its worktree, branch and tips.
use crate::{
    auth_request::Requests,
    git::{Cancel, Repository},
    operation_process::{Outcome, Progress},
    operation_state::State,
    rebase_saved,
    update_session::{RebaseStep, Session},
};
use std::{ffi::OsStr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Prepared {
    pub reference: String,
    pub target: String,
    pub branch: String,
    head: String,
    directory: PathBuf,
}
impl Prepared {
    pub fn read(repo: &Repository, reference: &str, token: &Cancel) -> Result<Self, String> {
        if !(reference.starts_with("refs/heads/") || reference.starts_with("refs/remotes/"))
            || reference.contains(['\0', '\r', '\n'])
        {
            return Err("Select a local or remote branch to rebase onto".into());
        }
        repo.text(&["check-ref-format", reference], token)?;
        let branch = crate::squash_check::symbolic_head(repo, token)?
            .ok_or("Checkout a branch before rebasing")?;
        if reference == branch {
            return Err("Cannot rebase a branch onto itself".into());
        }
        let prepared = Self {
            reference: reference.into(),
            target: resolve(repo, reference, token)?,
            branch,
            head: crate::commit_operation::head(repo, token)?
                .ok_or("Cannot rebase without HEAD")?,
            directory: repo.editor_directory(token)?,
        };
        prepared.validate(repo, token)?;
        Ok(prepared)
    }
    pub fn matches_displayed(&self, head: &str, target: &str) -> bool {
        self.head == head && self.target == target
    }
    fn validate(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.directory
            || crate::squash_check::symbolic_head(repo, token)?.as_ref() != Some(&self.branch)
            || crate::commit_operation::head(repo, token)?.as_ref() != Some(&self.head)
            || resolve(repo, &self.reference, token)? != self.target
        {
            return Err("Branches changed; review the rebase again".into());
        }
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before rebasing".into());
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
fn good(output: &Outcome) -> bool {
    output.status.success() && output.stopped.is_none() && output.input_error.is_none()
}
pub struct Report {
    pub step: Option<Result<RebaseStep, String>>,
    pub save: Option<Result<Outcome, String>>,
    pub restore: Option<Result<Option<Outcome>, String>>,
    pub saved_oid: Option<String>,
    pub error: Option<String>,
    pub state: Result<State, String>,
}
/// Save tracked edits before starting, retain ownership during conflicts, and
/// restore the index as well as working-tree edits on success. Untracked files
/// are never stashed or deleted. No fetch or tracking configuration is performed.
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    editor: &OsStr,
    requests: Option<&Requests>,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let mut session = Session::begin(repo, token)?;
    prepared.validate(repo, token)?;
    if editor.is_empty() {
        return Err("Rebase editor is unavailable".into());
    }
    if rebase_saved::read(&prepared.directory)?.is_some()
        || crate::commit_recovery::pending(&prepared.directory)?.is_some()
    {
        return Err("Resolve pending recovery before rebasing".into());
    }
    let dirty = !repo
        .text(&["status", "--porcelain", "--untracked-files=no"], token)?
        .is_empty();
    let save = dirty.then(|| session.save("Plico: before branch rebase", token, progress));
    let record = session.saved_oid().map(|stash| rebase_saved::Record {
        head: prepared.head.clone(),
        base: prepared.target.clone(),
        branch: Some(prepared.branch.clone()),
        stash: stash.to_owned(),
    });
    let mut error = record
        .as_ref()
        .and_then(|r| rebase_saved::create(&prepared.directory, r).err());
    let saved_ok = save.as_ref().is_none_or(|r| r.as_ref().is_ok_and(good));
    let step = if !saved_ok {
        error.get_or_insert_with(|| "Could not save local changes; rebase was not started".into());
        None
    } else if error.is_some() {
        None
    } else {
        match prepared.validate(repo, token) {
            Err(e) => {
                error = Some(e);
                None
            }
            Ok(()) => Some(session.rebase_onto_commit(
                &prepared.target,
                editor,
                token,
                progress,
                requests,
            )),
        }
    };
    let fresh = Cancel::default();
    let state = repo.operation_state(&fresh);
    let unchanged = crate::commit_operation::head(repo, &fresh)
        .is_ok_and(|h| h.as_ref() == Some(&prepared.head));
    let completed = step
        .as_ref()
        .is_some_and(|s| s.as_ref().is_ok_and(|s| good(&s.outcome)));
    let aborted = step.as_ref().is_some_and(|s| {
        s.as_ref().is_ok_and(|s| {
            s.cancellation_abort
                .as_ref()
                .is_some_and(|a| a.as_ref().is_ok_and(good))
        })
    });
    let can_restore = saved_ok
        && error.is_none()
        && state
            .as_ref()
            .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
        && crate::squash_check::symbolic_head(repo, &fresh)
            .is_ok_and(|b| b.as_ref() == Some(&prepared.branch))
        && (completed || (aborted && unchanged));
    let restore = (can_restore && record.is_some()).then(|| session.restore_saved(true, progress));
    if session.saved_oid().is_none()
        && let Some(record) = &record
        && let Err(e) = rebase_saved::remove(&prepared.directory, record)
    {
        error = Some(e);
    }
    Ok(Report {
        step,
        save,
        restore,
        saved_oid: session.saved_oid().map(str::to_owned),
        error,
        state: repo.operation_state(&fresh),
    })
}

impl Report {
    pub fn succeeded(&self) -> bool {
        self.error.is_none()
            && self.saved_oid.is_none()
            && self
                .step
                .as_ref()
                .is_some_and(|s| s.as_ref().is_ok_and(|s| good(&s.outcome)))
            && self
                .restore
                .as_ref()
                .is_none_or(|r| r.as_ref().is_ok_and(|o| o.as_ref().is_none_or(good)))
            && self
                .state
                .as_ref()
                .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
    }
    pub fn diagnostic(&self) -> String {
        let mut messages = Vec::new();
        if let Some(error) = &self.error {
            messages.push(error.clone());
        }
        if let Some(save) = &self.save {
            match save {
                Err(e) => messages.push(e.clone()),
                Ok(o) => messages.push(output_message(o)),
            }
        }
        if let Some(Ok(Some(output))) = &self.restore {
            messages.push(output_message(output));
        }
        if let Some(Err(e)) = &self.restore {
            messages.push(e.clone());
        }
        if let Some(step) = &self.step {
            match step {
                Err(e) => messages.push(e.clone()),
                Ok(s) => {
                    messages.push(crate::rebase_operation::collect_errors(
                        &String::from_utf8_lossy(&s.outcome.stdout.bytes()),
                        &String::from_utf8_lossy(&s.outcome.stderr.bytes()),
                    ));
                    if let Some(Err(e)) = &s.cancellation_abort {
                        messages.push(e.clone());
                    }
                }
            }
        }
        if let Err(e) = &self.state {
            messages.push(e.clone());
        }
        crate::rebase_operation::display_text(&messages.join("\n"))
    }
}

fn output_message(output: &Outcome) -> String {
    crate::rebase_operation::collect_errors(
        &String::from_utf8_lossy(&output.stdout.bytes()),
        &String::from_utf8_lossy(&output.stderr.bytes()),
    )
}
