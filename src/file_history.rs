// Copyright 2000-2024 JetBrains s.r.o. and contributors.
// Apache-2.0 adaptation of GitFileHistory, FileHistoryModel/Util, VcsStatusMerger,
// MergedChangeDiffRequestProvider, Block and DiffUtil; see reference notes.
//! Bounded file history, rename traversal and selection-block tracking.
use crate::git::{Cancel, Repository, Result};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::{OsStr, OsString},
    ops::Range,
    path::{Component, Path, PathBuf},
};
pub const PAGE: usize = 100;
// Bound each Git response, not the amount of history the user may browse.
const QUERY_PAGE: usize = 1000;
pub const TEXT_LIMIT: usize = 5 * 1024 * 1024;
pub const BLOCK_BUDGET: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Revision {
    pub id: String,
    pub parents: Vec<String>,
    pub path: PathBuf,
    pub timestamp: i64,
    pub author: String,
    pub message: String,
    pub deleted: bool,
}
#[derive(Clone)]
pub struct Selection {
    pub text: String,
    pub lines: Range<usize>,
}
pub struct History {
    pub revisions: Vec<Revision>,
    pub more: bool,
    pub older: Option<Revision>,
}
/// Rebased FileHistoryModel.getSelectedChange: display-order first and last
/// selected rows are endpoints; a single row is compared with its parent.
pub fn comparison_rows(
    history: &History,
    selection: &crate::log_selection::Selection,
) -> Option<(usize, Option<usize>)> {
    let mut rows = history
        .revisions
        .iter()
        .enumerate()
        .filter(|(_, r)| selection.contains(&r.id))
        .map(|(i, _)| i);
    let first = rows.next()?;
    Some((first, rows.last()))
}
/// FileHistoryUtil.createChangeToParents: source endpoints retain their own
/// historical paths. A missing file is distinct from an existing empty blob.
/// Endpoints carry content identity only; commit metadata stays in History.
#[derive(Clone, Debug)]
pub struct Comparison {
    pub before: Option<Revision>,
    pub after: Option<Revision>,
    /// Source changes in graph-parent order, as MergedChange.getSourceChanges.
    /// Empty for an ordinary (single-parent or selected-range) comparison.
    pub source_before: Vec<Option<Revision>>,
}

/// MergedChangeDiffRequestProvider: merge result is the center of a three-way
/// request; an absent left/right/result reduces it to the remaining two sides.
pub fn preview_revisions(comparison: Comparison) -> Vec<Option<Revision>> {
    if comparison.source_before.len() != 2 {
        return vec![comparison.before, comparison.after];
    }
    let mut sides = comparison.source_before.into_iter();
    let left = sides.next().unwrap();
    let right = sides.next().unwrap();
    let center = comparison.after;
    if left.is_none() {
        vec![center, right]
    } else if right.is_none() {
        vec![left, center]
    } else if center.is_none() {
        vec![left, right]
    } else {
        vec![left, center, right]
    }
}

pub fn selected_comparison(
    history: &History,
    links: &[Vec<(String, bool)>],
    selection: &crate::log_selection::Selection,
) -> Result<Option<Comparison>> {
    let Some((row, last)) = comparison_rows(history, selection) else {
        return Ok(None);
    };
    let after = &history.revisions[row];
    if let Some(last) = last {
        return Ok(comparison_to_parents(after, &[&history.revisions[last]]));
    }
    if links.len() != history.revisions.len() {
        return Err("Invalid file history graph".into());
    }
    let parents = links[row]
        .iter()
        .map(|(id, _)| {
            history
                .revisions
                .iter()
                .chain(history.older.iter())
                .find(|r| &r.id == id)
                .ok_or_else(|| "Load more file history to compare this version".to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(comparison_to_parents(after, &parents))
}

// Comparison endpoints only need immutable content identity, not repeated
// copies of commit messages, authors or ancestry for every merge parent.
fn content_revision(revision: &Revision) -> Revision {
    Revision {
        id: revision.id.clone(),
        path: revision.path.clone(),
        deleted: false,
        parents: vec![],
        timestamp: 0,
        author: String::new(),
        message: String::new(),
    }
}
fn comparison_to_parents(selected: &Revision, parents: &[&Revision]) -> Option<Comparison> {
    let after = (!selected.deleted).then_some(selected);
    if parents.is_empty() {
        return after.map(|after| Comparison {
            before: None,
            after: Some(content_revision(after)),
            source_before: vec![],
        });
    }
    let changes: Vec<_> = parents
        .iter()
        .filter_map(|parent| {
            let before = (!parent.deleted).then_some(*parent);
            (before.is_some() || after.is_some()).then_some((before, after))
        })
        .collect();
    let &(first_before, first_after) = changes.first()?;
    let source_before = if changes.len() > 1 {
        changes.iter().map(|c| c.0.map(content_revision)).collect()
    } else {
        vec![]
    };
    let kind = |c: &(Option<&Revision>, Option<&Revision>)| match c {
        (None, Some(_)) => b'A',
        (Some(_), None) => b'D',
        (Some(a), Some(b)) if a.path != b.path => b'R',
        _ => b'M',
    };
    let first_kind = kind(&changes[0]);
    let same_kind = changes.iter().all(|c| kind(c) == first_kind);
    let same_source = changes
        .iter()
        .all(|c| c.0.map(|r| &r.path) == first_before.map(|r| &r.path));
    if changes.len() == 1 || (same_kind && (first_kind != b'R' || same_source)) {
        return Some(Comparison {
            before: first_before.map(content_revision),
            after: first_after.map(content_revision),
            source_before,
        });
    }
    // MyVcsChangesMerger constructs a new revision at the first graph parent
    // and selected path for mixed types or differing rename sources. Do not
    // replace it with whichever parent happens to contain a convenient blob.
    let mut before = content_revision(parents[0]);
    before.path = selected.path.clone();
    let deleted = changes.iter().any(|c| kind(c) == b'D');
    Some(Comparison {
        before: Some(before),
        after: if deleted {
            None
        } else {
            after.map(content_revision)
        },
        source_before,
    })
}

fn validate(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err("Invalid file history path".into());
    }
    Ok(())
}
fn read(repo: &Repository, args: Vec<OsString>, token: &Cancel) -> Result<Vec<u8>> {
    repo.read(
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        token,
    )
}
struct Record {
    revision: Revision,
    added: bool,
}
fn parse(bytes: &[u8], path: &Path) -> Result<Vec<Record>> {
    let fields: Vec<_> = bytes.split(|b| *b == 0).collect();
    let mut i = 0;
    let mut records = vec![];
    while i < fields.len() {
        while i < fields.len() && fields[i].is_empty() {
            i += 1;
        }
        if i == fields.len() {
            break;
        }
        let field = |n: usize| -> Result<&str> {
            std::str::from_utf8(fields.get(n).ok_or("Truncated file history")?)
                .map_err(|_| "Non-UTF-8 file history".into())
        };
        let id = field(i)?.trim_start_matches('\n').to_owned();
        if !crate::git::is_oid(&id) {
            return Err("Invalid file history revision".into());
        }
        let parents = field(i + 1)?
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if parents.iter().any(|p| !crate::git::is_oid(p)) {
            return Err("Invalid file history parent".into());
        }
        let timestamp = field(i + 2)?
            .parse()
            .map_err(|_| "Invalid file history date")?;
        let author = field(i + 3)?.to_owned();
        let message = field(i + 4)?.trim_end().to_owned();
        i += 5;
        let mut added = false;
        let mut deleted = false;
        let mut revision_path = path.to_owned();
        loop {
            while i < fields.len() && fields[i].is_empty() {
                i += 1;
            }
            if i == fields.len() {
                break;
            }
            let status = fields[i].strip_prefix(b"\n").unwrap_or(fields[i]);
            if std::str::from_utf8(status).is_ok_and(crate::git::is_oid) {
                break;
            }
            let kind = *status.first().ok_or("Missing history status")?;
            if !b"AMDTURC".contains(&kind) {
                return Err("Unsupported file history status".into());
            }
            i += 1;
            let first = crate::git::path_from_bytes(fields.get(i).ok_or("Missing history path")?)?;
            i += 1;
            let next = if matches!(kind, b'R' | b'C') {
                let p = crate::git::path_from_bytes(fields.get(i).ok_or("Missing rename path")?)?;
                i += 1;
                p
            } else {
                first
            };
            if next == path {
                added |= kind == b'A';
                deleted |= kind == b'D';
                revision_path = next;
            }
        }
        records.push(Record {
            revision: Revision {
                id,
                parents,
                path: revision_path,
                timestamp,
                author,
                message,
                deleted,
            },
            added,
        });
    }
    Ok(records)
}
pub fn load(repo: &Repository, path: &Path, limit: usize, token: &Cancel) -> Result<History> {
    validate(path)?;
    let head = resolve(repo, "HEAD", token)?;
    load_root(repo, path, limit, head, token)
}
fn resolve(repo: &Repository, name: &str, token: &Cancel) -> Result<String> {
    let bytes = read(
        repo,
        vec![
            "rev-parse".into(),
            "--verify".into(),
            "--end-of-options".into(),
            format!("{name}^{{commit}}").into(),
        ],
        token,
    )?;
    let id = std::str::from_utf8(&bytes)
        .map_err(|_| "Invalid revision")?
        .trim()
        .to_owned();
    if !crate::git::is_oid(&id) {
        return Err("Invalid revision".into());
    }
    Ok(id)
}
fn load_root(
    repo: &Repository,
    path: &Path,
    limit: usize,
    head: String,
    token: &Cancel,
) -> Result<History> {
    let limit = limit.max(1);
    let mut queue = VecDeque::from([(head, path.to_owned())]);
    let mut seen = HashSet::new();
    let mut revisions = vec![];
    let mut metadata_bytes = 0usize;
    while let Some((start, path)) = queue.pop_front() {
        let mut offset = 0usize;
        let page_size = limit.saturating_add(1).min(QUERY_PAGE);
        'path_pages: loop {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let args = [
                "-c",
                "diff.renames=false",
                "log",
                "--date-order",
                "--no-notes",
                "--no-show-signature",
                "--encoding=UTF-8",
                "-z",
                "--format=%H%x00%P%x00%at%x00%an%x00%B%x00",
                "--name-status",
                "-m",
            ]
            .map(OsString::from)
            .to_vec();
            let mut args = args;
            args.extend([
                format!("--max-count={page_size}").into(),
                format!("--skip={offset}").into(),
                start.clone().into(),
                "--".into(),
                path.as_os_str().to_owned(),
            ]);
            let records = parse(&read(repo, args, token)?, &path)?;
            if records.is_empty() {
                break;
            }
            offset = offset
                .checked_add(page_size)
                .ok_or("File history offset overflow")?;
            for record in records {
                if !seen.insert(record.revision.id.clone()) {
                    continue;
                }
                if revisions.len() == limit {
                    return Ok(History {
                        revisions,
                        more: true,
                        older: Some(record.revision),
                    });
                }
                let boundary = record.added;
                let revision = record.revision;
                metadata_bytes += revision.message.len()
                    + revision.author.len()
                    + revision.path.as_os_str().len()
                    + 256;
                if metadata_bytes > BLOCK_BUDGET {
                    return Err("File history exceeds metadata budget".into());
                }
                revisions.push(revision.clone());
                if boundary {
                    // As upstream: log to the newest addition, then inspect each
                    // parent with rename detection. Never use --follow for the whole log.
                    for parent in &revision.parents {
                        let bytes = read(
                            repo,
                            vec![
                                "diff-tree".into(),
                                "--no-commit-id".into(),
                                "--name-status".into(),
                                "-r".into(),
                                "-M".into(),
                                "-z".into(),
                                parent.into(),
                                revision.id.clone().into(),
                                "--".into(),
                            ],
                            token,
                        )?;
                        let f: Vec<_> =
                            bytes.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
                        let mut j = 0;
                        while j < f.len() {
                            let kind = f[j].first().copied().ok_or("Invalid rename status")?;
                            j += 1;
                            let old = crate::git::path_from_bytes(
                                f.get(j).ok_or("Missing rename source")?,
                            )?;
                            j += 1;
                            if matches!(kind, b'R' | b'C') {
                                let new = crate::git::path_from_bytes(
                                    f.get(j).ok_or("Missing rename target")?,
                                )?;
                                j += 1;
                                if kind == b'R' && new == path {
                                    queue.push_back((parent.clone(), old));
                                }
                            }
                        }
                    }
                    break 'path_pages;
                }
            }
        }
    }
    Ok(History {
        revisions,
        more: false,
        older: None,
    })
}
/// File-history branch choices are independent of the main Log filter.
#[derive(Clone, Debug)]
pub struct RefChoice {
    pub name: String,
    pub id: String,
}
pub struct ScopedHistory {
    /// Actual checkout HEAD, which need not be a revision that changed this file.
    pub head_id: String,
    pub history: History,
    pub refs: Vec<RefChoice>,
    pub graph: Vec<crate::graph::Row>,
    /// Projected links are separate from Revision.parents, which remain real parents.
    pub links: Vec<Vec<(String, bool)>>,
}
pub fn load_scoped(
    repo: &Repository,
    path: &Path,
    branch: Option<&str>,
    limit: usize,
    token: &Cancel,
) -> Result<ScopedHistory> {
    Scope::open(repo, path, branch, token)?.load(repo, path, limit, token)
}
/// Immutable query identity reused while collecting successive batches.
#[derive(Clone)]
pub struct Scope {
    head: String,
    refs: Vec<RefChoice>,
    roots: Vec<String>,
}
impl Scope {
    pub fn open(
        repo: &Repository,
        path: &Path,
        branch: Option<&str>,
        token: &Cancel,
    ) -> Result<Self> {
        validate(path)?;
        let head = resolve(repo, "HEAD", token)?;
        let bytes = read(
            repo,
            vec![
            "for-each-ref".into(),
            "--format=%(refname)%00%(objectname)%00%(objecttype)%00%(*objectname)%00%(*objecttype)"
                .into(),
            "refs/heads".into(),
            "refs/remotes".into(),
            "refs/tags".into(),
        ],
            token,
        )?;
        let mut refs = vec![];
        for line in std::str::from_utf8(&bytes)
            .map_err(|_| "Invalid refs")?
            .lines()
        {
            let f: Vec<_> = line.split('\0').collect();
            if f.len() != 5 {
                return Err("Invalid history refs".into());
            }
            let id = match (f[2], f[4]) {
                ("commit", _) => f[1].to_owned(),
                ("tag", "commit") => f[3].to_owned(),
                ("tag", "tag") => resolve(repo, f[0], token)?,
                _ => continue,
            };
            if !crate::git::is_oid(&id) {
                return Err("Invalid history ref target".into());
            }
            refs.push(RefChoice {
                name: f[0].into(),
                id,
            });
            if refs.len() > 4096 {
                return Err("File history exceeds reference budget".into());
            }
        }
        let mut roots = match branch {
            Some("HEAD") => vec![head.clone()],
            Some(name) => vec![
                refs.iter()
                    .find(|r| r.name == name)
                    .ok_or("Branch no longer exists; refresh")?
                    .id
                    .clone(),
            ],
            None => refs
                .iter()
                .map(|r| r.id.clone())
                .chain([head.clone()])
                .collect(),
        };
        roots.sort();
        roots.dedup();
        if roots.len() > 128 {
            return Err("File history exceeds branch traversal budget".into());
        }
        Ok(Self { head, refs, roots })
    }
    pub fn load(
        &self,
        repo: &Repository,
        path: &Path,
        limit: usize,
        token: &Cancel,
    ) -> Result<ScopedHistory> {
        validate(path)?;
        let head = self.head.clone();
        let refs = self.refs.clone();
        let roots = &self.roots;
        let limit = limit.max(1);
        let mut revisions = HashMap::new();
        let mut more = false;
        let mut metadata_bytes = 0usize;
        for root in roots {
            let h = load_root(repo, path, limit, root.clone(), token)?;
            more |= h.more;
            for r in h.revisions.into_iter().chain(h.older) {
                if !revisions.contains_key(&r.id) {
                    metadata_bytes +=
                        r.message.len() + r.author.len() + r.path.as_os_str().len() + 256;
                    if revisions.len() >= 16000 || metadata_bytes > BLOCK_BUDGET {
                        return Err("File history exceeds metadata budget".into());
                    }
                    revisions.insert(r.id.clone(), r);
                }
            }
        }
        // Full ancestry is temporary: path-limited Git log can rewrite parents, and
        // cannot safely be used as the real revision graph (especially across renames).
        let mut args: Vec<OsString> = [
            "rev-list",
            "--topo-order",
            "--parents",
            "--max-count=100001",
        ]
        .map(Into::into)
        .to_vec();
        args.extend(roots.iter().map(Into::into));
        args.push("--".into());
        let bytes = read(repo, args, token)?;
        let topology: Vec<Vec<String>> = std::str::from_utf8(&bytes)
            .map_err(|_| "Invalid history topology")?
            .lines()
            .map(|l| l.split_whitespace().map(str::to_owned).collect())
            .collect();
        if topology.len() > 100000 {
            return Err("File history exceeds 100000-commit ancestry budget".into());
        }
        let positions: HashMap<_, _> = topology
            .iter()
            .enumerate()
            .map(|(i, n)| (n[0].clone(), i))
            .collect();
        let mut parents = vec![];
        let mut visible = vec![];
        let mut ordered = vec![];
        for n in &topology {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            parents.push(
                n.iter()
                    .skip(1)
                    .map(|id| {
                        positions
                            .get(id)
                            .copied()
                            .ok_or_else(|| "Incomplete file history ancestry".to_owned())
                    })
                    .collect::<Result<Vec<_>>>()?,
            );
            let r = revisions.remove(&n[0]);
            visible.push(r.is_some());
            if let Some(r) = r {
                ordered.push(r);
            }
        }
        if !revisions.is_empty() {
            return Err("File history changed during loading; refresh".into());
        }
        let edges = crate::filtered_graph::project(&parents, &visible, 400000)?;
        let mut projected: HashMap<String, Vec<(String, bool)>> = HashMap::new();
        for e in edges {
            projected
                .entry(topology[e.child][0].clone())
                .or_default()
                .push((topology[e.parent][0].clone(), e.dotted));
        }
        more |= ordered.len() > limit;
        let older = ordered.get(limit).cloned();
        ordered.truncate(limit);
        let links: Vec<_> = ordered
            .iter()
            .map(|r| projected.remove(&r.id).unwrap_or_default())
            .collect();
        let mut layout = crate::graph::Layout::default();
        let mut geometry = crate::graph::GeometryPool::default();
        let mut raw = Vec::with_capacity(ordered.len());
        for (revision, parents) in ordered.iter().zip(&links) {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let mut row = layout.append_projected_bounded(&revision.id, parents)?;
            if !geometry.intern(&mut row, 500_000) {
                return Err("File history graph exceeds geometry budget".into());
            }
            raw.push(row);
        }
        let graph = crate::graph_print::project(&raw, false, token)?;
        Ok(ScopedHistory {
            head_id: head,
            history: History {
                revisions: ordered,
                older,
                more,
            },
            refs,
            graph,
            links,
        })
    }
}
/// Compare with a real parent, never a neighbouring row from another branch.
/// Rename detection uses the same parent-by-parent boundary rule as load_root.
pub fn previous_revision(
    repo: &Repository,
    revision: &Revision,
    token: &Cancel,
) -> Result<Option<Revision>> {
    let Some(parent) = revision.parents.first() else {
        return Ok(None);
    };
    let bytes = read(
        repo,
        vec![
            "diff-tree".into(),
            "--no-commit-id".into(),
            "--name-status".into(),
            "-r".into(),
            "-M".into(),
            "-z".into(),
            parent.into(),
            revision.id.clone().into(),
            "--".into(),
        ],
        token,
    )?;
    let fields: Vec<_> = bytes.split(|b| *b == 0).filter(|f| !f.is_empty()).collect();
    let mut path = revision.path.clone();
    let mut i = 0;
    while i < fields.len() {
        let kind = *fields[i].first().ok_or("Invalid parent status")?;
        i += 1;
        let old = crate::git::path_from_bytes(fields.get(i).ok_or("Missing parent path")?)?;
        i += 1;
        if matches!(kind, b'R' | b'C') {
            let new = crate::git::path_from_bytes(fields.get(i).ok_or("Missing rename path")?)?;
            i += 1;
            if new == revision.path {
                path = old;
            }
        } else if kind == b'A' && old == revision.path {
            return Ok(None);
        }
    }
    Ok(Some(Revision {
        id: parent.clone(),
        path,
        parents: vec![],
        timestamp: 0,
        author: String::new(),
        message: String::new(),
        deleted: false,
    }))
}

pub fn contents(repo: &Repository, revision: &Revision, token: &Cancel) -> Result<String> {
    validate(&revision.path)?;
    if !crate::git::is_oid(&revision.id) {
        return Err("Invalid history revision".into());
    }
    if revision.deleted {
        return Ok(String::new());
    }
    let mut spec = OsString::from(&revision.id);
    spec.push(":");
    spec.push(&revision.path);
    let bytes = repo.read(&[OsStr::new("cat-file"), OsStr::new("blob"), &spec], token)?;
    if bytes.len() > TEXT_LIMIT {
        return Err("File exceeds the 5 MiB preview limit".into());
    }
    if bytes.contains(&0) {
        return Err("Binary file cannot be displayed as text".into());
    }
    String::from_utf8(bytes).map_err(|_| "File encoding is not supported".into())
}
/// Transfer a nonempty block backwards, following Block.createPreviousBlock's
/// whitespace-insensitive line matching and greedy damaged-boundary rules.
pub fn transfer_range(current: &str, previous: &str, range: Range<usize>) -> Result<Range<usize>> {
    let previous_lines = previous.lines().count();
    if current == previous {
        let start = range.start.min(previous_lines);
        return Ok(start..range.end.clamp(start, previous_lines));
    }
    if current.lines().count().saturating_add(previous_lines) > 200_000 {
        return Err("Line comparison exceeds the 200000 line budget".into());
    }
    let started = std::time::Instant::now();
    let mapped = transfer_range_impl(current, previous, range);
    if started.elapsed() > std::time::Duration::from_millis(900) {
        return Err("Line comparison exceeded the time budget".into());
    }
    Ok(mapped)
}
fn transfer_range_impl(current: &str, previous: &str, range: Range<usize>) -> Range<usize> {
    let a: Vec<String> = previous
        .lines()
        .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect())
        .collect();
    let b: Vec<String> = current
        .lines()
        .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect())
        .collect();
    let av: Vec<&str> = a.iter().map(String::as_str).collect();
    let bv: Vec<&str> = b.iter().map(String::as_str).collect();
    let diff = similar::TextDiff::configure()
        .algorithm(similar::Algorithm::Patience)
        .timeout(std::time::Duration::from_secs(1))
        .diff_slices(&av, &bv);
    let mut start = range.start as isize;
    let mut end = range.end as isize;
    let mut shift = 0isize;
    let greedy = start != end;
    for op in diff.ops() {
        let (tag, old_range, new_range) = op.as_tag_tuple();
        let old = old_range.start;
        let ol = old_range.len();
        let new = new_range.start;
        let nl = new_range.len();
        if tag == similar::DiffTag::Equal {
            continue;
        }
        let cs = new as isize + shift;
        let ce = cs + nl as isize;
        let delta = ol as isize - nl as isize;
        let ne = ce + delta;
        (start, end) = if end <= cs {
            (start, end)
        } else if start >= ce {
            (start + delta, end + delta)
        } else if start <= cs && end >= ce {
            (start, end + delta)
        } else if start >= cs && end <= ce {
            if greedy { (cs, ne) } else { (ne, ne) }
        } else if start < cs {
            (start, if greedy { ne } else { cs })
        } else {
            (if greedy { cs } else { ne }, end + delta)
        };
        shift += delta;
        let _ = old;
    }
    let start = (start.max(0) as usize).min(a.len());
    let end = (end.max(0) as usize).clamp(start, a.len());
    start..end
}
pub fn block(text: &str, range: Range<usize>) -> String {
    text.lines()
        .skip(range.start)
        .take(range.end.saturating_sub(range.start))
        .collect::<Vec<_>>()
        .join("\n")
}
pub struct Traced {
    pub revision: Option<Revision>,
    pub block: String,
    pub lines: Range<usize>,
    pub changed: bool,
}
pub fn trace(
    repo: &Repository,
    history: &History,
    selection: &Selection,
    token: &Cancel,
) -> Result<Vec<Traced>> {
    if selection.text.len() > TEXT_LIMIT || selection.lines.start > selection.lines.end {
        return Err("Invalid or oversized selection history request".into());
    }
    let mut text = selection.text.clone();
    let mut range = selection.lines.clone();
    let mut rows = vec![Traced {
        revision: None,
        block: block(&text, range.clone()),
        lines: range.clone(),
        changed: false,
    }];
    let mut budget = rows[0].block.len();
    for revision in history.revisions.iter().chain(history.older.iter()) {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let previous = contents(repo, revision, token)?;
        let mapped = transfer_range(&text, &previous, range.clone())?;
        let content = block(&previous, mapped.clone());
        rows.last_mut().unwrap().changed = rows.last().unwrap().block != content;
        budget += content.len();
        if budget > BLOCK_BUDGET {
            return Err("Selection history exceeds the content budget".into());
        }
        rows.push(Traced {
            revision: Some(revision.clone()),
            block: content,
            lines: mapped.clone(),
            changed: false,
        });
        text = previous;
        range = mapped;
        if range.is_empty() {
            break;
        }
    }
    if !history.more {
        if let Some(last) = rows.last_mut() {
            last.changed = !last.lines.is_empty();
        }
    }
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_follows_insertions_replacements_and_deletion() {
        assert_eq!(
            transfer_range("x\na\nb\nc\n", "a\nb\nc\n", 2..3).unwrap(),
            1..2
        );
        assert_eq!(
            transfer_range("a\nnew\nc\n", "a\nold\nc\n", 1..2).unwrap(),
            1..2
        );
        assert_eq!(transfer_range("a\nnew\nc\n", "a\nc\n", 1..2).unwrap(), 1..1);
        assert_eq!(
            transfer_range("a\n b \nc\n", "a\nb\nc\n", 1..2).unwrap(),
            1..2
        );
    }
    #[test]
    fn large_line_comparisons_are_bounded_but_identical_sources_need_no_diff() {
        let a = "x\n".repeat(100_001);
        let b = format!("y\n{a}");
        assert!(transfer_range(&a, &b, 1..2).is_err());
        assert_eq!(transfer_range(&a, &a, 3..4).unwrap(), 3..4);
    }
}
