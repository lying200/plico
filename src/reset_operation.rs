//! Rebased 4aec352 GitResetAction/GitNewResetDialog/GitResetOperation/GitImpl
//! (JetBrains, Apache-2.0), single-worktree translation with bounded diagnostics.
use crate::{
    git::{Cancel, Repository, is_oid},
    operation_process::{self, Progress},
    rebase_operation::ResultState,
};
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Soft,
    #[default]
    Mixed,
    Hard,
    Keep,
}
impl Mode {
    pub const ALL: [Self; 4] = [Self::Soft, Self::Mixed, Self::Hard, Self::Keep];
    pub fn name(self) -> &'static str {
        match self {
            Self::Soft => "Soft",
            Self::Mixed => "Mixed",
            Self::Hard => "Hard",
            Self::Keep => "Keep",
        }
    }
    pub fn argument(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
            Self::Keep => "--keep",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Soft => "Files won't change, differences will be staged for commit.",
            Self::Mixed => "Files won't change, differences won't be staged.",
            Self::Hard => {
                "Files will be reverted to the state of the selected commit.\nWarning: any local changes will be lost."
            }
            Self::Keep => {
                "Files will be reverted to the state of the selected commit,\nbut local changes will be kept intact."
            }
        }
    }
}
pub struct Prepared {
    directory: PathBuf,
    head: String,
    branch: Option<String>,
    pub target: String,
    pub summary: String,
    pub author: String,
}
impl Prepared {
    pub fn prepare(repo: &Repository, target: String, token: &Cancel) -> Result<Self, String> {
        if !is_oid(&target) {
            return Err("Invalid reset commit".into());
        }
        repo.text(&["cat-file", "-e", &format!("{target}^{{commit}}")], token)?;
        let head = crate::commit_operation::head(repo, token)?.ok_or("No HEAD to reset")?;
        let branch = crate::squash_check::symbolic_head(repo, token)?;
        let summary = repo.text(&["show", "-s", "--format=%s%x00%an", &target, "--"], token)?;
        let (subject, author) = summary
            .trim_end()
            .split_once('\0')
            .ok_or("Invalid reset metadata")?;
        let shorten = |v: &str| {
            if v.chars().count() > 40 {
                format!("{}…", v.chars().take(39).collect::<String>())
            } else {
                v.to_owned()
            }
        };
        let prepared = Self {
            directory: repo.editor_directory(token)?,
            head,
            branch,
            target,
            summary: shorten(subject),
            author: shorten(author),
        };
        prepared.check(repo, token)?;
        Ok(prepared)
    }
    pub fn source(&self) -> String {
        self.branch
            .as_deref()
            .map(|b| b.strip_prefix("refs/heads/").unwrap_or(b).to_owned())
            .unwrap_or_else(|| format!("HEAD ({})", &self.head[..8]))
    }
    fn check(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let directory = repo.editor_directory(token)?;
        if directory != self.directory
            || crate::commit_operation::head(repo, token)?.as_deref() != Some(&self.head)
        {
            return Err("Repository state changed; reopen Reset".into());
        }
        let branch = crate::squash_check::symbolic_head(repo, token)?;
        if branch != self.branch {
            return Err("Repository state changed; reopen Reset".into());
        }
        let state = repo.operation_state_at(&directory, branch.is_none(), token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before Reset".into());
        }
        if crate::commit_recovery::pending(&self.directory)?.is_some() {
            return Err("Resolve pending index recovery before Reset".into());
        }
        Ok(())
    }
    pub fn execute(
        &self,
        repo: &Repository,
        mode: Mode,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<ResultState, String> {
        let _guard = repo.operation_guard(token)?;
        self.check(repo, token)?;
        if token.cancelled() {
            return Err("Reset cancelled before execution".into());
        }
        // Git reset is not a transaction across worktree/index/HEAD. Never kill
        // it after checkout starts: even an attempted rollback could destroy
        // pre-existing unstaged/untracked content. Cancellation ends here.
        let execution = Cancel::default();
        let command = operation_process::run(
            repo.operation_command()
                .args(["reset", mode.argument(), &self.target]),
            &execution,
            progress,
            None,
        )
        .map_err(|e| e.to_string());
        let fresh = Cancel::default();
        let recovery_error = match crate::commit_operation::head(repo, &fresh) {
            Ok(Some(head)) if head == self.target => None,
            Ok(_) => Some("Reset did not reach the selected commit".into()),
            Err(e) => Some(e),
        };
        Ok(ResultState {
            command,
            state: repo.operation_state(&fresh),
            recovery_error,
            squash_undo: None,
        })
    }
}
