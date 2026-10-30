//! Multi-repository history identity and ordering.
//! Rules from Rebased 4aec352: CommitId and VcsLogMultiRepoJoiner (Apache-2.0).
//! Each source is already in Git's requested order. Only its next row competes
//! with other repositories; globally sorting timestamps would break topology.
pub mod navigation;
pub mod selection;

use crate::git::Commit;
use std::{collections::BinaryHeap, path::PathBuf, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CommitId {
    pub root: Arc<PathBuf>,
    pub hash: String,
}

pub struct RepositoryLog {
    pub root: Arc<PathBuf>,
    pub commits: Vec<Commit>,
    pub more: bool,
}

/// Presentation facts captured from this row's repository, never a project-wide hash lookup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeadState {
    pub included: bool,
    pub tip: bool,
}
impl HeadState {
    pub fn for_commit(
        id: &str,
        head: Option<&str>,
        included: &std::collections::HashSet<String>,
    ) -> Self {
        Self {
            included: included.contains(id),
            tip: head == Some(id),
        }
    }
}

pub struct Entry {
    pub head: HeadState,
    /// Projected display parents only; real parents stay in commit.
    pub display_parents: Option<Vec<(String, bool)>>,
    pub root: Arc<PathBuf>,
    pub commit: Commit,
}
impl Entry {
    pub fn identity(&self) -> CommitId {
        CommitId {
            root: self.root.clone(),
            hash: self.commit.id.clone(),
        }
    }
    pub fn parent_identity(&self, hash: &str) -> CommitId {
        CommitId {
            root: self.root.clone(),
            hash: hash.to_owned(),
        }
    }
}

pub struct Page {
    pub entries: Vec<Entry>,
    pub more: bool,
}
/// Graph frontier across bounded metadata pages. Root numbering is stable for
/// the query lifetime; neither commits nor past row geometry are retained.
#[derive(Clone, Default)]
pub struct Layout {
    roots: std::collections::HashMap<Arc<PathBuf>, usize>,
    graph: crate::graph::Layout,
}

impl Page {
    /// Rebased enumerates (root, hash) before building the shared graph. These
    /// private lane keys serve that purpose only; never pass them to Git or use
    /// them as selection identities. Actual commit metadata is unchanged.
    pub fn layout(&self, token: &crate::git::Cancel) -> Result<Vec<crate::graph::Row>, String> {
        self.layout_window(Layout::default(), token)
            .map(|(rows, _)| rows)
    }

    pub fn layout_window(
        &self,
        mut start: Layout,
        token: &crate::git::Cancel,
    ) -> Result<(Vec<crate::graph::Row>, Layout), String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if self.entries.len() > crate::git::HISTORY_MAX {
            return Err("Project graph exceeds history budget".into());
        }
        let mut geometry = crate::graph::GeometryPool::default();
        let mut rows = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let next = start.roots.len();
            let root = *start.roots.entry(entry.root.clone()).or_insert(next);
            let id = format!("{root}:{}", entry.commit.id);
            if entry
                .display_parents
                .as_ref()
                .map_or(entry.commit.parents.len(), Vec::len)
                > 512
            {
                return Err("Project graph exceeds the 512-lane complexity budget".into());
            }
            let parents = match &entry.display_parents {
                Some(parents) => parents
                    .iter()
                    .map(|(hash, dotted)| (format!("{root}:{hash}"), *dotted))
                    .collect::<Vec<_>>(),
                None => entry
                    .commit
                    .parents
                    .iter()
                    .map(|hash| (format!("{root}:{hash}"), false))
                    .collect(),
            };
            let mut row = start.graph.append_projected_bounded(&id, &parents)?;
            if !geometry.intern(&mut row, 500_000) {
                return Err("Project graph exceeds window geometry budget".into());
            }
            rows.push(row);
        }
        Ok((rows, start))
    }
}

/// Merge loaded prefixes with one overall row budget. Roots must be unique,
/// canonical repository paths supplied by discovery. Equal timestamps use input
/// root order for stable reloads; upstream's hash-map tie order is unspecified.
/// This does not concatenate/deduplicate histories by hash or follow gitlinks as
/// graph edges. The caller retains repository identity for every subsequent job.
pub fn merge(logs: Vec<RepositoryLog>, limit: usize) -> Page {
    let more = logs.iter().any(|log| log.more);
    let streams = logs
        .into_iter()
        .map(|log| {
            log.commits
                .into_iter()
                .map(|commit| Entry {
                    head: HeadState::default(),
                    root: log.root.clone(),
                    display_parents: None,
                    commit,
                })
                .collect::<Vec<_>>()
                .into_iter()
        })
        .collect();
    merge_streams(streams, more, limit)
}

fn merge_streams(mut streams: Vec<std::vec::IntoIter<Entry>>, more: bool, limit: usize) -> Page {
    let available = streams.iter().map(|s| s.len()).sum::<usize>();
    let mut heads = Vec::with_capacity(streams.len());
    let mut queue = BinaryHeap::new();
    for (index, rows) in streams.iter_mut().enumerate() {
        let head = rows.next();
        if let Some(row) = &head {
            queue.push((row.commit.committer_timestamp, std::cmp::Reverse(index)));
        }
        heads.push(head);
    }
    let mut entries = Vec::with_capacity(limit.min(available));
    while entries.len() < limit {
        let Some((_, std::cmp::Reverse(index))) = queue.pop() else {
            break;
        };
        entries.push(heads[index].take().expect("queued repository has a head"));
        heads[index] = streams[index].next();
        if let Some(row) = &heads[index] {
            queue.push((row.commit.committer_timestamp, std::cmp::Reverse(index)));
        }
    }
    Page {
        entries,
        more: more || !queue.is_empty(),
    }
}

/// Already resolved repository-local filters. The UI's Roots filter determines
/// which queries are supplied; paths and author identities belong to each root.
#[derive(Clone)]
pub struct Query {
    pub repository: crate::git::Repository,
    pub branches: Vec<String>,
    pub filter: Option<crate::git::LogFilter>,
}

#[derive(Clone)]
pub struct LoadedRoot {
    pub query: Query,
    /// Branch identity captured with this root's displayed references.
    pub current_branch: Option<String>,
    /// Repository-local reference labels; hashes are not project-wide identities.
    pub ref_titles: Arc<std::collections::HashMap<String, String>>,
    pub tips: Vec<String>,
}

impl LoadedRoot {
    /// Navigate this exact displayed query, retaining its repository and fixed
    /// tips even if refs change while the user is reading the Log.
    pub fn adjacent(
        &self,
        selected: &CommitId,
        parent: bool,
        token: &crate::git::Cancel,
    ) -> Result<Vec<CommitId>, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if self.query.repository.path != *selected.root {
            return Err("Commit does not belong to this history repository".into());
        }
        crate::history::adjacent(
            &self.query.repository,
            &self.tips,
            &selected.hash,
            parent,
            self.query.filter.as_ref(),
            token,
        )
        .map(|hashes| {
            hashes
                .into_iter()
                .map(|hash| CommitId {
                    root: selected.root.clone(),
                    hash,
                })
                .collect()
        })
    }
}

#[derive(Clone, Default)]
pub struct Scope {
    pub roots: Vec<LoadedRoot>,
    pub errors: Vec<(PathBuf, String)>,
}
impl Scope {
    /// Resolve navigation through the same source snapshot that produced the
    /// displayed rows. A missing/failed repository must never fall back to another
    /// root, including when both contain the requested hash.
    pub fn adjacent(
        &self,
        selected: &CommitId,
        parent: bool,
        token: &crate::git::Cancel,
    ) -> Result<Vec<CommitId>, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let root = self
            .roots
            .iter()
            .find(|root| root.query.repository.path == *selected.root)
            .ok_or("Commit repository is not in the loaded history")?;
        root.adjacent(selected, parent, token)
    }

    /// Separate from the first-page query so callers can count asynchronously.
    /// A failed root makes the total unknown, never an apparently exact subtotal.
    pub fn count(&self, token: &crate::git::Cancel) -> Result<usize, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if !self.errors.is_empty() {
            return Err(
                "History total is unavailable because a repository could not be read".into(),
            );
        }
        let mut total = 0usize;
        for root in &self.roots {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            total = total
                .checked_add(crate::history::count(
                    &root.query.repository,
                    &root.tips,
                    root.query.filter.as_ref(),
                    token,
                )?)
                .ok_or("History count exceeds supported size")?;
        }
        Ok(total)
    }
}

pub struct Read {
    pub page: Page,
    pub scope: Arc<Scope>,
    // Rows consumed through this page, independent of transferred UI metadata.
    offsets: std::collections::HashMap<PathBuf, usize>,
}
impl Read {
    fn cursor(&self) -> Self {
        Self {
            page: Page {
                entries: vec![],
                more: self.page.more,
            },
            scope: self.scope.clone(),
            offsets: self.offsets.clone(),
        }
    }
    /// Continue the fixed query without retaining prior commit metadata. Each
    /// root advances only by rows actually emitted, not its prefetched prefix.
    /// Failure is atomic: callers keep this page and may retry the same cursor.
    pub fn next_page(&self, limit: usize, token: &crate::git::Cancel) -> Result<Self, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let limit = limit.clamp(1, crate::git::HISTORY_MAX);
        let mut result = Self {
            page: Page {
                entries: vec![],
                more: false,
            },
            scope: self.scope.clone(),
            offsets: self.offsets.clone(),
        };
        for root in &self.scope.roots {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let offset = result
                .offsets
                .get(&root.query.repository.path)
                .copied()
                .unwrap_or(0);
            let window = match &root.query.filter {
                Some(filter) => crate::history::read_branches_filtered(
                    &root.query.repository,
                    &root.query.branches,
                    offset,
                    limit,
                    Some(&root.tips),
                    filter,
                    token,
                ),
                None => crate::history::read_branches(
                    &root.query.repository,
                    &root.query.branches,
                    offset,
                    limit,
                    Some(&root.tips),
                    Some(crate::graph::Layout::default()),
                    token,
                ),
            }
            .map_err(|error| format!("{}: {error}", root.query.repository.path.display()))?;
            merge_window(
                &mut result.page,
                Arc::new(root.query.repository.path.clone()),
                window,
                limit,
            )?;
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        result.advance_offsets()?;
        Ok(result)
    }

    fn advance_offsets(&mut self) -> Result<(), String> {
        for entry in &self.page.entries {
            let offset = self.offsets.entry(entry.root.as_ref().clone()).or_default();
            *offset = offset
                .checked_add(1)
                .ok_or("History offset exceeds supported size")?;
        }
        Ok(())
    }
}

/// Read the first N matching rows across the selected roots. Reuses existing
/// hash/date/author/path filtering, including hash precedence and true parents.
/// At most two N-row metadata prefixes coexist during merging, not N per root.
/// No total-count traversal is performed here. Refresh/new limits re-query tips.
pub fn read(queries: &[Query], limit: usize, token: &crate::git::Cancel) -> Result<Read, String> {
    read_impl(queries, limit, None, token).map(|(read, _)| read)
}

/// Read a project page and its primary repository's reference metadata in one
/// request. The primary may be excluded by Log filters or have unreadable
/// history; its reference result is independent of those per-root errors.
/// The returned snapshot has no commit/membership payload and is not a page.
pub fn read_with_primary(
    queries: &[Query],
    limit: usize,
    primary: &crate::git::Repository,
    token: &crate::git::Cancel,
) -> Result<(Read, Result<crate::git::Snapshot, String>), String> {
    let (read, captured) = read_impl(queries, limit, Some(primary), token)?;
    let references = match captured {
        Some(snapshot) => Ok(snapshot),
        None => primary.reference_snapshot(token),
    };
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok((read, references))
}

fn read_impl(
    queries: &[Query],
    limit: usize,
    primary: Option<&crate::git::Repository>,
    token: &crate::git::Cancel,
) -> Result<(Read, Option<crate::git::Snapshot>), String> {
    if queries.len() > 4096 {
        return Err("Too many history repositories".into());
    }
    let limit = limit.clamp(1, crate::git::HISTORY_MAX);
    let mut result = Read {
        page: Page {
            entries: vec![],
            more: false,
        },
        scope: Arc::default(),
        offsets: Default::default(),
    };
    let mut seen = std::collections::HashSet::new();
    let mut captured = None;
    for query in queries {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let root = match crate::submodules::worktree_root(&query.repository, token) {
            Ok(root) => root,
            Err(error) => {
                Arc::make_mut(&mut result.scope)
                    .errors
                    .push((query.repository.path.clone(), error));
                continue;
            }
        };
        if !seen.insert(root.clone()) {
            return Err("Duplicate history repository".into());
        }
        let repository = crate::git::Repository::new(&root);
        let filter = query.filter.as_ref().filter(|filter| filter.is_active());
        let window = match filter {
            Some(filter) => crate::history::read_branches_filtered(
                &repository,
                &query.branches,
                0,
                limit,
                None,
                filter,
                token,
            ),
            None => crate::history::read_branches(
                &repository,
                &query.branches,
                0,
                limit,
                None,
                None,
                token,
            ),
        };
        match window {
            Ok(mut window) => {
                if primary.is_some_and(|primary| {
                    primary.path == root || primary.path == query.repository.path
                }) {
                    // Move only the requested root's metadata before merging consumes
                    // the window. Do not clone commits or retain metadata per page/root.
                    let snapshot = &mut window.snapshot;
                    captured = Some(crate::git::Snapshot {
                        head_id: snapshot.head_id.clone(),
                        included_in_head: Arc::default(),
                        current_branch: snapshot.current_branch.clone(),
                        ref_titles: snapshot.ref_titles.clone(),
                        tracked_branches: std::mem::take(&mut snapshot.tracked_branches),
                        commits: vec![],
                        branches: std::mem::take(&mut snapshot.branches),
                        tags: std::mem::take(&mut snapshot.tags),
                        tips: snapshot.tips.clone(),
                        more: false,
                        shallow: snapshot.shallow,
                    });
                }
                let current_branch = window.snapshot.current_branch.clone();
                let ref_titles = window.snapshot.ref_titles.clone();
                let tips = merge_window(&mut result.page, Arc::new(root), window, limit)?;
                Arc::make_mut(&mut result.scope).roots.push(LoadedRoot {
                    query: Query {
                        repository,
                        filter: filter.cloned(),
                        ..query.clone()
                    },
                    tips,
                    current_branch,
                    ref_titles,
                });
            }
            Err(error) => Arc::make_mut(&mut result.scope).errors.push((root, error)),
        }
    }
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    result.advance_offsets()?;
    Ok((result, captured))
}

/// Append one bounded source prefix; metadata never accumulates N rows per root.
fn merge_window(
    page: &mut Page,
    root: Arc<PathBuf>,
    window: crate::history::Window,
    limit: usize,
) -> Result<Vec<String>, String> {
    let snapshot = window.snapshot;
    if window
        .projected_parents
        .as_ref()
        .is_some_and(|parents| parents.len() != snapshot.commits.len())
    {
        return Err("History graph and metadata do not match".into());
    }
    let mut projected = window.projected_parents.map(Vec::into_iter);
    let entries = snapshot
        .commits
        .into_iter()
        .map(|commit| Entry {
            head: HeadState::for_commit(
                &commit.id,
                snapshot.head_id.as_deref(),
                &snapshot.included_in_head,
            ),
            root: root.clone(),
            display_parents: projected.as_mut().and_then(Iterator::next),
            commit,
        })
        .collect::<Vec<_>>();
    *page = merge_streams(
        vec![
            std::mem::take(&mut page.entries).into_iter(),
            entries.into_iter(),
        ],
        page.more || snapshot.more,
        limit,
    );
    Ok(snapshot.tips)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn log(root: &str, rows: &[(&str, i64)]) -> RepositoryLog {
        RepositoryLog {
            root: Arc::new(root.into()),
            more: false,
            commits: rows
                .iter()
                .map(|(hash, timestamp)| Commit {
                    id: (*hash).into(),
                    parents: vec![],
                    shallow_boundary: false,
                    committer_timestamp: *timestamp,
                    author: String::new(),
                    date: String::new(),
                    subject: String::new(),
                    refs: vec![],
                })
                .collect(),
        }
    }
    #[test]
    fn paged_graph_retains_root_numbers_and_lane_frontier_without_old_metadata() {
        let mut first = log("parent", &[("tip", 8), ("base", 2)]);
        let mut second = log("child", &[("tip", 6), ("base", 1)]);
        first.commits[0].parents.push("base".into());
        second.commits[0].parents.push("base".into());
        let mut page = merge(vec![first, second], 10);
        let token = crate::git::Cancel::default();
        let whole = page.layout(&token).unwrap();
        let tail = Page {
            entries: page.entries.split_off(1),
            more: false,
        };
        let (mut rows, frontier) = page.layout_window(Layout::default(), &token).unwrap();
        drop(page);
        let (rest, _) = tail.layout_window(frontier, &token).unwrap();
        rows.extend(rest);
        assert_eq!(rows.len(), whole.len());
        for (paged, expected) in rows.iter().zip(whole.iter()) {
            assert_eq!(paged.node, expected.node);
            assert_eq!(paged.color, expected.color);
            assert_eq!(paged.width, expected.width);
            assert_eq!(paged.segments, expected.segments);
        }
    }

    #[test]
    fn graph_and_selection_keep_identical_histories_separate() {
        let mut a = log("parent", &[("tip", 4), ("base", 2)]);
        let mut b = log("child", &[("tip", 3), ("base", 1)]);
        a.commits[0].parents.push("base".into());
        b.commits[0].parents.push("base".into());
        let page = merge(vec![a, b], 10);
        let token = crate::git::Cancel::default();
        let rows = page.layout(&token).unwrap();
        assert_eq!(rows[0].color, rows[2].color);
        assert_eq!(rows[1].color, rows[3].color);
        assert_ne!(rows[0].color, rows[1].color);
        crate::graph_print::Model::new(&rows, &token)
            .unwrap()
            .project(false, &token)
            .unwrap();
        let ids = page.entries.iter().map(Entry::identity).collect::<Vec<_>>();
        let mut selection = crate::log_selection::IdentitySelection::default();
        selection.select(&ids, 0, false, false);
        assert!(!selection.contains(&ids[1]));
        selection.select(&ids, 1, true, false);
        assert_eq!(selection.ordered(&ids).len(), 2);
        selection.reconcile(&ids[1..]);
        assert_eq!(selection.ordered(&ids), vec![&ids[1]]);
        assert_eq!(selection.lead(&ids), Some(1));
        token.cancel();
        assert!(page.layout(&token).is_err());
    }

    #[test]
    fn joins_repository_heads_without_reordering_clock_skewed_ancestors() {
        let page = merge(
            vec![
                log("parent", &[("a", 6), ("b", 10)]),
                log("child", &[("c", 7), ("d", 5)]),
            ],
            10,
        );
        assert_eq!(
            page.entries
                .iter()
                .map(|r| r.commit.id.as_str())
                .collect::<Vec<_>>(),
            ["c", "a", "b", "d"]
        );
        assert!(!page.more);
    }
    #[test]
    fn identical_hashes_and_parents_in_distinct_roots_remain_distinct() {
        let page = merge(
            vec![log("parent", &[("same", 5)]), log("child", &[("same", 5)])],
            10,
        );
        assert_eq!(page.entries.len(), 2);
        assert_ne!(page.entries[0].identity(), page.entries[1].identity());
        assert_ne!(
            page.entries[0].parent_identity("base"),
            page.entries[1].parent_identity("base")
        );
        assert_eq!(page.entries[0].root.as_ref(), &PathBuf::from("parent"));
    }
    #[test]
    fn budget_and_repository_truncation_both_keep_more_available() {
        let page = merge(
            vec![log("parent", &[("a", 2)]), log("child", &[("b", 1)])],
            1,
        );
        assert_eq!(page.entries.len(), 1);
        assert!(page.more);
        let mut root = log("parent", &[("a", 2)]);
        root.more = true;
        assert!(merge(vec![root], 10).more);
        assert!(merge(vec![log("parent", &[("a", 2)])], 0).more);
        assert!(!merge(vec![], 0).more);
    }
}
