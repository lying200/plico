//! Direct public-client Authorization Code + PKCE. Lifecycle from Rebased
//! GitLabOAuthService (4aec352, Apache-2.0); GitHub uses direct platform endpoints.
use super::{Account, Credentials, Provider, api::request, server};
use crate::{credential_store::Store, git::Cancel};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const PORT: u16 = 34561;
const PATH: &str = "/oauth/callback";
const RESULT_PATH: &str = "/oauth/result";
pub const REDIRECT_URI: &str = "http://127.0.0.1:34561/oauth/callback";
pub struct Config {
    pub client_id: String,
    pub client_secret: String,
}
pub fn config(provider: Provider, host: &str) -> Result<Config, String> {
    let host = server(provider, host)?;
    if provider == Provider::GitLab {
        if let Some(client_id) = custom_client_ids(&custom_config_text()?)?.remove(&host) {
            return Ok(Config {
                client_id,
                client_secret: String::new(),
            });
        }
    }
    let raw = Store::system()
        .get(&format!("oauth:config:{host}"))?
        .unwrap_or_else(|| "{}".into());
    let v: Value = serde_json::from_str(&raw).map_err(|_| "Invalid OAuth configuration")?;
    // Public application identities belong to the distributed client, not its login form.
    // Packagers supply GitHub's application credential at build time; user tokens never do.
    let (default_id, default_secret) = match provider {
        Provider::GitHub => (
            "Ov23liDKwQoqAQptBsRl",
            option_env!("PLICO_GITHUB_CLIENT_SECRET").unwrap_or(""),
        ),
        Provider::GitLab if host == Provider::GitLab.server() => (
            "5e8285ed201e3dad521e49be59b7d35b3a4ebd48478e1eb9d6a61b6f73bd401b",
            "",
        ),
        _ => ("", ""),
    };
    Ok(Config {
        client_id: v["client_id"].as_str().unwrap_or(default_id).into(),
        client_secret: v["client_secret"].as_str().unwrap_or(default_secret).into(),
    })
}
pub fn custom_config_text() -> Result<String, String> {
    Ok(Store::system()
        .get("oauth:gitlab-custom-servers")?
        .unwrap_or_default())
}
pub fn save_custom_config(text: &str) -> Result<(), String> {
    custom_client_ids(text)?;
    if text.len() > 2400 {
        return Err("OAuth configuration is too large".into());
    }
    Store::system().set("oauth:gitlab-custom-servers", text)
}
fn custom_client_ids(text: &str) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut result = std::collections::BTreeMap::new();
    for line in text.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let (host, client) = line
            .split_once('=')
            .ok_or("Use one server URL = client ID per line")?;
        let host = server(Provider::GitLab, host.trim())?;
        let client = client.trim();
        if client.is_empty() || client.chars().any(char::is_whitespace) || client.contains('=') {
            return Err("Use one server URL = client ID per line".into());
        }
        if result.insert(host, client.into()).is_some() {
            return Err("Duplicate OAuth server".into());
        }
    }
    Ok(result)
}
pub fn save_config(provider: Provider, host: &str, c: &Config) -> Result<(), String> {
    let host = server(provider, host)?;
    Store::system().set(
        &format!("oauth:config:{host}"),
        &json!({"client_id":c.client_id.trim(),"client_secret":c.client_secret}).to_string(),
    )
}
pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random() -> Result<String, String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|_| "Could not generate OAuth verifier")?;
    Ok(URL_SAFE_NO_PAD.encode(b))
}
fn endpoint(provider: Provider, host: &str, kind: &str) -> String {
    match provider {
        Provider::GitHub => format!("{host}/login/oauth/{kind}"),
        Provider::GitLab => format!("{host}/oauth/{kind}"),
    }
}
fn token_response(v: Value, c: &Config) -> Result<Credentials, String> {
    if v.get("error").is_some() {
        return Err(
            "OAuth authorization failed; check application configuration or log in again".into(),
        );
    }
    Ok(Credentials {
        access: v["access_token"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Missing OAuth access token")?
            .into(),
        refresh: v["refresh_token"].as_str().map(str::to_owned),
        expires_at: v["expires_in"].as_u64().map(|s| now().saturating_add(s)),
        client_id: c.client_id.clone(),
        client_secret: c.client_secret.clone(),
    })
}
fn authorize_url(
    provider: Provider,
    host: &str,
    c: &Config,
    state: &str,
    verifier: &str,
) -> String {
    let mut u = url::Url::parse(&endpoint(provider, host, "authorize")).unwrap();
    u.query_pairs_mut().extend_pairs([
        ("client_id", c.client_id.as_str()),
        ("redirect_uri", REDIRECT_URI),
        ("response_type", "code"),
        ("state", state),
        ("code_challenge_method", "S256"),
        (
            "code_challenge",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        ),
        (
            "scope",
            match provider {
                Provider::GitHub => "repo read:user read:org workflow",
                Provider::GitLab => "read_api read_user write_repository",
            },
        ),
    ]);
    u.to_string()
}
fn callback(target: &str, state: &str) -> Result<String, String> {
    let u = url::Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "Invalid authorization callback")?;
    if u.path() != PATH {
        return Err("Invalid authorization callback".into());
    }
    let pairs: Vec<_> = u.query_pairs().collect();
    if pairs.iter().filter(|(k, _)| k == "state").count() != 1
        || pairs
            .iter()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.as_ref())
            != Some(state)
    {
        return Err("Invalid authorization state".into());
    }
    if pairs.iter().any(|(k, _)| k == "error") {
        return Err("Authorization was denied".into());
    }
    if pairs.iter().filter(|(k, _)| k == "code").count() != 1 {
        return Err("Missing authorization code".into());
    }
    pairs
        .into_iter()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .filter(|s| !s.is_empty())
        .ok_or("Missing authorization code".into())
}
/// Finish the browser on a code-free URL. The listener is still owned by this
/// authorization and cannot outlive cancellation or keep the port indefinitely.
fn result_page() -> String {
    use crate::i18n::{is_chinese, t};
    // Only trusted, static translations enter this template; no callback data.
    include_str!("../../assets/html/oauth-result.html")
        .replace("{{lang}}", if is_chinese() { "zh-CN" } else { "en" })
        .replace("{{title}}", t("Authorization response received"))
        .replace(
            "{{message}}",
            t("Return to Plico to view the sign-in result."),
        )
        .replace("{{hint}}", t("You can close this browser tab."))
}
// Windows accepted sockets inherit the listener's nonblocking mode. Receive
// timeouts alone do not make reads wait for a browser's delayed TCP packets.
fn configure_callback_socket(socket: &TcpStream, timeout: Duration) -> std::io::Result<()> {
    socket.set_nonblocking(false)?;
    socket.set_read_timeout(Some(timeout))?;
    socket.set_write_timeout(Some(timeout))
}

fn finish_browser(listener: &TcpListener, cancel: &Cancel) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !cancel.cancelled() && Instant::now() < deadline {
        match listener.accept() {
            Ok((mut socket, _)) => {
                if configure_callback_socket(&socket, Duration::from_millis(250)).is_err() {
                    continue;
                }
                let mut bytes = [0u8; 8192];
                let mut count = 0;
                while count < bytes.len()
                    && !cancel.cancelled()
                    && Instant::now() < deadline
                    && !bytes[..count].windows(4).any(|s| s == b"\r\n\r\n")
                {
                    match socket.read(&mut bytes[count..]) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => count += n,
                    }
                }
                let valid = std::str::from_utf8(&bytes[..count])
                    .ok()
                    .and_then(|s| s.lines().next())
                    .is_some_and(|s| s == format!("GET {RESULT_PATH} HTTP/1.1"));
                let body = if valid {
                    result_page()
                } else {
                    "Not found".to_owned()
                };
                let content_type = if valid { "text/html" } else { "text/plain" };
                let status = if valid { "200 OK" } else { "404 Not Found" };
                let _ = write!(
                    socket,
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                if valid {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(_) => break,
        }
    }
}
fn check_authorization_wait(cancel: &Cancel, deadline: Instant) -> Result<(), String> {
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    if Instant::now() >= deadline {
        return Err("Authorization timed out".into());
    }
    Ok(())
}

pub fn authorize(
    provider: Provider,
    host: &str,
    c: &Config,
    cancel: &Cancel,
    open: impl FnOnce(String) -> Result<(), String>,
) -> Result<Credentials, String> {
    let host = server(provider, host)?;
    if c.client_id.trim().is_empty() || (provider == Provider::GitHub && c.client_secret.is_empty())
    {
        return Err(
            "Browser login is unavailable in this build or for this server; use a token".into(),
        );
    }
    let listener =
        TcpListener::bind(("127.0.0.1", PORT)).map_err(|_| "OAuth callback port is in use")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "Could not start OAuth callback")?;
    let state = random()?;
    let verifier = random()?;
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    open(authorize_url(provider, &host, c, &state, &verifier))?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let code = loop {
        check_authorization_wait(cancel, deadline)?;
        match listener.accept() {
            Ok((mut socket, _)) => {
                if configure_callback_socket(&socket, Duration::from_millis(500)).is_err() {
                    continue;
                }
                let mut bytes = Vec::new();
                let mut b = [0u8; 1024];
                while bytes.len() < 8192 && !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    check_authorization_wait(cancel, deadline)?;
                    let remaining = (8192 - bytes.len()).min(b.len());
                    match socket.read(&mut b[..remaining]) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => bytes.extend_from_slice(&b[..n]),
                    }
                }
                // Cancellation also wins when the final read completes the headers.
                check_authorization_wait(cancel, deadline)?;
                let line = std::str::from_utf8(&bytes)
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("");
                let target = line
                    .strip_prefix("GET ")
                    .and_then(|s| s.strip_suffix(" HTTP/1.1"));
                let result = target
                    .ok_or("Invalid authorization callback".into())
                    .and_then(|t| callback(t, &state));
                let valid_state = target
                    .and_then(|t| url::Url::parse(&format!("http://localhost{t}")).ok())
                    .is_some_and(|u| u.query_pairs().any(|(k, v)| k == "state" && v == state));
                let response = if valid_state {
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: {RESULT_PATH}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
                    )
                } else {
                    "HTTP/1.1 400 Bad Request\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: 0\r\n\r\n".into()
                };
                let _ = socket.write_all(response.as_bytes());
                drop(socket);
                if valid_state {
                    finish_browser(&listener, cancel);
                    break result?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(40))
            }
            Err(_) => return Err("OAuth callback failed".into()),
        }
    };
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    let mut form = vec![
        ("client_id", c.client_id.as_str()),
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("code_verifier", &verifier),
        ("redirect_uri", REDIRECT_URI),
    ];
    if provider == Provider::GitHub {
        form.push(("client_secret", &c.client_secret));
    }
    token_response(
        request(
            &endpoint(
                provider,
                &host,
                if provider == Provider::GitHub {
                    "access_token"
                } else {
                    "token"
                },
            ),
            None,
            Some(&form),
            cancel,
        )?,
        c,
    )
}
pub(super) fn refresh(
    account: &Account,
    c: &Credentials,
    cancel: &Cancel,
) -> Result<Credentials, String> {
    let refresh = c
        .refresh
        .as_deref()
        .ok_or("Account needs to log in again")?;
    let config = Config {
        client_id: c.client_id.clone(),
        client_secret: c.client_secret.clone(),
    };
    let mut form = vec![
        ("client_id", config.client_id.as_str()),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh),
        ("redirect_uri", REDIRECT_URI),
    ];
    if account.provider == Provider::GitHub {
        form.push(("client_secret", &config.client_secret));
    }
    token_response(
        request(
            &endpoint(
                account.provider,
                &account.server,
                if account.provider == Provider::GitHub {
                    "access_token"
                } else {
                    "token"
                },
            ),
            None,
            Some(&form),
            cancel,
        )?,
        &config,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_servers_validate_before_saving_and_normalize_duplicates() {
        assert_eq!(
            custom_client_ids(" https://git.example/team = client-id \n").unwrap()["https://git.example/team"],
            "client-id"
        );
        for value in [
            "https://git.example",
            "https://git.example=",
            "https://git.example=a b",
            "http://git.example=a",
            "https://git.example=a\nhttps://git.example/=b",
        ] {
            assert!(custom_client_ids(value).is_err());
        }
    }
    #[test]
    fn result_page_has_no_code_and_replayed_callbacks_are_not_processed() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || finish_browser(&listener, &Cancel::default()));
        for (path, expected) in [
            ("/oauth/callback?code=fixture", "404 Not Found"),
            (RESULT_PATH, "200 OK"),
        ] {
            let mut client = std::net::TcpStream::connect(address).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            // TCP may be accepted before the browser writes, and headers may
            // arrive in multiple packets (notably on Windows).
            std::thread::sleep(Duration::from_millis(100));
            client.write_all(b"GET ").unwrap();
            std::thread::sleep(Duration::from_millis(100));
            write!(client, "{path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut reply = String::new();
            client.read_to_string(&mut reply).unwrap();
            assert!(reply.contains(expected));
            assert!(reply.contains("Referrer-Policy: no-referrer"));
            assert!(!reply.contains("fixture"));
            if path == RESULT_PATH {
                assert!(reply.contains("Content-Type: text/html; charset=utf-8"));
                assert!(reply.contains("<main>"));
                assert!(!reply.contains("{{"));
                let (headers, body) = reply.split_once("\r\n\r\n").unwrap();
                assert!(headers.contains(&format!("Content-Length: {}", body.len())));
            }
        }
        worker.join().unwrap();
        assert!(TcpListener::bind(address).is_ok());
    }
    #[test]
    fn callback_rejects_foreign_duplicate_and_denied_requests() {
        assert_eq!(
            callback("/oauth/callback?state=ok&code=value", "ok").unwrap(),
            "value"
        );
        for s in [
            "/oauth/callback?state=other&code=value",
            "/oauth/callback?state=ok&state=ok&code=value",
            "/oauth/callback?state=ok&error=access_denied",
            "/other?state=ok&code=value",
        ] {
            assert!(callback(s, "ok").is_err());
        }
    }
    #[test]
    fn cancel_releases_loopback_listener_without_exchanging_tokens() {
        let cancel = Cancel::default();
        let result = authorize(
            Provider::GitLab,
            "https://gitlab.com",
            &Config {
                client_id: "fixture".into(),
                client_secret: String::new(),
            },
            &cancel,
            |_| {
                cancel.cancel();
                Ok(())
            },
        );
        assert!(matches!(result, Err(ref e) if e == "Cancelled"));
        let listener = TcpListener::bind(("127.0.0.1", PORT)).unwrap();
        drop(listener);

        // Keep both listener scenarios in one test: authorize uses the registered
        // fixed callback port, so parallel tests must not compete for it.
        let cancel = Cancel::default();
        let trigger = cancel.clone();
        let began = Instant::now();
        let mut sender = None;
        let result = authorize(
            Provider::GitLab,
            "https://gitlab.com",
            &Config {
                client_id: "local-fixture".into(),
                client_secret: String::new(),
            },
            &cancel,
            |_| {
                let mut socket = std::net::TcpStream::connect(("127.0.0.1", PORT)).unwrap();
                socket.write_all(b"GET /").unwrap();
                sender = Some(std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(100));
                    trigger.cancel();
                    for _ in 0..15 {
                        if socket.write_all(b"x").is_err() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(150));
                    }
                }));
                Ok(())
            },
        );
        let elapsed = began.elapsed();
        sender.unwrap().join().unwrap();
        assert_eq!(result.err().as_deref(), Some("Cancelled"));
        assert!(
            elapsed < Duration::from_secs(1),
            "cancel waited for partial headers: {elapsed:?}"
        );
        drop(TcpListener::bind(("127.0.0.1", PORT)).unwrap());
    }
    #[test]
    fn token_rotation_keeps_expiration_and_new_refresh_token() {
        let c = Config {
            client_id: "fixture".into(),
            client_secret: String::new(),
        };
        let before = now();
        let credentials = token_response(
            json!({"access_token":"new-access", "refresh_token":"new-refresh", "expires_in":7200}),
            &c,
        )
        .unwrap();
        assert_eq!(credentials.refresh.as_deref(), Some("new-refresh"));
        assert!(credentials.expires_at.unwrap() >= before + 7200);
        assert!(
            token_response(
                json!({"error":"invalid_grant","error_description":"sensitive"}),
                &c
            )
            .err()
            .unwrap()
            .find("sensitive")
            .is_none()
        );
    }
    #[test]
    fn pkce_uses_s256_and_registered_loopback() {
        let c = Config {
            client_id: "plico".into(),
            client_secret: String::new(),
        };
        let u = url::Url::parse(&authorize_url(
            Provider::GitLab,
            "https://gitlab.com",
            &c,
            "state",
            "verifier",
        ))
        .unwrap();
        let p: std::collections::HashMap<_, _> = u.query_pairs().collect();
        assert_eq!(p["redirect_uri"], REDIRECT_URI);
        assert_eq!(p["code_challenge_method"], "S256");
        assert_ne!(p["code_challenge"], "verifier");
        assert!(!p.contains_key("client_secret"));
    }
}
