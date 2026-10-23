use plico::{
    git::{Cancel, Repository},
    rollback_operation as rollback,
};
use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut random = [0; 8];
        getrandom::fill(&mut random).unwrap();
        let p = std::env::temp_dir().join(format!("plico-rollback-{}", u64::from_le_bytes(random)));
        fs::create_dir_all(&p).unwrap();
        let f = Self(p);
        f.git(&["init", "-q"]);
        f.git(&["config", "user.name", "Fixture"]);
        f.git(&["config", "user.email", "fixture@example.test"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.autocrlf", "false"]);
        f
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&o.stderr)
        );
        o.stdout
    }
    fn write(&self, p: &str, value: &str) {
        let p = self.0.join(p);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, value).unwrap();
    }
    fn base(&self) {
        for p in ["a", "b", "delete", "old"] {
            self.write(p, "base\n");
        }
        self.git(&["add", "."]);
        self.git(&["commit", "-qm", "base"]);
    }
    fn repo(&self) -> Repository {
        Repository::new(&self.0)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn selected(p: &rollback::Prepared, names: &[&str]) -> BTreeSet<usize> {
    p.changes
        .iter()
        .enumerate()
        .filter(|(_, c)| names.iter().any(|n| c.path == PathBuf::from(n)))
        .map(|(i, _)| i)
        .collect()
}
#[test]
fn tracked_rollback_restores_head_and_index_without_touching_unselected_or_untracked() {
    let f = Fixture::new();
    f.base();
    f.write("a", "staged\n");
    f.git(&["add", "a"]);
    f.write("a", "working\n");
    f.write("b", "keep staged\n");
    f.git(&["add", "b"]);
    f.write("b", "keep draft\n");
    fs::remove_file(f.0.join("delete")).unwrap();
    f.write("untracked", "keep\n");
    let p = rollback::prepare(&f.repo(), &Cancel::default()).unwrap();
    let choice = selected(&p, &["a", "delete"]);
    assert_eq!(choice.len(), 2);
    rollback::execute(&f.repo(), &p, &choice, false).unwrap();
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"base\n");
    assert_eq!(f.git(&["show", ":a"]), b"base\n");
    assert_eq!(fs::read(f.0.join("delete")).unwrap(), b"base\n");
    assert_eq!(f.git(&["show", ":b"]), b"keep staged\n");
    assert_eq!(fs::read(f.0.join("b")).unwrap(), b"keep draft\n");
    assert!(f.0.join("untracked").exists());
}
#[test]
fn additions_default_to_unversioned_and_delete_is_explicit_including_unborn() {
    for unborn in [false, true] {
        for delete in [false, true] {
            let f = Fixture::new();
            if !unborn {
                f.base();
            }
            f.write("added", "mine\n");
            f.git(&["add", "added"]);
            let p = rollback::prepare(&f.repo(), &Cancel::default()).unwrap();
            rollback::execute(&f.repo(), &p, &selected(&p, &["added"]), delete).unwrap();
            assert!(f.git(&["ls-files", "--", "added"]).is_empty());
            assert_eq!(f.0.join("added").exists(), !delete);
        }
    }
}
#[test]
fn moved_file_reverts_both_endpoints_and_literal_names() {
    let f = Fixture::new();
    f.base();
    f.git(&["mv", "old", "new [1].txt"]);
    let p = rollback::prepare(&f.repo(), &Cancel::default()).unwrap();
    rollback::execute(&f.repo(), &p, &selected(&p, &["new [1].txt"]), false).unwrap();
    assert!(!f.0.join("new [1].txt").exists());
    assert_eq!(fs::read(f.0.join("old")).unwrap(), b"base\n");
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}
#[test]
fn cancelling_dialog_is_read_only_and_late_edits_or_index_or_head_changes_reject() {
    for mode in ["cancel", "file", "index", "head"] {
        let f = Fixture::new();
        f.base();
        f.write("a", "before\n");
        let p = rollback::prepare(&f.repo(), &Cancel::default()).unwrap();
        let index = f.git(&["write-tree"]);
        match mode {
            "cancel" => {
                drop(p);
                assert_eq!(f.git(&["write-tree"]), index);
                assert_eq!(fs::read(f.0.join("a")).unwrap(), b"before\n");
                continue;
            }
            "file" => f.write("a", "after\n"),
            "index" => {
                f.git(&["add", "a"]);
            }
            _ => {
                f.git(&["commit", "--allow-empty", "-qm", "new HEAD"]);
            }
        }
        assert!(rollback::execute(&f.repo(), &p, &selected(&p, &["a"]), false).is_err());
        assert_eq!(
            fs::read(f.0.join("a")).unwrap(),
            if mode == "file" {
                b"after\n".as_slice()
            } else {
                b"before\n"
            }
        );
    }
}
#[test]
fn ignored_listing_is_opt_in_metadata_and_does_not_enter_rollback() {
    let f = Fixture::new();
    f.base();
    f.write(".gitignore", "build/\n*.cache\n");
    f.git(&["add", ".gitignore"]);
    f.git(&["commit", "-qm", "ignore"]);
    f.write("build/large/nested/file", "ignored");
    f.write("x.cache", "ignored");
    f.write("loose", "untracked");
    let list = plico::local_changes::ignored(&f.repo(), &Cancel::default()).unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|c| c.area == plico::git::Area::Ignored));
    let p = rollback::prepare(&f.repo(), &Cancel::default()).unwrap();
    assert!(p.changes.is_empty());
    assert!(rollback::execute(&f.repo(), &p, &BTreeSet::new(), true).is_err());
    assert!(f.0.join("loose").exists());
}
#[test]
fn index_lock_failure_does_not_delete_added_files() {
    let f = Fixture::new();
    f.base();
    f.write("added", "keep");
    f.git(&["add", "added"]);
    let p = rollback::prepare(&f.repo(), &Cancel::default()).unwrap();
    f.write(".git/index.lock", "test owned");
    assert!(rollback::execute(&f.repo(), &p, &selected(&p, &["added"]), true).is_err());
    assert!(f.0.join("added").exists());
}
#[cfg(unix)]
#[test]
fn symlink_target_is_not_modified_and_symlink_parent_is_rejected() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.base();
    f.write("target", "outside\n");
    symlink("target", f.0.join("link")).unwrap();
    f.git(&["add", "link"]);
    let p = rollback::prepare(&f.repo(), &Cancel::default()).unwrap();
    rollback::execute(&f.repo(), &p, &selected(&p, &["link"]), true).unwrap();
    assert_eq!(fs::read(f.0.join("target")).unwrap(), b"outside\n");
    f.write("dir/file", "base");
    f.git(&["add", "dir/file"]);
    f.git(&["commit", "-qm", "dir"]);
    fs::remove_dir_all(f.0.join("dir")).unwrap();
    symlink(".", f.0.join("dir")).unwrap();
    let p = rollback::prepare(&f.repo(), &Cancel::default()).unwrap();
    assert!(rollback::execute(&f.repo(), &p, &selected(&p, &["dir/file"]), false).is_err());
}

#[test]
fn unrelated_gitlink_does_not_block_ordinary_file_rollback() {
    let f = Fixture::new();
    f.base();
    f.write("nested/file", "nested base\n");
    f.git(&["-C", "nested", "init", "-q"]);
    f.git(&["-C", "nested", "config", "user.name", "Fixture"]);
    f.git(&[
        "-C",
        "nested",
        "config",
        "user.email",
        "fixture@example.test",
    ]);
    f.git(&["-C", "nested", "config", "commit.gpgsign", "false"]);
    f.git(&["-C", "nested", "add", "file"]);
    f.git(&["-C", "nested", "commit", "-qm", "nested base"]);
    f.git(&["add", "nested"]);
    f.git(&["commit", "-qm", "gitlink"]);
    f.write("nested/file", "nested changed\n");
    f.git(&["-C", "nested", "commit", "-qam", "nested change"]);
    let nested_head = f.git(&["-C", "nested", "rev-parse", "HEAD"]);
    f.write("a", "ordinary change\n");
    let prepared = rollback::prepare(&f.repo(), &Cancel::default())
        .expect("an unrelated gitlink must not block the rollback dialog");
    let choice = selected(&prepared, &["a"]);
    assert_eq!(choice.len(), 1);
    let all = (0..prepared.changes.len()).collect();
    assert!(rollback::execute(&f.repo(), &prepared, &all, false).is_err());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"ordinary change\n");
    rollback::execute(&f.repo(), &prepared, &choice, false).unwrap();
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"base\n");
    assert_eq!(f.git(&["-C", "nested", "rev-parse", "HEAD"]), nested_head);
    assert_eq!(
        fs::read(f.0.join("nested/file")).unwrap(),
        b"nested changed\n"
    );
}
