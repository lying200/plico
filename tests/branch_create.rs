use plico::{
    branch_create::{self, Prepared},
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
        let f = Self(std::env::temp_dir().join(format!(
            "plico-branch-create-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&f.0).unwrap();
        f.git(&["init", "-b", "main"]);
        for (key, value) in [
            ("user.name", "Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "disabled-hooks"),
        ] {
            f.git(&["config", key, value]);
        }
        fs::write(f.0.join("a"), "base\n").unwrap();
        f.git(&["add", "a"]);
        f.git(&["commit", "-m", "base"]);
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
    fn repo(&self) -> Repository {
        Repository::new(&self.0)
    }
    fn plan(&self, name: &str, source: &str, checkout: bool) -> Prepared {
        Prepared::read(&self.repo(), name, source, checkout, &Cancel::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn creates_without_checkout_and_checks_out_preserving_changes() {
    let f = Fixture::new();
    fs::write(f.0.join("a"), "draft\n").unwrap();
    for checkout in [false, true] {
        let name = if checkout {
            "task/switched"
        } else {
            "task/created"
        };
        let result = branch_create::execute(
            &f.repo(),
            &f.plan(name, "HEAD", checkout),
            &Cancel::default(),
            &Default::default(),
        )
        .unwrap();
        assert!(result.verified);
        assert_eq!(
            f.git(&["symbolic-ref", "HEAD"]),
            if checkout {
                "refs/heads/task/switched"
            } else {
                "refs/heads/main"
            }
        );
        assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "draft\n");
    }
}
#[test]
fn rejects_names_collisions_and_stale_head() {
    let f = Fixture::new();
    f.git(&["branch", "task/one"]);
    for name in [
        "",
        "HEAD",
        "-bad",
        "a..b",
        "a b",
        "@{-1}",
        "main",
        "main/child",
        "task",
    ] {
        assert!(
            Prepared::read(&f.repo(), name, "HEAD", true, &Cancel::default()).is_err(),
            "{name}"
        );
    }
    let p = f.plan("new", "HEAD", true);
    f.git(&["checkout", "-b", "other"]);
    assert!(
        branch_create::execute(&f.repo(), &p, &Cancel::default(), &Default::default()).is_err()
    );
    assert!(f.git(&["for-each-ref", "refs/heads/new"]).is_empty());
}
#[test]
fn failed_checkout_does_not_leave_branch_or_destroy_draft() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "next\n").unwrap();
    f.git(&["commit", "-am", "next"]);
    let next = f.git(&["rev-parse", "HEAD"]);
    f.git(&["reset", "--hard", &base]);
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let result = branch_create::execute(
        &f.repo(),
        &f.plan("new", &next, true),
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!result.verified);
    assert!(f.git(&["for-each-ref", "refs/heads/new"]).is_empty());
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "draft\n");
}
#[test]
fn cancellation_and_changed_source_do_not_write() {
    let f = Fixture::new();
    f.git(&["tag", "start"]);
    let p = f.plan("new", "refs/tags/start", false);
    let token = Cancel::default();
    token.cancel();
    assert!(branch_create::execute(&f.repo(), &p, &token, &Default::default()).is_err());
    f.git(&["tag", "-d", "start"]);
    assert!(
        branch_create::execute(&f.repo(), &p, &Cancel::default(), &Default::default()).is_err()
    );
    assert!(f.git(&["for-each-ref", "refs/heads/new"]).is_empty());
}
#[test]
fn delete_and_restore_preserves_tip_tracking_and_other_branch_config() {
    use plico::branch_delete::{self, Prepared};
    let f = Fixture::new();
    f.git(&["branch", "task"]);
    f.git(&["branch", "task.other"]);
    f.git(&["config", "branch.task.remote", "."]);
    f.git(&["config", "branch.task.merge", "refs/heads/main"]);
    f.git(&["config", "branch.task.other.description", "unrelated"]);
    let p = Prepared::read(&f.repo(), "refs/heads/task", &Cancel::default()).unwrap();
    assert!(p.unmerged.is_empty());
    let r = branch_delete::execute(
        &f.repo(),
        &p,
        false,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(r.deleted);
    assert!(r.command.unwrap().status.success());
    branch_delete::restore(&f.repo(), &p, &Cancel::default(), &Default::default()).unwrap();
    assert_eq!(f.git(&["rev-parse", "task"]), p.tip);
    assert_eq!(f.git(&["config", "branch.task.remote"]), ".");
    assert_eq!(f.git(&["config", "branch.task.merge"]), "refs/heads/main");
    assert_eq!(
        f.git(&["config", "--get-all", "branch.task.other.description"]),
        "unrelated"
    );
    assert!(
        branch_delete::restore(&f.repo(), &p, &Cancel::default(), &Default::default()).is_err()
    );
}
#[test]
fn delete_requires_unmerged_confirmation_and_rejects_current_or_stale_branch() {
    use plico::branch_delete::{self, Prepared};
    let f = Fixture::new();
    assert!(Prepared::read(&f.repo(), "refs/heads/main", &Cancel::default()).is_err());
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("b"), "task").unwrap();
    f.git(&["add", "b"]);
    f.git(&["commit", "-m", "task"]);
    f.git(&["checkout", "main"]);
    let p = Prepared::read(&f.repo(), "refs/heads/task", &Cancel::default()).unwrap();
    assert_eq!(p.unmerged.len(), 1);
    assert!(
        branch_delete::execute(
            &f.repo(),
            &p,
            false,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    f.git(&["branch", "-f", "task", "main"]);
    assert!(
        branch_delete::execute(&f.repo(), &p, true, &Cancel::default(), &Default::default())
            .is_err()
    );
    f.git(&["branch", "-f", "task", &p.tip]);
    assert!(
        branch_delete::execute(&f.repo(), &p, true, &Cancel::default(), &Default::default())
            .unwrap()
            .deleted
    );
}
#[test]
fn delete_refuses_branch_checked_out_in_another_worktree() {
    use plico::branch_delete::Prepared;
    let f = Fixture::new();
    let other = f.0.join("linked");
    f.git(&["worktree", "add", "-b", "task", other.to_str().unwrap()]);
    assert!(Prepared::read(&f.repo(), "refs/heads/task", &Cancel::default()).is_err());
    f.git(&["worktree", "remove", other.to_str().unwrap()]);
}
#[test]
fn creation_at_old_commit_and_remote_name_validation() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "second\n").unwrap();
    f.git(&["commit", "-am", "second"]);
    let p = f.plan("old-base", &base, false);
    assert!(p.matches_displayed(&f.git(&["rev-parse", "HEAD"]), &base));
    assert!(
        branch_create::execute(&f.repo(), &p, &Cancel::default(), &Default::default())
            .unwrap()
            .verified
    );
    assert_eq!(f.git(&["rev-parse", "old-base"]), base);
    f.git(&["update-ref", "refs/remotes/origin/topic", &base]);
    for name in ["head", "HeAd", "origin/topic"] {
        assert!(Prepared::read(&f.repo(), name, "HEAD", true, &Cancel::default()).is_err());
    }
}
#[test]
fn deletion_uses_upstream_merge_status_and_cancel_preserves_ref() {
    use plico::branch_delete::{self, Prepared};
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "second\n").unwrap();
    f.git(&["commit", "-am", "second"]);
    f.git(&["branch", "task"]);
    f.git(&["remote", "add", "origin", "unused-local.git"]);
    f.git(&["update-ref", "refs/remotes/origin/task", &base]);
    f.git(&["config", "branch.task.remote", "origin"]);
    f.git(&["config", "branch.task.merge", "refs/heads/task"]);
    let p = Prepared::read(&f.repo(), "refs/heads/task", &Cancel::default()).unwrap();
    assert_eq!(p.unmerged.len(), 1); // merged to HEAD, but not the tracking branch.
    let cancel = Cancel::default();
    cancel.cancel();
    assert!(branch_delete::execute(&f.repo(), &p, true, &cancel, &Default::default()).is_err());
    assert_eq!(f.git(&["rev-parse", "task"]), p.tip);
}

#[test]
#[cfg(unix)]
fn cancelled_restore_after_ref_write_finishes_tracking_configuration() {
    use plico::branch_delete::{self, Prepared};
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let f = Fixture::new();
    f.git(&["branch", "task"]);
    f.git(&["config", "branch.task.remote", "."]);
    f.git(&["config", "branch.task.merge", "refs/heads/main"]);
    let plan = Prepared::read(&f.repo(), "refs/heads/task", &Cancel::default()).unwrap();
    branch_delete::execute(
        &f.repo(),
        &plan,
        false,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    let hooks = f.0.join("hooks");
    fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("reference-transaction");
    fs::write(
        &hook,
        "#!/bin/sh\nif [ \"$1\" = committed ]; then\n touch restore-written\n sleep 2\nfi\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    f.git(&["config", "core.hooksPath", hooks.to_str().unwrap()]);
    let token = Cancel::default();
    let cancel = token.clone();
    let marker = f.0.join("restore-written");
    let waiter = std::thread::spawn(move || {
        let start = Instant::now();
        while !marker.exists() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(marker.exists(), "Git must reach the post-write hook");
        cancel.cancel();
    });
    let result = branch_delete::restore(&f.repo(), &plan, &token, &Default::default());
    waiter.join().unwrap();
    result.unwrap();
    assert!(token.cancelled());
    assert_eq!(f.git(&["rev-parse", "task"]), plan.tip);
    assert_eq!(f.git(&["config", "branch.task.remote"]), ".");
    assert_eq!(f.git(&["config", "branch.task.merge"]), "refs/heads/main");
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/main");
}

#[test]
fn overwrite_requires_confirmation_and_rechecks_existing_tip() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "next\n").unwrap();
    f.git(&["commit", "-am", "next"]);
    f.git(&["branch", "task"]);
    let plan =
        Prepared::read_options(&f.repo(), "task", &base, false, true, &Cancel::default()).unwrap();
    assert_eq!(plan.displaced_commits.len(), 1);
    assert!(
        branch_create::execute(&f.repo(), &plan, &Cancel::default(), &Default::default()).is_err()
    );
    let old = f.git(&["rev-parse", "task"]);
    assert_ne!(old, base);
    let result = branch_create::execute_confirmed(
        &f.repo(),
        &plan,
        true,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(result.verified);
    assert_eq!(f.git(&["rev-parse", "task"]), base);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), old);
    // Same prepared confirmation cannot silently reset a branch changed since then.
    assert!(
        branch_create::execute_confirmed(
            &f.repo(),
            &plan,
            true,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
}

#[test]
fn overwrite_current_branch_uses_transactional_checkout_and_preserves_dirty_files() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "next\n").unwrap();
    f.git(&["commit", "-am", "next"]);
    let old = f.git(&["rev-parse", "HEAD"]);
    let plan =
        Prepared::read_options(&f.repo(), "main", &base, false, true, &Cancel::default()).unwrap();
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let result = branch_create::execute_confirmed(
        &f.repo(),
        &plan,
        true,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!result.verified);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), old);
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "draft\n");
    fs::write(f.0.join("a"), "next\n").unwrap();
    let result = branch_create::execute_confirmed(
        &f.repo(),
        &plan,
        true,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(result.verified);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), base);
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "base\n");
}

#[test]
fn overwrite_refuses_linked_worktree_branch() {
    let f = Fixture::new();
    f.git(&["branch", "task"]);
    let linked = f.0.join("linked");
    f.git(&["worktree", "add", linked.to_str().unwrap(), "task"]);
    assert!(
        Prepared::read_options(&f.repo(), "task", "HEAD", false, true, &Cancel::default()).is_err()
    );
    assert!(
        Prepared::read_options(&f.repo(), "task", "HEAD", true, true, &Cancel::default()).is_err()
    );
}

#[test]
fn existing_branch_can_be_kept_or_rebased_without_discarding_its_work() {
    use branch_create::Choice;
    let f = Fixture::new();
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("task"), "task\n").unwrap();
    f.git(&["add", "task"]);
    f.git(&["commit", "-m", "task work"]);
    let task = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("main"), "main\n").unwrap();
    f.git(&["add", "main"]);
    f.git(&["commit", "-m", "main work"]);
    let main = f.git(&["rev-parse", "HEAD"]);
    let plan =
        Prepared::read_options(&f.repo(), "task", &main, true, false, &Cancel::default()).unwrap();
    assert!(plan.can_rebase());
    let result = branch_create::execute_choice(
        &f.repo(),
        &plan,
        Choice::KeepLocal,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(result.verified);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), task);
    let plan =
        Prepared::read_options(&f.repo(), "task", &main, true, false, &Cancel::default()).unwrap();
    let result = branch_create::execute_choice(
        &f.repo(),
        &plan,
        Choice::Rebase,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(result.verified);
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/task");
    assert_eq!(f.git(&["rev-parse", "HEAD^"]), main);
    assert_eq!(fs::read_to_string(f.0.join("task")).unwrap(), "task\n");
    assert_eq!(fs::read_to_string(f.0.join("main")).unwrap(), "main\n");
}

#[test]
fn branch_rebase_conflict_is_reported_as_in_progress_not_success() {
    use branch_create::Choice;
    let f = Fixture::new();
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("a"), "task\n").unwrap();
    f.git(&["commit", "-am", "task work"]);
    let task = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("a"), "main\n").unwrap();
    f.git(&["commit", "-am", "main work"]);
    let plan =
        Prepared::read_options(&f.repo(), "task", "HEAD", true, false, &Cancel::default()).unwrap();
    let result = branch_create::execute_choice(
        &f.repo(),
        &plan,
        Choice::Rebase,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!result.verified);
    let state = result.state.unwrap();
    assert!(state.in_progress());
    assert!(!state.conflicts.is_empty());
    f.git(&["rebase", "--abort"]);
    assert_eq!(f.git(&["rev-parse", "task"]), task);
}

#[test]
fn rebase_preserves_staged_and_unstaged_changes_and_cleans_saved_record() {
    use branch_create::Choice;
    let f = Fixture::new();
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("task"), "task\n").unwrap();
    f.git(&["add", "task"]);
    f.git(&["commit", "-m", "task"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("main"), "main\n").unwrap();
    f.git(&["add", "main"]);
    f.git(&["commit", "-m", "main"]);
    fs::write(f.0.join("a"), "staged\n").unwrap();
    f.git(&["add", "a"]);
    fs::write(f.0.join("a"), "unstaged\n").unwrap();
    fs::write(f.0.join("untracked"), "keep\n").unwrap();
    let plan =
        Prepared::read_options(&f.repo(), "task", "HEAD", true, false, &Cancel::default()).unwrap();
    let result = branch_create::execute_choice(
        &f.repo(),
        &plan,
        Choice::Rebase,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(result.verified, "{:?}", result.refresh_error);
    assert_eq!(f.git(&["show", ":a"]), "staged");
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "unstaged\n");
    assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep\n");
    assert!(f.git(&["stash", "list"]).is_empty());
    assert!(
        plico::rebase_saved::pending(&f.repo(), &Cancel::default())
            .unwrap()
            .is_none()
    );
}

#[test]
fn conflicted_branch_rebase_retains_saved_changes_until_abort() {
    use branch_create::Choice;
    let f = Fixture::new();
    fs::write(f.0.join("draft"), "base\n").unwrap();
    f.git(&["add", "draft"]);
    f.git(&["commit", "-m", "draft base"]);
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("a"), "task\n").unwrap();
    f.git(&["commit", "-am", "task"]);
    let task = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("a"), "main\n").unwrap();
    f.git(&["commit", "-am", "main"]);
    fs::write(f.0.join("draft"), "my changes\n").unwrap();
    let plan =
        Prepared::read_options(&f.repo(), "task", "HEAD", true, false, &Cancel::default()).unwrap();
    let result = branch_create::execute_choice(
        &f.repo(),
        &plan,
        Choice::Rebase,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!result.verified);
    assert!(result.state.unwrap().in_progress());
    assert!(!f.git(&["stash", "list"]).is_empty());
    assert!(
        plico::rebase_saved::pending(&f.repo(), &Cancel::default())
            .unwrap()
            .is_some()
    );
    let aborted = plico::rebase_operation::execute(
        &f.repo(),
        plico::rebase_operation::Action::Abort,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!aborted.state.unwrap().in_progress());
    assert_eq!(f.git(&["rev-parse", "task"]), task);
    assert_eq!(
        fs::read_to_string(f.0.join("draft")).unwrap(),
        "my changes\n"
    );
    assert!(f.git(&["stash", "list"]).is_empty());
    assert!(
        plico::rebase_saved::pending(&f.repo(), &Cancel::default())
            .unwrap()
            .is_none()
    );
}

#[test]
fn branch_rebase_restore_conflict_keeps_stash_and_recovery_record() {
    use branch_create::Choice;
    let f = Fixture::new();
    fs::write(f.0.join("b"), "base b\n").unwrap();
    f.git(&["add", "b"]);
    f.git(&["commit", "-m", "second file"]);
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("a"), "task change\n").unwrap();
    fs::write(f.0.join("b"), "task b\n").unwrap();
    f.git(&["commit", "-am", "task"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("main"), "main\n").unwrap();
    f.git(&["add", "main"]);
    f.git(&["commit", "-m", "main"]);
    fs::write(f.0.join("a"), "draft change\n").unwrap();
    fs::write(f.0.join("b"), "draft b\n").unwrap();
    let plan =
        Prepared::read_options(&f.repo(), "task", "HEAD", true, false, &Cancel::default()).unwrap();
    let result = branch_create::execute_choice(
        &f.repo(),
        &plan,
        Choice::Rebase,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!result.verified);
    assert!(result.refresh_error.unwrap().contains("restored"));
    assert!(!result.state.unwrap().conflicts.is_empty());
    assert_eq!(
        f.git(&["stash", "show", "-p"]).contains("draft change"),
        true
    );
    assert!(
        plico::rebase_saved::pending(&f.repo(), &Cancel::default())
            .unwrap()
            .is_some()
    );
    let repo = f.repo();
    let token = Cancel::default();
    let snapshot = plico::conflicts::Snapshot::read(&repo, &token).unwrap();
    assert!(
        snapshot.reversed,
        "automatic stash restore must keep the user's draft on the left"
    );
    let file = &snapshot.files[0];
    assert_eq!(
        plico::conflicts::content(
            &repo,
            file.side(plico::conflicts::Side::Yours, snapshot.reversed)
                .unwrap(),
            &token
        )
        .unwrap(),
        b"draft change\n"
    );
    let result = plico::conflicts::resolve(
        &repo,
        &snapshot,
        file,
        plico::conflicts::Resolution::Accept(plico::conflicts::Side::Yours),
        &token,
        &Default::default(),
    )
    .unwrap();
    let remaining = result.snapshot.unwrap();
    assert_eq!(remaining.files.len(), 1);
    assert!(remaining.reversed);
    let reopened = plico::conflicts::Snapshot::read(&f.repo(), &token).unwrap();
    assert!(reopened.reversed);
    let result = plico::conflicts::resolve(
        &repo,
        &reopened,
        &reopened.files[0],
        plico::conflicts::Resolution::Accept(plico::conflicts::Side::Yours),
        &token,
        &Default::default(),
    )
    .unwrap();
    assert!(result.snapshot.unwrap().files.is_empty());
    assert_eq!(fs::read(f.0.join("b")).unwrap(), b"draft b\n");
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"draft change\n");
    assert!(!f.0.join(".git/plico-conflict-saved-changes").exists());
}

#[test]
#[cfg(unix)]
fn cancellation_during_save_restores_draft_without_starting_rebase() {
    use branch_create::Choice;
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let f = Fixture::new();
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("task"), "task\n").unwrap();
    f.git(&["add", "task"]);
    f.git(&["commit", "-m", "task"]);
    let task = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    let main = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let hooks = f.0.join("hooks");
    fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("reference-transaction");
    fs::write(&hook, "#!/bin/sh\nif [ \"$1\" = committed ] && grep -q 'refs/stash' ; then\n touch stash-written\n sleep 1\nfi\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    f.git(&["config", "core.hooksPath", hooks.to_str().unwrap()]);
    let plan =
        Prepared::read_options(&f.repo(), "task", "HEAD", true, false, &Cancel::default()).unwrap();
    let token = Cancel::default();
    let cancel = token.clone();
    let marker = f.0.join("stash-written");
    let waiter = std::thread::spawn(move || {
        let start = Instant::now();
        while !marker.exists() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(marker.exists());
        cancel.cancel();
    });
    let result = branch_create::execute_choice(
        &f.repo(),
        &plan,
        Choice::Rebase,
        &token,
        &Default::default(),
    )
    .unwrap();
    waiter.join().unwrap();
    assert!(!result.verified);
    assert!(!result.state.unwrap().in_progress());
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert_eq!(f.git(&["rev-parse", "HEAD"]), main);
    assert_eq!(f.git(&["rev-parse", "task"]), task);
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "draft\n");
    assert!(f.git(&["stash", "list"]).is_empty());
    assert!(
        plico::rebase_saved::pending(&f.repo(), &Cancel::default())
            .unwrap()
            .is_none()
    );
}

#[test]
fn existing_name_without_extra_commits_does_not_implicitly_checkout_or_overwrite() {
    use branch_create::Choice;
    let f = Fixture::new();
    f.git(&["branch", "existing"]);
    let plan = Prepared::read_options(
        &f.repo(),
        "existing",
        "HEAD",
        true,
        false,
        &Cancel::default(),
    )
    .unwrap();
    assert!(plan.displaced_commits.is_empty());
    assert_eq!(plan.default_choice(), Choice::Create);
    let report = branch_create::execute_choice(
        &f.repo(),
        &plan,
        plan.default_choice(),
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!report.verified);
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/main");
    let plan = Prepared::read_options(
        &f.repo(),
        "existing",
        "HEAD",
        false,
        false,
        &Cancel::default(),
    )
    .unwrap();
    assert_eq!(plan.default_choice(), Choice::KeepLocal);
    assert!(
        branch_create::execute_choice(
            &f.repo(),
            &plan,
            plan.default_choice(),
            &Cancel::default(),
            &Default::default()
        )
        .unwrap()
        .verified
    );
}

#[test]
fn rename_preserves_dirty_index_and_tracking_or_explicitly_unsets_it() {
    use plico::branch_rename::{self, Prepared};
    let f = Fixture::new();
    let token = Cancel::default();
    f.git(&["remote", "add", "origin", "."]);
    f.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    f.git(&["branch", "--set-upstream-to=origin/main", "main"]);
    f.git(&["config", "branch.main.pushRemote", "origin"]);
    fs::write(f.0.join("a"), "staged\n").unwrap();
    f.git(&["add", "a"]);
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    for (old, new, unset) in [("main", "renamed", false), ("renamed", "final", true)] {
        let p = Prepared::read(&f.repo(), &format!("refs/heads/{old}"), &token).unwrap();
        assert!(p.has_upstream);
        let report =
            branch_rename::execute(&f.repo(), &p, new, unset, &token, &Default::default()).unwrap();
        assert!(report.succeeded());
        assert_eq!(f.git(&["branch", "--show-current"]), new);
        assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
        assert_eq!(f.git(&["show", ":a"]), "staged");
        assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "draft\n");
        assert_eq!(
            f.git(&["config", &format!("branch.{new}.pushRemote")]),
            "origin"
        );
        let tracking = f.git(&[
            "for-each-ref",
            "--format=%(upstream)",
            &format!("refs/heads/{new}"),
        ]);
        assert_eq!(
            tracking,
            if unset {
                ""
            } else {
                "refs/remotes/origin/main"
            }
        );
    }
}

#[test]
fn rename_rejects_invalid_duplicate_and_stale_preparations() {
    use plico::branch_rename::{self, Prepared};
    let f = Fixture::new();
    let token = Cancel::default();
    f.git(&["branch", "topic"]);
    let p = Prepared::read(&f.repo(), "refs/heads/topic", &token).unwrap();
    for name in [
        "main",
        "topic",
        "main/child",
        "HEAD",
        "@{-1}",
        "-oops",
        "bad name",
    ] {
        assert!(
            branch_rename::execute(&f.repo(), &p, name, false, &token, &Default::default())
                .is_err(),
            "{name}"
        );
    }
    f.git(&[
        "config",
        "branch.topic.description",
        "changed after dialog opened",
    ]);
    assert!(
        branch_rename::execute(&f.repo(), &p, "new", false, &token, &Default::default()).is_err()
    );
    let p = Prepared::read(&f.repo(), "refs/heads/topic", &token).unwrap();
    let report =
        branch_rename::execute(&f.repo(), &p, "new", false, &token, &Default::default()).unwrap();
    assert!(report.succeeded());
    assert_eq!(f.git(&["branch", "--show-current"]), "main");
    assert_eq!(
        f.git(&["config", "branch.new.description"]),
        "changed after dialog opened"
    );
}

#[test]
fn rename_reports_lock_failure_and_cancel_without_changing_refs() {
    use plico::branch_rename::{self, Prepared};
    let f = Fixture::new();
    let token = Cancel::default();
    let p = Prepared::read(&f.repo(), "refs/heads/main", &token).unwrap();
    fs::write(f.0.join(".git/refs/heads/renamed.lock"), "owned test lock").unwrap();
    let r = branch_rename::execute(&f.repo(), &p, "renamed", false, &token, &Default::default())
        .unwrap();
    assert!(!r.succeeded());
    assert!(!r.renamed);
    assert!(!r.diagnostic().is_empty());
    assert_eq!(f.git(&["branch", "--show-current"]), "main");
    fs::remove_file(f.0.join(".git/refs/heads/renamed.lock")).unwrap();
    token.cancel();
    assert!(
        branch_rename::execute(&f.repo(), &p, "renamed", false, &token, &Default::default())
            .is_err()
    );
    assert_eq!(f.git(&["branch", "--show-current"]), "main");
}

#[test]
fn rename_checked_out_branch_updates_linked_worktree_without_touching_edits() {
    use plico::branch_rename::{self, Prepared};
    let f = Fixture::new();
    let token = Cancel::default();
    let linked = f.0.join("linked");
    f.git(&["worktree", "add", "-b", "topic", linked.to_str().unwrap()]);
    fs::write(linked.join("a"), "linked draft\n").unwrap();
    let p = Prepared::read(&f.repo(), "refs/heads/topic", &token).unwrap();
    let r = branch_rename::execute(&f.repo(), &p, "renamed", false, &token, &Default::default())
        .unwrap();
    assert!(r.succeeded(), "{}", r.diagnostic());
    assert_eq!(
        f.git(&["-C", linked.to_str().unwrap(), "branch", "--show-current"]),
        "renamed"
    );
    assert_eq!(f.git(&["branch", "--show-current"]), "main");
    assert_eq!(
        fs::read_to_string(linked.join("a")).unwrap(),
        "linked draft\n"
    );
}

#[test]
fn rename_reports_remaining_included_upstream_as_partial_success() {
    use plico::branch_rename::{self, Prepared};
    let f = Fixture::new();
    let token = Cancel::default();
    f.git(&["remote", "add", "origin", "."]);
    f.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    f.git(&["branch", "--set-upstream-to=origin/main", "main"]);
    fs::write(
        f.0.join(".git/included-config"),
        "[branch \"renamed\"]\nremote = origin\nmerge = refs/heads/main\n",
    )
    .unwrap();
    f.git(&["config", "include.path", "included-config"]);
    let p = Prepared::read(&f.repo(), "refs/heads/main", &token).unwrap();
    let r = branch_rename::execute(&f.repo(), &p, "renamed", true, &token, &Default::default())
        .unwrap();
    assert!(r.renamed);
    assert!(!r.succeeded());
    assert!(r.diagnostic().contains("upstream"));
    assert_eq!(f.git(&["branch", "--show-current"]), "renamed");
    assert_eq!(
        f.git(&["for-each-ref", "--format=%(upstream)", "refs/heads/renamed"]),
        "refs/remotes/origin/main"
    );
}

fn git_at(path: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn prepared_branch_rejects_other_repository_with_identical_refs_and_commits() {
    let f = Fixture::new();
    let clone = f.0.join("other-repository");
    f.git(&["clone", "-q", "--no-local", ".", clone.to_str().unwrap()]);
    let other = Repository::new(&clone);
    let token = Cancel::default();
    for checkout in [false, true] {
        let prepared = f.plan("identity-test", "HEAD", checkout);
        assert_eq!(prepared.repository().path, f.0);
        assert!(
            branch_create::execute(&other, &prepared, &token, &Default::default()).is_err(),
            "same HEAD and symbolic ref must not authorize a different repository"
        );
        assert!(git_at(&other.path, &["branch", "--list", "identity-test"]).is_empty());
        assert!(f.git(&["branch", "--list", "identity-test"]).is_empty());
        assert_eq!(
            git_at(&other.path, &["symbolic-ref", "--short", "HEAD"]),
            "main"
        );
    }
}

#[test]
fn prepared_branch_rejects_sibling_worktree_even_with_shared_common_directory() {
    let f = Fixture::new();
    let linked = f.0.join("linked");
    f.git(&[
        "worktree",
        "add",
        "--detach",
        linked.to_str().unwrap(),
        "HEAD",
    ]);
    f.git(&["checkout", "--detach", "HEAD"]);
    let other = Repository::new(&linked);
    let token = Cancel::default();
    let prepared = f.plan("worktree-identity", "HEAD", true);
    let error = branch_create::execute(&other, &prepared, &token, &Default::default())
        .err()
        .unwrap();
    assert!(error.contains("Repository changed"), "{error}");
    assert!(f.git(&["branch", "--list", "worktree-identity"]).is_empty());
    let report = branch_create::execute(
        &prepared.repository(),
        &prepared,
        &token,
        &Default::default(),
    )
    .unwrap();
    assert!(report.verified, "{:?}", report.refresh_error);
    assert_eq!(
        f.git(&["symbolic-ref", "--short", "HEAD"]),
        "worktree-identity"
    );
    assert_eq!(
        git_at(&other.path, &["rev-parse", "--symbolic-full-name", "HEAD"]),
        "HEAD"
    );
}

#[test]
fn branch_batch_retains_partial_success_failure_and_unexecuted_roots() {
    use branch_create::batch::{Batch, Status};
    let f = Fixture::new();
    let parent = f.0.join("parent");
    f.git(&["clone", "-q", "--no-local", ".", parent.to_str().unwrap()]);
    let child = parent.join("child");
    git_at(
        &parent,
        &["clone", "-q", "--no-local", ".", child.to_str().unwrap()],
    );
    git_at(
        &child,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            "child-only",
        ],
    );
    let target = git_at(&child, &["rev-parse", "HEAD"]);
    let parent_head = git_at(&parent, &["rev-parse", "HEAD"]);
    let repositories = vec![f.repo(), Repository::new(&parent), Repository::new(&child)];
    let token = Cancel::default();
    let batch = Batch::read(&repositories, "batch-test", &target, false, false, &token).unwrap();
    assert_eq!(batch.preparations().count(), 3);
    assert!(git_at(&child, &["branch", "--list", "batch-test"]).is_empty());
    let result = batch.execute(None, &token, &Default::default());
    assert!(!result.succeeded());
    assert_eq!(result.roots[0].repository.path, child);
    assert!(result.roots[0].succeeded());
    assert_eq!(result.roots[1].repository.path, parent);
    assert!(matches!(
        &result.roots[1].status,
        Status::PreparationFailed(_)
    ));
    assert_eq!(result.roots[2].repository.path, f.0);
    assert!(matches!(&result.roots[2].status, Status::NotRun));
    assert_eq!(
        git_at(&child, &["rev-parse", "refs/heads/batch-test"]),
        target
    );
    assert!(git_at(&parent, &["branch", "--list", "batch-test"]).is_empty());
    assert!(f.git(&["branch", "--list", "batch-test"]).is_empty());
    assert_eq!(git_at(&parent, &["rev-parse", "HEAD"]), parent_head);
    assert_eq!(git_at(&child, &["rev-parse", "HEAD"]), target);
    // Later branches in roots where this operation failed/did not run are not
    // owned by its rollback, even when their names match.
    git_at(&parent, &["branch", "batch-test"]);
    f.git(&["branch", "batch-test"]);
    let rolled_back = result.rollback_creation(&token, &Default::default());
    assert_eq!(rolled_back.len(), 1);
    assert_eq!(rolled_back[0].repository.path, child);
    assert!(rolled_back[0].succeeded());
    assert!(git_at(&child, &["branch", "--list", "batch-test"]).is_empty());
    assert!(!git_at(&parent, &["branch", "--list", "batch-test"]).is_empty());
    assert!(!f.git(&["branch", "--list", "batch-test"]).is_empty());
}

#[test]
fn branch_batch_revalidates_and_cancellation_does_not_start_remaining_roots() {
    use branch_create::batch::{Batch, Status};
    let f = Fixture::new();
    let token = Cancel::default();
    let repos = vec![f.repo(), f.repo()];
    let batch = Batch::read(&repos, "stale", "HEAD", false, false, &token).unwrap();
    assert_eq!(
        batch.preparations().count(),
        1,
        "same worktree is executed once"
    );
    f.git(&["commit", "--allow-empty", "-m", "advanced"]);
    let result = batch.execute(None, &token, &Default::default());
    assert!(!result.succeeded());
    assert!(matches!(&result.roots[0].status, Status::Executed(Err(_))));
    assert!(f.git(&["branch", "--list", "stale"]).is_empty());
    let batch = Batch::read(&repos, "cancelled", "HEAD", false, false, &token).unwrap();
    token.cancel();
    let result = batch.execute(None, &token, &Default::default());
    assert!(!result.succeeded());
    assert!(matches!(&result.roots[0].status, Status::NotRun));
    assert!(f.git(&["branch", "--list", "cancelled"]).is_empty());
}

#[test]
fn branch_batch_creates_and_checks_out_each_roots_own_head() {
    use branch_create::batch::Batch;
    let first = Fixture::new();
    let second = Fixture::new();
    second.git(&["commit", "--allow-empty", "-m", "second head"]);
    let before = [
        first.git(&["rev-parse", "HEAD"]),
        second.git(&["rev-parse", "HEAD"]),
    ];
    let roots = [first.repo(), second.repo()];
    let token = Cancel::default();
    for checkout in [false, true] {
        let name = if checkout {
            "batch-checkout"
        } else {
            "batch-create"
        };
        let result = Batch::read(&roots, name, "HEAD", checkout, false, &token)
            .unwrap()
            .execute(None, &token, &Default::default());
        assert!(result.succeeded());
        assert!(!result.cancelled);
        assert_eq!(result.roots.len(), 2);
        for (fixture, head) in [&first, &second].into_iter().zip(&before) {
            assert_eq!(
                &fixture.git(&["rev-parse", &format!("refs/heads/{name}")]),
                head
            );
            assert_eq!(&fixture.git(&["rev-parse", "HEAD"]), head);
            assert_eq!(
                fixture.git(&["branch", "--show-current"]),
                if checkout { name } else { "main" }
            );
        }
    }
}

#[test]
fn creation_rollback_preserves_drafts_and_refuses_newer_branch_state() {
    use branch_create::batch::Batch;
    for case in ["draft", "moved", "occupied", "cancelled", "checkout"] {
        let f = Fixture::new();
        let token = Cancel::default();
        let result = Batch::read(
            &[f.repo()],
            "rollback-test",
            "HEAD",
            case == "checkout",
            false,
            &token,
        )
        .unwrap()
        .execute(None, &token, &Default::default());
        assert!(result.succeeded());
        fs::write(f.0.join("a"), "staged draft\n").unwrap();
        f.git(&["add", "a"]);
        fs::write(f.0.join("a"), "unstaged draft\n").unwrap();
        let before_index = fs::read(f.0.join(".git/index")).unwrap();
        let mut expected = f.git(&["rev-parse", "refs/heads/rollback-test"]);
        match case {
            "moved" => {
                expected = f.git(&[
                    "commit-tree",
                    "HEAD^{tree}",
                    "-p",
                    "HEAD",
                    "-m",
                    "new owner",
                ]);
                f.git(&["branch", "-f", "rollback-test", &expected]);
            }
            "occupied" => {
                f.git(&[
                    "worktree",
                    "add",
                    f.0.join("linked").to_str().unwrap(),
                    "rollback-test",
                ]);
            }
            "cancelled" => token.cancel(),
            _ => {}
        }
        let rollback = result.rollback_creation(&token, &Default::default());
        assert_eq!(rollback.len(), 1);
        assert_eq!(rollback[0].succeeded(), case == "draft", "{case}");
        if case == "draft" {
            assert!(f.git(&["branch", "--list", "rollback-test"]).is_empty());
        } else {
            assert_eq!(
                f.git(&["rev-parse", "refs/heads/rollback-test"]),
                expected,
                "{case}"
            );
        }
        assert_eq!(
            fs::read(f.0.join(".git/index")).unwrap(),
            before_index,
            "{case}"
        );
        assert_eq!(
            fs::read_to_string(f.0.join("a")).unwrap(),
            "unstaged draft\n",
            "{case}"
        );
    }
}

#[test]
fn creation_rollback_reports_each_root_and_continues_after_a_failure() {
    use branch_create::batch::Batch;
    let first = Fixture::new();
    let second = Fixture::new();
    let token = Cancel::default();
    let result = Batch::read(
        &[first.repo(), second.repo()],
        "rollback-group",
        "HEAD",
        false,
        false,
        &token,
    )
    .unwrap()
    .execute(None, &token, &Default::default());
    assert!(result.succeeded());
    let moved = first.git(&[
        "commit-tree",
        "HEAD^{tree}",
        "-p",
        "HEAD",
        "-m",
        "external change",
    ]);
    first.git(&["branch", "-f", "rollback-group", &moved]);
    let outcomes = result.rollback_creation(&token, &Default::default());
    assert_eq!(outcomes.len(), 2);
    assert!(
        !outcomes
            .iter()
            .find(|outcome| outcome.repository.path == first.0)
            .unwrap()
            .succeeded()
    );
    assert!(
        outcomes
            .iter()
            .find(|outcome| outcome.repository.path == second.0)
            .unwrap()
            .succeeded()
    );
    assert_eq!(
        first.git(&["rev-parse", "refs/heads/rollback-group"]),
        moved
    );
    assert!(
        second
            .git(&["branch", "--list", "rollback-group"])
            .is_empty()
    );
}

#[test]
fn checkout_rollback_restores_original_attachment_and_preserves_staged_drafts() {
    use branch_create::batch::Batch;
    for detached in [false, true] {
        let f = Fixture::new();
        let base = f.git(&["rev-parse", "HEAD"]);
        fs::write(f.0.join("later"), "later tree\n").unwrap();
        f.git(&["add", "later"]);
        f.git(&["commit", "-m", "later"]);
        let original = f.git(&["rev-parse", "HEAD"]);
        if detached {
            f.git(&["checkout", "--detach", "HEAD"]);
        }
        let token = Cancel::default();
        let result = Batch::read(&[f.repo()], "rollback-checkout", &base, true, false, &token)
            .unwrap()
            .execute(None, &token, &Default::default());
        assert!(result.succeeded());
        fs::write(f.0.join("a"), "staged draft\n").unwrap();
        f.git(&["add", "a"]);
        fs::write(f.0.join("a"), "unstaged draft\n").unwrap();
        fs::write(f.0.join("untracked"), "keep me\n").unwrap();
        let report = result.roots[0]
            .rollback_checkout(&token, &Default::default())
            .unwrap();
        assert!(report.succeeded(), "detached={detached}");
        assert_eq!(f.git(&["rev-parse", "HEAD"]), original);
        assert_eq!(
            f.git(&["rev-parse", "--symbolic-full-name", "HEAD"]),
            if detached { "HEAD" } else { "refs/heads/main" }
        );
        assert!(f.git(&["branch", "--list", "rollback-checkout"]).is_empty());
        assert_eq!(f.git(&["show", ":a"]), "staged draft");
        assert_eq!(
            fs::read_to_string(f.0.join("a")).unwrap(),
            "unstaged draft\n"
        );
        assert_eq!(
            fs::read_to_string(f.0.join("untracked")).unwrap(),
            "keep me\n"
        );
        assert!(f.git(&["stash", "list"]).is_empty());
    }
}

#[test]
fn checkout_rollback_retains_stash_and_created_branch_when_drafts_conflict() {
    use branch_create::{batch::Batch, rollback::CheckoutStep};
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "original branch later content\n").unwrap();
    f.git(&["commit", "-am", "later"]);
    let original = f.git(&["rev-parse", "HEAD"]);
    let token = Cancel::default();
    let result = Batch::read(&[f.repo()], "rollback-conflict", &base, true, false, &token)
        .unwrap()
        .execute(None, &token, &Default::default());
    assert!(result.succeeded());
    fs::write(f.0.join("a"), "draft on temporary branch\n").unwrap();
    let report = result.roots[0]
        .rollback_checkout(&token, &Default::default())
        .unwrap();
    assert!(!report.succeeded());
    assert!(report.deletion.is_none());
    let CheckoutStep::Local(recovery) = report.checkout else {
        panic!("expected attached recovery")
    };
    assert!(recovery.saved_oid.is_some());
    assert!(!recovery.state.unwrap().conflicts.is_empty());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), original);
    assert_eq!(f.git(&["rev-parse", "refs/heads/rollback-conflict"]), base);
    assert!(
        f.git(&["show", "stash@{0}:a"])
            .contains("draft on temporary branch")
    );
}

#[test]
fn checkout_rollback_rejects_changed_heads_and_cancellation_before_saving() {
    use branch_create::batch::Batch;
    for case in ["original-moved", "created-moved", "switched", "cancelled"] {
        let f = Fixture::new();
        let token = Cancel::default();
        let result = Batch::read(&[f.repo()], "rollback-guard", "HEAD", true, false, &token)
            .unwrap()
            .execute(None, &token, &Default::default());
        assert!(result.succeeded());
        let moved = f.git(&[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "external work",
        ]);
        match case {
            "original-moved" => {
                f.git(&["branch", "-f", "main", &moved]);
            }
            "created-moved" => {
                f.git(&["update-ref", "refs/heads/rollback-guard", &moved]);
            }
            "switched" => {
                f.git(&["checkout", "--detach", &moved]);
            }
            "cancelled" => token.cancel(),
            _ => unreachable!(),
        }
        fs::write(f.0.join("a"), "new draft\n").unwrap();
        let head = f.git(&["rev-parse", "HEAD"]);
        let reference = f.git(&["rev-parse", "refs/heads/rollback-guard"]);
        assert!(
            result.roots[0]
                .rollback_checkout(&token, &Default::default())
                .is_err(),
            "{case}"
        );
        assert_eq!(f.git(&["rev-parse", "HEAD"]), head, "{case}");
        assert_eq!(
            f.git(&["rev-parse", "refs/heads/rollback-guard"]),
            reference,
            "{case}"
        );
        assert!(f.git(&["stash", "list"]).is_empty(), "{case}");
        assert_eq!(
            fs::read_to_string(f.0.join("a")).unwrap(),
            "new draft\n",
            "{case}"
        );
    }
}

#[test]
fn overwritten_branch_rollback_restores_previous_tip_and_configuration() {
    use branch_create::{Choice, batch::Batch};
    let f = Fixture::new();
    let before = f.git(&["rev-parse", "HEAD"]);
    f.git(&["branch", "existing"]);
    f.git(&["config", "branch.existing.description", "keep description"]);
    f.git(&["config", "branch.existing.remote", "."]);
    f.git(&["config", "branch.existing.merge", "refs/heads/main"]);
    f.git(&["commit", "--allow-empty", "-m", "later"]);
    let target = f.git(&["rev-parse", "HEAD"]);
    let token = Cancel::default();
    let result = Batch::read(&[f.repo()], "existing", &target, false, true, &token)
        .unwrap()
        .execute(Some(Choice::Discard), &token, &Default::default());
    assert!(result.succeeded());
    assert_eq!(f.git(&["rev-parse", "refs/heads/existing"]), target);
    fs::write(f.0.join("a"), "staged draft\n").unwrap();
    f.git(&["add", "a"]);
    fs::write(f.0.join("a"), "unstaged draft\n").unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let restored = result.roots[0]
        .rollback_overwritten_ref(&token, &Default::default())
        .unwrap();
    assert!(restored.succeeded());
    assert_eq!(f.git(&["rev-parse", "refs/heads/existing"]), before);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), target);
    assert_eq!(
        f.git(&["config", "branch.existing.description"]),
        "keep description"
    );
    assert_eq!(f.git(&["config", "branch.existing.remote"]), ".");
    assert_eq!(
        f.git(&["config", "branch.existing.merge"]),
        "refs/heads/main"
    );
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.0.join("a")).unwrap(),
        "unstaged draft\n"
    );
}

#[test]
fn overwritten_branch_rollback_rejects_external_movement_occupancy_and_cancel() {
    use branch_create::{Choice, batch::Batch};
    for case in ["moved", "occupied", "cancelled"] {
        let f = Fixture::new();
        f.git(&["branch", "existing"]);
        f.git(&["commit", "--allow-empty", "-m", "later"]);
        let token = Cancel::default();
        let result = Batch::read(&[f.repo()], "existing", "HEAD", false, true, &token)
            .unwrap()
            .execute(Some(Choice::Discard), &token, &Default::default());
        assert!(result.succeeded());
        match case {
            "moved" => {
                f.git(&["branch", "-f", "existing", "HEAD~1"]);
            }
            "occupied" => {
                f.git(&[
                    "worktree",
                    "add",
                    f.0.join("linked").to_str().unwrap(),
                    "existing",
                ]);
            }
            "cancelled" => token.cancel(),
            _ => unreachable!(),
        }
        let tip = f.git(&["rev-parse", "refs/heads/existing"]);
        assert!(
            result.roots[0]
                .rollback_overwritten_ref(&token, &Default::default())
                .is_err(),
            "{case}"
        );
        assert_eq!(f.git(&["rev-parse", "refs/heads/existing"]), tip, "{case}");
    }
}

#[test]
fn delete_and_restore_reject_another_repository_with_identical_refs() {
    use plico::branch_delete::{self, Prepared};
    let a = Fixture::new();
    let b = Fixture::new();
    a.git(&["branch", "task"]);
    b.git(&["branch", "task"]);
    let plan = Prepared::read(&a.repo(), "refs/heads/task", &Cancel::default()).unwrap();
    let error = branch_delete::execute(
        &b.repo(),
        &plan,
        false,
        &Cancel::default(),
        &Default::default(),
    )
    .err()
    .unwrap();
    assert!(error.contains("different repository"));
    assert!(!b.git(&["for-each-ref", "refs/heads/task"]).is_empty());
    b.git(&["branch", "-D", "task"]);
    let error = branch_delete::restore(&b.repo(), &plan, &Cancel::default(), &Default::default())
        .unwrap_err();
    assert!(error.contains("different repository"));
    assert!(b.git(&["for-each-ref", "refs/heads/task"]).is_empty());
    assert!(!a.git(&["for-each-ref", "refs/heads/task"]).is_empty());
}
