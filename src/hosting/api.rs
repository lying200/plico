use super::{Account, Credentials, Provider, server};
use crate::git::Cancel;
use serde_json::Value;
use std::{io::Read, time::Duration};
pub(super) fn request(
    url: &str,
    token: Option<&str>,
    form: Option<&[(&str, &str)]>,
    cancel: &Cancel,
) -> Result<Value, String> {
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(20))
        .build();
    let mut req = if form.is_some() {
        agent.post(url)
    } else {
        agent.get(url)
    }
    .set("Accept", "application/json")
    .set("User-Agent", "Plico");
    if let Some(token) = token {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    let response = if let Some(form) = form {
        req.send_form(form)
    } else {
        req.call()
    };
    let response = response.map_err(|e| match e {
        ureq::Error::Status(401, _) => "Account needs to log in again".into(),
        ureq::Error::Status(403, _) => {
            "Access denied: check account permissions or organization policy".into()
        }
        ureq::Error::Status(code, _) => format!("HTTP request failed ({code})"),
        _ => "Could not connect to hosting service".into(),
    })?;
    let mut data = Vec::new();
    response
        .into_reader()
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|_| "Could not read hosting response")?;
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    if data.len() > 2 * 1024 * 1024 {
        return Err("Hosting response is too large".into());
    }
    serde_json::from_slice(&data).map_err(|_| "Invalid hosting response".into())
}
fn api(provider: Provider, server: &str) -> String {
    if provider == Provider::GitHub {
        "https://api.github.com".into()
    } else {
        format!("{server}/api/v4")
    }
}
pub fn validate_token(
    provider: Provider,
    input: &str,
    credentials: &Credentials,
    cancel: &Cancel,
) -> Result<Account, String> {
    let server = server(provider, input)?;
    let value = request(
        &format!("{}/user", api(provider, &server)),
        Some(&credentials.access),
        None,
        cancel,
    )?;
    let login = value[if provider == Provider::GitHub {
        "login"
    } else {
        "username"
    }]
    .as_str()
    .filter(|s| !s.is_empty())
    .ok_or("Missing account username")?;
    Ok(Account {
        provider,
        server,
        login: login.into(),
    })
}
#[derive(Clone, Debug)]
pub struct Repository {
    pub name: String,
    pub url: String,
    pub private: bool,
}
/// Page-sized loading, no unbounded fetch or cross-origin Link following.
pub fn repositories(
    account: &Account,
    page: u32,
    cancel: &Cancel,
) -> Result<Vec<Repository>, String> {
    let credentials = super::credentials(account, cancel)?;
    let base = api(account.provider, &account.server);
    let url = match account.provider {
        Provider::GitHub => format!(
            "{base}/user/repos?affiliation=owner,collaborator,organization_member&sort=full_name&per_page=100&page={page}"
        ),
        Provider::GitLab => format!(
            "{base}/projects?membership=true&order_by=path&sort=asc&per_page=100&page={page}"
        ),
    };
    let data = request(&url, Some(&credentials.access), None, cancel)?;
    parse_repositories(account, data)
}
fn parse_repositories(account: &Account, data: Value) -> Result<Vec<Repository>, String> {
    data.as_array()
        .ok_or("Invalid repository list")?
        .iter()
        .map(|r| {
            let (name, url, private) = match account.provider {
                Provider::GitHub => (
                    "full_name",
                    "clone_url",
                    r["private"].as_bool().unwrap_or(true),
                ),
                Provider::GitLab => (
                    "name_with_namespace",
                    "http_url_to_repo",
                    r["visibility"].as_str() != Some("public"),
                ),
            };
            let url = r[url].as_str().ok_or("Missing repository URL")?;
            let u = url::Url::parse(url).map_err(|_| "Invalid repository URL")?;
            let host = url::Url::parse(&account.server).map_err(|_| "Invalid server URL")?;
            if u.origin() != host.origin()
                || (host.path() != "/"
                    && !u
                        .path()
                        .starts_with(&format!("{}/", host.path().trim_end_matches('/'))))
                || u.query().is_some()
                || u.fragment().is_some()
                || !u.username().is_empty()
                || u.password().is_some()
            {
                return Err("Unexpected repository host".into());
            }
            Ok(Repository {
                name: r[name]
                    .as_str()
                    .or_else(|| r["path_with_namespace"].as_str())
                    .ok_or("Missing repository name")?
                    .into(),
                url: url.into(),
                private,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_response_cannot_redirect_credentials() {
        let account = Account {
            provider: Provider::GitLab,
            server: "https://git.example/team".into(),
            login: "alice".into(),
        };
        for target in [
            "https://other.example/team/repo.git",
            "http://git.example/team/repo.git",
            "https://git.example/teammate/repo.git",
            "https://alice:secret@git.example/team/repo.git",
        ] {
            assert!(
                parse_repositories(
                    &account,
                    serde_json::json!([{"path_with_namespace":"repo", "http_url_to_repo":target}])
                )
                .is_err()
            );
        }
        let repos = parse_repositories(&account, serde_json::json!([{"path_with_namespace":"group/repo", "http_url_to_repo":"https://git.example/team/group/repo.git"}])).unwrap();
        assert_eq!(repos[0].name, "group/repo");
    }
    #[test]
    fn gitlab_displays_namespaces_without_changing_clone_path() {
        let account = Account {
            provider: Provider::GitLab,
            server: "https://git.example/team".into(),
            login: "alice".into(),
        };
        let repos = parse_repositories(
            &account,
            serde_json::json!([{
                "name_with_namespace": "Platform / Client Tools / Desktop App",
                "path_with_namespace": "platform/client-tools/desktop-app",
                "http_url_to_repo": "https://git.example/team/platform/client-tools/desktop-app.git"
            }]),
        )
        .unwrap();
        assert_eq!(repos[0].name, "Platform / Client Tools / Desktop App");
        assert_eq!(
            repos[0].url,
            "https://git.example/team/platform/client-tools/desktop-app.git"
        );
    }
    #[test]
    fn request_does_not_follow_redirect_or_expose_response_secrets() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            let read = socket.read(&mut request).unwrap();
            assert!(read > 0);
            socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: https://untrusted.invalid/token\r\nContent-Length: 12\r\nConnection: close\r\n\r\nsecret-token").unwrap();
        });
        let error = request(
            &format!("http://{address}"),
            Some("fixture-token"),
            None,
            &Cancel::default(),
        )
        .unwrap_err();
        assert!(!error.contains("secret-token"));
        assert!(!error.contains("fixture-token"));
        worker.join().unwrap();
    }
}

/// Avatar retrieval is independent from repository loading. Never forward a token
/// to the avatar CDN or follow a redirect with credentials.
pub fn avatar(account: &Account, cancel: &Cancel) -> Result<(String, Vec<u8>), String> {
    let credentials = super::credentials(account, cancel)?;
    let details = request(
        &format!("{}/user", api(account.provider, &account.server)),
        Some(&credentials.access),
        None,
        cancel,
    )?;
    let raw = details["avatar_url"].as_str().ok_or("No avatar")?;
    let mut url = url::Url::parse(&format!("{}/", account.server))
        .map_err(|_| "Invalid avatar URL")?
        .join(raw)
        .map_err(|_| "Invalid avatar URL")?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("Invalid avatar URL".into());
    }
    if account.provider == Provider::GitHub {
        url.query_pairs_mut().append_pair("s", "96");
    }
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    let response = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(10))
        .build()
        .get(url.as_str())
        .set("User-Agent", "Plico")
        .call()
        .map_err(|_| "Could not load avatar")?;
    let mime = response
        .header("Content-Type")
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    if !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/webp") {
        return Err("Unsupported avatar".into());
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not load avatar")?;
    if bytes.len() > 1024 * 1024 || cancel.cancelled() {
        return Err("Avatar unavailable".into());
    }
    Ok((mime, bytes))
}
