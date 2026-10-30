// Copyright 2000-2017 JetBrains s.r.o.
// Rust adaptation of VcsStatusMerger.kt, Apache-2.0; see THIRD-PARTY.txt.
//! Merge the per-parent statuses used by Rebased Log details. This differs
//! from interpreting the first status character in Git's combined output.
use crate::git::{Change, Result};
use std::collections::HashMap;

fn kind(change: &Change) -> Result<u8> {
    match change.status.as_bytes().first() {
        Some(b'A') => Ok(b'A'),
        Some(b'D') => Ok(b'D'),
        Some(b'R' | b'C') if change.old_path.is_some() => Ok(b'R'),
        Some(b'M' | b'T') => Ok(b'M'),
        _ => Err("Unsupported Log file status".into()),
    }
}

/// Inputs are in commit-parent order. Preserve first-parent path order and
/// include only paths present in every parent comparison. No file bodies.
pub fn merge_parents(groups: &[Vec<Change>]) -> Result<Vec<Change>> {
    let count = groups
        .iter()
        .try_fold(0usize, |total, group| total.checked_add(group.len()))
        .ok_or("Log change count overflow")?;
    if count > crate::combined_changes::MAX_INPUT_CHANGES {
        return Err("Log preview exceeds the change limit".into());
    }
    for file in groups.iter().flatten() {
        kind(file)?;
    }
    let Some(first) = groups.first() else {
        return Ok(Vec::new());
    };
    if groups.len() == 1 {
        return Ok(first.clone());
    }
    let maps: Vec<_> = groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|file| (file.path.as_path(), file))
                .collect::<HashMap<_, _>>()
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for file in first {
        if !seen.insert(file.path.as_path()) {
            continue;
        }
        let Some(statuses) = maps
            .iter()
            .map(|m| m.get(file.path.as_path()).copied())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let first = statuses[0];
        let first_kind = kind(first)?;
        let same_kind = statuses.iter().all(|s| kind(s).ok() == Some(first_kind));
        let same_source = statuses.iter().all(|s| s.old_path == first.old_path);
        if same_kind && (first_kind != b'R' || same_source) {
            result.push(first.clone());
        } else {
            let deleted = statuses.iter().any(|s| kind(s).ok() == Some(b'D'));
            result.push(Change {
                status: if deleted { "D" } else { "M" }.into(),
                old_path: None,
                path: first.path.clone(),
                area: first.area,
            });
        }
    }
    Ok(result)
}

/// Selection is supplied in displayed Log row order, with full immutable OIDs.
/// It is deliberately not expanded into a revision range: skipped commits stay
/// unselected, following VcsLogUtil.collectChanges.
pub struct Snapshot {
    pub commits: Vec<crate::git::Commit>,
    pub files: Vec<crate::combined_changes::Change>,
}
pub fn load(
    repo: &crate::git::Repository,
    ids: &[String],
    token: &crate::git::Cancel,
) -> Result<Snapshot> {
    let mut trace =
        crate::performance_trace::Span::new("request", "log-selection", Some(&repo.path));
    trace.outcome("error");
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    if ids.len() > crate::git::HISTORY_MAX
        || ids.iter().any(|id| !crate::git::is_oid(id))
        || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
    {
        return Err("Invalid Log selection".into());
    }
    if ids.is_empty() {
        trace.outcome("ok");
        return Ok(Snapshot {
            commits: vec![],
            files: vec![],
        });
    }
    let crate::git::SelectionRows { commits, shallow } = repo.selection_rows(ids, token)?;
    let mut changes = Vec::new();
    for files in repo.log_commit_changes_batch(&commits.iter().rev().collect::<Vec<_>>(), token)? {
        if files.len() > crate::combined_changes::MAX_INPUT_CHANGES.saturating_sub(changes.len()) {
            return Err("Log preview exceeds the change limit".into());
        }
        changes.extend(files);
    }
    if repo.shallow_boundaries(token)? != shallow {
        return Err("Shallow history changed; reload the Log selection".into());
    }
    let files = crate::combined_changes::zip_changes(changes)?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    trace.outcome("ok");
    Ok(Snapshot { commits, files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::Area;
    fn file(status: &str, path: &str, old: Option<&str>) -> Change {
        Change {
            area: Area::Commit,
            status: status.into(),
            path: path.into(),
            old_path: old.map(Into::into),
        }
    }
    #[test]
    fn intersects_all_parents_and_preserves_first_parent_order() {
        let merged = merge_parents(&[
            vec![
                file("M", "b", None),
                file("M", "a", None),
                file("M", "first-only", None),
            ],
            vec![
                file("M", "a", None),
                file("M", "b", None),
                file("M", "second-only", None),
            ],
            vec![file("M", "b", None), file("M", "a", None)],
        ])
        .unwrap();
        assert_eq!(
            merged.iter().map(|f| f.label()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert!(
            merge_parents(&[vec![file("M", "a", None)], vec![]])
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn mixed_statuses_use_deletion_then_modification() {
        for (left, right, expected) in [("A", "M", "M"), ("A", "D", "D"), ("M", "T", "M")] {
            let result =
                merge_parents(&[vec![file(left, "a", None)], vec![file(right, "a", None)]])
                    .unwrap();
            assert_eq!(result[0].status, expected);
        }
    }
    #[test]
    fn rename_requires_matching_destinations_and_sources() {
        for (other, expected) in [
            (file("R80", "new", Some("old")), "R100"),
            (file("R100", "new", Some("different")), "M"),
            (file("M", "new", None), "M"),
            (file("D", "new", None), "D"),
        ] {
            let result =
                merge_parents(&[vec![file("R100", "new", Some("old"))], vec![other]]).unwrap();
            assert_eq!(result[0].status, expected);
            assert_eq!(result[0].old_path.is_some(), expected == "R100");
        }
        assert!(
            merge_parents(&[
                vec![file("R100", "new", Some("old"))],
                vec![file("M", "old", None)]
            ])
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn validates_statuses_and_empty_input() {
        assert!(merge_parents(&[]).unwrap().is_empty());
        assert!(merge_parents(&[vec![file("R100", "new", None)]]).is_err());
        assert!(merge_parents(&[vec![file("?", "a", None)]]).is_err());
        assert_eq!(
            merge_parents(&[vec![file("A", "a", None)]]).unwrap().len(),
            1
        );
    }
}
