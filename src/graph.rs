//! Incremental lane layout: every edge connects a commit to an actual parent.
use crate::git::Commit;
use std::{collections::HashMap, hash::{Hash, Hasher}, sync::Arc};
/// Immutable geometry for one half-edge. Product layout is capped at 512
/// lanes; compact coordinates are checked, never truncated. Color remains a
/// full identity across paged windows, independent of the renderer's palette.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Segment {
    from: u16,
    to: u16,
    pub color: usize,
    flags: u8,
}
impl Segment {
    pub(crate) fn new(from: usize, to: usize, color: usize, upper: bool, full: bool, dotted: bool, adjacent: bool, arrow: bool) -> Self {
        Self {
            from: u16::try_from(from).expect("graph coordinate exceeds compact storage"),
            to: u16::try_from(to).expect("graph coordinate exceeds compact storage"),
            color,
            flags: (upper as u8) | ((full as u8) << 1) | ((dotted as u8) << 2)
                | ((adjacent as u8) << 3) | ((arrow as u8) << 4),
        }
    }
    pub fn from(&self) -> usize { self.from as usize }
    pub fn to(&self) -> usize { self.to as usize }
    pub fn upper(&self) -> bool { self.flags & 1 != 0 }
    /// Whole-row continuation, equivalent to both upper/lower halves.
    pub fn full(&self) -> bool { self.flags & 2 != 0 }
    pub fn dotted(&self) -> bool { self.flags & 4 != 0 }
    /// Adjacent row centers are joined, clipped to this row.
    pub fn adjacent(&self) -> bool { self.flags & 8 != 0 }
    /// Arrow at the outer end of this half-edge, including terminal stubs.
    pub fn arrow(&self) -> bool { self.flags & 16 != 0 }

}
#[derive(Clone, Debug)]
pub struct Row {
    pub node: usize,
    pub color: usize,
    pub width: usize,
    pub segments: Arc<[Segment]>,
    /// Sparse (segment index, target row) navigation identities, excluded from
    /// geometry sharing. None for raw layouts and rows without known targets.
    pub(crate) arrow_targets: Option<Arc<[(u16, u16)]>>,
}
impl Row {
    pub fn arrow_target(&self, segment: usize) -> Option<usize> {
        let targets = self.arrow_targets.as_deref()?;
        let index = targets.binary_search_by_key(&segment, |&(index, _)| index as usize).ok()?;
        Some(targets[index].1 as usize)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArrowHit {
    pub segment: usize,
    pub target: Option<usize>,
}

/// SimpleGraphCellPainter's node-first, edge-order elliptical hit test,
/// expressed in the same logical 16px lanes / 28px rows used by graph_cell.
/// Only arrow-bearing edges are actionable; an earlier ordinary edge or node
/// still wins an overlap, matching the painter's element selection order.
pub fn arrow_at(row: &Row, x: f32, y: f32) -> Option<ArrowHit> {
    if !(0. ..28.).contains(&y) || !x.is_finite() { return None; }
    if (x - (8. + row.node as f32 * 16.)).hypot(y - 14.) <= 4. { return None; }
    for (index, edge) in row.segments.iter().enumerate() {
        let current = if edge.upper() { edge.to() } else { edge.from() };
        let other = if edge.upper() { edge.from() } else { edge.to() };
        let (x1, y1) = (8. + current as f32 * 16., 14.);
        let (x2, y2) = (8. + other as f32 * 16., if edge.upper() { -14. } else { 42. });
        if (x1 - x).hypot(y1 - y) + (x2 - x).hypot(y2 - y) < (x1 - x2).hypot(y1 - y2) + 1.5 {
            return edge.arrow().then(|| ArrowHit { segment: index, target: row.arrow_target(index) });
        }
        // Raw full segments represent both halves, and are never arrows.
        if edge.full() && (x - x1).abs() < 1.5 && y >= 14. { return None; }
    }
    None
}

/// GraphCommitCellUtil.getGraphWidth: text follows this row's node/edges,
/// with at most six columns of recommended blank space. A wide row can still
/// exceed six; its owning Commit cell, not the other table columns, clips it.
pub fn text_offset_columns(row: &Row, recommended_width: usize) -> f32 {
    let mut last = row.node as f32;
    for edge in row.segments.iter() {
        let current = if edge.upper() { edge.to() } else { edge.from() } as f32;
        last = last.max(current).max((edge.from() as f32 + edge.to() as f32) / 2.);
    }
    (last + 1.).max(recommended_width.min(6) as f32)
}

/// One layout window owns the lookup; only immutable row geometry survives it.
/// Repeated lane patterns share storage, not identities or topology. Compare
/// the complete slice after hashing so collisions cannot alter a single edge.
#[derive(Default)]
pub(crate) struct GeometryPool {
    buckets: HashMap<u64, Vec<Arc<[Segment]>>>,
    stored: usize,
}
impl GeometryPool {
    pub(crate) fn intern(&mut self, row: &mut Row, max_segments: usize) -> bool {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        row.segments.hash(&mut hash);
        let bucket = self.buckets.entry(hash.finish()).or_default();
        if let Some(shared) = bucket.iter().find(|shared| **shared == row.segments) {
            row.segments = shared.clone();
            return true;
        }
        let stored = self.stored.saturating_add(row.segments.len());
        if stored > max_segments { return false; }
        self.stored = stored;
        bucket.push(row.segments.clone());
        true
    }
}
#[derive(Default, Clone, Debug)]
pub struct Layout {
    lanes: Vec<(String, usize, bool)>,
    next_color: usize,
}
impl Layout {
    pub fn append(&mut self, commit: &Commit) -> Row {
        self.append_edges(
            &commit.id,
            commit.parents.iter().map(|p| (p.as_str(), false)),
        )
    }
    /// Lay out explicit projected parent links without rewriting Git metadata.
    pub fn append_projected(&mut self, id: &str, parents: &[(String, bool)]) -> Row {
        self.append_edges(id, parents.iter().map(|(p, dotted)| (p.as_str(), *dotted)))
    }
    pub fn append_projected_bounded(
        &mut self,
        id: &str,
        parents: &[(String, bool)],
    ) -> Result<Row, String> {
        if self.lanes.len().saturating_add(parents.len()) > 512 {
            return Err("Filtered graph exceeds the 512-lane complexity budget".into());
        }
        Ok(self.append_projected(id, parents))
    }
    fn append_edges<'a>(
        &mut self,
        id: &str,
        parents: impl Iterator<Item = (&'a str, bool)> + Clone,
    ) -> Row {
        let incoming = self.lanes.iter().any(|(target, _, _)| target == id);
        let node = self
            .lanes
            .iter()
            .position(|(target, _, _)| target == id)
            .unwrap_or_else(|| {
                let n = self.lanes.len();
                self.lanes.push((id.to_owned(), self.next_color, false));
                self.next_color += 1;
                n
            });
        let before = self.lanes.clone();
        let color = before[node].1;
        self.lanes.remove(node);
        for (i, (parent, dotted)) in parents.clone().enumerate() {
            if let Some(lane) = self.lanes.iter_mut().find(|(id, _, _)| id == parent) {
                // At a shared lane, preserve a real parent continuation when
                // both direct and collapsed paths reach the same target.
                lane.2 &= dotted;
            } else {
                let c = if i == 0 {
                    color
                } else {
                    let c = self.next_color;
                    self.next_color += 1;
                    c
                };
                self.lanes.insert(
                    (node + i).min(self.lanes.len()),
                    (parent.to_owned(), c, dotted),
                );
            }
        }
        let mut segments = Vec::with_capacity(before.len().saturating_add(parents.clone().count()));
        for (from, (id, c, dotted)) in before.iter().enumerate() {
            if from != node {
                let to = self.lanes.iter().position(|(x, _, _)| x == id).unwrap();
                // Preserve the exact two halves, but store/draw one path when
                // their geometry and style form a continuous vertical edge.
                if from == to && *dotted == self.lanes[to].2 {
                    segments.push(Segment::new(from, to, *c, true, true, *dotted, false, false));
                    continue;
                }
            }
            if from != node || incoming {
                segments.push(Segment::new(from, from, *c, true, false, *dotted, false, false));
            }
            if from != node {
                let to = self.lanes.iter().position(|(x, _, _)| x == id).unwrap();
                segments.push(Segment::new(from, to, *c, false, false, self.lanes[to].2, false, false));
            }
        }
        for (parent, dotted) in parents {
            let to = self
                .lanes
                .iter()
                .position(|(id, _, _)| id == parent)
                .unwrap();
            segments.push(Segment::new(node, to, self.lanes[to].1, false, false, dotted, false, false));
        }
        Row {
            node,
            color,
            width: before.len().max(self.lanes.len()),
            segments: segments.into(),
            arrow_targets: None,
        }
    }
}
pub fn layout(commits: &[Commit]) -> Vec<Row> {
    let mut l = Layout::default();
    commits.iter().map(|c| l.append(c)).collect()
}
/// Bound the geometry as well as the number of commits. A wide DAG must not
/// silently turn a fixed history window into an unbounded allocation.
pub fn layout_bounded(commits: &[Commit], max_segments: usize) -> Result<Vec<Row>, String> {
    layout_window(commits, Layout::default(), max_segments).map(|(rows, _)| rows)
}

/// Resume exactly at a prior window's frontier; rows and frontier are returned
/// together so a failed layout cannot advance the caller's paging state.
pub fn layout_window(
    commits: &[Commit],
    mut state: Layout,
    max_segments: usize,
) -> Result<(Vec<Row>, Layout), String> {
    let mut rows = Vec::with_capacity(commits.len());
    let mut geometry = GeometryPool::default();
    for commit in commits {
        // Check before append's lane searches and cloning. An octopus commit
        // or wide frontier must not consume quadratic CPU before we reject it.
        const MAX_LANES: usize = 512;
        if commit.parents.len() > MAX_LANES
            || state.lanes.len().saturating_add(commit.parents.len()) > MAX_LANES
        {
            return Err("Graph exceeds the 512-lane complexity budget. Select a narrower branch to continue; no partial graph is shown.".into());
        }
        let mut row = state.append(commit);
        if !geometry.intern(&mut row, max_segments) {
            return Err("Graph exceeds this window's geometry budget. Select a narrower branch or refresh with a smaller history window.".into());
        }
        rows.push(row);
    }
    Ok((rows, state))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segment_storage_fits_two_machine_words() {
        // Up to 500,000 unique segments live in one history window. Their bounded coordinates
        // and boolean styles must not each consume a full machine word.
        assert!(std::mem::size_of::<Segment>() <= 2 * std::mem::size_of::<usize>(),
            "segment storage is {} bytes", std::mem::size_of::<Segment>());
    }
    #[test]
    fn compact_segment_preserves_all_style_combinations_and_full_color_identity() {
        for flags in 0..32 {
            let values = [1, 2, 4, 8, 16].map(|mask| flags & mask != 0);
            let segment = Segment::new(511, 0, usize::MAX, values[0], values[1], values[2], values[3], values[4]);
            assert_eq!((segment.from(), segment.to(), segment.color), (511, 0, usize::MAX));
            assert_eq!([segment.upper(), segment.full(), segment.dotted(), segment.adjacent(), segment.arrow()], values);
        }
    }
    #[test]
    fn arrow_hit_testing_follows_edges_and_gives_nodes_and_earlier_edges_priority() {
        let arrow = Segment::new(1, 3, 17, false, false, false, true, true);
        let mut row = Row { node: 0, color: 0, width: 4, segments: vec![arrow.clone()].into(), arrow_targets: Some(Arc::from([(0,39)])) };
        assert_eq!(arrow_at(&row, 32., 21.).unwrap().target, Some(39));
        assert!(arrow_at(&row, 24., 27.).is_none(), "diagonal bounding box is not a clickable edge");
        assert!(arrow_at(&row, 32., 28.).is_none(), "do not cross row boundaries");
        assert!(arrow_at(&row, f32::NAN, 21.).is_none());
        row.node = 1;
        assert!(arrow_at(&row, 24., 14.).is_none(), "node takes priority over arrow edge");
        row.node = 0;
        row.segments = vec![Segment::new(1, 3, 7, false, false, false, true, false), arrow].into();
        assert!(arrow_at(&row, 32., 21.).is_none(), "first ordinary edge wins an overlap");
        row.segments = vec![Segment::new(2, 2, 9, true, false, true, false, true)].into();
        row.arrow_targets = None;
        assert!(arrow_at(&row, 40., 5.).unwrap().target.is_none(), "unknown frontier is a hit, never a fabricated target");
    }
    #[test]
    fn shared_arrow_geometry_keeps_navigation_targets_owned_by_each_row() {
        let segments: Arc<[Segment]> = vec![Segment::new(1, 1, 8, false, false, false, false, true)].into();
        let mut first = Row { node: 0, color: 0, width: 2, segments: segments.clone(), arrow_targets: Some(Arc::from([(0, 39)])) };
        let mut second = Row { node: 0, color: 0, width: 2, segments, arrow_targets: Some(Arc::from([(0, 79)])) };
        let mut pool = GeometryPool::default();
        assert!(pool.intern(&mut first, 1));
        assert!(pool.intern(&mut second, 1));
        assert!(Arc::ptr_eq(&first.segments, &second.segments));
        assert_eq!(arrow_at(&first, 24., 24.).unwrap().target, Some(39));
        assert_eq!(arrow_at(&second, 24., 24.).unwrap().target, Some(79));
    }
    fn c(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: id.into(),
            parents: parents.iter().map(|s| (*s).into()).collect(),
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            refs: Vec::new(),
            shallow_boundary: false,
        }
    }
    #[test]
    fn projected_edge_style_survives_intermediate_rows_and_resume() {
        let mut state = Layout::default();
        let first = state.append_projected("child", &[("ancestor".into(), true)]);
        assert!(first.segments.iter().all(|e| e.dotted()));
        let mut resumed = state.clone();
        for layout in [&mut state, &mut resumed] {
            let middle = layout.append(&c("unrelated", &[]));
            assert!(middle.segments.iter().all(|e| e.dotted()));
            let last = layout.append_projected("ancestor", &[("root".into(), false)]);
            assert!(last.segments.iter().filter(|e| e.upper()).all(|e| e.dotted()));
            assert!(last.segments.iter().filter(|e| !e.upper()).all(|e| !e.dotted()));
        }
        assert_eq!(format!("{state:?}"), format!("{resumed:?}"));
    }
    #[test]
    fn direct_edge_wins_when_continuations_share_a_lane() {
        let mut state = Layout::default();
        state.append_projected("filtered", &[("parent".into(), true)]);
        state.append_projected("direct", &[("parent".into(), false)]);
        let row = state.append(&c("parent", &[]));
        assert!(row.segments.iter().all(|edge| !edge.dotted()));
    }
    #[test]
    fn merge_edges_and_paging() {
        let commits = vec![
            c("m", &["a", "b", "c"]),
            c("a", &["r"]),
            c("b", &["r"]),
            c("c", &["r"]),
            c("r", &[]),
        ];
        let rows = layout(&commits);
        assert_eq!(rows[0].segments.iter().filter(|e| !e.upper()).count(), 3);
        let mut p = Layout::default();
        let mut paged = vec![];
        for page in commits.chunks(2) {
            for c in page {
                paged.push(p.append(c));
            }
        }
        assert_eq!(format!("{rows:?}"), format!("{paged:?}"));
        assert_eq!(p.lanes.len(), 0);
    }
    #[test]
    fn disconnected_roots_have_no_invented_incoming_edges() {
        let rows = layout(&[c("a", &[]), c("b", &[])]);
        assert!(rows.iter().all(|row| row.segments.is_empty()));
    }

    #[test]
    fn rejects_excessive_parent_fanout_before_layout() {
        let mut commit = c("octopus", &[]);
        commit.parents = (0..513).map(|i| format!("parent-{i}")).collect();
        let error = layout_bounded(&[commit], usize::MAX).unwrap_err();
        assert!(error.contains("512-lane"));
    }

    #[test]
    fn generated_dags_keep_every_parent_and_continuation() {
        let mut seed = 12345_u64;
        let mut commits = Vec::new();
        for index in 0..150 {
            let mut parents = Vec::new();
            for _ in 0..3 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let remaining = 149 - index;
                if remaining > 0 {
                    let parent = (index + 1 + (seed as usize % remaining)).to_string();
                    if !parents.contains(&parent) {
                        parents.push(parent);
                    }
                }
            }
            let mut commit = c(&index.to_string(), &[]);
            commit.parents = parents;
            commits.push(commit);
        }
        let mut state = Layout::default();
        for commit in &commits {
            let before = state.lanes.clone();
            let row = state.append(commit);
            assert!(row.segments.iter().filter(|e| e.full()).all(|e| e.from() == e.to() && e.from() != row.node));
            for (lane, (id, color, dotted)) in before.iter().enumerate() {
                if id != &commit.id {
                    let top = row.segments.iter().find(|e| (e.upper() || e.full()) && e.from() == lane).unwrap();
                    assert_eq!((top.color, top.dotted()), (*color, *dotted), "upper continuation keeps its incoming style");
                }
            }
            let outgoing = row
                .segments
                .iter()
                .filter(|edge| !edge.upper() && edge.from() == row.node)
                .collect::<Vec<_>>();
            assert_eq!(outgoing.len(), commit.parents.len());
            for (edge, parent) in outgoing.iter().zip(&commit.parents) {
                assert_eq!(&state.lanes[edge.to()].0, parent);
            }
            for (lane, (id, _, _)) in before.iter().enumerate() {
                if id == &commit.id {
                    continue;
                }
                let edge = row
                    .segments
                    .iter()
                    .find(|edge| (!edge.upper() || edge.full()) && edge.from() == lane)
                    .unwrap();
                assert_eq!(&state.lanes[edge.to()].0, id);
            }
        }
        assert!(state.lanes.is_empty());
        assert!(layout_bounded(&commits, 1).is_err());
        let shared = layout_bounded(&commits, 500_000).unwrap();
        for (expected, actual) in layout(&commits).iter().zip(&shared) {
            assert_eq!((expected.node, expected.color, expected.width),
                (actual.node, actual.color, actual.width));
            assert_eq!(expected.segments, actual.segments, "sharing must preserve every edge and its style");
        }
    }

    #[test]
    fn repeated_rows_share_geometry_and_release_it_with_the_window() {
        let commits = (0..1000).map(|i| {
            let mut commit = c(&i.to_string(), &[]);
            if i < 999 { commit.parents.push((i + 1).to_string()); }
            commit
        }).collect::<Vec<_>>();
        // First row: 1 half-edge; middle pattern: 2; last row: 1.
        // A fifth distinct stored segment still fails the caller's budget.
        assert!(layout_bounded(&commits, 3).is_err());
        let rows = layout_bounded(&commits, 4).unwrap();
        assert_eq!(rows.len(), 1000);
        assert!(Arc::ptr_eq(&rows[1].segments, &rows[998].segments));
        assert_eq!(rows[998].segments.len(), 2);
        let weak = Arc::downgrade(&rows[1].segments);
        drop(rows);
        assert!(weak.upgrade().is_none(), "no global cache may retain closed windows");
    }
}

#[cfg(test)]
mod wide_regression {
    use super::*;
    fn commit(id: String, parents: Vec<String>) -> Commit {
        Commit { id, parents, author: String::new(), date: String::new(), subject: String::new(), refs: vec![], shallow_boundary: false }
    }
    #[test]
    fn ten_thousand_commits_on_32_lanes_fit_existing_geometry_budget() {
        let mut commits = Vec::new();
        for i in 0..14 { commits.push(commit(format!("post-{i}"), vec![if i==13 { "merge".into() } else { format!("post-{}",i+1) }])); }
        commits.push(commit("merge".into(), (0..32).map(|b| format!("branch-{b}-311")).collect()));
        for step in (0..312).rev() {
            for branch in (0..32).rev() {
                commits.push(commit(format!("branch-{branch}-{step}"), vec![if step==0 {"root".into()} else {format!("branch-{branch}-{}",step-1)}]));
            }
        }
        commits.push(commit("root".into(), vec![]));
        assert_eq!(commits.len(), 10000);
        let rows = layout_bounded(&commits, 500_000).expect("32-lane wide graph should fit without raising the existing budget");
        assert_eq!(rows.len(), 10000);
        assert_eq!(rows.iter().map(|r|r.width).max(),Some(32));
        assert_eq!(rows[14].segments.iter().filter(|e| !e.upper()).count(),32);
    }
    #[test]
    fn ten_thousand_commits_on_256_lanes_fit_existing_geometry_budget() {
        let mut commits = Vec::new();
        for i in 0..14 { commits.push(commit(format!("post-{i}"), vec![if i==13 { "merge".into() } else { format!("post-{}",i+1) }])); }
        commits.push(commit("merge".into(), (0..256).map(|b| format!("branch-{b}-38")).collect()));
        for step in (0..39).rev() {
            for branch in (0..256).rev() {
                commits.push(commit(format!("branch-{branch}-{step}"), vec![if step==0 {"root".into()} else {format!("branch-{branch}-{}",step-1)}]));
            }
        }
        commits.push(commit("root".into(), vec![]));
        assert_eq!(commits.len(), 10000);
        let rows = layout_bounded(&commits, 500_000).expect("256-lane wide graph should fit without raising the existing budget");
        assert_eq!(rows.len(), 10000);
        assert_eq!(rows.iter().map(|r|r.width).max(),Some(256));
        assert_eq!(rows[14].segments.iter().filter(|e| !e.upper()).count(),256);
        assert!(Arc::ptr_eq(&rows[15].segments, &rows[15 + 256].segments));
        assert_eq!(text_offset_columns(&rows[0], 256), 6.,
            "linear commits above a wide merge must keep their subjects visible");
        assert!(text_offset_columns(&rows[14], 256) > 6.,
            "six is a recommended-space limit, never a hard graph clip");
        assert!(rows.iter().all(|row| text_offset_columns(row, 256) > row.node as f32),
            "text must follow the actual node in every row");
        let printed = crate::graph_print::project(&rows, false, &crate::git::Cancel::default()).unwrap();
        for (mode, printed) in [("default", printed.clone()), ("expanded", crate::graph_print::project(&rows, true, &crate::git::Cancel::default()).unwrap())] {
            let mut unique = std::collections::HashSet::new();
            let segments: usize = printed.iter().filter(|row| unique.insert(Arc::as_ptr(&row.segments) as *const Segment)).map(|row| row.segments.len()).sum();
            assert!(segments * std::mem::size_of::<Segment>() <= 5 * 1024 * 1024,
                "{mode} print geometry exceeds the 5 MiB pressure-fixture storage budget");
        }
        assert_eq!(printed.len(), 10000);
        assert_eq!(printed[0].width, 1);
        assert!(printed[15].width >= 256,
            "all merge endpoint arrows remain explicit, even on a wide row");
        assert!(printed[16].width < 40 && printed[1000].width < 6,
            "the long-edge interior must compact after the endpoint arrow row");
        assert!(printed.iter().flat_map(|r| r.segments.iter()).any(|s| s.arrow()));
    }
}
