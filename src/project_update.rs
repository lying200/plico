//! Update Project preflight, separate from the explicit Pull target picker.
//! Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd:
//! GitUpdateProcess.update / checkTrackedBranchesConfiguration (Apache-2.0).
use crate::{
    git::{Cancel, Repository},
    merge_update::Request,
};

pub mod conflicts;
pub mod group;
pub mod preservation;
pub mod roots;

/// Snapshot all update roots and their allowed editor files before confirmation.
/// After document saving, the executor revalidates identities and re-reads the
/// worktrees under its reservation before deciding which changes to preserve.
pub struct Prepared {
    pub group: group::Group,
    pub editor_directories: Vec<std::path::PathBuf>,
}
impl Prepared {
    pub fn read_registered(registered: &[Repository], token: &Cancel) -> Result<Self, String> {
        let group = group::Group::prepare_registered(registered, token)?;
        let editor_directories = group.editor_directories(token)?;
        Ok(Self {
            group,
            editor_directories,
        })
    }

    pub fn read(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        let group = group::Group::prepare(repo, prepare(repo, token)?, token)?;
        let editor_directories = group.editor_directories(token)?;
        Ok(Self {
            group,
            editor_directories,
        })
    }
}

/// Capture the current branch and its actual upstream. No fallback to origin or
/// a same-named branch: those are Pull dialog conveniences, not update rules.
/// This is a read-only snapshot, not authorization to skip worker revalidation.
/// Existing merge/rebase updaters validate again before and after Fetch.
pub fn prepare(repo: &Repository, token: &Cancel) -> Result<Request, String> {
    let state = repo.operation_state(token)?;
    if state.in_progress() || !state.conflicts.is_empty() {
        return Err("An unfinished operation prevents update".into());
    }
    let targets = repo.pull_targets(token)?;
    let branch = targets
        .current_branch
        .ok_or("Update Project requires an attached branch")?;
    let target = targets
        .tracked
        .ok_or("The current branch has no existing tracked remote branch")?;
    let head = repo
        .text(&["rev-parse", "--verify", "HEAD"], token)?
        .trim()
        .to_owned();
    // Match the existing preserving-process policy. This does not write/stash;
    // the selected updater decides whether saving local changes is necessary.
    let request = Request {
        branch,
        head,
        target,
        reinstate_index: true,
    };
    crate::merge_update::validate(repo, &request, token)?;
    Ok(request)
}

pub use crate::update_report::Report;

/// Runs once after an explicit Update Project choice. The request is the
/// identity captured before confirmation; never recapture a new branch here.
/// There is no Push, rejected-push prompt, preference write or retry loop.
/// Callers must service authentication/editor and risk callbacks on the UI.
#[allow(clippy::too_many_arguments)]
pub fn execute(
    repo: &Repository,
    request: &Request,
    method: crate::update_method::Method,
    editor: &std::ffi::OsStr,
    token: &Cancel,
    progress: &crate::operation_process::Progress,
    auth: Option<(&crate::auth_request::Requests, &std::path::Path)>,
    protected: crate::published_commits::Protection<'_>,
    mut before_save: impl FnMut(
        &Result<bool, String>,
    ) -> Result<Option<crate::update_method::Method>, String>,
    confirm_published: impl FnMut(&Result<Option<crate::pull::Target>, String>) -> bool,
) -> Result<Report, String> {
    use crate::update_method::Method;
    match method {
        Method::Merge => crate::merge_update::execute(repo, request, editor, token, progress, auth)
            .map(Report::Merge),
        Method::Rebase => crate::rebase_update::execute(
            repo,
            request,
            editor,
            token,
            progress,
            auth,
            protected,
            |problem| {
                // Unlike Push's initial rejection prompt, Update Project has
                // not already asked about rebasing merge commits.
                if matches!(problem, Ok(false)) {
                    Ok(Some(Method::Rebase))
                } else {
                    before_save(problem)
                }
            },
            confirm_published,
        )
        .map(Report::Rebase),
    }
}
