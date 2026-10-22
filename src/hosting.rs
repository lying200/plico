//! Hosted Git accounts. Rebased 4aec352 GitLabAccountManager/GH clone models,
//! Apache-2.0, JetBrains and contributors. No collaboration/PR API integration.
mod api;
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
    let _g = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    let store = Store::system();
    let mut entries = load(&store)?;
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
    store.set("hosting:accounts", &index)
}
pub fn remove(account: &Account) -> Result<(), String> {
    let _g = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
    let store = Store::system();
    let mut entries = load(&store)?;
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
    // Serialize token rotation without holding the UI account-list lock during HTTP.
    let _refresh = REFRESH.lock().unwrap_or_else(|e| e.into_inner());
    if cancel.cancelled() {
        return Err("Cancelled".into());
    }
    let store = Store::system();
    let key = format!("hosting:token:{}", account.id());
    let raw = store.get(&key)?.ok_or("Account needs to log in again")?;
    let mut c =
        Credentials::parse(serde_json::from_str(&raw).map_err(|_| "Invalid account credentials")?)?;
    if c.expires_at.is_some_and(|e| e <= oauth::now() + 60) {
        c = oauth::refresh(account, &c, cancel)?;
        if cancel.cancelled() {
            return Err("Cancelled".into());
        }
        let _g = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
        // A logout or new login during the request must win over this response.
        if store.get(&key)?.as_deref() != Some(raw.as_str()) {
            return Err("Account changed during authorization".into());
        }
        store.set(&key, &c.value().to_string())?;
    }
    Ok(c)
}
/// Only an explicit repository binding or a unique matching account may supply Git.
pub(crate) fn git_credentials(
    target: &str,
    known: Option<&str>,
    cancel: &Cancel,
) -> Option<(String, String)> {
    let u = url::Url::parse(target).ok()?;
    let entries = accounts().ok()?;
    let binding = Store::system()
        .get(&format!(
            "hosting:binding:{:x}",
            Sha256::digest(target.as_bytes())
        ))
        .ok()
        .flatten();
    let matches: Vec<_> = entries
        .into_iter()
        .filter(|a| {
            let Ok(s) = url::Url::parse(&a.server) else {
                return false;
            };
            u.origin() == s.origin()
                && (s.path() == "/"
                    || u.path()
                        .starts_with(&format!("{}/", s.path().trim_end_matches('/'))))
                && known.is_none_or(|k| {
                    k == a.login || (a.provider == Provider::GitLab && k == "oauth2")
                })
        })
        .collect();
    let a = if let Some(id) = binding {
        matches.iter().find(|a| a.id() == id)?
    } else {
        if matches.len() != 1 {
            return None;
        }
        &matches[0]
    };
    let c = credentials(a, cancel).ok()?;
    Some((a.login.clone(), c.access))
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
