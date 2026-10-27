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

#[test]
fn remote_and_tag_overwrite_recovery_preserves_checkout_identity() {
    for remote in [false, true] {
        for smart in [false, true] {
            let f = Fixture::new();
            let base = f.head();
            // Non-overlapping edits in the same file: ordinary checkout refuses,
            // smart checkout can restore cleanly on top of the target.
            f.commit("review", "base\n1\n2\n3\n4\n5\nend\n");
            f.commit("staged", "separately staged");
            let start = f.head();
            f.commit("review", "target\n1\n2\n3\n4\n5\nend\n");
            let target = f.head();
            f.git(&["tag", "version"]);
            f.git(&["update-ref", "refs/remotes/origin/topic", &target]);
            f.git(&["reset", "--hard", &start]);
            fs::write(f.0.join("review"), "base\n1\n2\n3\n4\n5\nlocal\n").unwrap();
            // Keep a staged edit while preserving HEAD used by preparation.
            fs::write(f.0.join("staged"), "staged edit").unwrap();
            f.git(&["add", "staged"]);
            let reference = if remote {
                "refs/remotes/origin/topic"
            } else {
                "refs/tags/version"
            };
            let repo = Repository::new(&f.0);
            let token = Cancel::default();
            let p = Prepared::read(&repo, reference, &target, &f.head(), None, &token).unwrap();
            let failed = f.run(&p, Choice::Checkout);
            assert_eq!(failed.status(), plico::checkout::Status::LocalChanges);
            assert!(failed.affected_files().paths.iter().any(|p| p == "review"));
            let recovered = reference_checkout::recover(
                &repo,
                &p,
                Choice::Checkout,
                smart,
                &token,
                &Default::default(),
            )
            .unwrap();
            assert!(recovered.checked_out());
            if smart {
                // The separate staged edit remains staged after restoration.
                assert!(recovered.restored());
                assert_eq!(
                    fs::read_to_string(f.0.join("review")).unwrap(),
                    "target\n1\n2\n3\n4\n5\nlocal\n"
                );
                assert!(
                    f.git(&["diff", "--cached", "--name-only"])
                        .contains("staged")
                );
            } else {
                assert_eq!(
                    fs::read_to_string(f.0.join("review")).unwrap(),
                    "target\n1\n2\n3\n4\n5\nend\n"
                );
            }
            assert_eq!(f.head(), target);
            if remote {
                assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/topic");
                assert_eq!(
                    f.git(&["rev-parse", "--symbolic-full-name", "@{upstream}"]),
                    reference
                );
            } else {
                assert_eq!(
                    f.git(&["rev-parse", "--symbolic-full-name", "HEAD"]),
                    "HEAD"
                );
            }
            assert_ne!(f.head(), base);
        }
    }
}

#[test]
fn recovery_rejects_stale_refs_and_keeps_conflicting_saved_changes() {
    let f = Fixture::new();
    let base = f.head();
    f.commit("base", "remote change");
    f.git(&["update-ref", "refs/remotes/origin/topic", &f.head()]);
    f.git(&["reset", "--hard", &base]);
    fs::write(f.0.join("base"), "local change").unwrap();
    let p = f.plan(None);
    let repo = Repository::new(&f.0);
    f.git(&["update-ref", "refs/remotes/origin/topic", &base]);
    for smart in [false, true] {
        assert!(
            reference_checkout::recover(
                &repo,
                &p,
                Choice::Checkout,
                smart,
                &Cancel::default(),
                &Default::default()
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(f.0.join("base")).unwrap(),
            "local change"
        );
        assert!(f.git(&["stash", "list"]).is_empty());
    }
    f.git(&["update-ref", "refs/remotes/origin/topic", &p.target]);
    let r = reference_checkout::recover(
        &repo,
        &p,
        Choice::Checkout,
        true,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(r.checked_out());
    assert!(!r.restored());
    assert!(r.saved_oid.is_some());
    assert!(!r.state.unwrap().conflicts.is_empty());
    assert!(f.git(&["stash", "list"]).contains("Plico: checkout"));
}

#[test]
fn checkout_during_resolved_merge_preserves_staged_content() {
    for tag in [false, true] {
        let f = Fixture::new();
        let head = f.head();
        f.git(&["tag", "audit-tag"]);
        let reference = if tag {
            "refs/tags/audit-tag"
        } else {
            "refs/remotes/origin/topic"
        };
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let prepared = Prepared::read(&repo, reference, &head, &head, None, &token).unwrap();
        f.git(&["checkout", "-b", "side"]);
        f.commit("merged", "side content");
        f.git(&["checkout", "main"]);
        f.git(&["merge", "--no-ff", "--no-commit", "side"]);
        assert!(f.0.join(".git/MERGE_HEAD").exists());
        let staged = f.git(&["ls-files", "--stage"]);
        let result = reference_checkout::execute(
            &repo,
            &prepared,
            Choice::Checkout,
            &token,
            &Default::default(),
        )
        .unwrap();
        assert!(result.verified);
        assert_eq!(result.state.detached, tag);
        assert_eq!(result.head, head);
        assert!(!result.state.in_progress());
        assert!(!f.0.join(".git/MERGE_HEAD").exists());
        assert_eq!(f.git(&["ls-files", "--stage"]), staged);
        assert_eq!(fs::read(f.0.join("merged")).unwrap(), b"side content");
        if !tag {
            assert_eq!(
                f.git(&["rev-parse", "--symbolic-full-name", "@{upstream}"]),
                reference
            );
        }
    }
}

#[test]
fn unresolved_merge_and_rebase_choice_remain_blocked() {
    for unresolved in [false, true] {
        let f = Fixture::new();
        f.git(&["checkout", "-b", "topic"]);
        f.commit("local", "local commit");
        f.git(&["checkout", "main"]);
        f.git(&["checkout", "-b", "side"]);
        f.commit("base", "side version");
        f.git(&["checkout", "main"]);
        if unresolved {
            f.commit("base", "main version");
        }
        let prepared = f.plan(if unresolved {
            Some("fresh-topic")
        } else {
            None
        });
        assert_eq!(prepared.local_ahead, !unresolved);
        let merge = Command::new("git")
            .arg("-C")
            .arg(&f.0)
            .args(["merge", "--no-ff", "--no-commit", "side"])
            .output()
            .unwrap();
        assert_eq!(merge.status.success(), !unresolved);
        let marker = fs::read(f.0.join(".git/MERGE_HEAD")).unwrap();
        let index = f.git(&["ls-files", "--stage"]);
        let content = fs::read(f.0.join("base")).unwrap();
        let head = f.head();
        let result = reference_checkout::execute(
            &Repository::new(&f.0),
            &prepared,
            if unresolved {
                Choice::Checkout
            } else {
                Choice::Rebase
            },
            &Cancel::default(),
            &Default::default(),
        );
        assert!(
            matches!(result, Err(ref error) if error == "Resolve the current Git operation before checkout")
        );
        assert_eq!(f.head(), head);
        assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/main");
        assert_eq!(f.git(&["ls-files", "--stage"]), index);
        assert_eq!(fs::read(f.0.join("base")).unwrap(), content);
        assert_eq!(fs::read(f.0.join(".git/MERGE_HEAD")).unwrap(), marker);
        assert!(
            f.git(&["for-each-ref", "refs/heads/fresh-topic"])
                .is_empty()
        );
    }
}

#[test]
fn prepared_checkout_cannot_be_replayed_in_another_worktree_with_identical_refs() {
    let f = Fixture::new();
    let root = f.0.join("linked");
    f.git(&[
        "worktree",
        "add",
        "--detach",
        root.to_str().unwrap(),
        "HEAD",
    ]);
    // Both trees are detached at the same object, with the same shared tag.
    f.git(&["checkout", "--detach", "HEAD"]);
    f.git(&["tag", "target"]);
    let head = f.head();
    let prepared = Prepared::read(
        &Repository::new(&f.0),
        "refs/tags/target",
        &head,
        &head,
        None,
        &Cancel::default(),
    )
    .unwrap();
    assert_eq!(prepared.repository().path, f.0);
    let other = Repository::new(&root);
    for smart in [false, true] {
        let error = reference_checkout::recover(
            &other,
            &prepared,
            Choice::Checkout,
            smart,
            &Cancel::default(),
            &Default::default(),
        )
        .err()
        .expect("must reject foreign recovery");
        assert!(error.contains("repository changed"), "{error}");
    }
    let error = reference_checkout::execute(
        &other,
        &prepared,
        Choice::Checkout,
        &Cancel::default(),
        &Default::default(),
    )
    .err()
    .expect("must reject foreign execution");
    assert!(error.contains("repository changed"), "{error}");
    assert_eq!(f.head(), head);
    assert!(f.git(&["stash", "list"]).is_empty());
    assert!(f.run(&prepared, Choice::Checkout).verified);
}
