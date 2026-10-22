//! Pull dialog options and dispatch translated from Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd:
//! plugins/git4idea/backend/src/pull/GitPullOption.kt, config/GitPullSettings.java,
//! actions/GitPull.java. Ordinary Pull UI is wired in pull_dialog.rs and ui.rs.
use std::{collections::BTreeSet, ffi::OsString};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OptionFlag {
    Rebase,
    FfOnly,
    NoFf,
    Squash,
    NoCommit,
    NoVerify,
}
impl OptionFlag {
    pub fn argument(self) -> &'static str {
        match self {
            Self::Rebase => "--rebase",
            Self::FfOnly => "--ff-only",
            Self::NoFf => "--no-ff",
            Self::Squash => "--squash",
            Self::NoCommit => "--no-commit",
            Self::NoVerify => "--no-verify",
        }
    }
    pub fn compatible(self, other: Self) -> bool {
        use OptionFlag::*;
        match self {
            Rebase => !matches!(other, FfOnly | NoFf | Squash | NoCommit),
            FfOnly => !matches!(other, NoFf | Squash | Rebase),
            NoFf => !matches!(other, FfOnly | Squash | Rebase),
            Squash => !matches!(other, NoFf | FfOnly | Rebase),
            NoCommit => other != Rebase,
            NoVerify => true,
        }
    }
}
/// GitPullSettings starts with no flags; never invent an ff-only/merge override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options(BTreeSet<OptionFlag>);
impl Options {
    pub fn new(flags: impl IntoIterator<Item = OptionFlag>) -> Result<Self, &'static str> {
        let flags: BTreeSet<_> = flags.into_iter().collect();
        if flags
            .iter()
            .any(|a| flags.iter().any(|b| !a.compatible(*b)))
        {
            return Err("Incompatible pull options");
        }
        Ok(Self(flags))
    }
    pub fn enabled(&self, flag: OptionFlag) -> bool {
        self.0.iter().all(|selected| selected.compatible(flag))
    }
    pub fn contains(&self, flag: OptionFlag) -> bool {
        self.0.contains(&flag)
    }
    /// GitPullDialog.isCommitAfterMerge; SQUASH alone does not alter this flag.
    pub fn commit_after_merge(&self) -> bool {
        !self.contains(OptionFlag::NoCommit)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Dispatch {
    /// GitPull.getHandlerProvider -> GitMergeAction, with Git configuration intact.
    PullCommand {
        args: Vec<OsString>,
        commit_after_merge: bool,
    },
    /// GitPull.performRebase -> GitUpdateExecutionProcess(UpdateMethod.REBASE).
    /// Requires an attached current branch and the update pipeline, not a raw pull.
    RebaseUpdate { remote: String, branch: String },
}
pub fn dispatch(remote: &str, branch: &str, options: &Options) -> Result<Dispatch, &'static str> {
    if [remote, branch]
        .iter()
        .any(|value| value.is_empty() || value.contains(['\0', '\r', '\n']))
    {
        return Err("Invalid pull target");
    }
    if options.contains(OptionFlag::Rebase) {
        return Ok(Dispatch::RebaseUpdate {
            remote: remote.into(),
            branch: branch.into(),
        });
    }
    let mut args = vec![OsString::from("pull"), "--no-stat".into()];
    args.extend(options.0.iter().map(|flag| OsString::from(flag.argument())));
    args.extend(
        ["-v", "--progress", "--", remote, branch]
            .into_iter()
            .map(OsString::from),
    );
    Ok(Dispatch::PullCommand {
        args,
        commit_after_merge: options.commit_after_merge(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use OptionFlag::*;
    #[test]
    fn option_compatibility_matches_rebased_matrix() {
        let flags = [Rebase, FfOnly, NoFf, Squash, NoCommit, NoVerify];
        // Rows/columns follow GitPullOption declaration order; true means selectable.
        let expected = [
            [true, false, false, false, false, true],
            [false, true, false, false, true, true],
            [false, false, true, false, true, true],
            [false, false, false, true, true, true],
            [false, true, true, true, true, true],
            [true, true, true, true, true, true],
        ];
        for (row, &a) in flags.iter().enumerate() {
            for (col, &b) in flags.iter().enumerate() {
                assert_eq!(
                    Options::new([a, b]).is_ok(),
                    expected[row][col],
                    "{a:?} with {b:?}"
                );
                assert_eq!(Options::new([a]).unwrap().enabled(b), expected[row][col]);
            }
        }
    }
    #[test]
    fn default_preserves_git_config_and_explicit_rebase_uses_update_pipeline() {
        assert_eq!(
            dispatch("origin", "main", &Options::default()).unwrap(),
            Dispatch::PullCommand {
                args: [
                    "pull",
                    "--no-stat",
                    "-v",
                    "--progress",
                    "--",
                    "origin",
                    "main"
                ]
                .into_iter()
                .map(OsString::from)
                .collect(),
                commit_after_merge: true,
            }
        );
        // Upstream rebase dispatch does not pass the raw pull options into a command.
        assert_eq!(
            dispatch(
                "company",
                "topic",
                &Options::new([Rebase, NoVerify]).unwrap()
            )
            .unwrap(),
            Dispatch::RebaseUpdate {
                remote: "company".into(),
                branch: "topic".into()
            }
        );
        assert!(
            !Options::new([Squash, NoCommit])
                .unwrap()
                .commit_after_merge()
        );
        assert!(Options::new([Squash]).unwrap().commit_after_merge());
    }
    #[test]
    fn target_values_are_arguments_not_options() {
        let Dispatch::PullCommand { args, .. } =
            dispatch("--upload-pack=other", "--force", &Options::default()).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            &args[4..],
            &[
                OsString::from("--"),
                "--upload-pack=other".into(),
                "--force".into()
            ]
        );
        assert!(dispatch("origin", "bad\nbranch", &Options::default()).is_err());
        assert!(dispatch("", "main", &Options::default()).is_err());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub remote: String,
    pub branch: String,
}

/// Identity shown when the Pull dialog opens. Revalidated before explicit Rebase.
#[derive(Clone, Debug)]
pub struct Source {
    pub branch: String,
    pub head: String,
}
pub fn source(
    repo: &crate::git::Repository,
    token: &crate::git::Cancel,
) -> Result<Option<Source>, String> {
    let state = repo.operation_state(token)?;
    if state.detached {
        return Ok(None);
    }
    let branch = repo
        .text(&["symbolic-ref", "--quiet", "--short", "HEAD"], token)?
        .trim()
        .to_owned();
    let head = match repo.text(&["rev-parse", "--verify", "HEAD"], token) {
        Ok(head) => head.trim().to_owned(),
        Err(_) if !token.cancelled() => return Ok(None), // An unborn branch can still use ordinary Pull.
        Err(error) => return Err(error),
    };
    Ok(Some(Source { branch, head }))
}

/// Editor commands are supplied by application integration, not repository data.
pub struct CommandSpec {
    pub target: Target,
    pub options: Options,
    pub commit_editor: OsString,
    pub sequence_editor: Option<OsString>,
}
pub struct CommandResult {
    pub command: Result<crate::operation_process::Outcome, String>,
    pub state: Result<crate::operation_state::State, String>,
    pub commit_after_merge: bool,
}

/// GitPull's non-REBASE route through GitMergeAction. Keeps Git's configured
/// strategy; explicitly selected REBASE must use the separate update pipeline.
/// A successful exit with --no-commit may still leave MERGE_HEAD. Consumers must
/// use real state, and must never describe cancellation as a rollback.
pub fn execute_command(
    repo: &crate::git::Repository,
    spec: &CommandSpec,
    token: &crate::git::Cancel,
    progress: &crate::operation_process::Progress,
    auth: Option<(&crate::auth_request::Requests, &std::path::Path)>,
) -> Result<CommandResult, String> {
    let Dispatch::PullCommand {
        args,
        commit_after_merge,
    } = dispatch(&spec.target.remote, &spec.target.branch, &spec.options)?
    else {
        return Err("Explicit Rebase requires the update pipeline".into());
    };
    let _guard = repo.operation_guard(token)?;
    if !repo
        .pull_targets(token)?
        .contains(&spec.target.remote, &spec.target.branch)
    {
        return Err("Pull target is no longer available".into());
    }
    let state = repo.operation_state(token)?;
    if state.in_progress() || !state.conflicts.is_empty() {
        return Err("An unfinished Git operation prevents Pull".into());
    }
    if spec.commit_editor.is_empty() {
        return Err("Commit editor is unavailable".into());
    }
    let interactive = repo
        .text(&["config", "--get", "pull.rebase"], token)
        .is_ok_and(|value| value.trim() == "interactive");
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    let mut command = repo.operation_command();
    command.env("GIT_EDITOR", &spec.commit_editor);
    if interactive {
        let editor = spec
            .sequence_editor
            .as_ref()
            .filter(|editor| !editor.is_empty())
            .ok_or("Interactive rebase editor is unavailable")?;
        command
            .args(["-c", "core.commentChar=\u{1}"])
            .env("GIT_SEQUENCE_EDITOR", editor);
    }
    command.args(args);
    let initial_head = repo.text(&["rev-parse", "HEAD"], token).ok();
    let result =
        crate::authenticated_command::run(&mut command, auth, token, progress, None, |_| {
            initial_head.is_some()
                && repo.text(&["rev-parse", "HEAD"], token).ok() == initial_head
                && repo
                    .operation_state(token)
                    .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty())
        });
    // A cancelled network/update command may already have written refs or index.
    let state = repo.operation_state(&crate::git::Cancel::default());
    Ok(CommandResult {
        command: result,
        state,
        commit_after_merge,
    })
}
#[derive(Debug)]
pub struct Targets {
    pub remotes: Vec<String>,
    pub branches: Vec<Target>,
    pub current_branch: Option<String>,
    pub tracked: Option<Target>,
}
impl Targets {
    /// GitPullDialog: tracked remote, origin, then first configured remote.
    pub fn default_remote(&self) -> Option<&str> {
        self.tracked
            .as_ref()
            .map(|t| t.remote.as_str())
            .or_else(|| {
                self.remotes
                    .iter()
                    .find(|r| r.as_str() == "origin")
                    .map(String::as_str)
            })
            .or_else(|| self.remotes.first().map(String::as_str))
    }
    pub fn contains(&self, remote: &str, branch: &str) -> bool {
        self.branches
            .iter()
            .any(|t| t.remote == remote && t.branch == branch)
    }
    /// Preserve a valid draft; otherwise tracking name, current-name match, empty.
    /// The tracking name may not exist on a newly selected remote. Keep it as an
    /// invalid draft rather than silently substituting a different pull target.
    pub fn default_branch<'a>(
        &'a self,
        remote: &str,
        previous: Option<&'a str>,
    ) -> Option<&'a str> {
        previous
            .filter(|branch| self.contains(remote, branch))
            .or_else(|| self.tracked.as_ref().map(|t| t.branch.as_str()))
            .or_else(|| {
                self.current_branch
                    .as_deref()
                    .filter(|branch| self.contains(remote, branch))
            })
    }
}
/// GitUtil.parseRemoteBranch tests configured prefixes from the first slash on,
/// so the first matching prefix wins (not the longest remote name).
pub(crate) fn parse_remote_ref(reference: &str, remotes: &[String]) -> Option<Target> {
    let name = reference.strip_prefix("refs/remotes/")?;
    for (offset, _) in name.match_indices('/') {
        let remote = &name[..offset];
        if remotes.iter().any(|r| r == remote) {
            return Some(Target {
                remote: remote.into(),
                branch: name[offset + 1..].into(),
            });
        }
    }
    None // Orphan refs cannot be selected in a configured-remote Pull dialog.
}
