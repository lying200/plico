//! Rebased 4aec352 GitRenameBranchAction/GitRenameBranchOperation
//! (JetBrains and contributors, Apache-2.0): rename, then optionally unset upstream.
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub name: String,
    pub tip: String,
    pub has_upstream: bool,
    directory: PathBuf,
    head: String,
    symbolic: String,
    config: Vec<(String, String)>,
}
impl Prepared {
    pub fn read(repo: &Repository, reference: &str, token: &Cancel) -> Result<Self, String> {
        let name = reference
            .strip_prefix("refs/heads/")
            .ok_or("Only local branches can be renamed")?;
        repo.text(&["check-ref-format", reference], token)?;
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before renaming a branch".into());
        }
        let config: Vec<_> = repo
            .text(&["config", "--null", "--list"], token)?
            .split('\0')
            .filter_map(|entry| entry.split_once('\n'))
            .filter(|(key, _)| {
                key.strip_prefix("branch.")
                    .and_then(|k| k.rsplit_once('.'))
                    .is_some_and(|(section, _)| section == name)
            })
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect();
        let has_upstream = config
            .iter()
            .any(|(k, _)| k == &format!("branch.{name}.remote"))
            && config
                .iter()
                .any(|(k, _)| k == &format!("branch.{name}.merge"));
        Ok(Self {
            name: name.into(),
            tip: resolve(repo, reference, token)?,
            has_upstream,
            directory: repo.editor_directory(token)?,
            head: resolve(repo, "HEAD", token)?,
            symbolic: repo
                .text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?
                .trim()
                .into(),
            config,
        })
    }
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
    .map(|s| s.trim().into())
}
pub fn validate_name(repo: &Repository, name: &str, token: &Cancel) -> Result<(), String> {
    if name.is_empty()
        || name.trim() != name
        || name.starts_with('-')
        || name.eq_ignore_ascii_case("HEAD")
    {
        return Err("Invalid branch name".into());
    }
    let reference = format!("refs/heads/{name}");
    repo.text(&["check-ref-format", &reference], token)?;
    let refs = repo.text(
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads/",
            "refs/remotes/",
        ],
        token,
    )?;
    for r in refs.lines() {
        if r == reference
            || r.starts_with(&format!("{reference}/"))
            || reference.starts_with(&format!("{r}/"))
        {
            return Err("A branch or branch directory with this name already exists".into());
        }
        if r.strip_prefix("refs/remotes/") == Some(name) {
            return Err("Branch name clashes with a remote branch".into());
        }
    }
    Ok(())
}
pub struct Report {
    pub rename: Result<Outcome, String>,
    pub unset: Option<Result<Outcome, String>>,
    /// Actual ref state, even when Git was cancelled or a later config write failed.
    pub renamed: bool,
    pub error: Option<String>,
}
fn good(o: &Outcome) -> bool {
    o.status.success() && o.stopped.is_none() && o.input_error.is_none()
}
impl Report {
    pub fn diagnostic(&self) -> String {
        let mut messages = Vec::new();
        for result in std::iter::once(&self.rename).chain(self.unset.iter()) {
            match result {
                Err(e) => messages.push(e.clone()),
                Ok(o) if !good(o) => messages.push(crate::rebase_operation::collect_errors(
                    &String::from_utf8_lossy(&o.stdout.bytes()),
                    &String::from_utf8_lossy(&o.stderr.bytes()),
                )),
                _ => {}
            }
        }
        if let Some(e) = &self.error {
            messages.push(e.clone());
        }
        crate::rebase_operation::display_text(&messages.join("\n"))
    }
    pub fn succeeded(&self) -> bool {
        self.renamed
            && self.error.is_none()
            && self.rename.as_ref().is_ok_and(good)
            && self
                .unset
                .as_ref()
                .is_none_or(|r| r.as_ref().is_ok_and(good))
    }
}
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    name: &str,
    unset_upstream: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let _guard = repo.operation_guard(token)?;
    let reference = format!("refs/heads/{}", prepared.name);
    if Prepared::read(repo, &reference, token)? != *prepared {
        return Err("Repository changed; review branch rename again".into());
    }
    validate_name(repo, name, token)?;
    let mut cmd = repo.operation_command();
    cmd.args(["branch", "-m", "--", &prepared.name, name]);
    let rename = operation_process::run(&mut cmd, token, progress, None).map_err(|e| e.to_string());
    let mut unset = None;
    if rename.as_ref().is_ok_and(good) && unset_upstream && prepared.has_upstream {
        let mut cmd = repo.operation_command();
        cmd.args(["branch", "--unset-upstream", "--", name]);
        unset = Some(
            operation_process::run(&mut cmd, token, progress, None).map_err(|e| e.to_string()),
        );
    }
    // Cancellation does not prove that no mutation happened. Always inspect with a fresh token.
    let fresh = Cancel::default();
    let target = format!("refs/heads/{name}");
    let refs = repo.text(
        &[
            "for-each-ref",
            "--format=%(refname)%09%(objectname)",
            "refs/heads/",
        ],
        &fresh,
    );
    let renamed = refs.as_ref().is_ok_and(|refs| {
        !refs
            .lines()
            .any(|r| r.split_once('\t').is_some_and(|(r, _)| r == reference))
            && refs.lines().any(|r| {
                r.split_once('\t')
                    .is_some_and(|(r, oid)| r == target && oid == prepared.tip)
            })
    });
    let mut error = refs.err();
    if renamed && unset_upstream && prepared.has_upstream {
        // Included/global config can survive a successful local --unset-upstream.
        // Verify the effective tracking state rather than trusting exit status alone.
        match repo.text(
            &["for-each-ref", "--format=%(refname)%09%(upstream)", &target],
            &fresh,
        ) {
            Ok(rows)
                if rows.lines().any(|row| {
                    row.split_once('\t')
                        .is_some_and(|(r, u)| r == target && !u.is_empty())
                }) =>
            {
                error = Some(
                    "Branch renamed, but upstream tracking remains in Git configuration".into(),
                );
            }
            Err(e) => {
                error = Some(e);
            }
            _ => {}
        }
    }
    Ok(Report {
        rename,
        unset,
        renamed,
        error,
    })
}
