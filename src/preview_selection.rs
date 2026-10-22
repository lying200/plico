//! Rebased Change.equals / ChangeWrapper: before+after paths and group,
//! intentionally independent of revision numbers. This is selection identity,
//! not the immutable identity used to cache or open a Diff Tab.
use crate::git::{Area, Change};
use std::path::Path;

fn paths(change: &Change) -> (Option<&Path>, Option<&Path>) {
    let added = change.area == Area::Untracked || change.status.starts_with('A');
    let deleted = change.status.starts_with('D');
    (
        (!added).then(|| change.old_path.as_deref().unwrap_or(&change.path)),
        (!deleted).then_some(change.path.as_path()),
    )
}

pub fn index(files: &[Change], preferred: Option<&Change>) -> Option<usize> {
    preferred
        .and_then(|old| {
            files
                .iter()
                .position(|file| file.area == old.area && paths(file) == paths(old))
        })
        .or_else(|| (!files.is_empty()).then_some(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(status: &str, path: &str, before: Option<&str>) -> Change {
        Change {
            area: Area::Commit,
            status: status.into(),
            path: path.into(),
            old_path: before.map(Into::into),
        }
    }
    #[test]
    fn retains_matching_change_but_not_same_name_with_other_endpoints() {
        let files = [
            file("M", "first", None),
            file("M", "wanted", None),
            file("R100", "new", Some("old")),
        ];
        assert_eq!(index(&files, Some(&file("T", "wanted", None))), Some(1));
        assert_eq!(
            index(&files, Some(&file("R80", "new", Some("old")))),
            Some(2)
        );
        for old in [
            file("A", "wanted", None),
            file("D", "wanted", None),
            file("R100", "new", Some("other")),
        ] {
            assert_eq!(index(&files, Some(&old)), Some(0));
        }
        let mut other_group = file("M", "wanted", None);
        other_group.area = Area::Staged;
        assert_eq!(index(&files, Some(&other_group)), Some(0));
        assert_eq!(index(&files, None), Some(0));
        assert_eq!(index(&[], Some(&files[1])), None);
    }
}
