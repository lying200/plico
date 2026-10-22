//! NativeSshGuiAuthenticator / SshPrompts (Rebased 4aec352, Apache-2.0).
//! OpenSSH owns key selection, agent access and known_hosts; we only answer prompts.
use crate::{credential_store::Store, http_auth::Mode};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Mutex};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prompt {
    Passphrase(String),
    Password(String),
    Pin(String),
    Confirmation(String),
    Generic(String),
}
impl Prompt {
    pub fn parse(text: &str) -> Self {
        let value = text.trim();
        for prefix in [
            "Enter passphrase for key ",
            "Enter passphrase for ",
            "Enter PIN for ",
        ] {
            if let Some(value) = value.strip_prefix(prefix).and_then(|v| v.strip_suffix(':')) {
                let identity = value.trim_matches('\'').to_owned();
                return if prefix.contains("PIN") {
                    Self::Pin(identity)
                } else {
                    Self::Passphrase(identity)
                };
            }
        }
        if let Some(user) = value.strip_suffix("'s password:") {
            return Self::Password(user.into());
        }
        if value.contains("Are you sure you want to continue connecting") {
            return Self::Confirmation(value.into());
        }
        Self::Generic(value.into())
    }
    pub fn title(&self) -> &'static str {
        match self {
            Self::Passphrase(_) => "SSH Key Passphrase",
            Self::Password(_) => "SSH Password",
            Self::Pin(_) => "SSH PIN",
            Self::Confirmation(_) => "SSH Confirmation",
            Self::Generic(_) => "SSH Authentication",
        }
    }
    pub fn message(&self) -> String {
        match self {
            Self::Passphrase(key) => format!("{} {key}", crate::i18n::t("Password for SSH key:")),
            Self::Password(user) => format!("{} {user}", crate::i18n::t("SSH password for:")),
            Self::Pin(label) => format!("PIN: {label}"),
            Self::Confirmation(v) => v
                .replace(
                    "Are you sure you want to continue connecting (yes/no/[fingerprint])?",
                    "Are you sure you want to continue connecting?",
                )
                .replace(
                    "Are you sure you want to continue connecting (yes/no)?",
                    "Are you sure you want to continue connecting?",
                ),
            Self::Generic(v) => v.clone(),
        }
    }
    pub fn confirmation(&self) -> bool {
        matches!(self, Self::Confirmation(_))
    }
    pub fn rememberable(&self) -> bool {
        self.key().is_some()
    }
    fn key(&self) -> Option<String> {
        let (kind, identity) = match self {
            Self::Passphrase(v) => ("key", v),
            Self::Password(v) => ("password", v),
            Self::Pin(v) => ("pin", v),
            _ => return None,
        };
        Some(format!(
            "ssh:{kind}:{:x}",
            Sha256::digest(identity.as_bytes())
        ))
    }
}
pub(crate) struct Auth {
    store: Store,
    used: Mutex<HashMap<String, Option<String>>>,
}
impl Auth {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            used: Mutex::new(HashMap::new()),
        }
    }
    pub fn lookup(&self, prompt: &Prompt, mode: Mode) -> Option<String> {
        if mode == Mode::None {
            return None;
        }
        let key = prompt.key()?;
        let mut used = self.used.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = used.get(&key) {
            if let Some(previous) = previous {
                let _ = self.store.remove_if(&key, previous);
            }
            return None;
        }
        let value = self.store.get(&key).ok().flatten();
        used.insert(key, value.clone());
        value
    }
    pub fn remember(&self, prompt: &Prompt, value: &str, save: bool) -> Result<(), String> {
        let Some(key) = prompt.key() else {
            return Ok(());
        };
        if save {
            self.store.set(&key, value)?;
        }
        self.used
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, save.then(|| value.to_owned()));
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_types_do_not_share_secrets_or_remember_host_confirmation() {
        assert_eq!(
            Prompt::parse("\rEnter passphrase for key 'C:/key': "),
            Prompt::Passphrase("C:/key".into())
        );
        assert_eq!(
            Prompt::parse("user@host's password: "),
            Prompt::Password("user@host".into())
        );
        assert_eq!(
            Prompt::parse("Enter PIN for 'token':"),
            Prompt::Pin("token".into())
        );
        assert!(
            Prompt::parse("Are you sure you want to continue connecting (yes/no)?").confirmation()
        );
        assert!(!Prompt::parse("Verification code:").rememberable());
    }
    #[test]
    fn repeated_prompt_rejects_old_secret_but_preserves_concurrent_update() {
        let store = Store::default();
        let prompt = Prompt::Passphrase("fixture".into());
        let auth = Auth::new(store.clone());
        auth.remember(&prompt, "old", true).unwrap();
        let attempt = Auth::new(store.clone());
        assert_eq!(
            attempt.lookup(&prompt, Mode::Silent).as_deref(),
            Some("old")
        );
        auth.remember(&prompt, "new", true).unwrap();
        assert!(attempt.lookup(&prompt, Mode::Full).is_none());
        assert_eq!(
            Auth::new(store).lookup(&prompt, Mode::Full).as_deref(),
            Some("new")
        );
        assert!(auth.lookup(&prompt, Mode::None).is_none());
    }
}
