//! Bounded history loading and reconstruction of an older window's frontier.
use crate::{
    git::{Cancel, HISTORY_PAGE, Repository, Snapshot},
    graph::{self, Layout, Row},
};

pub struct Window {
    pub snapshot: Snapshot,
    pub rows: Vec<Row>,
    pub start: Layout,
    pub end: Layout,
}

/// Same fixed roots and filter precedence as read_filtered, without metadata or geometry.
/// Hash ID/message unions deliberately override other filters, as in the Log.
pub fn count(
    repo: &Repository,
    roots: &[String],
    filter: Option<&crate::git::LogFilter>,
    token: &Cancel,
) -> Result<usize, String> {
    if let Some(hashes) = filter.and_then(crate::git::LogFilter::hashes) {
        let topology = read_topology(repo, roots, token)?;
        let matched = hash_matches(repo, roots, &topology, &hashes, token)?;
        if !matched.is_empty() {
            return Ok(matched.len());
        }
    }
    repo.count_history(roots, filter, token)
}

pub fn read(
    repo: &Repository,
    branch: Option<&str>,
    offset: usize,
    limit: usize,
    fixed: Option<&[String]>,
    start: Option<Layout>,
    token: &Cancel,
) -> Result<Window, String> {
    // The first window needs no frontier reconstruction. Its own snapshot
    // establishes roots, so a one-row bootstrap would duplicate all metadata
    // reads and run history twice on every open / first-window refresh.
    if offset == 0 {
        let snapshot = repo.snapshot_at(branch, 0, limit, fixed, token)?;
        let start = start.unwrap_or_default();
        let (rows, end) = graph::layout_window(&snapshot.commits, start.clone(), 500_000)?;
        return Ok(Window {
            snapshot,
            rows,
            start,
            end,
        });
    }
    // On refresh, establish fresh roots once and reconstruct only the lane
    // frontier. Old commits and row geometry are discarded after each chunk.
    let roots = match fixed {
        Some(roots) => roots.to_vec(),
        None => repo.snapshot(branch, 1, None, token)?.tips,
    };
    let start = match start {
        Some(start) => start,
        None => {
            let mut start = Layout::default();
            let mut skipped = 0;
            while skipped < offset {
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                let count = HISTORY_PAGE.min(offset - skipped);
                let page = repo.snapshot_at(branch, skipped, count, Some(&roots), token)?;
                if page.commits.is_empty() {
                    break;
                }
                skipped += page.commits.len();
                start = graph::layout_window(&page.commits, start, 500_000)?.1;
            }
            start
        }
    };
    let snapshot = repo.snapshot_at(branch, offset, limit, Some(&roots), token)?;
    let (rows, end) = graph::layout_window(&snapshot.commits, start.clone(), 500_000)?;
    Ok(Window {
        snapshot,
        rows,
        start,
        end,
    })
}

/// Full-history text filtering with compact temporary topology. The temporary
/// complete-graph budget is explicit until streaming projection is available.
pub fn read_filtered(
    repo: &Repository,
    branch: Option<&str>,
    offset: usize,
    limit: usize,
    fixed: Option<&[String]>,
    filter: &crate::git::LogFilter,
    token: &Cancel,
) -> Result<Window, String> {
    if let Some(hashes) = filter.hashes() {
        if let Some(window) = read_hashes(repo, &hashes, offset, limit, fixed, token)? {
            return Ok(window);
        }
    }
    filter.validate_paths()?;
    let path_filter = !filter.paths.is_empty();
    // Path simplification may change traversal order and parent IDs. Select IDs
    // through Git's structure query, then use the complete graph for display.
    let snapshot = if path_filter {
        repo.snapshot_at(branch, 0, 1, fixed, token)?
    } else {
        repo.snapshot_filtered_at(branch, offset, limit, fixed, Some(filter), token)?
    };
    if snapshot.commits.is_empty() {
        return Ok(Window {
            snapshot,
            rows: Vec::new(),
            start: Layout::default(),
            end: Layout::default(),
        });
    }
    let topology = read_topology(repo, &snapshot.tips, token)?;
    let matched = read_matches(repo, &snapshot.tips, filter, token)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    let projected =
        crate::filtered_graph::window(&topology, &matched, offset, limit, 500_000, 500_000)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    if path_filter {
        let ids = projected
            .indices
            .iter()
            .map(|&i| topology[i].id.clone())
            .collect::<Vec<_>>();
        let mut snapshot = repo.snapshot_commits(&ids, &snapshot.tips, token)?;
        snapshot.more = projected.more;
        return Ok(Window {
            snapshot,
            rows: projected.rows,
            start: Layout::default(),
            end: Layout::default(),
        });
    }
    if projected.indices.len() != snapshot.commits.len()
        || projected
            .indices
            .iter()
            .zip(&snapshot.commits)
            .any(|(&index, commit)| topology[index].id != commit.id)
    {
        return Err("History changed while filtering; refresh to reload".into());
    }
    Ok(Window {
        snapshot,
        rows: projected.rows,
        start: Layout::default(),
        end: Layout::default(),
    })
}

/// Hash matches take priority over other filters, as in Rebased applyHashFilter.
fn read_hashes(
    repo: &Repository,
    hashes: &[String],
    offset: usize,
    limit: usize,
    fixed: Option<&[String]>,
    token: &Cancel,
) -> Result<Option<Window>, String> {
    let initial = repo.snapshot_at(None, 0, 1, fixed, token)?;
    let topology = read_topology(repo, &initial.tips, token)?;
    let matched = hash_matches(repo, &initial.tips, &topology, hashes, token)?;
    if matched.is_empty() {
        return Ok(None);
    }
    let projected =
        crate::filtered_graph::window(&topology, &matched, offset, limit, 500_000, 500_000)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    let ids: Vec<_> = projected
        .indices
        .iter()
        .map(|&i| topology[i].id.clone())
        .collect();
    let mut snapshot = repo.snapshot_commits(&ids, &initial.tips, token)?;
    snapshot.more = projected.more;
    Ok(Some(Window {
        snapshot,
        rows: projected.rows,
        start: Layout::default(),
        end: Layout::default(),
    }))
}

/// A history window containing a navigation target and its selected row.
/// `locate` handles unfiltered history; `locate_filtered` preserves active filters.
pub struct Located {
    pub window: Window,
    pub offset: usize,
    pub selected: usize,
}

pub fn locate(
    repo: &Repository,
    roots: &[String],
    target: &str,
    limit: usize,
    token: &Cancel,
) -> Result<Option<Located>, String> {
    let Some(position) = repo.commit_position(roots, target, token)? else {
        return Ok(None);
    };
    let limit = limit.clamp(1, crate::git::HISTORY_MAX);
    let offset = position / limit * limit;
    let window = read(repo, None, offset, limit, Some(roots), None, token)?;
    let selected = position - offset;
    if !window
        .snapshot
        .commits
        .get(selected)
        .is_some_and(|commit| commit.id.eq_ignore_ascii_case(target))
    {
        return Err("History changed while locating the commit; refresh and try again".into());
    }
    Ok(Some(Located {
        window,
        offset,
        selected,
    }))
}

/// The distinction lets the UI offer an explicit filter reset only for hidden
/// commits, without confusing a different page with a different filter scope.
pub enum FilteredLocation {
    Found(Located),
    Hidden,
    OutsideRoots,
}

pub fn locate_filtered(
    repo: &Repository,
    roots: &[String],
    target: &str,
    limit: usize,
    filter: &crate::git::LogFilter,
    token: &Cancel,
) -> Result<FilteredLocation, String> {
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    if !crate::git::is_oid(target) || roots.iter().any(|id| !crate::git::is_oid(id)) {
        return Err("Commit navigation requires full commit IDs".into());
    }
    let topology = read_topology(repo, roots, token)?;
    let mut matched = match filter.hashes() {
        Some(hashes) => hash_matches(repo, roots, &topology, &hashes, token)?,
        None => std::collections::HashSet::new(),
    };
    // ID and message-reference matches override other filters, including paths.
    // When neither matches, the normal text query is the fallback.
    if matched.is_empty() {
        matched = read_matches(repo, roots, filter, token)?;
    }
    let Some(target) = topology
        .iter()
        .find(|commit| commit.id.eq_ignore_ascii_case(target))
    else {
        return Ok(FilteredLocation::OutsideRoots);
    };
    if !matched.contains(&target.id) {
        return Ok(FilteredLocation::Hidden);
    }
    let position = topology
        .iter()
        .filter(|commit| matched.contains(&commit.id))
        .position(|commit| commit.id == target.id)
        .ok_or("Navigation target disappeared")?;
    let limit = limit.clamp(1, crate::git::HISTORY_MAX);
    let offset = position / limit * limit;
    let projected =
        crate::filtered_graph::window(&topology, &matched, offset, limit, 500_000, 500_000)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    let ids = projected
        .indices
        .iter()
        .map(|&i| topology[i].id.clone())
        .collect::<Vec<_>>();
    let mut snapshot = repo.snapshot_commits(&ids, roots, token)?;
    snapshot.more = projected.more;
    let selected = position - offset;
    if !snapshot
        .commits
        .get(selected)
        .is_some_and(|commit| commit.id == target.id)
    {
        return Err("History changed while locating the commit; refresh and try again".into());
    }
    Ok(FilteredLocation::Found(Located {
        window: Window {
            snapshot,
            rows: projected.rows,
            start: Layout::default(),
            end: Layout::default(),
        },
        offset,
        selected,
    }))
}

fn read_topology(
    repo: &Repository,
    roots: &[String],
    token: &Cancel,
) -> Result<Vec<crate::git::TopologyCommit>, String> {
    repo.bounded_topology(roots, None, token)
}

/// Read the same bounded, filtered graph used by Log navigation. Only target
/// identities are returned; messages and file contents remain unloaded.
pub fn adjacent(
    repo: &Repository,
    roots: &[String],
    selected: &str,
    parent: bool,
    filter: Option<&crate::git::LogFilter>,
    token: &Cancel,
) -> Result<Vec<String>, String> {
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    if !crate::git::is_oid(selected) || roots.iter().any(|id| !crate::git::is_oid(id)) {
        return Err("Commit navigation requires full commit IDs".into());
    }
    let topology = read_topology(repo, roots, token)?;
    let matched = if let Some(filter) = filter {
        filter.validate_paths()?;
        let mut matched = match filter.hashes() {
            Some(hashes) => hash_matches(repo, roots, &topology, &hashes, token)?,
            None => std::collections::HashSet::new(),
        };
        if matched.is_empty() {
            matched = read_matches(repo, roots, filter, token)?;
        }
        matched
    } else {
        topology.iter().map(|c| c.id.clone()).collect()
    };
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    let result = crate::filtered_graph::adjacent(&topology, &matched, selected, parent, 500_000)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(result)
}

fn read_matches(
    repo: &Repository,
    roots: &[String],
    filter: &crate::git::LogFilter,
    token: &Cancel,
) -> Result<std::collections::HashSet<String>, String> {
    Ok(repo
        .bounded_topology(roots, Some(filter), token)?
        .into_iter()
        .map(|commit| commit.id)
        .collect())
}

fn hash_matches(
    repo: &Repository,
    roots: &[String],
    topology: &[crate::git::TopologyCommit],
    hashes: &[String],
    token: &Cancel,
) -> Result<std::collections::HashSet<String>, String> {
    let mut matched = std::collections::HashSet::new();
    for commit in topology {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if hashes.iter().any(|h| commit.id.starts_with(h)) {
            matched.insert(commit.id.clone());
        }
    }
    // Rebased's default hash search includes message references, independently
    // of the user's other filters and text toggles. Tokens are validated hex,
    // so an ERE union is equivalent to its case-insensitive literal patterns.
    let messages = crate::git::LogFilter {
        text: hashes.join("|"),
        regex: true,
        ..Default::default()
    };
    matched.extend(read_matches(repo, roots, &messages, token)?);
    Ok(matched)
}
