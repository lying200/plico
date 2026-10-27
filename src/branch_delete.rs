//! Rebased GitDeleteBranchOperation: merged check, explicit unmerged handling,
//! post-operation refresh and restore of the deleted tip and branch config.
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
};
#[derive(Clone)]
pub struct Prepared {
    root: std::path::PathBuf,
    pub name: String,
    pub tip: String,
    pub base: String,
    pub unmerged: Vec<String>,
    head: String,
    symbolic: String,
    config: Vec<(String, String)>,
}
fn tip(repo: &Repository, reference: &str, token: &Cancel) -> Result<String, String> {
    repo.text(
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ],
        token,
    )
    .map(|s| s.trim().into())
}
fn config(repo: &Repository, name: &str, token: &Cancel) -> Result<Vec<(String, String)>, String> {
    Ok(repo
        .text(&["config", "--local", "--null", "--list"], token)?
        .split('\0')
        .filter_map(|v| v.split_once('\n'))
        .filter(|(key, _)| {
            key.strip_prefix("branch.")
                .and_then(|k| k.rsplit_once('.'))
                .is_some_and(|(section, _)| section == name)
        })
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect())
}
impl Prepared {
    pub fn repository(&self) -> Repository {
        Repository::new(&self.root)
    }
    fn validate_repository(&self, repo: &Repository) -> Result<(), String> {
        if crate::recent_projects::identity(&repo.path)
            != crate::recent_projects::identity(&self.root)
        {
            return Err("Branch deletion belongs to a different repository".into());
        }
        Ok(())
    }

    pub fn read(repo: &Repository, reference: &str, token: &Cancel) -> Result<Self, String> {
        let name = reference
            .strip_prefix("refs/heads/")
            .ok_or("Only local branches can be deleted here")?;
        repo.text(&["check-ref-format", reference], token)?;
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before deleting a branch".into());
        }
        let worktrees = repo.text(&["worktree", "list", "--porcelain", "-z"], token)?;
        if worktrees
            .split('\0')
            .any(|v| v.strip_prefix("branch ") == Some(reference))
        {
            return Err("This branch is checked out in a worktree".into());
        }
        let head = tip(repo, "HEAD", token)?;
        let symbolic = repo
            .text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?
            .trim()
            .to_owned();
        let branch_tip = tip(repo, reference, token)?;
        let refs = repo.text(
            &[
                "for-each-ref",
                "--format=%(refname)%09%(upstream)",
                reference,
            ],
            token,
        )?;
        let upstream = refs
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .find(|(r, _)| *r == reference)
            .map(|(_, u)| u)
            .filter(|u| !u.is_empty());
        // A removed tracking ref falls back to HEAD, just as git branch -d does.
        let base = if let Some(u) = upstream {
            let refs = repo.text(&["for-each-ref", "--format=%(refname)", u], token)?;
            if refs.lines().any(|r| r == u) {
                tip(repo, u, token)?
            } else {
                head.clone()
            }
        } else {
            head.clone()
        };
        let unmerged = repo
            .text(
                &[
                    "log",
                    "--max-count=101",
                    "--format=%h %s",
                    &format!("{base}..{branch_tip}"),
                    "--",
                ],
                token,
            )?
            .lines()
            .map(str::to_owned)
            .collect();
        Ok(Self {
            root: repo.path.clone(),
            name: name.into(),
            tip: branch_tip,
            base,
            unmerged,
            head,
            symbolic,
            config: config(repo, name, token)?,
        })
    }
}
pub struct Report {
    pub command: Result<Outcome, String>,
    pub deleted: bool,
    pub refresh_error: Option<String>,
}
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    discard_unmerged: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    prepared.validate_repository(repo)?;
    let _guard = repo.operation_guard(token)?;
    let current = Prepared::read(repo, &format!("refs/heads/{}", prepared.name), token)?;
    if current.tip != prepared.tip
        || current.head != prepared.head
        || current.symbolic != prepared.symbolic
        || current.base != prepared.base
        || current.config != prepared.config
    {
        return Err("Repository changed; review branch deletion again".into());
    }
    if !current.unmerged.is_empty() && !discard_unmerged {
        return Err("Branch has unmerged commits; explicit confirmation is required".into());
    }
    let mut command = repo.operation_command();
    command.args([
        "branch",
        if discard_unmerged { "-D" } else { "-d" },
        "--",
        &prepared.name,
    ]);
    let command =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    let reference = format!("refs/heads/{}", prepared.name);
    let remaining = repo.text(
        &["for-each-ref", "--format=%(refname)", &reference],
        &Cancel::default(),
    );
    let deleted = remaining
        .as_ref()
        .is_ok_and(|v| !v.lines().any(|r| r == reference));
    Ok(Report {
        command,
        deleted,
        refresh_error: remaining.err(),
    })
}
/// Restore only into an absent name. Never reset or check out the restored branch.
pub fn restore(
    repo: &Repository,
    prepared: &Prepared,
    token: &Cancel,
    progress: &Progress,
) -> Result<(), String> {
    prepared.validate_repository(repo)?;
    let _guard = repo.operation_guard(token)?;
    let reference = format!("refs/heads/{}", prepared.name);
    let refs = repo.text(&["for-each-ref", "--format=%(refname)", &reference], token)?;
    if refs.lines().any(|r| r == reference) {
        return Err("A branch with this name already exists".into());
    }
    let mut command = repo.operation_command();
    command.args(["branch", "--no-track", "--", &prepared.name, &prepared.tip]);
    let outcome = operation_process::run(&mut command, token, progress, None);
    // Cancellation can arrive after Git wrote the ref. Inspect the actual result
    // before deciding whether the configuration cleanup should run.
    let restored = tip(repo, &reference, &Cancel::default());
    if !restored.as_ref().is_ok_and(|id| id == &prepared.tip) {
        return Err(match outcome {
            Err(error) => error.to_string(),
            Ok(result) => format!(
                "Branch restore did not complete: {}\n{}",
                String::from_utf8_lossy(&result.stderr.bytes()),
                restored
                    .err()
                    .unwrap_or_else(|| "Restored branch points to a different commit".into())
            ),
        });
    }
    // Once the ref exists, finish restoring its tracking config even if cancellation
    // arrived. A config failure leaves the restored ref and is reported explicitly.
    for (key, value) in &prepared.config {
        repo.text(
            &["config", "--local", "--add", key, value],
            &Cancel::default(),
        )
        .map_err(|e| {
            format!("Branch restored, but its configuration could not be restored: {e}")
        })?;
    }
    if tip(repo, &reference, &Cancel::default())? != prepared.tip {
        return Err("Restored branch changed; refresh the repository".into());
    }
    Ok(())
}
