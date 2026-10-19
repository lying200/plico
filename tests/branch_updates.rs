use plico::{
    branch_updates::{References, RemoteState},
    git::{Cancel, Repository},
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
        let p = std::env::temp_dir().join(format!(
            "plico-updates-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        let f = Self(p);
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Fixture"]);
        f.git(&["config", "user.email", "fixture@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8(o.stdout).unwrap().trim().into()
    }
    fn commit(&self, message: &str) {
        self.git(&["commit", "--allow-empty", "-m", message]);
    }
    fn refs(&self) -> References {
        References::read(&Repository::new(self.0.clone()), &Cancel::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn tracks_external_commits_fetch_divergence_and_remote_without_fetching() {
    let f = Fixture::new();
    f.commit("base");
    let remote = f.0.join("remote.git");
    let o = Command::new("git")
        .args(["init", "--bare"])
        .arg(&remote)
        .output()
        .unwrap();
    assert!(o.status.success());
    f.git(&["remote", "add", "origin", remote.to_str().unwrap()]);
    f.git(&["push", "-u", "origin", "main"]);
    let repo = Repository::new(f.0.clone());
    let token = Cancel::default();
    let before = f.refs();
    assert_eq!(before.counts(&repo, &token).unwrap().outgoing, 0);
    assert_eq!(
        before.check_remote(&repo, &token).unwrap(),
        RemoteState::Current
    );
    f.git(&["checkout", "-b", "remote-work"]);
    f.commit("remote commit");
    f.git(&["push", "origin", "HEAD:main"]);
    f.git(&["checkout", "main"]);
    f.commit("local commit");
    // Simulate a new server tip without updating this clone's tracking ref.
    f.git(&[
        "update-ref",
        "refs/remotes/origin/main",
        before.head.as_ref().unwrap(),
    ]);
    let changed = f.refs();
    assert_ne!(before, changed);
    assert_eq!(changed.counts(&repo, &token).unwrap().outgoing, 1);
    assert_eq!(
        changed.check_remote(&repo, &token).unwrap(),
        RemoteState::Unfetched
    );
    assert_eq!(
        f.refs(),
        changed,
        "remote check must not fetch or mutate local refs"
    );
    f.git(&["fetch", "origin"]);
    let fetched = f.refs();
    let counts = fetched.counts(&repo, &token).unwrap();
    assert_eq!((counts.incoming, counts.outgoing), (1, 1));
    assert_eq!(
        fetched.check_remote(&repo, &token).unwrap(),
        RemoteState::Current
    );
    f.git(&["checkout", "--detach"]);
    let detached = f.refs();
    assert!(detached.branch.is_none());
    assert_eq!(detached.counts(&repo, &token).unwrap().outgoing, 0);
}
#[test]
fn unborn_no_upstream_tags_and_cancellation() {
    let f = Fixture::new();
    let unborn = f.refs();
    assert!(unborn.head.is_none());
    f.commit("base");
    let initial = f.refs();
    assert!(initial.upstream.is_none());
    f.git(&["tag", "v1"]);
    assert_ne!(initial, f.refs());
    let token = Cancel::default();
    token.cancel();
    assert!(References::read(&Repository::new(f.0.clone()), &token).is_err());
}
#[test]
fn outgoing_uses_push_target_in_triangular_workflow() {
    let f = Fixture::new();
    f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["remote", "add", "origin", "/unused/origin"]);
    f.git(&["remote", "add", "fork", "/unused/fork"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &base]);
    f.git(&["update-ref", "refs/remotes/fork/main", &base]);
    f.git(&["config", "branch.main.remote", "origin"]);
    f.git(&["config", "branch.main.merge", "refs/heads/main"]);
    f.git(&["config", "remote.pushDefault", "fork"]);
    f.git(&["config", "push.default", "current"]);
    f.commit("local");
    let head = f.git(&["rev-parse", "HEAD"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &head]);
    let counts = f
        .refs()
        .counts(&Repository::new(f.0.clone()), &Cancel::default())
        .unwrap();
    assert_eq!((counts.incoming, counts.outgoing), (0, 1));
}
