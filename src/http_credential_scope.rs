//! Generic password scope is distinct from the full hosted repository identity.
use crate::{
    authenticated_command::config_command,
    git::Cancel,
    operation_process::{self, Progress},
};
use std::{collections::HashMap, process::Command, time::Duration};

// Deliberately no Debug: inherited Git configuration can contain credentials.
pub(crate) struct Scope {
    original: Command,
    cache: HashMap<String, bool>,
}
impl Scope {
    pub fn new(command: &Command) -> Option<Self> {
        Some(Self {
            original: config_command(command)?,
            cache: HashMap::new(),
        })
    }
    // Called only by the AskPass worker, before publishing any UI request.
    pub fn target(&mut self, mut url: url::Url, cancel: &Cancel) -> String {
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        let key = url.as_str().to_owned();
        let use_path = self.cache.get(&key).copied().unwrap_or_else(|| {
            let value = self.use_path(&key, cancel);
            if self.cache.len() >= 64 {
                self.cache.clear();
            }
            self.cache.insert(key, value);
            value
        });
        let _ = url.set_username("");
        if !use_path {
            url.set_path("/");
        }
        url.into()
    }
    fn use_path(&self, url: &str, cancel: &Cancel) -> bool {
        let Some(mut query) = config_command(&self.original) else {
            return true;
        };
        query.args([
            "config",
            "--type=bool",
            "--get-urlmatch",
            "credential.useHttpPath",
            url,
        ]);
        let Ok(result) = operation_process::run(
            &mut query,
            cancel,
            &Progress::default(),
            Some(Duration::from_secs(5)),
        ) else {
            return true;
        };
        if result.stopped.is_some() || result.stdout.truncated || result.stderr.truncated {
            return true;
        }
        if result.status.success() {
            // Invalid/unknown output must not broaden a credential's scope.
            result.stdout.bytes().as_slice() != b"false\n"
        } else {
            // Git exits 1 without output when there is no matching setting.
            !(result.status.code() == Some(1)
                && result.stdout.bytes().is_empty()
                && result.stderr.bytes().is_empty())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command() -> Command {
        let mut command = Command::new("git");
        command
            .current_dir(std::env::temp_dir())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env("GIT_CONFIG_COUNT", "0");
        command
    }
    #[test]
    fn absent_setting_uses_origin_but_keeps_protocol_and_port() {
        let mut scope = Scope::new(&command()).unwrap();
        let cancel = Cancel::default();
        for url in [
            "https://user@host:8443/gitlab/repo.git",
            "http://host:8443/other.git",
        ] {
            let parsed = url::Url::parse(url).unwrap();
            let expected = format!("{}/", parsed.origin().ascii_serialization());
            assert_eq!(scope.target(parsed, &cancel), expected);
        }
    }
    #[test]
    fn command_environment_and_username_specific_config_are_preserved() {
        let mut command = command();
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "credential.useHttpPath")
            .env("GIT_CONFIG_VALUE_0", "true")
            .args([
                "-c",
                "credential.https://alice@host.useHttpPath=false",
                "fetch",
                "origin",
            ]);
        let mut scope = Scope::new(&command).unwrap();
        let cancel = Cancel::default();
        assert_eq!(
            scope.target(
                url::Url::parse("https://alice@host/repo.git").unwrap(),
                &cancel
            ),
            "https://host/"
        );
        assert_eq!(
            scope.target(
                url::Url::parse("https://bob@host/repo.git").unwrap(),
                &cancel
            ),
            "https://host/repo.git"
        );
    }
    #[test]
    fn invalid_setting_never_broadens_scope() {
        let mut command = command();
        command.args([
            "-c",
            "credential.useHttpPath=invalid",
            "clone",
            "https://host/repo.git",
        ]);
        let mut scope = Scope::new(&command).unwrap();
        assert_eq!(
            scope.target(
                url::Url::parse("https://host/repo.git").unwrap(),
                &Cancel::default()
            ),
            "https://host/repo.git"
        );
    }
}
