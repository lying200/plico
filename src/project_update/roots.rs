//! Registered project roots for GitUpdateProcess's two updater groups.
//! Rebased 4aec352: tracked branches first, then detached submodules whose
//! parent is not itself a detached submodule (those update recursively).
use crate::{
    git::{Cancel, Repository},
    merge_update::Request,
    project_repositories::Repositories,
    submodules::IndexState,
};
use std::{collections::BTreeMap, path::PathBuf};

pub struct Branch {
    pub repository: Repository,
    pub request: Request,
}
pub struct Detached {
    pub parent: Repository,
    pub path: PathBuf,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    Uninitialized,
    NoUpstream,
    DetachedHead,
}
pub struct Skipped {
    pub path: PathBuf,
    pub reason: SkipReason,
}
/// Read-only plan, not authorization to skip executor revalidation. In
/// particular a parent update can change gitlinks before detached roots run.
#[derive(Default)]
pub struct Roots {
    pub branches: Vec<Branch>,
    pub detached: Vec<Detached>,
    pub skipped: Vec<Skipped>,
}
impl Roots {
    pub fn discover(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        let inventory = Repositories::discover(repo, token)?;
        let mut roots = Self::from_inventory(&inventory, token)?;
        if let Some(primary) = inventory.roots.first() {
            roots
                .branches
                .retain(|branch| branch.repository.path != primary.path);
            roots.skipped.retain(|root| root.path != primary.path);
        }
        Ok(roots)
    }

    /// Membership is shared with read-only views; update eligibility is not.
    /// All registered roots are peers; submodule relationships only affect detached routing.
    pub(super) fn from_inventory(inventory: &Repositories, token: &Cancel) -> Result<Self, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if let Some((path, error)) = inventory.errors.first() {
            return Err(format!("{}: {error}", path.display()));
        }
        if inventory
            .submodules
            .iter()
            .any(|module| module.index == IndexState::Conflict)
        {
            return Err("Resolve submodule gitlink conflicts before updating".into());
        }
        let mut detached_states = BTreeMap::new();
        let mut result = Self::default();
        for module in &inventory.submodules {
            if !module.initialized {
                result.skipped.push(Skipped {
                    path: module.root.clone(),
                    reason: SkipReason::Uninitialized,
                });
            }
        }
        for child in &inventory.roots {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let state = child.operation_state(token)?;
            if state.in_progress() || !state.conflicts.is_empty() {
                return Err(format!(
                    "Finish the Git operation in {} before updating",
                    child.path.display()
                ));
            }
            detached_states.insert(child.path.clone(), state.detached);
            if state.detached {
                let Some(module) = inventory
                    .submodules
                    .iter()
                    .find(|m| m.initialized && m.root == child.path)
                else {
                    result.skipped.push(Skipped {
                        path: child.path.clone(),
                        reason: SkipReason::DetachedHead,
                    });
                    continue;
                };
                let parent = Repository::new(&module.parent);
                let parent_detached = match detached_states.get(&module.parent) {
                    Some(detached) => *detached,
                    None => {
                        let detached = parent.operation_state(token)?.detached;
                        detached_states.insert(module.parent.clone(), detached);
                        detached
                    }
                };
                let parent_is_detached_submodule = parent_detached
                    && inventory
                        .submodules
                        .iter()
                        .any(|candidate| candidate.initialized && candidate.root == module.parent);
                if !parent_is_detached_submodule {
                    result.detached.push(Detached {
                        parent,
                        path: module.path.clone(),
                    });
                }
            } else if child.pull_targets(token)?.tracked.is_some() {
                result.branches.push(Branch {
                    repository: child.clone(),
                    request: super::prepare(child, token)?,
                });
            } else {
                result.skipped.push(Skipped {
                    path: child.path.clone(),
                    reason: SkipReason::NoUpstream,
                });
            }
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(result)
    }
}
