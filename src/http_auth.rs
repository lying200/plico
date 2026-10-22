//! Generic HTTPS provider lifecycle translated from GitHttpGuiAuthenticator.
//! No hosting API scope is required: PATs are Git HTTP passwords.
use crate::credential_store::Store;
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Full,
    Silent,
    None,
}
struct Used {
    key: String,
    secret: String,
}
struct State {
    requested: bool,
    hosted_used: Option<String>,
    hosted_rejected: HashSet<String>,
    helper: bool,
    switched: bool,
    helper_available: bool,
    used: Vec<Used>,
    logins: HashMap<String, String>,
    storage_error: bool,
    rejected: HashSet<String>,
}
pub(crate) struct Auth {
    pub mode: Mode,
    store: Store,
    state: Mutex<State>,
}
fn login_key(target: &str) -> String {
    format!("login:{target}")
}
fn password_key(target: &str, login: &str) -> String {
    format!("password:{}:{target}{login}", target.len())
}
pub(crate) fn username(url: &url::Url) -> String {
    let bytes = url.username().as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(value) = u8::from_str_radix(&url.username()[i + 1..i + 3], 16)
        {
            out.push(value);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}
/// Application-wide choice, matching GitVcsApplicationSettings.
pub fn use_credential_helper() -> bool {
    Store::system().get("use-helper").ok().flatten().as_deref() == Some("true")
}
pub fn set_use_credential_helper(enabled: bool) -> Result<(), String> {
    Store::system().set("use-helper", if enabled { "true" } else { "false" })
}
impl Auth {
    pub fn new(mode: Mode, store: Store) -> Self {
        let helper = store.get("use-helper").ok().flatten().as_deref() == Some("true");
        Self {
            mode,
            store,
            state: Mutex::new(State {
                requested: false,
                hosted_used: None,
                hosted_rejected: HashSet::new(),
                helper,
                switched: false,
                helper_available: false,
                used: Vec::new(),
                logins: HashMap::new(),
                storage_error: false,
                rejected: HashSet::new(),
            }),
        }
    }
    pub fn begin(&self, helper_available: bool) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.requested = false;
        s.switched = false;
        s.used.clear();
        s.hosted_used = None;
        s.helper_available = helper_available;
    }
    pub fn helper(&self) -> bool {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).helper && self.mode != Mode::None
    }
    pub fn helper_available(&self) -> bool {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.helper_available && !s.helper
    }
    pub fn switched(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .switched
    }
    pub fn switch_helper(&self) -> Result<(), String> {
        let saved = self.store.set("use-helper", "true").is_ok();
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        // A broken app vault must not block a working external helper. Its
        // preference is session-local if persistence is unavailable.
        s.storage_error |= !saved;
        s.helper = true;
        s.switched = true;
        Ok(())
    }
    pub fn remember_default(&self) -> bool {
        self.store.get("remember-default").ok().flatten().as_deref() != Some("false")
    }
    pub fn requested(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .requested
    }
    pub fn mark_requested(&self) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .requested = true;
    }
    pub fn suggested_login(&self, target: &str) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .logins
            .get(target)
            .cloned()
            .or_else(|| self.store.get(&login_key(target)).ok().flatten())
    }
    pub fn lookup(&self, target: &str, known: Option<&str>) -> Option<(String, String)> {
        // The latest provider acquisition owns failure cleanup. A later manual
        // or helper login must not invalidate an earlier endpoint's credential.
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .used
            .clear();
        if self.mode == Mode::None || self.switched() {
            return None;
        }
        let login = known
            .map(str::to_owned)
            .or_else(|| self.suggested_login(target))?;
        let key = password_key(target, &login);
        if self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rejected
            .contains(&key)
        {
            return None;
        }
        let secret = match self.store.get(&key) {
            Ok(Some(secret)) => secret,
            Ok(None) => return None,
            Err(_) => {
                self.state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .storage_error = true;
                return None;
            }
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.used.clear();
        state.used.push(Used {
            key,
            secret: secret.clone(),
        });
        Some((login, secret))
    }
    pub fn remember(
        &self,
        target: &str,
        login: &str,
        secret: &str,
        remember: bool,
    ) -> Result<(), String> {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.used.clear();
        // A failed preference write must not prevent an explicitly unsaved login.
        let _ = self
            .store
            .set("remember-default", if remember { "true" } else { "false" });
        s.logins.insert(target.into(), login.into());
        if s.helper {
            // Explicitly choosing manual credentials restores the app provider.
            if self.store.set("use-helper", "false").is_ok() {
                s.helper = false;
            }
        }
        // Remembered login and password have distinct lifetimes, as upstream.
        if let Err(error) = self.store.set(&login_key(target), login) {
            s.storage_error = true;
            if remember {
                return Err(error);
            }
        }
        if remember {
            let key = password_key(target, login);
            self.store.set(&key, secret)?;
            s.rejected.remove(&key);
            s.used.clear();
            s.used.push(Used {
                key,
                secret: secret.into(),
            });
        }
        Ok(())
    }
    pub fn lookup_hosted(
        &self,
        target: &str,
        known: Option<&str>,
        cancel: &crate::git::Cancel,
    ) -> Option<(String, String)> {
        if !self.store.is_system() || self.mode == Mode::None || self.helper() || self.switched() {
            return None;
        }
        {
            let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if s.hosted_rejected.contains(target) {
                return None;
            }
        }
        let credentials = crate::hosting::git_credentials(target, known, cancel)?;
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .hosted_used = Some(target.into());
        Some(credentials)
    }
    pub fn failed(&self) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(target) = s.hosted_used.take() {
            s.hosted_rejected.insert(target);
        }
        for used in std::mem::take(&mut s.used) {
            s.rejected.insert(used.key.clone());
            if self.store.remove_if(&used.key, &used.secret).is_err() {
                s.storage_error = true;
            }
        }
    }
    pub fn storage_error(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .storage_error
    }
}
/// Authentication manager only invokes this after an HTTP AskPass request.
/// Ordinary network / non-fast-forward / branch protection failures are excluded.
pub fn authentication_failed(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    text.lines().any(|line| {
        line.contains("authentication failed")
            || line.contains("403 forbidden")
            || line.contains("but was used to access one of realms")
            || line.contains("error: 400")
            || (line.contains("fatal: repository ") && line.contains("not found"))
            || (line.contains("fatal: unable to access ")
                && line.contains("requested url returned error: 403"))
            || line.contains("[remote rejected] (permission denied)")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helper_can_be_used_when_app_vault_is_unavailable() {
        let auth = Auth::new(Mode::Full, Store::unavailable());
        auth.begin(true);
        auth.switch_helper().unwrap();
        assert!(auth.helper());
        assert!(auth.switched());
        assert!(auth.storage_error());
    }
    #[test]
    fn identities_and_known_accounts_do_not_cross_boundaries() {
        let store = Store::default();
        let first = Auth::new(Mode::Full, store.clone());
        first
            .remember("https://host:8443/a.git", "alice", "alice-token", true)
            .unwrap();
        first
            .remember("https://host:8443/a.git", "bob", "bob-token", true)
            .unwrap();
        let later = Auth::new(Mode::Silent, store);
        assert_eq!(
            later
                .lookup("https://host:8443/a.git", Some("alice"))
                .unwrap()
                .1,
            "alice-token"
        );
        assert_eq!(
            later.lookup("https://host:8443/a.git", None).unwrap().0,
            "bob"
        );
        for target in [
            "http://host:8443/a.git",
            "https://host/a.git",
            "https://host:8443/b.git",
        ] {
            assert!(later.lookup(target, Some("alice")).is_none());
        }
        assert!(
            later
                .lookup("https://host:8443/a.git", Some("carol"))
                .is_none()
        );
    }
    #[test]
    fn failure_removes_only_used_value_and_preserves_newer_credentials() {
        let store = Store::default();
        let target = "https://host/a.git";
        let first = Auth::new(Mode::Full, store.clone());
        first.remember(target, "alice", "old", true).unwrap();
        let other = Auth::new(Mode::Full, store.clone());
        other.remember(target, "alice", "new", true).unwrap();
        first.failed();
        assert!(first.lookup(target, Some("alice")).is_none());
        assert_eq!(
            Auth::new(Mode::Full, store)
                .lookup(target, Some("alice"))
                .unwrap()
                .1,
            "new"
        );
    }
    #[test]
    fn remembered_login_without_password_and_none_mode() {
        let store = Store::default();
        let auth = Auth::new(Mode::Full, store.clone());
        auth.remember("https://host/repo", "alice", "secret", false)
            .unwrap();
        let later = Auth::new(Mode::Full, store.clone());
        assert!(!later.remember_default());
        assert_eq!(
            later.suggested_login("https://host/repo").as_deref(),
            Some("alice")
        );
        assert!(later.lookup("https://host/repo", None).is_none());
        auth.remember("https://host/repo", "alice", "secret", true)
            .unwrap();
        assert!(
            Auth::new(Mode::None, store)
                .lookup("https://host/repo", None)
                .is_none()
        );
    }
    #[test]
    fn failure_classifier_excludes_transport_and_git_policy_errors() {
        for text in [
            "fatal: Authentication failed for 'https://host/repo'",
            "fatal: repository 'https://host/repo' not found",
            "fatal: unable to access 'https://host/repo': The requested URL returned error: 403",
        ] {
            assert!(authentication_failed(text.as_bytes()));
        }
        for text in [
            "Could not resolve host",
            "Connection reset by peer",
            "[rejected] main -> main (non-fast-forward)",
            "remote: Protected branch update failed",
            "SSL certificate problem",
            "remote: HTTP 500",
        ] {
            assert!(!authentication_failed(text.as_bytes()));
        }
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn later_unremembered_failure_does_not_clear_an_earlier_provider() {
        let store = Store::default();
        let auth = Auth::new(Mode::Full, store.clone());
        auth.remember("https://host/first", "alice", "valid", true)
            .unwrap();
        auth.lookup("https://host/first", Some("alice")).unwrap();
        auth.remember("https://host/second", "bob", "invalid", false)
            .unwrap();
        auth.failed();
        assert_eq!(
            Auth::new(Mode::Full, store)
                .lookup("https://host/first", Some("alice"))
                .unwrap()
                .1,
            "valid"
        );
    }
}
