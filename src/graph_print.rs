// Copyright 2000-2024 JetBrains s.r.o. and contributors.
// Adapted from Rebased GraphLayoutBuilder, GraphElementComparatorByLayoutIndex
// and PrintElementGeneratorImpl (Apache-2.0), commit 4aec352. See REFERENCE.
//! A bounded display projection. Raw lanes/frontiers and Git parents stay intact.
use crate::{
    git::Cancel,
    graph::{GeometryPool, Row, Segment},
};
use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Node(usize),
    Boundary(usize),
}
#[derive(Clone, Debug)]
struct Edge {
    up: Option<usize>,
    down: Option<usize>,
    color: usize,
    dotted: bool,
}

// Recover the logical projected endpoints from the existing complete lane
// geometry, including filtered dotted links. Do not read Commit.parents here:
// those are deliberately unmodified metadata, not filtered display topology.
fn endpoints(rows: &[Row], token: &Cancel) -> Result<Vec<Edge>, String> {
    let mut below: Vec<_> = (0..rows.last().map_or(0, |r| r.width))
        .map(Target::Boundary)
        .collect();
    let mut edges = Vec::new();
    let mut first_styles = HashMap::new();
    for (index, row) in rows.iter().enumerate().rev() {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let mut before = vec![Target::Boundary(usize::MAX); row.width];
        before[row.node] = Target::Node(index);
        for segment in row.segments.iter().filter(|s| s.full() || !s.upper()) {
            let target = below
                .get(segment.to())
                .copied()
                .unwrap_or(Target::Boundary(segment.to()));
            if segment.from() == row.node && !segment.full() {
                if edges.len() >= 500_000 {
                    return Err("Graph print window exceeds edge budget".into());
                }
                edges.push(Edge {
                    up: Some(index),
                    down: match target {
                        Target::Node(i) => Some(i),
                        _ => None,
                    },
                    color: segment.color,
                    dotted: segment.dotted(),
                });
            } else {
                before[segment.from()] = target;
            }
        }
        if index == 0 {
            for segment in row.segments.iter().filter(|s| s.full() || s.upper()) {
                first_styles
                    .entry(segment.to())
                    .or_insert((segment.color, segment.dotted()));
            }
        }
        below = before;
    }
    for (lane, (color, dotted)) in first_styles {
        if edges.len() >= 500_000 {
            return Err("Graph print window exceeds edge budget".into());
        }
        edges.push(Edge {
            up: None,
            down: match below.get(lane) {
                Some(Target::Node(i)) => Some(*i),
                _ => None,
            },
            color,
            dotted,
        });
    }
    Ok(edges)
}

// GraphLayoutBuilder's iterative first-parent DFS, in visible head order.
fn layout_indices(count: usize, edges: &[Edge]) -> Vec<usize> {
    let mut parents = vec![Vec::new(); count];
    let mut children = vec![false; count];
    for edge in edges {
        if let (Some(up), Some(down)) = (edge.up, edge.down) {
            parents[up].push(down);
            children[down] = true;
        }
    }
    // Endpoints were collected backwards; retain each node's parent order.
    let mut result = vec![0; count];
    let mut next = 1;
    for head in (0..count).filter(|&i| !children[i]) {
        let mut stack = vec![head];
        while let Some(&node) = stack.last() {
            let first = result[node] == 0;
            if first {
                result[node] = next;
            }
            if let Some(&parent) = parents[node].iter().find(|&&p| result[p] == 0) {
                stack.push(parent);
            } else {
                if first {
                    next += 1;
                }
                stack.pop();
            }
        }
    }
    result
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Element {
    Node(usize),
    Edge(usize),
}
fn edge_node(edge: &Edge, node: usize, indices: &[usize]) -> Ordering {
    let rank = match (edge.up, edge.down) {
        (Some(up), Some(down)) => indices[up].max(indices[down]),
        (Some(endpoint), None) | (None, Some(endpoint)) => indices[endpoint],
        _ => return Ordering::Greater, // unknown through-frontiers sort after loaded topology
    };
    let order = rank.cmp(&indices[node]);
    if order != Ordering::Equal || edge.up.is_none() || edge.down.is_none() {
        order
    } else {
        edge.up.unwrap().cmp(&node)
    }
}
fn compare(a: Element, b: Element, edges: &[Edge], indices: &[usize]) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    match (a, b) {
        (Element::Node(a), Element::Node(b)) => a.cmp(&b),
        (Element::Edge(a), Element::Node(b)) => edge_node(&edges[a], b, indices),
        (Element::Node(a), Element::Edge(b)) => edge_node(&edges[b], a, indices).reverse(),
        (Element::Edge(a), Element::Edge(b)) => {
            let (a, b) = (&edges[a], &edges[b]);
            if a.up.is_none() && a.down.is_none() {
                return if b.up.is_none() && b.down.is_none() {
                    a.color.cmp(&b.color)
                } else {
                    Ordering::Greater
                };
            }
            if b.up.is_none() && b.down.is_none() {
                return Ordering::Less;
            }
            if a.up.is_none() || a.down.is_none() {
                return edge_node(b, a.up.or(a.down).unwrap(), indices).reverse();
            }
            if b.up.is_none() || b.down.is_none() {
                return edge_node(a, b.up.or(b.down).unwrap(), indices);
            }
            match (a.up, a.down, b.up, b.down) {
                (Some(au), Some(ad), Some(bu), Some(bd)) if au == bu => {
                    if ad < bd {
                        edge_node(b, ad, indices).reverse()
                    } else {
                        edge_node(a, bd, indices)
                    }
                }
                (Some(au), Some(_), Some(bu), Some(_)) => {
                    if au < bu {
                        edge_node(a, bu, indices)
                    } else {
                        edge_node(b, au, indices).reverse()
                    }
                }
                (_, _, Some(bu), Some(bd)) => edge_node(a, bu.min(bd), indices),
                (Some(au), Some(ad), _, _) => edge_node(b, au.min(ad), indices).reverse(),
                _ => a.color.cmp(&b.color),
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Policy {
    long: usize,
    part: usize,
    edge_with_arrow: usize,
}
fn arrow(edge: &Edge, row: usize, policy: Policy) -> Option<bool> {
    match (edge.up, edge.down) {
        (Some(up), Some(down)) => {
            let (long, part) = (policy.long, policy.part);
            if down - up >= long {
                if row == up + part {
                    return Some(false);
                }
                if row + part == down {
                    return Some(true);
                }
            }
            if down - up >= policy.edge_with_arrow {
                if row == up + 1 {
                    return Some(false);
                }
                if row + 1 == down {
                    return Some(true);
                }
            }
            None
        }
        (Some(up), None) if row == up + 1 => Some(false),
        (None, Some(down)) if row + 1 == down => Some(true),
        _ => None,
    }
}

/// Translate default/long-edge print visibility and compact position ordering.
/// Cancellation is polled per row; lookup/event state is released on return.
pub struct Model {
    edges: Vec<Edge>,
    colors: Vec<usize>,
    indices: Vec<usize>,
    adjacent: Vec<Vec<usize>>,
}
impl Model {
    pub fn new(rows: &[Row], token: &Cancel) -> Result<Self, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let count = rows.len();
        if count > crate::git::HISTORY_MAX {
            return Err("Graph print window exceeds history budget".into());
        }

        let edges = endpoints(rows, token)?;
        if edges.len() > 500_000 {
            return Err("Graph print window exceeds edge budget".into());
        }
        let indices = layout_indices(count, &edges);
        let mut adjacent = vec![Vec::new(); count];
        for (i, edge) in edges.iter().enumerate() {
            if let Some(up) = edge.up {
                adjacent[up].push(i);
            }
            if let Some(down) = edge.down {
                adjacent[down].push(i);
            }
        }
        Ok(Self {
            edges,
            colors: rows.iter().map(|r| r.color).collect(),
            indices,
            adjacent,
        })
    }
    pub fn project(&self, show_long: bool, token: &Cancel) -> Result<Vec<Row>, String> {
        self.project_with_policy(
            if show_long {
                Policy {
                    long: 1000,
                    part: 250,
                    edge_with_arrow: 30,
                }
            } else {
                Policy {
                    long: 30,
                    part: 1,
                    edge_with_arrow: usize::MAX,
                }
            },
            token,
        )
    }
    fn project_with_policy(&self, policy: Policy, token: &Cancel) -> Result<Vec<Row>, String> {
        let count = self.colors.len();
        let edges = &self.edges;
        let indices = &self.indices;
        let adjacent = &self.adjacent;
        let mut starts = vec![Vec::new(); count + 1];
        let mut ends = vec![Vec::new(); count + 1];
        let mut range = |edge, start: usize, end: usize| {
            if start <= end && start < count {
                starts[start].push(edge);
                ends[(end + 1).min(count)].push(edge);
            }
        };
        for (i, edge) in edges.iter().enumerate() {
            match (edge.up, edge.down) {
                (Some(up), Some(down)) => {
                    let (long, part) = (policy.long, policy.part);
                    if down - up >= long {
                        range(i, up + 1, up + part);
                        range(i, down - part, down - 1);
                    } else if down > up + 1 {
                        range(i, up + 1, down - 1);
                    }
                }
                (Some(up), None) => range(i, up + 1, up + 1),
                (None, Some(down)) if down > 0 => range(i, down - 1, down - 1),
                (None, None) => range(i, 0, count - 1),
                _ => {}
            }
        }
        // Like the upstream bounded row cache, only adjacent row positions are
        // needed to paint a row. Never retain O(rows * active lanes) hash maps.
        let mut active = BTreeSet::new();
        let mut next_row = |row: usize| -> Result<_, String> {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            if row >= count {
                return Ok((Vec::new(), HashMap::new()));
            }
            for edge in &ends[row] {
                active.remove(edge);
            }
            active.extend(starts[row].iter().copied());
            if active.len() >= 512 {
                return Err("Graph print row exceeds lane budget".into());
            }
            let mut list: Vec<_> = active.iter().copied().map(Element::Edge).collect();
            list.push(Element::Node(row));
            list.sort_by(|a, b| compare(*a, *b, &edges, &indices));
            let map = list
                .iter()
                .enumerate()
                .map(|(i, &element)| (element, i))
                .collect::<HashMap<_, _>>();
            Ok((list, map))
        };
        let (mut current, mut current_positions) = next_row(0)?;
        let mut previous_positions = HashMap::new();
        let mut pool = GeometryPool::default();
        let mut output = Vec::with_capacity(count);
        for row in 0..count {
            let (next, next_positions) = next_row(row + 1)?;
            let mut segments = Vec::new();
            let mut arrow_targets = Vec::new();
            let node = current_positions[&Element::Node(row)];
            for (position, element) in current.iter().enumerate() {
                let ids: Vec<_> = match *element {
                    Element::Node(_) => adjacent[row].clone(),
                    Element::Edge(i) => vec![i],
                };
                for id in ids {
                    let edge = &edges[id];
                    for upper in [true, false] {
                        let neighbor = if upper {
                            row.checked_sub(1)
                        } else {
                            (row + 1 < count).then_some(row + 1)
                        };
                        let map = if upper {
                            &previous_positions
                        } else {
                            &next_positions
                        };
                        let other = neighbor.and_then(|r| {
                            map.get(&Element::Edge(id)).copied().or_else(|| {
                                (if upper { edge.up } else { edge.down })
                                    .filter(|&endpoint| endpoint == r)
                                    .and_then(|endpoint| map.get(&Element::Node(endpoint)).copied())
                            })
                        });
                        let terminal = arrow(edge, row, policy) == Some(upper);
                        if other.is_none() && !terminal {
                            continue;
                        }
                        let other = other.unwrap_or(position);
                        if terminal {
                            if let Some(target) = if upper { edge.up } else { edge.down } {
                                arrow_targets.push((
                                    u16::try_from(segments.len())
                                        .expect("segment index exceeds lane budget"),
                                    u16::try_from(target).expect("target exceeds history budget"),
                                ));
                            }
                        }
                        segments.push(Segment::new(
                            if upper { other } else { position },
                            if upper { position } else { other },
                            edge.color,
                            upper,
                            false,
                            edge.dotted,
                            !terminal || map.contains_key(&Element::Edge(id)),
                            terminal,
                        ));
                    }
                }
            }
            let mut printed = Row {
                node,
                color: self.colors[row],
                width: current.len(),
                segments: segments.into(),
                arrow_targets: (!arrow_targets.is_empty()).then(|| arrow_targets.into()),
            };
            if !pool.intern(&mut printed, 500_000) {
                return Err("Graph print geometry exceeds window budget".into());
            }
            output.push(printed);
            previous_positions = current_positions;
            current_positions = next_positions;
            current = next;
        }
        Ok(output)
    }
}

pub fn project(rows: &[Row], show_long: bool, token: &Cancel) -> Result<Vec<Row>, String> {
    Model::new(rows, token)?.project(show_long, token)
}
#[cfg(test)]
fn project_with_policy(rows: &[Row], policy: Policy, token: &Cancel) -> Result<Vec<Row>, String> {
    Model::new(rows, token)?.project_with_policy(policy, token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{git::Commit, graph};
    #[test]
    fn reusable_model_releases_raw_geometry_and_switches_without_changing_topology() {
        let raw = graph::layout_bounded(&chain(40, true), 500_000).unwrap();
        let geometry = std::sync::Arc::downgrade(&raw[1].segments);
        let model = Model::new(&raw, &Cancel::default()).unwrap();
        drop(raw);
        assert!(
            geometry.upgrade().is_none(),
            "the display model must not retain raw row matrices"
        );
        let hidden = model.project(false, &Cancel::default()).unwrap();
        assert_eq!(model.project(true, &Cancel::default()).unwrap()[2].width, 2);
        let restored = model.project(false, &Cancel::default()).unwrap();
        for (before, after) in hidden.iter().zip(&restored) {
            assert_eq!(
                (before.node, &before.segments),
                (after.node, &after.segments)
            );
        }
    }
    fn upstream_fixture(input: &str, expected: &str) {
        let mut layout = graph::Layout::default();
        let mut raw = Vec::new();
        for line in input.lines() {
            let (id, parents) = line.split_once("|-").unwrap();
            let id = id.split('_').next().unwrap();
            let parents: Vec<_> = parents
                .split_whitespace()
                .map(|parent| {
                    let (id, style) = parent.split_once('_').unwrap();
                    (id.to_owned(), style == "D")
                })
                .collect();
            raw.push(layout.append_projected(id, &parents));
        }
        // Exactly PrintElementGeneratorTest's default test policy, not the
        // production 30/1 policy. Compare node/edge geometry, style and arrows;
        // that Java fixture intentionally uses a different color manager.
        let printed = project_with_policy(
            &raw,
            Policy {
                long: 7,
                part: 2,
                edge_with_arrow: 10,
            },
            &Cancel::default(),
        )
        .unwrap();
        let mut wanted_nodes = Vec::new();
        let mut wanted_edges = Vec::new();
        for line in expected.lines() {
            let (kind, rest) = line.trim().split_once("|-").unwrap();
            let coords: Vec<usize> = rest
                .split("|-")
                .next()
                .unwrap()
                .split(':')
                .map(|v| v.parse().unwrap())
                .collect();
            if kind == "Node" {
                wanted_nodes.push((coords[0], coords[1]));
            } else {
                wanted_edges.push((
                    coords[0],
                    coords[1],
                    coords[2],
                    kind.contains("UP"),
                    kind.contains("ARROW"),
                    kind.contains("DASHED"),
                ));
            }
        }
        let actual_nodes: Vec<_> = printed
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r.node))
            .collect();
        let mut actual_edges: Vec<_> = printed
            .iter()
            .enumerate()
            .flat_map(|(i, r)| {
                r.segments.iter().map(move |s| {
                    (
                        i,
                        if s.upper() { s.to() } else { s.from() },
                        if s.upper() { s.from() } else { s.to() },
                        s.upper(),
                        s.arrow(),
                        s.dotted(),
                    )
                })
            })
            .collect();
        wanted_nodes.sort();
        wanted_edges.sort();
        actual_edges.sort();
        assert_eq!(actual_nodes, wanted_nodes);
        assert_eq!(actual_edges, wanted_edges);
    }
    #[test]
    fn rebased_many_nodes_and_long_edges_print_fixtures_match() {
        upstream_fixture(
            include_str!("../tests/fixtures/rebased-manyNodes-in.txt"),
            include_str!("../tests/fixtures/rebased-manyNodes-out.txt"),
        );
        upstream_fixture(
            include_str!("../tests/fixtures/rebased-longEdges-in.txt"),
            include_str!("../tests/fixtures/rebased-longEdges-out.txt"),
        );
    }
    fn chain(count: usize, long: bool) -> Vec<Commit> {
        (0..count)
            .map(|i| Commit {
                id: i.to_string(),
                parents: {
                    let mut p = Vec::new();
                    if i + 1 < count {
                        p.push((i + 1).to_string());
                    }
                    if i == 0 && long {
                        p.push((count - 1).to_string());
                    }
                    p
                },
                author: String::new(),
                date: String::new(),
                subject: String::new(),
                refs: vec![],
                shallow_boundary: false,
            })
            .collect()
    }
    #[test]
    fn long_edge_uses_endpoint_arrows_and_compacts_intermediate_positions() {
        let commits = chain(40, true);
        let raw = graph::layout_bounded(&commits, 500_000).unwrap();
        let printed = project(&raw, false, &Cancel::default()).unwrap();
        assert_eq!(printed.len(), commits.len());
        assert_eq!(printed[2].width, 1);
        assert_eq!(
            printed[1].arrow_target(
                printed[1]
                    .segments
                    .iter()
                    .position(|s| s.arrow() && !s.upper())
                    .unwrap()
            ),
            Some(39)
        );
        assert_eq!(
            printed[38].arrow_target(
                printed[38]
                    .segments
                    .iter()
                    .position(|s| s.arrow() && s.upper())
                    .unwrap()
            ),
            Some(0)
        );
        assert!(
            printed[1]
                .segments
                .iter()
                .any(|s| s.arrow() && !s.upper() && !s.adjacent())
        );
        assert!(
            printed[38]
                .segments
                .iter()
                .any(|s| s.arrow() && s.upper() && !s.adjacent())
        );
        let expanded = project(&raw, true, &Cancel::default()).unwrap();
        assert_eq!(expanded[2].width, 2);
        assert_eq!(
            expanded[1].arrow_target(
                expanded[1]
                    .segments
                    .iter()
                    .position(|s| s.arrow() && !s.upper())
                    .unwrap()
            ),
            Some(39)
        );
        assert!(
            expanded[1]
                .segments
                .iter()
                .any(|s| s.arrow() && !s.upper() && s.adjacent())
        );
        assert_eq!(
            commits[0].parents,
            ["1", "39"],
            "display must never replace Git parents"
        );
    }
    #[test]
    fn short_edges_remain_continuous_and_missing_endpoints_stay_explicit() {
        let raw = graph::layout_bounded(&chain(30, true), 500_000).unwrap();
        let short = project(&raw, false, &Cancel::default()).unwrap();
        assert!(
            short
                .iter()
                .flat_map(|r| r.segments.iter())
                .all(|s| !s.arrow())
        );
        assert_eq!(short[2].width, 2);
        let partial = project(&raw[..10], false, &Cancel::default()).unwrap();
        assert!(
            partial[1].segments.iter().any(|s| s.arrow()),
            "out-of-window parents need a boundary marker"
        );
        assert!(partial[1].arrow_targets.is_none());
        let middle = project(&raw[5..15], false, &Cancel::default()).unwrap();
        assert_eq!(middle.len(), 10);
        assert!(
            middle.iter().all(|r| r.width >= 2),
            "unknown through-frontier must not be silently dropped"
        );
        let cancelled = Cancel::default();
        cancelled.cancel();
        assert!(project(&raw, false, &cancelled).is_err());
    }
    #[test]
    fn filtered_arrow_targets_visible_projected_rows_not_original_parent_indices() {
        let commits = chain(81, true);
        let matching = commits
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, c)| c.id.clone())
            .collect();
        let filtered =
            crate::filtered_graph::window(&commits, &matching, 0, 41, 500_000, 500_000).unwrap();
        let printed = project(&filtered.rows, false, &Cancel::default()).unwrap();
        let target = printed[1].arrow_target(
            printed[1]
                .segments
                .iter()
                .position(|s| s.arrow() && !s.upper())
                .unwrap(),
        );
        assert_eq!(target, Some(40));
        assert_eq!(
            commits[0].parents,
            ["1", "80"],
            "source metadata stays untouched"
        );
        assert!(printed.iter().all(|r| {
            r.segments
                .iter()
                .enumerate()
                .filter(|(_, s)| s.arrow())
                .all(|(index, _)| r.arrow_target(index).is_none_or(|i| i < printed.len()))
        }));
    }
    #[test]
    fn recovered_endpoints_match_every_real_parent_on_generated_dags() {
        let mut commits = chain(100, false);
        let mut seed = 7_u64;
        for i in 0..99 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let extra = i + 1 + seed as usize % (99 - i);
            if !commits[i].parents.contains(&extra.to_string()) {
                commits[i].parents.push(extra.to_string());
            }
        }
        let raw = graph::layout_bounded(&commits, 500_000).unwrap();
        let recovered = endpoints(&raw, &Cancel::default()).unwrap();
        for (i, commit) in commits.iter().enumerate() {
            let actual: Vec<_> = recovered
                .iter()
                .filter(|e| e.up == Some(i))
                .map(|e| e.down.unwrap().to_string())
                .collect();
            assert_eq!(actual, commit.parents);
        }
        let printed = project(&raw, false, &Cancel::default()).unwrap();
        assert_eq!(printed.len(), commits.len());
        assert!(printed.iter().all(|r| r.node < r.width));
    }
    #[test]
    fn filtering_and_paged_frontiers_keep_valid_print_positions() {
        for seed in 1..40 {
            let mut commits = chain(60, false);
            let mut random = seed as u64;
            for i in 0..59 {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                let parent = (i + 1 + random as usize % (59 - i)).to_string();
                if !commits[i].parents.contains(&parent) {
                    commits[i].parents.push(parent);
                }
            }
            let raw = graph::layout_bounded(&commits, 500_000).unwrap();
            for (start, end) in [(0, 20), (15, 45), (40, 60)] {
                let printed = project(&raw[start..end], false, &Cancel::default()).unwrap();
                assert_eq!(printed.len(), end - start);
                assert!(printed.iter().all(|r| r.node < r.width));
            }
            let matching = commits
                .iter()
                .enumerate()
                .filter(|(i, _)| i % 3 != 1)
                .map(|(_, c)| c.id.clone())
                .collect();
            let filtered =
                crate::filtered_graph::window(&commits, &matching, 5, 20, 500_000, 500_000)
                    .unwrap();
            let printed = project(&filtered.rows, false, &Cancel::default()).unwrap();
            assert_eq!(printed.len(), filtered.rows.len());
            assert!(
                printed
                    .iter()
                    .flat_map(|r| r.segments.iter())
                    .any(|s| s.dotted())
            );
        }
    }
}
