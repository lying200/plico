use plico::{
    git::{Cancel, Repository},
    line_changes::{Kind, base_oid, base_text, blocks},
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "plico-markers-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-q", "-b", "main"]);
        f.git(&["config", "user.name", "Fixture"]);
        f.git(&["config", "user.email", "fixture@example.invalid"]);
        f.git(&["config", "core.autocrlf", "false"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }
    fn write(&self, text: &str) {
        fs::write(self.0.join("sample.txt"), text).unwrap();
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
#[test]
fn baseline_is_head_not_last_save_or_index() {
    let f = Fixture::new();
    f.write("a\r\nold\r\nz\r\n");
    f.git(&["add", "sample.txt"]);
    f.git(&["commit", "-qm", "base"]);
    let stop = Cancel::default();
    let path = std::path::Path::new("sample.txt");
    let oid = base_oid(&f.repo(), path, &stop).unwrap().unwrap();
    assert_eq!(base_text(&f.repo(), &oid, &stop).unwrap(), "a\nold\nz\n");
    f.write("a\nnew\nz\n");
    f.git(&["add", "sample.txt"]);
    let index = f.git(&["write-tree"]);
    f.write("a\nnewer\nz\n");
    assert_eq!(base_oid(&f.repo(), path, &stop).unwrap(), Some(oid.clone()));
    assert_eq!(
        blocks(&base_text(&f.repo(), &oid, &stop).unwrap(), "a\nnewer\nz\n")[0].kind(),
        Kind::Modified
    );
    assert_eq!(index, f.git(&["write-tree"]));
    f.git(&["commit", "-qm", "next"]);
    assert_ne!(base_oid(&f.repo(), path, &stop).unwrap(), Some(oid));
}
#[test]
fn new_files_and_conflicts_do_not_acquire_empty_rollback_baselines() {
    let f = Fixture::new();
    let stop = Cancel::default();
    let path = std::path::Path::new("sample.txt");
    f.write("base\n");
    assert!(base_oid(&f.repo(), path, &stop).unwrap().is_none());
    f.git(&["add", "sample.txt"]);
    f.git(&["commit", "-qm", "base"]);
    f.git(&["checkout", "-qb", "side"]);
    f.write("side\n");
    f.git(&["commit", "-qam", "side"]);
    f.git(&["checkout", "-q", "main"]);
    f.write("main\n");
    f.git(&["commit", "-qam", "main"]);
    let out = Command::new("git")
        .arg("-C")
        .arg(&f.0)
        .args(["merge", "side"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(base_oid(&f.repo(), path, &stop).unwrap().is_none());
}
#[test]
fn invalid_paths_and_cancelled_reads_are_errors() {
    let f = Fixture::new();
    let stop = Cancel::default();
    assert!(base_oid(&f.repo(), std::path::Path::new("../escape"), &stop).is_err());
    stop.cancel();
    assert!(base_oid(&f.repo(), std::path::Path::new("sample.txt"), &stop).is_err());
}
