use plico::{
    git::{Cancel, Repository},
    project_log::CommitId,
    project_sequence::{Kind, Plan, Report},
};
use std::{
    ffi::OsStr,
    fs,
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let f = Self(std::env::temp_dir().join(format!(
            "plico-project-sequence-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&f.0).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.hooksPath", "disabled-hooks"],
            vec!["commit", "--allow-empty", "-qm", "base"],
        ] {
            f.git(&args);
        }
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    fn commit(&self, name: &str) -> CommitId {
        fs::write(self.0.join(name), name).unwrap();
        self.git(&["add", name]);
        self.git(&["commit", "-qm", name]);
        CommitId {
            root: Arc::new(self.0.clone()),
            hash: self.git(&["rev-parse", "HEAD"]),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn execute(plan: Plan, token: &Cancel) -> plico::project_sequence::Results {
    plan.execute(OsStr::new("true"), None, token, &Default::default())
}
#[test]
fn revert_stops_after_dirty_root_and_retains_success_and_untouched_queue() {
    let a = Fixture::new();
    let b = Fixture::new();
    let c = Fixture::new();
    let a_id = a.commit("a");
    let b_id = b.commit("b");
    let c_id = c.commit("c");
    fs::write(b.0.join("b"), "draft").unwrap();
    let token = Cancel::default();
    let report = execute(
        Plan::read(&[a_id, b_id.clone(), c_id.clone()], Kind::Revert, &token).unwrap(),
        &token,
    );
    assert!(!report.complete());
    assert!(report.roots[0].complete());
    assert!(!a.0.join("a").exists());
    assert!(
        matches!(report.roots[1].result.as_ref().unwrap().as_ref().unwrap(), Report::Revert(batch) if batch.retry.is_some() && batch.remaining == vec![b_id.hash.clone()])
    );
    assert_eq!(fs::read_to_string(b.0.join("b")).unwrap(), "draft");
    assert!(report.roots[2].result.is_none());
    assert_eq!(c.git(&["rev-parse", "HEAD"]), c_id.hash);
    assert!(b.git(&["stash", "list"]).is_empty());
}
#[test]
fn cherry_pick_groups_keep_oldest_first_and_root_specific_results() {
    let a = Fixture::new();
    let b = Fixture::new();
    let a_base = a.git(&["rev-parse", "HEAD"]);
    let b_base = b.git(&["rev-parse", "HEAD"]);
    let a1 = a.commit("one");
    let a2 = a.commit("two");
    let b1 = b.commit("three");
    a.git(&["checkout", "-qb", "destination", &a_base]);
    b.git(&["checkout", "-qb", "destination", &b_base]);
    let token = Cancel::default();
    let report = execute(
        Plan::read(&[a2, b1, a1], Kind::CherryPick, &token).unwrap(),
        &token,
    );
    assert!(report.complete());
    assert_eq!(report.roots.len(), 2);
    assert_eq!(a.git(&["log", "-2", "--format=%s"]), "two\none");
    assert_eq!(b.git(&["log", "-1", "--format=%s"]), "three");
    assert_eq!(a.git(&["branch", "--show-current"]), "destination");
    assert_eq!(b.git(&["branch", "--show-current"]), "destination");
}
#[test]
fn empty_cherry_pick_stops_later_roots_and_keeps_empty_choice() {
    let a = Fixture::new();
    let b = Fixture::new();
    let a_id = a.commit("one");
    let b_id = b.commit("two");
    let head = b.git(&["rev-parse", "HEAD"]);
    let token = Cancel::default();
    let report = execute(
        Plan::read(&[a_id, b_id], Kind::CherryPick, &token).unwrap(),
        &token,
    );
    assert!(!report.complete());
    assert!(
        matches!(report.roots[0].result.as_ref().unwrap().as_ref().unwrap(), Report::CherryPick(result) if result.status == plico::cherry_pick::Status::Empty && result.empty.is_some())
    );
    assert!(report.roots[1].result.is_none());
    assert_eq!(b.git(&["rev-parse", "HEAD"]), head);
}
#[test]
fn cancellation_and_changed_head_do_not_silently_reprepare_targets() {
    let a = Fixture::new();
    let b = Fixture::new();
    let a_id = a.commit("one");
    let b_id = b.commit("two");
    let token = Cancel::default();
    let ids = [a_id, b_id.clone()];
    let plan = Plan::read(&ids, Kind::Revert, &token).unwrap();
    a.commit("external");
    let report = execute(plan, &token);
    assert!(report.roots[0].result.as_ref().unwrap().is_err());
    assert!(report.roots[1].result.is_none());
    assert_eq!(b.git(&["rev-parse", "HEAD"]), b_id.hash);
    let plan = Plan::read(&ids, Kind::Revert, &token).unwrap();
    token.cancel();
    let report = execute(plan, &token);
    assert!(report.cancelled);
    assert!(report.retry_plan().is_err());
    assert!(report.roots.iter().all(|r| r.result.is_none()));
    assert!(!report.complete());
    assert!(
        Repository::new(&a.0)
            .operation_state(&Cancel::default())
            .unwrap()
            .conflicts
            .is_empty()
    );
}

#[test]
fn cherry_pick_conflict_stays_in_its_root_and_does_not_start_the_next() {
    let a = Fixture::new();
    let b = Fixture::new();
    let base = a.git(&["rev-parse", "HEAD"]);
    let picked = a.commit("shared");
    a.git(&["checkout", "-qb", "destination", &base]);
    fs::write(a.0.join("shared"), "destination content").unwrap();
    a.git(&["add", "shared"]);
    a.git(&["commit", "-qm", "destination"]);
    let later = b.commit("later");
    let token = Cancel::default();
    let report = execute(
        Plan::read(&[picked, later.clone()], Kind::CherryPick, &token).unwrap(),
        &token,
    );
    assert!(!report.complete());
    assert_eq!(report.roots[0].repository.path, a.0);
    assert!(
        matches!(report.roots[0].result.as_ref().unwrap().as_ref().unwrap(), Report::CherryPick(result) if result.status == plico::cherry_pick::Status::Conflicts)
    );
    assert!(!a.git(&["ls-files", "-u"]).is_empty());
    assert!(report.retry_plan().is_err());
    assert!(report.roots[1].result.is_none());
    assert_eq!(b.git(&["rev-parse", "HEAD"]), later.hash);
    assert!(b.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn revert_retains_partial_commit_results_within_the_failed_root() {
    let a = Fixture::new();
    let b = Fixture::new();
    let oldest = a.commit("older");
    let newest = a.commit("newer");
    let later = b.commit("later");
    fs::write(a.0.join("older"), "keep draft").unwrap();
    let token = Cancel::default();
    let report = execute(
        Plan::read(
            &[newest.clone(), later.clone(), oldest.clone()],
            Kind::Revert,
            &token,
        )
        .unwrap(),
        &token,
    );
    let Report::Revert(batch) = report.roots[0].result.as_ref().unwrap().as_ref().unwrap() else {
        panic!("wrong report");
    };
    assert_eq!(batch.successful, vec![newest.hash]);
    assert_eq!(batch.remaining, vec![oldest.hash]);
    assert!(!a.0.join("newer").exists());
    assert_eq!(fs::read_to_string(a.0.join("older")).unwrap(), "keep draft");
    assert!(report.roots[1].result.is_none());
    assert_eq!(b.git(&["rev-parse", "HEAD"]), later.hash);
}

#[test]
fn invalid_commit_in_later_root_fails_preparation_before_any_write() {
    let a = Fixture::new();
    let b = Fixture::new();
    let selected = a.commit("one");
    let missing = CommitId {
        root: Arc::new(b.0.clone()),
        hash: "f".repeat(40),
    };
    for kind in [Kind::Revert, Kind::CherryPick] {
        assert!(
            Plan::read(
                &[selected.clone(), missing.clone()],
                kind,
                &Cancel::default()
            )
            .is_err()
        );
        assert_eq!(a.git(&["rev-parse", "HEAD"]), selected.hash);
        assert!(a.0.join("one").exists());
    }
}

#[test]
fn retry_excludes_completed_roots_and_the_successful_prefix_of_the_stopped_root() {
    let a = Fixture::new();
    let b = Fixture::new();
    let c = Fixture::new();
    let first = a.commit("first");
    let older = b.commit("older");
    let newer = b.commit("newer");
    let last = c.commit("last");
    fs::write(b.0.join("older"), "draft").unwrap();
    let token = Cancel::default();
    let stopped = execute(
        Plan::read(&[first, newer, older, last], Kind::Revert, &token).unwrap(),
        &token,
    );
    let first_head = a.git(&["rev-parse", "HEAD"]);
    let second_head = b.git(&["rev-parse", "HEAD"]);
    assert!(!b.0.join("newer").exists());
    let retry = stopped.retry_plan().unwrap();
    // Simulate an explicit user save. The retry plan itself must not stash.
    b.git(&["stash", "push", "-qm", "user saved draft"]);
    let completed = execute(retry, &token);
    assert!(completed.complete());
    assert_eq!(completed.roots.len(), 2);
    assert_eq!(a.git(&["rev-parse", "HEAD"]), first_head);
    assert_eq!(b.git(&["rev-parse", "HEAD~"]), second_head);
    assert!(!b.0.join("older").exists());
    assert!(!b.0.join("newer").exists());
    assert!(!c.0.join("last").exists());
    assert_eq!(b.git(&["show", "stash:older"]), "draft");
    assert!(completed.retry_plan().is_err());
}

#[test]
fn retry_keeps_untouched_root_guards_instead_of_accepting_external_head_changes() {
    let a = Fixture::new();
    let b = Fixture::new();
    let first = a.commit("first");
    let last = b.commit("last");
    fs::write(a.0.join("first"), "draft").unwrap();
    let token = Cancel::default();
    let stopped = execute(
        Plan::read(&[first, last], Kind::Revert, &token).unwrap(),
        &token,
    );
    a.git(&["stash", "push", "-qm", "saved"]);
    let external = b.commit("external");
    let retried = execute(stopped.retry_plan().unwrap(), &token);
    assert!(retried.roots[0].complete());
    assert!(retried.roots[1].result.as_ref().unwrap().is_err());
    assert_eq!(b.git(&["rev-parse", "HEAD"]), external.hash);
    assert!(b.0.join("last").exists());
    assert!(retried.retry_plan().is_err());
}

#[test]
fn cherry_retry_does_not_repeat_a_completed_root() {
    let a = Fixture::new();
    let b = Fixture::new();
    let a_base = a.git(&["rev-parse", "HEAD"]);
    let first = a.commit("first");
    let original = b.commit("original");
    fs::write(b.0.join("original"), "picked content").unwrap();
    b.git(&["commit", "-qam", "change original"]);
    let older = CommitId {
        root: Arc::new(b.0.clone()),
        hash: b.git(&["rev-parse", "HEAD"]),
    };
    a.git(&["checkout", "-qb", "destination", &a_base]);
    b.git(&["checkout", "-qb", "destination", &original.hash]);
    fs::write(b.0.join("original"), "draft").unwrap();
    let token = Cancel::default();
    let stopped = execute(
        Plan::read(&[first, older], Kind::CherryPick, &token).unwrap(),
        &token,
    );
    assert!(stopped.roots[0].complete());
    let first_head = a.git(&["rev-parse", "HEAD"]);
    let retry = stopped.retry_plan().unwrap();
    b.git(&["stash", "push", "-qm", "user draft"]);
    let completed = execute(retry, &token);
    assert!(completed.complete());
    assert_eq!(a.git(&["rev-parse", "HEAD"]), first_head);
    assert_eq!(b.git(&["log", "-1", "--format=%s"]), "change original");
    assert_eq!(b.git(&["show", "stash:original"]), "draft");
}

#[test]
fn retry_does_not_replace_a_git_owned_cherry_pick_queue() {
    let root = Fixture::new();
    let base = root.commit("original");
    fs::write(root.0.join("original"), "picked content").unwrap();
    root.git(&["commit", "-qam", "change original"]);
    let older = CommitId {
        root: Arc::new(root.0.clone()),
        hash: root.git(&["rev-parse", "HEAD"]),
    };
    let newer = root.commit("newer");
    root.git(&["checkout", "-qb", "destination", &base.hash]);
    fs::write(root.0.join("original"), "draft").unwrap();
    let token = Cancel::default();
    let stopped = execute(
        Plan::read(&[newer, older], Kind::CherryPick, &token).unwrap(),
        &token,
    );
    let todo = fs::read(root.0.join(".git/sequencer/todo")).unwrap();
    assert!(stopped.retry_plan().is_err());
    assert_eq!(fs::read(root.0.join(".git/sequencer/todo")).unwrap(), todo);
    assert_eq!(root.git(&["rev-parse", "HEAD"]), base.hash);
    assert_eq!(
        fs::read_to_string(root.0.join("original")).unwrap(),
        "draft"
    );
}

fn draft(f: &Fixture) {
    fs::write(f.0.join("draft"), "staged draft").unwrap();
    f.git(&["add", "draft"]);
    fs::write(f.0.join("draft"), "unstaged draft").unwrap();
}

#[test]
fn saved_group_restores_each_roots_index_and_worktree_only_after_success() {
    let a = Fixture::new();
    let b = Fixture::new();
    a.commit("draft");
    b.commit("draft");
    let first = a.commit("first");
    let second = b.commit("second");
    draft(&a);
    draft(&b);
    let token = Cancel::default();
    let stopped = execute(
        Plan::read(&[first, second], Kind::Revert, &token).unwrap(),
        &token,
    );
    assert!(!stopped.complete());
    let result = stopped
        .retry_plan()
        .unwrap()
        .save_and_execute(OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(result.complete());
    for root in [&a, &b] {
        assert_eq!(root.git(&["show", ":draft"]), "staged draft");
        assert_eq!(
            fs::read_to_string(root.0.join("draft")).unwrap(),
            "unstaged draft"
        );
        assert!(root.git(&["stash", "list"]).is_empty());
    }
    assert!(!a.0.join("first").exists());
    assert!(!b.0.join("second").exists());
}

#[test]
fn saved_group_conflict_retains_all_stashes_including_completed_and_unstarted_roots() {
    saved_conflict_continuation(false, false);
}

#[test]
fn saved_continuation_does_not_pop_an_unrelated_new_stash() {
    saved_conflict_continuation(true, false);
}

#[test]
fn saved_continuation_keeps_drafts_until_a_second_root_conflict_is_resolved() {
    saved_conflict_continuation(false, true);
}

fn saved_conflict_continuation(intervening_stash: bool, second_conflict: bool) {
    let a = Fixture::new();
    let b = Fixture::new();
    let c = Fixture::new();
    for root in [&a, &b, &c] {
        root.commit("draft");
    }
    let first = a.commit("first");
    let second = b.commit("second");
    let third = c.commit("third");
    fs::write(b.0.join("second"), "later committed content").unwrap();
    b.git(&["commit", "-qam", "later modification"]);
    for root in [&a, &b, &c] {
        draft(root);
    }
    if second_conflict {
        fs::write(c.0.join("third"), "later committed third").unwrap();
        // Preserve the unrelated draft state while preparing a second conflict.
        c.git(&["add", "third"]);
        c.git(&["commit", "-qm", "later third", "--", "third"]);
    }
    let third_head = c.git(&["rev-parse", "HEAD"]);
    let token = Cancel::default();
    let mut result = Plan::read(&[first, second, third.clone()], Kind::Revert, &token)
        .unwrap()
        .save_and_execute(OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(!result.complete());
    assert!(result.results.roots[0].complete());
    assert!(result.results.roots[2].result.is_none());
    assert_eq!(c.git(&["rev-parse", "HEAD"]), third_head);
    assert_eq!(result.saved.len(), 3);
    for (root, saved) in [&a, &b, &c].into_iter().zip(&result.saved) {
        assert_eq!(saved.path, root.0);
        assert_eq!(
            saved.saved_oid.as_deref(),
            Some(root.git(&["rev-parse", "refs/stash"]).as_str())
        );
        assert!(saved.restore.is_none());
        assert_eq!(root.git(&["show", "stash:draft"]), "unstaged draft");
        assert_eq!(fs::read_to_string(root.0.join("draft")).unwrap(), "draft");
    }
    assert!(!b.git(&["ls-files", "-u"]).is_empty());
    let completed_head = a.git(&["rev-parse", "HEAD"]);
    b.git(&["rm", "-q", "second"]);
    let unrelated = if intervening_stash {
        fs::write(a.0.join("draft"), "another application draft").unwrap();
        a.git(&["stash", "push", "-qm", "unrelated"]);
        Some(a.git(&["rev-parse", "refs/stash"]))
    } else {
        None
    };
    result
        .resume_revert(OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    if second_conflict {
        assert!(!result.results.complete());
        assert!(result.saved.iter().all(|saved| saved.restore.is_none()));
        for root in [&a, &b, &c] {
            assert_eq!(fs::read_to_string(root.0.join("draft")).unwrap(), "draft");
            assert!(!root.git(&["stash", "list"]).is_empty());
        }
        c.git(&["rm", "-q", "third"]);
        result
            .resume_revert(OsStr::new("true"), None, &token, &Default::default())
            .unwrap();
    }
    assert!(result.results.complete());
    assert_eq!(a.git(&["rev-parse", "HEAD"]), completed_head);
    assert!(!c.0.join("third").exists());
    assert_eq!(result.complete(), !intervening_stash);
    for root in [&b, &c] {
        assert_eq!(root.git(&["show", ":draft"]), "staged draft");
        assert_eq!(
            fs::read_to_string(root.0.join("draft")).unwrap(),
            "unstaged draft"
        );
        assert!(root.git(&["stash", "list"]).is_empty());
    }
    if let Some(unrelated) = unrelated {
        assert_eq!(a.git(&["rev-parse", "refs/stash"]), unrelated);
        assert!(result.saved[0].saved_oid.is_some());
        assert!(result.saved[0].restore.as_ref().unwrap().is_err());
    } else {
        assert_eq!(a.git(&["show", ":draft"]), "staged draft");
        assert_eq!(
            fs::read_to_string(a.0.join("draft")).unwrap(),
            "unstaged draft"
        );
        assert!(a.git(&["stash", "list"]).is_empty());
    }
    assert!(
        result
            .resume_revert(OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
}

#[test]
fn group_save_failure_keeps_earlier_stash_and_starts_no_reverts() {
    let a = Fixture::new();
    let b = Fixture::new();
    for root in [&a, &b] {
        root.commit("draft");
    }
    let first = a.commit("first");
    let second = b.commit("second");
    for root in [&a, &b] {
        draft(root);
    }
    let token = Cancel::default();
    let plan = Plan::read(&[first.clone(), second.clone()], Kind::Revert, &token).unwrap();
    fs::write(b.0.join(".git/index.lock"), "owned test lock").unwrap();
    let result = plan
        .save_and_execute(OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(result.error.is_some());
    assert!(!result.complete());
    assert!(
        result
            .results
            .roots
            .iter()
            .all(|root| root.result.is_none())
    );
    assert_eq!(a.git(&["rev-parse", "HEAD"]), first.hash);
    assert_eq!(b.git(&["rev-parse", "HEAD"]), second.hash);
    assert!(result.saved[0].saved_oid.is_some());
    assert!(result.saved[0].restore.is_none());
    assert_eq!(
        fs::read_to_string(b.0.join("draft")).unwrap(),
        "unstaged draft"
    );
}

#[test]
fn successful_commands_with_restore_conflict_are_not_a_completed_saved_operation() {
    let root = Fixture::new();
    let target = root.commit("changed");
    fs::write(root.0.join("changed"), "local draft").unwrap();
    let token = Cancel::default();
    let result = Plan::read(&[target], Kind::Revert, &token)
        .unwrap()
        .save_and_execute(OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(result.results.complete());
    assert!(!result.complete());
    assert!(result.saved[0].saved_oid.is_some());
    assert!(result.saved[0].restore.is_some());
    assert_eq!(root.git(&["show", "stash:changed"]), "local draft");
    assert!(!root.git(&["ls-files", "-u"]).is_empty());
}

#[test]
fn saved_linked_worktrees_share_a_reservation_but_keep_separate_drafts() {
    let root = Fixture::new();
    root.commit("draft");
    let target = root.commit("changed");
    let linked = Fixture(root.0.with_extension("linked"));
    root.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "linked",
        linked.0.to_str().unwrap(),
        "HEAD",
    ]);
    draft(&root);
    draft(&linked);
    fs::write(linked.0.join("draft"), "linked draft").unwrap();
    let second = CommitId {
        root: Arc::new(linked.0.clone()),
        hash: target.hash.clone(),
    };
    let token = Cancel::default();
    let result = Plan::read(&[target, second], Kind::Revert, &token)
        .unwrap()
        .save_and_execute(OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(result.complete());
    assert_eq!(result.saved.len(), 2);
    assert_eq!(
        fs::read_to_string(root.0.join("draft")).unwrap(),
        "unstaged draft"
    );
    assert_eq!(
        fs::read_to_string(linked.0.join("draft")).unwrap(),
        "linked draft"
    );
    assert!(!root.0.join("changed").exists());
    assert!(!linked.0.join("changed").exists());
}

#[test]
fn original_revert_workflow_resumes_conflicted_root_then_untouched_roots() {
    let a = Fixture::new();
    let b = Fixture::new();
    let c = Fixture::new();
    let first = a.commit("first");
    let conflicted = b.commit("conflicted");
    fs::write(b.0.join("conflicted"), "later committed content").unwrap();
    b.git(&["commit", "-qam", "later change"]);
    let prefix = b.commit("prefix");
    let last = c.commit("last");
    let token = Cancel::default();
    let mut results = execute(
        Plan::read(
            &[first, prefix.clone(), conflicted.clone(), last],
            Kind::Revert,
            &token,
        )
        .unwrap(),
        &token,
    );
    assert!(results.roots[0].complete());
    assert!(results.roots[2].result.is_none());
    let first_head = a.git(&["rev-parse", "HEAD"]);
    let stopped_head = b.git(&["rev-parse", "HEAD"]);
    assert!(!b.0.join("prefix").exists());
    b.git(&["rm", "-q", "conflicted"]);
    results
        .resume_revert(OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(results.complete());
    assert_eq!(a.git(&["rev-parse", "HEAD"]), first_head);
    assert_eq!(b.git(&["rev-parse", "HEAD~"]), stopped_head);
    assert!(!c.0.join("last").exists());
    let Report::Revert(report) = results.roots[1].result.as_ref().unwrap().as_ref().unwrap() else {
        panic!("wrong operation");
    };
    assert_eq!(report.successful, vec![prefix.hash, conflicted.hash]);
    assert!(report.remaining.is_empty());
    assert!(
        results
            .resume_revert(OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
}

#[test]
fn unresolved_or_externally_aborted_revert_does_not_start_other_roots() {
    let a = Fixture::new();
    let b = Fixture::new();
    let conflict = a.commit("conflicted");
    fs::write(a.0.join("conflicted"), "later content").unwrap();
    a.git(&["commit", "-qam", "later"]);
    let untouched = b.commit("untouched");
    let token = Cancel::default();
    let mut results = execute(
        Plan::read(&[conflict, untouched.clone()], Kind::Revert, &token).unwrap(),
        &token,
    );
    // Unresolved conflicts are rejected by the existing recovery command.
    assert!(
        results
            .resume_revert(OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
    assert!(!results.complete());
    assert!(results.roots[1].result.is_none());
    assert_eq!(b.git(&["rev-parse", "HEAD"]), untouched.hash);
    a.git(&["revert", "--abort"]);
    assert!(
        results
            .resume_revert(OsStr::new("true"), None, &token, &Default::default())
            .is_err()
    );
    assert!(results.roots[1].result.is_none());
    assert!(b.0.join("untouched").exists());
}

#[test]
fn resumed_revert_keeps_original_guards_for_other_roots() {
    let a = Fixture::new();
    let b = Fixture::new();
    let conflict = a.commit("conflicted");
    fs::write(a.0.join("conflicted"), "later content").unwrap();
    a.git(&["commit", "-qam", "later"]);
    let untouched = b.commit("untouched");
    let token = Cancel::default();
    let mut results = execute(
        Plan::read(&[conflict, untouched], Kind::Revert, &token).unwrap(),
        &token,
    );
    a.git(&["rm", "-q", "conflicted"]);
    let changed = b.commit("external");
    results
        .resume_revert(OsStr::new("true"), None, &token, &Default::default())
        .unwrap();
    assert!(results.roots[0].complete());
    assert!(results.roots[1].result.as_ref().unwrap().is_err());
    assert!(!results.complete());
    assert_eq!(b.git(&["rev-parse", "HEAD"]), changed.hash);
    assert!(b.0.join("untouched").exists());
}

#[test]
fn original_cherry_workflow_continues_other_roots_without_repreparing_them() {
    for changed in [false, true] {
        let a = Fixture::new();
        let b = Fixture::new();
        let base = a.git(&["rev-parse", "HEAD"]);
        let picked = a.commit("shared");
        a.git(&["checkout", "-qb", "destination", &base]);
        fs::write(a.0.join("shared"), "destination content").unwrap();
        a.git(&["add", "shared"]);
        a.git(&["commit", "-qm", "destination"]);
        let later = b.commit("later");
        b.git(&["checkout", "-qb", "destination", "HEAD~"]);
        let token = Cancel::default();
        let mut report = execute(
            Plan::read(&[picked, later], Kind::CherryPick, &token).unwrap(),
            &token,
        );
        assert!(!report.complete());
        assert!(report.roots[1].result.is_none());
        if changed {
            b.commit("external");
        }
        fs::write(a.0.join("shared"), "resolved").unwrap();
        a.git(&["add", "shared"]);
        report
            .resume_cherry_pick(OsStr::new("true"), None, &token, &Default::default())
            .unwrap();
        assert!(report.roots[0].complete());
        assert_eq!(report.complete(), !changed);
        assert_eq!(b.0.join("later").exists(), !changed);
        if changed {
            assert!(report.roots[1].result.as_ref().unwrap().is_err());
        }
        assert!(
            report
                .resume_cherry_pick(OsStr::new("true"), None, &token, &Default::default())
                .is_err()
        );
    }
}
