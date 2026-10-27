// Algorithm adapted from JetBrains DottedFilterEdgesGenerator.kt (Apache-2.0).
// Copyright 2000-2024 JetBrains s.r.o. and contributors.
// Reference: detachhead/rebased 4aec35256d765c217aa0d662b82ac4e860c396fd,
// platform/vcs-log/graph/src/com/intellij/vcs/log/graph/collapsing/
// DottedFilterEdgesGenerator.kt. See LICENSE and docs/REBASED-REFERENCE.md.
//! Visible graph projection following Rebased's DottedFilterEdgesGenerator.
//! Input is a complete, bounded DAG in children-before-parents order. Missing
//! parents/window boundary arrows must be handled by the caller, not invented.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub child: usize,
    pub parent: usize,
    pub dotted: bool,
}

/// Keep direct visible edges and generate dotted connections across hidden rows.
/// Indices refer to the input graph, not to positions in the filtered result.
/// This deliberately does not mutate Commit.parents (used by actual diffs).
pub fn project(
    parents: &[Vec<usize>],
    visible: &[bool],
    max_edges: usize,
) -> Result<Vec<Edge>, String> {
    const MAX_NODES: usize = 100_000;
    let count = parents.len();
    if count != visible.len() || count > MAX_NODES {
        return Err("Invalid or oversized filtered graph".into());
    }
    let mut input_edges = 0usize;
    for (child, links) in parents.iter().enumerate() {
        input_edges = input_edges.saturating_add(links.len());
        if input_edges > max_edges || links.iter().any(|&p| p <= child || p >= count) {
            return Err(
                "Filtered graph exceeds edge budget or is not topologically ordered".into(),
            );
        }
    }
    let mut children = vec![Vec::new(); count];
    let mut edges = BTreeMap::new();
    for (child, links) in parents.iter().enumerate() {
        for &parent in links {
            children[parent].push(child);
            if visible[child] && visible[parent] {
                insert(&mut edges, child, parent, false, max_edges)?;
            }
        }
    }
    // Rebased downWalk: propagate the nearest visible descendant index.
    let mut numbers = vec![None; count];
    for node in 0..count {
        let mut nearest = None;
        let mut adjacent = None;
        for &child in &children[node] {
            if visible[node] {
                if visible[child] {
                    adjacent = adjacent.max(numbers[child]);
                } else {
                    nearest = nearest.max(numbers[child]);
                }
            } else {
                nearest = nearest.max(if visible[child] {
                    Some(child)
                } else {
                    numbers[child]
                });
            }
        }
        numbers[node] = if !visible[node] {
            nearest
        } else if nearest.is_none() || nearest == adjacent {
            adjacent
        } else {
            insert(&mut edges, nearest.unwrap(), node, true, max_edges)?;
            nearest
        };
    }
    // Rebased cleanup + upWalk: propagate the nearest visible ancestor index.
    numbers.fill(None);
    for node in (0..count).rev() {
        let mut nearest = None;
        let mut adjacent = None;
        for &parent in &parents[node] {
            if visible[node] {
                if visible[parent] {
                    adjacent = min_present(adjacent, numbers[parent]);
                } else {
                    nearest = min_present(nearest, numbers[parent]);
                }
            } else {
                nearest = min_present(
                    nearest,
                    if visible[parent] {
                        Some(parent)
                    } else {
                        numbers[parent]
                    },
                );
            }
        }
        numbers[node] = if !visible[node] {
            nearest
        } else if nearest.is_none() || nearest == adjacent {
            adjacent
        } else {
            insert(&mut edges, node, nearest.unwrap(), true, max_edges)?;
            nearest
        };
    }
    Ok(edges
        .into_iter()
        .map(|((child, parent), dotted)| Edge {
            child,
            parent,
            dotted,
        })
        .collect())
}

fn min_present(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
fn insert(
    edges: &mut BTreeMap<(usize, usize), bool>,
    child: usize,
    parent: usize,
    dotted: bool,
    budget: usize,
) -> Result<(), String> {
    if let Some(existing) = edges.get_mut(&(child, parent)) {
        // An actual parent edge takes precedence over an alternate hidden path.
        *existing &= dotted;
    } else {
        if edges.len() >= budget {
            return Err("Filtered graph exceeds projected edge budget".into());
        }
        edges.insert((child, parent), dotted);
    }
    Ok(())
}

/// Minimal graph view shared by visible commits and lightweight hidden nodes.
pub trait GraphNode {
    fn id(&self) -> &str;
    fn parents(&self) -> &[String];
}
impl GraphNode for crate::git::Commit {
    fn id(&self) -> &str {
        &self.id
    }
    fn parents(&self) -> &[String] {
        &self.parents
    }
}
impl GraphNode for crate::git::TopologyCommit {
    fn id(&self) -> &str {
        &self.id
    }
    fn parents(&self) -> &[String] {
        &self.parents
    }
}

/// Adjacent *visible* graph nodes, in row order, including dotted edges.
/// Rebased GoToParentOrChildAction uses the compiled graph, not Git parents.
pub fn adjacent<T: GraphNode>(
    commits: &[T],
    matching_ids: &std::collections::HashSet<String>,
    selected: &str,
    parent: bool,
    max_edges: usize,
) -> Result<Vec<String>, String> {
    let (visible, edges) = snapshot_edges(commits, matching_ids, max_edges)?;
    let Some(index) = commits.iter().position(|c| c.id() == selected) else {
        return Ok(Vec::new());
    };
    if !visible[index] {
        return Ok(Vec::new());
    }
    let mut rows = edges
        .into_iter()
        .filter_map(|edge| {
            if parent && edge.child == index {
                Some(edge.parent)
            } else if !parent && edge.parent == index {
                Some(edge.child)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    rows.sort_unstable();
    rows.dedup();
    Ok(rows
        .into_iter()
        .map(|i| commits[i].id().to_owned())
        .collect())
}

fn snapshot_edges<T: GraphNode>(
    commits: &[T],
    matching_ids: &std::collections::HashSet<String>,
    max_edges: usize,
) -> Result<(Vec<bool>, Vec<Edge>), String> {
    if commits.len() > 100_000 {
        return Err("Filtered graph snapshot exceeds node budget".into());
    }
    let mut ids = std::collections::HashMap::with_capacity(commits.len());
    for (index, commit) in commits.iter().enumerate() {
        if ids.insert(commit.id(), index).is_some() {
            return Err("Duplicate commit in filtered graph snapshot".into());
        }
    }
    if matching_ids.iter().any(|id| !ids.contains_key(id.as_str())) {
        return Err("Filter results do not belong to this graph snapshot".into());
    }
    let mut parents = Vec::with_capacity(commits.len());
    let mut edge_count = 0usize;
    for commit in commits {
        edge_count = edge_count.saturating_add(commit.parents().len());
        if edge_count > max_edges {
            return Err("Filtered graph exceeds edge budget".into());
        }
        parents.push(
            commit
                .parents()
                .iter()
                .map(|id| {
                    ids.get(id.as_str()).copied().ok_or_else(|| {
                        "Filtered graph snapshot is incomplete: parent history is missing"
                            .to_owned()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    let visible: Vec<_> = commits
        .iter()
        .map(|commit| matching_ids.contains(commit.id()))
        .collect();
    let edges = project(&parents, &visible, max_edges)?;
    Ok((visible, edges))
}

/// One bounded visible window. Commit metadata is owned by the caller; indices
/// identify the exact source commits whose Diff comparisons must be preserved.
pub struct Window {
    /// Display-only projected parents, aligned with indices/rows.
    pub parents: Vec<Vec<(String, bool)>>,
    pub indices: Vec<usize>,
    pub rows: Vec<crate::graph::Row>,
    pub more: bool,
}

/// Assemble a filtered window from a complete snapshot. Reject incomplete input
/// rather than treating missing parents as roots. Reconstruct skipped geometry
/// one row at a time so previous windows do not remain allocated.
pub fn window<T: GraphNode>(
    commits: &[T],
    matching_ids: &std::collections::HashSet<String>,
    offset: usize,
    limit: usize,
    max_edges: usize,
    max_segments: usize,
) -> Result<Window, String> {
    let (visible, edges) = snapshot_edges(commits, matching_ids, max_edges)?;
    // Projection edges are ordered by child, so no per-node adjacency clone is
    // needed. Only one row's projected parent names are materialized at a time.
    let mut edges = edges.iter().peekable();
    let total = visible.iter().filter(|&&v| v).count();
    let limit = limit.clamp(1, crate::git::HISTORY_MAX);
    let end = offset.saturating_add(limit).min(total);
    let mut state = crate::graph::Layout::default();
    let mut rows = Vec::new();
    let mut indices = Vec::new();
    let mut parents = Vec::new();
    let mut ordinal = 0usize;
    let mut geometry = crate::graph::GeometryPool::default();
    for (index, commit) in commits.iter().enumerate() {
        if !visible[index] {
            continue;
        }
        if ordinal >= end {
            break;
        }
        let mut links = Vec::new();
        while edges.peek().is_some_and(|edge| edge.child == index) {
            let edge = edges.next().unwrap();
            links.push((commits[edge.parent].id().to_owned(), edge.dotted));
        }
        let mut row = state.append_projected_bounded(commit.id(), &links)?;
        if ordinal >= offset {
            if !geometry.intern(&mut row, max_segments) {
                return Err("Filtered graph exceeds window geometry budget".into());
            }
            rows.push(row);
            indices.push(index);
            parents.push(links);
        }
        ordinal += 1;
    }
    Ok(Window {
        parents,
        indices,
        rows,
        more: end < total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_orders_merge_and_fork_choices_and_rejects_partial_graph() {
        use crate::git::TopologyCommit;
        let graph = vec![
            TopologyCommit {
                id: "merge".into(),
                parents: vec!["right".into(), "left".into()],
            },
            TopologyCommit {
                id: "left".into(),
                parents: vec!["base".into()],
            },
            TopologyCommit {
                id: "right".into(),
                parents: vec!["base".into()],
            },
            TopologyCommit {
                id: "base".into(),
                parents: vec![],
            },
        ];
        let visible = graph.iter().map(|c| c.id.clone()).collect();
        assert_eq!(
            adjacent(&graph, &visible, "merge", true, 8).unwrap(),
            ["left", "right"]
        );
        assert_eq!(
            adjacent(&graph, &visible, "base", false, 8).unwrap(),
            ["left", "right"]
        );
        assert!(
            adjacent(&graph, &visible, "merge", false, 8)
                .unwrap()
                .is_empty()
        );
        assert!(adjacent(&graph, &visible, "merge", true, 1).is_err());
        let partial = graph[..3].to_vec();
        let visible = partial.iter().map(|c| c.id.clone()).collect();
        assert!(adjacent(&partial, &visible, "merge", true, 8).is_err());
    }
    #[test]
    fn hidden_chain_and_disconnected_matches() {
        let graph = vec![vec![1], vec![2], vec![], vec![]];
        assert_eq!(
            project(&graph, &[true, false, true, true], 20).unwrap(),
            vec![Edge {
                child: 0,
                parent: 2,
                dotted: true
            }]
        );
        assert_eq!(
            project(&graph, &[true; 4], 20).unwrap(),
            vec![
                Edge {
                    child: 0,
                    parent: 1,
                    dotted: false
                },
                Edge {
                    child: 1,
                    parent: 2,
                    dotted: false
                }
            ]
        );
    }
    #[test]
    fn hidden_merge_connects_both_visible_sides() {
        let graph = vec![vec![1], vec![2, 3], vec![4], vec![4], vec![]];
        let edges = project(&graph, &[true, false, true, true, true], 20).unwrap();
        for parent in [2, 3] {
            assert!(edges.contains(&Edge {
                child: 0,
                parent,
                dotted: true
            }));
        }
        for child in [2, 3] {
            assert!(edges.contains(&Edge {
                child,
                parent: 4,
                dotted: false
            }));
        }
        assert_eq!(edges.len(), 4);
    }
    #[test]
    fn invalid_and_oversized_input_is_rejected() {
        assert!(project(&[vec![0]], &[true], 10).is_err());
        assert!(project(&[vec![1]], &[true], 10).is_err());
        assert!(project(&[vec![]], &[], 10).is_err());
        assert!(project(&[vec![1], vec![]], &[true, true], 0).is_err());
    }
    #[test]
    fn generated_projection_edges_always_have_a_real_ancestry_path() {
        let mut seed = 891_u64;
        for _ in 0..40 {
            let mut graph = vec![vec![]; 40];
            let mut visible = vec![false; 40];
            for child in 0..40 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                visible[child] = seed & 4 != 0;
                for parent in child + 1..40 {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    if seed % 13 == 0 {
                        graph[child].push(parent);
                    }
                }
            }
            for edge in project(&graph, &visible, 10_000).unwrap() {
                assert!(visible[edge.child] && visible[edge.parent]);
                if !edge.dotted {
                    assert!(graph[edge.child].contains(&edge.parent));
                }
                let mut reachable = vec![false; 40];
                reachable[edge.child] = true;
                for node in edge.child..40 {
                    if reachable[node] {
                        for &p in &graph[node] {
                            reachable[p] = true;
                        }
                    }
                }
                assert!(reachable[edge.parent]);
            }
        }
    }
}
