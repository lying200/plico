use plico::{
    cherry_pick::{self, Prepared, Status},
    git::{Cancel, Repository},
    operation_state::Operation,
    sequencer_operation::{self, Action},
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
            "plico-cherry-start-{}-{}",
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

fn run(f: &Fixture, ids: &[String]) -> cherry_pick::Report {
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, ids, &[], &token).unwrap();
    cherry_pick::execute(
        &repo,
        &prepared,
        std::ffi::OsStr::new("true"),
        None,
        &token,
        &Default::default(),
    )
    .unwrap()
}
#[test]
fn selected_log_rows_apply_oldest_first_and_keep_unrelated_changes() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["checkout", "-b", "feature"], true);
    f.commit("one");
    let one = f.git(&["rev-parse", "HEAD"], true);
    f.commit("two");
    let two = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["checkout", "main"], true);
    fs::write(f.0.join("untracked"), "keep").unwrap();
    assert_eq!(run(&f, &[two, one]).status, Status::Success);
    assert_eq!(f.git(&["log", "-2", "--format=%s"], true), "two\none");
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "two");
    assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep");
}
#[test]
fn middle_conflict_retains_queue_for_continue_and_abort() {
    for abort in [false, true] {
        let f = Fixture::new();
        f.commit("base");
        f.git(&["checkout", "-b", "feature"], true);
        fs::write(f.0.join("first"), "first").unwrap();
        f.git(&["add", "first"], true);
        f.git(&["commit", "-m", "first"], true);
        let first = f.git(&["rev-parse", "HEAD"], true);
        f.commit("conflicting");
        let middle = f.git(&["rev-parse", "HEAD"], true);
        fs::write(f.0.join("last"), "last").unwrap();
        f.git(&["add", "last"], true);
        f.git(&["commit", "-m", "last"], true);
        let last = f.git(&["rev-parse", "HEAD"], true);
        f.git(&["checkout", "main"], true);
        f.commit("ours");
        let before = f.git(&["rev-parse", "HEAD"], true);
        assert_eq!(run(&f, &[last, middle, first]).status, Status::Conflicts);
        assert!(f.0.join(".git/sequencer/todo").exists());
        assert!(!f.0.join("last").exists());
        let repo = Repository::new(&f.0);
        if !abort {
            fs::write(f.0.join("a"), "resolved").unwrap();
            f.git(&["add", "a"], true);
        }
        let action = if abort {
            Action::Abort
        } else {
            Action::Continue {
                editor: std::ffi::OsStr::new("true"),
                requests: None,
            }
        };
        let result = sequencer_operation::execute(
            &repo,
            Operation::CherryPick,
            action,
            &Cancel::default(),
            &Default::default(),
        )
        .unwrap();
        assert!(result.command.unwrap().status.success());
        assert!(!result.state.unwrap().in_progress());
        if abort {
            assert_eq!(f.git(&["rev-parse", "HEAD"], true), before);
            assert!(!f.0.join("first").exists());
        } else {
            assert_eq!(fs::read_to_string(f.0.join("last")).unwrap(), "last");
            assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "resolved");
        }
    }
}
#[test]
fn stale_head_and_dirty_index_do_not_silently_commit_local_files() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["checkout", "-b", "feature"], true);
    f.commit("feature");
    let target = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["checkout", "main"], true);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, std::slice::from_ref(&target), &[], &token).unwrap();
    f.commit("another");
    assert!(
        cherry_pick::execute(
            &repo,
            &prepared,
            std::ffi::OsStr::new("true"),
            None,
            &token,
            &Default::default()
        )
        .is_err()
    );
    fs::write(f.0.join("staged"), "keep").unwrap();
    f.git(&["add", "staged"], true);
    let before = f.git(&["rev-parse", "HEAD"], true);
    let index = fs::read(f.0.join(".git/index")).unwrap();
    assert_eq!(run(&f, &[target]).status, Status::LocalChanges);
    assert_eq!(f.git(&["rev-parse", "HEAD"], true), before);
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
}
#[test]
fn published_suffix_and_empty_result_are_explicit() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["checkout", "-b", "feature"], true);
    f.commit("feature");
    let target = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["update-ref", "refs/remotes/origin/main", &target], true);
    f.git(&["checkout", "main"], true);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let prepared = Prepared::read(
        &repo,
        std::slice::from_ref(&target),
        &["refs/remotes/origin/main".into()],
        &token,
    )
    .unwrap();
    let report = cherry_pick::execute(
        &repo,
        &prepared,
        std::ffi::OsStr::new("true"),
        None,
        &token,
        &Default::default(),
    )
    .unwrap();
    assert_eq!(report.status, Status::Success);
    assert!(
        f.git(&["log", "-1", "--format=%B"], true)
            .contains(&format!("(cherry picked from commit {target})"))
    );
    let report = run(&f, &[target]);
    assert_eq!(report.status, Status::Empty);
    assert!(report.empty.is_some());
}

#[test]
fn save_retry_preserves_staged_and_unstaged_changes() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["checkout", "-b", "feature"], true);
    fs::write(f.0.join("picked"), "picked").unwrap();
    f.git(&["add", "picked"], true);
    f.git(&["commit", "-m", "pick"], true);
    let target = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["checkout", "main"], true);
    fs::write(f.0.join("a"), "staged").unwrap();
    f.git(&["add", "a"], true);
    fs::write(f.0.join("a"), "unstaged").unwrap();
    let report = run(&f, &[target]);
    let retry = report.retry.expect("local blockage should offer retry");
    let repo = Repository::new(&f.0);
    let saved = retry
        .save_and_execute(
            &repo,
            std::ffi::OsStr::new("true"),
            None,
            &Cancel::default(),
            &Default::default(),
        )
        .unwrap();
    assert!(saved.error.is_none(), "{:?}", saved.error);
    assert!(saved.saved.is_none());
    assert_eq!(saved.report.unwrap().status, Status::Success);
    assert_eq!(f.git(&["show", ":a"], true), "staged");
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "unstaged");
    assert_eq!(fs::read_to_string(f.0.join("picked")).unwrap(), "picked");
    assert!(f.git(&["stash", "list"], true).is_empty());
}

#[test]
fn failed_saved_retry_retains_stash_and_can_abort_without_losing_draft() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["checkout", "-b", "feature"], true);
    f.commit("feature");
    let target = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["checkout", "main"], true);
    f.commit("ours");
    fs::write(f.0.join("local"), "local draft").unwrap();
    f.git(&["add", "local"], true);
    let retry = run(&f, &[target]).retry.unwrap();
    let repo = Repository::new(&f.0);
    let saved = retry
        .save_and_execute(
            &repo,
            std::ffi::OsStr::new("true"),
            None,
            &Cancel::default(),
            &Default::default(),
        )
        .unwrap();
    assert_eq!(saved.report.unwrap().status, Status::Conflicts);
    let stash = saved.saved.unwrap();
    assert_eq!(f.git(&["rev-parse", "refs/stash"], true), stash);
    assert_eq!(
        f.git(&["show", &format!("{stash}:local")], true),
        "local draft"
    );
    sequencer_operation::execute(
        &repo,
        Operation::CherryPick,
        Action::Abort,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    // Explicit restoration via the existing Stashes workflow remains possible.
    f.git(&["stash", "pop", "--index"], true);
    assert_eq!(f.git(&["show", ":local"], true), "local draft");
}

#[test]
fn untracked_blockage_and_cancel_never_offer_silent_overwrite() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["checkout", "-b", "feature"], true);
    fs::write(f.0.join("new"), "incoming").unwrap();
    f.git(&["add", "new"], true);
    f.git(&["commit", "-m", "incoming"], true);
    let target = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["checkout", "main"], true);
    fs::write(f.0.join("new"), "local untracked").unwrap();
    let before = f.git(&["rev-parse", "HEAD"], true);
    let report = run(&f, std::slice::from_ref(&target));
    assert_eq!(report.status, Status::UntrackedFiles);
    assert!(report.retry.is_none());
    assert_eq!(
        fs::read_to_string(f.0.join("new")).unwrap(),
        "local untracked"
    );
    let repo = Repository::new(&f.0);
    let prepared = Prepared::read(&repo, &[target], &[], &Cancel::default()).unwrap();
    let cancel = Cancel::default();
    cancel.cancel();
    assert!(
        prepared
            .save_and_execute(
                &repo,
                std::ffi::OsStr::new("true"),
                None,
                &cancel,
                &Default::default()
            )
            .is_err()
    );
    assert_eq!(f.git(&["rev-parse", "HEAD"], true), before);
    assert!(f.git(&["stash", "list"], true).is_empty());
}

#[test]
fn original_recovery_keeps_applied_prefix_and_remaining_git_queue() {
    original_recovery_case("continue");
}

#[test]
fn original_recovery_rejects_changed_queue_branch_or_aborted_operation() {
    for change in ["todo", "branch", "abort"] {
        original_recovery_case(change);
    }
}

#[test]
fn original_recovery_reports_empty_resolution_without_skipping_it() {
    for choice in ["empty", "empty-skip", "empty-keep"] {
        original_recovery_case(choice);
    }
}

fn original_recovery_case(change: &str) {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["checkout", "-b", "feature"], true);
    fs::write(f.0.join("first"), "first").unwrap();
    f.git(&["add", "first"], true);
    f.git(&["commit", "-qm", "prefix"], true);
    let first = f.git(&["rev-parse", "HEAD"], true);
    f.commit("conflicting");
    let middle = f.git(&["rev-parse", "HEAD"], true);
    fs::write(f.0.join("last"), "last").unwrap();
    f.git(&["add", "last"], true);
    f.git(&["commit", "-qm", "last"], true);
    let last = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["checkout", "main"], true);
    f.commit("ours");
    let report = run(&f, &[last, middle, first]);
    assert_eq!(report.status, Status::Conflicts);
    let recovery = report.recovery.unwrap();
    let head = f.git(&["rev-parse", "HEAD"], true);
    let repo = Repository::new(&f.0);
    assert!(
        recovery
            .resume(
                &repo,
                std::ffi::OsStr::new("true"),
                None,
                &Cancel::default(),
                &Default::default()
            )
            .is_err(),
        "unresolved conflicts must stop"
    );
    fs::write(
        f.0.join("a"),
        if change.starts_with("empty") {
            "ours"
        } else {
            "resolved"
        },
    )
    .unwrap();
    f.git(&["add", "a"], true);
    match change {
        "todo" => {
            let path = f.0.join(".git/sequencer/todo");
            let old = fs::read_to_string(&path).unwrap();
            fs::write(
                path,
                old.lines().take(1).collect::<Vec<_>>().join("\n") + "\n",
            )
            .unwrap();
        }
        "branch" => {
            f.git(&["branch", "-m", "renamed"], true);
        }
        "abort" => {
            f.git(&["cherry-pick", "--abort"], true);
        }
        _ => {}
    }
    let result = recovery.resume(
        &repo,
        std::ffi::OsStr::new("true"),
        None,
        &Cancel::default(),
        &Default::default(),
    );
    if change == "continue" {
        assert_eq!(result.unwrap().status, Status::Success);
        assert!(f.0.join("last").exists());
        assert_eq!(
            f.git(&["log", "--format=%s", "--all"], true)
                .lines()
                .filter(|s| *s == "prefix")
                .count(),
            2
        );
        assert_eq!(
            f.git(&["rev-list", "--count", &format!("{head}..HEAD")], true),
            "2"
        );
    } else if change.starts_with("empty") {
        let report = result.unwrap();
        assert_eq!(report.status, Status::Empty);
        assert!(report.empty.is_some());
        assert!(report.recovery.is_some());
        assert!(!f.0.join("last").exists());
        assert_eq!(f.git(&["rev-parse", "HEAD"], true), head);
        if change != "empty" {
            let choice = if change == "empty-keep" {
                sequencer_operation::EmptyChoice::Keep
            } else {
                sequencer_operation::EmptyChoice::Skip
            };
            let done = report
                .recovery
                .unwrap()
                .resolve_empty(
                    &repo,
                    (report.empty.as_ref().unwrap(), choice),
                    std::ffi::OsStr::new("true"),
                    None,
                    &Cancel::default(),
                    &Default::default(),
                )
                .unwrap();
            assert_eq!(done.status, Status::Success);
            assert!(f.0.join("last").exists());
            assert_eq!(
                f.git(&["rev-list", "--count", &format!("{head}..HEAD")], true),
                if change == "empty-keep" { "2" } else { "1" }
            );
        }
    } else {
        assert!(result.is_err());
        assert!(!f.0.join("last").exists());
        if change != "abort" {
            assert_eq!(f.git(&["rev-parse", "HEAD"], true), head);
        }
    }
}
