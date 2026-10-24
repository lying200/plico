//! Rebased 4aec352 workingTrees services/dialog and GitImpl (Apache-2.0).
//! Removal requires UI confirmation and follows upstream worktree remove --force.
use crate::{
    git::{Cancel, Repository, path_from_bytes},
    operation_process::{self, Outcome, Progress},
};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub main: bool,
    pub current: bool,
    pub bare: bool,
    pub locked: bool,
    pub prunable: bool,
}

pub fn list(repo: &Repository, token: &Cancel) -> Result<Vec<Tree>, String> {
    let raw = repo.read(
        &[
            OsStr::new("worktree"),
            OsStr::new("list"),
            OsStr::new("--porcelain"),
            OsStr::new("-z"),
        ],
        token,
    )?;
    let bare = repo
        .text(&["rev-parse", "--is-bare-repository"], token)?
        .trim()
        == "true";
    let directory = if bare {
        repo.editor_directory(token)?
    } else {
        let root = repo.read(
            &[OsStr::new("rev-parse"), OsStr::new("--show-toplevel")],
            token,
        )?;
        let root = root.strip_suffix(b"\n").unwrap_or(&root);
        path_from_bytes(root.strip_suffix(b"\r").unwrap_or(root))?
    }
    .canonicalize()
    .map_err(|e| e.to_string())?;
    let mut trees = Vec::new();
    let mut tree: Option<Tree> = None;
    for field in raw.split(|b| *b == 0) {
        if field.is_empty() {
            if let Some(mut t) = tree.take() {
                t.current = t.path.canonicalize().is_ok_and(|p| p == directory);
                trees.push(t);
            }
        } else if let Some(path) = field.strip_prefix(b"worktree ") {
            if tree.is_some() {
                return Err("Invalid worktree list record".into());
            }
            tree = Some(Tree {
                path: path_from_bytes(path)?,
                head: String::new(),
                branch: None,
                main: trees.is_empty(),
                current: false,
                bare: false,
                locked: false,
                prunable: false,
            });
        } else if let Some(t) = tree.as_mut() {
            if let Some(head) = field.strip_prefix(b"HEAD ") {
                t.head = String::from_utf8(head.to_vec()).map_err(|e| e.to_string())?;
            } else if let Some(branch) = field.strip_prefix(b"branch ") {
                t.branch = Some(String::from_utf8(branch.to_vec()).map_err(|e| e.to_string())?);
            } else if field == b"bare" {
                t.bare = true;
            } else if field == b"locked" || field.starts_with(b"locked ") {
                t.locked = true;
            } else if field == b"prunable" || field.starts_with(b"prunable ") {
                t.prunable = true;
            }
        }
    }
    if tree.is_some() {
        return Err("Incomplete worktree list".into());
    }
    Ok(trees)
}

fn resolve(repo: &Repository, reference: &str, token: &Cancel) -> Result<String, String> {
    repo.text(
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ],
        token,
    )
    .map(|s| s.trim().to_owned())
}
fn destination(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Worktree location must be absolute".into());
    }
    match std::fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err("Worktree directory is not empty".into());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Create {
    pub path: PathBuf,
    pub reference: String,
    pub new_branch: Option<String>,
    tip: String,
    directory: PathBuf,
}
impl Create {
    pub fn prepare(
        repo: &Repository,
        path: PathBuf,
        reference: String,
        new_branch: Option<String>,
        token: &Cancel,
    ) -> Result<Self, String> {
        if !["refs/heads/", "refs/remotes/", "refs/tags/"]
            .iter()
            .any(|p| reference.starts_with(p))
        {
            return Err("Choose a local branch, remote branch or tag".into());
        }
        repo.text(&["check-ref-format", &reference], token)?;
        destination(&path)?;
        let new_branch = if new_branch.is_none() && reference.starts_with("refs/remotes/") {
            let remote = repo.text(
                &[
                    "for-each-ref",
                    "--format=%(refname)%09%(symref)%09%(refname:short)",
                    &reference,
                ],
                token,
            )?;
            let row = remote
                .lines()
                .find(|r| r.split('\t').next() == Some(reference.as_str()))
                .ok_or("Remote branch no longer exists")?;
            if !row.split('\t').nth(1).unwrap_or_default().is_empty() {
                return Err("Choose a remote branch, not its HEAD alias".into());
            }
            let remotes = repo.text(&["remote"], token)?;
            let short = reference.trim_start_matches("refs/remotes/");
            let name = remotes
                .lines()
                .filter_map(|r| short.strip_prefix(&format!("{r}/")))
                .min_by_key(|s| s.len())
                .ok_or("Remote branch has no configured remote")?;
            Some(name.to_owned())
        } else {
            new_branch
        };
        if let Some(name) = &new_branch {
            crate::branch_rename::validate_name(repo, name, token)?;
        } else if reference.starts_with("refs/heads/")
            && list(repo, token)?
                .iter()
                .any(|t| t.branch.as_deref() == Some(&reference))
        {
            return Err(
                "Branch is already checked out; open its worktree or create a new branch".into(),
            );
        }
        Ok(Self {
            tip: resolve(repo, &reference, token)?,
            directory: repo.editor_directory(token)?,
            path,
            reference,
            new_branch,
        })
    }
}

pub struct Report {
    pub command: Result<Outcome, String>,
    /// Read with a fresh cancellation token even after an interrupted mutation.
    pub trees: Result<Vec<Tree>, String>,
}
impl Report {
    pub fn succeeded(&self) -> bool {
        self.command
            .as_ref()
            .is_ok_and(|o| o.status.success() && o.stopped.is_none() && o.input_error.is_none())
            && self.trees.is_ok()
    }
    pub fn diagnostic(&self) -> String {
        match &self.command {
            Err(e) => e.clone(),
            Ok(o) => {
                let mut output = crate::rebase_operation::collect_errors(
                    &String::from_utf8_lossy(&o.stdout.bytes()),
                    &String::from_utf8_lossy(&o.stderr.bytes()),
                );
                let reason = match o.stopped {
                    Some(operation_process::StopReason::Cancelled) => {
                        Some("Worktree operation cancelled".into())
                    }
                    Some(operation_process::StopReason::TimedOut) => {
                        Some("Worktree operation timed out".into())
                    }
                    None => o.input_error.as_ref().map(|e| e.to_string()).or_else(|| {
                        (!o.status.success()).then(|| format!("Git exited with {}", o.status))
                    }),
                };
                if let Some(reason) = reason {
                    output = crate::rebase_operation::collect_errors(&reason, &output);
                }
                match &self.trees {
                    Err(e) => format!("{output}\n{e}"),
                    _ => output,
                }
            }
        }
    }
}
fn run(
    repo: &Repository,
    cmd: &mut std::process::Command,
    token: &Cancel,
    progress: &Progress,
) -> Report {
    let command = operation_process::run(cmd, token, progress, None).map_err(|e| e.to_string());
    let trees = list(repo, &Cancel::default());
    Report { command, trees }
}
pub fn create(
    repo: &Repository,
    prepared: &Create,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let _guard = repo.operation_guard(token)?;
    let fresh = Create::prepare(
        repo,
        prepared.path.clone(),
        prepared.reference.clone(),
        prepared.new_branch.clone(),
        token,
    )?;
    if fresh != *prepared {
        return Err("Repository changed; review worktree creation again".into());
    }
    let mut cmd = repo.operation_command();
    // Do not start unprompted network access for missing partial-clone objects.
    cmd.env("GIT_NO_LAZY_FETCH", "1").args(["worktree", "add"]);
    if let Some(name) = &prepared.new_branch {
        cmd.args(["-b", name]);
    }
    cmd.arg("--").arg(&prepared.path);
    // Git needs a local branch name here; a full refs/heads path would detach HEAD.
    cmd.arg(
        prepared
            .reference
            .strip_prefix("refs/heads/")
            .unwrap_or(&prepared.reference),
    );
    Ok(run(repo, &mut cmd, token, progress))
}
pub fn remove(
    repo: &Repository,
    expected: &Tree,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let _guard = repo.operation_guard(token)?;
    let fresh = list(repo, token)?
        .into_iter()
        .find(|t| t.path == expected.path)
        .ok_or("Worktree no longer exists")?;
    if fresh != *expected {
        return Err("Worktree changed; review removal again".into());
    }
    if fresh.main || fresh.current || fresh.bare {
        return Err("This worktree cannot be removed".into());
    }
    let mut cmd = repo.operation_command();
    cmd.args(["worktree", "remove", "--force", "--"])
        .arg(&fresh.path);
    Ok(run(repo, &mut cmd, token, progress))
}

/// Remove metadata for the missing trees shown in the confirmation, respecting Git's
/// expiry and locked-worktree rules. Never deletes working directories.
pub fn prune(
    repo: &Repository,
    expected: &[PathBuf],
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let _guard = repo.operation_guard(token)?;
    let mut fresh: Vec<_> = list(repo, token)?
        .into_iter()
        .filter(|t| t.prunable)
        .map(|t| t.path)
        .collect();
    let mut expected = expected.to_vec();
    fresh.sort();
    expected.sort();
    if fresh.is_empty() || fresh != expected {
        return Err("Missing worktrees changed; review pruning again".into());
    }
    let mut cmd = repo.operation_command();
    cmd.args(["worktree", "prune"]);
    Ok(run(repo, &mut cmd, token, progress))
}
