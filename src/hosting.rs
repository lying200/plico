//! Hosted Git accounts. Rebased 4aec352 GitLabAccountManager/GH clone models,
//! Apache-2.0, JetBrains and contributors. No collaboration/PR API integration.
mod api;
pub(crate) mod failures;
pub mod oauth;
use crate::{credential_store::Store, git::Cancel};
pub use api::{Repository, avatar, repositories, validate_token};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
static ACCOUNTS: Mutex<()> = Mutex::new(());
static REFRESH: Mutex<()> = Mutex::new(());
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    GitHub,
    GitLab,
}
impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::GitLab => "GitLab",
        }
    }
    pub fn server(self) -> &'static str {
        match self {
            Self::GitHub => "https://github.com",
            Self::GitLab => "https://gitlab.com",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub provider: Provider,
    pub server: String,
    pub login: String,
}
impl Account {
    pub fn id(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(
                format!("{}|{}|{}", self.provider.name(), self.server, self.login).as_bytes()
            )
        )
    }
    fn value(&self) -> Value {
        json!({"provider": self.provider.name(), "server":self.server,"login":self.login})
    }
}
/// Never derive Debug: all fields except expiration/client ID can hold secrets.
pub struct Credentials {
    pub access: String,
    pub refresh: Option<String>,
    pub expires_at: Option<u64>,
    pub client_id: String,
    pub client_secret: String,
}
impl Credentials {
    pub fn token(access: String) -> Self {
        Self {
            access,
            refresh: None,
            expires_at: None,
            client_id: String::new(),
            client_secret: String::new(),
        }
    }
    fn value(&self) -> Value {
        json!({"access":self.access,"refresh":self.refresh,"expires_at":self.expires_at,"client_id":self.client_id,"client_secret":self.client_secret})
    }
    fn parse(v: Value) -> Result<Self, String> {
        Ok(Self {
            access: v["access"].as_str().ok_or("Missing access token")?.into(),
            refresh: v["refresh"].as_str().map(str::to_owned),
            expires_at: v["expires_at"].as_u64(),
            client_id: v["client_id"].as_str().unwrap_or_default().into(),
            client_secret: v["client_secret"].as_str().unwrap_or_default().into(),
        })
    }
}
pub fn server(provider: Provider, input: &str) -> Result<String, String> {
    let u = url::Url::parse(input.trim()).map_err(|_| "Invalid server URL")?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("Use an HTTPS server URL without credentials".into());
    }
    if provider == Provider::GitHub
        && (u.host_str() != Some("github.com") || u.port().is_some() || u.path() != "/")
    {
        return Err("GitHub Enterprise is not supported".into());
    }
    Ok(u.as_str().trim_end_matches('/').into())
}
fn load(store: &Store) -> Result<Vec<Account>, String> {
    let raw = store
        .get("hosting:accounts")?
        .unwrap_or_else(|| "[]".into());
    let v: Value = serde_json::from_str(&raw).map_err(|_| "Invalid account storage")?;
    v.as_array()
        .ok_or("Invalid account storage")?
        .iter()
        .map(|v| {
            let provider = match v["provider"].as_str() {
                Some("GitHub") => Provider::GitHub,
                Some("GitLab") => Provider::GitLab,
                _ => return Err("Unknown account provider".into()),
            };
            Ok(Account {
                provider,
                server: server(provider, v["server"].as_str().ok_or("Missing server")?)?,
                login: v["login"].as_str().ok_or("Missing login")?.into(),
            })
        })
        .collect()
}
pub fn accounts() -> Result<Vec<Account>, String> {
    let _g = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
    load(&Store::system())
}
pub fn save(account: &Account, credentials: &Credentials, cancel: &Cancel) -> Result<(), String> {
    save_to(&Store::system(), account, credentials, cancel)
}
fn save_to(
    store: &Store,
    account: &Account,
    credentials: &Credentials,
    cancel: &Cancel,
) -> Result<(), String> {
    let _g = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    let mut entries = load(store)?;
    if !entries.contains(account) {
        entries.push(account.clone());
    }
    let index = Value::Array(entries.iter().map(Account::value).collect()).to_string();
    // Keep the index within WinCred's single blob limit; never silently lose accounts.
    if index.len() > 2400 {
        return Err("Account storage is full".into());
    }
    store.set(
        &format!("hosting:token:{}", account.id()),
        &credentials.value().to_string(),
    )?;
    store.set("hosting:accounts", &index)?;
    failures::clear_account(&account.id());
    Ok(())
}
pub fn remove(account: &Account) -> Result<(), String> {
    remove_from(&Store::system(), account)
}
fn remove_from(store: &Store, account: &Account) -> Result<(), String> {
    let _g = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut entries = load(store)?;
    entries.retain(|a| a != account);
    store.set(
        "hosting:accounts",
        &Value::Array(entries.iter().map(Account::value).collect()).to_string(),
    )?;
    let key = format!("hosting:token:{}", account.id());
    if let Some(value) = store.get(&key)? {
        store.remove_if(&key, &value)?;
    }
    Ok(())
}
pub fn credentials(account: &Account, cancel: &Cancel) -> Result<Credentials, String> {
    credentials_from(&Store::system(), account, cancel, oauth::refresh)
}
fn credentials_from(
    store: &Store,
    account: &Account,
    cancel: &Cancel,
    refresh: impl FnOnce(&Account, &Credentials, &Cancel) -> Result<Credentials, String>,
) -> Result<Credentials, String> {
    // Serialize token rotation without holding the UI account-list lock during HTTP.
    let _refresh = REFRESH.lock().unwrap_or_else(|e| e.into_inner());
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    let key = format!("hosting:token:{}", account.id());
    let raw = {
        let _g = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
        // A failed credential deletion may leave a token behind. The account
        // index, not the existence of that token, determines login membership.
        if !load(store)?.contains(account) {
            return Err("Account needs to log in again".into());
        }
        store.get(&key)?.ok_or("Account needs to log in again")?
    };
    let mut c =
        Credentials::parse(serde_json::from_str(&raw).map_err(|_| "Invalid account credentials")?)?;
    if c.expires_at.is_some_and(|e| e <= oauth::now() + 60) {
        c = refresh(account, &c, cancel)?;
        if cancel.cancelled() {
            return Err("Cancelled".into());
        }
        let _g = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
        // A logout or new login during the request must win over this response.
        if !load(store)?.contains(account) || store.get(&key)?.as_deref() != Some(raw.as_str()) {
            return Err("Account changed during authorization".into());
        }
        store.set(&key, &c.value().to_string())?;
    }
    Ok(c)
}
pub fn git_account_credentials(
    account: &Account,
    target: &str,
    cancel: &Cancel,
) -> Result<Credentials, String> {
    let c = credentials(account, cancel)?;
    if failures::Attempt::new(target, &account.id(), &c.access).rejected() {
        return Err("Account needs to log in again".into());
    }
    Ok(c)
}
/// Only an explicit repository binding or a unique matching account may supply Git.
pub fn matching_accounts(target: &str, known: Option<&str>) -> Result<Vec<Account>, String> {
    let u = url::Url::parse(target).map_err(|_| "Invalid repository URL")?;
    let entries = accounts()?;
    Ok(entries
        .into_iter()
        .filter(|a| account_matches(a, &u, known))
        .collect())
}
pub fn account_matches(account: &Account, target: &url::Url, known: Option<&str>) -> bool {
    let Ok(server) = url::Url::parse(&account.server) else {
        return false;
    };
    target.origin() == server.origin()
        && (server.path() == "/"
            || target
                .path()
                .starts_with(&format!("{}/", server.path().trim_end_matches('/'))))
        && known.is_none_or(|login| {
            login == account.login || (account.provider == Provider::GitLab && login == "oauth2")
        })
}

fn default_key(provider: Provider, project: &std::path::Path) -> String {
    format!(
        "hosting:default:{}:{:x}",
        provider.name(),
        Sha256::digest(project.to_string_lossy().as_bytes())
    )
}
pub fn default_account(
    provider: Provider,
    project: &std::path::Path,
) -> Result<Option<String>, String> {
    Ok(Store::system()
        .get(&default_key(provider, project))?
        .filter(|s| !s.is_empty()))
}
pub fn set_default(
    provider: Provider,
    project: &std::path::Path,
    account: Option<&Account>,
) -> Result<(), String> {
    Store::system().set(
        &default_key(provider, project),
        &account.map(Account::id).unwrap_or_default(),
    )
}
pub(crate) fn git_credentials(
    target: &str,
    known: Option<&str>,
    cancel: &Cancel,
    project: Option<&std::path::Path>,
) -> Option<(String, String, failures::Attempt)> {
    let binding = Store::system()
        .get(&format!(
            "hosting:binding:{:x}",
            Sha256::digest(target.as_bytes())
        ))
        .ok()
        .flatten();
    let mut matches = matching_accounts(target, known).ok()?;
    // Upstream excludes rejected accounts before deciding whether the remaining
    // account is unique. Inspect stored credentials only; do not refresh every
    // matching account merely to choose one.
    matches.retain(|account| {
        let cached = Store::system()
            .get(&format!("hosting:token:{}", account.id()))
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .and_then(|value| Credentials::parse(value).ok());
        cached.is_none_or(|c| {
            c.expires_at
                .is_some_and(|expires| expires <= oauth::now() + 60)
                || !failures::Attempt::new(target, &account.id(), &c.access).rejected()
        })
    });
    let default = project.and_then(|path| {
        matches.iter().find_map(|a| {
            default_account(a.provider, path)
                .ok()
                .flatten()
                .filter(|id| *id == a.id())
        })
    });
    let preferred = binding
        .as_deref()
        .filter(|id| matches.iter().any(|a| a.id() == *id))
        .or(default.as_deref());
    let a = select_account(&matches, preferred)?;
    let c = credentials(a, cancel).ok()?;
    let attempt = failures::Attempt::new(target, &a.id(), &c.access);
    if attempt.rejected() {
        return None;
    }
    Some((a.login.clone(), c.access, attempt))
}
pub fn bind(account: &Account, target: &str) -> Result<(), String> {
    let mut u = url::Url::parse(target).map_err(|_| "Invalid repository URL")?;
    u.set_username("").map_err(|_| "Invalid repository URL")?;
    let _ = u.set_password(None);
    u.set_query(None);
    u.set_fragment(None);
    Store::system().set(
        &format!(
            "hosting:binding:{:x}",
            Sha256::digest(u.as_str().as_bytes())
        ),
        &account.id(),
    )
}

fn select_account<'a>(accounts: &'a [Account], binding: Option<&str>) -> Option<&'a Account> {
    binding
        .and_then(|id| accounts.iter().find(|a| a.id() == id))
        .or_else(|| {
            if accounts.len() == 1 {
                accounts.first()
            } else {
                None
            }
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosted_credentials_match_origin_namespace_and_explicit_login() {
        let account = Account {
            provider: Provider::GitLab,
            server: "https://git.example/team".into(),
            login: "alice".into(),
        };
        let good = url::Url::parse("https://git.example/team/repo.git").unwrap();
        assert!(account_matches(&account, &good, None));
        assert!(account_matches(&account, &good, Some("oauth2")));
        assert!(!account_matches(&account, &good, Some("bob")));
        for target in [
            "http://git.example/team/repo.git",
            "https://git.example:8443/team/repo.git",
            "https://git.example.attacker/team/repo.git",
            "https://git.example/team-other/repo.git",
        ] {
            assert!(!account_matches(
                &account,
                &url::Url::parse(target).unwrap(),
                None
            ));
        }
    }
    #[test]
    fn stale_binding_falls_back_only_to_unambiguous_account() {
        let alice = Account {
            provider: Provider::GitHub,
            server: Provider::GitHub.server().into(),
            login: "alice".into(),
        };
        let bob = Account {
            login: "bob".into(),
            ..alice.clone()
        };
        assert_eq!(
            select_account(std::slice::from_ref(&alice), Some("removed")),
            Some(&alice)
        );
        assert!(select_account(&[alice.clone(), bob.clone()], Some("removed")).is_none());
        assert_eq!(
            select_account(&[alice.clone(), bob.clone()], Some(&bob.id())),
            Some(&bob)
        );
    }
}

#[cfg(test)]
mod storage_tests;
