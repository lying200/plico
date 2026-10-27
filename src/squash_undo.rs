//! Rebased GitCommitEditingOperationResult.Complete (4aec352, Apache-2.0):
//! unchanged result HEAD + unpublished first changed commit, then reset --keep.
use crate::{
    git::{Cancel, HISTORY_MAX, Repository, is_oid},
    operation_process::{self, Outcome, Progress},
    operation_state::State,
    published_commits::Protection,
    squash_check,
    squash_operation::Report,
};
use std::path::PathBuf;

pub struct Undo {
    root: PathBuf,
    directory: PathBuf,
    old_head: String,
    new_head: String,
    branch: Option<String>,
    first_changed: Option<String>,
    original_first_changed: Option<String>,
}
pub struct ResultState {
    pub command: Result<Outcome, String>,
    pub head: Result<Option<String>, String>,
    pub state: Result<State, String>,
    expected_head: String,
}
impl ResultState {
    pub fn succeeded(&self) -> bool {
        self.command
            .as_ref()
            .is_ok_and(|o| o.status.success() && o.stopped.is_none())
            && self
                .head
                .as_ref()
                .is_ok_and(|id| id.as_deref() == Some(&self.expected_head))
            && self
                .state
                .as_ref()
                .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
    }
}
impl Undo {
    /// Keep Undo tied to the worktree that produced this result.
    pub fn repository(&self) -> Repository {
        Repository::new(&self.root)
    }
    pub fn commit_to_focus(&self) -> Option<&str> {
        self.first_changed.as_deref()
    }
    pub fn commit_to_focus_on_undo(&self) -> &str {
        self.original_first_changed
            .as_deref()
            .unwrap_or(&self.old_head)
    }
    pub fn result_head(&self) -> &str {
        &self.new_head
    }
    /// Create an in-memory Undo action only from a completed Squash. This binds
    /// the action to one worktree; app restart does not silently recreate it.
    pub fn capture(repo: &Repository, report: &Report, token: &Cancel) -> Result<Self, String> {
        if !report.succeeded() {
            return Err("Only a completed Squash can be undone".into());
        }
        Self::capture_completed(
            repo,
            &report.old_head,
            &report.base,
            report.branch.clone(),
            report.head.as_ref().unwrap().as_ref().unwrap(),
            token,
        )
    }
    pub(crate) fn after_continue(
        repo: &Repository,
        origin: &crate::squash_session::Origin,
        token: &Cancel,
    ) -> Result<Self, String> {
        let head = crate::commit_operation::head(repo, token)?
            .ok_or("No HEAD after Squash continuation")?;
        Self::capture_completed(
            repo,
            &origin.head,
            &origin.base,
            origin.branch.clone(),
            &head,
            token,
        )
    }
    fn capture_completed(
        repo: &Repository,
        old_head: &str,
        base: &str,
        branch: Option<String>,
        new_head: &str,
        token: &Cancel,
    ) -> Result<Self, String> {
        if [old_head, new_head, base].iter().any(|id| !is_oid(id)) {
            return Err("Invalid Squash result identities".into());
        }
        let range = format!("{}..{new_head}", base);
        let limit = format!("--max-count={}", HISTORY_MAX + 1);
        let text = repo.text(&["rev-list", "--first-parent", &limit, &range, "--"], token)?;
        let ids = text.lines().collect::<Vec<_>>();
        if ids.len() > HISTORY_MAX || ids.iter().any(|id| !is_oid(id)) {
            return Err("Squash Undo history is invalid or exceeds limit".into());
        }
        let old_range = format!("{}..{}", base, old_head);
        let old_text = repo.text(
            &["rev-list", "--first-parent", &limit, &old_range, "--"],
            token,
        )?;
        let old_ids = old_text.lines().collect::<Vec<_>>();
        if old_ids.len() > HISTORY_MAX || old_ids.iter().any(|id| !is_oid(id)) {
            return Err("Original Squash history is invalid or exceeds limit".into());
        }
        let action = Self {
            root: repo.path.clone(),
            directory: repo.editor_directory(token)?,
            old_head: old_head.to_owned(),
            new_head: new_head.to_owned(),
            branch: branch.clone(),
            first_changed: ids.last().map(|id| (*id).into()),
            original_first_changed: old_ids.last().map(|id| (*id).into()),
        };
        action.check_identity(repo, token)?;
        Ok(action)
    }
    fn check_identity(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.directory {
            return Err("Squash Undo belongs to a different worktree".into());
        }
        if crate::commit_operation::head(repo, token)?.as_deref() != Some(&self.new_head) {
            return Err("HEAD moved after Squash; Undo is unavailable".into());
        }
        if squash_check::symbolic_head(repo, token)? != self.branch {
            return Err("HEAD branch changed after Squash; Undo is unavailable".into());
        }
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before Undo".into());
        }
        if crate::commit_recovery::pending(&self.directory)?.is_some() {
            return Err("Resolve pending index recovery before Undo".into());
        }
        Ok(())
    }
    /// A menu/notification can use this read-only check; execute always repeats it
    /// under the operation guard. Errors mean unavailable, never permission.
    pub fn check(
        &self,
        repo: &Repository,
        protection: Protection<'_>,
        token: &Cancel,
    ) -> Result<(), String> {
        self.check_identity(repo, token)?;
        let refs = squash_check::inventory(repo, token)?;
        if let Some(id) = &self.first_changed {
            if let Some(name) = squash_check::protected_containing(repo, id, protection, token)? {
                return Err(format!(
                    "Squash result was published to protected branch {name}; Undo is unavailable"
                ));
            }
        }
        self.check_identity(repo, token)?;
        if squash_check::inventory(repo, token)? != refs {
            return Err("References changed during Undo check; retry".into());
        }
        Ok(())
    }
    /// Explicit action only. `--keep` preserves working content where Git permits;
    /// it is not a promise to retain the index's staged/unstaged partition.
    pub fn execute(
        &self,
        repo: &Repository,
        protection: Protection<'_>,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<ResultState, String> {
        let _guard = repo.operation_guard(token)?;
        self.check(repo, protection, token)?;
        let command = operation_process::run(
            repo.operation_command()
                .args(["reset", "--keep", &self.old_head]),
            token,
            progress,
            None,
        )
        .map_err(|e| e.to_string());
        let fresh = Cancel::default();
        Ok(ResultState {
            command,
            head: crate::commit_operation::head(repo, &fresh),
            state: repo.operation_state(&fresh),
            expected_head: self.old_head.clone(),
        })
    }
}
