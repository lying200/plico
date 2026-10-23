use plico::commit_index::ExcludedIndex;
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "gitui-commit-index-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Test"]);
        f.git(&["config", "user.email", "test@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.autocrlf", "false"]);
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f
    }
    fn command(&self) -> Command {
        let mut c = Command::new("git");
        c.arg("-C").arg(&self.0);
        c
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let out = self.command().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }
    fn write(&self, name: &str, content: &str) {
        fs::write(self.0.join(name), content).unwrap();
    }
    fn capture(&self, paths: &[&str]) -> ExcludedIndex {
        let status = self.git(&[
            "status",
            "--porcelain=2",
            "-z",
            "--no-renames",
            "--untracked-files=no",
            "--ignored=no",
        ]);
        ExcludedIndex::capture(
            &status,
            &paths.iter().map(|p| p.as_bytes().to_vec()).collect(),
        )
        .unwrap()
    }
    fn apply(&self, input: Vec<u8>) {
        use plico::{
            git::Cancel,
            operation_process::{Progress, run_with_input},
        };
        let result = run_with_input(
            self.command().args(["update-index", "-z", "--index-info"]),
            &Cancel::default(),
            &Progress::default(),
            Some(std::time::Duration::from_secs(10)),
            &input,
        )
        .unwrap();
        assert!(
            result.status.success() && result.stopped.is_none() && result.input_error.is_none(),
            "{:?}: {}",
            result.input_error,
            String::from_utf8_lossy(&result.stderr.bytes())
        );
    }
    fn base(&self) {
        self.write("selected", "base\n");
        self.write("excluded", "base\n");
        self.git(&["add", "."]);
        self.git(&["commit", "-m", "base"]);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn excluded_staged_blob_survives_commit_amend_and_failed_commit() {
    for mode in ["commit", "amend", "failure"] {
        let f = Fixture::new();
        f.base();
        f.write("excluded", "staged\n");
        f.git(&["add", "excluded"]);
        let saved = f.git(&["ls-files", "--stage", "-z", "--", "excluded"]);
        f.write("excluded", "unstaged valuable content\n");
        f.write("selected", "chosen\n");
        let before = f.git(&["rev-parse", "HEAD"]);
        let plan = f.capture(&["selected"]);
        f.apply(plan.prepare_input());
        assert_eq!(f.git(&["show", ":excluded"]), b"base\n");
        f.git(&["add", "--", "selected"]);
        let args = match mode {
            "amend" => vec!["commit", "--amend", "-m", "new"],
            "failure" => vec!["-c", "user.name=", "commit", "-m", "new"],
            _ => vec!["commit", "-m", "new"],
        };
        let output = f.command().args(args).output().unwrap();
        assert_eq!(output.status.success(), mode != "failure");
        f.apply(plan.restore_input());
        assert_eq!(
            f.git(&["ls-files", "--stage", "-z", "--", "excluded"]),
            saved
        );
        assert_eq!(
            fs::read(f.0.join("excluded")).unwrap(),
            b"unstaged valuable content\n"
        );
        assert_eq!(f.git(&["show", "HEAD:excluded"]), b"base\n");
        if mode == "failure" {
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
        } else {
            assert_eq!(f.git(&["show", "HEAD:selected"]), b"chosen\n");
        }
    }
}

#[test]
fn excluded_add_delete_rename_and_file_directory_replacement_round_trip() {
    for mode in [
        "added-deleted",
        "deleted-recreated",
        "rename",
        "file-to-directory",
        "directory-to-file",
    ] {
        let f = Fixture::new();
        f.base();
        if mode == "directory-to-file" {
            fs::remove_file(f.0.join("excluded")).unwrap();
            fs::create_dir(f.0.join("excluded")).unwrap();
            f.write("excluded/child", "old child");
            f.git(&["add", "-A"]);
            f.git(&["commit", "-m", "directory base"]);
        }
        match mode {
            "added-deleted" => {
                f.write("new file 中文", "staged");
                f.git(&["add", "."]);
                fs::remove_file(f.0.join("new file 中文")).unwrap();
            }
            "deleted-recreated" => {
                f.git(&["rm", "excluded"]);
                f.write("excluded", "untracked replacement");
            }
            "rename" => {
                f.git(&["mv", "excluded", "renamed"]);
                f.write("renamed", "unstaged rename edit");
            }
            "file-to-directory" => {
                fs::remove_file(f.0.join("excluded")).unwrap();
                fs::create_dir(f.0.join("excluded")).unwrap();
                f.write("excluded/child", "child");
                f.git(&["add", "-A"]);
            }
            _ => {
                fs::remove_dir_all(f.0.join("excluded")).unwrap();
                f.write("excluded", "new file");
                f.git(&["add", "-A"]);
            }
        }
        let saved = f.git(&["ls-files", "--stage", "-z"]);
        let status = f.git(&["status", "--porcelain=2", "-z"]);
        let plan = f.capture(&["selected"]);
        f.apply(plan.prepare_input());
        assert!(
            f.git(&["diff", "--cached", "--name-only"]).is_empty(),
            "{mode}"
        );
        f.apply(plan.restore_input());
        assert_eq!(f.git(&["ls-files", "--stage", "-z"]), saved, "{mode}");
        assert_eq!(f.git(&["status", "--porcelain=2", "-z"]), status, "{mode}");
    }
}

#[test]
fn unborn_head_and_selected_rename_endpoints() {
    let f = Fixture::new();
    f.write("selected", "chosen");
    f.write("excluded", "staged");
    f.git(&["add", "."]);
    let plan = f.capture(&["selected"]);
    f.apply(plan.prepare_input());
    f.git(&["commit", "-m", "first"]);
    f.apply(plan.restore_input());
    assert_eq!(f.git(&["ls-tree", "--name-only", "HEAD"]), b"selected\n");
    assert_eq!(f.git(&["show", ":excluded"]), b"staged");
    f.git(&["commit", "-m", "second"]);
    f.git(&["mv", "selected", "renamed"]);
    assert!(f.capture(&["selected", "renamed"]).is_empty());
}

#[test]
fn conflicts_and_truncated_or_unexpected_records_fail_closed() {
    let f = Fixture::new();
    f.base();
    f.git(&["checkout", "-b", "other"]);
    f.write("excluded", "other\n");
    f.git(&["commit", "-am", "other"]);
    f.git(&["checkout", "main"]);
    f.write("excluded", "main\n");
    f.git(&["commit", "-am", "main"]);
    assert!(
        !f.command()
            .args(["merge", "other"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let status = f.git(&[
        "status",
        "--porcelain=2",
        "-z",
        "--no-renames",
        "--untracked-files=no",
    ]);
    assert!(
        ExcludedIndex::capture(&status, &HashSet::new())
            .unwrap_err()
            .contains("unmerged")
    );
    for bad in [b"1 truncated".as_slice(), b"2 unexpected\0", b"\0"] {
        assert!(ExcludedIndex::capture(bad, &HashSet::new()).is_err());
    }
}

#[cfg(unix)]
#[test]
fn symlinks_executable_modes_and_newline_paths_preserve_identity() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.base();
    symlink("selected", f.0.join("link\nwith\ttab")).unwrap();
    f.write("executable", "script");
    f.git(&["add", "."]);
    f.git(&["update-index", "--chmod=+x", "executable"]);
    let saved = f.git(&["ls-files", "--stage", "-z"]);
    let plan = f.capture(&["selected"]);
    f.apply(plan.prepare_input());
    f.apply(plan.restore_input());
    assert_eq!(f.git(&["ls-files", "--stage", "-z"]), saved);
    assert_eq!(
        fs::read_link(f.0.join("link\nwith\ttab")).unwrap(),
        PathBuf::from("selected")
    );
}

#[test]
fn sha256_gitlinks_and_intent_to_add_are_not_flattened() {
    let f = Fixture::new();
    // Reinitialize a separate SHA-256 repository; object width must not be hardcoded.
    let sha = f.0.join("sha");
    fs::create_dir(&sha).unwrap();
    let result = Command::new("git")
        .arg("-C")
        .arg(&sha)
        .args(["init", "--object-format=sha256", "-b", "main"])
        .output()
        .unwrap();
    assert!(result.status.success(), "Git with SHA-256 support required");
    let s = Fixture(sha);
    for (key, value) in [
        ("user.name", "Test"),
        ("user.email", "test@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "disabled-hooks"),
    ] {
        s.git(&["config", key, value]);
    }
    s.base();
    s.write("excluded", "sha staged");
    s.git(&["add", "excluded"]);
    let saved = s.git(&["ls-files", "--stage", "-z"]);
    let plan = s.capture(&["selected"]);
    s.apply(plan.prepare_input());
    s.apply(plan.restore_input());
    assert_eq!(s.git(&["ls-files", "--stage", "-z"]), saved);
    drop(s);

    f.base();
    let oid = String::from_utf8(f.git(&["rev-parse", "HEAD"])).unwrap();
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{},gitlink", oid.trim()),
    ]);
    f.write("intent", "not staged");
    f.git(&["add", "-N", "intent"]);
    let saved = f.git(&["ls-files", "--stage", "-z"]);
    let status = f.git(&["status", "--porcelain=2", "-z"]);
    let plan = f.capture(&["selected"]);
    f.apply(plan.prepare_input());
    f.apply(plan.restore_input());
    assert_eq!(f.git(&["ls-files", "--stage", "-z"]), saved);
    assert_eq!(f.git(&["status", "--porcelain=2", "-z"]), status);
}

fn request(f: &Fixture, amend: bool) -> plico::commit_operation::Request {
    use plico::{
        commit_operation::{self, Request},
        git::{Cancel, Repository},
    };
    Request {
        add: vec!["selected".into()],
        remove: vec![],
        message: "中文提交\n\nReviewed content\n".into(),
        amend,
        expected_head: commit_operation::head(&Repository::new(&f.0), &Cancel::default()).unwrap(),
    }
}
fn execute(f: &Fixture, r: &plico::commit_operation::Request) -> plico::commit_operation::Report {
    plico::commit_operation::execute(
        &plico::git::Repository::new(&f.0),
        r,
        &plico::git::Cancel::default(),
        &plico::operation_process::Progress::default(),
    )
    .unwrap()
}
fn hook(f: &Fixture, name: &str, body: &str) {
    fs::create_dir_all(f.0.join("test-hooks")).unwrap();
    let path = f.0.join("test-hooks").join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    f.git(&["config", "core.hooksPath", "test-hooks"]);
}
fn assert_no_message_file(f: &Fixture) {
    assert!(!fs::read_dir(f.0.join(".git")).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("gitui-commit-message-")
    }));
}

#[test]
fn executor_commits_selected_worktree_and_amends_without_unselected_index_leak() {
    for amend in [false, true] {
        let f = Fixture::new();
        f.base();
        f.write("excluded", "excluded staged\n");
        f.git(&["add", "excluded"]);
        f.write("excluded", "excluded unstaged\n");
        f.write("selected", "chosen\n");
        let r = request(&f, amend);
        let report = execute(&f, &r);
        assert!(report.succeeded(), "{:?}", report.restore);
        assert!(plico::commit_feedback::from_report(&report).is_ok());
        assert_eq!(f.git(&["show", "HEAD:selected"]), b"chosen\n");
        assert_eq!(f.git(&["show", "HEAD:excluded"]), b"base\n");
        assert_eq!(f.git(&["show", ":excluded"]), b"excluded staged\n");
        assert_eq!(
            fs::read(f.0.join("excluded")).unwrap(),
            b"excluded unstaged\n"
        );
        assert_eq!(
            f.git(&["rev-list", "--count", "HEAD"]),
            if amend { b"1\n" } else { b"2\n" }
        );
        assert!(
            String::from_utf8(f.git(&["log", "-1", "--format=%B"]))
                .unwrap()
                .contains("中文提交\n\nReviewed content")
        );
        assert_no_message_file(&f);
    }
}

#[test]
fn executor_hook_rejection_restores_excluded_index_and_retains_selected_staging() {
    let f = Fixture::new();
    f.base();
    f.write("excluded", "staged\n");
    f.git(&["add", "excluded"]);
    f.write("excluded", "unstaged\n");
    f.write("selected", "chosen\n");
    hook(&f, "pre-commit", "echo rejected-by-hook >&2\nexit 1");
    let r = request(&f, false);
    let report = execute(&f, &r);
    assert!(!report.succeeded());
    assert!(report.restore.is_ok());
    assert_eq!(report.head.unwrap(), r.expected_head);
    assert_eq!(f.git(&["show", ":excluded"]), b"staged\n");
    assert_eq!(f.git(&["show", ":selected"]), b"chosen\n");
    assert_eq!(fs::read(f.0.join("excluded")).unwrap(), b"unstaged\n");
    assert!(
        String::from_utf8_lossy(
            &report
                .steps
                .last()
                .unwrap()
                .result
                .as_ref()
                .unwrap()
                .stderr
                .bytes()
        )
        .contains("rejected-by-hook")
    );
    assert_no_message_file(&f);
}

#[test]
fn executor_preflight_and_prepare_failures_do_not_commit() {
    use plico::{
        commit_operation,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    for mode in ["stale", "encoding", "cancel", "path", "lock"] {
        let f = Fixture::new();
        f.base();
        f.write("excluded", "staged\n");
        f.git(&["add", "excluded"]);
        f.write("selected", "chosen\n");
        let saved = f.git(&["write-tree"]);
        let mut r = request(&f, false);
        let token = Cancel::default();
        match mode {
            "stale" => r.expected_head = None,
            "encoding" => {
                f.git(&["config", "i18n.commitEncoding", "ISO-8859-1"]);
            }
            "cancel" => token.cancel(),
            "path" => r.add = vec!["../escape".into()],
            _ => f.write(".git/index.lock", "test-owned lock"),
        }
        let result =
            commit_operation::execute(&Repository::new(&f.0), &r, &token, &Progress::default());
        if mode == "lock" {
            let report = result.unwrap();
            assert!(!report.succeeded());
            assert!(report.restore.is_err());
            assert!(report.recovery_input.is_some());
            fs::remove_file(f.0.join(".git/index.lock")).unwrap();
        } else {
            assert!(result.is_err(), "{mode}");
        }
        assert_eq!(f.git(&["write-tree"]), saved, "{mode}");
        assert_eq!(f.git(&["rev-list", "--count", "HEAD"]), b"1\n");
        assert_no_message_file(&f);
    }
}

#[test]
fn executor_reports_restore_failure_even_when_commit_itself_succeeded() {
    let f = Fixture::new();
    f.base();
    f.write("excluded", "staged\n");
    f.git(&["add", "excluded"]);
    f.write("selected", "chosen\n");
    hook(
        &f,
        "post-commit",
        &format!(
            "echo test-owned-lock > .git/index.lock\nprintf '%s\\n' '{}' >&2",
            "x".repeat(70_000)
        ),
    );
    let r = request(&f, false);
    let report = execute(&f, &r);
    assert!(!report.succeeded());
    assert!(report.restore.is_err());
    assert!(
        report
            .steps
            .last()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .status
            .success()
    );
    let feedback = plico::commit_feedback::from_report(&report).unwrap_err();
    assert_eq!(feedback.summary, "Commit index recovery is required");
    assert!(
        feedback
            .details
            .starts_with(report.restore.as_ref().unwrap_err())
    );
    assert!(feedback.details.ends_with("[Diagnostic truncated]"));
    assert!(feedback.details.chars().count() <= 32_000);
    assert_ne!(report.head.unwrap(), r.expected_head);
    fs::remove_file(f.0.join(".git/index.lock")).unwrap();
    f.apply(report.recovery_input.unwrap());
    assert_eq!(f.git(&["show", ":excluded"]), b"staged\n");
    assert_eq!(f.git(&["show", "HEAD:excluded"]), b"base\n");
    assert_no_message_file(&f);
}

#[test]
fn executor_cancellation_in_hook_reports_actual_state_and_keeps_recovery_data() {
    use plico::{
        commit_operation,
        git::{Cancel, Repository},
        operation_process::{Progress, StopReason},
    };
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let f = Fixture::new();
    f.base();
    f.write("excluded", "staged\n");
    f.git(&["add", "excluded"]);
    f.write("excluded", "unstaged\n");
    f.write("selected", "chosen\n");
    hook(&f, "pre-commit", "echo commit-hook-ready >&2\nsleep 30");
    let r = request(&f, false);
    let before = r.expected_head.clone();
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let progress = Progress::default();
    let worker_token = token.clone();
    let worker_progress = progress.clone();
    let worker = thread::spawn(move || {
        commit_operation::execute(&repo, &r, &worker_token, &worker_progress)
    });
    let start = Instant::now();
    while !String::from_utf8_lossy(&progress.snapshot().bytes()).contains("commit-hook-ready") {
        if start.elapsed() > Duration::from_secs(10) {
            token.cancel();
            let _ = worker.join();
            panic!("hook did not start");
        }
        thread::sleep(Duration::from_millis(10));
    }
    token.cancel();
    let report = worker.join().unwrap().unwrap();
    assert!(!report.succeeded());
    assert_eq!(report.head.unwrap(), before);
    assert_eq!(
        report
            .steps
            .last()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .stopped,
        Some(StopReason::Cancelled)
    );
    if report.restore.is_err() {
        // A killed Git can leave its lock. The product must not guess ownership
        // and delete a lock; only this fixture knows every writer is terminated.
        let _ = fs::remove_file(f.0.join(".git/index.lock"));
        f.apply(report.recovery_input.unwrap());
    } else {
        assert!(report.recovery_input.is_none());
    }
    assert_eq!(f.git(&["show", ":excluded"]), b"staged\n");
    assert_eq!(fs::read(f.0.join("excluded")).unwrap(), b"unstaged\n");
    assert_no_message_file(&f);
}

#[test]
fn executor_handles_unborn_message_only_amend_and_selected_rename_endpoints() {
    let f = Fixture::new();
    f.write("selected", "initial\n");
    let mut r = request(&f, false);
    assert!(r.expected_head.is_none());
    assert!(execute(&f, &r).succeeded());
    r = request(&f, true);
    r.add.clear();
    r.message = "message-only amend".into();
    assert!(execute(&f, &r).succeeded());
    assert_eq!(f.git(&["rev-list", "--count", "HEAD"]), b"1\n");
    f.git(&["mv", "selected", "Selected"]);
    f.write("Selected", "rename with worktree edit\n");
    r = request(&f, false);
    r.add = vec!["Selected".into()];
    r.remove = vec!["selected".into()];
    assert!(execute(&f, &r).succeeded());
    assert_eq!(f.git(&["ls-tree", "--name-only", "HEAD"]), b"Selected\n");
    assert_eq!(
        f.git(&["show", "HEAD:Selected"]),
        b"rename with worktree edit\n"
    );
    // Pathspec magic must be literal, and deleted selected files stay deleted.
    #[cfg(unix)]
    let literal_name = ":(literal) 中文.txt";
    #[cfg(windows)]
    let literal_name = "[literal] 中文.txt";
    f.write(literal_name, "literal file\n");
    fs::remove_file(f.0.join("Selected")).unwrap();
    r = request(&f, false);
    r.add = vec![literal_name.into()];
    r.remove = vec!["Selected".into()];
    assert!(execute(&f, &r).succeeded());
    assert_eq!(
        f.git(&["show", &format!("HEAD:{literal_name}")]),
        b"literal file\n"
    );
    assert_no_message_file(&f);
}

#[test]
fn executor_classifies_empty_and_signing_failure_without_retrying_unsigned() {
    use plico::commit_operation::Status;
    for signing in [false, true] {
        let f = Fixture::new();
        f.base();
        f.write("excluded", "excluded staged\n");
        f.git(&["add", "excluded"]);
        if signing {
            f.write("selected", "chosen\n");
            f.git(&["config", "commit.gpgsign", "true"]);
            f.git(&["config", "gpg.format", "openpgp"]);
            f.git(&["config", "user.signingkey", "TEST-MISSING-KEY"]);
            f.git(&["config", "gpg.program", "gitui-nonexistent-signing-program"]);
        }
        let r = request(&f, false);
        let report = execute(&f, &r);
        assert_eq!(
            report.status(),
            if signing {
                Status::SigningFailed
            } else {
                Status::Empty
            }
        );
        let feedback = plico::commit_feedback::from_report(&report).unwrap_err();
        assert_eq!(
            feedback.summary,
            if signing {
                "Commit signing failed"
            } else {
                "Nothing to commit"
            }
        );
        assert!(!feedback.details.is_empty());
        assert!(!report.succeeded());
        assert_eq!(report.head.unwrap(), r.expected_head);
        assert_eq!(f.git(&["show", ":excluded"]), b"excluded staged\n");
        assert_no_message_file(&f);
    }
}

#[test]
fn recovery_record_survives_report_drop_and_blocks_overwrite_after_failure() {
    use plico::{
        commit_operation, commit_recovery,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    let f = Fixture::new();
    f.base();
    f.write("excluded", "staged\n");
    f.git(&["add", "excluded"]);
    f.write("excluded", "unstaged\n");
    f.write("selected", "chosen\n");
    hook(&f, "post-commit", "echo test-owned-lock > .git/index.lock");
    let report = execute(&f, &request(&f, false));
    assert!(!report.succeeded());
    assert!(report.recovery_record_error.is_none());
    let path = report.recovery_record.clone().unwrap();
    let bytes = fs::read(&path).unwrap();
    let expected = report.recovery_input.clone().unwrap();
    drop(report);
    // Reconstruct from disk, without retaining the original operation object.
    let record = commit_recovery::read(&f.0.join(".git")).unwrap();
    assert_eq!(record.restore, expected);
    assert!(!record.prepared.is_empty());
    assert!(record.before_head.is_some());
    fs::remove_file(f.0.join(".git/index.lock")).unwrap();
    let current = f.git(&["write-tree"]);
    let error = commit_operation::execute(
        &Repository::new(&f.0),
        &request(&f, true),
        &Cancel::default(),
        &Progress::default(),
    )
    .err()
    .unwrap();
    assert!(error.contains("Pending commit index recovery"));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(f.git(&["write-tree"]), current);
    // Only this isolated fixture knows no other writer changed these paths.
    f.apply(record.restore);
    assert_eq!(f.git(&["show", ":excluded"]), b"staged\n");
    assert_eq!(fs::read(f.0.join("excluded")).unwrap(), b"unstaged\n");
    fs::remove_file(path).unwrap();
}

#[test]
fn successful_restore_removes_journal_even_if_hook_rejects_commit() {
    use plico::commit_recovery;
    for reject in [false, true] {
        let f = Fixture::new();
        f.base();
        f.write("excluded", "staged\n");
        f.git(&["add", "excluded"]);
        f.write("selected", "chosen\n");
        if reject {
            hook(&f, "pre-commit", "exit 1");
        }
        let report = execute(&f, &request(&f, false));
        assert_eq!(report.succeeded(), !reject);
        assert!(report.restore.is_ok());
        assert!(report.recovery_record.is_none() && report.recovery_record_error.is_none());
        assert!(
            commit_recovery::pending(&f.0.join(".git"))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn corrupt_recovery_records_fail_closed_without_mutating_git_state() {
    use plico::{
        commit_operation, commit_recovery,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    let f = Fixture::new();
    f.base();
    let path = commit_recovery::path(&f.0.join(".git"));
    let saved = f.git(&["write-tree"]);
    for bytes in [
        b"truncated".to_vec(),
        [
            b"GitReview commit index recovery v1\n".as_slice(),
            &u64::MAX.to_le_bytes(),
        ]
        .concat(),
    ] {
        fs::write(&path, &bytes).unwrap();
        assert!(commit_recovery::read(&f.0.join(".git")).is_err());
        assert!(
            commit_operation::execute(
                &Repository::new(&f.0),
                &request(&f, true),
                &Cancel::default(),
                &Progress::default()
            )
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(f.git(&["write-tree"]), saved);
    }
}

#[test]
fn crash_recovery_fixture() {
    let Some(path) = std::env::var_os("GITUI_COMMIT_CRASH_FIXTURE") else {
        return;
    };
    let f = Fixture(PathBuf::from(path));
    let _ = execute(&f, &request(&f, false));
    // Parent owns this repository and tears it down, not this child.
    std::mem::forget(f);
    panic!("parent should terminate operation while hook waits");
}

#[test]
fn killed_operation_leaves_complete_record_before_index_mutation() {
    use plico::{commit_recovery, process_tree::ProcessTree};
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let f = Fixture::new();
    f.base();
    f.write("excluded", "saved blob\n");
    f.git(&["add", "excluded"]);
    f.write("excluded", "working copy\n");
    f.write("selected", "chosen\n");
    let before = f.git(&["rev-parse", "HEAD"]);
    hook(&f, "pre-commit", "echo ready > crash-ready\nsleep 30");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "crash_recovery_fixture", "--nocapture"])
        .env("GITUI_COMMIT_CRASH_FIXTURE", &f.0)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut tree = ProcessTree::spawn(&mut command).unwrap();
    let start = Instant::now();
    while !f.0.join("crash-ready").exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "hook never started"
        );
        assert!(
            tree.child.try_wait().unwrap().is_none(),
            "child exited before hook"
        );
        thread::sleep(Duration::from_millis(10));
    }
    // Prove prepare already happened, then kill without execute's restore path.
    assert_eq!(f.git(&["show", ":excluded"]), b"base\n");
    tree.terminate().unwrap();
    let _ = tree.child.wait();
    drop(tree);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    let record = commit_recovery::read(&f.0.join(".git")).unwrap();
    assert!(!record.restore.is_empty());
    let _ = fs::remove_file(f.0.join(".git/index.lock"));
    f.apply(record.restore);
    assert_eq!(f.git(&["show", ":excluded"]), b"saved blob\n");
    assert_eq!(fs::read(f.0.join("excluded")).unwrap(), b"working copy\n");
}

#[test]
fn explicit_recovery_restores_recorded_paths_and_preserves_unrelated_staging() {
    use plico::{
        commit_recovery,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    let f = Fixture::new();
    f.base();
    f.write("excluded", "saved staged\n");
    f.git(&["add", "excluded"]);
    f.write("excluded", "unstaged\n");
    f.write("selected", "chosen\n");
    hook(&f, "post-commit", "echo owned-lock > .git/index.lock");
    let report = execute(&f, &request(&f, false));
    assert!(report.restore.is_err());
    drop(report);
    fs::remove_file(f.0.join(".git/index.lock")).unwrap();
    f.write("unrelated", "new staging\n");
    f.git(&["add", "unrelated"]);
    let before = f.git(&["rev-parse", "HEAD"]);
    commit_recovery::restore(
        &Repository::new(&f.0),
        &Cancel::default(),
        &Progress::default(),
    )
    .unwrap();
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    assert_eq!(f.git(&["show", ":excluded"]), b"saved staged\n");
    assert_eq!(f.git(&["show", ":unrelated"]), b"new staging\n");
    assert_eq!(fs::read(f.0.join("excluded")).unwrap(), b"unstaged\n");
    assert!(
        commit_recovery::pending(&f.0.join(".git"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn explicit_recovery_refuses_new_affected_content_and_keeps_record_on_lock_or_cancel() {
    use plico::{
        commit_recovery,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    for mode in ["changed", "lock", "cancel", "descendant"] {
        let f = Fixture::new();
        f.base();
        if mode == "descendant" {
            f.git(&["rm", "excluded"]);
            f.git(&["commit", "-m", "remove base path"]);
        }
        f.write("excluded", "saved staged\n");
        f.git(&["add", "excluded"]);
        f.write("selected", "chosen\n");
        hook(&f, "post-commit", "echo owned-lock > .git/index.lock");
        let report = execute(&f, &request(&f, false));
        let path = report.recovery_record.unwrap();
        fs::remove_file(f.0.join(".git/index.lock")).unwrap();
        let token = Cancel::default();
        match mode {
            "changed" => {
                f.write("excluded", "new staged content\n");
                f.git(&["add", "excluded"]);
            }
            "descendant" => {
                f.git(&["rm", "--ignore-unmatch", "--cached", "excluded"]);
                fs::remove_file(f.0.join("excluded")).unwrap();
                fs::create_dir(f.0.join("excluded")).unwrap();
                f.write("excluded/new", "new child");
                f.git(&["add", "excluded/new"]);
            }
            "cancel" => token.cancel(),
            _ => {}
        }
        let tree = f.git(&["write-tree"]);
        let record = fs::read(&path).unwrap();
        if mode == "lock" {
            f.write(".git/index.lock", "owned-lock");
        }
        let error = commit_recovery::restore(&Repository::new(&f.0), &token, &Progress::default())
            .unwrap_err();
        if mode == "descendant" {
            assert!(error.contains("overlaps"), "{error}");
        }
        if mode == "lock" {
            fs::remove_file(f.0.join(".git/index.lock")).unwrap();
        }
        assert_eq!(f.git(&["write-tree"]), tree, "{mode}");
        assert_eq!(fs::read(path).unwrap(), record, "{mode}");
    }
}

#[test]
fn amend_message_load_uses_full_utf8_message_and_rejects_stale_head() {
    use plico::{
        commit_draft,
        git::{Cancel, Repository},
    };
    let f = Fixture::new();
    f.base();
    f.git(&["commit", "--amend", "-m", "标题\n\n正文一\n正文二"]);
    let oid = String::from_utf8(f.git(&["rev-parse", "HEAD"]))
        .unwrap()
        .trim()
        .to_owned();
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let before = f.git(&["write-tree"]);
    let message = commit_draft::load_amend_message(&repo, &oid, &token).unwrap();
    assert_eq!(message, "标题\n\n正文一\n正文二\n");
    assert_eq!(f.git(&["write-tree"]), before);
    f.write("selected", "next");
    f.git(&["commit", "-am", "next"]);
    assert!(
        commit_draft::load_amend_message(&repo, &oid, &token)
            .unwrap_err()
            .contains("HEAD changed")
    );
    token.cancel();
    assert!(commit_draft::load_amend_message(&repo, &oid, &token).is_err());
}

#[test]
fn inclusion_from_real_status_commits_whole_file_and_excludes_review_only_files() {
    use plico::{
        commit_changes::Snapshot,
        commit_inclusion::{Identity, Inclusion},
        commit_operation,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    let f = Fixture::new();
    f.base();
    f.write("selected", "staged selected\n");
    f.git(&["add", "selected"]);
    f.write("selected", "working selected\n");
    f.write("excluded", "staged excluded\n");
    f.git(&["add", "excluded"]);
    f.write("excluded", "working excluded\n");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let changes = repo.local_changes(&token).unwrap();
    assert_eq!(
        changes
            .iter()
            .filter(|c| c.path == std::path::Path::new("selected"))
            .count(),
        2
    );
    let snapshot = Snapshot::load(&repo, &token).unwrap();
    let selected = Identity::of(
        snapshot
            .files
            .iter()
            .find(|file| file.change().path == std::path::Path::new("selected"))
            .unwrap(),
    );
    let mut s = Inclusion::default();
    s.replace(&snapshot);
    s.set(&selected, true);
    assert_eq!(s.count(), 1);
    let r = s.request("chosen file".into(), false).unwrap();
    let report = commit_operation::execute(&repo, &r, &token, &Progress::default()).unwrap();
    assert!(report.succeeded());
    assert_eq!(f.git(&["show", "HEAD:selected"]), b"working selected\n");
    assert_eq!(f.git(&["show", "HEAD:excluded"]), b"base\n");
    assert_eq!(f.git(&["show", ":excluded"]), b"staged excluded\n");
    assert_eq!(
        fs::read(f.0.join("excluded")).unwrap(),
        b"working excluded\n"
    );
}

#[test]
fn inclusion_uses_recreated_file_instead_of_its_staged_deletion() {
    use plico::{
        commit_changes::{File, Snapshot},
        commit_inclusion::{Identity, Inclusion},
        commit_operation,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    let f = Fixture::new();
    f.base();
    f.git(&["rm", "selected"]);
    f.write("selected", "recreated\n");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let snapshot = Snapshot::load(&repo, &token).unwrap();
    let recreated = Identity::of(
        snapshot
            .files
            .iter()
            .find(|file| matches!(file, File::Unversioned(_)))
            .unwrap(),
    );
    let mut s = Inclusion::default();
    s.replace(&snapshot);
    s.set(&recreated, true);
    let r = s.request("recreated".into(), false).unwrap();
    assert!(r.remove.is_empty());
    assert!(
        commit_operation::execute(&repo, &r, &token, &Progress::default())
            .unwrap()
            .succeeded()
    );
    assert_eq!(f.git(&["show", "HEAD:selected"]), b"recreated\n");
}

#[test]
fn net_working_tree_review_combines_versions_and_omits_reverted_changes() {
    use plico::git::{Cancel, Repository};
    let f = Fixture::new();
    f.base();
    f.write("selected", "staged\n");
    f.git(&["add", "selected"]);
    f.write("selected", "staged\nunstaged\n");
    f.write("excluded", "temporary staged\n");
    f.git(&["add", "excluded"]);
    f.write("excluded", "base\n");
    f.write("gone", "added\n");
    f.git(&["add", "gone"]);
    fs::remove_file(f.0.join("gone")).unwrap();
    let before = f.git(&["ls-files", "--stage", "-z"]);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let changes = repo.working_tree_changes(&token).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, PathBuf::from("selected"));
    let patch = repo.working_tree_patch(&changes[0], true, &token).unwrap();
    assert!(
        patch.contains("-base\n") && patch.contains("+staged\n") && patch.contains("+unstaged\n"),
        "{patch}"
    );
    assert_eq!(before, f.git(&["ls-files", "--stage", "-z"]));
}

#[test]
fn net_working_tree_review_supports_unborn_and_metadata_only_rename() {
    use plico::git::{Cancel, Repository};
    let f = Fixture::new();
    f.write("new", "first\n");
    f.git(&["add", "new"]);
    f.write("new", "first\nsecond\n");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let changes = repo.working_tree_changes(&token).unwrap();
    assert_eq!(changes.len(), 1);
    let patch = repo.working_tree_patch(&changes[0], true, &token).unwrap();
    assert!(
        patch.contains("+first\n") && patch.contains("+second\n"),
        "{patch}"
    );
    f.git(&["add", "new"]);
    f.git(&["commit", "-m", "base"]);
    f.git(&["mv", "new", "renamed"]);
    let changes = repo.working_tree_changes(&token).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].old_path, Some("new".into()));
    let patch = repo.working_tree_patch(&changes[0], true, &token).unwrap();
    assert!(
        patch.contains(" first\n") && patch.contains(" second\n"),
        "{patch}"
    );
}

#[test]
fn net_working_tree_review_uses_literal_paths_and_cancels_without_writing() {
    use plico::git::{Cancel, Repository};
    let f = Fixture::new();
    f.write("[review].txt", "base\n");
    f.write("r.txt", "other\n");
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "base"]);
    f.write("[review].txt", "chosen\n");
    f.write("r.txt", "not chosen\n");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let changes = repo.working_tree_changes(&token).unwrap();
    let chosen = changes
        .iter()
        .find(|c| c.path == PathBuf::from("[review].txt"))
        .unwrap();
    let patch = repo.working_tree_patch(chosen, true, &token).unwrap();
    assert!(patch.contains("+chosen\n"));
    assert!(!patch.contains("not chosen"));
    let before = f.git(&["ls-files", "--stage", "-z"]);
    token.cancel();
    assert!(repo.working_tree_changes(&token).is_err());
    assert!(repo.working_tree_patch(chosen, true, &token).is_err());
    assert_eq!(before, f.git(&["ls-files", "--stage", "-z"]));
}

#[test]
fn commit_collector_merges_versions_and_preserves_rename_deletion_origin() {
    use plico::{
        commit_changes::{File, Snapshot},
        git::{Cancel, Repository},
    };
    let f = Fixture::new();
    f.base();
    f.write("selected", "staged\n");
    f.git(&["add", "selected"]);
    f.write("selected", "final\n");
    f.git(&["mv", "excluded", "renamed"]);
    fs::remove_file(f.0.join("renamed")).unwrap();
    f.write("unversioned", "new\n");
    let before = f.git(&["ls-files", "--stage", "-z"]);
    let snapshot = Snapshot::load(&Repository::new(&f.0), &Cancel::default()).unwrap();
    let tracked: Vec<_> = snapshot
        .files
        .iter()
        .filter_map(|f| {
            if let File::Tracked(c) = f {
                Some(c)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(tracked.len(), 2);
    assert!(
        tracked
            .iter()
            .any(|c| c.path == PathBuf::from("selected") && c.status == "M")
    );
    assert!(
        tracked
            .iter()
            .any(|c| c.path == PathBuf::from("excluded") && c.status == "D")
    );
    assert!(
        snapshot
            .review_versions
            .iter()
            .any(|c| c.path == PathBuf::from("renamed") && c.old_path == Some("excluded".into()))
    );
    assert!(
        snapshot
            .files
            .iter()
            .any(|f| matches!(f,File::Unversioned(c) if c.path==PathBuf::from("unversioned")))
    );
    assert_eq!(before, f.git(&["ls-files", "--stage", "-z"]));
}

#[test]
fn commit_collector_keeps_recreated_unversioned_distinct_from_staged_deletion() {
    use plico::{
        commit_changes::{File, Snapshot},
        git::{Cancel, Repository},
    };
    let f = Fixture::new();
    f.base();
    f.git(&["rm", "--cached", "selected"]);
    let snapshot = Snapshot::load(&Repository::new(&f.0), &Cancel::default()).unwrap();
    assert!(snapshot.files.iter().any(
        |f| matches!(f,File::Tracked(c) if c.path==PathBuf::from("selected") && c.status=="D")
    ));
    assert!(
        snapshot
            .files
            .iter()
            .any(|f| matches!(f,File::Unversioned(c) if c.path==PathBuf::from("selected")))
    );
}

#[test]
fn commit_collector_omits_reverted_changes_and_keeps_conflicts_out_of_tracked_group() {
    use plico::{
        commit_changes::{File, Snapshot},
        git::{Cancel, Repository},
    };
    let f = Fixture::new();
    f.base();
    f.write("selected", "staged\n");
    f.git(&["add", "selected"]);
    f.write("selected", "base\n");
    f.write("temporary", "added\n");
    f.git(&["add", "temporary"]);
    fs::remove_file(f.0.join("temporary")).unwrap();
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    assert!(Snapshot::load(&repo, &token).unwrap().files.is_empty());
    // All resets and merge writes here belong to this disposable fixture.
    f.git(&["reset", "--hard", "HEAD"]);
    f.git(&["checkout", "-b", "side"]);
    f.write("selected", "side\n");
    f.git(&["commit", "-am", "side"]);
    f.git(&["checkout", "main"]);
    f.write("selected", "main\n");
    f.git(&["commit", "-am", "main"]);
    assert!(
        !f.command()
            .args(["merge", "side"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let before = f.git(&["ls-files", "--stage", "-z"]);
    let snapshot = Snapshot::load(&repo, &token).unwrap();
    assert_eq!(snapshot.files.len(), 1);
    assert!(matches!(&snapshot.files[0],File::Conflict(c) if c.path==PathBuf::from("selected")));
    assert_eq!(before, f.git(&["ls-files", "--stage", "-z"]));
}

#[test]
fn commit_inclusion_deletion_preserves_unselected_rename_destination_in_index() {
    use plico::{
        commit_changes::{File, Snapshot},
        commit_inclusion::{Identity, Inclusion},
        git::{Cancel, Repository},
    };
    let f = Fixture::new();
    f.base();
    f.git(&["mv", "selected", "renamed"]);
    fs::remove_file(f.0.join("renamed")).unwrap();
    let repo = Repository::new(&f.0);
    let snapshot = Snapshot::load(&repo, &Cancel::default()).unwrap();
    let id = Identity::of(
        snapshot
            .files
            .iter()
            .find(|f| matches!(f,File::Tracked(c) if c.path==PathBuf::from("selected")))
            .unwrap(),
    );
    let mut included = Inclusion::default();
    included.replace(&snapshot);
    included.set(&id, true);
    let request = included.request("delete original".into(), false).unwrap();
    assert_eq!(request.remove, vec![PathBuf::from("selected")]);
    assert!(request.add.is_empty());
    assert!(execute(&f, &request).succeeded());
    assert_eq!(f.git(&["ls-tree", "--name-only", "HEAD"]), b"excluded\n");
    assert_eq!(f.git(&["show", ":renamed"]), b"base\n");
    assert!(!f.0.join("renamed").exists());
}

#[test]
fn commit_inclusion_distinguishes_deletion_from_same_path_unversioned_file() {
    use plico::{
        commit_changes::{File, Snapshot},
        commit_inclusion::{Identity, Inclusion},
        git::{Cancel, Repository},
    };
    let f = Fixture::new();
    f.base();
    f.git(&["rm", "--cached", "selected"]);
    f.write("selected", "recreated\n");
    let repo = Repository::new(&f.0);
    let snapshot = Snapshot::load(&repo, &Cancel::default()).unwrap();
    let deletion = Identity::of(
        snapshot
            .files
            .iter()
            .find(|f| matches!(f, File::Tracked(_)))
            .unwrap(),
    );
    let unversioned = Identity::of(
        snapshot
            .files
            .iter()
            .find(|f| matches!(f, File::Unversioned(_)))
            .unwrap(),
    );
    let mut included = Inclusion::default();
    included.replace(&snapshot);
    included.set(&deletion, true);
    assert!(!included.included(&unversioned));
    let request = included.request("delete only".into(), false).unwrap();
    assert!(execute(&f, &request).succeeded());
    assert_eq!(fs::read(f.0.join("selected")).unwrap(), b"recreated\n");
    assert_eq!(f.git(&["ls-tree", "--name-only", "HEAD"]), b"excluded\n");
    included.replace(&Snapshot::load(&repo, &Cancel::default()).unwrap());
    assert_eq!(included.count(), 0);
    included.set(&unversioned, true);
    assert!(
        execute(
            &f,
            &included.request("include recreated".into(), false).unwrap()
        )
        .succeeded()
    );
    assert_eq!(f.git(&["show", "HEAD:selected"]), b"recreated\n");
}

#[test]
fn commit_inclusion_scope_add_wins_and_refresh_preserves_only_same_identity() {
    use plico::{
        commit_changes::{File, Snapshot},
        commit_inclusion::{Identity, Inclusion},
        git::{Cancel, Repository},
    };
    let f = Fixture::new();
    f.base();
    f.git(&["rm", "--cached", "selected"]);
    f.write("selected", "recreated\n");
    f.write("excluded", "keep staged\n");
    f.git(&["add", "excluded"]);
    f.write("excluded", "keep working\n");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let snapshot = Snapshot::load(&repo, &token).unwrap();
    let ids: HashSet<_> = snapshot
        .files
        .iter()
        .filter(|f| match f {
            File::Tracked(c) => c.path == PathBuf::from("selected"),
            File::Unversioned(c) => c.path == PathBuf::from("selected"),
            _ => false,
        })
        .map(Identity::of)
        .collect();
    assert_eq!(ids.len(), 2);
    let mut included = Inclusion::default();
    included.replace(&snapshot);
    included.set_scope(&ids, true);
    included.replace(&Snapshot::load(&repo, &token).unwrap());
    assert_eq!(included.scope_counts(&ids), (2, 2));
    let request = included.request("recreate".into(), false).unwrap();
    assert_eq!(request.add, vec![PathBuf::from("selected")]);
    assert!(request.remove.is_empty());
    assert!(execute(&f, &request).succeeded());
    assert_eq!(f.git(&["show", "HEAD:selected"]), b"recreated\n");
    assert_eq!(f.git(&["show", "HEAD:excluded"]), b"base\n");
    assert_eq!(f.git(&["show", ":excluded"]), b"keep staged\n");
    assert_eq!(fs::read(f.0.join("excluded")).unwrap(), b"keep working\n");
    included.replace(&Snapshot::load(&repo, &token).unwrap());
    assert_eq!(included.count(), 0);
    assert!(included.request("nothing selected".into(), false).is_err());
}

#[test]
fn commit_tree_bridge_groups_net_changes_and_opens_full_working_tree_target() {
    use plico::{
        commit_changes::{File, Snapshot},
        diff_target::{Chain, Target},
        git::{Area, Cancel, Repository},
        local_changes::{Model, Row},
    };
    let f = Fixture::new();
    f.base();
    f.write("selected", "staged\n");
    f.git(&["add", "selected"]);
    f.write("selected", "staged\nunstaged\n");
    f.write("new", "new file\n");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let snapshot = Snapshot::load(&repo, &token).unwrap();
    let mut model = Model::default();
    model.replace(snapshot.files.iter().map(File::change).collect());
    assert_eq!(model.files().len(), 2);
    assert_eq!(
        model
            .rows()
            .iter()
            .filter(|r| matches!(r, Row::Group { .. }))
            .count(),
        2
    );
    assert!(
        !model
            .files()
            .iter()
            .any(|f| matches!(f.area, Area::Staged | Area::Unstaged))
    );
    let target = Target::new(repo, None, model.files()[0].clone()).unwrap();
    assert_eq!(target.identity().area, Area::WorkingTree);
    let patch = target.read_full(&token).unwrap();
    assert!(patch.contains("+staged\n") && patch.contains("+unstaged\n"));
    assert_eq!(Chain::new(&target, model.files()).unwrap().len(), 1);
    assert_eq!(model.selected(), None); // Preview fallback must not imply a tree or commit selection.
}

#[test]
fn executor_ignores_stash_display_configuration_and_preserves_staging() {
    for amend in [false, true] {
        let f = Fixture::new();
        f.base();
        f.write("selected", "saved work\n");
        f.git(&["stash", "push", "-qm", "saved work"]);
        let stash = f.git(&["rev-parse", "refs/stash"]);
        f.git(&["config", "status.showStash", "true"]);
        f.write("selected", "chosen\n");
        f.write("excluded", "excluded staged\n");
        f.git(&["add", "excluded"]);
        let report = execute(&f, &request(&f, amend));
        assert!(report.succeeded());
        assert_eq!(f.git(&["show", "HEAD:selected"]), b"chosen\n");
        assert_eq!(f.git(&["show", "HEAD:excluded"]), b"base\n");
        assert_eq!(f.git(&["show", ":excluded"]), b"excluded staged\n");
        assert_eq!(f.git(&["rev-parse", "refs/stash"]), stash);
    }
}

#[test]
fn explicit_recovery_after_checkout_validates_content_instead_of_only_head() {
    use plico::{
        commit_recovery,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    for changed in [false, true] {
        let f = Fixture::new();
        f.base();
        f.git(&["checkout", "-b", "destination"]);
        if changed {
            f.write("excluded", "destination content\n");
            f.git(&["add", "excluded"]);
        } else {
            f.write("destination-only", "other branch\n");
            f.git(&["add", "destination-only"]);
        }
        f.git(&["commit", "-m", "destination"]);
        f.git(&["checkout", "main"]);
        f.write("excluded", "saved staged\n");
        f.git(&["add", "excluded"]);
        f.write("selected", "chosen\n");
        hook(&f, "post-commit", "echo owned-lock > .git/index.lock");
        let report = execute(&f, &request(&f, false));
        assert!(report.restore.is_err());
        let recovery_path = report.recovery_record.unwrap();
        let record = fs::read(&recovery_path).unwrap();
        fs::remove_file(f.0.join(".git/index.lock")).unwrap();
        // An external Git client can change branches while our recovery record
        // exists. The explicit replay must still inspect the current index.
        f.git(&["checkout", "--force", "destination"]);
        f.write("unrelated", "new staging\n");
        f.git(&["add", "unrelated"]);
        let head = f.git(&["rev-parse", "HEAD"]);
        let before_index = f.git(&["write-tree"]);
        let worktree = fs::read(f.0.join("excluded")).unwrap();
        let result = commit_recovery::restore(
            &Repository::new(&f.0),
            &Cancel::default(),
            &Progress::default(),
        );
        if changed {
            assert!(
                result
                    .unwrap_err()
                    .contains("Affected index content changed")
            );
            assert_eq!(f.git(&["write-tree"]), before_index);
            assert_eq!(fs::read(&recovery_path).unwrap(), record);
        } else {
            result.unwrap();
            assert_eq!(f.git(&["show", ":excluded"]), b"saved staged\n");
            assert!(!recovery_path.exists());
        }
        assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
        assert_eq!(
            f.git(&["symbolic-ref", "HEAD"]),
            b"refs/heads/destination\n"
        );
        assert_eq!(f.git(&["show", ":unrelated"]), b"new staging\n");
        assert_eq!(fs::read(f.0.join("excluded")).unwrap(), worktree);
    }
}

#[test]
fn commit_collector_treats_copied_file_as_addition_even_with_copy_detection_configured() {
    use plico::{
        commit_changes::Snapshot,
        commit_inclusion::{Identity, Inclusion},
        git::{Cancel, Repository},
    };
    let f = Fixture::new();
    f.base();
    f.git(&["config", "diff.renames", "copies"]);
    f.write("copied", "base\n");
    f.git(&["add", "copied"]);
    let repo = Repository::new(&f.0);
    let snapshot = Snapshot::load(&repo, &Cancel::default()).unwrap();
    let copied = snapshot
        .files
        .iter()
        .find(|f| f.change().path == std::path::Path::new("copied"))
        .unwrap();
    let id = Identity::of(copied);
    assert!(id.before.is_none());
    let mut inclusion = Inclusion::default();
    inclusion.replace(&snapshot);
    inclusion.set(&id, true);
    let request = inclusion.request("copy".into(), false).unwrap();
    assert!(request.remove.is_empty());
    assert!(execute(&f, &request).succeeded());
    assert_eq!(f.git(&["show", "HEAD:selected"]), b"base\n");
    assert_eq!(f.git(&["show", "HEAD:copied"]), b"base\n");
}

#[test]
fn explicit_recovery_does_not_replace_a_new_staged_ancestor() {
    use plico::{
        commit_recovery,
        git::{Cancel, Repository},
        operation_process::Progress,
    };
    let f = Fixture::new();
    f.base();
    fs::create_dir(f.0.join("nested")).unwrap();
    f.write("nested/excluded", "saved staged child\n");
    f.git(&["add", "nested/excluded"]);
    f.write("selected", "chosen\n");
    hook(&f, "post-commit", "echo owned-lock > .git/index.lock");
    let report = execute(&f, &request(&f, false));
    assert!(report.restore.is_err());
    let path = report.recovery_record.unwrap();
    fs::remove_file(f.0.join(".git/index.lock")).unwrap();
    // Prepared index has no nested/excluded entry. A later writer stages its
    // parent as a file; replaying the old child would silently remove it.
    fs::remove_file(f.0.join("nested/excluded")).unwrap();
    fs::remove_dir(f.0.join("nested")).unwrap();
    f.write("nested", "new staged parent\n");
    f.git(&["add", "nested"]);
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let record = fs::read(&path).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    let error = commit_recovery::restore(
        &Repository::new(&f.0),
        &Cancel::default(),
        &Progress::default(),
    )
    .unwrap_err();
    assert!(error.contains("overlaps"), "{error}");
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(path).unwrap(), record);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(f.git(&["show", ":nested"]), b"new staged parent\n");
    assert_eq!(
        fs::read(f.0.join("nested")).unwrap(),
        b"new staged parent\n"
    );
}
