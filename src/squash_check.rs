//! Read-only Squash preparation following Rebased GitCommitEditingActionBase
//! and GitProtectedBranches (4aec352, JetBrains, Apache-2.0).
use crate::{git::{Cancel, Repository, HISTORY_MAX, is_oid}, published_commits::{Protection, default_protected},
    operation_state::State, squash_plan::Plan};
use std::collections::HashSet;

/// A snapshot for the message dialog, never a durable authorization to write.
/// The executor must prepare again under its operation guard immediately before
/// starting Git and compare HEAD/branch/range to the dialog's original snapshot.
#[derive(Debug)]
pub struct Prepared {
    pub head: String,
    /// Full symbolic ref, or None for detached HEAD (not rejected by this gate).
    pub branch: Option<String>,
    pub base: String,
    pub selected_oldest_first: Vec<String>,
    pub plan: Plan,
    /// Observe ongoing operations separately; this read-only gate does not claim
    /// the upstream default Allow policy means starting another rebase is safe.
    pub state: State,
}

pub fn prepare(repo: &Repository, expected_head: &str, selected: &[String], protection: Protection<'_>, token: &Cancel)
    -> Result<Prepared, String> {
    let mut missing = selected.iter().map(String::as_str).collect::<HashSet<_>>();
    if !is_oid(expected_head) || selected.len() < 2 || selected.len() > HISTORY_MAX
        || missing.len() != selected.len() || selected.iter().any(|id| !is_oid(id)) {
        return Err("Squash requires at least two distinct full commit IDs".into());
    }
    let head = crate::commit_operation::head(repo, token)?.ok_or("There is no HEAD to squash")?;
    if head != expected_head { return Err("HEAD changed; refresh before squashing".into()); }
    let refs = inventory(repo, token)?;
    let branch = symbolic_head(repo, token)?;
    let shallow = repo.shallow_boundaries(token)?;
    let max = format!("--max-count={}", HISTORY_MAX + 1);
    let history = repo.text(&["rev-list", "--first-parent", "--parents", &max, &head, "--"], token)?;
    let mut replay = Vec::new();
    let mut base = None;
    for line in history.lines() {
        if token.cancelled() { return Err("Cancelled".into()); }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.is_empty() || fields.iter().any(|id| !is_oid(id)) { return Err("Invalid Squash history record".into()); }
        if shallow.1.contains(fields[0]) { return Err("Squash path reaches a shallow boundary".into()); }
        // Match upstream checkHeadLinearHistory: check parents before removing
        // the selected node, so a selected root is not silently accepted.
        if fields.len() != 2 { return Err(format!("Squash HEAD path contains root or merge commit {}", fields[0])); }
        if replay.len() == HISTORY_MAX { return Err("Squash HEAD path exceeds history limit".into()); }
        replay.push(fields[0].to_owned());
        missing.remove(fields[0]);
        if missing.is_empty() { base = Some(fields[1].to_owned()); break; }
    }
    let base = base.ok_or("Selected commits are not all on the linear HEAD path")?;
    replay.reverse();
    let selected_set = selected.iter().map(String::as_str).collect::<HashSet<_>>();
    let selected_oldest_first = replay.iter().filter(|id| selected_set.contains(id.as_str())).cloned().collect::<Vec<_>>();
    let oldest = &selected_oldest_first[0];
    if let Some(name) = protected_containing(repo, oldest, protection, token)? {
        return Err(format!("Selected commits are published to protected branch {name}"));
    }
    let state = repo.operation_state(token)?;
    if crate::commit_operation::head(repo, token)?.as_deref() != Some(&head)
        || symbolic_head(repo, token)? != branch || inventory(repo, token)? != refs
        || repo.shallow_boundaries(token)? != shallow {
        return Err("Repository references changed during Squash preparation; refresh".into());
    }
    let plan = Plan::new(&replay, selected)?;
    Ok(Prepared { head, branch, base, selected_oldest_first, plan, state })
}
/// Read-only confirmation check, also repeated by the executor under its queue.
/// A successful result is a snapshot, never permission to skip write-time checks.
pub fn revalidate(repo: &Repository, expected: &Prepared, protection: Protection<'_>, token: &Cancel)
    -> Result<Prepared, String> {
    let checked = prepare(repo, &expected.head, &expected.selected_oldest_first, protection, token)?;
    if checked.base != expected.base || checked.branch != expected.branch || checked.plan.entries() != expected.plan.entries() {
        return Err("Squash selection changed; reopen the message dialog".into());
    }
    if checked.state.in_progress() || !checked.state.conflicts.is_empty() {
        return Err("Finish the current Git operation before starting Squash".into());
    }
    let gitdir = repo.editor_directory(token)?;
    if crate::commit_recovery::pending(&gitdir)?.is_some() {
        return Err("Resolve pending index recovery before Squash".into());
    }
    if crate::squash_saved::read(&gitdir)?.is_some() {
        return Err("Resolve previously saved Squash changes before starting another Squash".into());
    }
    Ok(checked)
}

pub(crate) fn inventory(repo: &Repository, token: &Cancel) -> Result<String, String> {
    repo.text(&["for-each-ref", "--format=%(refname)%00%(objectname)%00%(symref)", "refs/heads/", "refs/remotes/"], token)
}
pub(crate) fn symbolic_head(repo: &Repository, token: &Cancel) -> Result<Option<String>, String> {
    use crate::operation_process::{self, Progress};
    let out = operation_process::run(repo.command().args(["symbolic-ref", "--quiet", "HEAD"]), token,
        &Progress::default(), Some(std::time::Duration::from_secs(30))).map_err(|e| e.to_string())?;
    if out.stopped.is_some() || out.stdout.truncated { return Err("HEAD branch query interrupted".into()); }
    if out.status.code() == Some(1) { return Ok(None); }
    if !out.status.success() { return Err("Cannot determine HEAD branch".into()); }
    let bytes = out.stdout.bytes();
    let name = std::str::from_utf8(&bytes).map_err(|_| "Invalid HEAD branch")?.trim_end_matches(['\r','\n']);
    if !name.starts_with("refs/heads/") { return Err("Unexpected HEAD symbolic reference".into()); }
    Ok(Some(name.into()))
}

pub(crate) fn protected_containing(repo: &Repository, id: &str, protection: Protection<'_>, token: &Cancel) -> Result<Option<String>, String> {
    let containing = repo.remote_branches_containing(id, token)?;
    let protected = match protection {
        Protection::Defaults => containing.iter().find(|target| default_protected(target)),
        Protection::Explicit(targets) => targets.iter().find(|target| containing.contains(target)),
    };
    Ok(protected.map(|target| format!("{}/{}", target.remote, target.branch)))
}

/// Load only the selected messages, in Log selection order, for the dialog.
/// This draft is not authorization: execute repeats prepare under the queue.
pub fn message(repo: &Repository, selected: &[String], token: &Cancel) -> Result<String, String> {
    if selected.len() < 2 || selected.len() > HISTORY_MAX || selected.iter().any(|id| !is_oid(id)) {
        return Err("Invalid Squash message selection".into());
    }
    let mut messages = Vec::new(); let mut total = 0usize;
    for oid in selected {
        let raw = repo.text(&["show", "--no-patch", "--no-show-signature", "--encoding=UTF-8", "--format=%B", oid, "--"], token)?;
        let message = raw.strip_suffix('\n').unwrap_or(&raw).to_owned();
        total = total.checked_add(message.len()).ok_or("Squash messages exceed preview limit")?;
        if total > crate::git::OUTPUT_LIMIT { return Err("Squash messages exceed preview limit".into()); }
        messages.push(message);
    }
    let draft = crate::squash_plan::draft(&messages)?;
    if draft.len() > 1024 * 1024 { return Err("Squash message exceeds editor limit".into()); }
    Ok(draft)
}
