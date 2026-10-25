//! Rebased GitCompareBranchesUi: two opposite immutable revision ranges.
//! Resolve again on refresh, but keep both panes and their file diffs on one pair.
use crate::git::{Cancel, Commit, HISTORY_MAX, LogFilter, Repository, is_oid, parse_history};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    pub first_ref: String,
    pub second_ref: String,
    pub first: String,
    pub second: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    First,
    Second,
}
impl Pair {
    pub fn resolve(
        repo: &Repository,
        first: &str,
        second: &str,
        token: &Cancel,
    ) -> Result<Self, String> {
        Ok(Self {
            first_ref: first.into(),
            second_ref: second.into(),
            first: resolve_reference(repo, first, token)?,
            second: resolve_reference(repo, second, token)?,
        })
    }
    pub fn reversed(&self) -> Self {
        Self {
            first_ref: self.second_ref.clone(),
            second_ref: self.first_ref.clone(),
            first: self.second.clone(),
            second: self.first.clone(),
        }
    }
    fn range(&self, side: Side) -> Result<String, String> {
        if !is_oid(&self.first) || !is_oid(&self.second) {
            return Err("Invalid comparison revision".into());
        }
        Ok(match side {
            Side::First => format!("{}..{}", self.second, self.first),
            Side::Second => format!("{}..{}", self.first, self.second),
        })
    }
    pub fn page(
        &self,
        repo: &Repository,
        side: Side,
        offset: usize,
        limit: usize,
        token: &Cancel,
    ) -> Result<Page, String> {
        self.page_filtered(repo, side, offset, limit, &LogFilter::default(), token)
    }
    /// Detail filters intersect this side's immutable range. Hash-search precedence
    /// belongs to the query controller, not to Git's --grep arguments.
    pub fn page_filtered(
        &self,
        repo: &Repository,
        side: Side,
        offset: usize,
        limit: usize,
        filter: &LogFilter,
        token: &Cancel,
    ) -> Result<Page, String> {
        let range = self.range(side)?;
        filter.validate_paths()?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if filter.matches_no_authors() {
            return Ok(Page {
                commits: vec![],
                more: false,
            });
        }
        let limit = limit.clamp(1, HISTORY_MAX);
        let count = format!("--max-count={}", limit + 1);
        let skip = format!("--skip={offset}");
        let filter_args = filter.arguments();
        let mut args = vec![
            "--literal-pathspecs",
            "-c",
            "log.mailmap=false",
            "log",
            "--date-order",
            "--no-show-signature",
            "--encoding=UTF-8",
            "--date=iso-strict",
            "-z",
            &count,
            &skip,
            "--format=%H%x00%P%x00%an%x00%ad%x00%s",
        ];
        args.extend(filter_args.iter().map(String::as_str));
        args.extend([range.as_str(), "--"]);
        args.extend(filter.paths.iter().map(String::as_str));
        let output = repo.text(&args, token)?;
        let mut commits = parse_history(&output)?;
        let more = commits.len() > limit;
        commits.truncate(limit);
        if !filter.paths.is_empty() && !commits.is_empty() {
            // Path-history simplification rewrites parent IDs. Restore real parents
            // before constructing a commit Diff, including all merge parents.
            let ids = commits.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
            commits = repo
                .snapshot_commits(&ids, &[self.first.clone(), self.second.clone()], token)?
                .commits;
        }
        let (_, boundaries) = repo.shallow_boundaries(token)?;
        for commit in &mut commits {
            commit.shallow_boundary = boundaries.contains(&commit.id);
        }
        Ok(Page { commits, more })
    }
    /// Separate from page loading so the first paint never waits for a full count.
    pub fn count(&self, repo: &Repository, side: Side, token: &Cancel) -> Result<usize, String> {
        self.count_filtered(repo, side, &LogFilter::default(), token)
    }
    pub fn count_filtered(
        &self,
        repo: &Repository,
        side: Side,
        filter: &LogFilter,
        token: &Cancel,
    ) -> Result<usize, String> {
        let range = self.range(side)?;
        filter.validate_paths()?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if filter.matches_no_authors() {
            return Ok(0);
        }
        let filter_args = filter.arguments();
        let mut args = vec![
            "--literal-pathspecs",
            "-c",
            "log.mailmap=false",
            "rev-list",
            "--count",
        ];
        args.extend(filter_args.iter().map(String::as_str));
        args.extend([range.as_str(), "--"]);
        args.extend(filter.paths.iter().map(String::as_str));
        repo.text(&args, token)?
            .trim()
            .parse()
            .map_err(|_| "Invalid comparison count".into())
    }
    /// Net tree changes, not a concatenation of each exclusive commit's patches.
    pub fn changes(
        &self,
        repo: &Repository,
        token: &Cancel,
    ) -> Result<Vec<crate::combined_changes::Change>, String> {
        repo.revision_changes(&self.second, &self.first, &[], token)
    }
}
pub struct Page {
    pub commits: Vec<Commit>,
    pub more: bool,
}

/// One pane's query. Ordinary filters retain its range; successful hash lookup
/// overrides the range and other filters, matching VcsLogFiltererImpl.
#[derive(Clone)]
pub struct Query {
    pair: Pair,
    side: Side,
    filter: LogFilter,
    hashes: Option<(Vec<String>, Vec<String>)>,
}
impl Query {
    pub fn prepare(
        repo: &Repository,
        pair: Pair,
        side: Side,
        filter: LogFilter,
        token: &Cancel,
    ) -> Result<Self, String> {
        pair.range(side)?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let hashes = if let Some(prefixes) = filter.hashes() {
            let mut roots = repo.snapshot(None, 1, None, token)?.tips;
            roots.extend([pair.first.clone(), pair.second.clone()]);
            roots.sort();
            roots.dedup();
            let ids = crate::history::ordered_hash_matches(repo, &roots, &prefixes, token)?;
            (!ids.is_empty()).then_some((roots, ids))
        } else {
            None
        };
        if hashes.is_none() {
            filter.validate_paths()?;
        }
        Ok(Self {
            pair,
            side,
            filter,
            hashes,
        })
    }
    pub fn page(
        &self,
        repo: &Repository,
        offset: usize,
        limit: usize,
        token: &Cancel,
    ) -> Result<Page, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if let Some((roots, ids)) = &self.hashes {
            let start = offset.min(ids.len());
            let end = start
                .saturating_add(limit.clamp(1, HISTORY_MAX))
                .min(ids.len());
            let commits = if start == end {
                vec![]
            } else {
                repo.snapshot_commits(&ids[start..end], roots, token)?
                    .commits
            };
            return Ok(Page {
                commits,
                more: end < ids.len(),
            });
        }
        self.pair
            .page_filtered(repo, self.side, offset, limit, &self.filter, token)
    }
    pub fn needs_graph_projection(&self) -> bool {
        self.hashes.is_some()
            || !self.filter.text.trim().is_empty()
            || self.filter.authors.is_some()
            || self.filter.after.is_some()
            || self.filter.before.is_some()
            || !self.filter.paths.is_empty()
    }

    /// Project the whole bounded range before slicing, so page boundaries and
    /// hidden commits cannot sever the displayed ancestry. Diff parents stay real.
    pub fn graph_page(
        &self,
        repo: &Repository,
        offset: usize,
        limit: usize,
        token: &Cancel,
    ) -> Result<(Page, Vec<crate::graph::Row>), String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let (topology, matched, roots) = if let Some((roots, ids)) = &self.hashes {
            (
                repo.bounded_topology(roots, None, token)?,
                ids.iter().cloned().collect(),
                roots.clone(),
            )
        } else {
            self.filter.validate_paths()?;
            if self.filter.matches_no_authors() {
                return Ok((
                    Page {
                        commits: vec![],
                        more: false,
                    },
                    vec![],
                ));
            }
            let (excluded, included) = match self.side {
                Side::First => (&self.pair.second, &self.pair.first),
                Side::Second => (&self.pair.first, &self.pair.second),
            };
            let topology = repo.bounded_topology(&[included.clone()], None, token)?;
            let matched = repo
                .bounded_range_topology(excluded, included, Some(&self.filter), token)?
                .into_iter()
                .map(|c| c.id)
                .collect();
            (
                topology,
                matched,
                vec![self.pair.first.clone(), self.pair.second.clone()],
            )
        };
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let projected =
            crate::filtered_graph::window(&topology, &matched, offset, limit, 500_000, 500_000)?;
        let ids = projected
            .indices
            .iter()
            .map(|&i| topology[i].id.clone())
            .collect::<Vec<_>>();
        let commits = if ids.is_empty() {
            vec![]
        } else {
            repo.snapshot_commits(&ids, &roots, token)?.commits
        };
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok((
            Page {
                commits,
                more: projected.more,
            },
            projected.rows,
        ))
    }

    pub fn count(&self, repo: &Repository, token: &Cancel) -> Result<usize, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if let Some((_, ids)) = &self.hashes {
            return Ok(ids.len());
        }
        self.pair
            .count_filtered(repo, self.side, &self.filter, token)
    }
}

pub fn resolve_reference(
    repo: &Repository,
    reference: &str,
    token: &Cancel,
) -> Result<String, String> {
    if reference != "HEAD"
        && !reference.starts_with("refs/heads/")
        && !reference.starts_with("refs/remotes/")
    {
        return Err("Choose a local or remote branch".into());
    }
    if reference != "HEAD" {
        repo.text(&["check-ref-format", reference], token)?;
    }
    let id = repo.text(
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ],
        token,
    )?;
    let id = id.trim();
    if !is_oid(id) {
        return Err("Invalid comparison revision".into());
    }
    Ok(id.to_owned())
}
