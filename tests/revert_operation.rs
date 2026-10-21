use plico::{
    git::{Cancel, Repository},
    revert_operation::{self, Prepared, Status},
};
use std::{ffi::OsStr, fs, path::PathBuf, process::Command};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let f = Self(std::env::temp_dir().join(format!("plico-revert-{:x?}", random)));
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
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    fn commit(&self, text: &str, message: &str) -> String {
        fs::write(self.0.join("a"), text).unwrap();
        self.git(&["add", "a"]);
        self.git(&["commit", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }
    fn repo(&self) -> Repository {
        Repository::new(&self.0)
    }
    fn revert(&self, target: &str) -> revert_operation::Report {
        let repo = self.repo();
        let token = Cancel::default();
        let prepared = Prepared::read(&repo, target, &token).unwrap();
        revert_operation::execute(
            &repo,
            &prepared,
            OsStr::new("true"),
            None,
            &token,
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
fn root_and_ordinary_revert_create_new_commits_and_empty_is_explicit() {
    let f = Fixture::new();
    let root = f.commit("base\n", "base");
    let changed = f.commit("changed\n", "change");
    assert_eq!(f.revert(&changed).status, Status::Success);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"base\n");
    let head = f.git(&["rev-parse", "HEAD"]);
    assert_eq!(f.revert(&changed).status, Status::NothingToRevert);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(f.revert(&root).status, Status::Success);
    assert!(!f.0.join("a").exists());
}
#[test]
fn local_and_staged_changes_are_preserved_and_stale_plan_is_rejected() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let target = f.commit("changed\n", "change");
    let repo = f.repo();
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, &target, &token).unwrap();
    fs::write(f.0.join("a"), "draft\n").unwrap();
    assert_eq!(f.revert(&target).status, Status::LocalChanges);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"draft\n");
    fs::write(f.0.join("a"), "changed\n").unwrap();
    fs::write(f.0.join("staged"), "keep\n").unwrap();
    f.git(&["add", "staged"]);
    assert_eq!(f.revert(&target).status, Status::LocalChanges);
    assert_eq!(f.git(&["show", ":staged"]), "keep");
    f.git(&["commit", "-m", "other"]);
    assert!(
        revert_operation::execute(
            &repo,
            &prepared,
            OsStr::new("true"),
            None,
            &token,
            &Default::default()
        )
        .is_err()
    );
}
#[test]
fn conflict_can_be_resolved_or_aborted_using_shared_recovery() {
    use plico::{
        conflicts,
        operation_state::Operation,
        sequencer_operation::{self, Action},
    };
    for abort in [false, true] {
        let f = Fixture::new();
        f.commit("base\n", "base");
        let target = f.commit("change\n", "change");
        let before = f.commit("later\n", "later");
        let report = f.revert(&target);
        assert_eq!(report.status, Status::Conflicts);
        assert_eq!(report.state.unwrap().operation, Operation::Revert);
        let repo = f.repo();
        let token = Cancel::default();
        if !abort {
            fs::write(f.0.join("a"), "resolved\n").unwrap();
            let snap = conflicts::Snapshot::read(&repo, &token).unwrap();
            conflicts::resolve(
                &repo,
                &snap,
                &snap.files[0],
                conflicts::Resolution::MarkResolved,
                &token,
                &Default::default(),
            )
            .unwrap();
        }
        let action = if abort {
            Action::Abort
        } else {
            Action::Continue {
                editor: OsStr::new("true"),
                requests: None,
            }
        };
        let result = sequencer_operation::execute(
            &repo,
            Operation::Revert,
            action,
            &token,
            &Default::default(),
        )
        .unwrap();
        assert!(result.command.unwrap().status.success());
        assert_eq!(result.state.unwrap().operation, Operation::None);
        if abort {
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
        } else {
            assert_eq!(fs::read(f.0.join("a")).unwrap(), b"resolved\n");
        }
    }
}

#[test]
fn merge_commit_and_cancelled_request_do_not_modify_repository() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("task"), "task\n").unwrap();
    f.git(&["add", "task"]);
    f.git(&["commit", "-m", "task"]);
    let target = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.git(&["merge", "--no-ff", "-m", "merge", "task"]);
    let before = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo();
    assert!(
        Prepared::read(&repo, &before, &Cancel::default())
            .unwrap_err()
            .contains("merge commit")
    );
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, &target, &token).unwrap();
    token.cancel();
    assert!(
        revert_operation::execute(
            &repo,
            &prepared,
            OsStr::new("true"),
            None,
            &token,
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
    assert_eq!(fs::read(f.0.join("task")).unwrap(), b"task\n");
}

#[test]
fn unrelated_unstaged_edit_is_preserved() {
    let f = Fixture::new();
    fs::write(f.0.join("other"), "original\n").unwrap();
    f.git(&["add", "other"]);
    f.commit("base\n", "base");
    let target = f.commit("changed\n", "change");
    fs::write(f.0.join("other"), "draft\n").unwrap();
    assert_eq!(f.revert(&target).status, Status::Success);
    assert_eq!(fs::read(f.0.join("other")).unwrap(), b"draft\n");
    assert_eq!(f.git(&["show", "HEAD:other"]), "original");
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"base\n");
}

#[test]
fn batch_preserves_ui_order_and_skips_already_reverted_changes() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let older = f.commit("first\n", "first");
    let newer = f.commit("second\n", "second");
    assert_eq!(f.revert(&newer).status, Status::Success);
    let repo = f.repo();
    let token = Cancel::default();
    let batch =
        revert_operation::Batch::read(&repo, &[newer.clone(), older.clone()], &token).unwrap();
    let report = batch
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert_eq!(report.skipped, vec![newer]);
    assert_eq!(report.successful, vec![older]);
    assert!(report.remaining.is_empty());
    assert!(report.error.is_none());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"base\n");
}

#[test]
fn batch_reports_success_before_later_conflict_without_repeating_it() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let conflicting = f.commit("first\n", "first");
    let untouched = f.commit("second\n", "second");
    fs::write(f.0.join("other"), "other\n").unwrap();
    f.git(&["add", "other"]);
    f.git(&["commit", "-m", "independent"]);
    let independent = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo();
    let token = Cancel::default();
    let batch = revert_operation::Batch::read(
        &repo,
        &[independent.clone(), conflicting.clone(), untouched.clone()],
        &token,
    )
    .unwrap();
    let report = batch
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert_eq!(report.successful, vec![independent]);
    assert_eq!(report.remaining, vec![conflicting, untouched]);
    assert_eq!(report.last.unwrap().status, Status::Conflicts);
    assert!(!f.0.join("other").exists());
    assert!(report.error.is_none());
}

#[test]
fn batch_rejects_invalid_later_target_before_any_write() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let target = f.commit("changed\n", "changed");
    let repo = f.repo();
    assert!(
        revert_operation::Batch::read(&repo, &[target.clone(), "0".repeat(40)], &Cancel::default())
            .is_err()
    );
    assert!(
        revert_operation::Batch::read(&repo, &[target.clone(), target.clone()], &Cancel::default())
            .is_err()
    );
    assert_eq!(f.git(&["rev-parse", "HEAD"]), target);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn batch_revalidates_starting_head_and_rejects_oversized_selection() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let target = f.commit("changed\n", "changed");
    let repo = f.repo();
    let token = Cancel::default();
    let batch = revert_operation::Batch::read(&repo, &[target.clone()], &token).unwrap();
    let before = f.commit("later\n", "later");
    assert!(
        batch
            .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
    assert!(
        revert_operation::Batch::read(&repo, &vec![target; 1001], &token)
            .unwrap_err()
            .contains("1000")
    );
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"later\n");
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn batch_recovery_continues_remaining_commits_after_resolution() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let conflicting = f.commit("first\n", "first");
    f.commit("second\n", "second");
    fs::write(f.0.join("other"), "other\n").unwrap();
    f.git(&["add", "other"]);
    f.git(&["commit", "-m", "independent"]);
    let independent = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo();
    let token = Cancel::default();
    let report =
        revert_operation::Batch::read(&repo, &[conflicting.clone(), independent.clone()], &token)
            .unwrap()
            .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
            .unwrap();
    assert!(report.recovery.is_some());
    drop(report);
    // Reconstruct both repository and continuation, as after closing the window.
    let repo = f.repo();
    let recovery = revert_operation::Recovery::read(&repo, &token)
        .unwrap()
        .unwrap();
    assert!(
        recovery
            .resume(&repo, OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
    fs::write(f.0.join("a"), "resolved\n").unwrap();
    f.git(&["add", "a"]);
    let result = recovery
        .resume(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert_eq!(result.successful, vec![conflicting, independent]);
    assert!(result.remaining.is_empty());
    assert!(result.recovery.is_none());
    assert!(result.error.is_none());
    assert!(
        revert_operation::Recovery::read(&repo, &token)
            .unwrap()
            .is_none()
    );
    assert!(!f.0.join("other").exists());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"resolved\n");
    assert!(f.git(&["status", "--porcelain"]).is_empty());
    assert!(
        recovery
            .resume(&repo, OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
}

#[test]
fn aborted_conflict_does_not_allow_the_saved_queue_to_resume() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let conflicting = f.commit("first\n", "first");
    let before = f.commit("second\n", "second");
    let repo = f.repo();
    let token = Cancel::default();
    let report = revert_operation::Batch::read(&repo, &[conflicting], &token)
        .unwrap()
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    let recovery = report.recovery.unwrap();
    f.git(&["revert", "--abort"]);
    assert!(
        recovery
            .resume(&repo, OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"second\n");
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn abort_cleans_durable_queue_and_invalid_record_never_runs() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let first = f.commit("first\n", "first");
    let before = f.commit("second\n", "second");
    let repo = f.repo();
    let token = Cancel::default();
    let report = revert_operation::Batch::read(&repo, &[first], &token)
        .unwrap()
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(report.recovery.is_some());
    let record = f.0.join(".git/plico-revert-queue");
    assert!(record.exists());
    let result = plico::sequencer_operation::execute(
        &repo,
        plico::operation_state::Operation::Revert,
        plico::sequencer_operation::Action::Abort,
        &token,
        &Default::default(),
    )
    .unwrap();
    assert!(result.command.unwrap().status.success());
    assert!(!record.exists());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    fs::write(&record, "Plico revert queue v1\ninvalid\n\n--help\n").unwrap();
    assert!(revert_operation::Recovery::read(&repo, &token).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
}

#[test]
fn external_abort_queue_requires_review_and_clear_preserves_local_changes() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let first = f.commit("first\n", "first");
    let before = f.commit("second\n", "second");
    let repo = f.repo();
    let token = Cancel::default();
    revert_operation::Batch::read(&repo, &[first.clone()], &token)
        .unwrap()
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    let pending = revert_operation::Pending::read(&repo, &token)
        .unwrap()
        .unwrap();
    assert!(!pending.can_clear);
    assert!(pending.clear(&repo, &token).is_err());
    f.git(&["revert", "--abort"]);
    let batch = revert_operation::Batch::read(&repo, &[first.clone()], &token).unwrap();
    assert!(
        batch
            .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
    let pending = revert_operation::Pending::read(&repo, &token)
        .unwrap()
        .unwrap();
    assert_eq!(pending.targets(), &[first]);
    assert!(pending.can_clear);
    f.git(&["checkout", "-b", "another"]);
    assert!(pending.clear(&repo, &token).is_err());
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("a"), "staged\n").unwrap();
    f.git(&["add", "a"]);
    fs::write(f.0.join("a"), "unstaged\n").unwrap();
    let status = f.git(&["status", "--porcelain"]);
    let index = f.git(&["write-tree"]);
    pending.clear(&repo, &token).unwrap();
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    assert_eq!(f.git(&["status", "--porcelain"]), status);
    assert_eq!(f.git(&["write-tree"]), index);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"unstaged\n");
    assert!(!revert_operation::Recovery::record_exists(&repo, &token).unwrap());
}

#[test]
fn retry_plan_excludes_successes_and_is_bound_to_post_success_head() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let first = f.commit("first\n", "first");
    fs::write(f.0.join("other"), "other\n").unwrap();
    f.git(&["add", "other"]);
    f.git(&["commit", "-m", "second"]);
    let second = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let repo = f.repo();
    let token = Cancel::default();
    let report = revert_operation::Batch::read(&repo, &[second.clone(), first.clone()], &token)
        .unwrap()
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert_eq!(report.successful, vec![second]);
    assert_eq!(report.last.as_ref().unwrap().status, Status::LocalChanges);
    assert_eq!(report.remaining, vec![first.clone()]);
    let retry = report.retry.unwrap();
    assert_eq!(retry.targets().collect::<Vec<_>>(), vec![first.as_str()]);
    let before = f.git(&["rev-parse", "HEAD"]);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"draft\n");
    // Nothing is stashed until the user chooses to save. Here the fixture does
    // that explicitly; this test isolates retry selection and identity checks.
    assert!(f.git(&["stash", "list"]).is_empty());
    f.git(&["stash", "push", "-m", "explicit fixture save"]);
    let result = retry
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert_eq!(result.successful, vec![first]);
    assert_eq!(
        f.git(&["rev-list", "--count", &format!("{before}..HEAD")]),
        "1"
    );
    assert!(!f.0.join("other").exists());
    assert!(
        retry
            .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
}

#[test]
fn explicit_save_retry_restores_index_and_worktree_after_success() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    fs::write(f.0.join("other"), "base other\n").unwrap();
    f.git(&["add", "other"]);
    f.git(&["commit", "-m", "other"]);
    let target = f.commit("changed\n", "change");
    fs::write(f.0.join("other"), "staged other\n").unwrap();
    f.git(&["add", "other"]);
    fs::write(f.0.join("other"), "unstaged other\n").unwrap();
    let repo = f.repo();
    let token = Cancel::default();
    let failed = revert_operation::Batch::read(&repo, &[target.clone()], &token)
        .unwrap()
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    let retry = failed.retry.unwrap();
    let result = retry
        .save_and_execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    assert!(result.saved.is_none());
    assert_eq!(result.batch.unwrap().successful, vec![target]);
    assert_eq!(f.git(&["show", ":other"]), "staged other");
    assert_eq!(fs::read(f.0.join("other")).unwrap(), b"unstaged other\n");
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"base\n");
    assert!(f.git(&["stash", "list"]).is_empty());
    assert!(
        revert_operation::SavedChanges::read(&repo, &token)
            .unwrap()
            .is_none()
    );
    assert!(
        retry
            .save_and_execute(&repo, OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
}

#[test]
fn save_retry_conflict_retains_draft_and_durable_stash_identity() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let target = f.commit("first\n", "first");
    f.commit("second\n", "second");
    fs::write(f.0.join("other"), "draft\n").unwrap();
    f.git(&["add", "other"]);
    let repo = f.repo();
    let token = Cancel::default();
    let failed = revert_operation::Batch::read(&repo, &[target], &token)
        .unwrap()
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    let result = failed
        .retry
        .unwrap()
        .save_and_execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(result.error.is_none());
    assert_eq!(
        result.batch.unwrap().last.unwrap().status,
        Status::Conflicts
    );
    let saved = result.saved.unwrap();
    assert_eq!(
        revert_operation::SavedChanges::read(&f.repo(), &token).unwrap(),
        Some(saved.clone())
    );
    assert_eq!(f.git(&["rev-parse", "refs/stash"]), saved.stash);
    assert_eq!(f.git(&["show", &format!("{}:other", saved.stash)]), "draft");
    assert!(
        revert_operation::Recovery::read(&repo, &token)
            .unwrap()
            .is_some()
    );
}

#[test]
fn untracked_collision_does_not_offer_save_retry() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    fs::write(f.0.join("other"), "tracked\n").unwrap();
    f.git(&["add", "other"]);
    f.git(&["commit", "-m", "add other"]);
    f.git(&["rm", "other"]);
    f.git(&["commit", "-m", "delete other"]);
    let target = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("other"), "untracked draft\n").unwrap();
    let repo = f.repo();
    let token = Cancel::default();
    let report = revert_operation::Batch::read(&repo, &[target.clone()], &token)
        .unwrap()
        .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(report.retry.is_none());
    assert!(report.recovery.is_none());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), target);
    assert_eq!(fs::read(f.0.join("other")).unwrap(), b"untracked draft\n");
    assert!(f.git(&["stash", "list"]).is_empty());
}

#[test]
fn saved_revert_drafts_survive_reopen_continue_and_abort() {
    for action in ["continue", "abort", "other-stash"] {
        let f = Fixture::new();
        f.commit("base\n", "base");
        fs::write(f.0.join("other"), "original\n").unwrap();
        f.git(&["add", "other"]);
        f.git(&["commit", "-m", "other"]);
        let target = f.commit("first\n", "first");
        let before = f.commit("second\n", "second");
        fs::write(f.0.join("other"), "staged\n").unwrap();
        f.git(&["add", "other"]);
        fs::write(f.0.join("other"), "unstaged\n").unwrap();
        let repo = f.repo();
        let token = Cancel::default();
        let failed = revert_operation::Batch::read(&repo, &[target.clone()], &token)
            .unwrap()
            .execute(&repo, OsStr::new("true"), None, &token, &Default::default())
            .unwrap();
        let saved = failed
            .retry
            .unwrap()
            .save_and_execute(&repo, OsStr::new("true"), None, &token, &Default::default())
            .unwrap();
        let record = saved.saved.unwrap();
        drop(saved.batch);
        let repo = f.repo();
        if action == "abort" {
            let result = plico::sequencer_operation::execute(
                &repo,
                plico::operation_state::Operation::Revert,
                plico::sequencer_operation::Action::Abort,
                &token,
                &Default::default(),
            )
            .unwrap();
            assert!(result.command.unwrap().status.success());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
        } else {
            fs::write(f.0.join("a"), "resolved\n").unwrap();
            f.git(&["add", "a"]);
            if action == "other-stash" {
                f.git(&["update-ref", "refs/stash", &before]);
            }
            let recovery = revert_operation::Recovery::read(&repo, &token)
                .unwrap()
                .unwrap();
            let result = recovery
                .resume(&repo, OsStr::new("true"), None, &token, &Default::default())
                .unwrap();
            assert_eq!(result.successful, vec![target]);
            assert!(result.remaining.is_empty());
            if action == "other-stash" {
                assert!(result.error.unwrap().contains("top stash"));
                assert_eq!(f.git(&["rev-parse", "refs/stash"]), before);
                assert_eq!(
                    revert_operation::SavedChanges::read(&repo, &token).unwrap(),
                    Some(record)
                );
                assert_eq!(fs::read(f.0.join("other")).unwrap(), b"original\n");
                continue;
            }
            assert!(result.error.is_none(), "{:?}", result.error);
        }
        assert_eq!(f.git(&["show", ":other"]), "staged");
        assert_eq!(fs::read(f.0.join("other")).unwrap(), b"unstaged\n");
        assert!(f.git(&["stash", "list"]).is_empty());
        assert!(
            revert_operation::SavedChanges::read(&repo, &token)
                .unwrap()
                .is_none()
        );
        assert!(!revert_operation::Recovery::record_exists(&repo, &token).unwrap());
    }
}

#[test]
fn explicit_saved_revert_recovery_revalidates_and_preserves_drafts() {
    for action in ["retry", "clear", "edit", "branch", "head"] {
        let f = Fixture::new();
        f.commit("base\n", "base");
        fs::write(f.0.join("other"), "original\n").unwrap();
        f.git(&["add", "other"]);
        f.git(&["commit", "-m", "other"]);
        let target = f.commit("first\n", "first");
        let before = f.commit("second\n", "second");
        fs::write(f.0.join("other"), "staged\n").unwrap();
        f.git(&["add", "other"]);
        fs::write(f.0.join("other"), "unstaged\n").unwrap();
        let repo = f.repo();
        let token = Cancel::default();
        let saved = revert_operation::Batch::read(&repo, &[target], &token)
            .unwrap()
            .save_and_execute(&repo, OsStr::new("true"), None, &token, &Default::default())
            .unwrap();
        let record = saved.saved.unwrap();
        let conflicted = revert_operation::SavedPending::read(&repo, &token)
            .unwrap()
            .unwrap();
        assert!(!conflicted.can_clear && !conflicted.can_restore);
        assert!(conflicted.acknowledge(&repo, &token).is_err());
        assert!(
            conflicted
                .retry(&repo, &token, &Default::default())
                .is_err()
        );
        f.git(&["revert", "--abort"]);
        let repo = f.repo();
        let pending = revert_operation::SavedPending::read(&repo, &token)
            .unwrap()
            .unwrap();
        assert!(pending.can_clear && pending.can_restore);
        match action {
            "retry" => {
                pending.retry(&repo, &token, &Default::default()).unwrap();
                assert_eq!(f.git(&["show", ":other"]), "staged");
                assert_eq!(fs::read(f.0.join("other")).unwrap(), b"unstaged\n");
                assert!(f.git(&["stash", "list"]).is_empty());
                assert!(!revert_operation::SavedChanges::record_exists(&repo, &token).unwrap());
                assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
                continue;
            }
            "clear" => {
                fs::write(f.0.join("other"), "new staged\n").unwrap();
                f.git(&["add", "other"]);
                fs::write(f.0.join("other"), "new unstaged\n").unwrap();
                pending.acknowledge(&repo, &token).unwrap();
                assert!(!revert_operation::SavedChanges::record_exists(&repo, &token).unwrap());
                assert_eq!(f.git(&["show", ":other"]), "new staged");
                assert_eq!(fs::read(f.0.join("other")).unwrap(), b"new unstaged\n");
                assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
            }
            "edit" => {
                fs::write(f.0.join("other"), "new draft\n").unwrap();
                assert!(pending.retry(&repo, &token, &Default::default()).is_err());
                assert_eq!(fs::read(f.0.join("other")).unwrap(), b"new draft\n");
                assert_eq!(f.git(&["show", ":other"]), "original");
            }
            changed => {
                if changed == "branch" {
                    f.git(&["checkout", "-b", "elsewhere"]);
                } else {
                    f.commit("later\n", "later");
                }
                let head = f.git(&["rev-parse", "HEAD"]);
                assert!(pending.retry(&repo, &token, &Default::default()).is_err());
                assert!(pending.acknowledge(&repo, &token).is_err());
                assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
            }
        }
        assert_eq!(f.git(&["rev-parse", "refs/stash"]), record.stash);
        assert_eq!(
            f.git(&["show", &format!("{}:other", record.stash)]),
            "unstaged"
        );
        if action != "clear" {
            assert_eq!(
                revert_operation::SavedChanges::read(&repo, &token).unwrap(),
                Some(record)
            );
        }
    }
}
