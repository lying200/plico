//! GitUpdateProcess routes attached submodules through the ordinary updater.
//! Reference: Rebased 4aec352 collectDetachedSubmodules / defineUpdaters.
use crate::{
    auth_request::Requests,
    git::{Cancel, Repository},
    merge_update,
    operation_process::Progress,
    project_update, published_commits,
    submodules::{self, Checkout},
    update_method::Method,
};
use std::{ffi::OsStr, path::Path};

pub enum Request {
    Branch {
        repository: Repository,
        request: merge_update::Request,
        snapshot: super::Update,
    },
    Detached(super::Update),
}
pub enum Report {
    Branch(Box<project_update::Report>),
    Detached(super::Report),
}
impl Report {
    pub fn complete(&self) -> bool {
        match self {
            Self::Branch(report) => report.complete(),
            Self::Detached(report) => report.succeeded(),
        }
    }
}
impl Request {
    /// Snapshot before confirmation; attached roots require their own upstream.
    /// Do not invent a branch, initialize a missing checkout, or fall back to the
    /// parent's gitlink when a branch has no tracking configuration.
    pub fn prepare(repo: &Repository, path: &Path, token: &Cancel) -> Result<Self, String> {
        let root = submodules::worktree_root(repo, token)?;
        if !submodules::list(repo, token)?
            .iter()
            .any(|module| module.path == path)
        {
            return Err("Submodule declaration no longer exists".into());
        }
        match submodules::inspect(repo, path, token)? {
            Checkout::Initialized {
                branch: Some(_), ..
            } => {
                let repository = Repository::new(root.join(path));
                let request = project_update::prepare(&repository, token)?;
                let snapshot = super::Update::prepare(repo, path, token)?;
                Ok(Self::Branch {
                    repository,
                    request,
                    snapshot,
                })
            }
            Checkout::Initialized { branch: None, .. } => {
                super::Update::prepare(repo, path, token).map(Self::Detached)
            }
            Checkout::Uninitialized { .. } => Err("Submodule is not initialized".into()),
        }
    }

    /// Same worker callbacks as Update Project. Both executors revalidate their
    /// captured identity; neither silently recaptures a different current branch.
    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        method: Method,
        editor: &OsStr,
        token: &Cancel,
        progress: &Progress,
        auth: Option<(&Requests, &Path)>,
        protected: published_commits::Protection<'_>,
        before_save: impl FnMut(&Result<bool, String>) -> Result<Option<Method>, String>,
        confirm_published: impl FnMut(&Result<Option<crate::pull::Target>, String>) -> bool,
    ) -> Result<Report, String> {
        match self {
            Self::Branch {
                repository,
                request,
                snapshot,
            } => {
                let parent = Repository::new(&snapshot.snapshot.parent);
                let current =
                    super::Update::prepare(&parent, &snapshot.snapshot.module.path, token)?;
                if snapshot.keys != current.keys || snapshot.snapshot != current.snapshot {
                    return Err("Submodule state changed; refresh before updating".into());
                }
                project_update::execute(
                    repository,
                    request,
                    method,
                    editor,
                    token,
                    progress,
                    auth,
                    protected,
                    before_save,
                    confirm_published,
                )
                .map(|report| Report::Branch(Box::new(report)))
            }
            Self::Detached(update) => update
                .execute_preserving(token, progress, auth)
                .map(Report::Detached),
        }
    }
}
