//! Log tag creation: Rebased 4aec352 GitCreateTagAction / GitBranchWorker.
use crate::{
    git::{Cancel, Repository, is_oid},
    operation_process::{self, Outcome, Progress},
};
use std::path::PathBuf;

pub struct Prepared {
    name: String,
    target: String,
    directory: PathBuf,
    source: String,
    previous: Option<String>,
    message: String,
}
impl Prepared {
    pub fn read(
        repo: &Repository,
        name: &str,
        target: &str,
        token: &Cancel,
    ) -> Result<Self, String> {
        if !is_oid(target) {
            return Err("Invalid tag target".into());
        }
        Self::read_options(repo, name, target, "", false, token)
    }
    pub fn read_options(
        repo: &Repository,
        name: &str,
        source: &str,
        message: &str,
        force: bool,
        token: &Cancel,
    ) -> Result<Self, String> {
        if name.is_empty() || name.starts_with('-') || name.chars().any(char::is_whitespace) {
            return Err("Invalid tag name".into());
        }
        repo.text(&["check-ref-format", &format!("refs/tags/{name}")], token)?;
        let source = if source.trim().is_empty() {
            "HEAD"
        } else {
            source.trim()
        };
        let target = resolve(repo, source, token)?;
        let previous = existing(repo, name, token)?;
        if previous.is_some() && !force {
            return Err("A tag with this name already exists".into());
        }
        Ok(Self {
            name: name.into(),
            target,
            directory: repo.editor_directory(token)?,
            source: source.into(),
            previous,
            message: message.into(),
        })
    }
    fn validate(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.directory {
            return Err("Repository changed".into());
        }
        if existing(repo, &self.name, token)? != self.previous {
            return Err("Tag changed; review the operation again".into());
        }
        if resolve(repo, &self.source, token)? != self.target {
            return Err("Tag target changed; review the operation again".into());
        }
        let target = repo.text(
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{}^{{commit}}", self.target),
            ],
            token,
        )?;
        if target.trim() != self.target {
            return Err("Tag target is not a commit".into());
        }
        Ok(())
    }
}
/// Force is explicit and revalidated against the prior tag object. Git tag -f
/// retains upstream semantics; external writers are not covered by our operation lock.
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    token: &Cancel,
    progress: &Progress,
) -> Result<Outcome, String> {
    let _guard = repo.operation_guard(token)?;
    prepared.validate(repo, token)?;
    let mut command = repo.operation_command();
    command.arg("tag");
    if prepared.previous.is_some() {
        command.arg("-f");
    }
    let annotated = !prepared.message.trim().is_empty();
    if annotated {
        command.args(["-a", "-F", "-"]);
    }
    command.args(["--", &prepared.name, &prepared.target]);
    if annotated {
        operation_process::run_with_input(
            &mut command,
            token,
            progress,
            None,
            prepared.message.as_bytes(),
        )
    } else {
        operation_process::run(&mut command, token, progress, None)
    }
    .map_err(|e| e.to_string())
}

/// Resolve the optional commit field without letting Git interpret it as an option.
pub fn resolve(repo: &Repository, source: &str, token: &Cancel) -> Result<String, String> {
    let source = if source.trim().is_empty() {
        "HEAD"
    } else {
        source.trim()
    };
    let oid = repo
        .text(
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{source}^{{commit}}"),
            ],
            token,
        )?
        .trim()
        .to_owned();
    if !is_oid(&oid) {
        return Err("Invalid commit".into());
    }
    Ok(oid)
}
fn existing(repo: &Repository, name: &str, token: &Cancel) -> Result<Option<String>, String> {
    let reference = format!("refs/tags/{name}");
    let output = repo.text(
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            &reference,
        ],
        token,
    )?;
    Ok(output.lines().find_map(|line| {
        line.split_once(' ')
            .filter(|(r, _)| *r == reference)
            .map(|(_, oid)| oid.to_owned())
    }))
}

/// Metadata-only validation preview. Blob contents remain on-demand in the viewer.
pub struct Preview {
    pub id: String,
    pub message: String,
    pub files: Vec<crate::combined_changes::Change>,
}
pub fn preview(repo: &Repository, source: &str, token: &Cancel) -> Result<Preview, String> {
    let id = resolve(repo, source, token)?;
    let snapshot =
        repo.snapshot_commits(std::slice::from_ref(&id), std::slice::from_ref(&id), token)?;
    let commit = snapshot
        .commits
        .into_iter()
        .find(|c| c.id == id)
        .ok_or("Commit is unavailable")?;
    let details = repo.commit_metadata(&commit, token)?;
    let files = repo.log_commit_changes(&commit, token)?;
    Ok(Preview {
        id,
        message: details.message,
        files,
    })
}
