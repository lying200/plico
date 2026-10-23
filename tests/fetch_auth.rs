use plico::{
    auth_request::{Kind, Requests},
    fetch::Spec,
    git::{Cancel, Repository},
    operation_process::StopReason,
};
use std::{
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener},
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "gitui fetch auth {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        let fixture = Self(path);
        fixture.git(&["init", "-b", "main"]);
        fixture.git(&["config", "credential.helper", ""]);
        fixture.git(&["config", "http.proxy", ""]);
        fixture
    }
    fn git(&self, args: &[&str]) {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&self.0)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    fn helper(&self) -> PathBuf {
        let source = std::env::var_os("GITUI_TEST_ASKPASS_EXE")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_plico-askpass").into());
        let path = self.0.join(if cfg!(windows) {
            "askpass helper.exe"
        } else {
            "askpass helper"
        });
        fs::copy(source, &path).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
// Local-only HTTP fixture, optionally serving a Git directory after exact Basic
// authentication. No received credentials are persisted or printed.
fn http_server(repository: Option<PathBuf>) -> (String, Cancel, thread::JoinHandle<bool>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let url = format!(
        "http://127.0.0.1:{}/repo.git",
        listener.local_addr().unwrap().port()
    );
    listener.set_nonblocking(true).unwrap();
    let stop = Cancel::default();
    let worker_stop = stop.clone();
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut authorized_request = false;
        while !worker_stop.cancelled() && Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => panic!("HTTP fixture accept: {e}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while request.len() < 16384 && !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buffer).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..n]);
            }
            let request = String::from_utf8_lossy(&request);
            authorized_request |= request
                .to_ascii_lowercase()
                .contains("authorization: basic ");
            let authenticated = request.lines().any(|line| {
                line.split_once(':').is_some_and(|(key, value)| {
                    key.eq_ignore_ascii_case("authorization")
                        && value.trim() == "Basic Zml4dHVyZS11c2VyOmZpeHR1cmUtcGFzc3dvcmQ="
                })
            });
            // These endpoints fail only after authentication, so the tests
            // exercise cleanup of a credential actually supplied to Git.
            if authenticated {
                let first_line = request.lines().next().unwrap_or_default();
                let failure = if first_line.starts_with("GET /unavailable.git/") {
                    Some("500 Internal Server Error")
                } else if first_line.starts_with("GET /forbidden.git/") {
                    Some("403 Forbidden")
                } else {
                    None
                };
                if let Some(status) = failure {
                    let _ = stream.write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .as_bytes(),
                    );
                    continue;
                }
            }
            if let Some(root) = repository.as_ref().filter(|_| authenticated) {
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.strip_prefix("GET "))
                    .and_then(|line| line.split_once(' '))
                    .map(|(path, _)| path)
                    .and_then(|path| path.split('?').next())
                    .and_then(|path| {
                        path.strip_prefix("/repo.git/")
                            .or_else(|| path.strip_prefix("/other.git/"))
                            .or_else(|| path.strip_prefix("/gitlab/repo.git/"))
                            .or_else(|| path.strip_prefix("/gitlab/other.git/"))
                    });
                let body = path
                    .filter(|path| {
                        !path.is_empty()
                            && path.split('/').all(|part| {
                                !part.is_empty()
                                    && part != "."
                                    && part != ".."
                                    && !part.contains(['\\', ':', '%'])
                            })
                    })
                    .and_then(|path| fs::read(root.join(path)).ok());
                let (status, body) =
                    body.map_or(("404 Not Found", Vec::new()), |body| ("200 OK", body));
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
                continue;
            }
            let _ = stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"fixture\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
        authorized_request
    });
    (url, stop, worker)
}
fn verify(cancel_prompt: bool, successful_auth: bool, pull: bool) {
    let fixture = Fixture::new();
    let helper = fixture.helper();
    let source = Fixture::new();
    source.git(&["config", "user.name", "Fixture"]);
    source.git(&["config", "user.email", "fixture@example.invalid"]);
    source.git(&["config", "commit.gpgsign", "false"]);
    fs::write(
        source.0.join("review.txt"),
        "authenticated remote content\n",
    )
    .unwrap();
    source.git(&["add", "review.txt"]);
    source.git(&["commit", "-m", "authenticated source"]);
    if pull {
        fixture.git(&["remote", "add", "origin", source.0.to_str().unwrap()]);
        fixture.git(&["fetch", "origin"]);
        fixture.git(&["checkout", "-B", "main", "origin/main"]);
        fs::write(source.0.join("after.txt"), "new authenticated commit\n").unwrap();
        source.git(&["add", "after.txt"]);
        source.git(&["commit", "-m", "advance for pull"]);
    }
    source.git(&["update-server-info"]);
    let (url, http_stop, http) = http_server(successful_auth.then(|| source.0.join(".git")));
    fixture.git(&[
        "remote",
        if pull { "set-url" } else { "add" },
        "origin",
        &url,
    ]);
    let repo = Repository::new(fixture.0.clone());
    let cancel = Cancel::default();
    let requests = Requests::new(cancel.clone());
    let worker_requests = requests.clone();
    let worker_cancel = cancel.clone();
    let worker = thread::spawn(move || {
        if pull {
            use plico::pull::{self, CommandSpec, Options, Target};
            let spec = CommandSpec {
                target: Target {
                    remote: "origin".into(),
                    branch: "main".into(),
                },
                options: Options::default(),
                commit_editor: "true".into(),
                sequence_editor: None,
            };
            let result = pull::execute_command(
                &repo,
                &spec,
                &worker_cancel,
                &Default::default(),
                Some((&worker_requests, &helper)),
            )?;
            assert!(!result.state.unwrap().in_progress());
            result.command
        } else {
            repo.fetch_authenticated(
                &Spec::new("origin".into()).unwrap(),
                &worker_cancel,
                &Default::default(),
                &worker_requests,
                &helper,
            )
        }
    });
    let start = Instant::now();
    let mut kinds = Vec::new();
    while !worker.is_finished() {
        if let Some(prompt) = requests.take_pending() {
            kinds.push(prompt.kind);
            if cancel_prompt {
                drop(prompt);
            } else if successful_auth && prompt.kind == Kind::HttpUsername {
                prompt
                    .answer_credentials("fixture-user".into(), "fixture-password".into())
                    .unwrap();
            } else {
                let response = if prompt.kind == Kind::HttpUsername {
                    "fixture-user"
                } else {
                    "fixture-password"
                };
                prompt.answer(response.into()).unwrap();
            }
        }
        if start.elapsed() > Duration::from_secs(10) {
            cancel.cancel();
            http_stop.cancel();
            panic!("fetch authentication did not finish");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let outcome = worker.join().unwrap().unwrap();
    http_stop.cancel();
    let authorized_request = http.join().unwrap();
    if cancel_prompt {
        assert_eq!(kinds, [Kind::HttpUsername]);
        assert_eq!(outcome.stopped, Some(StopReason::Cancelled));
        assert!(!authorized_request);
    } else {
        if successful_auth {
            assert_eq!(kinds, [Kind::HttpUsername]);
        } else {
            assert_eq!(
                kinds,
                [
                    Kind::HttpUsername,
                    Kind::HttpPassword,
                    Kind::HttpUsername,
                    Kind::HttpPassword
                ]
            );
        }
        assert_eq!(outcome.stopped, None);
        assert_eq!(outcome.status.success(), successful_auth);
        assert!(authorized_request);
    }
    assert!(requests.take_pending().is_none());
    let refs = Command::new("git")
        .arg("-C")
        .arg(&fixture.0)
        .arg("show-ref")
        .output()
        .unwrap();
    if successful_auth {
        assert!(refs.status.success());
        let source_head = Command::new("git")
            .arg("-C")
            .arg(&source.0)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        let fetched_head = Command::new("git")
            .arg("-C")
            .arg(&fixture.0)
            .args(["rev-parse", "refs/remotes/origin/main"])
            .output()
            .unwrap();
        assert!(source_head.status.success() && fetched_head.status.success());
        assert_eq!(source_head.stdout, fetched_head.stdout);
        let content = Command::new("git")
            .arg("-C")
            .arg(&fixture.0)
            .args(["show", "origin/main:review.txt"])
            .output()
            .unwrap();
        assert!(content.status.success());
        assert_eq!(content.stdout, b"authenticated remote content\n");
        // Fetch updates refs/objects, not the consumer's worktree or unborn HEAD.
        assert_eq!(fixture.0.join("review.txt").exists(), pull);
        let head = Command::new("git")
            .arg("-C")
            .arg(&fixture.0)
            .args(["rev-parse", "--verify", "HEAD"])
            .output()
            .unwrap();
        assert_eq!(head.status.success(), pull);
        if pull {
            assert_eq!(head.stdout, source_head.stdout);
            assert_eq!(
                fs::read_to_string(fixture.0.join("after.txt")).unwrap(),
                "new authenticated commit\n"
            );
        }
    } else if pull {
        assert!(!fixture.0.join("after.txt").exists());
        let local = Command::new("git")
            .arg("-C")
            .arg(&fixture.0)
            .args(["rev-parse", "HEAD", "origin/main"])
            .output()
            .unwrap();
        let values = String::from_utf8(local.stdout).unwrap();
        let values: Vec<_> = values.lines().collect();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], values[1]);
    } else {
        assert!(refs.stdout.is_empty());
    }
}
#[test]
fn fetch_http_authentication_rejection_preserves_failure() {
    verify(false, false, false);
}
#[test]
fn fetch_http_prompt_dismissal_cancels_operation() {
    verify(true, false, false);
}
#[test]
fn fetch_http_authentication_success_downloads_refs_and_objects() {
    verify(false, true, false);
}

#[test]
fn pull_http_authentication_success_updates_worktree() {
    verify(false, true, true);
}
#[test]
fn pull_http_authentication_rejection_preserves_head() {
    verify(false, false, true);
}
#[test]
fn pull_http_authentication_cancellation_preserves_head() {
    verify(true, false, true);
}

fn verify_push_auth_failure(dismiss: bool) {
    use plico::{
        push::{self, Status},
        push_target::{Origin, Target},
    };
    let fixture = Fixture::new();
    fixture.git(&["config", "user.name", "Fixture"]);
    fixture.git(&["config", "user.email", "fixture@example.invalid"]);
    fixture.git(&["config", "commit.gpgsign", "false"]);
    fs::write(fixture.0.join("review.txt"), "committed\n").unwrap();
    fixture.git(&["add", "."]);
    fixture.git(&["commit", "-m", "source"]);
    fs::write(fixture.0.join("review.txt"), "uncommitted\n").unwrap();
    let (url, http_stop, http) = http_server(None);
    fixture.git(&["remote", "add", "origin", &url]);
    let repo = Repository::new(fixture.0.clone());
    let snapshot = repo.push_snapshot(&Cancel::default()).unwrap().unwrap();
    let oid = snapshot.source_oid;
    let spec = push::Spec {
        tags: None,
        force: false,
        source_branch: "main".into(),
        source_oid: oid.clone(),
        set_upstream: true,
        target: Target {
            remote: "origin".into(),
            branch: "main".into(),
            special: false,
            new_branch: true,
            origin: Origin::TrackingBranch,
        },
    };
    let token = Cancel::default();
    let requests = Requests::new(token.clone());
    let worker_token = token.clone();
    let worker_requests = requests.clone();
    let helper = fixture.helper();
    let worker = thread::spawn(move || {
        push::execute(
            &repo,
            &spec,
            &worker_token,
            &Default::default(),
            Some((&worker_requests, &helper)),
        )
    });
    let start = Instant::now();
    let mut prompts = 0;
    while !worker.is_finished() {
        if let Some(prompt) = requests.take_pending() {
            prompts += 1;
            assert_eq!(prompt.kind, Kind::HttpUsername);
            if dismiss {
                drop(prompt);
            } else {
                prompt
                    .answer_credentials("fixture-user".into(), "fixture-password".into())
                    .unwrap();
            }
        }
        if start.elapsed() > Duration::from_secs(10) {
            token.cancel();
            http_stop.cancel();
            panic!("Push authentication did not finish");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let result = worker.join().unwrap().unwrap();
    http_stop.cancel();
    let sent_auth = http.join().unwrap();
    assert_eq!(prompts, if dismiss { 1 } else { 2 });
    assert_eq!(sent_auth, !dismiss);
    assert_eq!(
        result.status,
        if dismiss {
            Status::Cancelled
        } else {
            Status::Failed
        }
    );
    assert!(!result.state.unwrap().in_progress());
    assert!(requests.take_pending().is_none());
    assert_eq!(
        fs::read_to_string(fixture.0.join("review.txt")).unwrap(),
        "uncommitted\n"
    );
    let snapshot = Repository::new(fixture.0.clone())
        .push_snapshot(&Cancel::default())
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.source_oid, oid);
    assert!(snapshot.targets.tracked.is_none());
    let tracking = Command::new("git")
        .arg("-C")
        .arg(&fixture.0)
        .args(["config", "--get", "branch.main.remote"])
        .output()
        .unwrap();
    assert!(!tracking.status.success());
}
#[test]
fn push_http_authentication_rejection_preserves_failure() {
    verify_push_auth_failure(false);
}
#[test]
fn push_http_prompt_dismissal_cancels_operation() {
    verify_push_auth_failure(true);
}

// Define Remote uses ls-remote, not fetch: authentication must work without
// adding configuration, downloading objects, or changing repository refs.
fn verify_remote_access(dismiss: bool, authorized: bool, clone: bool) {
    let fixture = Fixture::new();
    let source = Fixture::new();
    source.git(&["config", "user.name", "Fixture"]);
    source.git(&["config", "user.email", "fixture@example.invalid"]);
    source.git(&["config", "commit.gpgsign", "false"]);
    fs::write(source.0.join("review.txt"), "remote content\n").unwrap();
    source.git(&["add", "review.txt"]);
    source.git(&["commit", "-m", "source"]);
    source.git(&["update-server-info"]);
    let config_before = fs::read(fixture.0.join(".git/config")).unwrap();
    let (url, stop, server) = http_server(authorized.then(|| source.0.join(".git")));
    let token = Cancel::default();
    let requests = Requests::new(token.clone());
    let worker_token = token.clone();
    let worker_requests = requests.clone();
    let helper = fixture.helper();
    let repo = Repository::new(&fixture.0);
    let destination = fixture.0.join("cloned repository");
    let worker_destination = destination.clone();
    let worker = thread::spawn(move || {
        if clone {
            let request = plico::clone_repository::Request {
                source: url,
                destination: worker_destination,
                depth: None,
                recurse_submodules: false,
            };
            plico::clone_repository::execute(
                &request,
                &worker_token,
                &Default::default(),
                Some((&worker_requests, &helper)),
            )
            .map(|report| report.outcome)
        } else {
            plico::remote_management::check_access(
                &repo,
                &url,
                &worker_requests,
                &helper,
                &worker_token,
                &Default::default(),
            )
        }
    });
    let start = Instant::now();
    let mut prompts = 0;
    while !worker.is_finished() {
        if let Some(prompt) = requests.take_pending() {
            prompts += 1;
            assert_eq!(prompt.kind, Kind::HttpUsername);
            if dismiss {
                drop(prompt);
            } else {
                prompt
                    .answer_credentials("fixture-user".into(), "fixture-password".into())
                    .unwrap();
            }
        }
        if start.elapsed() > Duration::from_secs(10) {
            token.cancel();
            stop.cancel();
            panic!("Remote access authentication did not finish");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let outcome = worker.join().unwrap().unwrap();
    stop.cancel();
    let sent_auth = server.join().unwrap();
    assert_eq!(prompts, if dismiss || authorized { 1 } else { 2 });
    assert_eq!(sent_auth, !dismiss);
    assert_eq!(outcome.stopped, dismiss.then_some(StopReason::Cancelled));
    assert_eq!(outcome.status.success(), authorized && !dismiss);
    if authorized && !dismiss {
        if clone {
            assert_eq!(
                fs::read_to_string(destination.join("review.txt")).unwrap(),
                "remote content\n"
            );
        } else {
            assert!(String::from_utf8_lossy(&outcome.stdout.bytes()).contains("refs/heads/main"));
        }
    } else if clone {
        assert!(!destination.join("review.txt").exists());
    }
    assert_eq!(
        fs::read(fixture.0.join(".git/config")).unwrap(),
        config_before
    );
    let refs = Command::new("git")
        .arg("-C")
        .arg(&fixture.0)
        .arg("show-ref")
        .output()
        .unwrap();
    assert!(refs.stdout.is_empty());
    assert!(!fixture.0.join("review.txt").exists());
    assert!(requests.take_pending().is_none());
}
#[test]
fn remote_access_authentication_checks_refs_without_fetching() {
    verify_remote_access(false, true, false);
}
#[test]
fn remote_access_authentication_rejection_remains_failure() {
    verify_remote_access(false, false, false);
}
#[test]
fn remote_access_authentication_dismissal_cancels_check() {
    verify_remote_access(true, false, false);
}

#[test]
fn clone_http_authentication_success_opens_complete_worktree() {
    verify_remote_access(false, true, true);
}
#[test]
fn clone_http_authentication_failure_is_not_success() {
    verify_remote_access(false, false, true);
}
#[test]
fn clone_http_authentication_dismissal_cancels_operation() {
    verify_remote_access(true, false, true);
}

// Exercise the production command/AskPass/store lifecycle, not a mirrored model.
fn access_with_store(
    fixture: &Fixture,
    url: &str,
    store: plico::credential_store::Store,
    mode: plico::http_auth::Mode,
    answers: &[(&str, bool)],
) -> (bool, usize) {
    let token = Cancel::default();
    let requests = Requests::with_store(token.clone(), mode, store);
    let worker_requests = requests.clone();
    let worker_token = token.clone();
    let repo = Repository::new(fixture.0.clone());
    let url = url.to_owned();
    let helper = fixture.helper();
    let expected_target = url.clone();
    let worker = thread::spawn(move || {
        plico::remote_management::check_access(
            &repo,
            &url,
            &worker_requests,
            &helper,
            &worker_token,
            &Default::default(),
        )
    });
    let start = Instant::now();
    let mut prompts = 0;
    while !worker.is_finished() {
        if let Some(pending) = requests.take_pending() {
            assert_eq!(
                pending.http_target().as_deref(),
                Some(expected_target.as_str()),
                "platform account selection must retain the complete repository target"
            );
            let Some((password, remember)) = answers.get(prompts) else {
                drop(pending);
                panic!("Unexpected authentication prompt");
            };
            assert_eq!(pending.kind, Kind::HttpUsername);
            if *password == "select-test-helper" {
                assert!(pending.helper_available());
                pending.select_helper().unwrap();
                pending.answer(String::new()).unwrap();
                prompts += 1;
                continue;
            }
            pending
                .save_credentials("fixture-user", password, *remember)
                .unwrap();
            pending
                .answer_credentials("fixture-user".into(), (*password).into())
                .unwrap();
            prompts += 1;
        }
        if start.elapsed() > Duration::from_secs(8) {
            token.cancel();
            panic!("Authentication lifecycle timed out");
        }
        thread::sleep(Duration::from_millis(5));
    }
    let result = worker.join().unwrap().unwrap();
    assert!(requests.take_pending().is_none());
    (result.status.success() && result.stopped.is_none(), prompts)
}
#[test]
fn rejected_token_retries_then_saved_token_is_reused_by_silent_operation() {
    use plico::{credential_store::Store, http_auth::Mode};
    let fixture = Fixture::new();
    let source = Fixture::new();
    source.git(&["update-server-info"]);
    let (url, stop, server) = http_server(Some(source.0.join(".git")));
    let store = Store::default();
    assert_eq!(
        access_with_store(
            &fixture,
            &url,
            store.clone(),
            Mode::Full,
            &[("wrong", true), ("fixture-password", true)]
        ),
        (true, 2)
    );
    assert_eq!(
        access_with_store(&fixture, &url, store.clone(), Mode::Silent, &[]),
        (true, 0)
    );
    assert_eq!(
        access_with_store(&fixture, &url, store, Mode::None, &[]),
        (false, 0)
    );
    stop.cancel();
    server.join().unwrap();
}
#[test]
fn unremembered_token_does_not_leak_into_next_operation() {
    use plico::{credential_store::Store, http_auth::Mode};
    let fixture = Fixture::new();
    let source = Fixture::new();
    source.git(&["update-server-info"]);
    let (url, stop, server) = http_server(Some(source.0.join(".git")));
    let store = Store::default();
    assert_eq!(
        access_with_store(
            &fixture,
            &url,
            store.clone(),
            Mode::Full,
            &[("fixture-password", false)]
        ),
        (true, 1)
    );
    assert_eq!(
        access_with_store(&fixture, &url, store, Mode::Silent, &[]),
        (false, 0)
    );
    stop.cancel();
    server.join().unwrap();
}
#[test]
fn failed_saved_token_is_forgotten_after_two_attempts() {
    use plico::{credential_store::Store, http_auth::Mode};
    let fixture = Fixture::new();
    let (url, stop, server) = http_server(None);
    let store = Store::default();
    assert_eq!(
        access_with_store(
            &fixture,
            &url,
            store.clone(),
            Mode::Full,
            &[("wrong", true), ("wrong-again", true)]
        ),
        (false, 2)
    );
    assert_eq!(
        access_with_store(&fixture, &url, store, Mode::Silent, &[]),
        (false, 0)
    );
    stop.cancel();
    server.join().unwrap();
}

#[test]
fn explicit_helper_switch_rebuilds_command_and_none_never_uses_helper() {
    use plico::{credential_store::Store, http_auth::Mode};
    let fixture = Fixture::new();
    let source = Fixture::new();
    source.git(&["update-server-info"]);
    fixture.git(&["config", "credential.useHttpPath", "false"]);
    // A helper that only accepts host-scoped requests detects forced path overrides.
    fixture.git(&["config", "credential.helper", r#"!f() { if test "$1" = get; then while IFS= read -r entry; do case "$entry" in path=*) exit 0;; esac; done; printf '%s\n' username=fixture-user password=fixture-password; fi; }; f"#]);
    let (url, stop, server) = http_server(Some(source.0.join(".git")));
    let store = Store::default();
    assert_eq!(
        access_with_store(
            &fixture,
            &url,
            store.clone(),
            Mode::Full,
            &[("select-test-helper", false)]
        ),
        (true, 1)
    );
    assert_eq!(
        access_with_store(&fixture, &url, store.clone(), Mode::Silent, &[]),
        (true, 0)
    );
    assert_eq!(
        access_with_store(&fixture, &url, store, Mode::None, &[]),
        (false, 0)
    );
    stop.cancel();
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn fetch_routes_native_ssh_prompt_and_cancel_without_http_retry() {
    let fixture = Fixture::new();
    let script = fixture.0.join("ssh fixture.sh");
    fs::write(&script, "#!/bin/sh\n\"$SSH_ASKPASS\" \"Enter passphrase for key '/fixture/key':\" >/dev/null\nexit 1\n").unwrap();
    fixture.git(&[
        "config",
        "core.sshCommand",
        &format!("sh '{}'", script.display()),
    ]);
    fixture.git(&["config", "ssh.variant", "ssh"]);
    fixture.git(&[
        "remote",
        "add",
        "origin",
        "ssh://fixture@127.0.0.1/repo.git",
    ]);
    let helper = fixture.helper();
    let requests = Requests::new(Cancel::default());
    let worker_requests = requests.clone();
    let repo = Repository::new(fixture.0.clone());
    let worker = thread::spawn(move || {
        repo.fetch_authenticated(
            &Spec::new("origin".into()).unwrap(),
            &Cancel::default(),
            &Default::default(),
            &worker_requests,
            &helper,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(prompt) = requests.take_pending() {
            assert_eq!(prompt.kind, Kind::Ssh);
            assert!(!prompt.helper_available());
            drop(prompt);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "SSH prompt did not reach UI bridge"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!worker.join().unwrap().unwrap().status.success());
}

#[test]
fn batch_auth_cancellation_preserves_completed_remote_and_skips_remaining() {
    use plico::fetch::{self, RemoteState};
    let source = Fixture::new();
    source.git(&["config", "user.name", "Fixture"]);
    source.git(&["config", "user.email", "fixture@example.invalid"]);
    source.git(&["config", "commit.gpgsign", "false"]);
    fs::write(source.0.join("review.txt"), "completed fetch\n").unwrap();
    source.git(&["add", "review.txt"]);
    source.git(&["commit", "-m", "source"]);
    let fixture = Fixture::new();
    let helper = fixture.helper();
    let (url, http_stop, http) = http_server(None);
    fixture.git(&["remote", "add", "a-complete", source.0.to_str().unwrap()]);
    fixture.git(&["remote", "add", "b-auth", &url]);
    fixture.git(&["remote", "add", "c-skipped", source.0.to_str().unwrap()]);
    let repo = Repository::new(fixture.0.clone());
    let cancel = Cancel::default();
    let requests = Requests::new(cancel.clone());
    let worker_requests = requests.clone();
    let worker_cancel = cancel.clone();
    let worker = thread::spawn(move || {
        let specs = repo.fetch_specs(&worker_cancel).unwrap();
        fetch::execute_authenticated(
            &repo,
            &specs,
            &worker_cancel,
            &Default::default(),
            &worker_requests,
            &helper,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut dismissed = false;
    while !worker.is_finished() && Instant::now() < deadline {
        if let Some(prompt) = requests.take_pending() {
            assert_eq!(prompt.kind, Kind::HttpUsername);
            dismissed = true;
            drop(prompt);
        }
        thread::sleep(Duration::from_millis(10));
    }
    let timed_out = !worker.is_finished();
    if timed_out {
        cancel.cancel();
    }
    let result = worker.join().unwrap().unwrap();
    http_stop.cancel();
    let authorized = http.join().unwrap();
    assert!(
        !timed_out,
        "batch did not finish after dismissing authentication"
    );
    assert!(dismissed && !authorized);
    assert!(!result.successful());
    assert_eq!(
        result
            .remotes
            .iter()
            .map(|r| (r.remote.as_str(), &r.state))
            .collect::<Vec<_>>(),
        [
            ("a-complete", &RemoteState::Succeeded),
            ("b-auth", &RemoteState::Cancelled),
            ("c-skipped", &RemoteState::NotStarted),
        ]
    );
    let refs = Command::new("git")
        .arg("-C")
        .arg(&fixture.0)
        .args(["for-each-ref", "--format=%(refname)", "refs/remotes/"])
        .output()
        .unwrap();
    assert!(refs.status.success());
    let refs = String::from_utf8(refs.stdout).unwrap();
    assert!(refs.lines().any(|r| r == "refs/remotes/a-complete/main"));
    // Newer Git also records the remote's symbolic HEAD on the first fetch.
    assert!(refs.lines().all(|r| matches!(
        r,
        "refs/remotes/a-complete/main" | "refs/remotes/a-complete/HEAD"
    )));
    let content = Command::new("git")
        .arg("-C")
        .arg(&fixture.0)
        .args(["show", "a-complete/main:review.txt"])
        .output()
        .unwrap();
    assert!(content.status.success());
    assert_eq!(content.stdout, b"completed fetch\n");
    assert!(!fixture.0.join("review.txt").exists());
    assert!(requests.take_pending().is_none());
}

#[test]
fn remembered_http_credentials_follow_git_path_scope_across_projects() {
    use plico::{credential_store::Store, http_auth::Mode};
    let source = Fixture::new();
    source.git(&["update-server-info"]);
    let (url, stop, server) = http_server(Some(source.0.join(".git")));
    let other_url = url.replace("/repo.git", "/other.git");
    let first = Fixture::new();
    let second = Fixture::new();
    for fixture in [&first, &second] {
        fixture.git(&["config", "credential.useHttpPath", "false"]);
    }
    let store = Store::default();
    assert_eq!(
        access_with_store(
            &first,
            &url,
            store.clone(),
            Mode::Full,
            &[("fixture-password", true)]
        ),
        (true, 1)
    );
    let reused = access_with_store(&second, &other_url, store, Mode::Silent, &[]);
    stop.cancel();
    server.join().unwrap();
    assert_eq!(
        reused,
        (true, 0),
        "same server credentials must work across projects when Git path scoping is disabled"
    );
}

#[test]
fn explicit_http_path_scope_prevents_cross_repository_reuse() {
    use plico::{credential_store::Store, http_auth::Mode};
    let source = Fixture::new();
    source.git(&["update-server-info"]);
    let (url, stop, server) = http_server(Some(source.0.join(".git")));
    let other_url = url.replace("/repo.git", "/other.git");
    let first = Fixture::new();
    let second = Fixture::new();
    for fixture in [&first, &second] {
        fixture.git(&["config", "credential.useHttpPath", "true"]);
    }
    let store = Store::default();
    assert_eq!(
        access_with_store(
            &first,
            &url,
            store.clone(),
            Mode::Full,
            &[("fixture-password", true)]
        ),
        (true, 1)
    );
    assert_eq!(
        access_with_store(&second, &other_url, store.clone(), Mode::Silent, &[]),
        (false, 0)
    );
    assert_eq!(
        access_with_store(&second, &url, store.clone(), Mode::Silent, &[]),
        (true, 0)
    );
    assert_eq!(
        access_with_store(
            &second,
            &other_url,
            store,
            Mode::Full,
            &[("fixture-password", true)]
        ),
        (true, 1)
    );
    stop.cancel();
    server.join().unwrap();
}

#[test]
fn url_specific_http_path_scope_is_respected() {
    use plico::{credential_store::Store, http_auth::Mode};
    let source = Fixture::new();
    source.git(&["update-server-info"]);
    let (url, stop, server) = http_server(Some(source.0.join(".git")));
    let other_url = url.replace("/repo.git", "/other.git");
    let fixture = Fixture::new();
    fixture.git(&["config", "credential.useHttpPath", "false"]);
    fixture.git(&[
        "config",
        &format!("credential.{other_url}.useHttpPath"),
        "true",
    ]);
    let store = Store::default();
    assert_eq!(
        access_with_store(
            &fixture,
            &url,
            store.clone(),
            Mode::Full,
            &[("fixture-password", true)]
        ),
        (true, 1)
    );
    assert_eq!(
        access_with_store(&fixture, &other_url, store.clone(), Mode::Silent, &[]),
        (false, 0)
    );
    assert_eq!(
        access_with_store(&fixture, &url, store, Mode::Silent, &[]),
        (true, 0)
    );
    stop.cancel();
    server.join().unwrap();
}

#[test]
fn subpath_hosting_target_survives_shared_generic_credentials() {
    use plico::{credential_store::Store, http_auth::Mode};
    let source = Fixture::new();
    source.git(&["update-server-info"]);
    let (url, stop, server) = http_server(Some(source.0.join(".git")));
    let first_url = url.replace("/repo.git", "/gitlab/repo.git");
    let second_url = url.replace("/repo.git", "/gitlab/other.git");
    let fixture = Fixture::new();
    fixture.git(&["config", "credential.useHttpPath", "false"]);
    let store = Store::default();
    // access_with_store asserts that Pending.http_target remains the full URL,
    // so hosted matching and per-repository rejection retain their identity.
    assert_eq!(
        access_with_store(
            &fixture,
            &first_url,
            store.clone(),
            Mode::Full,
            &[("fixture-password", true)]
        ),
        (true, 1)
    );
    assert_eq!(
        access_with_store(&fixture, &second_url, store, Mode::Silent, &[]),
        (true, 0)
    );
    stop.cancel();
    server.join().unwrap();
}

#[test]
fn shared_credentials_distinguish_server_failure_from_authentication_rejection() {
    use plico::{credential_store::Store, http_auth::Mode};
    let source = Fixture::new();
    source.git(&["update-server-info"]);
    let (url, stop, server) = http_server(Some(source.0.join(".git")));
    let first = Fixture::new();
    let second = Fixture::new();
    for fixture in [&first, &second] {
        fixture.git(&["config", "credential.useHttpPath", "false"]);
    }
    for (endpoint, retained) in [("unavailable.git", true), ("forbidden.git", false)] {
        let store = Store::default();
        assert_eq!(
            access_with_store(
                &first,
                &url,
                store.clone(),
                Mode::Full,
                &[("fixture-password", true)]
            ),
            (true, 1)
        );
        let failed_url = url.replace("repo.git", endpoint);
        assert_eq!(
            access_with_store(&second, &failed_url, store.clone(), Mode::Silent, &[]),
            (false, 0)
        );
        assert_eq!(
            access_with_store(&first, &url, store, Mode::Silent, &[]),
            (retained, 0),
            "shared credential lifetime after {endpoint} must follow Git authentication classification"
        );
    }
    stop.cancel();
    assert!(server.join().unwrap());
}
