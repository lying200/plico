use plico::{
    git::{Cancel, Repository},
    push::Status,
    tag_push,
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
            "plico-tag-push-{}-{}",
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
fn pushes_only_selected_annotated_tag_and_rejects_remote_replacement() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["init", "--bare", "remote.git"], true);
    f.git(&["remote", "add", "origin", "remote.git"], true);
    f.git(&["tag", "-a", "v1", "-m", "annotation"], true);
    f.git(&["tag", "-a", "other", "-m", "other"], true);
    f.git(&["config", "push.followTags", "true"], true);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let p = tag_push::Prepared::read(&repo, "refs/tags/v1", "origin", &token).unwrap();
    let result = tag_push::execute(&repo, &p, &token, &Default::default(), None).unwrap();
    assert_eq!(result.status, Status::NewBranch);
    let oid = f.git(&["rev-parse", "refs/tags/v1"], true);
    assert_eq!(
        f.git(&["--git-dir=remote.git", "show-ref"], true),
        format!("{oid} refs/tags/v1")
    );
    assert_eq!(
        f.git(
            &["--git-dir=remote.git", "cat-file", "-t", "refs/tags/v1"],
            true
        ),
        "tag"
    );
    assert_eq!(
        tag_push::execute(&repo, &p, &token, &Default::default(), None)
            .unwrap()
            .status,
        Status::UpToDate
    );
    f.git(&["tag", "-d", "v1"], true);
    f.commit("later");
    f.git(&["tag", "v1"], true);
    assert!(tag_push::execute(&repo, &p, &token, &Default::default(), None).is_err());
    let p = tag_push::Prepared::read(&repo, "refs/tags/v1", "origin", &token).unwrap();
    assert_eq!(
        tag_push::execute(&repo, &p, &token, &Default::default(), None)
            .unwrap()
            .status,
        Status::RejectedOther
    );
    assert_eq!(
        f.git(&["--git-dir=remote.git", "rev-parse", "refs/tags/v1"], true),
        oid
    );
}
#[test]
fn changed_remote_and_cancelled_request_are_rejected() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["tag", "v1"], true);
    f.git(&["init", "--bare", "remote.git"], true);
    f.git(&["remote", "add", "origin", "remote.git"], true);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let p = tag_push::Prepared::read(&repo, "refs/tags/v1", "origin", &token).unwrap();
    f.git(&["remote", "set-url", "origin", "other.git"], true);
    assert!(tag_push::execute(&repo, &p, &token, &Default::default(), None).is_err());
    f.git(&["remote", "set-url", "origin", "remote.git"], true);
    token.cancel();
    assert!(tag_push::execute(&repo, &p, &token, &Default::default(), None).is_err());
    assert_eq!(f.git(&["--git-dir=remote.git", "for-each-ref"], true), "");
}
