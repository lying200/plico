use plico::{
    branch_create,
    git::{Cancel, Repository},
    reflog,
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let f = Self(std::env::temp_dir().join(format!(
            "plico-reflog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&f.0).unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.hooksPath", "disabled-hooks"],
        ] {
            f.git(&args, true);
        }
        f
    }
    fn git(&self, args: &[&str], ok: bool) -> String {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            o.status.success(),
            ok,
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8(o.stdout).unwrap().trim().to_owned()
    }
    fn commit(&self, value: &str) {
        fs::write(self.0.join("a"), value).unwrap();
        self.git(&["add", "a"], true);
        self.git(&["commit", "-m", value], true);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reset_commit_is_recovered_by_oid_without_changing_head_or_index() {
    let f = Fixture::new();
    f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"], true);
    f.commit("lost");
    let lost = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["reset", "--hard", &base], true);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let page = reflog::read(&repo, 200, &token).unwrap();
    let entry = page
        .entries
        .iter()
        .find(|e| e.commit == lost && e.reference == "HEAD")
        .unwrap();
    assert!(entry.timestamp > 0);
    assert!(entry.action.contains("lost"));
    // Move the ordinal, then recover the immutable ID selected earlier.
    f.commit("later");
    let head = f.git(&["rev-parse", "HEAD"], true);
    fs::write(f.0.join("a"), "staged").unwrap();
    f.git(&["add", "a"], true);
    fs::write(f.0.join("a"), "unstaged").unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let prepared =
        branch_create::Prepared::read(&repo, "recovered", &entry.commit, false, &token).unwrap();
    let result = branch_create::execute(&repo, &prepared, &token, &Default::default()).unwrap();
    assert!(result.verified);
    assert_eq!(f.git(&["rev-parse", "recovered"], true), lost);
    assert_eq!(f.git(&["rev-parse", "HEAD"], true), head);
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "unstaged");
}
#[test]
fn bounded_prefix_retains_events_and_reports_more() {
    let f = Fixture::new();
    f.commit("one");
    f.commit("two");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let one = reflog::read(&repo, 1, &token).unwrap();
    assert_eq!(one.entries.len(), 1);
    assert!(one.has_more);
    let all = reflog::read(&repo, 100, &token).unwrap();
    assert!(!all.has_more);
    assert!(all.entries.iter().any(|e| e.reference == "HEAD"));
    assert!(all.entries.iter().any(|e| e.reference == "refs/heads/main"));
    assert_eq!(one.entries[0], all.entries[0]);
    f.git(&["config", "log.date", "relative"], true);
    assert_eq!(
        reflog::read(&repo, 100, &token).unwrap().entries,
        all.entries
    );
    token.cancel();
    assert!(reflog::read(&repo, 100, &token).is_err());
}
#[test]
fn unborn_repository_and_invalid_limits() {
    let f = Fixture::new();
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    assert!(reflog::read(&repo, 100, &token).unwrap().entries.is_empty());
    assert!(reflog::read(&repo, 0, &token).is_err());
    assert!(reflog::read(&repo, reflog::MAX_ENTRIES + 1, &token).is_err());
}
