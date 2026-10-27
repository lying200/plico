//! Rebased 4aec352 GitDefaultBranchesPopupStep and GitRefUtil scopes.
//! A shared name is not a shared commit: each target retains its repository.
use crate::git::{Cancel, Repository, Snapshot};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub struct Root {
    pub repository: Repository,
    pub references: Result<std::sync::Arc<Snapshot>, String>,
}
#[derive(Default)]
pub struct Project {
    pub roots: Vec<Root>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Empty,
    Repository(PathBuf),
    Common,
    Search,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Target<'a> {
    pub root: &'a Path,
    pub commit: &'a str,
}
impl Project {
    /// Background-only; errors retain their root rather than silently changing
    /// the scope of a common-reference operation. Cancellation aborts the load.
    pub fn read(repositories: &[Repository], token: &Cancel) -> Result<Self, String> {
        if repositories.len() > 4096 {
            return Err("Project repository count exceeds the supported size".into());
        }
        let mut roots = Vec::new();
        let mut seen = BTreeSet::new();
        for repository in repositories {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            if !seen.insert(repository.path.clone()) {
                continue;
            }
            roots.push(Root {
                repository: repository.clone(),
                references: repository
                    .reference_snapshot(token)
                    .map(std::sync::Arc::new),
            });
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(Self { roots })
    }
    /// Search always spans the project. A repository submenu passes a
    /// single-root Project, independently of the parent popup's mode.
    pub fn scope(&self, selected: Option<&Path>, all_roots: bool, searching: bool) -> Scope {
        if self.roots.is_empty() {
            return Scope::Empty;
        }
        if self.roots.len() == 1 {
            return Scope::Repository(self.roots[0].repository.path.clone());
        }
        if searching {
            return Scope::Search;
        }
        if !all_roots
            && let Some(selected) = selected
            && self
                .roots
                .iter()
                .any(|root| root.repository.path == selected)
        {
            return Scope::Repository(selected.to_owned());
        }
        Scope::Common
    }
    /// Common branches/tags are an intersection by full ref name, never OID.
    /// An unreadable member cannot be omitted to produce a misleading "all".
    pub fn common_references(&self) -> BTreeSet<&str> {
        let mut sets = self.roots.iter().map(|root| {
            root.references
                .as_ref()
                .map(|refs| {
                    refs.branches
                        .iter()
                        .map(|r| r.name.as_str())
                        .chain(refs.tags.iter().map(|r| r.name.as_str()))
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default()
        });
        let Some(mut common) = sets.next() else {
            return BTreeSet::new();
        };
        for refs in sets {
            common.retain(|name| refs.contains(name));
        }
        common
    }
    /// A common branch only has an upstream label when every affected root
    /// tracks the same remote branch. Errors and missing tracking hide the label.
    pub fn tracked_branch(&self, scope: &Scope, reference: &str) -> Option<&str> {
        let roots: Vec<_> = match scope {
            Scope::Repository(path) => self
                .roots
                .iter()
                .filter(|root| &root.repository.path == path)
                .collect(),
            Scope::Common => self.roots.iter().collect(),
            _ => return None,
        };
        let mut names = roots.into_iter().map(|root| {
            root.references
                .as_ref()
                .ok()?
                .tracked_branches
                .get(reference)
                .map(String::as_str)
        });
        let first = names.next()??;
        names.all(|name| name == Some(first)).then_some(first)
    }
    /// An explicit repository node/reference always overrides project-wide
    /// operation scope. Callers cannot execute an unscoped search result.
    pub fn targets(&self, scope: &Scope, reference: &str) -> Result<Vec<Target<'_>>, String> {
        let members: Vec<_> = match scope {
            Scope::Repository(path) => {
                vec![
                    self.roots
                        .iter()
                        .find(|root| &root.repository.path == path)
                        .ok_or("Repository no longer belongs to the project")?,
                ]
            }
            Scope::Common if !self.roots.is_empty() => self.roots.iter().collect(),
            _ => return Err("Select a repository or a common reference".into()),
        };
        members
            .into_iter()
            .map(|root| {
                let refs = root.references.as_ref().map_err(Clone::clone)?;
                let commit = refs
                    .branches
                    .iter()
                    .map(|r| (&r.name, &r.id))
                    .chain(refs.tags.iter().map(|r| (&r.name, &r.id)))
                    .find(|(name, _)| name.as_str() == reference)
                    .map(|(_, commit)| commit.as_str())
                    .ok_or("Reference is unavailable in a target repository")?;
                Ok(Target {
                    root: &root.repository.path,
                    commit,
                })
            })
            .collect()
    }
}
