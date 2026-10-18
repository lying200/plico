use plico::{askpass::Server, auth_request::{Kind, Pending, Requests}, git::Cancel};
use std::{process::{Command, Stdio}, thread, time::{Duration, Instant}};
fn helper() -> Command {
    Command::new(std::env::var_os("GITUI_TEST_ASKPASS_EXE").unwrap_or_else(|| env!("CARGO_BIN_EXE_gitui-askpass").into()))
}
fn pending(requests: &Requests) -> Pending {
    let start = Instant::now();
    loop {
        if let Some(prompt) = requests.take_pending() { return prompt; }
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn native_helper_routes_prompt_and_returns_exact_reply() {
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    let mut command = helper(); server.configure(&mut command);
    let child = command.arg("Username for 'https://example.invalid':").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let prompt = pending(&requests);
    assert_eq!(prompt.kind, Kind::HttpUsername);
    assert_eq!(prompt.prompt, "Username for 'https://example.invalid':");
    prompt.answer("测试 user".into()).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap().trim_end(), "测试 user");
    assert!(result.stderr.is_empty());
}
#[test]
fn dismissing_prompt_returns_no_secret_and_cancels_operation() {
    let cancel = Cancel::default();
    let requests = Requests::new(cancel.clone());
    let server = Server::start(requests.clone()).unwrap();
    let mut command = helper(); server.configure(&mut command);
    let child = command.arg("Password:").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    drop(pending(&requests));
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    assert!(result.stdout.is_empty());
    assert!(cancel.cancelled());
}
#[test]
fn server_drop_releases_waiting_helper_and_wrong_session_is_rejected() {
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    let mut wrong = helper(); server.configure(&mut wrong);
    let result = wrong.env("GITUI_ASKPASS_TOKEN", "0".repeat(64)).arg("Password:").output().unwrap();
    assert!(!result.status.success());
    assert!(requests.take_pending().is_none());
    let mut command = helper(); server.configure(&mut command);
    let child = command.arg("Password:").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let prompt = pending(&requests);
    let start = Instant::now();
    drop(server);
    let result = child.wait_with_output().unwrap();
    assert!(result.stdout.is_empty());
    assert!(start.elapsed() < Duration::from_secs(3));
    drop(prompt);
}

#[test]
fn real_git_credential_fill_uses_helper_for_username_and_password() {
    use std::io::Write;
    let requests = Requests::new(Cancel::default());
    let server = Server::start(requests.clone()).unwrap();
    let mut command = Command::new("git");
    server.configure(&mut command);
    command.args(["-c", "credential.helper=", "credential", "fill"])
        .env("GIT_ASKPASS", helper().get_program())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GITUI_ASKPASS_SSH")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"protocol=https\nhost=gitui-auth-test.invalid\n\n").unwrap();
    let username = pending(&requests);
    assert_eq!(username.kind, Kind::HttpUsername);
    username.answer("fixture-user".into()).unwrap();
    let password = pending(&requests);
    assert_eq!(password.kind, Kind::HttpPassword);
    password.answer("fixture-password".into()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(output.lines().any(|line| line == "username=fixture-user"));
    assert!(output.lines().any(|line| line == "password=fixture-password"));
    // credential fill does not store credentials or contact this nonexistent host.
}
