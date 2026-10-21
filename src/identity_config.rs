//! Rebased 4aec352 GitUserNameCheckinHandler/GitUserNameNotDefinedDialog
//! and GitConfigUtil (JetBrains and contributors, Apache-2.0).
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
};

#[derive(Clone, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub email: String,
}
impl Identity {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        let value = |key| -> Result<String, String> {
            let text = repo.text(&["config", "--null", "--default=", "--get", key], token)?;
            text.strip_suffix('\0')
                .map(str::to_owned)
                .ok_or_else(|| "Invalid identity configuration".into())
        };
        Ok(Self {
            name: value("user.name")?,
            email: value("user.email")?,
        })
    }
    pub fn missing(&self) -> bool {
        self.name.trim().is_empty() || self.email.trim().is_empty()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.missing()
            || !self.email.contains('@')
            || self
                .name
                .chars()
                .chain(self.email.chars())
                .any(char::is_control)
        {
            return Err("Enter a name and an email address containing @".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub enum Scope {
    Repository,
    Global,
}

pub struct Report {
    pub commands: Vec<Outcome>,
    pub effective: Result<Identity, String>,
}
impl Report {
    pub fn succeeded(&self) -> bool {
        self.commands.len() == 2
            && self
                .commands
                .iter()
                .all(|o| o.status.success() && o.stopped.is_none() && o.input_error.is_none())
            && self.effective.as_ref().is_ok_and(|i| !i.missing())
    }
}
/// Explicit configuration only. The caller resumes the original commit request
/// after success; no commit is created by this function. Global is opt-in.
pub fn set(
    repo: &Repository,
    identity: &Identity,
    scope: Scope,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    identity.validate()?;
    let _guard = repo.operation_guard(token)?;
    let mut commands = Vec::new();
    for (key, value) in [
        ("user.name", &identity.name),
        ("user.email", &identity.email),
    ] {
        let outcome = operation_process::run(
            repo.operation_command().args([
                "config",
                match scope {
                    Scope::Repository => "--local",
                    Scope::Global => "--global",
                },
                key,
                value,
            ]),
            token,
            progress,
            None,
        )
        .map_err(|e| e.to_string())?;
        let ok =
            outcome.status.success() && outcome.stopped.is_none() && outcome.input_error.is_none();
        commands.push(outcome);
        if !ok {
            break;
        }
    }
    Ok(Report {
        commands,
        effective: Identity::read(repo, &Cancel::default()),
    })
}
