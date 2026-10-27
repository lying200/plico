//! Ref checkout preparation. Rebased GitCheckoutAction / GitRemoteBranchesUtil /
//! GitBranchCheckoutOperation (4aec35256d765c217aa0d662b82ac4e860c396fd).
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    root: std::path::PathBuf,
    git_dir: std::path::PathBuf,
    pub reference: String,
    pub target: String,
    pub head: String,
    symbolic_head: String,
    raw_ref: String,
    pub local: Option<String>,
    local_tip: Option<String>,
    upstream: Option<String>,
    pub naming_conflict: bool,
    pub local_ahead: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Checkout,
    Rebase,
    Reset,
}

fn optional_ref(
    repo: &Repository,
    reference: &str,
    token: &Cancel,
) -> Result<Option<String>, String> {
    let refs = repo.text(
        &[
            "for-each-ref",
            "--format=%(refname)%09%(objectname)",
            reference,
        ],
        token,
    )?;
    Ok(refs
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(name, _)| *name == reference)
        .map(|(_, oid)| oid.to_owned()))
}
fn upstream(repo: &Repository, reference: &str, token: &Cancel) -> Result<Option<String>, String> {
    let refs = repo.text(
        &[
            "for-each-ref",
            "--format=%(refname)%09%(upstream)",
            reference,
        ],
        token,
    )?;
    Ok(refs
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(name, _)| *name == reference)
        .and_then(|(_, value)| (!value.is_empty()).then(|| value.to_owned())))
}
impl Prepared {
    pub fn repository(&self) -> Repository {
        Repository::new(&self.root)
    }
    pub fn read(
        repo: &Repository,
        reference: &str,
        target: &str,
        head: &str,
        local_name: Option<&str>,
        token: &Cancel,
    ) -> Result<Self, String> {
        if !["refs/heads/", "refs/remotes/", "refs/tags/"]
            .iter()
            .any(|p| reference.starts_with(p))
        {
            return Err("Unsupported checkout reference".into());
        }
        repo.text(&["check-ref-format", reference], token)?;
        let actual = repo.text(
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{reference}^{{commit}}"),
            ],
            token,
        )?;
        if actual.trim() != target {
            return Err("Reference changed; refresh before checking out".into());
        }
        let current = repo.text(&["rev-parse", "--verify", "HEAD"], token)?;
        if current.trim() != head {
            return Err("HEAD changed; refresh before checking out".into());
        }
        let symbolic_head = repo
            .text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?
            .trim()
            .to_owned();
        let raw_ref = optional_ref(repo, reference, token)?.ok_or("Reference no longer exists")?;
        let mut result = Self {
            root: repo.path.clone(),
            git_dir: repo.editor_directory(token)?,
            reference: reference.into(),
            target: target.into(),
            head: head.into(),
            symbolic_head,
            raw_ref,
            local: None,
            local_tip: None,
            upstream: None,
            naming_conflict: false,
            local_ahead: false,
        };
        if let Some(remote_ref) = reference.strip_prefix("refs/remotes/") {
            // Remote names may contain slashes; use configured names, longest first.
            let remotes = repo.text(&["remote"], token)?;
            let remote = remotes
                .lines()
                .filter(|r| remote_ref.starts_with(&format!("{r}/")))
                .max_by_key(|r| r.len())
                .ok_or("Remote no longer exists")?;
            let suggested = &remote_ref[remote.len() + 1..];
            let name = local_name.unwrap_or(suggested);
            if name.is_empty() || name.starts_with('-') {
                return Err("Invalid branch name".into());
            }
            let local_ref = format!("refs/heads/{name}");
            repo.text(&["check-ref-format", &local_ref], token)?;
            let tip = optional_ref(repo, &local_ref, token)?;
            let tracking = upstream(repo, &local_ref, token)?;
            result.naming_conflict = if local_name.is_some() {
                tip.is_some()
            } else {
                tracking.as_deref().is_some_and(|v| v != reference)
            };
            if let Some(tip) = &tip {
                result.local_ahead = !repo
                    .text(
                        &[
                            "rev-list",
                            "--max-count=1",
                            &format!("{target}..{tip}"),
                            "--",
                        ],
                        token,
                    )?
                    .trim()
                    .is_empty();
            }
            result.local = Some(name.into());
            result.local_tip = tip;
            result.upstream = tracking;
        }
        Ok(result)
    }
    fn validate(&self, repo: &Repository, choice: Choice, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.git_dir {
            return Err("Checkout repository changed; refresh before checking out".into());
        }
        let current = repo.text(&["rev-parse", "--verify", "HEAD"], token)?;
        let symbolic = repo.text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?;
        if current.trim() != self.head || symbolic.trim() != self.symbolic_head {
            return Err("HEAD changed; refresh before checking out".into());
        }
        if optional_ref(repo, &self.reference, token)?.as_ref() != Some(&self.raw_ref) {
            return Err("Reference changed; refresh before checking out".into());
        }
        if let Some(name) = &self.local {
            let reference = format!("refs/heads/{name}");
            if optional_ref(repo, &reference, token)? != self.local_tip
                || upstream(repo, &reference, token)? != self.upstream
            {
                return Err("Local branch changed; retry checkout".into());
            }
        }
        let state = repo.operation_state(token)?;
        // A merge awaiting its commit may still be checked out, as in
        // GitCheckoutOperation. Git preserves the staged changes and ends the
        // merge. This does not authorize starting another rebase or proceeding
        // with unmerged index entries.
        let checkout_from_merge =
            state.operation == crate::operation_state::Operation::Merge && choice != Choice::Rebase;
        if (state.in_progress() && !checkout_from_merge) || !state.conflicts.is_empty() {
            return Err("Resolve the current Git operation before checkout".into());
        }
        if self.naming_conflict {
            return Err("Choose a new local branch name".into());
        }
        Ok(())
    }
}

pub struct Report {
    pub verified: bool,
    pub command: Outcome,
    pub state: crate::operation_state::State,
    pub head: String,
    pub symbolic_head: String,
}
/// Preparation never writes. All choices revalidate the displayed refs under the
/// repository operation lock; resetting local commits is an explicit choice.
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    choice: Choice,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let _guard = repo.operation_guard(token)?;
    attempt(repo, prepared, choice, false, token, progress)
}
fn attempt(
    repo: &Repository,
    prepared: &Prepared,
    choice: Choice,
    force: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    prepared.validate(repo, choice, token)?;
    if prepared.local_ahead && choice == Choice::Checkout {
        return Err("Choose how to handle local commits".into());
    }
    if choice == Choice::Rebase && (!prepared.local_ahead || prepared.local.is_none()) {
        return Err("Invalid checkout rebase choice".into());
    }
    let mut command = repo.operation_command();
    command.env("LC_ALL", "C").env("LANG", "C");
    match (&prepared.local, choice) {
        (Some(name), Choice::Rebase) => {
            command.env("GIT_EDITOR", "true").args([
                "rebase",
                "--no-autostash",
                &prepared.target,
                name,
            ]);
        }
        (Some(name), _) => {
            command.arg("checkout");
            if force {
                command.arg("--force");
            }
            command.args([
                "--track",
                if prepared.local_tip.is_some() {
                    "-B"
                } else {
                    "-b"
                },
                name,
                &prepared.reference,
                "--",
            ]);
        }
        (None, _) => {
            let reference = prepared
                .reference
                .strip_prefix("refs/heads/")
                .unwrap_or(&prepared.reference);
            command.arg("checkout");
            if force {
                command.arg("--force");
            }
            command.args([reference, "--"]);
        }
    }
    let command =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string())?;
    let refresh = Cancel::default();
    let state = repo.operation_state(&refresh)?;
    let head = repo
        .text(&["rev-parse", "--verify", "HEAD"], &refresh)?
        .trim()
        .to_owned();
    let symbolic_head = repo
        .text(&["rev-parse", "--symbolic-full-name", "HEAD"], &refresh)?
        .trim()
        .to_owned();
    let expected_branch = prepared
        .local
        .as_ref()
        .map(|name| format!("refs/heads/{name}"))
        .or_else(|| {
            prepared
                .reference
                .starts_with("refs/heads/")
                .then(|| prepared.reference.clone())
        });
    let attached = expected_branch.as_ref().map_or(state.detached, |name| {
        !state.detached && &symbolic_head == name
    });
    let target_matches = if choice == Choice::Rebase {
        repo.text(
            &[
                "rev-list",
                "--max-count=1",
                &format!("{}..{}", head, prepared.target),
                "--",
            ],
            &refresh,
        )?
        .trim()
        .is_empty()
    } else {
        head == prepared.target
    };
    let verified = command.status.success()
        && command.stopped.is_none()
        && !state.in_progress()
        && state.conflicts.is_empty()
        && attached
        && target_matches;
    Ok(Report {
        verified,
        command,
        state,
        head,
        symbolic_head,
    })
}

impl Report {
    pub fn status(&self) -> crate::checkout::Status {
        crate::checkout::command_status(&self.command, self.verified)
    }
    pub fn affected_files(&self) -> crate::checkout_files::Files {
        let status = self.status();
        if !matches!(
            status,
            crate::checkout::Status::LocalChanges | crate::checkout::Status::UntrackedFiles
        ) {
            return Default::default();
        }
        crate::checkout_files::parse(
            &self.command.stderr.bytes(),
            self.command.stderr.truncated,
            status == crate::checkout::Status::UntrackedFiles,
        )
    }
}

pub struct RecoveryReport {
    pub checkout: Option<Result<Report, String>>,
    pub save: Option<Result<Outcome, String>>,
    pub restore: Result<Option<Outcome>, String>,
    pub saved_oid: Option<String>,
    pub head: Result<String, String>,
    pub branch: Result<String, String>,
    pub state: Result<crate::operation_state::State, String>,
}
impl RecoveryReport {
    pub fn restored(&self) -> bool {
        self.saved_oid.is_none()
            && self.restore.as_ref().is_ok_and(|r| {
                r.as_ref()
                    .is_none_or(|o| o.status.success() && o.stopped.is_none())
            })
            && self.state.as_ref().is_ok_and(|s| s.conflicts.is_empty())
    }
    pub fn checked_out(&self) -> bool {
        self.checkout.as_ref().is_some_and(|r| {
            r.as_ref().is_ok_and(|r| {
                r.verified
                    && self.head.as_ref().is_ok_and(|head| head == &r.head)
                    && self
                        .branch
                        .as_ref()
                        .is_ok_and(|branch| branch == &r.symbolic_head)
            })
        })
    }
}
/// Explicit recovery only. Retains the original pinned reference and branch
/// creation/reset options, and restores saved changes even if checkout fails.
pub fn recover(
    repo: &Repository,
    prepared: &Prepared,
    choice: Choice,
    smart: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<RecoveryReport, String> {
    if choice == Choice::Rebase {
        return Err("Checkout recovery cannot replace a rebase".into());
    }
    let mut session = crate::update_session::Session::begin(repo, token)?;
    prepared.validate(repo, choice, token)?;
    // Validate every user choice before saving any local changes.
    if prepared.local_ahead && choice == Choice::Checkout {
        return Err("Choose how to handle local commits".into());
    }
    let save = smart.then(|| {
        session.save(
            &format!("Plico: checkout {}", prepared.reference),
            token,
            progress,
        )
    });
    let checkout = if save.as_ref().is_none_or(|r| {
        r.as_ref()
            .is_ok_and(|o| o.status.success() && o.stopped.is_none())
    }) {
        Some(attempt(repo, prepared, choice, !smart, token, progress))
    } else {
        None
    };
    let restore = if smart {
        session.restore_saved(true, progress)
    } else {
        Ok(None)
    };
    Ok(RecoveryReport {
        checkout,
        save,
        restore,
        saved_oid: session.saved_oid().map(str::to_owned),
        head: repo
            .text(&["rev-parse", "--verify", "HEAD"], &Cancel::default())
            .map(|s| s.trim().to_owned()),
        branch: repo
            .text(
                &["rev-parse", "--symbolic-full-name", "HEAD"],
                &Cancel::default(),
            )
            .map(|s| s.trim().to_owned()),
        state: repo.operation_state(&Cancel::default()),
    })
}
