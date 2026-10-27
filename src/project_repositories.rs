//! Project repository membership, separate from Git operation eligibility.
//! Rebased 4aec352: VcsRepositoryManager and GitRepositoryManager (Apache-2.0).
use crate::{
    git::{Cancel, Repository},
    submodules::{self, IndexState, Located},
};
use std::{collections::BTreeSet, path::PathBuf};
mod context;
mod store;
pub use store::Store;

/// Project membership outlives the gitlinks that originally discovered it.
/// Keep this owner across scans; a failed/cancelled scan never changes mappings.
/// Mappings are not permission to write: every scan validates actual Git roots.
#[derive(Clone)]
pub struct Mappings {
    registered: Vec<Repository>,
}
impl Mappings {
    pub fn new(registered: Vec<Repository>) -> Self {
        Self { registered }
    }

    pub fn registered(&self) -> &[Repository] {
        &self.registered
    }

    /// Background-only discovery with durable membership. Storage failures are
    /// visible alongside per-root errors without hiding readable repositories.
    pub fn refresh_persisted(
        &mut self,
        store: &Store,
        token: &Cancel,
    ) -> Result<Repositories, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let mut next = self.clone();
        let loaded = store.load_into(&mut next);
        let mut inventory = next.refresh(token)?;
        let stored = match loaded {
            Ok(()) => store.save(&next, token),
            Err(error) => Err(error),
        };
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if let Err(error) = stored {
            let primary = self
                .registered
                .first()
                .map(|r| r.path.clone())
                .unwrap_or_default();
            inventory
                .errors
                .push((primary, format!("Project repository mappings: {error}")));
        }
        *self = next;
        Ok(inventory)
    }

    pub fn refresh(&mut self, token: &Cancel) -> Result<Repositories, String> {
        let inventory = Repositories::from_registered(&self.registered, token)?;
        let mut registered = self.registered.clone();
        let mut keys = registered
            .iter()
            .map(|repo| crate::recent_projects::identity(&repo.path))
            .collect::<BTreeSet<_>>();
        for repo in &inventory.roots {
            if keys.insert(crate::recent_projects::identity(&repo.path)) {
                registered.push(repo.clone());
            }
        }
        if registered.len() > 4096 {
            return Err("Project repository count exceeds the supported size".into());
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        self.registered = registered;
        Ok(inventory)
    }
}

/// A declared gitlink relationship, distinct from shared Git storage.
/// Uninitialized relationships are metadata, never readable repositories.
pub struct SubmoduleRelation {
    pub parent: PathBuf,
    pub path: PathBuf,
    pub root: PathBuf,
    pub index: IndexState,
    pub initialized: bool,
}

/// Project repository inventory, independent of operation eligibility.
/// A broken root does not hide other readable roots; callers own content budgets.
#[derive(Default)]
pub struct Repositories {
    pub roots: Vec<Repository>,
    pub errors: Vec<(PathBuf, String)>,
    pub submodules: Vec<SubmoduleRelation>,
}
impl Repositories {
    pub fn discover(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        // Preserve the existing single-project failure contract.
        let root = Repository::new(submodules::worktree_root(repo, token)?);
        Self::from_registered(std::slice::from_ref(&root), token)
    }

    /// Explicit project roots only. Ordinary repositories and worktrees enter
    /// through the same seam; initialized submodules are discovered beneath them.
    /// No ancestor-directory scan or common-dir identity deduplication.
    pub fn from_registered(registered: &[Repository], token: &Cancel) -> Result<Self, String> {
        if registered.len() > 4096 {
            return Err("Project repository count exceeds the supported size".into());
        }
        let mut result = Self::default();
        let mut seen = BTreeSet::new();
        let mut remaining = 4096;
        for repo in registered {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let root = submodules::worktree_root(repo, token).and_then(|path| {
                let key = path.canonicalize().map_err(|e| e.to_string())?;
                if key != repo.path.canonicalize().map_err(|e| e.to_string())? {
                    return Err("Registered repository path is not a Git worktree root".into());
                }
                Ok((path, key))
            });
            let (root, key) = match root {
                Ok(root) => root,
                Err(error) => {
                    result.errors.push((repo.path.clone(), error));
                    continue;
                }
            };
            if !seen.insert(key) {
                continue;
            }
            if remaining == 0 {
                return Err("Project repository count exceeds the supported size".into());
            }
            remaining -= 1;
            let root = Repository::new(root);
            result.roots.push(root.clone());
            result.visit(&root, token, &mut seen, &mut remaining, 0)?;
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(result)
    }

    fn visit(
        &mut self,
        parent: &Repository,
        token: &Cancel,
        seen: &mut BTreeSet<PathBuf>,
        remaining: &mut usize,
        depth: usize,
    ) -> Result<(), String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if depth >= 32 {
            self.errors.push((
                parent.path.clone(),
                "Submodule hierarchy exceeds the supported depth".into(),
            ));
            return Ok(());
        }
        let modules = match submodules::list(parent, token) {
            Ok(modules) => modules,
            Err(error) => {
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                self.errors.push((parent.path.clone(), error));
                return Ok(());
            }
        };
        for module in modules {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            if *remaining == 0 {
                self.errors.push((
                    parent.path.clone(),
                    "Submodule hierarchy exceeds the supported size".into(),
                ));
                break;
            }
            *remaining -= 1;
            if module.index == IndexState::NotTracked {
                continue;
            }
            // Conflicting gitlinks still identify a valid local repository.
            // Do not run update eligibility checks here: this is recovery.
            let path = parent.path.join(&module.path);
            let child = match submodules::initialized_repository(parent, &module.path, token) {
                Ok(Located::Initialized(child)) => child,
                Ok(Located::Uninitialized { .. }) => {
                    self.submodules.push(SubmoduleRelation {
                        parent: parent.path.clone(),
                        path: module.path.clone(),
                        root: path,
                        index: module.index.clone(),
                        initialized: false,
                    });
                    continue;
                }
                Err(error) => {
                    self.errors.push((path, error));
                    continue;
                }
            };
            match child.path.canonicalize().map_err(|e| e.to_string()) {
                Ok(key) => {
                    self.submodules.push(SubmoduleRelation {
                        parent: parent.path.clone(),
                        path: module.path.clone(),
                        root: child.path.clone(),
                        index: module.index.clone(),
                        initialized: true,
                    });
                    if seen.insert(key) {
                        self.roots.push(child.clone());
                        self.visit(&child, token, seen, remaining, depth + 1)?;
                    }
                }
                Err(error) => self.errors.push((path, error)),
            }
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(())
    }
}
