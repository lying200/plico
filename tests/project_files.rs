use plico::{git::Cancel, project_files::{Content, Error, Kind, Project, FILE_LIMIT, DIRECTORY_LIMIT}};
use std::{fs, path::{Path, PathBuf}, sync::atomic::{AtomicUsize, Ordering}};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("gitui-project-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn project(&self) -> Project { Project::new(&self.0).unwrap() }
    fn write(&self, path: &str, bytes: &[u8]) { fs::write(self.0.join(path), bytes).unwrap(); }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[test]
fn lazy_tree_includes_untracked_files_and_hides_git_administration() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("src")).unwrap();
    fs::create_dir(f.0.join(".git")).unwrap();
    f.write("src/deep.rs", b"not part of root listing");
    for name in ["item10.txt", "item2.txt", "未跟踪.txt", ".gitignore"] { f.write(name, b""); }
    let project = f.project();
    let children = project.children(Path::new(""), &Cancel::default()).unwrap();
    assert_eq!(children.len(), 5);
    assert_eq!(children[0].path, Path::new("src"));
    assert_eq!(children[0].kind, Kind::Directory);
    let files: Vec<_> = children.iter().filter(|e| e.kind == Kind::File).map(|e| &e.path).collect();
    assert!(files.contains(&&PathBuf::from("未跟踪.txt")));
    let two = files.iter().position(|p| p.as_path() == Path::new("item2.txt")).unwrap();
    let ten = files.iter().position(|p| p.as_path() == Path::new("item10.txt")).unwrap();
    assert!(two < ten);
    let nested = project.children(Path::new("src"), &Cancel::default()).unwrap();
    assert_eq!(nested[0].path, Path::new("src/deep.rs"));
}

#[test]
fn text_read_preserves_content_and_detects_changed_or_deleted_paths_on_reopen() {
    let f = Fixture::new();
    f.write("审阅.txt", b"\xef\xbb\xbfhello\r\nworld");
    let project = f.project();
    assert_eq!(project.read(Path::new("审阅.txt"), &Cancel::default()).unwrap(), Content::Text("hello\r\nworld".into()));
    f.write("审阅.txt", "新的内容\n".as_bytes());
    assert_eq!(project.read(Path::new("审阅.txt"), &Cancel::default()).unwrap(), Content::Text("新的内容\n".into()));
    fs::remove_file(f.0.join("审阅.txt")).unwrap();
    assert!(matches!(project.read(Path::new("审阅.txt"), &Cancel::default()), Err(Error::Io(_))));
    f.write("empty", b"");
    assert_eq!(project.read(Path::new("empty"), &Cancel::default()).unwrap(), Content::Text(String::new()));
}

#[test]
fn non_text_and_large_files_are_explicit_without_lossy_preview() {
    let f = Fixture::new();
    f.write("binary", &[1, 0, 2]);
    f.write("legacy", &[0x80, 0xff]);
    f.write("utf16", &[0xff, 0xfe, 65, 0]);
    let file = fs::File::create(f.0.join("large")).unwrap();
    file.set_len(FILE_LIMIT as u64 + 1).unwrap();
    let project = f.project();
    assert_eq!(project.read(Path::new("binary"), &Cancel::default()).unwrap(), Content::Binary);
    assert_eq!(project.read(Path::new("legacy"), &Cancel::default()).unwrap(), Content::UnsupportedEncoding);
    assert_eq!(project.read(Path::new("utf16"), &Cancel::default()).unwrap(), Content::UnsupportedEncoding);
    assert!(matches!(project.read(Path::new("large"), &Cancel::default()), Err(Error::FileTooLarge)));
    assert!(matches!(project.read(Path::new(""), &Cancel::default()), Err(Error::NotFile)));
}

#[test]
fn traversal_git_metadata_and_cancelled_requests_do_not_read() {
    let f = Fixture::new();
    f.write("file", b"text");
    f.write(".git", b"gitdir: external administration");
    let project = f.project();
    for path in [Path::new("../outside"), Path::new(".git"), f.0.as_path()] {
        assert!(matches!(project.read(path, &Cancel::default()), Err(Error::InvalidPath)));
    }
    assert_eq!(project.children(Path::new(""), &Cancel::default()).unwrap().len(), 1);
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(matches!(project.children(Path::new(""), &cancelled), Err(Error::Cancelled)));
    assert!(matches!(project.read(Path::new("file"), &cancelled), Err(Error::Cancelled)));
}

#[test]
fn oversized_directory_does_not_return_a_silently_partial_listing() {
    let f = Fixture::new();
    for i in 0..=DIRECTORY_LIMIT { f.write(&format!("file{i}"), b""); }
    assert!(matches!(f.project().children(Path::new(""), &Cancel::default()), Err(Error::DirectoryTooLarge)));
}

#[test]
fn linked_directory_is_visible_but_not_followed() {
    let f = Fixture::new();
    let outside = Fixture::new();
    outside.write("outside.txt", b"outside");
    let link = f.0.join("linked");
    #[cfg(unix)] std::os::unix::fs::symlink(&outside.0, &link).unwrap();
    #[cfg(windows)] {
        let status = std::process::Command::new("cmd.exe").current_dir(&f.0).args(["/d", "/c", "mklink", "/J"])
            .arg(&link).arg(&outside.0).status().unwrap();
        assert!(status.success());
    }
    let project = f.project();
    let entries = project.children(Path::new(""), &Cancel::default()).unwrap();
    assert_eq!(entries[0].kind, Kind::Link);
    assert!(matches!(project.children(Path::new("linked"), &Cancel::default()), Err(Error::LinkedPath)));
    assert!(matches!(project.read(Path::new("linked/outside.txt"), &Cancel::default()), Err(Error::LinkedPath)));
    #[cfg(windows)] fs::remove_dir(link).unwrap();
}

#[test]
fn paged_directory_preserves_natural_order_without_dropping_entries() {
    use plico::project_files::PAGE_LIMIT;
    let f = Fixture::new();
    fs::create_dir(f.0.join(".git")).unwrap();
    for name in ["dir10", "dir2"] { fs::create_dir(f.0.join(name)).unwrap(); }
    for i in (0..DIRECTORY_LIMIT + 17).rev() { f.write(&format!("文件{i}.txt"), b""); }
    let project = f.project(); let cancel = Cancel::default();
    let mut cursor = None; let mut names = Vec::new(); let mut pages = 0;
    loop {
        let page = project.children_page(Path::new(""), cursor.as_ref(), &cancel).unwrap();
        assert!(!page.entries.is_empty()); assert!(page.entries.len() <= PAGE_LIMIT);
        names.extend(page.entries.into_iter().map(|e| e.path)); pages += 1;
        cursor = page.next;
        if cursor.is_none() { break; }
    }
    let mut expected = vec![PathBuf::from("dir2"), PathBuf::from("dir10")];
    expected.extend((0..DIRECTORY_LIMIT + 17).map(|i| PathBuf::from(format!("文件{i}.txt"))));
    assert_eq!(names, expected); assert!(pages > 16);
}

#[test]
fn directory_cursor_rejects_other_roots_changes_and_cancel() {
    let f = Fixture::new(); let other = Fixture::new();
    for i in 0..257 { f.write(&format!("file{i}"), b""); }
    fs::create_dir(f.0.join("sub")).unwrap();
    let project = f.project(); let cancel = Cancel::default();
    let first = project.children_page(Path::new(""), None, &cancel).unwrap();
    let cursor = first.next.unwrap();
    assert!(matches!(other.project().children_page(Path::new(""), Some(&cursor), &cancel), Err(Error::InvalidPath)));
    assert!(matches!(project.children_page(Path::new("sub"), Some(&cursor), &cancel), Err(Error::InvalidPath)));
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(matches!(project.children_page(Path::new(""), Some(&cursor), &cancelled), Err(Error::Cancelled)));
    // Set an explicit timestamp so the stale-cursor check does not depend on
    // the speed of this fixture or the filesystem's timestamp granularity.
    let mut options = fs::OpenOptions::new(); options.read(true);
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.access_mode(0x100).custom_flags(0x02000000); // FILE_WRITE_ATTRIBUTES, BACKUP_SEMANTICS
    }
    options.open(&f.0).unwrap().set_times(fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1000))).unwrap();
    assert!(matches!(project.children_page(Path::new(""), Some(&cursor), &cancel), Err(Error::ChangedDuringRead)));
    assert!(project.children_page(Path::new(""), None, &cancel).is_ok());
    let empty = other.project().children_page(Path::new(""), None, &cancel).unwrap();
    assert!(empty.entries.is_empty()); assert!(empty.next.is_none());
}

#[test]
fn refresh_restores_expanded_paths_and_loaded_boundary_after_insertions() {
    use plico::project_files::{ExpansionState, DISPLAY_NODE_LIMIT};
    let f = Fixture::new();
    fs::create_dir_all(f.0.join("src/nested")).unwrap();
    fs::create_dir_all(f.0.join("closed")).unwrap();
    f.write("closed/not-loaded", b"hidden subtree");
    for i in 0..300 { f.write(&format!("src/item{i:04}.txt"), b"x"); }
    f.write("src/nested/中文.txt", b"nested");
    let p = f.project(); let cancel = Cancel::default();
    let mut expanded = ExpansionState::new();
    for path in ["", "src", "src/nested"] {
        let page = p.children_page(Path::new(path), None, &cancel).unwrap();
        expanded.insert(PathBuf::from(path), page.entries.last().cloned());
    }
    let old_boundary = expanded[Path::new("src")].clone().unwrap();
    f.write("src/aaa-new.txt", b"added before previous boundary");
    let refreshed = p.refresh_expanded(&expanded, DISPLAY_NODE_LIMIT, &cancel).unwrap();
    assert!(refreshed.error.is_none()); assert!(!refreshed.limited);
    assert!(!refreshed.directories.contains_key(Path::new("closed")));
    assert!(refreshed.directories[Path::new("src")].contains(&old_boundary));
    assert!(refreshed.directories[Path::new("src")].iter().any(|e| e.path == Path::new("src/aaa-new.txt")));
    assert_eq!(refreshed.directories[Path::new("src/nested")][0].path, Path::new("src/nested/中文.txt"));
    // Replacing an expanded directory with a file must discard its old descendants.
    fs::remove_dir_all(f.0.join("src/nested")).unwrap(); f.write("src/nested", b"now a file");
    let refreshed = p.refresh_expanded(&expanded, DISPLAY_NODE_LIMIT, &cancel).unwrap();
    assert!(!refreshed.directories.contains_key(Path::new("src/nested")));
    // The replacement now sorts after the old loaded boundary as a file;
    // it belongs to the next page, rather than forcing an unbounded scan.
    let next = p.children_page(Path::new("src"), refreshed.cursors.get(Path::new("src")), &cancel).unwrap();
    assert!(next.entries.iter().any(|e| e.path == Path::new("src/nested") && e.kind == Kind::File));
}

#[test]
fn refresh_budget_preserves_continuation_without_skipping_files() {
    use plico::project_files::{ExpansionState, DISPLAY_NODE_LIMIT};
    let f = Fixture::new();
    for i in 0..600 { f.write(&format!("file{i:04}"), b""); }
    let p = f.project(); let cancel = Cancel::default();
    let all = p.children(Path::new(""), &cancel).unwrap();
    let expanded = ExpansionState::from([(PathBuf::new(), all.last().cloned())]);
    let tree = p.refresh_expanded(&expanded, 300, &cancel).unwrap();
    assert!(tree.limited); assert!(tree.error.is_none());
    let first = &tree.directories[Path::new("")];
    assert_eq!(first.len(), 256);
    let next = p.children_page(Path::new(""), tree.cursors.get(Path::new("")), &cancel).unwrap();
    assert_eq!(first.last().unwrap().path, Path::new("file0255"));
    assert_eq!(next.entries.first().unwrap().path, Path::new("file0256"));
    let full = p.refresh_expanded(&expanded, DISPLAY_NODE_LIMIT, &cancel).unwrap();
    assert_eq!(full.directories[Path::new("")], all);
    assert!(full.cursors.is_empty()); assert!(!full.limited);
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(matches!(p.refresh_expanded(&expanded, DISPLAY_NODE_LIMIT, &cancelled), Err(Error::Cancelled)));
    assert!(p.refresh_expanded(&ExpansionState::new(), DISPLAY_NODE_LIMIT, &cancel).unwrap().directories.is_empty());
}

#[test]
fn project_name_filters_are_distinct_from_git_ignore_and_dotfiles() {
    let f = Fixture::new();
    for name in [".git", ".hg", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".svn", "CVS", "__pycache__", "_svn"] {
        fs::create_dir(f.0.join(name)).unwrap();
        f.write(&format!("{name}/nested.txt"), b"not traversed by listing");
    }
    for name in [".DS_Store", "vssver.scc", "vssver2.scc", "backup~", "cache.pyc", "cache.PYO", "cache.rbc", "cache.YARB"] {
        f.write(name, b"hidden by default name rule");
    }
    let visible = [".env", ".gitignore", ".hidden", "ignored.log", "keep.py", "keep.pyc.txt", "cache.pycache", "untracked.txt"];
    for name in visible { f.write(name, b"reviewable"); }
    f.write(".gitignore", b"ignored.log\n");
    let project = f.project();
    let full = project.children(Path::new(""), &Cancel::default()).unwrap();
    let page = project.children_page(Path::new(""), None, &Cancel::default()).unwrap();
    assert_eq!(full, page.entries);
    assert!(page.next.is_none());
    assert_eq!(full.len(), visible.len());
    for name in visible { assert!(full.iter().any(|e| e.path == Path::new(name)), "{name}"); }
    // Presentation rules must not invalidate an already open file after renaming.
    assert_eq!(project.read(Path::new("cache.pyc"), &Cancel::default()).unwrap(), Content::Text("hidden by default name rule".into()));
    let expanded = std::collections::BTreeMap::from([(PathBuf::new(), full.last().cloned())]);
    assert_eq!(project.refresh_expanded(&expanded, 256, &Cancel::default()).unwrap().directories[Path::new("")], full);
}
