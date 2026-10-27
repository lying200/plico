use plico::{
    git::{Cancel, Repository},
    interactive_rebase::prepare::Prepared,
    published_commits::Protection,
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
            "plico-interactive-prepare-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Test"]);
        f.git(&["config", "user.email", "test@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.autocrlf", "false"]);
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f
    }
    fn command(&self) -> Command {
        let mut c = Command::new("git");
        c.arg("-C").arg(&self.0);
        c
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let out = self.command().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }
    fn write(&self, name: &str, content: &str) {
        fs::write(self.0.join(name), content).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Fixture {
    fn text(&self, args: &[&str]) -> String {
        String::from_utf8(self.git(args))
            .unwrap()
            .trim_end()
            .to_owned()
    }
    fn commit(&self, name: &str) -> String {
        self.git(&["add", "."]);
        self.git(&["commit", "--allow-empty", "-m", name]);
        self.text(&["rev-parse", "HEAD"])
    }
}

#[test]
fn root_detached_and_dirty_are_allowed_without_mutating_files() {
    let f = Fixture::new();
    f.write("a", "original");
    let root = f.commit("root");
    f.write("b", "child");
    let head = f.commit("child");
    f.git(&["checkout", "--detach", &head]);
    f.write("a", "staged");
    f.git(&["add", "a"]);
    f.write("a", "unstaged");
    let status = f.git(&["status", "--porcelain=v1"]);
    let tree = f.text(&["write-tree"]);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let p = Prepared::prepare(&repo, &root, Protection::Defaults, &token).unwrap();
    assert!(p.initial());
    assert_eq!(p.head(), head);
    p.validate(Protection::Defaults, &token).unwrap();
    let p = Prepared::prepare(&repo, &head, Protection::Defaults, &token).unwrap();
    assert_eq!(p.base(), Some(root.as_str()));
    assert_eq!(f.git(&["status", "--porcelain=v1"]), status);
    assert_eq!(f.text(&["write-tree"]), tree);
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "unstaged");
}
#[test]
fn stale_head_refs_or_operation_invalidate_prepared_gate() {
    let f = Fixture::new();
    let root = f.commit("root");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let p = Prepared::prepare(&repo, &root, Protection::Defaults, &token).unwrap();
    f.commit("new head");
    assert!(p.validate(Protection::Defaults, &token).is_err());
    let p = Prepared::prepare(&repo, &root, Protection::Defaults, &token).unwrap();
    f.git(&["branch", "new-branch"]);
    assert!(p.validate(Protection::Defaults, &token).is_err());
    let p = Prepared::prepare(&repo, &root, Protection::Defaults, &token).unwrap();
    fs::write(f.0.join(".git/CHERRY_PICK_HEAD"), format!("{root}\n")).unwrap();
    assert!(p.validate(Protection::Defaults, &token).is_err());
    assert!(Prepared::prepare(&repo, &root, Protection::Defaults, &token).is_err());
}
#[test]
fn protected_remote_and_unreachable_commit_are_rejected() {
    let f = Fixture::new();
    let root = f.commit("root");
    f.git(&["checkout", "-b", "other"]);
    let other = f.commit("other");
    f.git(&["checkout", "main"]);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    assert!(Prepared::prepare(&repo, &other, Protection::Defaults, &token).is_err());
    f.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/repo.git",
    ]);
    f.git(&["update-ref", "refs/remotes/origin/main", &root]);
    assert!(
        Prepared::prepare(&repo, &root, Protection::Defaults, &token)
            .unwrap_err()
            .contains("protected")
    );
    assert!(Prepared::prepare(&repo, &root, Protection::Explicit(&[]), &token).is_ok());
}
#[test]
fn selected_merge_is_disabled_but_merge_in_replay_range_is_not() {
    let f = Fixture::new();
    f.write("base", "base");
    let root = f.commit("root");
    f.git(&["checkout", "-b", "side"]);
    f.write("side", "side");
    f.commit("side");
    f.git(&["checkout", "main"]);
    f.write("main", "main");
    f.commit("main");
    f.git(&["merge", "--no-ff", "side", "-m", "merge"]);
    let head = f.text(&["rev-parse", "HEAD"]);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    assert!(
        Prepared::prepare(&repo, &head, Protection::Defaults, &token)
            .unwrap_err()
            .contains("merge commit")
    );
    assert!(Prepared::prepare(&repo, &root, Protection::Defaults, &token).is_ok());
}

#[test]
fn shallow_boundary_and_cancelled_preparation_fail_without_rewriting() {
    let f = Fixture::new();
    f.commit("root");
    let head = f.commit("child");
    let path = f.0.join("shallow-clone");
    let source = f.0.to_string_lossy().replace('\\', "/");
    let url = if source.starts_with('/') {
        format!("file://{source}")
    } else {
        format!("file:///{source}")
    };
    let out = Command::new("git")
        .args(["clone", "--depth=1", &url])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let repo = Repository::new(&path);
    assert!(
        Prepared::prepare(&repo, &head, Protection::Explicit(&[]), &Cancel::default())
            .unwrap_err()
            .contains("shallow boundary")
    );
    let token = Cancel::default();
    token.cancel();
    assert!(
        Prepared::prepare(&Repository::new(&f.0), &head, Protection::Defaults, &token).is_err()
    );
    assert_eq!(f.text(&["rev-parse", "HEAD"]), head);
}

#[test]
fn selection_details_preserve_requested_order_zip_changes_and_ignore_worktree() {
    use plico::interactive_rebase::details;
    let f = Fixture::new();
    f.write("a", "original\n");
    let first = f.commit("first");
    f.write("a", "modified\n");
    let second = f.commit("second");
    f.git(&["mv", "a", "renamed"]);
    let third = f.commit("third");
    f.write("renamed", "uncommitted\n");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let result = details::load(
        &repo,
        &[first.clone(), second.clone(), third.clone()],
        &token,
    )
    .unwrap();
    assert_eq!(
        result
            .cards
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec![first.as_str(), second.as_str(), third.as_str()]
    );
    assert_eq!(result.files.len(), 1);
    assert!(result.files[0].before.is_none());
    assert_eq!(
        result.files[0].after.as_ref().unwrap().path,
        PathBuf::from("renamed")
    );
    let contents = plico::combined_changes::read_contents(&repo, &result.files[0], &token).unwrap();
    assert_eq!(contents.after.as_deref(), Some("modified\n"));
    let reversed = details::load(&repo, &[third.clone(), first.clone()], &token).unwrap();
    assert_eq!(reversed.cards[0].id, third);
    assert_eq!(reversed.cards[1].id, first);
    assert!(details::load(&repo, &[second.clone(), second.clone()], &token).is_err());
    token.cancel();
    assert!(details::load(&repo, &[second], &token).is_err());
    assert_eq!(
        fs::read_to_string(f.0.join("renamed")).unwrap(),
        "uncommitted\n"
    );
}
