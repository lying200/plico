use plico::git::{Cancel, Repository};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Fixture(PathBuf);
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
impl Fixture {
    fn new() -> Self {
        let f = Self(std::env::temp_dir().join(format!(
                "plico-unshallow-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        fs::create_dir_all(&f.0).unwrap();
        git(&f.0, &["init", "-b", "main", "seed"]);
        let seed = f.0.join("seed");
        for (key, value) in [
            ("user.name", "Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "disabled-hooks"),
        ] {
            git(&seed, &["config", key, value]);
        }
        for index in 0..5 {
            fs::write(seed.join("file.txt"), format!("version {index}\n")).unwrap();
            git(&seed, &["add", "file.txt"]);
            git(&seed, &["commit", "-m", &format!("commit {index}")]);
        }
        git(&f.0, &["clone", "--bare", "seed", "remote.git"]);
        git(
            &f.0,
            &["clone", "--no-local", "--depth=2", "remote.git", "project"],
        );
        f
    }
    fn repo(&self) -> Repository {
        Repository::new(self.0.join("project"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn obtains_full_history_without_touching_head_index_or_worktree() {
    let f = Fixture::new();
    let repo = f.repo();
    let token = Cancel::default();
    assert_eq!(git(&repo.path, &["rev-list", "--count", "HEAD"]), "2");
    let head = git(&repo.path, &["rev-parse", "HEAD"]);
    fs::write(repo.path.join("file.txt"), "staged\n").unwrap();
    git(&repo.path, &["add", "file.txt"]);
    fs::write(repo.path.join("file.txt"), "draft\n").unwrap();
    let index = fs::read(repo.path.join(".git/index")).unwrap();
    let spec = repo.unshallow_spec(&token).unwrap();
    assert_eq!(spec.remote(), "origin");
    assert!(spec.unshallow);
    let result =
        plico::fetch::execute(&repo, &[spec.clone()], &token, &Default::default()).unwrap();
    assert!(result.successful());
    assert_eq!(
        git(&repo.path, &["rev-parse", "--is-shallow-repository"]),
        "false"
    );
    assert_eq!(git(&repo.path, &["rev-list", "--count", "HEAD"]), "5");
    assert_eq!(git(&repo.path, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(repo.path.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(repo.path.join("file.txt")).unwrap(),
        "draft\n"
    );
    assert!(repo.unshallow_spec(&token).is_err());
    assert!(repo.fetch(&spec, &token, &Default::default()).is_err());
}
#[test]
fn project_context_unshallows_only_the_selected_child() {
    let fixture = Fixture::new();
    let parent = fixture.repo();
    git(&parent.path, &["fetch", "--unshallow"]);
    git(
        &parent.path,
        &["clone", "--no-local", "--depth=2", "../remote.git", "child"],
    );
    let child = Repository::new(parent.path.join("child"));
    let token = Cancel::default();
    let roots = plico::project_repositories::Repositories::from_registered(
        &[parent.clone(), child.clone()],
        &token,
    )
    .unwrap();
    let parent_head = git(&parent.path, &["rev-parse", "HEAD"]);
    let parent_index = fs::read(parent.path.join(".git/index")).unwrap();
    fs::write(parent.path.join("draft.txt"), "keep this draft").unwrap();
    let selected = roots
        .operation_repository(Some(&child.path), None, &parent.path)
        .unwrap();
    assert_eq!(selected.path, child.path);
    let spec = selected.unshallow_spec(&token).unwrap();
    assert!(
        plico::fetch::execute(selected, &[spec], &token, &Default::default())
            .unwrap()
            .successful()
    );
    assert_eq!(git(&child.path, &["rev-list", "--count", "HEAD"]), "5");
    assert_eq!(
        git(&child.path, &["rev-parse", "--is-shallow-repository"]),
        "false"
    );
    assert_eq!(git(&parent.path, &["rev-parse", "HEAD"]), parent_head);
    assert_eq!(
        fs::read(parent.path.join(".git/index")).unwrap(),
        parent_index
    );
    assert_eq!(
        fs::read_to_string(parent.path.join("draft.txt")).unwrap(),
        "keep this draft"
    );
    assert!(
        roots
            .operation_repository(None, None, &parent.path)
            .unwrap()
            .unshallow_spec(&token)
            .is_err()
    );
}

#[test]
fn startup_notice_is_once_per_project_and_requires_a_fetch_remote() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let token = Cancel::default();
    let path = fixture.0.join("preferences/checked");
    let store = plico::unshallow_notice::Store::new(path.clone());
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(store.check(&repo, &cancelled).is_err());
    assert!(!path.exists());
    assert!(store.check(&repo, &token).unwrap());
    assert!(
        !plico::unshallow_notice::Store::new(path)
            .check(&repo, &token)
            .unwrap()
    );
    let no_remote = plico::unshallow_notice::Store::new(fixture.0.join("preferences/no-remote"));
    git(&repo.path, &["remote", "remove", "origin"]);
    assert!(!no_remote.check(&repo, &token).unwrap());
    git(&repo.path, &["remote", "add", "origin", "../remote.git"]);
    assert!(
        !no_remote.check(&repo, &token).unwrap(),
        "upstream consumes the startup check before testing remotes"
    );
    let complete = plico::unshallow_notice::Store::new(fixture.0.join("preferences/complete"));
    assert!(
        !complete
            .check(&Repository::new(fixture.0.join("seed")), &token)
            .unwrap()
    );
    let competing = plico::unshallow_notice::Store::new(fixture.0.join("preferences/competing"));
    std::thread::scope(|scope| {
        let first = scope.spawn(|| competing.check(&repo, &token).unwrap());
        let second = scope.spawn(|| competing.check(&repo, &token).unwrap());
        assert_eq!(
            usize::from(first.join().unwrap()) + usize::from(second.join().unwrap()),
            1,
            "two windows must not both show the first-open notice"
        );
    });
}

#[test]
fn uses_tracking_remote_and_preserves_shallow_state_on_failure_or_cancel() {
    let f = Fixture::new();
    let repo = f.repo();
    git(&repo.path, &["remote", "rename", "origin", "upstream"]);
    git(&repo.path, &["remote", "add", "origin", "../missing.git"]);
    let token = Cancel::default();
    assert_eq!(repo.unshallow_spec(&token).unwrap().remote(), "upstream");
    git(
        &repo.path,
        &["remote", "set-url", "upstream", "../missing.git"],
    );
    let spec = repo.unshallow_spec(&token).unwrap();
    let result = plico::fetch::execute(&repo, &[spec], &token, &Default::default()).unwrap();
    assert!(!result.successful());
    assert!(result.diagnostic().unwrap().contains("missing.git"));
    assert_eq!(
        git(&repo.path, &["rev-parse", "--is-shallow-repository"]),
        "true"
    );
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(repo.unshallow_spec(&cancelled).is_err());
    git(&repo.path, &["remote", "remove", "origin"]);
    git(&repo.path, &["remote", "remove", "upstream"]);
    assert!(repo.unshallow_spec(&token).is_err());
}

#[test]
fn cancelling_an_inflight_fetch_keeps_the_repository_shallow_and_retriable() {
    use std::{
        net::{Ipv4Addr, TcpListener},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let original = git(&repo.path, &["remote", "get-url", "origin"]);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!(
        "http://127.0.0.1:{}/slow.git",
        listener.local_addr().unwrap().port()
    );
    git(&repo.path, &["config", "http.proxy", ""]);
    git(&repo.path, &["remote", "set-url", "origin", &url]);
    let token = Cancel::default();
    let server_stop = token.clone();
    let (seen_tx, seen_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !server_stop.cancelled() {
            match listener.accept() {
                Ok((socket, _)) => {
                    let _ = seen_tx.send(());
                    while !server_stop.cancelled() && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(5));
                    }
                    drop(socket);
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("{error}"),
            }
        }
    });
    let worker_repo = repo.clone();
    let worker_token = token.clone();
    let spec = repo.unshallow_spec(&token).unwrap();
    let worker = thread::spawn(move || {
        plico::fetch::execute(&worker_repo, &[spec], &worker_token, &Default::default())
    });
    let request_started = seen_rx.recv_timeout(Duration::from_secs(5)).is_ok();
    token.cancel();
    let result = worker.join().unwrap().unwrap();
    server.join().unwrap();
    assert!(request_started, "fetch did not reach the local test server");
    assert_eq!(
        result.remotes[0].state,
        plico::fetch::RemoteState::Cancelled
    );
    assert_eq!(
        git(&repo.path, &["rev-parse", "--is-shallow-repository"]),
        "true"
    );
    git(&repo.path, &["remote", "set-url", "origin", &original]);
    let token = Cancel::default();
    let spec = repo.unshallow_spec(&token).unwrap();
    assert!(
        plico::fetch::execute(&repo, &[spec], &token, &Default::default())
            .unwrap()
            .successful()
    );
    assert_eq!(git(&repo.path, &["rev-list", "--count", "HEAD"]), "5");
}
