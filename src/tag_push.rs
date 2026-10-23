//! Single-tag push: Rebased GitPushTagAction / GitPushOperation, 4aec352.
use crate::{
    git::{Cancel, Repository},
    operation_process::Progress,
    push::{ResultState, Status},
    remote_management::{self, Remote},
};
use std::path::Path;

pub struct Prepared {
    reference: String,
    oid: String,
    remote: Remote,
}
impl Prepared {
    pub fn read(
        repo: &Repository,
        reference: &str,
        remote: &str,
        token: &Cancel,
    ) -> Result<Self, String> {
        if !reference.starts_with("refs/tags/") {
            return Err("Select a tag to push".into());
        }
        repo.text(&["check-ref-format", reference], token)?;
        let oid = repo
            .text(
                &["rev-parse", "--verify", "--end-of-options", reference],
                token,
            )?
            .trim()
            .to_owned();
        if !crate::git::is_oid(&oid) {
            return Err("Invalid tag object".into());
        }
        let remote = remote_management::read(repo, token)?
            .into_iter()
            .find(|r| r.name == remote)
            .ok_or("Push remote is unavailable")?;
        Ok(Self {
            reference: reference.into(),
            oid,
            remote,
        })
    }
    fn validate(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let current = Self::read(repo, &self.reference, &self.remote.name, token)?;
        if current.oid != self.oid || current.remote != self.remote {
            return Err("Tag or remote changed; review the push again".into());
        }
        Ok(())
    }
}
/// Push only the selected immutable tag object (including annotated metadata).
/// No branch, implicit follow-tags, force replacement or upstream configuration.
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    token: &Cancel,
    progress: &Progress,
    auth: Option<(&crate::auth_request::Requests, &Path)>,
) -> Result<ResultState, String> {
    let _guard = repo.operation_guard(token)?;
    prepared.validate(repo, token)?;
    let refspec = format!("{}:{}", prepared.oid, prepared.reference);
    let mut command = repo.operation_command();
    command.args([
        "push",
        "--progress",
        "--porcelain",
        "--no-follow-tags",
        "--no-mirror",
        "--",
        &prepared.remote.name,
        &refspec,
    ]);
    let result =
        crate::authenticated_command::run(&mut command, auth, token, progress, None, |outcome| {
            crate::authenticated_command::push_can_retry(outcome)
                && prepared.validate(repo, token).is_ok()
        });
    let status = match &result {
        Ok(outcome) => crate::push::classify(outcome, &prepared.oid, &prepared.reference, false),
        Err(_) if token.cancelled() => Status::Cancelled,
        Err(_) => Status::Failed,
    };
    Ok(ResultState {
        command: result,
        status,
        state: repo.operation_state(&Cancel::default()),
    })
}
