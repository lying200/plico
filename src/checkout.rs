//! Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd:
//! GitCheckoutRevisionAction, GitCheckoutOperation and GitImpl.checkout.
//! Upstream copyright JetBrains s.r.o. and contributors, Apache-2.0.
//! One ordinary attempt; smart checkout/force require separate caller choices.
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
    operation_state::State,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Cancelled,
    TimedOut,
    Unmerged,
    LocalChanges,
    UntrackedFiles,
    Failed,
}

pub struct ResultState {
    pub command: Result<Outcome, String>,
    pub state: Result<State, String>,
    pub head: Result<String, String>,
}
impl ResultState {
    pub fn affected_files(&self, target: &str) -> crate::checkout_files::Files {
        let status = self.status(target);
        if !matches!(status, Status::LocalChanges | Status::UntrackedFiles) {
            return Default::default();
        }
        let Ok(command) = &self.command else {
            return Default::default();
        };
        crate::checkout_files::parse(
            &command.stderr.bytes(),
            command.stderr.truncated,
            status == Status::UntrackedFiles,
        )
    }

    pub fn status(&self, target: &str) -> Status {
        self.status_with_success(self.checked_out(target))
    }
    fn status_with_success(&self, success: bool) -> Status {
        self.command
            .as_ref()
            .map_or(Status::Failed, |outcome| command_status(outcome, success))
    }

    /// A successful exit alone is insufficient: hooks or another process may
    /// have changed HEAD. Retain command diagnostics and actual state separately.
    pub fn checked_out(&self, target: &str) -> bool {
        self.command
            .as_ref()
            .is_ok_and(|out| out.status.success() && out.stopped.is_none())
            && self.state.as_ref().is_ok_and(|state| state.detached)
            && self.head.as_ref().is_ok_and(|head| head == target)
    }
}

#[derive(Debug)]
pub struct ResolvedInput {
    pub target: String,
    pub expected_head: String,
}

/// Branch-menu checkout must stay attached, unlike the revision-input action.
/// Keep the post-command symbolic ref as evidence, including failed commands.
pub struct LocalResult {
    pub result: ResultState,
    pub branch: Result<String, String>,
}
impl LocalResult {
    pub fn status(&self, full_ref: &str, target: &str) -> Status {
        self.result
            .status_with_success(self.checked_out(full_ref, target))
    }
    pub fn affected_files(&self, full_ref: &str, target: &str) -> crate::checkout_files::Files {
        let status = self.status(full_ref, target);
        if !matches!(status, Status::LocalChanges | Status::UntrackedFiles) {
            return Default::default();
        }
        let Ok(command) = &self.result.command else {
            return Default::default();
        };
        crate::checkout_files::parse(
            &command.stderr.bytes(),
            command.stderr.truncated,
            status == Status::UntrackedFiles,
        )
    }
    pub fn checked_out(&self, full_ref: &str, target: &str) -> bool {
        self.result
            .command
            .as_ref()
            .is_ok_and(|out| out.status.success() && out.stopped.is_none())
            && self
                .result
                .state
                .as_ref()
                .is_ok_and(|state| !state.detached)
            && self.result.head.as_ref().is_ok_and(|head| head == target)
            && self.branch.as_ref().is_ok_and(|branch| branch == full_ref)
    }
}

/// GitCheckoutAction uses the local branch's short name and detach=false.
/// Revalidate the displayed branch tip before issuing the command. Remote
/// branches need their own upstream workflow; never silently detach them here.
pub fn execute_local(
    repo: &Repository,
    full_ref: &str,
    target: &str,
    expected_head: &str,
    token: &Cancel,
    progress: &Progress,
) -> Result<LocalResult, String> {
    let _guard = repo.operation_guard(token)?;
    attempt_local(
        repo,
        full_ref,
        target,
        expected_head,
        false,
        token,
        progress,
    )
}

fn validate_local<'a>(
    repo: &Repository,
    full_ref: &'a str,
    target: &str,
    expected_head: &str,
    token: &Cancel,
) -> Result<&'a str, String> {
    let name = full_ref
        .strip_prefix("refs/heads/")
        .filter(|s| !s.is_empty() && !s.starts_with('-'))
        .ok_or("Expected a local branch reference")?;
    repo.text(&["check-ref-format", full_ref], token)?;
    validate(repo, target, expected_head, token)?;
    let actual = repo.text(
        &["rev-parse", "--verify", "--end-of-options", full_ref],
        token,
    )?;
    if actual.trim() != target {
        return Err("Branch changed; refresh before checking out".into());
    }
    Ok(name)
}

fn attempt_local(
    repo: &Repository,
    full_ref: &str,
    target: &str,
    expected_head: &str,
    force: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<LocalResult, String> {
    let name = validate_local(repo, full_ref, target, expected_head, token)?;
    let mut command = repo.operation_command();
    command.env("LC_ALL", "C").env("LANG", "C");
    command.arg("checkout");
    if force {
        command.arg("--force");
    }
    command.args([name, "--"]);
    let command =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    let refresh = Cancel::default();
    let state = repo.operation_state(&refresh);
    let head = repo
        .text(&["rev-parse", "--verify", "HEAD"], &refresh)
        .map(|s| s.trim().to_owned());
    let branch = repo
        .text(&["symbolic-ref", "--quiet", "HEAD"], &refresh)
        .map(|s| s.trim().to_owned());
    Ok(LocalResult {
        result: ResultState {
            command,
            state,
            head,
        },
        branch,
    })
}

pub struct LocalSmartResult {
    pub save: Result<Outcome, String>,
    pub checkout: Option<Result<LocalResult, String>>,
    pub restore: Result<Option<Outcome>, String>,
    pub saved_oid: Option<String>,
    pub state: Result<State, String>,
    pub head: Result<String, String>,
    pub branch: Result<String, String>,
}

/// Explicit Smart Checkout, preserving the local-branch identity through save,
/// checkout and restore. Session owns the operation lock; do not reacquire it.
pub fn smart_execute_local(
    repo: &Repository,
    full_ref: &str,
    target: &str,
    expected_head: &str,
    reinstate_index: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<LocalSmartResult, String> {
    let mut session = crate::update_session::Session::begin(repo, token)?;
    validate_local(repo, full_ref, target, expected_head, token)?;
    let save = session.save(&format!("Git Review: checkout {full_ref}"), token, progress);
    let checkout = if save
        .as_ref()
        .is_ok_and(|out| out.status.success() && out.stopped.is_none())
    {
        Some(attempt_local(
            repo,
            full_ref,
            target,
            expected_head,
            false,
            token,
            progress,
        ))
    } else {
        None
    };
    let restore = session.restore_saved(reinstate_index, progress);
    let saved_oid = session.saved_oid().map(str::to_owned);
    let refresh = Cancel::default();
    let state = repo.operation_state(&refresh);
    let head = repo
        .text(&["rev-parse", "--verify", "HEAD"], &refresh)
        .map(|s| s.trim().to_owned());
    let branch = repo
        .text(&["symbolic-ref", "--quiet", "HEAD"], &refresh)
        .map(|s| s.trim().to_owned());
    Ok(LocalSmartResult {
        save,
        checkout,
        restore,
        saved_oid,
        state,
        head,
        branch,
    })
}

/// Explicit destructive choice only, never an automatic failure retry.
pub fn force_execute_local(
    repo: &Repository,
    full_ref: &str,
    target: &str,
    expected_head: &str,
    token: &Cancel,
    progress: &Progress,
) -> Result<LocalResult, String> {
    let _guard = repo.operation_guard(token)?;
    attempt_local(repo, full_ref, target, expected_head, true, token, progress)
}

/// GitCheckoutFromInputAction passes detach=true even for branch names.
/// Resolve text without mutation; the existing checkout path revalidates HEAD
/// and the pinned commit before writing. Never pass revision text as an option.
pub fn resolve_input(
    repo: &Repository,
    input: &str,
    token: &Cancel,
) -> Result<ResolvedInput, String> {
    let input = input.trim();
    if input.is_empty() || input.contains(['\0', '\n', '\r']) {
        return Err("Enter a branch, tag or commit revision".into());
    }
    let expected_head = repo
        .text(&["rev-parse", "--verify", "HEAD"], token)?
        .trim()
        .to_owned();
    let target = repo
        .text(
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{input}^{{commit}}"),
            ],
            token,
        )?
        .trim()
        .to_owned();
    if !matches!(target.len(), 40 | 64) || !target.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Reference did not resolve to one commit".into());
    }
    Ok(ResolvedInput {
        target,
        expected_head,
    })
}

/// Worker-only; full commit IDs from the selected Log row, never revision text
/// or a path. `expected_head` guards stale UI selection before starting writes.
pub fn execute(
    repo: &Repository,
    target: &str,
    expected_head: &str,
    token: &Cancel,
    progress: &Progress,
) -> Result<ResultState, String> {
    let _guard = repo.operation_guard(token)?;
    attempt(repo, target, expected_head, false, token, progress)
}

fn validate(
    repo: &Repository,
    target: &str,
    expected_head: &str,
    token: &Cancel,
) -> Result<(), String> {
    let oid = |s: &str| matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit());
    if !oid(target) || !oid(expected_head) {
        return Err("Invalid checkout commit ID".into());
    }
    let current = repo.text(&["rev-parse", "--verify", "HEAD"], token)?;
    if current.trim() != expected_head {
        return Err("HEAD changed; refresh before checking out".into());
    }
    let resolved = repo.text(
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{target}^{{commit}}"),
        ],
        token,
    )?;
    if resolved.trim() != target {
        return Err("Checkout target is not a commit".into());
    }
    Ok(())
}

fn attempt(
    repo: &Repository,
    target: &str,
    expected_head: &str,
    force: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<ResultState, String> {
    validate(repo, target, expected_head, token)?;
    // Same hash checkout as GitCheckoutRevisionAction; suffix peels explicitly
    // as GitImpl's detach path, also avoiding a ref named like the full hash.
    let mut command = repo.operation_command();
    command.env("LC_ALL", "C").env("LANG", "C");
    command.arg("checkout");
    if force {
        command.arg("--force");
    }
    command.args([&format!("{target}^0"), "--"]);
    let command =
        operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string());
    // Never use a cancelled token to infer post-write state.
    let refresh = Cancel::default();
    let state = repo.operation_state(&refresh);
    let head = repo
        .text(&["rev-parse", "--verify", "HEAD"], &refresh)
        .map(|s| s.trim().to_owned());
    Ok(ResultState {
        command,
        state,
        head,
    })
}

/// Every stage is retained: a failed save can still create a recoverable stash,
/// and a successful checkout can be followed by a restore conflict.
pub struct SmartResult {
    pub save: Result<Outcome, String>,
    pub checkout: Option<Result<ResultState, String>>,
    pub restore: Result<Option<Outcome>, String>,
    pub saved_oid: Option<String>,
    pub state: Result<State, String>,
    pub head: Result<String, String>,
}

/// Only call after the user chooses Smart Checkout. This is not an automatic
/// retry policy. Like GitPreservingProcess.execute(), restoration is attempted
/// after a successful save regardless of the checkout attempt's outcome.
pub fn smart_execute(
    repo: &Repository,
    target: &str,
    expected_head: &str,
    reinstate_index: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<SmartResult, String> {
    let mut session = crate::update_session::Session::begin(repo, token)?;
    validate(repo, target, expected_head, token)?;
    let save = session.save(&format!("Git Review: checkout {target}"), token, progress);
    let checkout = if save
        .as_ref()
        .is_ok_and(|out| out.status.success() && out.stopped.is_none())
    {
        Some(attempt(repo, target, expected_head, false, token, progress))
    } else {
        None
    };
    let restore = session.restore_saved(reinstate_index, progress);
    let saved_oid = session.saved_oid().map(str::to_owned);
    let refresh = Cancel::default();
    let state = repo.operation_state(&refresh);
    let head = repo
        .text(&["rev-parse", "--verify", "HEAD"], &refresh)
        .map(|s| s.trim().to_owned());
    Ok(SmartResult {
        save,
        checkout,
        restore,
        saved_oid,
        state,
        head,
    })
}

/// Explicit Force Checkout choice only, matching GitImpl.checkout(force=true).
/// This discards tracked modifications and may overwrite obstructing files.
pub fn force_execute(
    repo: &Repository,
    target: &str,
    expected_head: &str,
    token: &Cancel,
    progress: &Progress,
) -> Result<ResultState, String> {
    let _guard = repo.operation_guard(token)?;
    attempt(repo, target, expected_head, true, token, progress)
}

/// Shared ordering for local, revision, tag and remote checkout failures.
pub(crate) fn command_status(outcome: &Outcome, success: bool) -> Status {
    use crate::{operation_events::Event, operation_process::StopReason};
    match outcome.stopped {
        Some(StopReason::Cancelled) => return Status::Cancelled,
        Some(StopReason::TimedOut) => return Status::TimedOut,
        None => {}
    }
    if success {
        return Status::Success;
    }
    // Match GitCheckoutOperation failure ordering. Do not reinterpret a
    // successful command with unexpected post-state as an overwrite failure.
    if !outcome.status.success() {
        if outcome.events.contains(Event::UnmergedPreventingCheckout) {
            return Status::Unmerged;
        }
        if outcome
            .events
            .contains(Event::LocalChangesWouldBeOverwritten)
        {
            return Status::LocalChanges;
        }
        if outcome
            .events
            .contains(Event::UntrackedFilesWouldBeOverwritten)
        {
            return Status::UntrackedFiles;
        }
    }
    Status::Failed
}
