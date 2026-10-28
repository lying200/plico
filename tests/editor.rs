use plico::{
    askpass::Server,
    auth_request::{Pending, Requests},
    editor::{self, Document},
    git::Cancel,
};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};
static SEQ: AtomicUsize = AtomicUsize::new(0);
// Must be created inside the scope, after spawning its worker: unwinding then
// requests cancellation before thread::scope waits for that worker to finish.
struct CancelOnDrop<'a>(&'a Cancel);
impl Drop for CancelOnDrop<'_> {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.cancel();
        }
    }
}

#[test]
fn failed_editor_assertion_cancels_before_scope_joins() {
    let token = Cancel::default();
    let worker_observed_cancel = AtomicBool::new(false);
    let result = std::panic::catch_unwind(|| {
        thread::scope(|scope| {
            scope.spawn(|| {
                let deadline = Instant::now() + Duration::from_secs(2);
                while !token.cancelled() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(1));
                }
                worker_observed_cancel.store(token.cancelled(), Ordering::Relaxed);
            });
            let _cancel_on_panic = CancelOnDrop(&token);
            panic!("simulate a failed readiness assertion");
        });
    });
    assert!(result.is_err());
    assert!(token.cancelled());
    assert!(worker_observed_cancel.load(Ordering::Relaxed));
    let normal = Cancel::default();
    drop(CancelOnDrop(&normal));
    assert!(
        !normal.cancelled(),
        "successful scopes must preserve their token"
    );
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let f = Self(std::env::temp_dir().join(format!(
            "gitui editor 中文 {} {}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir_all(&f.0).unwrap();
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Fixture"]);
        f.git(&["config", "user.email", "fixture@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let result = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap().trim().into()
    }
    fn helper(&self) -> PathBuf {
        let src = std::env::var_os("GITUI_TEST_EDITOR_EXE")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_plico-editor").into());
        let path = self.0.join(if cfg!(windows) {
            "editor helper.exe"
        } else {
            "editor helper"
        });
        // Keep the quoted Unicode/spaced executable path without creating a
        // writable executable inode while other tests concurrently spawn Git.
        // A Unix child can retain such a write handle until exec (ETXTBSY).
        #[cfg(unix)]
        std::os::unix::fs::symlink(fs::canonicalize(src).unwrap(), &path).unwrap();
        #[cfg(not(unix))]
        fs::copy(src, &path).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn pending(requests: &Requests) -> Pending {
    let started = Instant::now();
    loop {
        if let Some(pending) = requests.take_pending() {
            return pending;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn real_git_editor_saves_multiline_unicode_and_cancel_preserves_head() {
    let f = Fixture::new();
    let helper = f.helper();
    let cancel = Cancel::default();
    let requests = Requests::new(cancel.clone());
    let server = Server::start(requests.clone()).unwrap();
    for accept in [true, false] {
        let mut command = Command::new("git");
        server.configure(&mut command);
        command
            .arg("-C")
            .arg(&f.0)
            .args(["commit", "--allow-empty", "--edit", "-m", "initial"])
            .env("GIT_EDITOR", editor::command(&helper, false).unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().unwrap();
        let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
        assert!(!document.sequence);
        assert!(document.text().contains("initial"));
        if accept {
            document.save("中文提交\n\nDetailed message\n").unwrap();
        }
        drop(document);
        let result = child.wait_with_output().unwrap();
        assert_eq!(
            result.status.success(),
            accept,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!cancel.cancelled());
        assert_eq!(
            f.git(&["log", "-1", "--format=%B"]),
            "中文提交\n\nDetailed message"
        );
        assert_eq!(f.git(&["rev-list", "--count", "HEAD"]), "1");
    }
}
#[test]
fn editor_rejects_external_files_and_changed_content() {
    let f = Fixture::new();
    let helper = f.helper();
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    for outside in [true, false] {
        let path = f.0.join(if outside {
            "outside.txt"
        } else {
            ".git/COMMIT_EDITMSG"
        });
        fs::write(&path, "original").unwrap();
        let mut command = Command::new(&helper);
        server.configure(&mut command);
        let child = command.arg(&path).spawn().unwrap();
        let document = Document::open(pending(&requests), &f.0.join(".git"));
        if outside {
            assert!(document.is_err());
        } else {
            let mut document = document.unwrap();
            fs::write(&path, "externally changed").unwrap();
            assert!(document.save("replacement").is_err());
            drop(document);
        }
        assert!(!child.wait_with_output().unwrap().status.success());
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            if outside {
                "original"
            } else {
                "externally changed"
            }
        );
    }
}

#[test]
fn editor_compares_external_changes_as_bytes_before_writing() {
    let f = Fixture::new();
    let helper = f.helper();
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    let path = f.0.join(".git/COMMIT_EDITMSG");
    fs::write(&path, "original").unwrap();
    let mut command = Command::new(&helper);
    server.configure(&mut command);
    let child = command.arg(&path).spawn().unwrap();
    let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
    let external = b"external caf\xe9\n";
    fs::write(&path, external).unwrap();
    let error = document.save("replacement").unwrap_err();
    assert_eq!(error.to_string(), "Editor file changed while editing");
    assert_eq!(fs::read(&path).unwrap(), external);
    drop(document);
    assert!(!child.wait_with_output().unwrap().status.success());
}

#[test]
fn real_git_sequence_editor_receives_todo_and_preserves_commit_order() {
    let f = Fixture::new();
    let helper = f.helper();
    f.git(&["commit", "--allow-empty", "-m", "first"]);
    f.git(&["commit", "--allow-empty", "-m", "second"]);
    let before = f.git(&["rev-parse", "HEAD"]);
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    let mut command = Command::new("git");
    server.configure(&mut command);
    command
        .arg("-C")
        .arg(&f.0)
        .args(["rebase", "-i", "HEAD~1"])
        .env(
            "GIT_SEQUENCE_EDITOR",
            editor::command(&helper, true).unwrap(),
        )
        .env("GIT_EDITOR", editor::command(&helper, false).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().unwrap();
    let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
    assert!(document.sequence);
    assert!(document.text().contains("second"));
    let text = document.text().to_owned();
    document.save(&text).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
}

#[test]
fn ended_session_cannot_write_from_stale_editor() {
    let f = Fixture::new();
    let helper = f.helper();
    let path = f.0.join(".git/COMMIT_EDITMSG");
    fs::write(&path, "original").unwrap();
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    let mut command = Command::new(&helper);
    server.configure(&mut command);
    let child = command.arg(&path).spawn().unwrap();
    let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
    drop(server);
    assert!(!child.wait_with_output().unwrap().status.success());
    assert!(document.save("late edit").is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "original");
}

#[test]
fn merge_update_routes_editor_and_preserves_saved_changes_on_rejection() {
    use plico::{
        git::Repository,
        merge_update::{self, Request},
        operation_state::Operation,
        pull::Target,
    };
    for accept in [true, false] {
        let remote = Fixture::new();
        fs::write(remote.0.join("local"), "base\n").unwrap();
        remote.git(&["add", "."]);
        remote.git(&["commit", "-m", "base"]);
        let f = Fixture::new();
        f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
        f.git(&["fetch", "origin"]);
        f.git(&["reset", "--hard", "origin/main"]);
        f.git(&["branch", "--set-upstream-to=origin/main", "main"]);
        fs::write(remote.0.join("incoming"), "incoming\n").unwrap();
        remote.git(&["add", "."]);
        remote.git(&["commit", "-m", "incoming"]);
        f.git(&["commit", "--allow-empty", "-m", "ours"]);
        f.git(&["config", "branch.main.mergeOptions", "--edit"]);
        let before = f.git(&["rev-parse", "HEAD"]);
        let incoming = remote.git(&["rev-parse", "HEAD"]);
        fs::write(f.0.join("local"), "valuable staged\n").unwrap();
        f.git(&["add", "local"]);
        let helper = f.helper();
        let askpass = PathBuf::from(
            std::env::var_os("GITUI_TEST_ASKPASS_EXE")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_plico-askpass").into()),
        );
        let cancel = Cancel::default();
        let requests = Requests::new(cancel.clone());
        let worker_requests = requests.clone();
        let worker_cancel = cancel.clone();
        let repo = Repository::new(&f.0);
        let request = Request {
            branch: "main".into(),
            head: before.clone(),
            target: Target {
                remote: "origin".into(),
                branch: "main".into(),
            },
            reinstate_index: true,
        };
        let worker = thread::spawn(move || {
            merge_update::execute(
                &repo,
                &request,
                &editor::command(&helper, false).unwrap(),
                &worker_cancel,
                &Default::default(),
                Some((&worker_requests, &askpass)),
            )
            .unwrap()
        });
        let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
        assert!(!document.sequence);
        assert!(document.text().contains("Merge"));
        if accept {
            document
                .save("合并更新\n\nReviewed incoming changes\n")
                .unwrap();
        }
        drop(document);
        let report = worker.join().unwrap();
        assert!(!cancel.cancelled());
        assert!(requests.take_pending().is_none());
        assert_eq!(report.ready_to_push(), accept);
        if accept {
            assert_eq!(
                f.git(&["log", "-1", "--format=%B"]),
                "合并更新\n\nReviewed incoming changes"
            );
            assert_eq!(f.git(&["rev-parse", "HEAD^1"]), before);
            assert_eq!(f.git(&["rev-parse", "HEAD^2"]), incoming);
            assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "local");
            assert_eq!(
                fs::read_to_string(f.0.join("local")).unwrap(),
                "valuable staged\n"
            );
            assert!(report.saved_oid.is_none());
        } else {
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
            assert_eq!(report.state.unwrap().operation, Operation::Merge);
            assert!(report.saved_oid.is_some());
            assert_eq!(f.git(&["show", "refs/stash:local"]), "valuable staged");
        }
    }
}

#[test]
fn rebase_continuation_editor_can_reject_then_retry_without_losing_resolution() {
    use plico::{
        git::Repository,
        pull::Target,
        update_session::{Session, UpdateResult},
    };
    let f = Fixture::new();
    fs::write(f.0.join("shared"), "base\n").unwrap();
    fs::write(f.0.join("dirty"), "base\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "base"]);
    f.git(&["checkout", "-b", "incoming"]);
    fs::write(f.0.join("shared"), "incoming\n").unwrap();
    f.git(&["commit", "-am", "incoming"]);
    let incoming = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("shared"), "ours\n").unwrap();
    f.git(&["commit", "-am", "ours"]);
    f.git(&["remote", "add", "origin", "https://example.invalid/test"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &incoming]);
    fs::write(f.0.join("dirty"), "valuable staged\n").unwrap();
    f.git(&["add", "dirty"]);
    let helper = f.helper();
    let repo = Repository::new(&f.0);
    let cancel = Cancel::default();
    let requests = Requests::new(cancel.clone());
    let command = editor::command(&helper, false).unwrap();
    let mut session = Session::begin(&repo, &cancel).unwrap();
    assert!(
        session
            .save("test rebase", &cancel, &Default::default())
            .unwrap()
            .status
            .success()
    );
    let step = session
        .rebase_with_requests(
            &Target {
                remote: "origin".into(),
                branch: "main".into(),
            },
            &command,
            &cancel,
            &Default::default(),
            Some(&requests),
        )
        .unwrap();
    assert!(!step.outcome.status.success());
    assert_eq!(step.state.unwrap().conflicts.len(), 1);
    fs::write(f.0.join("shared"), "resolved\n").unwrap();
    f.git(&["add", "shared"]);
    let paused_head = f.git(&["rev-parse", "HEAD"]);
    for accept in [false, true] {
        let step = thread::scope(|scope| {
            let worker = scope.spawn(|| {
                session
                    .continue_rebase_with_requests(
                        &command,
                        &cancel,
                        &Default::default(),
                        Some(&requests),
                    )
                    .unwrap()
            });
            let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
            assert!(!document.sequence);
            assert!(document.text().contains("ours"));
            if accept {
                document.save("解决冲突\n\nReviewed resolution\n").unwrap();
            }
            drop(document);
            worker.join().unwrap()
        });
        assert_eq!(step.outcome.status.success(), accept);
        assert_eq!(step.state.unwrap().in_progress(), !accept);
        assert!(!cancel.cancelled());
        assert!(requests.take_pending().is_none());
        assert!(step.cancellation_abort.is_none());
        assert_eq!(
            fs::read_to_string(f.0.join("shared")).unwrap(),
            "resolved\n"
        );
        if !accept {
            assert_eq!(f.git(&["rev-parse", "HEAD"]), paused_head);
            assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "shared");
            assert!(session.saved_oid().is_some());
        }
    }
    assert_eq!(f.git(&["rev-parse", "HEAD^"]), incoming);
    assert_eq!(
        f.git(&["log", "-1", "--format=%B"]),
        "解决冲突\n\nReviewed resolution"
    );
    assert!(
        session
            .finish(UpdateResult::ResolvedConflicts, true, &Default::default())
            .unwrap()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "dirty");
    assert_eq!(
        fs::read_to_string(f.0.join("dirty")).unwrap(),
        "valuable staged\n"
    );
    assert!(session.saved_oid().is_none());
}

#[test]
fn squash_plan_replays_recent_middle_and_gapped_selections_and_leaves_conflicts_recoverable() {
    use plico::squash_plan::Plan;
    for mode in ["recent", "middle", "gapped", "conflict"] {
        let f = Fixture::new();
        let helper = f.helper();
        fs::write(f.0.join("shared"), "base\n").unwrap();
        f.git(&["add", "."]);
        f.git(&["commit", "-m", "base"]);
        let base = f.git(&["rev-parse", "HEAD"]);
        let mut ids = Vec::new();
        for i in 0..5 {
            fs::write(f.0.join(format!("file{i}")), format!("value{i}\n")).unwrap();
            if mode == "conflict" {
                fs::write(f.0.join("shared"), format!("step{i}\n")).unwrap();
            }
            // Add only fixture files: the copied helper is intentionally untracked.
            f.git(&["add", "shared", &format!("file{i}")]);
            f.git(&["commit", "-m", &format!("commit{i}")]);
            ids.push(f.git(&["rev-parse", "HEAD"]));
        }
        let head = f.git(&["rev-parse", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        let index = f.git(&["ls-files", "--stage"]);
        let status = f.git(&["status", "--porcelain"]);
        let selection = match mode {
            "recent" => vec![4, 3, 2],
            "middle" => vec![3, 2, 1],
            _ => vec![4, 2, 0],
        };
        let selected = selection
            .iter()
            .map(|&i| ids[i].clone())
            .collect::<Vec<_>>();
        let plan = Plan::new(&ids, &selected).unwrap();
        let requests = Requests::new(Cancel::default());
        let server = Server::start(requests.clone()).unwrap();
        let mut command = Command::new("git");
        server.configure(&mut command);
        command
            .arg("-C")
            .arg(&f.0)
            .args(["rebase", "-i", "--no-autosquash", "--no-update-refs", &base])
            .env(
                "GIT_SEQUENCE_EDITOR",
                editor::command(&helper, true).unwrap(),
            )
            .env("GIT_EDITOR", editor::command(&helper, false).unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().unwrap();
        let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
        assert!(document.sequence);
        let rewritten = plan.rewrite_todo(document.text(), "#").unwrap();
        let root_index = *selection.iter().min().unwrap();
        plico::squash_message::install(
            &f.0.join(".git"),
            &head,
            &base,
            &format!("commit{root_index}"),
            "压缩提交\n\nreviewed message\n",
        )
        .unwrap();
        assert!(!plico::squash_message::apply(&mut document).unwrap()); // sequence request is never a message
        document.save(&rewritten).unwrap();
        let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
        assert!(!document.sequence);
        assert!(plico::squash_message::apply(&mut document).unwrap());
        let result = child.wait_with_output().unwrap();
        if mode == "conflict" {
            assert!(!result.status.success());
            assert!(f.0.join(".git/rebase-merge").is_dir());
            assert!(!f.git(&["ls-files", "--unmerged"]).is_empty());
            f.git(&["rebase", "--abort"]);
            assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
            assert_eq!(f.git(&["ls-files", "--stage"]), index);
        } else {
            assert!(
                result.status.success(),
                "{mode}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                f.git(&["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]),
                match mode {
                    "recent" => "commit0\ncommit1\n压缩提交",
                    "middle" => "commit0\n压缩提交\ncommit4",
                    _ => "压缩提交\ncommit1\ncommit3",
                }
            );
            // Inspect the squash commit itself, not just the final tree.
            let squash = match mode {
                "recent" => "HEAD",
                "middle" => "HEAD^",
                _ => "HEAD~2",
            };
            let files = f.git(&["diff-tree", "--no-commit-id", "--name-only", "-r", squash]);
            let mut wanted = selection.clone();
            wanted.sort();
            assert_eq!(
                files,
                wanted
                    .iter()
                    .map(|i| format!("file{i}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            assert_eq!(
                f.git(&["show", "-s", "--format=%B", squash]),
                "压缩提交\n\nreviewed message"
            );
            assert!(!f.0.join(".git/rebase-merge").exists());
        }
        assert_eq!(f.git(&["rev-parse", "HEAD^{tree}"]), tree);
        assert_eq!(f.git(&["status", "--porcelain"]), status);
        assert!(!f.0.join(".git/rebase-merge").exists()); // mapping follows completion/abort lifecycle
    }
}

#[test]
fn squash_todo_rejection_leaves_history_and_files_unchanged() {
    use plico::squash_plan::Plan;
    for mode in ["missing", "reordered", "unexpected-command"] {
        let f = Fixture::new();
        let helper = f.helper();
        fs::write(f.0.join("a"), "base").unwrap();
        f.git(&["add", "a"]);
        f.git(&["commit", "-m", "base"]);
        let base = f.git(&["rev-parse", "HEAD"]);
        let mut ids = Vec::new();
        for text in ["first", "second", "third"] {
            fs::write(f.0.join("a"), text).unwrap();
            f.git(&["add", "a"]);
            f.git(&["commit", "-m", text]);
            ids.push(f.git(&["rev-parse", "HEAD"]));
        }
        let plan = Plan::new(&ids, &ids[..2]).unwrap();
        let head = f.git(&["rev-parse", "HEAD"]);
        let status = f.git(&["status", "--porcelain"]);
        let index = f.git(&["ls-files", "--stage"]);
        let token = Cancel::default();
        let requests = Requests::new(token.clone());
        let server = Server::start(requests.clone()).unwrap();
        let mut command = Command::new("git");
        server.configure(&mut command);
        let child = command
            .arg("-C")
            .arg(&f.0)
            .args(["rebase", "-i", &base])
            .env(
                "GIT_SEQUENCE_EDITOR",
                editor::command(&helper, true).unwrap(),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
        let original = document.text();
        assert!(plan.rewrite_todo(original, "#").is_ok());
        let mut entries = original
            .lines()
            .filter(|line| line.starts_with("pick "))
            .collect::<Vec<_>>();
        let altered = match mode {
            "missing" => {
                entries.pop();
                entries.join("\n")
            }
            "reordered" => {
                entries.swap(0, 1);
                entries.join("\n")
            }
            _ => format!("{original}\nexec must-not-run\n"),
        };
        assert!(plan.rewrite_todo(&altered, "#").is_err());
        // Rejected document is never saved. Drop answers editor exit 1, rather
        // than killing Git or pretending the operation successfully completed.
        drop(document);
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert!(!token.cancelled());
        assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
        assert_eq!(f.git(&["ls-files", "--stage"]), index);
        assert_eq!(f.git(&["status", "--porcelain"]), status);
        assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "third");
        assert!(!f.0.join(".git/rebase-merge").exists());
    }
}

#[test]
fn squash_message_mapping_is_persistent_scoped_and_consumed_only_for_matching_editor() {
    use plico::squash_message::{apply, install};
    for mode in [
        "match",
        "message",
        "onto",
        "head",
        "used",
        "corrupt",
        "changed-file",
    ] {
        let f = Fixture::new();
        let helper = f.helper();
        let gitdir = f.0.join(".git");
        let rebase = gitdir.join("rebase-merge");
        fs::create_dir(&rebase).unwrap();
        let head = "1".repeat(40);
        let onto = "2".repeat(40);
        fs::write(rebase.join("orig-head"), &head).unwrap();
        fs::write(rebase.join("onto"), &onto).unwrap();
        install(&gitdir, &head, &onto, "old subject\n", "新消息\n\nbody\n").unwrap();
        assert!(install(&gitdir, &head, &onto, "old", "must not replace").is_err());
        match mode {
            "onto" => fs::write(rebase.join("onto"), "3".repeat(40)).unwrap(),
            "head" => fs::write(rebase.join("orig-head"), "3".repeat(40)).unwrap(),
            "used" => fs::write(rebase.join("gitui-squash-message-used"), "reserved").unwrap(),
            "corrupt" => fs::write(rebase.join("gitui-squash-message"), "bad data").unwrap(),
            _ => {}
        }
        let path = gitdir.join("COMMIT_EDITMSG");
        let original = if mode == "message" {
            "unrelated message\n"
        } else {
            "old subject\n\n# Git comment\n"
        };
        fs::write(&path, original).unwrap();
        let token = Cancel::default();
        let requests = Requests::new(token.clone());
        let server = Server::start(requests.clone()).unwrap();
        let mut command = Command::new(&helper);
        server.configure(&mut command);
        let child = command.arg(&path).spawn().unwrap();
        let mut document = Document::open(pending(&requests), &gitdir).unwrap();
        if mode == "changed-file" {
            fs::write(&path, "external edit").unwrap();
        }
        let result = apply(&mut document);
        match mode {
            "corrupt" | "changed-file" => assert!(result.is_err()),
            "match" => assert!(result.unwrap()),
            _ => assert!(!result.unwrap()),
        }
        drop(document);
        assert_eq!(
            child.wait_with_output().unwrap().status.success(),
            mode == "match"
        );
        assert!(!token.cancelled());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            match mode {
                "match" => "新消息\n\nbody\n",
                "changed-file" => "external edit",
                _ => original,
            }
        );
        assert_eq!(
            rebase.join("gitui-squash-message-used").exists(),
            matches!(mode, "match" | "used")
        );
        if mode == "match" {
            // New helper/document, as after reopening the application: the same
            // mapping cannot be consumed twice even with matching original text.
            fs::write(&path, original).unwrap();
            let mut command = Command::new(&helper);
            server.configure(&mut command);
            let child = command.arg(&path).spawn().unwrap();
            let mut document = Document::open(pending(&requests), &gitdir).unwrap();
            assert!(!apply(&mut document).unwrap());
            document.save("manual next message\n").unwrap();
            assert!(child.wait_with_output().unwrap().status.success());
            assert_eq!(fs::read_to_string(path).unwrap(), "manual next message\n");
        }
    }
}

#[test]
fn squash_executor_runs_checked_plan_and_reports_actual_conflicts_without_rollback() {
    use plico::{
        git::Repository, operation_process::Progress, operation_state::Operation,
        published_commits::Protection, squash_check, squash_operation,
    };
    for mode in [
        "recent",
        "middle",
        "gapped",
        "conflict",
        "stale",
        "branch",
        "dirty",
        "protected",
        "hook",
        "cancel",
    ] {
        let f = Fixture::new();
        let helper = f.helper();
        fs::write(f.0.join("shared"), "base\n").unwrap();
        f.git(&["add", "shared"]);
        f.git(&["commit", "-m", "base"]);
        let base = f.git(&["rev-parse", "HEAD"]);
        let mut ids = Vec::new();
        for i in 0..5 {
            fs::write(f.0.join(format!("file{i}")), format!("value{i}\n")).unwrap();
            if mode == "conflict" {
                fs::write(f.0.join("shared"), format!("step{i}\n")).unwrap();
            }
            f.git(&["add", "shared", &format!("file{i}")]);
            f.git(&["commit", "-m", &format!("commit{i}")]);
            ids.push(f.git(&["rev-parse", "HEAD"]));
        }
        let head = f.git(&["rev-parse", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        let selection = match mode {
            "recent" => vec![4, 3, 2],
            "middle" => vec![3, 2, 1],
            _ => vec![4, 2, 0],
        };
        let selected = selection
            .iter()
            .map(|&i| ids[i].clone())
            .collect::<Vec<_>>();
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let prepared =
            squash_check::prepare(&repo, &head, &selected, Protection::Defaults, &token).unwrap();
        match mode {
            "stale" => {
                f.git(&["commit", "--allow-empty", "-m", "external commit"]);
            }
            "branch" => {
                f.git(&["checkout", "-b", "other"]);
            }
            "dirty" => {
                fs::write(f.0.join("shared"), "local edit\n").unwrap();
                f.git(&["add", "shared"]);
            }
            "protected" => {
                f.git(&["remote", "add", "origin", "."]);
                f.git(&["update-ref", "refs/remotes/origin/main", &head]);
            }
            "hook" => {
                let path = f.0.join(".git/hooks/pre-rebase");
                fs::write(&path, "#!/bin/sh\nexit 1\n").unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
                }
            }
            "cancel" => token.cancel(),
            _ => {}
        }
        let before = f.git(&["rev-parse", "HEAD"]);
        let index = f.git(&["ls-files", "--stage"]);
        let status = f.git(&["status", "--porcelain"]);
        let result = squash_operation::execute(
            &repo,
            &prepared,
            "压缩完成\n\nreviewed\n",
            &helper,
            Protection::Defaults,
            &token,
            &Progress::default(),
            None,
        );
        if matches!(mode, "stale" | "branch" | "protected" | "cancel") {
            assert!(result.is_err(), "{mode}");
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
            assert_eq!(f.git(&["ls-files", "--stage"]), index);
        } else {
            let report = result.unwrap();
            assert!(
                report.editor_error.is_none(),
                "{mode}: {:?}",
                report.editor_error
            );
            assert_eq!(report.old_head, head);
            if mode == "conflict" {
                assert!(!report.succeeded());
                let state = report.state.unwrap();
                assert_eq!(state.operation, Operation::RebaseMerge);
                assert!(!state.conflicts.is_empty());
                assert!(f.0.join(".git/rebase-merge/gitui-squash-message").is_file());
                // Only an explicit caller action aborts; executor never resets.
                f.git(&["rebase", "--abort"]);
                assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
            } else if mode == "hook" {
                assert!(!report.succeeded());
                assert!(!report.state.unwrap().in_progress());
                assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
                assert_eq!(f.git(&["ls-files", "--stage"]), index);
            } else {
                assert!(report.succeeded(), "{mode}: {:?}", report.command);
                if mode == "dirty" {
                    assert_eq!(f.git(&["ls-files", "--stage"]), index);
                }
                assert_eq!(report.head.unwrap().unwrap(), f.git(&["rev-parse", "HEAD"]));
                assert_eq!(
                    f.git(&["log", "--reverse", "--format=%s", &format!("{base}..HEAD")]),
                    match mode {
                        "recent" => "commit0\ncommit1\n压缩完成",
                        "middle" => "commit0\n压缩完成\ncommit4",
                        _ => "压缩完成\ncommit1\ncommit3",
                    }
                );
            }
            assert_eq!(f.git(&["rev-parse", "HEAD^{tree}"]), tree);
        }
        assert_eq!(f.git(&["status", "--porcelain"]), status);
        assert!(!f.0.join(".git/rebase-merge").exists());
    }
}

#[test]
fn squash_undo_restores_original_history_and_rechecks_publication_head_and_worktree() {
    use plico::{
        git::Repository, operation_process::Progress, published_commits::Protection, squash_check,
        squash_operation, squash_undo::Undo,
    };
    for mode in [
        "clean",
        "local",
        "staged",
        "published",
        "head-moved",
        "branch-changed",
        "different-worktree",
        "lock",
    ] {
        let f = Fixture::new();
        let helper = f.helper();
        fs::write(f.0.join("base"), "base\n").unwrap();
        f.git(&["add", "base"]);
        f.git(&["commit", "-m", "base"]);
        let mut ids = Vec::new();
        for i in 0..5 {
            fs::write(f.0.join(format!("file{i}")), format!("value{i}\n")).unwrap();
            f.git(&["add", &format!("file{i}")]);
            f.git(&["commit", "-m", &format!("commit{i}")]);
            ids.push(f.git(&["rev-parse", "HEAD"]));
        }
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let progress = Progress::default();
        let old_history = f.git(&["rev-list", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        let prepared = squash_check::prepare(
            &repo,
            &ids[4],
            &[ids[4].clone(), ids[2].clone(), ids[0].clone()],
            Protection::Defaults,
            &token,
        )
        .unwrap();
        let report = squash_operation::execute(
            &repo,
            &prepared,
            "squashed\n",
            &helper,
            Protection::Defaults,
            &token,
            &progress,
            None,
        )
        .unwrap();
        assert!(
            report.succeeded(),
            "{:?} {:?}",
            report.editor_error,
            report.command
        );
        let undo = Undo::capture(&repo, &report, &token).unwrap();
        assert_eq!(prepared.repository().path, repo.path);
        assert_eq!(undo.repository().path, repo.path);
        assert_eq!(undo.commit_to_focus_on_undo(), ids[0]);
        assert_eq!(
            undo.commit_to_focus().unwrap(),
            f.git(&["rev-parse", "HEAD~2"])
        );
        match mode {
            "local" | "staged" => {
                fs::write(f.0.join("base"), "valuable local edit\n").unwrap();
                fs::write(f.0.join("untracked"), "untracked content\n").unwrap();
                if mode == "staged" {
                    f.git(&["add", "base"]);
                }
            }
            "published" => {
                f.git(&["remote", "add", "origin", "."]);
                f.git(&["update-ref", "refs/remotes/origin/main", "HEAD~2"]);
            }
            "head-moved" => {
                f.git(&["commit", "--allow-empty", "-m", "new"]);
            }
            "branch-changed" => {
                f.git(&["checkout", "-b", "other"]);
            }
            "lock" => {
                fs::write(f.0.join(".git/index.lock"), "owned by test").unwrap();
            }
            "different-worktree" => {
                let other = Fixture::new();
                assert!(
                    undo.execute(
                        &Repository::new(&other.0),
                        Protection::Defaults,
                        &token,
                        &progress
                    )
                    .is_err()
                );
                continue;
            }
            _ => {}
        }
        let before = f.git(&["rev-parse", "HEAD"]);
        let index = f.git(&["ls-files", "--stage"]);
        let status = f.git(&["status", "--porcelain"]);
        let result = undo.execute(&undo.repository(), Protection::Defaults, &token, &progress);
        if matches!(mode, "published" | "head-moved" | "branch-changed") {
            assert!(result.is_err(), "{mode}");
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
            assert_eq!(f.git(&["ls-files", "--stage"]), index);
            assert_eq!(f.git(&["status", "--porcelain"]), status);
        } else if mode == "lock" {
            assert!(!result.unwrap().succeeded());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
            fs::remove_file(f.0.join(".git/index.lock")).unwrap();
        } else {
            let result = result.unwrap();
            assert!(result.succeeded(), "{mode}: {:?}", result.command);
            assert_eq!(f.git(&["rev-list", "HEAD"]), old_history);
            assert_eq!(f.git(&["rev-parse", "HEAD^{tree}"]), tree);
            assert!(
                undo.execute(&repo, Protection::Defaults, &token, &progress)
                    .is_err()
            );
            if matches!(mode, "local" | "staged") {
                assert_eq!(
                    fs::read_to_string(f.0.join("base")).unwrap(),
                    "valuable local edit\n"
                );
                assert_eq!(
                    fs::read_to_string(f.0.join("untracked")).unwrap(),
                    "untracked content\n"
                );
                // Git --keep resets the index but retains this working edit.
                assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "");
                assert_eq!(f.git(&["diff", "--name-only"]), "base");
            }
        }
    }
}

#[test]
fn rebase_continue_restores_matching_mapping_and_routes_other_messages_to_editor() {
    use plico::{
        git::Repository,
        operation_process::Progress,
        rebase_operation::{self, Action},
    };
    for mode in ["automatic", "unmatched", "used", "cancel", "corrupt"] {
        let f = Fixture::new();
        let helper = f.helper();
        f.git(&["commit", "--allow-empty", "-m", "base"]);
        let base = f.git(&["rev-parse", "HEAD"]);
        f.git(&["commit", "--allow-empty", "-m", "first"]);
        let first = f.git(&["rev-parse", "HEAD"]);
        f.git(&["commit", "--allow-empty", "-m", "second"]);
        let original_head = f.git(&["rev-parse", "HEAD"]);
        let original_history = f.git(&["rev-list", "HEAD"]);
        let gitdir = f.0.join(".git");
        let requests = Requests::new(Cancel::default());
        let server = Server::start(requests.clone()).unwrap();
        let mut command = Command::new("git");
        server.configure(&mut command);
        let child = command
            .arg("-C")
            .arg(&f.0)
            .args(["rebase", "-i", &base])
            .env(
                "GIT_SEQUENCE_EDITOR",
                editor::command(&helper, true).unwrap(),
            )
            .env("GIT_EDITOR", editor::command(&helper, false).unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut document = Document::open(pending(&requests), &gitdir).unwrap();
        plico::squash_message::install(
            &gitdir,
            &original_head,
            &base,
            if matches!(mode, "unmatched" | "cancel") {
                "another message"
            } else {
                "second"
            },
            "恢复后的消息\n",
        )
        .unwrap();
        document
            .save(&format!("edit {first}\nreword {original_head}\n"))
            .unwrap();
        assert!(child.wait_with_output().unwrap().status.success());
        drop(server); // A later operation uses a fresh bridge, no in-memory mapping.
        if mode == "used" {
            fs::write(
                gitdir.join("rebase-merge/gitui-squash-message-used"),
                "used",
            )
            .unwrap();
        }
        if mode == "corrupt" {
            fs::write(gitdir.join("rebase-merge/gitui-squash-message"), "bad").unwrap();
        }
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let progress = Progress::default();
        let requests = Requests::new(token.clone());
        let editor = editor::command(&helper, false).unwrap();
        let mut prompts = 0;
        let result = thread::scope(|scope| {
            let worker = scope.spawn(|| {
                rebase_operation::execute(
                    &repo,
                    Action::Continue {
                        editor: &editor,
                        requests: Some(&requests),
                    },
                    &token,
                    &progress,
                )
            });
            let deadline = Instant::now() + Duration::from_secs(30);
            while !worker.is_finished() {
                if let Some(pending) = requests.take_pending() {
                    prompts += 1;
                    let mut document = Document::open(pending, &gitdir).unwrap();
                    assert!(document.text().starts_with("second"));
                    if mode != "cancel" {
                        document.save("ordinary editor message\n").unwrap();
                    }
                }
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(10));
            }
            worker.join().unwrap().unwrap()
        });
        assert_eq!(
            prompts,
            usize::from(matches!(mode, "unmatched" | "used" | "cancel"))
        );
        if matches!(mode, "cancel" | "corrupt") {
            assert!(result.command.is_err() || !result.command.as_ref().unwrap().status.success());
            assert!(result.state.unwrap().in_progress());
            assert!(!token.cancelled());
            let aborted =
                rebase_operation::execute(&repo, Action::Abort, &token, &progress).unwrap();
            assert!(aborted.command.unwrap().status.success());
            assert!(aborted.squash_undo.is_none());
            assert!(!aborted.state.unwrap().in_progress());
            assert_eq!(f.git(&["rev-list", "HEAD"]), original_history);
        } else {
            assert!(result.command.unwrap().status.success());
            assert!(!result.state.unwrap().in_progress());
            assert_eq!(
                f.git(&["log", "-1", "--format=%B"]),
                if mode == "automatic" {
                    "恢复后的消息"
                } else {
                    "ordinary editor message"
                }
            );
            assert_eq!(f.git(&["rev-list", "--count", "HEAD"]), "3");
        }
        assert!(!gitdir.join("rebase-merge").exists());
    }
}

#[test]
fn squash_conflicts_continue_through_existing_rebase_action_without_reusing_consumed_message() {
    use plico::{
        git::Repository,
        operation_process::Progress,
        published_commits::Protection,
        rebase_operation::{self, Action},
        squash_check, squash_operation,
    };
    for mode in ["owned", "missing", "corrupt", "mismatch"] {
        let f = Fixture::new();
        let helper = f.helper();
        fs::write(f.0.join("local"), "base\n").unwrap();
        f.git(&["add", "local"]);
        f.git(&["commit", "-m", "base"]);
        fs::write(f.0.join("a"), "a").unwrap();
        f.git(&["add", "a"]);
        f.git(&["commit", "-m", "first"]);
        let first = f.git(&["rev-parse", "HEAD"]);
        fs::write(f.0.join("shared"), "base\n").unwrap();
        fs::write(f.0.join("middle"), "middle").unwrap();
        f.git(&["add", "shared", "middle"]);
        f.git(&["commit", "-m", "middle"]);
        fs::write(f.0.join("shared"), "edited\n").unwrap();
        f.git(&["add", "shared"]);
        f.git(&["commit", "-m", "last"]);
        let head = f.git(&["rev-parse", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let progress = Progress::default();
        let prepared = squash_check::prepare(
            &repo,
            &head,
            &[head.clone(), first],
            Protection::Defaults,
            &token,
        )
        .unwrap();
        fs::write(f.0.join("local"), "staged\n").unwrap();
        f.git(&["add", "local"]);
        fs::write(f.0.join("local"), "unstaged\n").unwrap();
        let index = f.git(&["ls-files", "--stage"]);
        let report = squash_operation::execute(
            &repo,
            &prepared,
            "squashed\n",
            &helper,
            Protection::Defaults,
            &token,
            &progress,
            None,
        )
        .unwrap();
        assert!(!report.succeeded());
        assert!(!report.state.unwrap().conflicts.is_empty());
        let origin = f.0.join(".git/rebase-merge/gitui-squash-origin");
        assert!(origin.is_file());
        match mode {
            "missing" => fs::remove_file(&origin).unwrap(),
            "corrupt" => fs::write(&origin, "bad record").unwrap(),
            "mismatch" => {
                let data = fs::read_to_string(&origin)
                    .unwrap()
                    .replace(&head, &"f".repeat(40));
                fs::write(&origin, data).unwrap();
            }
            _ => {}
        }
        let mut completed_undo = None;
        let editor = editor::command(&helper, false).unwrap();
        let requests = Requests::new(token.clone());
        let mut rounds = 0;
        while repo.operation_state(&token).unwrap().in_progress() {
            rounds += 1;
            assert!(rounds <= 3);
            fs::write(f.0.join("shared"), "edited\n").unwrap();
            f.git(&["add", "shared"]);
            thread::scope(|scope| {
                let worker = scope.spawn(|| {
                    rebase_operation::execute(
                        &repo,
                        Action::Continue {
                            editor: &editor,
                            requests: Some(&requests),
                        },
                        &token,
                        &progress,
                    )
                });
                let deadline = Instant::now() + Duration::from_secs(30);
                while !worker.is_finished() {
                    if let Some(pending) = requests.take_pending() {
                        let mut document = Document::open(pending, &f.0.join(".git")).unwrap();
                        let text = document.text().to_owned();
                        document.save(&text).unwrap();
                    }
                    assert!(Instant::now() < deadline);
                    thread::sleep(Duration::from_millis(10));
                }
                let result = worker.join().unwrap().unwrap();
                assert!(result.command.is_ok());
                if result.state.as_ref().unwrap().in_progress() {
                    assert!(result.squash_undo.is_none());
                } else {
                    assert_eq!(
                        result.recovery_error.is_some(),
                        matches!(mode, "corrupt" | "mismatch")
                    );
                    completed_undo = result.squash_undo;
                }
            });
        }
        assert_eq!(rounds, 2);
        assert_eq!(f.git(&["ls-files", "--stage"]), index);
        assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "unstaged\n");
        assert!(
            plico::rebase_saved::read(&f.0.join(".git"))
                .unwrap()
                .is_none()
        );
        assert_eq!(f.git(&["rev-parse", "HEAD^{tree}"]), tree);
        assert_eq!(
            f.git(&["log", "--reverse", "--format=%s"]),
            "base\nsquashed\nmiddle"
        );
        assert!(!f.0.join(".git/rebase-merge").exists());
        if mode == "owned" {
            let undo = completed_undo.expect("Completed owned Squash must provide Undo");
            assert_eq!(
                undo.commit_to_focus().unwrap(),
                f.git(&["rev-parse", "HEAD^"])
            );
            assert!(
                undo.execute(&repo, Protection::Defaults, &token, &progress)
                    .unwrap()
                    .succeeded()
            );
            assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
            assert_eq!(
                f.git(&["log", "--reverse", "--format=%s"]),
                "base\nfirst\nmiddle\nlast"
            );
            assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "unstaged\n");
        } else {
            assert!(completed_undo.is_none(), "{mode}");
        }
    }
}

#[test]
fn squash_local_changes_preserve_index_worktree_and_existing_stash() {
    use plico::{
        git::Repository, operation_process::Progress, published_commits::Protection, rebase_saved,
        squash_check, squash_operation,
    };
    for mode in [
        "success",
        "hook",
        "conflict",
        "conflict-foreign",
        "conflict-record",
        "conflict-manual-apply",
        "conflict-manual-pop",
        "conflict-manual-other",
        "conflict-manual-blocked",
        "save-failure",
    ] {
        let f = Fixture::new();
        let helper = f.helper();
        fs::write(f.0.join("local"), "base\n").unwrap();
        fs::write(f.0.join("shared"), "base\n").unwrap();
        f.git(&["add", "local", "shared"]);
        f.git(&["commit", "-m", "base"]);
        let mut ids = Vec::new();
        for i in 0..3 {
            fs::write(f.0.join(format!("file{i}")), "content\n").unwrap();
            if mode.starts_with("conflict") {
                fs::write(f.0.join("shared"), format!("step{i}\n")).unwrap();
            }
            f.git(&["add", "shared", &format!("file{i}")]);
            f.git(&["commit", "-m", &format!("commit{i}")]);
            ids.push(f.git(&["rev-parse", "HEAD"]));
        }
        fs::write(f.0.join("local"), "older stash\n").unwrap();
        f.git(&["stash", "push", "-m", "unrelated"]);
        let previous = f.git(&["rev-parse", "refs/stash"]);
        fs::write(f.0.join("local"), "staged\n").unwrap();
        f.git(&["add", "local"]);
        fs::write(f.0.join("local"), "unstaged\n").unwrap();
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let prepared = squash_check::prepare(
            &repo,
            &ids[2],
            &[ids[0].clone(), ids[2].clone()],
            Protection::Defaults,
            &token,
        )
        .unwrap();
        let index = f.git(&["ls-files", "--stage"]);
        let status = f.git(&["status", "--porcelain"]);
        if mode == "hook" {
            let hook = f.0.join(".git/hooks/pre-rebase");
            fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(hook, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        if mode == "save-failure" {
            fs::write(f.0.join(".git/index.lock"), "owned by test").unwrap();
        }
        let report = squash_operation::execute(
            &repo,
            &prepared,
            "squashed\n",
            &helper,
            Protection::Defaults,
            &token,
            &Progress::default(),
            None,
        )
        .unwrap();
        assert!(
            report.recovery_error.is_none(),
            "{mode}: {:?}",
            report.recovery_error
        );
        if mode.starts_with("conflict") {
            assert!(!report.succeeded());
            assert!(report.state.as_ref().unwrap().in_progress());
            let record = rebase_saved::read(&f.0.join(".git")).unwrap().unwrap();
            assert_eq!(Some(record.stash.as_str()), report.saved_oid.as_deref());
            assert_eq!(f.git(&["rev-parse", "stash@{0}"]), record.stash);
            assert_eq!(f.git(&["rev-parse", "stash@{1}"]), previous);
            assert_eq!(f.git(&["show", "stash@{0}:local"]), "unstaged");
            assert_eq!(f.git(&["show", "stash@{0}^2:local"]), "staged");
            if mode.starts_with("conflict-manual") {
                use plico::{
                    stash,
                    stash_operation::{self, Action, Status},
                };
                f.git(&["rebase", "--abort"]); // User finished the rebase outside the app.
                let mut entries = stash::list(&repo, 10, &token).unwrap().entries;
                let entry = entries.remove(if mode == "conflict-manual-other" {
                    1
                } else {
                    0
                });
                if mode == "conflict-manual-blocked" {
                    fs::write(f.0.join("local"), "valuable new work\n").unwrap();
                }
                let report = stash_operation::execute(
                    &repo,
                    &entry,
                    if mode == "conflict-manual-pop" {
                        Action::Pop
                    } else {
                        Action::Apply
                    },
                    mode == "conflict-manual-pop",
                    &token,
                    &Progress::default(),
                )
                .unwrap();
                assert!(report.recovery_error.is_none());
                let remaining = rebase_saved::read(&f.0.join(".git")).unwrap();
                if mode == "conflict-manual-blocked" {
                    assert_ne!(report.status, Status::Success);
                    assert_eq!(remaining, Some(record));
                    assert_eq!(
                        fs::read_to_string(f.0.join("local")).unwrap(),
                        "valuable new work\n"
                    );
                } else if mode == "conflict-manual-other" {
                    assert_eq!(report.status, Status::Success);
                    assert_eq!(remaining, Some(record));
                    assert_eq!(
                        fs::read_to_string(f.0.join("local")).unwrap(),
                        "older stash\n"
                    );
                } else {
                    assert_eq!(report.status, Status::Success);
                    assert!(remaining.is_none());
                    assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "unstaged\n");
                    if mode == "conflict-manual-pop" {
                        assert_eq!(f.git(&["ls-files", "--stage"]), index);
                        assert_eq!(f.git(&["rev-parse", "refs/stash"]), previous);
                    } else {
                        assert_eq!(f.git(&["show", ":local"]), "base");
                        assert_eq!(f.git(&["rev-parse", "refs/stash"]), record.stash);
                    }
                }
                continue;
            }
            if mode == "conflict-foreign" {
                f.git(&["update-ref", "refs/stash", &previous]);
            }
            if mode == "conflict-record" {
                let path = rebase_saved::path(&f.0.join(".git"));
                let text = fs::read_to_string(&path)
                    .unwrap()
                    .replace(record.base.as_deref().unwrap(), &record.head);
                fs::write(path, text).unwrap();
            }
            let aborted = plico::rebase_operation::execute(
                &repo,
                plico::rebase_operation::Action::Abort,
                &token,
                &Progress::default(),
            )
            .unwrap();
            assert!(aborted.command.unwrap().status.success());
            if mode != "conflict" {
                assert!(aborted.recovery_error.is_some(), "{mode}");
                assert!(rebase_saved::read(&f.0.join(".git")).unwrap().is_some());
                assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "base\n");
                assert_eq!(
                    f.git(&["show", &format!("{}:local", record.stash)]),
                    "unstaged"
                );
                continue;
            }
            assert!(
                aborted.recovery_error.is_none(),
                "{:?}",
                aborted.recovery_error
            );
            assert_eq!(f.git(&["rev-parse", "HEAD"]), ids[2]);
            assert_eq!(f.git(&["ls-files", "--stage"]), index);
            assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "unstaged\n");
            assert_eq!(f.git(&["rev-parse", "refs/stash"]), previous);
            assert!(rebase_saved::read(&f.0.join(".git")).unwrap().is_none());
        } else {
            assert_eq!(
                report.succeeded(),
                mode == "success",
                "{mode}: {:?}",
                report.command
            );
            assert!(report.saved_oid.is_none());
            assert!(rebase_saved::read(&f.0.join(".git")).unwrap().is_none());
            assert_eq!(f.git(&["ls-files", "--stage"]), index, "{mode}");
            assert_eq!(f.git(&["status", "--porcelain"]), status, "{mode}");
            assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "unstaged\n");
            assert_eq!(f.git(&["rev-parse", "refs/stash"]), previous);
            if mode != "success" {
                assert_eq!(f.git(&["rev-parse", "HEAD"]), ids[2]);
            }
        }
    }
}

#[test]
fn multi_root_editor_routes_real_git_commits_without_accepting_unlisted_roots() {
    let parent = Fixture::new();
    let child = Fixture::new();
    let helper = parent.helper();
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    for accept in [false, true] {
        let mut command = Command::new("git");
        server.configure(&mut command);
        let process = command
            .arg("-C")
            .arg(&child.0)
            .args(["commit", "--allow-empty", "--edit", "-m", "child message"])
            .env("GIT_EDITOR", editor::command(&helper, false).unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut roots = vec![parent.0.join(".git")];
        if accept {
            roots.push(child.0.join(".git"));
        }
        let document = Document::open_in_roots(pending(&requests), &roots);
        if accept {
            let mut document = document.unwrap();
            assert!(document.text().contains("child message"));
            document.save("edited child message\n").unwrap();
        } else {
            assert!(document.is_err());
        }
        assert_eq!(process.wait_with_output().unwrap().status.success(), accept);
    }
    assert_eq!(
        child.git(&["log", "-1", "--format=%s"]),
        "edited child message"
    );
    assert!(!parent.0.join(".git/COMMIT_EDITMSG").exists());
}

#[test]
fn multi_root_editor_does_not_authorize_nested_git_directory_by_parent_prefix() {
    let parent = Fixture::new();
    let nested_git = parent.0.join(".git/modules/nested");
    fs::create_dir_all(&nested_git).unwrap();
    let file = nested_git.join("COMMIT_EDITMSG");
    fs::write(&file, "nested draft").unwrap();
    let helper = parent.helper();
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    for allow in [false, true] {
        let mut command = Command::new(&helper);
        server.configure(&mut command);
        let process = command
            .arg(&file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut roots = vec![parent.0.join(".git")];
        if allow {
            roots.push(nested_git.clone());
        }
        let document = Document::open_in_roots(pending(&requests), &roots);
        if allow {
            document.unwrap().save("nested edited").unwrap();
        } else {
            assert!(document.is_err());
        }
        assert_eq!(process.wait_with_output().unwrap().status.success(), allow);
    }
    assert_eq!(fs::read_to_string(file).unwrap(), "nested edited");
}

#[test]
fn interactive_messages_route_multiple_real_editor_requests_and_fallback() {
    use plico::interactive_rebase::messages::{Mapping, apply, install};
    let f = Fixture::new();
    let helper = f.helper();
    let gitdir = f.0.join(".git");
    let rebase = gitdir.join("rebase-merge");
    fs::create_dir(&rebase).unwrap();
    let head = "1".repeat(40);
    fs::write(rebase.join("orig-head"), &head).unwrap();
    fs::write(rebase.join("onto"), "2".repeat(40)).unwrap();
    install(
        &gitdir,
        &head,
        vec![
            Mapping {
                original: "same\n".into(),
                replacement: "第一个\n".into(),
            },
            Mapping {
                original: "same\n".into(),
                replacement: "第二个\n".into(),
            },
        ],
    )
    .unwrap();
    let path = gitdir.join("COMMIT_EDITMSG");
    for (original, expected, mapped) in [
        ("unrelated\n", "manual\n", false),
        ("same\n\n# comment", "第一个\n", true),
        ("same\n", "第二个\n", true),
        ("same\n", "manual\n", false),
    ] {
        // Each request uses a new server and document, exercising persisted state.
        let requests = Requests::new(Cancel::default());
        let server = Server::start(requests.clone()).unwrap();
        fs::write(&path, original).unwrap();
        let mut command = Command::new(&helper);
        server.configure(&mut command);
        let child = command.arg(&path).spawn().unwrap();
        let mut document = Document::open(pending(&requests), &gitdir).unwrap();
        assert_eq!(apply(&mut document).unwrap(), mapped);
        if !mapped {
            document.save(expected).unwrap();
        }
        assert!(child.wait_with_output().unwrap().status.success());
        assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    }
}

#[test]
fn interactive_plan_bridge_rewrites_real_git_history_including_root() {
    use plico::{
        git::Repository,
        interactive_rebase::{
            editor::{Response, handle},
            interaction::{Answer, Question, Requests as PlanRequests},
            messages,
            model::Action,
        },
    };
    for (root, latin1) in [(false, false), (true, false), (false, true), (true, true)] {
        let f = Fixture::new();
        f.git(&["config", "core.autocrlf", "false"]);
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        if latin1 {
            f.git(&["config", "i18n.commitEncoding", "ISO-8859-1"]);
        }
        let replacement = if latin1 {
            "café rewritten\n"
        } else {
            "合并后的消息\n"
        };
        let helper = f.helper();
        for n in 0..4 {
            fs::write(f.0.join(format!("file-{n}")), format!("value {n}\n")).unwrap();
            f.git(&["add", "."]);
            f.git(&["commit", "-m", &format!("commit {n}")]);
        }
        let head = f.git(&["rev-parse", "HEAD"]);
        let base = f.git(&["rev-parse", "HEAD~3"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        let repo = Repository::new(&f.0);
        let gitdir = f.0.join(".git");
        let token = Cancel::default();
        let requests = Requests::new(token.clone());
        let server = Server::start(requests.clone()).unwrap();
        let mut command = Command::new("git");
        command.arg("-C").arg(&f.0).args([
            "-c",
            "core.commentChar=\u{1}",
            "rebase",
            "--interactive",
        ]);
        command.arg(if root { "--root" } else { &base });
        command
            .env(
                "GIT_SEQUENCE_EDITOR",
                editor::command(&helper, true).unwrap(),
            )
            .env("GIT_EDITOR", editor::command(&helper, false).unwrap());
        server.configure(&mut command);
        let child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut document = Document::open(pending(&requests), &gitdir).unwrap();
        let plans = PlanRequests::new(token.clone());
        thread::scope(|scope| {
            let worker = scope.spawn(|| {
                handle(
                    &repo,
                    &mut document,
                    &head,
                    &plans,
                    &Cancel::default(),
                    &token,
                )
            });
            let started = Instant::now();
            let question = loop {
                if let Some(question) = plans.take_pending() {
                    break question;
                }
                assert!(started.elapsed() < Duration::from_secs(5));
                thread::sleep(Duration::from_millis(2));
            };
            let Question::Plan(model, _) = &question.question else {
                panic!("expected structured plan")
            };
            assert_eq!(model.rows().len(), if root { 4 } else { 3 });
            let mut edited = model.clone();
            assert!(edited.set(&[0], Action::Reword(replacement.into())));
            assert!(edited.unite(&[0, 1]).is_some());
            question.answer(Answer::Plan(edited)).unwrap();
            assert!(matches!(worker.join().unwrap().unwrap(), Response::Saved));
        });
        let mut message = Document::open(pending(&requests), &gitdir).unwrap();
        assert!(messages::apply(&mut message).unwrap());
        let outcome = child.wait_with_output().unwrap();
        assert!(
            outcome.status.success(),
            "{}",
            String::from_utf8_lossy(&outcome.stderr)
        );
        assert_eq!(f.git(&["rev-parse", "HEAD^{tree}"]), tree);
        assert_eq!(f.git(&["rev-list", "--count", "HEAD"]), "3");
        assert!(
            f.git(&["log", "--encoding=UTF-8", "--format=%B"])
                .contains(replacement.trim())
        );
        assert!(!gitdir.join("rebase-merge").exists());
    }
}

#[test]
fn interactive_operation_preserves_dirty_index_and_restores_after_cancel_or_root_abort() {
    use plico::{
        git::Repository,
        interactive_rebase::{
            interaction::{Answer, Question, Requests as PlanRequests},
            model::Action,
            operation,
            prepare::Prepared,
        },
        operation_process::Progress,
        published_commits::Protection,
    };
    for (root, mode) in [
        (false, "success"),
        (true, "success"),
        (true, "cancel"),
        (true, "edit"),
        (true, "edit-continue"),
    ] {
        let f = Fixture::new();
        let helper = f.helper();
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f.git(&["config", "core.autocrlf", "false"]);
        fs::write(f.0.join("local"), "original\n").unwrap();
        f.git(&["add", "local"]);
        f.git(&["commit", "-m", "base"]);
        let base = f.git(&["rev-parse", "HEAD"]);
        fs::write(f.0.join("a"), "a\n").unwrap();
        f.git(&["add", "a"]);
        f.git(&["commit", "-m", "first"]);
        let first = f.git(&["rev-parse", "HEAD"]);
        fs::write(f.0.join("b"), "b\n").unwrap();
        f.git(&["add", "b"]);
        f.git(&["commit", "-m", "last"]);
        let head = f.git(&["rev-parse", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        fs::write(f.0.join("local"), "staged\n").unwrap();
        f.git(&["add", "local"]);
        fs::write(f.0.join("local"), "unstaged\n").unwrap();
        let index = f.git(&["ls-files", "--stage"]);
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let progress = Progress::default();
        let prepared = Prepared::prepare(
            &repo,
            if root { &base } else { &first },
            Protection::Defaults,
            &token,
        )
        .unwrap();
        let plans = PlanRequests::new(token.clone());
        let fallback = Requests::new(token.clone());
        let report = thread::scope(|scope| {
            let worker = scope.spawn(|| {
                operation::execute(
                    &prepared,
                    root,
                    &helper,
                    Protection::Defaults,
                    &plans,
                    &fallback,
                    &token,
                    &progress,
                )
            });
            let _cancel_on_panic = CancelOnDrop(&token);
            let started = Instant::now();
            let pending = loop {
                if let Some(pending) = plans.take_pending() {
                    break pending;
                }
                assert!(
                    !worker.is_finished(),
                    "operation ended before plan; mode {mode}"
                );
                assert!(started.elapsed() < Duration::from_secs(10));
                thread::sleep(Duration::from_millis(2));
            };
            let Question::Plan(model, _) = &pending.question else {
                panic!("plan expected")
            };
            let mut edited = model.clone();
            if mode == "cancel" {
                drop(pending);
            } else {
                if mode.starts_with("edit") {
                    assert!(edited.set(&[0], Action::Edit));
                    assert!(edited.set(&[1], Action::Reword("after pause\n".into())));
                } else {
                    assert!(edited.set(&[0], Action::Reword("rewritten\n".into())));
                }
                pending.answer(Answer::Plan(edited)).unwrap();
            }
            worker.join().unwrap().unwrap()
        });
        assert!(
            report.recovery_error.is_none(),
            "{:?}",
            report.recovery_error
        );
        assert!(report.editor_error.is_none(), "{:?}", report.editor_error);
        if mode.starts_with("edit") {
            assert!(!report.succeeded());
            assert!(report.stopped_for_editing());
            assert!(report.saved_oid.is_some());
            assert!(report.state.as_ref().unwrap().in_progress());
            let commit_editor = editor::command(&helper, false).unwrap();
            let action = if mode == "edit" {
                plico::rebase_operation::Action::Abort
            } else {
                plico::rebase_operation::Action::Continue {
                    editor: &commit_editor,
                    requests: Some(&fallback),
                }
            };
            let result =
                plico::rebase_operation::execute(&repo, action, &token, &progress).unwrap();
            assert!(result.command.as_ref().unwrap().status.success());
            assert!(
                result.recovery_error.is_none(),
                "{:?}",
                result.recovery_error
            );
            if mode == "edit" {
                assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
            } else {
                assert!(f.git(&["log", "--format=%B"]).contains("after pause"));
            }
        } else if mode == "cancel" {
            assert!(!report.succeeded());
            assert!(!report.stopped_for_editing());
            assert!(report.saved_oid.is_none());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
        } else {
            assert!(report.succeeded());
            assert!(!report.stopped_for_editing());
        }
        assert_eq!(f.git(&["ls-files", "--stage"]), index, "{mode}");
        assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "unstaged\n");
        assert_eq!(f.git(&["rev-parse", "HEAD^{tree}"]), tree);
        assert!(!f.0.join(".git/rebase-merge").exists());
        assert!(
            plico::rebase_saved::read(&f.0.join(".git"))
                .unwrap()
                .is_none()
        );
        assert_eq!(f.git(&["stash", "list"]), "");
    }
}

#[test]
fn editor_uses_commit_and_log_encodings_without_lossy_writes() {
    let f = Fixture::new();
    f.git(&["config", "i18n.commitEncoding", "ISO-8859-1"]);
    f.git(&["config", "i18n.logOutputEncoding", "UTF-8"]);
    let helper = f.helper();
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    for sequence in [false, true] {
        let path = f.0.join(if sequence {
            ".git/rebase-merge/git-rebase-todo"
        } else {
            ".git/COMMIT_EDITMSG"
        });
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original: &[u8] = if sequence {
            "pick abc café\n".as_bytes()
        } else {
            b"caf\xe9\n"
        };
        fs::write(&path, original).unwrap();
        let mut command = Command::new(&helper);
        server.configure(&mut command);
        if sequence {
            command.arg("--sequence");
        }
        let child = command.arg(&path).spawn().unwrap();
        let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
        assert!(document.text().contains("café"));
        if !sequence {
            assert!(document.save("中文").is_err());
            assert_eq!(fs::read(&path).unwrap(), original);
        }
        document.save("café rewritten\n").unwrap();
        assert!(child.wait_with_output().unwrap().status.success());
        assert_eq!(
            fs::read(&path).unwrap(),
            if sequence {
                "café rewritten\n".as_bytes()
            } else {
                b"caf\xe9 rewritten\n"
            }
        );
    }
}

#[test]
fn real_git_commit_editor_preserves_latin1_commit_object() {
    let f = Fixture::new();
    f.git(&["config", "i18n.commitEncoding", "ISO-8859-1"]);
    let helper = f.helper();
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    let mut command = Command::new("git");
    server.configure(&mut command);
    let child = command
        .arg("-C")
        .arg(&f.0)
        .args(["commit", "--allow-empty", "--edit", "-m", "initial"])
        .env("GIT_EDITOR", editor::command(&helper, false).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut document = Document::open(pending(&requests), &f.0.join(".git")).unwrap();
    document.save("café rewritten\n").unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let object = Command::new("git")
        .arg("-C")
        .arg(&f.0)
        .args(["cat-file", "commit", "HEAD"])
        .output()
        .unwrap();
    assert!(object.status.success());
    assert!(object.stdout.ends_with(b"\n\ncaf\xe9 rewritten\n"));
    assert!(
        object
            .stdout
            .windows(b"encoding ISO-8859-1".len())
            .any(|s| s == b"encoding ISO-8859-1")
    );
}

#[test]
fn linked_worktree_editor_reads_its_effective_encoding_and_rejects_unknown_encoding() {
    let f = Fixture::new();
    f.git(&["commit", "--allow-empty", "-m", "base"]);
    let worktree = f.0.join("linked");
    f.git(&[
        "worktree",
        "add",
        "-b",
        "linked",
        worktree.to_str().unwrap(),
    ]);
    f.git(&["config", "extensions.worktreeConfig", "true"]);
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(&worktree)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    let directory = PathBuf::from(run(&["rev-parse", "--absolute-git-dir"]));
    let helper = f.helper();
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    for encoding in ["ISO-8859-1", "plico-invalid-encoding"] {
        run(&["config", "--worktree", "i18n.commitEncoding", encoding]);
        let path = directory.join("COMMIT_EDITMSG");
        fs::write(&path, b"caf\xe9\n").unwrap();
        let mut command = Command::new(&helper);
        server.configure(&mut command);
        let child = command.arg(&path).spawn().unwrap();
        let result = Document::open(pending(&requests), &directory);
        if encoding == "ISO-8859-1" {
            let mut document = result.unwrap();
            assert_eq!(document.text(), "café\n");
            document.save("café linked\n").unwrap();
            assert!(child.wait_with_output().unwrap().status.success());
            assert_eq!(fs::read(path).unwrap(), b"caf\xe9 linked\n");
        } else {
            assert!(result.is_err());
            assert!(!child.wait_with_output().unwrap().status.success());
            assert_eq!(fs::read(path).unwrap(), b"caf\xe9\n");
        }
    }
}

#[test]
fn interactive_autosquash_preserves_messages_and_updates_other_branch_refs() {
    use plico::{
        git::Repository,
        interactive_rebase::{
            editor::{Input, load},
            messages,
        },
    };
    for kind in ["fixup", "squash", "amend"] {
        let f = Fixture::new();
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f.git(&["config", "core.autocrlf", "false"]);
        let helper = f.helper();
        fs::write(f.0.join("base"), "base\n").unwrap();
        f.git(&["add", "."]);
        f.git(&["commit", "-m", "base"]);
        let base = f.git(&["rev-parse", "HEAD"]);
        fs::write(f.0.join("feature"), "original\n").unwrap();
        f.git(&["add", "."]);
        f.git(&["commit", "-m", "feature", "-m", "original body"]);
        fs::write(f.0.join("feature"), "corrected\n").unwrap();
        f.git(&["add", "."]);
        f.git(&[
            "commit",
            "-m",
            &format!("{kind}! feature"),
            "-m",
            "replacement body",
        ]);
        f.git(&["branch", "checkpoint"]);
        let checkpoint = f.git(&["rev-parse", "checkpoint"]);
        fs::write(f.0.join("tail"), "tail\n").unwrap();
        f.git(&["add", "."]);
        f.git(&["commit", "-m", "tail"]);
        let head = f.git(&["rev-parse", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        let token = Cancel::default();
        let requests = Requests::new(token.clone());
        let server = Server::start(requests.clone()).unwrap();
        let gitdir = f.0.join(".git");
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&f.0)
            .args([
                "-c",
                "core.commentChar=\u{1}",
                "rebase",
                "-i",
                "--autosquash",
                "--update-refs",
                &base,
            ])
            .env(
                "GIT_SEQUENCE_EDITOR",
                editor::command(&helper, true).unwrap(),
            )
            .env("GIT_EDITOR", editor::command(&helper, false).unwrap());
        server.configure(&mut command);
        let child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut document = Document::open(pending(&requests), &gitdir).unwrap();
        assert!(document.text().contains("update-ref refs/heads/checkpoint"));
        let Input::Plan(plan) = load(&Repository::new(&f.0), &document, &token).unwrap() else {
            panic!("expected structured autosquash plan");
        };
        let edited = plan.model.clone();
        plan.save(&edited, &mut document, &head).unwrap();
        if kind != "fixup" {
            let mut message = Document::open(pending(&requests), &gitdir).unwrap();
            assert!(messages::apply(&mut message).unwrap());
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{kind}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(f.git(&["rev-parse", "HEAD^{tree}"]), tree);
        assert_eq!(f.git(&["rev-list", "--count", "HEAD"]), "3");
        let updated = f.git(&["rev-parse", "checkpoint"]);
        assert_ne!(updated, checkpoint);
        assert_eq!(updated, f.git(&["rev-parse", "HEAD~1"]));
        let message = f.git(&["show", "-s", "--format=%B", "checkpoint"]);
        match kind {
            "fixup" => assert_eq!(message, "feature\n\noriginal body"),
            "amend" => assert_eq!(message, "replacement body"),
            _ => {
                assert!(message.starts_with("feature\n\noriginal body"));
                assert!(message.ends_with("replacement body"));
            }
        }
        assert!(f.git(&["status", "--porcelain"]).is_empty());
        assert!(!gitdir.join("rebase-merge").exists());
    }
}
