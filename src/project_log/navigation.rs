//! Root-qualified navigation over bounded project history pages.
use super::{CommitId, Layout, Page, Read, Scope};
use crate::{git::Cancel, history};
use std::sync::Arc;

pub struct Window {
    pub read: Read,
    pub rows: Vec<crate::graph::Row>,
    pub start: Layout,
    pub end: Layout,
    pub offset: usize,
    pub selected: usize,
    /// Survives transferring metadata/geometry into the view.
    len: usize,
}

pub struct Continuation {
    read: Read,
    start: Layout,
    offset: usize,
}
impl Window {
    pub fn continuation(&self) -> Result<Continuation, String> {
        Ok(Continuation {
            read: self.read.cursor(),
            start: self.end.clone(),
            offset: self
                .offset
                .checked_add(self.len)
                .ok_or("History offset exceeds supported size")?,
        })
    }
}
impl Continuation {
    pub fn load(self, limit: usize, token: &Cancel) -> Result<Window, String> {
        let read = self.read.next_page(limit, token)?;
        let (rows, end) = read.page.layout_window(self.start.clone(), token)?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let len = read.page.entries.len();
        Ok(Window {
            read,
            rows,
            start: self.start,
            end,
            offset: self.offset,
            selected: 0,
            len,
        })
    }
}
pub enum Location {
    Found(Window),
    Hidden,
    OutsideRoots,
}

/// Resolve filtering in the target repository before traversing global pages.
/// No metadata from skipped pages survives; graph frontier and per-root cursors
/// remain bounded. The caller owns cancellation/generation and result publication.
pub fn locate(
    scope: Arc<Scope>,
    target: &CommitId,
    limit: usize,
    token: &Cancel,
) -> Result<Location, String> {
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    let root = scope
        .roots
        .iter()
        .find(|root| root.query.repository.path == *target.root)
        .ok_or("Commit repository is not in the loaded history")?;
    let matching = match &root.query.filter {
        Some(filter) => history::locate_filtered(
            &root.query.repository,
            &root.tips,
            &target.hash,
            1,
            filter,
            token,
        )?,
        None => {
            match history::locate(&root.query.repository, &root.tips, &target.hash, 1, token)? {
                Some(found) => history::FilteredLocation::Found(found),
                None => history::FilteredLocation::OutsideRoots,
            }
        }
    };
    match matching {
        history::FilteredLocation::Hidden => return Ok(Location::Hidden),
        history::FilteredLocation::OutsideRoots => {
            let all = root.query.repository.snapshot(None, 1, None, token)?;
            return Ok(
                if root
                    .query
                    .repository
                    .commit_position(&all.tips, &target.hash, token)?
                    .is_some()
                {
                    Location::Hidden
                } else {
                    Location::OutsideRoots
                },
            );
        }
        history::FilteredLocation::Found(_) => {}
    }
    let mut read = Read {
        page: Page {
            entries: vec![],
            more: true,
        },
        scope,
        offsets: Default::default(),
    };
    let mut offset = 0usize;
    let mut start = Layout::default();
    loop {
        read = read.next_page(limit, token)?;
        let selected = read
            .page
            .entries
            .iter()
            .position(|entry| entry.identity() == *target);
        let (rows, end) = read.page.layout_window(start.clone(), token)?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if let Some(selected) = selected {
            let len = read.page.entries.len();
            return Ok(Location::Found(Window {
                len,
                read,
                rows,
                start,
                end,
                offset,
                selected,
            }));
        }
        if !read.page.more || read.page.entries.is_empty() {
            return Ok(Location::OutsideRoots);
        }
        offset = offset
            .checked_add(read.page.entries.len())
            .ok_or("History offset exceeds supported size")?;
        start = end;
        // Query state/cursors survive; previous metadata does not.
        read.page.entries.clear();
    }
}

/// Rebuild a bounded global window at an explicit offset using fixed sources.
/// Skipped metadata is discarded as the graph frontier advances.
pub fn window_at(
    scope: Arc<Scope>,
    offset: usize,
    limit: usize,
    token: &Cancel,
) -> Result<Window, String> {
    let mut read = Read {
        page: Page {
            entries: vec![],
            more: true,
        },
        scope,
        offsets: Default::default(),
    };
    let mut skipped = 0usize;
    let mut start = Layout::default();
    while skipped < offset {
        read = read.next_page((offset - skipped).min(crate::git::HISTORY_PAGE), token)?;
        let (_, end) = read.page.layout_window(start, token)?;
        start = end;
        skipped = skipped
            .checked_add(read.page.entries.len())
            .ok_or("History offset exceeds supported size")?;
        let exhausted = !read.page.more || read.page.entries.is_empty();
        read.page.entries.clear();
        if exhausted {
            break;
        }
    }
    read = read.next_page(limit, token)?;
    let (rows, end) = read.page.layout_window(start.clone(), token)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    let len = read.page.entries.len();
    Ok(Window {
        len,
        read,
        rows,
        start,
        end,
        offset: skipped,
        selected: 0,
    })
}
