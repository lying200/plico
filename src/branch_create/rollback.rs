//! Checkout rollback for branches created by this operation. Rebased 4aec352
//! GitCheckoutOperation restores the previous branch then deletes the new one.
//! Preserve local drafts through the existing Smart Checkout lifecycle instead
//! of upstream's force-checkout, which discards them.
use super::Prepared;
use crate::{
    branch_delete, checkout,
    git::Cancel,
    operation_process::{Outcome, Progress},
};

pub enum CheckoutStep {
    AlreadyRestored,
    Local(checkout::LocalSmartResult),
    Detached(checkout::SmartResult),
}
pub struct Report {
    pub checkout: CheckoutStep,
    pub deletion: Option<Result<branch_delete::Report, String>>,
}
impl Report {
    pub fn succeeded(&self) -> bool {
        self.deletion.as_ref().is_some_and(|result| {
            result.as_ref().is_ok_and(|report| {
                report.deleted
                    && report.refresh_error.is_none()
                    && report.command.as_ref().is_ok_and(command_succeeded)
            })
        })
    }
}
fn command_succeeded(outcome: &Outcome) -> bool {
    outcome.status.success() && outcome.stopped.is_none()
}
fn restored(result: &Result<Option<Outcome>, String>) -> bool {
    result
        .as_ref()
        .is_ok_and(|outcome| outcome.as_ref().is_none_or(command_succeeded))
}

pub(super) fn checkout_created(
    prepared: &Prepared,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    if !prepared.checkout || prepared.previous_tip.is_some() {
        return Err("This operation requires a different rollback mode".into());
    }
    let repo = prepared.repository();
    if repo.editor_directory(token)? != prepared.directory {
        return Err("Repository changed; checkout rollback was not performed".into());
    }
    let reference = format!("refs/heads/{}", prepared.name);
    if super::resolve(&repo, &reference, token)? != prepared.target {
        return Err("Created branch changed; checkout rollback was not performed".into());
    }
    let head = super::resolve(&repo, "HEAD", token)?;
    let symbolic = repo.text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?;
    let symbolic = symbolic.trim();
    let original_ref = prepared
        .symbolic_head
        .strip_prefix("refs/heads/")
        .map(|_| prepared.symbolic_head.as_str());
    if let Some(original_ref) = original_ref {
        if super::resolve(&repo, original_ref, token)? != prepared.head {
            return Err("Original branch changed; checkout rollback was not performed".into());
        }
    }
    let already_restored = head == prepared.head && symbolic == prepared.symbolic_head;
    if !already_restored && (head != prepared.target || symbolic != reference) {
        return Err("HEAD changed; checkout rollback was not performed".into());
    }
    let checkout = if already_restored {
        CheckoutStep::AlreadyRestored
    } else if let Some(original_ref) = original_ref {
        CheckoutStep::Local(checkout::smart_execute_local(
            &repo,
            original_ref,
            &prepared.head,
            &prepared.target,
            true,
            token,
            progress,
        )?)
    } else {
        CheckoutStep::Detached(checkout::smart_execute(
            &repo,
            &prepared.head,
            &prepared.target,
            true,
            token,
            progress,
        )?)
    };
    let can_delete = match &checkout {
        CheckoutStep::AlreadyRestored => true,
        CheckoutStep::Local(result) => {
            result.save.as_ref().is_ok_and(command_succeeded)
                && restored(&result.restore)
                && result.saved_oid.is_none()
                && result.checkout.as_ref().is_some_and(|result| {
                    result
                        .as_ref()
                        .is_ok_and(|r| r.checked_out(&prepared.symbolic_head, &prepared.head))
                })
                && result
                    .state
                    .as_ref()
                    .is_ok_and(|state| !state.in_progress() && state.conflicts.is_empty())
        }
        CheckoutStep::Detached(result) => {
            result.save.as_ref().is_ok_and(command_succeeded)
                && restored(&result.restore)
                && result.saved_oid.is_none()
                && result.checkout.as_ref().is_some_and(|result| {
                    result.as_ref().is_ok_and(|r| r.checked_out(&prepared.head))
                })
                && result
                    .state
                    .as_ref()
                    .is_ok_and(|state| !state.in_progress() && state.conflicts.is_empty())
        }
    };
    // Keep every save/restore result, including retained stash identity on a
    // conflict. A successful checkout alone must not delete the recovery branch.
    let deletion = can_delete.then(|| {
        let deletion = branch_delete::Prepared::read(&repo, &reference, token)?;
        if deletion.tip != prepared.target
            || super::resolve(&repo, "HEAD", token)? != prepared.head
            || repo
                .text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?
                .trim()
                != prepared.symbolic_head
        {
            return Err("Repository changed before rollback branch deletion".into());
        }
        branch_delete::execute(&repo, &deletion, true, token, progress)
    });
    Ok(Report { checkout, deletion })
}

/// An overwritten existing ref is restored, never deleted. The command and
/// observed ref state remain separate when cancellation arrives after a write.
pub struct RefReport {
    pub command: Result<Outcome, String>,
    pub restored: bool,
    pub refresh_error: Option<String>,
}
impl RefReport {
    pub fn succeeded(&self) -> bool {
        self.restored
            && self.refresh_error.is_none()
            && self.command.as_ref().is_ok_and(command_succeeded)
    }
}

pub(super) fn overwritten_ref(
    prepared: &Prepared,
    token: &Cancel,
    progress: &Progress,
) -> Result<RefReport, String> {
    let reference = format!("refs/heads/{}", prepared.name);
    let previous = prepared
        .previous_tip
        .as_ref()
        .ok_or("Branch did not previously exist")?;
    if !prepared.overwrite || prepared.checkout || prepared.symbolic_head == reference {
        return Err("This operation requires a different rollback mode".into());
    }
    let repo = prepared.repository();
    let _guard = repo.operation_guard(token)?;
    if repo.editor_directory(token)? != prepared.directory {
        return Err("Repository changed; branch restoration was not performed".into());
    }
    let state = repo.operation_state(token)?;
    if state.in_progress() || !state.conflicts.is_empty() {
        return Err("Finish the current Git operation before restoring the branch".into());
    }
    if super::resolve(&repo, "HEAD", token)? != prepared.head
        || repo
            .text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?
            .trim()
            != prepared.symbolic_head
    {
        return Err("HEAD changed; branch restoration was not performed".into());
    }
    let worktrees = repo.text(&["worktree", "list", "--porcelain", "-z"], token)?;
    if worktrees
        .split('\0')
        .any(|field| field.strip_prefix("branch ") == Some(reference.as_str()))
    {
        return Err("This branch is checked out in a worktree".into());
    }
    if super::resolve(&repo, &reference, token)? != prepared.target {
        return Err("Overwritten branch changed; restoration was not performed".into());
    }
    let command = crate::operation_process::run(
        repo.operation_command().args([
            "update-ref",
            "-m",
            "Plico: rollback overwritten branch",
            &reference,
            previous,
            &prepared.target,
        ]),
        token,
        progress,
        None,
    )
    .map_err(|error| error.to_string());
    let actual = super::resolve(&repo, &reference, &Cancel::default());
    Ok(RefReport {
        restored: actual.as_ref().is_ok_and(|tip| tip == previous),
        refresh_error: actual.err(),
        command,
    })
}
