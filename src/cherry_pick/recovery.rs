//! Original interaction authority for Git-owned cherry-pick queues.
use super::*;
use crate::{
    operation_state::Operation,
    sequencer_operation::{self, Action},
};
use std::{fs::File, io::Read};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
    directory: PathBuf,
    head: String,
    branch: Option<String>,
    markers: Vec<Option<Vec<u8>>>,
}
impl Recovery {
    pub(super) fn capture(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        let snapshot = Self::read(repo, token)?;
        if snapshot != Self::read(repo, token)? {
            return Err("Cherry-pick queue changed while reading recovery state".into());
        }
        Ok(snapshot)
    }
    fn read(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if repo.operation_state(token)?.operation != Operation::CherryPick {
            return Err("No original cherry-pick to continue".into());
        }
        let directory = repo.editor_directory(token)?;
        let head = crate::commit_operation::head(repo, token)?.ok_or("No HEAD for cherry-pick")?;
        let branch = crate::squash_check::symbolic_head(repo, token)?;
        let mut markers = Vec::new();
        for name in [
            "CHERRY_PICK_HEAD",
            "sequencer/head",
            "sequencer/todo",
            "sequencer/opts",
            "sequencer/abort-safety",
        ] {
            let file = match File::open(directory.join(name)) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    markers.push(None);
                    continue;
                }
                Err(error) => return Err(error.to_string()),
            };
            let mut bytes = Vec::new();
            file.take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 1024 * 1024 {
                return Err("Cherry-pick queue exceeds the recovery read limit".into());
            }
            markers.push(Some(bytes));
        }
        Ok(Self {
            directory,
            head,
            branch,
            markers,
        })
    }

    /// Only an active original workflow may invoke this. Independent Continue
    /// deliberately remains separate and cannot resume another root's work.
    pub fn resume(
        &self,
        repo: &Repository,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<Report, String> {
        self.run(
            repo,
            Action::Continue { editor, requests },
            editor,
            requests,
            token,
            progress,
        )
    }

    pub fn resolve_empty(
        &self,
        repo: &Repository,
        decision: (&EmptyCommit, sequencer_operation::EmptyChoice),
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<Report, String> {
        self.run(
            repo,
            Action::Empty {
                expected: decision.0,
                choice: decision.1,
            },
            editor,
            requests,
            token,
            progress,
        )
    }

    fn run(
        &self,
        repo: &Repository,
        action: Action<'_>,
        editor: &OsStr,
        requests: Option<&Requests>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<Report, String> {
        let keep_empty = matches!(
            &action,
            Action::Empty {
                choice: sequencer_operation::EmptyChoice::Keep,
                ..
            }
        );
        let _guard = repo.operation_guard(token)?;
        if Self::capture(repo, token)? != *self {
            return Err("Cherry-pick queue changed; review before continuing".into());
        }
        let mut result = sequencer_operation::execute_locked(
            repo,
            Operation::CherryPick,
            action,
            token,
            progress,
        )?;
        // Creating an empty commit clears CHERRY_PICK_HEAD but leaves a multi-
        // commit sequencer queue. Continue that same owned queue before other roots.
        if keep_empty
            && !token.cancelled()
            && result.command.as_ref().is_ok_and(|out| {
                out.status.success() && out.stopped.is_none() && out.input_error.is_none()
            })
            && result.state.as_ref().is_ok_and(|state| {
                state.operation == Operation::CherryPick && state.conflicts.is_empty()
            })
            && result.empty.is_none()
        {
            result = sequencer_operation::execute_locked(
                repo,
                Operation::CherryPick,
                Action::Continue { editor, requests },
                token,
                progress,
            )?;
        }
        let status = classify(&result.command, &result.state, result.empty.is_some());
        let recovery = if matches!(status, Status::Conflicts | Status::Empty) {
            Self::capture(repo, &Cancel::default()).ok()
        } else {
            None
        };
        Ok(Report {
            command: result.command,
            state: result.state,
            empty: result.empty,
            status,
            retry: None,
            recovery,
        })
    }
}
