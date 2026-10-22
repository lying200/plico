use plico::{
    branch_merge::{self, Prepared, Status},
    git::{Cancel, Repository},
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn merge(f: &Fixture, reference: &str, smart: bool) -> branch_merge::Report {
    let repo = f.repo();
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, reference, &token).unwrap();
    branch_merge::execute(
        &repo,
        &prepared,
        smart,
        OsStr::new("true"),
        None,
        &token,
        &Default::default(),
    )
    .unwrap()
}
#[test]
fn branch_merge_uses_full_reference_and_reports_up_to_date() {
    let f = Fixture::new();
    let base = f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    let target = f.commit("task\n", "task");
    f.git(&["checkout", "main"]);
    f.git(&["tag", "task", &base]);
    assert_eq!(merge(&f, "refs/heads/task", false).status, Status::Success);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), target);
    assert_eq!(
        merge(&f, "refs/heads/task", false).status,
        Status::AlreadyUpToDate
    );
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"task\n");
}
#[test]
fn local_edits_require_explicit_smart_merge_and_are_restored() {
    let f = Fixture::new();
    f.commit("one\ntwo\nthree\nfour\nfive\n", "base");
    f.git(&["checkout", "-b", "task"]);
    f.commit("ONE\ntwo\nthree\nfour\nfive\n", "task");
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("a"), "one\ntwo\nthree\nfour\nFIVE\n").unwrap();
    let result = merge(&f, "refs/heads/task", false);
    assert_eq!(result.status, Status::LocalChanges);
    assert!(result.save.is_none());
    assert!(f.git(&["stash", "list"]).is_empty());
    let result = merge(&f, "refs/heads/task", true);
    assert_eq!(result.status, Status::Success);
    assert!(result.error.is_none());
    assert!(result.saved_oid.is_none());
    assert_eq!(
        fs::read(f.0.join("a")).unwrap(),
        b"ONE\ntwo\nthree\nfour\nFIVE\n"
    );
    assert_eq!(f.git(&["show", "HEAD:a"]), "ONE\ntwo\nthree\nfour\nfive");
}
#[test]
fn conflicts_keep_saved_drafts_for_recovery_and_support_abort() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    f.commit("task\n", "task");
    f.git(&["checkout", "main"]);
    let before = f.commit("main\n", "main");
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let result = merge(&f, "refs/heads/task", true);
    assert_eq!(result.status, Status::Conflicts);
    assert!(result.restore.is_none());
    let saved = result.saved_oid.unwrap();
    assert_eq!(f.git(&["show", &format!("{saved}:a")]), "draft");
    let aborted = plico::merge_operation::execute(
        &f.repo(),
        plico::merge_operation::Action::Abort,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(aborted.command.unwrap().status.success());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    assert!(aborted.restore.unwrap().is_ok());
    assert!(f.git(&["stash", "list"]).is_empty());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"draft\n");
    assert!(
        plico::merge_saved::read(&f.repo().editor_directory(&Cancel::default()).unwrap())
            .unwrap()
            .is_none()
    );
}
#[test]
fn untracked_collision_and_stale_target_are_not_overwritten() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("other"), "tracked\n").unwrap();
    f.git(&["add", "other"]);
    f.git(&["commit", "-m", "other"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("other"), "untracked\n").unwrap();
    assert_eq!(
        merge(&f, "refs/heads/task", false).status,
        Status::UntrackedFiles
    );
    assert_eq!(fs::read(f.0.join("other")).unwrap(), b"untracked\n");
    let repo = f.repo();
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, "refs/heads/task", &token).unwrap();
    f.git(&["branch", "-f", "task", "main"]);
    assert!(
        branch_merge::execute(
            &repo,
            &prepared,
            false,
            OsStr::new("true"),
            None,
            &token,
            &Default::default()
        )
        .is_err()
    );
    assert!(Prepared::read(&repo, "task", &token).is_err());
    assert!(Prepared::read(&repo, "refs/heads/main", &token).is_err());
}

#[test]
fn remote_branch_can_create_a_merge_commit_without_fetch_or_tracking() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    fs::write(f.0.join("remote-file"), "remote\n").unwrap();
    f.git(&["add", "remote-file"]);
    f.git(&["commit", "-m", "remote"]);
    let target = f.git(&["rev-parse", "HEAD"]);
    f.git(&["update-ref", "refs/remotes/example/task", &target]);
    f.git(&["checkout", "main"]);
    let before = f.commit("local\n", "local");
    let report = merge(&f, "refs/remotes/example/task", false);
    assert_eq!(report.status, Status::Success);
    assert_eq!(
        f.git(&["show", "-s", "--format=%P", "HEAD"]),
        format!("{before} {target}")
    );
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"local\n");
    assert_eq!(fs::read(f.0.join("remote-file")).unwrap(), b"remote\n");
    assert!(f.git(&["remote"]).is_empty());
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn finishing_smart_merge_restores_staged_and_unstaged_drafts_after_reopen() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    fs::write(f.0.join("other"), "original\n").unwrap();
    f.git(&["add", "other"]);
    f.git(&["commit", "-m", "other"]);
    f.git(&["checkout", "-b", "task"]);
    f.commit("task\n", "task");
    f.git(&["checkout", "main"]);
    f.commit("main\n", "main");
    fs::write(f.0.join("other"), "staged\n").unwrap();
    f.git(&["add", "other"]);
    fs::write(f.0.join("other"), "unstaged\n").unwrap();
    let result = merge(&f, "refs/heads/task", true);
    assert_eq!(result.status, Status::Conflicts);
    // No UI/session object survives; on-disk ownership must be sufficient.
    let directory = f.repo().editor_directory(&Cancel::default()).unwrap();
    assert!(plico::merge_saved::read(&directory).unwrap().is_some());
    fs::write(f.0.join("a"), "resolved\n").unwrap();
    f.git(&["add", "a"]);
    let completed = plico::merge_operation::execute(
        &f.repo(),
        plico::merge_operation::Action::Commit,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(completed.command.unwrap().status.success());
    assert!(completed.restore.unwrap().is_ok());
    assert_eq!(f.git(&["show", ":other"]), "staged");
    assert_eq!(fs::read(f.0.join("other")).unwrap(), b"unstaged\n");
    assert_eq!(f.git(&["show", "HEAD:other"]), "original");
    assert!(f.git(&["stash", "list"]).is_empty());
    assert!(plico::merge_saved::read(&directory).unwrap().is_none());
}

#[test]
fn unrelated_top_stash_is_not_restored_or_dropped() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    f.commit("task\n", "task");
    f.git(&["checkout", "main"]);
    f.commit("main\n", "main");
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let report = merge(&f, "refs/heads/task", true);
    let saved = report.saved_oid.unwrap();
    // Use another valid stash object as an unrelated top entry without touching the merge index.
    let alternate = f.git(&["rev-parse", &format!("{saved}^2")]);
    f.git(&["update-ref", "refs/stash", &alternate]);
    let aborted = plico::merge_operation::execute(
        &f.repo(),
        plico::merge_operation::Action::Abort,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(aborted.command.unwrap().status.success());
    assert!(aborted.restore.unwrap().is_err());
    assert_eq!(f.git(&["rev-parse", "refs/stash"]), alternate);
    assert_eq!(f.git(&["show", &format!("{saved}:a")]), "draft");
    assert!(
        plico::merge_saved::read(&f.repo().editor_directory(&Cancel::default()).unwrap())
            .unwrap()
            .is_some()
    );
}

#[test]
fn restore_conflict_preserves_stash_and_reports_incomplete_restoration() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    f.commit("task\n", "task");
    f.git(&["checkout", "main"]);
    f.commit("main\n", "main");
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let report = merge(&f, "refs/heads/task", true);
    let saved = report.saved_oid.unwrap();
    fs::write(f.0.join("a"), "resolved\n").unwrap();
    f.git(&["add", "a"]);
    let completed = plico::merge_operation::execute(
        &f.repo(),
        plico::merge_operation::Action::Commit,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(completed.command.unwrap().status.success());
    assert!(completed.restore.unwrap().is_err());
    assert_eq!(f.git(&["rev-parse", "refs/stash"]), saved);
    assert_eq!(f.git(&["show", &format!("{saved}:a")]), "draft");
    assert_eq!(f.git(&["show", "HEAD:a"]), "resolved");
    assert!(
        plico::merge_saved::read(&f.repo().editor_directory(&Cancel::default()).unwrap())
            .unwrap()
            .is_some()
    );
}

#[test]
fn external_abort_can_retry_saved_draft_but_dirty_tree_cannot() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    f.commit("task\n", "task");
    f.git(&["checkout", "main"]);
    f.commit("main\n", "main");
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let report = merge(&f, "refs/heads/task", true);
    assert!(report.saved_oid.is_some());
    f.git(&["merge", "--abort"]);
    let repo = f.repo();
    let token = Cancel::default();
    let pending = plico::merge_saved::Pending::read(&repo, &token)
        .unwrap()
        .unwrap();
    assert!(pending.can_restore);
    fs::write(f.0.join("a"), "new draft\n").unwrap();
    assert!(pending.retry(&repo, &token, &Default::default()).is_err());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"new draft\n");
    fs::write(f.0.join("a"), "main\n").unwrap();
    pending.retry(&repo, &token, &Default::default()).unwrap();
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"draft\n");
    assert!(
        plico::merge_saved::Pending::read(&repo, &token)
            .unwrap()
            .is_none()
    );
}

#[test]
fn acknowledge_only_removes_record_and_rejects_stale_dialog() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    f.commit("task\n", "task");
    f.git(&["checkout", "main"]);
    f.commit("main\n", "main");
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let report = merge(&f, "refs/heads/task", true);
    let saved = report.saved_oid.unwrap();
    let repo = f.repo();
    let token = Cancel::default();
    let conflict = plico::merge_saved::Pending::read(&repo, &token)
        .unwrap()
        .unwrap();
    assert!(conflict.acknowledge(&repo, &token).is_err());
    f.git(&["merge", "--abort"]);
    let pending = plico::merge_saved::Pending::read(&repo, &token)
        .unwrap()
        .unwrap();
    f.git(&["checkout", "-b", "other"]);
    assert!(pending.acknowledge(&repo, &token).is_err());
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("a"), "handled\n").unwrap();
    pending.acknowledge(&repo, &token).unwrap();
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"handled\n");
    assert_eq!(f.git(&["rev-parse", "refs/stash"]), saved);
    assert!(
        plico::merge_saved::Pending::read(&repo, &token)
            .unwrap()
            .is_none()
    );
}

#[test]
fn merged_branch_deletion_revalidates_tip_and_protects_default_branches() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    f.git(&["checkout", "-b", "task"]);
    let task = f.commit("task\n", "task");
    f.git(&["checkout", "main"]);
    let repo = f.repo();
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, "refs/heads/task", &token).unwrap();
    assert!(prepared.may_propose_delete());
    assert!(prepared.prepare_deletion(&repo, &token).is_err());
    assert_eq!(merge(&f, "refs/heads/task", false).status, Status::Success);
    let deletion = prepared.prepare_deletion(&repo, &token).unwrap();
    assert_eq!(deletion.tip, task);
    assert!(deletion.unmerged.is_empty());
    f.git(&["branch", "other"]);
    f.git(&["checkout", "other"]);
    assert!(prepared.prepare_deletion(&repo, &token).is_err());
    let protected = Prepared::read(&repo, "refs/heads/main", &token).unwrap();
    assert!(!protected.may_propose_delete());
    assert!(protected.prepare_deletion(&repo, &token).is_err());
    f.git(&["checkout", "task"]);
    f.commit("new task\n", "new task");
    f.git(&["checkout", "main"]);
    assert!(prepared.prepare_deletion(&repo, &token).is_err());
}

#[test]
fn conflict_completion_keeps_deletion_bound_to_original_merge() {
    for action in ["complete", "abort", "source-moved"] {
        let f = Fixture::new();
        f.commit("base\n", "base");
        f.git(&["checkout", "-b", "task"]);
        let task = f.commit("task\n", "task");
        f.git(&["checkout", "main"]);
        let before = f.commit("main\n", "main");
        let repo = f.repo();
        let token = Cancel::default();
        let prepared = Prepared::read(&repo, "refs/heads/task", &token).unwrap();
        assert!(!prepared.matches_pending_merge(&repo, &token).unwrap());
        assert_eq!(
            merge(&f, "refs/heads/task", false).status,
            Status::Conflicts
        );
        assert!(prepared.matches_pending_merge(&repo, &token).unwrap());
        if action == "source-moved" {
            f.git(&["update-ref", "refs/heads/task", &before]);
            assert!(!prepared.matches_pending_merge(&repo, &token).unwrap());
        }
        let op = if action == "abort" {
            plico::merge_operation::Action::Abort
        } else {
            fs::write(f.0.join("a"), "resolved\n").unwrap();
            f.git(&["add", "a"]);
            plico::merge_operation::Action::Commit
        };
        let result =
            plico::merge_operation::execute(&repo, op, &token, &Default::default()).unwrap();
        assert!(result.command.unwrap().status.success());
        assert!(!prepared.matches_pending_merge(&repo, &token).unwrap());
        if action == "complete" {
            let deletion = prepared.prepare_deletion(&repo, &token).unwrap();
            assert_eq!(deletion.tip, task);
            assert!(deletion.unmerged.is_empty());
        } else {
            assert!(prepared.prepare_deletion(&repo, &token).is_err());
        }
    }
}

#[test]
fn merge_recovery_diagnostic_keeps_git_failure_details() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let repo = f.repo();
    let output = plico::operation_process::run(
        Command::new("git")
            .arg("-C")
            .arg(&f.0)
            .env("LC_ALL", "C")
            .args(["rev-parse", "--verify", "refs/heads/missing-merge-test"]),
        &Cancel::default(),
        &Default::default(),
        None,
    )
    .unwrap();
    assert!(!output.status.success());
    let result = Ok(plico::merge_operation::ResultState {
        command: Ok(output),
        state: repo.operation_state(&Cancel::default()),
        restore: None,
    });
    let diagnostic = plico::merge_operation::diagnostic(&result).unwrap_or_default();
    assert!(
        diagnostic.contains("fatal:"),
        "Git error lost: {diagnostic:?}"
    );
}

#[test]
fn merge_recovery_diagnostic_includes_transport_state_and_restore_errors() {
    let f = Fixture::new();
    f.commit("base\n", "base");
    let mut output = plico::operation_process::run(
        Command::new("git")
            .arg("-C")
            .arg(&f.0)
            .args(["rev-parse", "HEAD"]),
        &Cancel::default(),
        &Default::default(),
        None,
    )
    .unwrap();
    assert!(output.status.success());
    output.input_error = Some(std::io::Error::other("input bridge failed"));
    let result = Ok(plico::merge_operation::ResultState {
        command: Ok(output),
        state: Err("state read failed".into()),
        restore: Some(Err(
            "restore https://user:secret@example.invalid/path?token=private failed".into(),
        )),
    });
    let diagnostic = plico::merge_operation::diagnostic(&result).unwrap();
    assert!(diagnostic.contains("input bridge failed"));
    assert!(diagnostic.contains("state read failed"));
    assert!(diagnostic.contains("restore https://example.invalid/path failed"));
    assert!(!diagnostic.contains("secret"));
    assert!(!diagnostic.contains("private"));
}
