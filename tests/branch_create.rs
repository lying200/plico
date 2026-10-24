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
