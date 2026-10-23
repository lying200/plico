//! Log Cherry-pick. Rebased 4aec352 VcsCherryPickManager / CherryPickProcess.
//! Uses upstream's Git-sequencer mode so Git retains the queue across restarts.
use crate::{
    auth_request::Requests,
    git::{Cancel, Repository, is_oid},
    operation_events::Event,
    operation_process::{self, Outcome, Progress},
    operation_state::State,
    sequencer_operation::EmptyCommit,
};
use std::{collections::HashSet, ffi::OsStr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Prepared {
    /// Oldest selected row first, as VcsCherryPickManager reverses Log selection.
    pub targets: Vec<String>,
    head: String,
    branch: Option<String>,
    directory: PathBuf,
    add_source: bool,
}
impl Prepared {
    /// `protected_refs` contains full remote refs selected by the settings policy.
    /// As in upstream sequencer mode, -x applies only if every commit is published.
    pub fn read(
        repo: &Repository,
        newest_first: &[String],
        protected_refs: &[String],
        token: &Cancel,
    ) -> Result<Self, String> {
        if newest_first.is_empty() || newest_first.iter().any(|id| !is_oid(id)) {
            return Err("Select commits to cherry-pick".into());
        }
        let mut seen = HashSet::new();
        if newest_first.iter().any(|id| !seen.insert(id)) {
            return Err("Duplicate cherry-pick commit".into());
        }
        let head =
            crate::commit_operation::head(repo, token)?.ok_or("Cannot cherry-pick without HEAD")?;
        let mut add_source = !protected_refs.is_empty();
        for id in newest_first {
            // Validate every target before writing anything; do not silently
            // omit unavailable or merge commits from the selected sequence.
            let metadata = repo.text(
                &["show", "-s", "--no-show-signature", "--format=%P", id, "--"],
                token,
            )?;
            if metadata.split_whitespace().count() > 1 {
                return Err("Cherry-picking a merge commit requires a mainline parent".into());
            }
            if add_source {
                let containing = repo.text(
                    &[
                        "for-each-ref",
                        "--format=%(refname)",
                        "--contains",
                        id,
                        "refs/remotes/",
                    ],
                    token,
                )?;
                add_source = containing
                    .lines()
                    .any(|r| protected_refs.iter().any(|p| p == r));
            }
        }
        let prepared = Self {
            targets: newest_first.iter().rev().cloned().collect(),
            head,
            branch: crate::squash_check::symbolic_head(repo, token)?,
            directory: repo.editor_directory(token)?,
            add_source,
        };
        prepared.validate(repo, token)?;
        Ok(prepared)
    }
    fn validate(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.directory
            || crate::commit_operation::head(repo, token)?.as_ref() != Some(&self.head)
            || crate::squash_check::symbolic_head(repo, token)? != self.branch
        {
            return Err("Repository changed; review the cherry-pick again".into());
        }
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before cherry-picking".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    Conflicts,
    Empty,
    UntrackedFiles,
    LocalChanges,
    Cancelled,
    Failed,
}
pub struct Report {
    pub command: Result<Outcome, String>,
    pub state: Result<State, String>,
    pub empty: Option<EmptyCommit>,
    pub status: Status,
    pub retry: Option<Prepared>,
}
/// Does not stage local files, automatically stash, or discard a failed queue.
/// The caller refreshes even on cancellation and offers existing sequencer recovery.
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    editor: &OsStr,
    requests: Option<&Requests>,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let _guard = repo.operation_guard(token)?;
    execute_locked(repo, prepared, editor, requests, token, progress)
}
fn execute_locked(
    repo: &Repository,
    prepared: &Prepared,
    editor: &OsStr,
    requests: Option<&Requests>,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    prepared.validate(repo, token)?;
    if editor.is_empty() {
        return Err("Commit editor is unavailable".into());
    }
    let server = requests
        .map(|r| crate::askpass::Server::start(r.clone()))
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut command = repo.operation_command();
    command.arg("cherry-pick");
    if prepared.add_source {
        command.arg("-x");
    }
    command.args(&prepared.targets).env("GIT_EDITOR", editor);
    if let Some(server) = &server {
        server.configure(&mut command);
    }
    let command =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    drop(server);
    let inspect = Cancel::default();
    let state = repo.operation_state(&inspect);
    let empty = EmptyCommit::read(repo, &inspect)?;
    let status = match (&command, &state) {
        (Ok(output), _) if output.stopped.is_some() => Status::Cancelled,
        (_, Ok(state)) if !state.conflicts.is_empty() => Status::Conflicts,
        (Ok(output), Ok(state))
            if output.status.success() && output.input_error.is_none() && !state.in_progress() =>
        {
            Status::Success
        }
        _ if empty.is_some() => Status::Empty,
        (Ok(output), _)
            if output
                .events
                .contains(Event::UntrackedFilesWouldBeOverwritten) =>
        {
            Status::UntrackedFiles
        }
        (Ok(output), _)
            if output
                .events
                .contains(Event::ApplyChangesWouldBeOverwritten) =>
        {
            Status::LocalChanges
        }
        _ => Status::Failed,
    };
    // Never reapply an already-written prefix. Git normally rejects dirty
    // index/worktree before starting the sequence; otherwise use Continue.
    let retry = (status == Status::LocalChanges && prepared.validate(repo, &inspect).is_ok())
        .then(|| prepared.clone());
    Ok(Report {
        retry,
        command,
        state,
        empty,
        status,
    })
}

/// Stashes are Git-owned durable recovery objects. A failed/unfinished retry
/// retains its stash and reports the OID for the existing Stashes UI.
pub struct SavedReport {
    pub report: Option<Report>,
    pub saved: Option<String>,
    pub error: Option<String>,
}
impl Prepared {
    pub fn save_and_execute(
        &self,
        repo: &Repository,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<SavedReport, String> {
        let mut session = crate::update_session::Session::begin(repo, token)?;
        self.validate(repo, token)?;
        if editor.is_empty() {
            return Err("Commit editor is unavailable".into());
        }
        let save = session.save("Plico: before Cherry-pick", token, progress);
        let mut result = SavedReport {
            report: None,
            saved: session.saved_oid().map(str::to_owned),
            error: None,
        };
        match save {
            Ok(output)
                if output.status.success()
                    && output.stopped.is_none()
                    && output.input_error.is_none() => {}
            Ok(output) => {
                result.error = Some(format!(
                    "Could not save local changes: {}",
                    String::from_utf8_lossy(&output.stderr.bytes())
                ));
                return Ok(result);
            }
            Err(error) => {
                result.error = Some(error);
                return Ok(result);
            }
        }
        match execute_locked(repo, self, editor, requests, token, progress) {
            Ok(report) => result.report = Some(report),
            Err(error) => {
                result.error = Some(error);
                return Ok(result);
            }
        }
        if result
            .report
            .as_ref()
            .is_some_and(|report| report.status == Status::Success)
        {
            match session.finish(crate::update_session::UpdateResult::Success, true, progress) {
                Ok(output)
                    if output.as_ref().is_none_or(|output| {
                        output.status.success()
                            && output.stopped.is_none()
                            && output.input_error.is_none()
                    }) => {}
                Ok(_) => result.error = Some(
                    "Could not restore saved changes; review the retained stash and working tree"
                        .into(),
                ),
                Err(error) => result.error = Some(error),
            }
            result.saved = session.saved_oid().map(str::to_owned);
            // A stash restore conflict is a new conflict, even after successful picks.
            if let Some(report) = &mut result.report {
                report.state = repo.operation_state(&Cancel::default());
                if report
                    .state
                    .as_ref()
                    .is_ok_and(|state| !state.conflicts.is_empty())
                {
                    report.status = Status::Conflicts;
                }
            }
        }
        Ok(result)
    }
}
