//! Non-staging Commit view metadata, translated from Rebased GitChangesCollector
//! (4aec352). Review areas remain separate; this model never changes the index.
use crate::git::{Area, Cancel, Change, Repository, WorkingTreeChange};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

#[derive(Clone, Debug)]
pub enum File {
    Tracked(WorkingTreeChange),
    Unversioned(Change),
    Conflict(Change),
}
impl File {
    pub fn change(&self) -> Change {
        match self {
            Self::Tracked(c) => Change {
                path: c.path.clone(),
                old_path: c.old_path.clone(),
                status: c.status.clone(),
                area: Area::WorkingTree,
            },
            Self::Unversioned(c) | Self::Conflict(c) => c.clone(),
        }
    }
    pub fn from_change(c: &Change) -> Option<Self> {
        match c.area {
            Area::WorkingTree => Some(Self::Tracked(WorkingTreeChange {
                path: c.path.clone(),
                old_path: c.old_path.clone(),
                status: c.status.clone(),
            })),
            Area::Untracked => Some(Self::Unversioned(c.clone())),
            Area::Conflict => Some(Self::Conflict(c.clone())),
            _ => None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub head: Option<String>,
    pub files: Vec<File>,
    /// Original index/worktree metadata for diagnostics and refresh reconciliation.
    /// Do not expand a displayed deletion to its index rename destination: Rebased
    /// preserves that unselected staged destination through the staging-area saver.
    pub review_versions: Vec<Change>,
}
impl Snapshot {
    pub fn load(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        let mut trace = crate::performance_trace::Span::new("request", "changes", Some(&repo.path));
        trace.outcome("error");
        let head = crate::commit_operation::head(repo, token)?;
        let review_versions = repo.local_changes(token)?;
        let staged: HashSet<_> = review_versions
            .iter()
            .filter(|c| c.area == Area::Staged && c.status == "M")
            .map(|c| c.path.as_path())
            .collect();
        let both_modified: Vec<_> = review_versions
            .iter()
            .filter(|c| {
                c.area == Area::Unstaged && c.status == "M" && staged.contains(c.path.as_path())
            })
            .map(|c| c.path.clone())
            .collect();
        let net = if both_modified.is_empty() {
            Vec::new()
        } else {
            repo.working_tree_changes_for(
                head.as_deref()
                    .ok_or("HEAD changed; refresh local changes")?,
                &both_modified,
                token,
            )?
        };
        if crate::commit_operation::head(repo, token)? != head {
            return Err("HEAD changed; refresh local changes".into());
        }
        let files = collect(&review_versions, &net)?;
        trace.outcome("ok");
        Ok(Self {
            head,
            files,
            review_versions,
        })
    }
}
/// Preserve status-based rename/deletion semantics. Only MM needs the net diff
/// to suppress an index edit reverted in the working tree, as in Rebased.
fn collect(versions: &[Change], net: &[WorkingTreeChange]) -> Result<Vec<File>, String> {
    let net_paths: HashSet<_> = net.iter().map(|c| &c.path).collect();
    let mut paths = BTreeMap::<PathBuf, Vec<&Change>>::new();
    for c in versions {
        paths.entry(c.path.clone()).or_default().push(c);
    }
    let mut tracked = vec![];
    let mut unversioned = vec![];
    let mut conflicts = vec![];
    for (path, entries) in paths {
        if let Some(c) = entries.iter().find(|c| c.area == Area::Conflict) {
            conflicts.push(File::Conflict((*c).clone()));
            continue;
        }
        for c in entries.iter().filter(|c| c.area == Area::Untracked) {
            unversioned.push(File::Unversioned((*c).clone()));
        }
        let x = entries.iter().find(|c| c.area == Area::Staged);
        let y = entries.iter().find(|c| c.area == Area::Unstaged);
        if x.is_none() && y.is_none() {
            continue;
        }
        let status = |c: Option<&&Change>| c.and_then(|c| c.status.chars().next()).unwrap_or(' ');
        let (xs, ys) = (status(x), status(y));
        let old = x
            .and_then(|c| c.old_path.clone())
            .or_else(|| y.and_then(|c| c.old_path.clone()));
        let (kind, rename) = match (xs, ys) {
            (' ', 'M') | ('M', ' ' | 'T') | ('T', ' ' | 'M') => ('M', false),
            (' ', 'T') => ('T', false),
            (' ', 'D') | ('M' | 'T', 'D') | ('D', ' ' | 'M' | 'T') => ('D', false),
            (' ', 'A' | 'C') | ('A' | 'C', ' ' | 'M' | 'T') => ('A', false),
            ('A' | 'C', 'D') => continue,
            ('M', 'M') => {
                if !net_paths.contains(&path) {
                    continue;
                }
                ('M', false)
            }
            ('D', 'A' | 'C') => ('M', false),
            (' ', 'R') | ('D', 'R') | ('R', ' ' | 'M' | 'T') => ('R', true),
            ('R', 'D') => {
                tracked.push(File::Tracked(WorkingTreeChange {
                    path: old.ok_or("Rename has no source")?,
                    old_path: None,
                    status: "D".into(),
                }));
                continue;
            }
            ('T', 'T') => {
                conflicts.push(File::Conflict(Change {
                    path,
                    old_path: old,
                    status: "TT".into(),
                    area: Area::Conflict,
                }));
                continue;
            }
            _ => {
                return Err(format!(
                    "Unsupported local status {xs}{ys}; refresh local changes"
                ));
            }
        };
        let old_path = if rename {
            Some(old.ok_or("Rename has no source")?)
        } else {
            None
        };
        tracked.push(File::Tracked(WorkingTreeChange {
            path,
            old_path,
            status: kind.to_string(),
        }));
    }
    conflicts.extend(tracked);
    conflicts.extend(unversioned);
    Ok(conflicts)
}
