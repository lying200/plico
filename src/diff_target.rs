//! Lightweight, immutable identity of an opened review. No file contents retained.
use crate::git::{Area, Cancel, Change, Commit, Repository, Result};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identity {
    pub repository: PathBuf,
    pub revision: Option<String>,
    pub parents: Vec<String>,
    pub area: Area,
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub combined: Option<crate::combined_changes::Change>,
    pub working_comparison: Option<crate::working_comparison::Change>,
}

#[derive(Clone, Debug)]
pub struct Target {
    identity: Identity,
    repository: Repository,
    commit: Option<Commit>,
    change: Change,
}

impl Target {
    pub fn new(repository: Repository, commit: Option<Commit>, change: Change) -> Result<Self> {
        if change.area == Area::Ignored {
            return Err("Ignored files have no review diff".into());
        }
        if (change.area == Area::Commit) != commit.is_some() {
            return Err("Diff target does not match its revision area".into());
        }
        let identity = Identity {
            repository: repository.path.clone(),
            revision: commit.as_ref().map(|c| c.id.clone()),
            parents: commit
                .as_ref()
                .map(|c| c.parents.clone())
                .unwrap_or_default(),
            area: change.area,
            path: change.path.clone(),
            old_path: change.old_path.clone(),
            combined: None,
            working_comparison: None,
        };
        Ok(Self {
            identity,
            repository,
            commit,
            change,
        })
    }

    pub fn combined(
        repository: Repository,
        endpoints: crate::combined_changes::Change,
    ) -> Result<Self> {
        let side = endpoints
            .after
            .as_ref()
            .or(endpoints.before.as_ref())
            .ok_or("Change has no revisions")?;
        for revision in endpoints.before.iter().chain(endpoints.after.iter()) {
            if !crate::git::is_oid(&revision.revision)
                || revision.path.as_os_str().is_empty()
                || revision
                    .path
                    .components()
                    .any(|p| !matches!(p, std::path::Component::Normal(_)))
            {
                return Err("Invalid preview revision or file path".into());
            }
        }
        let status = match (&endpoints.before, &endpoints.after) {
            (None, _) => "A",
            (_, None) => "D",
            (Some(a), Some(b)) if a.path != b.path => "R",
            _ => "M",
        };
        let change = Change {
            area: Area::Commit,
            path: side.path.clone(),
            old_path: endpoints
                .before
                .as_ref()
                .filter(|r| r.path != side.path)
                .map(|r| r.path.clone()),
            status: status.into(),
        };
        let identity = Identity {
            repository: repository.path.clone(),
            revision: endpoints.after.as_ref().map(|r| r.revision.clone()),
            parents: endpoints
                .before
                .iter()
                .map(|r| r.revision.clone())
                .collect(),
            area: Area::Commit,
            path: change.path.clone(),
            old_path: change.old_path.clone(),
            combined: Some(endpoints),
            working_comparison: None,
        };
        Ok(Self {
            identity,
            repository,
            commit: None,
            change,
        })
    }
    pub fn working_comparison(
        repository: Repository,
        endpoints: crate::working_comparison::Change,
    ) -> Result<Self> {
        let side = endpoints
            .after
            .as_ref()
            .or(endpoints.before.as_ref())
            .ok_or("Change has no endpoints")?;
        for endpoint in endpoints.before.iter().chain(endpoints.after.iter()) {
            if endpoint.path().as_os_str().is_empty()
                || endpoint
                    .path()
                    .components()
                    .any(|p| !matches!(p, std::path::Component::Normal(_)))
            {
                return Err("Invalid comparison path".into());
            }
            if let crate::working_comparison::Endpoint::Revision { commit, .. } = endpoint {
                if !crate::git::is_oid(commit) {
                    return Err("Invalid comparison revision".into());
                }
            }
        }
        let change = Change {
            area: Area::WorkingTree,
            path: side.path().to_path_buf(),
            old_path: endpoints
                .before
                .as_ref()
                .filter(|e| e.path() != side.path())
                .map(|e| e.path().to_path_buf()),
            status: endpoints.status().into(),
        };
        let identity = Identity {
            repository: repository.path.clone(),
            revision: None,
            parents: vec![],
            area: Area::WorkingTree,
            path: change.path.clone(),
            old_path: change.old_path.clone(),
            combined: None,
            working_comparison: Some(endpoints),
        };
        Ok(Self {
            identity,
            repository,
            commit: None,
            change,
        })
    }
    pub fn content_paths(&self) -> Option<(Option<&std::path::Path>, Option<&std::path::Path>)> {
        if let Some(c) = &self.identity.working_comparison {
            return Some((
                c.before.as_ref().map(|e| e.path()),
                c.after.as_ref().map(|e| e.path()),
            ));
        }
        self.identity.combined.as_ref().map(|c| {
            (
                c.before.as_ref().map(|e| e.path.as_path()),
                c.after.as_ref().map(|e| e.path.as_path()),
            )
        })
    }
    pub fn read_contents(
        &self,
        cancel: &Cancel,
    ) -> Option<Result<crate::combined_changes::Contents>> {
        if let Some(c) = &self.identity.working_comparison {
            return Some(c.read(&self.repository, cancel));
        }
        self.identity
            .combined
            .as_ref()
            .map(|c| crate::combined_changes::read_contents(&self.repository, c, cancel))
    }

    /// Candidate local document, independent of display orientation. The caller must still
    /// validate the on-disk file (encoding, size, links and write permissions).
    /// Never infer editability from a translated side caption or its position.
    pub fn working_tree_document(&self) -> Option<&std::path::Path> {
        if let Some(change) = &self.identity.working_comparison {
            return change.working_path();
        }
        if self.identity.combined.is_some()
            || !matches!(
                self.change.area,
                Area::WorkingTree | Area::Unstaged | Area::Untracked
            )
            || self.change.status == "D"
        {
            return None;
        }
        Some(&self.change.path)
    }

    /// Gitlinks are commit identities, not editable files in the parent repo.
    /// Query in the loader worker; never perform Git IO during rendering.
    pub fn is_working_gitlink(&self, cancel: &Cancel) -> Result<bool> {
        if self.working_tree_document().is_none() {
            return Ok(false);
        }
        use std::ffi::OsStr;
        let records = self.repository.read(
            &[
                OsStr::new("--literal-pathspecs"),
                OsStr::new("ls-files"),
                OsStr::new("--stage"),
                OsStr::new("-z"),
                OsStr::new("--"),
                self.source_path().as_os_str(),
            ],
            cancel,
        )?;
        let expected = crate::commit_operation::path_input(&[self.source_path().to_path_buf()])?;
        Ok(records.split(|b| *b == 0).any(|record| {
            record.iter().position(|b| *b == b'\t').is_some_and(|tab| {
                record[..tab].starts_with(b"160000 ")
                    && record[tab + 1..] == expected[..expected.len() - 1]
            })
        }))
    }

    /// Source navigation follows the local endpoint even when sides are reversed.
    pub fn source_path(&self) -> &std::path::Path {
        self.identity
            .working_comparison
            .as_ref()
            .and_then(|c| {
                c.before.iter().chain(c.after.iter()).find_map(|e| match e {
                    crate::working_comparison::Endpoint::WorkingTree { path } => {
                        Some(path.as_path())
                    }
                    _ => None,
                })
            })
            .unwrap_or(&self.identity.path)
    }
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn change(&self) -> &Change {
        &self.change
    }
    pub fn read_full(&self, cancel: &Cancel) -> Result<String> {
        if self.identity.working_comparison.is_some() || self.identity.combined.is_some() {
            return Err("Use revision contents for a combined target".into());
        }
        self.repository
            .full_patch(self.commit.as_ref(), &self.change, cancel)
    }
    /// Both embedded previews and editor tabs retain complete source context.
    pub fn read(&self, cancel: &Cancel) -> Result<String> {
        if self.identity.working_comparison.is_some() || self.identity.combined.is_some() {
            return Err("Use revision contents for a combined target".into());
        }
        self.repository
            .full_patch(self.commit.as_ref(), &self.change, cancel)
    }
}

/// Immutable metadata shared by tabs reviewing files in the same commit.
#[derive(Debug)]
struct ChainData {
    repository: Repository,
    commit: Option<Commit>,
    changes: Vec<Change>,
    combined: Option<Vec<crate::combined_changes::Change>>,
    working_comparison: Option<Vec<crate::working_comparison::Change>>,
}

/// Only the cursor belongs to a tab. No file contents are cached here.
#[derive(Clone, Debug)]
pub struct Chain {
    data: std::sync::Arc<ChainData>,
    index: usize,
}
impl Chain {
    pub fn new(target: &Target, changes: &[Change]) -> Result<Self> {
        if target.identity.working_comparison.is_some() || target.identity.combined.is_some() {
            return Err("Combined targets require their revision chain".into());
        }
        let changes: Vec<Change> = changes
            .iter()
            .filter(|c| c.area == target.change.area)
            .cloned()
            .collect();
        let index = changes
            .iter()
            .position(|c| same_change(c, &target.change))
            .ok_or("Selected file is absent from the diff chain")?;
        Ok(Self {
            data: std::sync::Arc::new(ChainData {
                repository: target.repository.clone(),
                commit: target.commit.clone(),
                changes,
                combined: None,
                working_comparison: None,
            }),
            index,
        })
    }
    pub fn combined(
        repository: Repository,
        endpoints: Vec<crate::combined_changes::Change>,
        index: usize,
    ) -> Result<(Target, Self)> {
        let target = Target::combined(
            repository.clone(),
            endpoints
                .get(index)
                .ok_or("Invalid diff file index")?
                .clone(),
        )?;
        let changes = endpoints
            .iter()
            .map(|c| Target::combined(repository.clone(), c.clone()).map(|t| t.change))
            .collect::<Result<Vec<_>>>()?;
        let chain = Self {
            data: std::sync::Arc::new(ChainData {
                repository,
                commit: None,
                changes,
                combined: Some(endpoints),
                working_comparison: None,
            }),
            index,
        };
        Ok((target, chain))
    }
    pub fn working_comparison(
        repository: Repository,
        endpoints: Vec<crate::working_comparison::Change>,
        index: usize,
    ) -> Result<(Target, Self)> {
        let target = Target::working_comparison(
            repository.clone(),
            endpoints
                .get(index)
                .ok_or("Invalid diff file index")?
                .clone(),
        )?;
        let changes = endpoints
            .iter()
            .map(|c| Target::working_comparison(repository.clone(), c.clone()).map(|t| t.change))
            .collect::<Result<Vec<_>>>()?;
        Ok((
            target,
            Self {
                data: std::sync::Arc::new(ChainData {
                    repository,
                    commit: None,
                    changes,
                    combined: None,
                    working_comparison: Some(endpoints),
                }),
                index,
            },
        ))
    }
    /// Swap only the stored endpoints; do not reread Git or change the selected file.
    pub fn reversed_working_comparison(&self) -> Result<Self> {
        let endpoints = self
            .data
            .working_comparison
            .as_ref()
            .ok_or("Not a working tree comparison")?;
        Self::working_comparison(
            self.data.repository.clone(),
            endpoints
                .iter()
                .map(crate::working_comparison::Change::reversed)
                .collect(),
            self.index,
        )
        .map(|(_, chain)| chain)
    }
    /// Reuse only immutable committed snapshots. Local/index file lists can change.
    pub fn fork_for(&self, target: &Target) -> Option<Self> {
        if let Some(endpoints) = &self.data.working_comparison {
            if self.data.repository.path != target.repository.path {
                return None;
            }
            let index = endpoints
                .iter()
                .position(|c| Some(c) == target.identity.working_comparison.as_ref())?;
            return Some(Self {
                data: self.data.clone(),
                index,
            });
        }
        if let Some(endpoints) = &self.data.combined {
            if self.data.repository.path != target.repository.path {
                return None;
            }
            let index = endpoints
                .iter()
                .position(|c| Some(c) == target.identity.combined.as_ref())?;
            return Some(Self {
                data: self.data.clone(),
                index,
            });
        }
        if target.identity.working_comparison.is_some() || target.identity.combined.is_some() {
            return None;
        }
        let commit = self.data.commit.as_ref()?;
        let other = target.commit.as_ref()?;
        if self.data.repository.path != target.repository.path
            || commit.id != other.id
            || commit.parents != other.parents
            || commit.shallow_boundary != other.shallow_boundary
        {
            return None;
        }
        let index = self
            .data
            .changes
            .iter()
            .position(|c| same_change(c, &target.change))?;
        Some(Self {
            data: self.data.clone(),
            index,
        })
    }
    /// Reorder a complete chain without changing revision endpoints or the selected file.
    pub fn reordered(&self, order: &[usize]) -> Option<Self> {
        if order.len() != self.len() {
            return None;
        }
        self.subset(order)
    }
    /// Select an ordered, unique subset containing the current target.
    pub fn subset(&self, order: &[usize]) -> Option<Self> {
        let mut seen = vec![false; self.len()];
        for &i in order {
            if i >= seen.len() || std::mem::replace(&mut seen[i], true) {
                return None;
            }
        }
        let index = order.iter().position(|&i| i == self.index)?;
        Some(Self {
            data: std::sync::Arc::new(ChainData {
                repository: self.data.repository.clone(),
                commit: self.data.commit.clone(),
                changes: order
                    .iter()
                    .map(|&i| self.data.changes[i].clone())
                    .collect(),
                combined: self
                    .data
                    .combined
                    .as_ref()
                    .map(|items| order.iter().map(|&i| items[i].clone()).collect()),
                working_comparison: self
                    .data
                    .working_comparison
                    .as_ref()
                    .map(|items| order.iter().map(|&i| items[i].clone()).collect()),
            }),
            index,
        })
    }
    pub fn changes(&self) -> &[Change] {
        &self.data.changes
    }
    pub fn select(&mut self, index: usize) -> Option<Target> {
        if let Some(endpoints) = &self.data.working_comparison {
            let target = Target::working_comparison(
                self.data.repository.clone(),
                endpoints.get(index)?.clone(),
            )
            .ok()?;
            self.index = index;
            return Some(target);
        }
        if let Some(endpoints) = &self.data.combined {
            let target =
                Target::combined(self.data.repository.clone(), endpoints.get(index)?.clone())
                    .ok()?;
            self.index = index;
            return Some(target);
        }
        let change = self.data.changes.get(index)?.clone();
        let target = Target::new(
            self.data.repository.clone(),
            self.data.commit.clone(),
            change,
        )
        .ok()?;
        self.index = index;
        Some(target)
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn len(&self) -> usize {
        self.data.changes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.changes.is_empty()
    }
    pub fn step(&mut self, next: bool) -> Option<Target> {
        let index = if next {
            self.index.checked_add(1)?
        } else {
            self.index.checked_sub(1)?
        };
        self.select(index)
    }
}
fn same_change(a: &Change, b: &Change) -> bool {
    a.area == b.area && a.path == b.path && a.old_path == b.old_path && a.status == b.status
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn editable_endpoint_requires_a_live_working_tree_side() {
        let repo = Repository::new("fixture");
        for area in [
            Area::WorkingTree,
            Area::Unstaged,
            Area::Untracked,
            Area::Staged,
            Area::Conflict,
        ] {
            for status in ["M", "A", "R", "D"] {
                let target = Target::new(
                    repo.clone(),
                    None,
                    Change {
                        area,
                        status: status.into(),
                        path: "new.rs".into(),
                        old_path: Some("old.rs".into()),
                    },
                )
                .unwrap();
                let allowed = matches!(area, Area::WorkingTree | Area::Unstaged | Area::Untracked)
                    && status != "D";
                assert_eq!(
                    target.working_tree_document(),
                    allowed.then_some(std::path::Path::new("new.rs"))
                );
            }
        }
        // A revision snapshot remains immutable even when its path is local.
        let revision = crate::combined_changes::Revision {
            revision: "a".repeat(40),
            path: "new.rs".into(),
        };
        let target = Target::combined(
            repo,
            crate::combined_changes::Change {
                before: None,
                after: Some(revision),
            },
        )
        .unwrap();
        assert!(target.working_tree_document().is_none());
    }

    #[test]
    fn combined_chain_keeps_revision_identity_for_same_path_and_independent_cursors() {
        use crate::combined_changes::{Change as Combined, Revision};
        let repo = Repository::new("fixture");
        let revision = |n: char| Revision {
            revision: n.to_string().repeat(40),
            path: "same".into(),
        };
        let deleted = Combined {
            before: Some(revision('a')),
            after: None,
        };
        let added = Combined {
            before: None,
            after: Some(revision('b')),
        };
        let (target, mut chain) =
            Chain::combined(repo.clone(), vec![deleted.clone(), added.clone()], 0).unwrap();
        let mut fork = chain.fork_for(&target).unwrap();
        assert!(Arc::ptr_eq(&chain.data, &fork.data));
        let next = fork.step(true).unwrap();
        assert_ne!(target.identity(), next.identity());
        assert_eq!(next.identity().combined.as_ref(), Some(&added));
        assert_eq!(chain.index(), 0);
        assert_eq!(fork.index(), 1);
        assert!(fork.step(true).is_none());
        assert_eq!(fork.index(), 1);
        assert_eq!(chain.select(1).unwrap().identity(), next.identity());
        assert!(
            chain
                .fork_for(&Target::combined(Repository::new("other"), added).unwrap())
                .is_none()
        );
        let different = Combined {
            before: Some(revision('c')),
            after: None,
        };
        assert!(
            chain
                .fork_for(&Target::combined(repo.clone(), different).unwrap())
                .is_none()
        );
        assert!(Chain::new(&target, chain.changes()).is_err());
        assert!(Chain::combined(repo.clone(), vec![deleted], 1).is_err());
        assert!(
            Target::combined(
                repo,
                Combined {
                    before: None,
                    after: None
                }
            )
            .is_err()
        );
    }

    #[test]
    fn many_tabs_share_metadata_keep_independent_cursors_and_release_it() {
        let repository = Repository::new("fixture");
        let commit = Commit {
            id: "a".repeat(40),
            parents: vec!["b".repeat(40)],
            shallow_boundary: false,
            author: "author".into(),
            date: "date".into(),
            subject: "review".into(),
            refs: vec![],
        };
        let changes: Vec<_> = (0..10_000)
            .map(|i| Change {
                path: format!("directory/file-{i:05}.rs").into(),
                old_path: None,
                status: "M".into(),
                area: Area::Commit,
            })
            .collect();
        let target =
            Target::new(repository.clone(), Some(commit.clone()), changes[0].clone()).unwrap();
        let chain = Chain::new(&target, &changes).unwrap();
        let weak = Arc::downgrade(&chain.data);
        let mut tabs: Vec<_> = (0..100)
            .map(|i| {
                let target =
                    Target::new(repository.clone(), Some(commit.clone()), changes[i].clone())
                        .unwrap();
                chain.fork_for(&target).unwrap()
            })
            .collect();
        assert!(tabs.iter().all(|tab| Arc::ptr_eq(&tab.data, &chain.data)));
        tabs[0].step(true).unwrap();
        assert_eq!(tabs[0].index(), 1);
        assert_eq!(chain.index(), 0);
        assert_eq!(tabs[99].index(), 99);
        drop(chain);
        assert!(weak.upgrade().is_some());
        drop(tabs);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn sharing_rejects_different_repositories_revisions_and_live_lists() {
        let repository = Repository::new("fixture");
        let change = Change {
            path: "a".into(),
            old_path: None,
            status: "M".into(),
            area: Area::Commit,
        };
        let commit = Commit {
            id: "a".repeat(40),
            parents: vec![],
            shallow_boundary: false,
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            refs: vec![],
        };
        let target = Target::new(repository.clone(), Some(commit.clone()), change.clone()).unwrap();
        let chain = Chain::new(&target, &[change.clone()]).unwrap();
        let other_repo = Target::new(
            Repository::new("other"),
            Some(commit.clone()),
            change.clone(),
        )
        .unwrap();
        assert!(chain.fork_for(&other_repo).is_none());
        let other_revision = Target::new(
            repository.clone(),
            Some(Commit {
                id: "b".repeat(40),
                ..commit
            }),
            change.clone(),
        )
        .unwrap();
        assert!(chain.fork_for(&other_revision).is_none());
        let local = Change {
            area: Area::Unstaged,
            ..change
        };
        let target = Target::new(repository, None, local.clone()).unwrap();
        let local_chain = Chain::new(&target, &[local]).unwrap();
        assert!(local_chain.fork_for(&target).is_none());
    }
}
