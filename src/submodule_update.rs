//! Rebased GitSubmoduleUpdater (4aec352, Apache-2.0): run recursive update
//! from the parent, even when the parent itself has nothing new to pull.
//! This clean-checkout executor is the mutation layer, not the complete Update
//! Project orchestration (branch updates and save/restore belong above it).
use crate::{
    auth_request::Requests,
    git::{Cancel, Repository},
    operation_process::{Outcome, Progress},
    submodules::{self, Checkout, IndexState, Module},
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub mod routing;

/// Intentionally not Debug: declarations may contain credential-bearing URLs.
#[derive(PartialEq, Eq)]
struct Snapshot {
    parent: PathBuf,
    module: Module,
    checkout: Checkout,
    children: Vec<Snapshot>,
    tracked_files: bool,
}

/// An opaque read snapshot. Execution checks it again after acquiring all known
/// repositories together. It is never permission to overwrite local changes.
pub struct Update {
    snapshot: Snapshot,
    keys: Vec<PathBuf>,
}
pub struct Result {
    pub outcome: Outcome,
    /// Refresh failures do not erase the command result or cause a retry of a
    /// possibly partially completed recursive operation.
    pub checkout: std::result::Result<Checkout, String>,
}
impl Update {
    pub fn prepare(
        repo: &Repository,
        path: &Path,
        token: &Cancel,
    ) -> std::result::Result<Self, String> {
        let prepared = Self::prepare_any(repo, path, token)?;
        if !matches!(prepared.snapshot.checkout, Checkout::Initialized { .. }) {
            return Err("Initialize the submodule before updating it".into());
        }
        Ok(prepared)
    }

    fn prepare_any(
        repo: &Repository,
        path: &Path,
        token: &Cancel,
    ) -> std::result::Result<Self, String> {
        let parent = submodules::worktree_root(repo, token)?;
        let repo = Repository::new(&parent);
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the parent Git operation before updating submodules".into());
        }
        let module = submodules::list(&repo, token)?
            .into_iter()
            .find(|module| module.path == path)
            .ok_or("Submodule declaration no longer exists")?;
        let mut keys = BTreeSet::new();
        keys.insert(repo.operation_key(token)?);
        let mut count = 0;
        let snapshot = capture(parent, module, token, &mut keys, &mut count, 0)?;
        Ok(Self {
            snapshot,
            keys: keys.into_iter().collect(),
        })
    }

    /// Only clean detached checkouts enter this layer. The orchestration must
    /// first preserve dirty roots and route attached roots to normal updates.
    /// No --force, implicit init, hard reset, or retry after partial progress.
    pub fn execute_clean(
        &self,
        token: &Cancel,
        progress: &Progress,
        auth: Option<(&Requests, &Path)>,
    ) -> std::result::Result<Result, String> {
        let _guard = crate::operation_queue::acquire_many(self.keys.clone(), token)?;
        let repo = Repository::new(&self.snapshot.parent);
        let current = Self::prepare(&repo, &self.snapshot.module.path, token)?;
        if current.keys != self.keys || current.snapshot != self.snapshot {
            return Err("Submodule state changed; refresh before updating".into());
        }
        check_clean(&current.snapshot)?;
        let mut command = repo.operation_command();
        command
            .args(["submodule", "update", "--recursive", "--"])
            .arg(&self.snapshot.module.path);
        let outcome =
            crate::authenticated_command::run(&mut command, auth, token, progress, None, |_| {
                false
            })?;
        let checkout = submodules::inspect(&repo, &self.snapshot.module.path, &Cancel::default());
        Ok(Result { outcome, checkout })
    }
}

fn capture(
    parent: PathBuf,
    module: Module,
    token: &Cancel,
    keys: &mut BTreeSet<PathBuf>,
    count: &mut usize,
    depth: usize,
) -> std::result::Result<Snapshot, String> {
    *count += 1;
    if depth > 32 || *count > 4096 {
        return Err("Submodule hierarchy exceeds the supported size".into());
    }
    if !matches!(module.index, IndexState::Commit(_)) {
        return Err("Submodule requires a resolved gitlink in the index".into());
    }
    let repo = Repository::new(&parent);
    let checkout = submodules::inspect(&repo, &module.path, token)?;
    let mut children = Vec::new();
    let mut tracked_files = false;
    if matches!(checkout, Checkout::Initialized { .. }) {
        let root = parent.join(&module.path);
        let child = Repository::new(&root);
        if !keys.insert(child.operation_key(token)?) {
            return Err("Submodule hierarchy repeats a Git repository".into());
        }
        let state = child.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the submodule's Git operation before updating".into());
        }
        for module in submodules::list(&child, token)? {
            children.push(capture(
                root.clone(),
                module,
                token,
                keys,
                count,
                depth + 1,
            )?);
        }
        // A clean descendant checked out at an older gitlink makes its parent
        // look modified. It is an update target, not stashable file content.
        tracked_files = child.local_changes(token)?.iter().any(|change| {
            change.area != crate::git::Area::Untracked
                && !(change.area == crate::git::Area::Unstaged
                    && children
                        .iter()
                        .any(|child| child.module.path == change.path))
        });
    } else {
        // Deinitialized checkouts can still have a shared object/ref directory.
        // Reserve it too, so reinitialization cannot race an open linked worktree.
        let name = Path::new(&module.name);
        if name.as_os_str().is_empty()
            || name
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("Invalid submodule name".into());
        }
        let relative = Path::new("modules").join(name);
        let raw = repo.read(
            &[
                std::ffi::OsStr::new("rev-parse"),
                std::ffi::OsStr::new("--path-format=absolute"),
                std::ffi::OsStr::new("--git-path"),
                relative.as_os_str(),
            ],
            token,
        )?;
        let raw = raw.strip_suffix(b"\n").unwrap_or(&raw);
        let directory = crate::git::path_from_bytes(raw.strip_suffix(b"\r").unwrap_or(raw))?;
        match std::fs::symlink_metadata(&directory) {
            Ok(_) => {
                let key = Repository::new(directory).operation_key(token)?;
                if !keys.insert(key) {
                    return Err("Submodule hierarchy repeats a Git repository".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(Snapshot {
        parent,
        module,
        checkout,
        children,
        tracked_files,
    })
}
fn check_clean(snapshot: &Snapshot) -> std::result::Result<(), String> {
    match &snapshot.checkout {
        Checkout::Initialized {
            branch: Some(_), ..
        } => {
            return Err("Submodules on a branch require a branch update".into());
        }
        Checkout::Initialized { .. } if snapshot.tracked_files => {
            return Err("Preserve local submodule changes before updating".into());
        }
        Checkout::Initialized {
            untracked: true, ..
        } => {
            return Err("Local untracked submodule files require a preservation decision".into());
        }
        // `submodule update --recursive` without --init skips these nodes.
        // Keep any local placeholder files in place; never initialize implicitly.
        Checkout::Uninitialized { .. } => {}
        _ => {}
    }
    for child in &snapshot.children {
        check_clean(child)?;
    }
    Ok(())
}

pub use crate::project_update::preservation::SavedRoot;
pub struct Report {
    pub update: Option<std::result::Result<Outcome, String>>,
    pub saved: Vec<SavedRoot>,
    pub error: Option<String>,
    pub checkout: std::result::Result<Checkout, String>,
}
impl Report {
    pub fn succeeded(&self) -> bool {
        self.error.is_none()
            && self
                .update
                .as_ref()
                .is_some_and(|result| result.as_ref().is_ok_and(good))
            && self.saved.iter().all(|root| {
                root.saved_oid.is_none()
                    && root.save.as_ref().is_ok_and(good)
                    && root.restore.as_ref().is_none_or(|result| {
                        result
                            .as_ref()
                            .is_ok_and(|outcome| outcome.as_ref().is_none_or(good))
                    })
            })
    }
}
impl Update {
    /// Stash tracked edits in all dirty descendants, then update and restore.
    /// Like GitPreservingProcess, an error/cancelled update leaves those stashes
    /// available instead of trying to pop them into a partially updated tree.
    /// Untracked files stay on disk; Git detects collisions during checkout.
    pub fn execute_preserving(
        &self,
        token: &Cancel,
        progress: &Progress,
        auth: Option<(&Requests, &Path)>,
    ) -> std::result::Result<Report, String> {
        use crate::{project_update::preservation::SavedChanges, update_session::UpdateResult};
        let guard = crate::operation_queue::acquire_many(self.keys.clone(), token)?;
        let repo = Repository::new(&self.snapshot.parent);
        let current = Self::prepare(&repo, &self.snapshot.module.path, token)?;
        if current.keys != self.keys || current.snapshot != self.snapshot {
            return Err("Submodule state changed; refresh before updating".into());
        }
        check_detached(&current.snapshot)?;
        let mut dirty = Vec::new();
        dirty_roots(&current.snapshot, &mut dirty);
        let repositories: Vec<_> = dirty.iter().map(Repository::new).collect();
        let mut preservation = SavedChanges::new(&repositories, &guard, token)?;
        let mut error = preservation
            .save_all("Plico: before submodule update", token, progress)
            .err();
        let mut update = None;
        if error.is_none() {
            // Saving can itself reveal a different .gitmodules/index snapshot.
            // Do not reuse a now-different topology or target after releasing edits.
            match Self::prepare(&repo, &self.snapshot.module.path, token) {
                Ok(after_save)
                    if after_save.keys == self.keys
                        && same_targets(&self.snapshot, &after_save.snapshot) =>
                {
                    let mut command = repo.operation_command();
                    command
                        .args(["submodule", "update", "--recursive", "--"])
                        .arg(&self.snapshot.module.path);
                    update = Some(crate::authenticated_command::run(
                        &mut command,
                        auth,
                        token,
                        progress,
                        None,
                        |_| false,
                    ));
                }
                Ok(_) => {
                    error = Some(
                        "Submodule targets changed while saving; saved edits were retained".into(),
                    )
                }
                Err(e) => error = Some(e),
            }
        }
        let successful = update
            .as_ref()
            .is_some_and(|result| result.as_ref().is_ok_and(good));
        preservation.finish(
            if successful {
                UpdateResult::Success
            } else {
                UpdateResult::Error
            },
            true,
            progress,
        );
        let saved = preservation.into_reports();
        Ok(Report {
            update,
            saved,
            error,
            checkout: submodules::inspect(&repo, &self.snapshot.module.path, &Cancel::default()),
        })
    }
}
fn good(outcome: &Outcome) -> bool {
    outcome.status.success() && outcome.stopped.is_none() && outcome.input_error.is_none()
}
fn check_detached(snapshot: &Snapshot) -> std::result::Result<(), String> {
    match snapshot.checkout {
        Checkout::Initialized {
            branch: Some(_), ..
        } => {
            return Err("Submodules on a branch require a branch update".into());
        }
        // `submodule update --recursive` without --init skips these nodes.
        // Keep any local placeholder files in place; never initialize implicitly.
        Checkout::Uninitialized { .. } => {}
        _ => {}
    }
    for child in &snapshot.children {
        check_detached(child)?;
    }
    Ok(())
}
fn dirty_roots(snapshot: &Snapshot, roots: &mut Vec<PathBuf>) {
    for child in &snapshot.children {
        dirty_roots(child, roots);
    }
    if snapshot.tracked_files {
        roots.push(snapshot.parent.join(&snapshot.module.path));
    }
}
fn same_targets(before: &Snapshot, after: &Snapshot) -> bool {
    let identity = |checkout: &Checkout| match checkout {
        Checkout::Initialized { head, branch, .. } => Some((head.clone(), branch.clone())),
        _ => None,
    };
    before.parent == after.parent
        && before.module == after.module
        && identity(&before.checkout) == identity(&after.checkout)
        && before.children.len() == after.children.len()
        && before
            .children
            .iter()
            .zip(&after.children)
            .all(|(a, b)| same_targets(a, b))
}

impl Update {
    pub(crate) fn path(&self) -> PathBuf {
        self.snapshot.parent.join(&self.snapshot.module.path)
    }
    pub(crate) fn reserved_keys(&self) -> &[PathBuf] {
        &self.keys
    }
    pub(crate) fn dirty_repositories(&self) -> Vec<Repository> {
        let mut paths = Vec::new();
        dirty_roots(&self.snapshot, &mut paths);
        paths.into_iter().map(Repository::new).collect()
    }
    /// Update Project saves editor documents after confirmation. Re-read the
    /// worktree under the group reservation, without recapturing its identity.
    /// Standalone update entry points retain their strict snapshot checks.
    pub(crate) fn refresh_worktree_reserved(
        &mut self,
        guard: &crate::operation_queue::Guard,
        token: &Cancel,
    ) -> std::result::Result<(), String> {
        let parent = Repository::new(&self.snapshot.parent);
        let current = Self::prepare(&parent, &self.snapshot.module.path, token)?;
        if current.keys != self.keys
            || !current.keys.iter().all(|key| guard.owns(key))
            || !same_targets(&self.snapshot, &current.snapshot)
        {
            return Err("Submodule state changed; refresh before updating".into());
        }
        check_detached(&current.snapshot)?;
        self.snapshot = current.snapshot;
        Ok(())
    }
    /// Parent integration can advance this gitlink; the submodule's own HEAD,
    /// branches and nested declarations must still match the saved selection.
    pub(crate) fn execute_reserved(
        &self,
        guard: &crate::operation_queue::Guard,
        token: &Cancel,
        progress: &Progress,
        auth: Option<(&Requests, &Path)>,
    ) -> std::result::Result<Outcome, String> {
        let parent = Repository::new(&self.snapshot.parent);
        let current = Self::prepare(&parent, &self.snapshot.module.path, token)?;
        if !current.keys.iter().all(|key| guard.owns(key))
            || !same_checkout_targets(&self.snapshot, &current.snapshot)
        {
            return Err("Submodule checkout changed during project update".into());
        }
        check_detached(&current.snapshot)?;
        let mut command = parent.operation_command();
        command
            .args(["submodule", "update", "--recursive", "--"])
            .arg(&self.snapshot.module.path);
        crate::authenticated_command::run(&mut command, auth, token, progress, None, |_| false)
    }
}
fn same_checkout_targets(before: &Snapshot, after: &Snapshot) -> bool {
    let identity = |checkout: &Checkout| match checkout {
        Checkout::Initialized { head, branch, .. } => Some((head.clone(), branch.clone())),
        _ => None,
    };
    before.parent == after.parent
        && before.module.path == after.module.path
        && before.module.name == after.module.name
        && identity(&before.checkout) == identity(&after.checkout)
        && before.children.len() == after.children.len()
        && before
            .children
            .iter()
            .zip(&after.children)
            .all(|(a, b)| same_targets(a, b))
}
