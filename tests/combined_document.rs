#![cfg(feature = "desktop")]
use gpui_kit::component::diff::{DiffFile, DiffFileStatus, DiffSide};

#[test]
fn complete_sources_preserve_endings_paths_and_file_existence() {
    let old = "unchanged\r\nold\r\nremoved\r\ntail";
    let new = "unchanged\r\nnew\r\ntail\r\n";
    let file = DiffFile::from_sources(Some(("old[1]", old)), Some(("new[2]", new))).unwrap();
    assert_eq!(file.status(), DiffFileStatus::Renamed);
    assert_eq!(file.source(DiffSide::Original), old);
    assert_eq!(file.source(DiffSide::Modified), new);
    assert_eq!((file.additions(), file.deletions()), (2, 3));
    let same = DiffFile::from_sources(Some(("old", old)), Some(("new", old))).unwrap();
    assert_eq!(same.status(), DiffFileStatus::Renamed);
    assert_eq!((same.additions(), same.deletions()), (0, 0));
    assert_eq!(same.source(DiffSide::Original), old);
    for (before, after, status) in [
        (None, Some(("empty", "")), DiffFileStatus::Added),
        (Some(("empty", "")), None, DiffFileStatus::Deleted),
    ] {
        let file = DiffFile::from_sources(before, after).unwrap();
        assert_eq!(file.status(), status);
        assert_eq!(file.original_path().is_some(), before.is_some());
        assert_eq!(file.modified_path().is_some(), after.is_some());
    }
    assert!(DiffFile::from_sources(None, None).is_err());
}
