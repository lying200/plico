#![cfg(feature = "desktop")]
use gpui_kit::component::diff::{DiffFile, DiffFileStatus};

#[test]
fn untracked_preview_counts_added_lines_and_handles_empty_files() {
    let text = "中文\r\nsecond\nlast without newline";
    let file = DiffFile::added("src/中文 [x].rs", text);
    assert_eq!(file.status(), DiffFileStatus::Added);
    assert!(file.has_changes());
    assert!(file.original_path().is_none());
    assert_eq!(file.path().as_str(), "src/中文 [x].rs");
    assert_eq!(file.additions(), 3);
    assert_eq!(file.deletions(), 0);
    let empty = DiffFile::added("empty.txt", "");
    assert_eq!(empty.status(), DiffFileStatus::Added);
    assert!(empty.has_changes());
}

#[test]
fn mode_only_patch_is_a_change_even_without_added_or_deleted_lines() {
    let files = DiffFile::parse("diff --git a/run.sh b/run.sh\nold mode 100644\nnew mode 100755\n")
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].has_changes());
    assert_eq!(files[0].additions(), 0);
    assert_eq!(files[0].deletions(), 0);
    assert!(
        files[0]
            .extended_headers()
            .iter()
            .any(|h| h.as_str() == "new mode 100755")
    );
}
