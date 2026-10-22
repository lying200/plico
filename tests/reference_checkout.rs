use plico::{
    git::{Cancel, Repository},
    reference_checkout::{self, Choice, Prepared},
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
        let f = Self(std::env::temp_dir().join(format!(
            "plico-ref-checkout-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&f.0).unwrap();
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Test"]);
        f.git(&["config", "user.email", "test@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f.commit("base", "base");
        f.git(&["remote", "add", "origin", "unused-local-fixture.git"]);
        f.git(&["update-ref", "refs/remotes/origin/topic", &f.head()]);
        f
    }
    fn git(&self, args: &[&str]) -> String {
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
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }
    fn commit(&self, file: &str, text: &str) {
        fs::write(self.0.join(file), text).unwrap();
        self.git(&["add", file]);
        self.git(&["commit", "-m", text]);
    }
    fn plan(&self, name: Option<&str>) -> Prepared {
        Prepared::read(
            &Repository::new(&self.0),
            "refs/remotes/origin/topic",
            &self.git(&["rev-parse", "origin/topic"]),
            &self.head(),
            name,
            &Cancel::default(),
        )
        .unwrap()
    }
    fn run(&self, p: &Prepared, c: Choice) -> reference_checkout::Report {
        reference_checkout::execute(
            &Repository::new(&self.0),
            p,
            c,
            &Cancel::default(),
            &Default::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn remote_checkout_attaches_and_tracks() {
    let f = Fixture::new();
    let p = f.plan(None);
    let r = f.run(&p, Choice::Checkout);
    assert!(r.command.status.success());
    assert!(r.verified);
    assert_eq!(r.symbolic_head, "refs/heads/topic");
    assert_eq!(
        f.git(&["rev-parse", "--symbolic-full-name", "@{upstream}"]),
        p.reference
    );
}
#[test]
fn existing_branch_fast_forwards_without_losing_tracking() {
    let f = Fixture::new();
    f.git(&["branch", "topic"]);
    f.commit("remote", "remote");
    f.git(&["update-ref", "refs/remotes/origin/topic", &f.head()]);
    let p = f.plan(None);
    assert!(!p.local_ahead);
    let r = f.run(&p, Choice::Checkout);
    assert!(r.command.status.success());
    assert!(r.verified);
    assert_eq!(r.head, p.target);
    assert_eq!(
        f.git(&["rev-parse", "--symbolic-full-name", "@{upstream}"]),
        p.reference
    );
}
#[test]
fn local_only_commits_require_explicit_choice() {
    let f = Fixture::new();
    f.git(&["checkout", "-b", "topic"]);
    f.commit("local", "local");
    f.git(&["checkout", "main"]);
    let p = f.plan(None);
    assert!(p.local_ahead);
    assert!(
        reference_checkout::execute(
            &Repository::new(&f.0),
            &p,
            Choice::Checkout,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(f.head(), p.head);
    let r = f.run(&p, Choice::Reset);
    assert!(r.command.status.success());
    assert!(r.verified);
    assert_eq!(r.head, p.target);
}
#[test]
fn rebase_preserves_local_commit_and_attaches() {
    let f = Fixture::new();
    f.git(&["checkout", "-b", "topic"]);
    f.commit("local", "local");
    f.git(&["checkout", "main"]);
    f.commit("remote", "remote");
    f.git(&["update-ref", "refs/remotes/origin/topic", &f.head()]);
    let p = f.plan(None);
    let r = f.run(&p, Choice::Rebase);
    assert!(r.command.status.success(), "{:?}", r.command.stderr.bytes());
    assert_eq!(r.symbolic_head, "refs/heads/topic");
    assert_eq!(
        f.git(&["rev-list", "--count", &format!("{}..HEAD", p.target)]),
        "1"
    );
    assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "local");
}
#[test]
fn tracking_conflict_needs_unused_name() {
    let f = Fixture::new();
    f.git(&["branch", "topic"]);
    f.git(&["config", "branch.topic.remote", "origin"]);
    f.git(&["config", "branch.topic.merge", "refs/heads/other"]);
    assert!(f.plan(None).naming_conflict);
    assert!(f.plan(Some("main")).naming_conflict);
    let p = f.plan(Some("another"));
    assert!(!p.naming_conflict);
    let r = f.run(&p, Choice::Checkout);
    assert!(r.command.status.success());
    assert!(r.verified);
    assert_eq!(r.symbolic_head, "refs/heads/another");
}
#[test]
fn changed_ref_or_symbolic_head_rejects_stale_plan() {
    let f = Fixture::new();
    let p = f.plan(None);
    f.git(&["checkout", "-b", "other"]);
    assert!(
        reference_checkout::execute(
            &Repository::new(&f.0),
            &p,
            Choice::Checkout,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    f.git(&["checkout", "main"]);
    f.commit("new", "new");
    f.git(&["update-ref", "refs/remotes/origin/topic", &f.head()]);
    f.git(&["reset", "--hard", &p.head]);
    assert!(
        reference_checkout::execute(
            &Repository::new(&f.0),
            &p,
            Choice::Checkout,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
}
#[test]
fn tags_checkout_detached_and_revalidate_tag_object() {
    let f = Fixture::new();
    f.git(&["tag", "-a", "release", "-m", "release"]);
    let repo = Repository::new(&f.0);
    let p = Prepared::read(
        &repo,
        "refs/tags/release",
        &f.head(),
        &f.head(),
        None,
        &Cancel::default(),
    )
    .unwrap();
    let r = f.run(&p, Choice::Checkout);
    assert!(r.command.status.success());
    assert!(r.verified);
    assert!(r.state.detached);
    f.git(&["tag", "-f", "-a", "release", "-m", "replaced"]);
    assert!(
        reference_checkout::execute(
            &repo,
            &p,
            Choice::Checkout,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
}
#[test]
fn dirty_file_block_does_not_create_local_branch() {
    let f = Fixture::new();
    let base = f.head();
    f.commit("base", "remote change");
    f.git(&["update-ref", "refs/remotes/origin/topic", &f.head()]);
    f.git(&["reset", "--hard", &base]);
    fs::write(f.0.join("base"), "my draft").unwrap();
    let p = f.plan(None);
    let r = f.run(&p, Choice::Checkout);
    assert!(!r.command.status.success());
    assert_eq!(r.symbolic_head, "refs/heads/main");
    assert_eq!(fs::read_to_string(f.0.join("base")).unwrap(), "my draft");
    assert!(f.git(&["for-each-ref", "refs/heads/topic"]).is_empty());
}
#[test]
fn cancelled_prepare_and_execute_leave_repository_unchanged() {
    let f = Fixture::new();
    let p = f.plan(None);
    let token = Cancel::default();
    token.cancel();
    assert!(
        Prepared::read(
            &Repository::new(&f.0),
            &p.reference,
            &p.target,
            &p.head,
            None,
            &token
        )
        .is_err()
    );
    assert!(
        reference_checkout::execute(
            &Repository::new(&f.0),
            &p,
            Choice::Checkout,
            &token,
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(f.head(), p.head);
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert!(f.git(&["for-each-ref", "refs/heads/topic"]).is_empty());
}
#[test]
fn changed_local_tip_or_upstream_rejects_confirmation() {
    let f = Fixture::new();
    f.git(&["branch", "topic"]);
    let p = f.plan(None);
    f.git(&["config", "branch.topic.remote", "origin"]);
    f.git(&["config", "branch.topic.merge", "refs/heads/other"]);
    assert!(
        reference_checkout::execute(
            &Repository::new(&f.0),
            &p,
            Choice::Checkout,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(f.head(), p.head);
}
#[test]
fn rebase_conflict_is_not_reported_as_checkout_success() {
    let f = Fixture::new();
    f.git(&["checkout", "-b", "topic"]);
    f.commit("base", "local");
    f.git(&["checkout", "main"]);
    f.commit("base", "remote");
    f.git(&["update-ref", "refs/remotes/origin/topic", &f.head()]);
    let p = f.plan(None);
    let r = f.run(&p, Choice::Rebase);
    assert!(!r.verified);
    assert!(!r.command.status.success());
    assert!(r.state.in_progress());
    assert!(!r.state.conflicts.is_empty());
    f.git(&["rebase", "--abort"]);
    assert_eq!(f.git(&["show", "topic:base"]), "local");
}
