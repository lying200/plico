//! Branch creation: Rebased 4aec352 GitNewBranchDialog and
//! GitCheckoutNewBranchOperation (JetBrains and contributors, Apache-2.0).
pub mod batch;
pub mod rollback;

use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
};

/// A displayed starting commit and HEAD identity, rechecked before writing.
pub struct Prepared {
    repository: Repository,
    directory: std::path::PathBuf,
    name: String,
    source: String,
    target: String,
    head: String,
    symbolic_head: String,
    checkout: bool,
    overwrite: bool,
    previous_tip: Option<String>,
    pub displaced_commits: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Create,
    Discard,
    KeepLocal,
    Rebase,
}
impl Prepared {
    /// The worktree captured when this operation was prepared.
    pub fn repository(&self) -> Repository {
        self.repository.clone()
    }
    pub fn default_choice(&self) -> Choice {
        if self.previous_tip.is_some() && !self.overwrite && !self.checkout {
            Choice::KeepLocal
        } else {
            Choice::Create
        }
    }
    pub fn can_rebase(&self) -> bool {
        self.checkout && self.previous_tip.is_some() && !self.displaced_commits.is_empty()
    }
    pub fn overwrites(&self) -> bool {
        self.overwrite
    }

    pub fn matches_displayed(&self, head: &str, target: &str) -> bool {
        self.head == head && self.target == target
    }

    pub fn read(
        repo: &Repository,
        name: &str,
        source: &str,
        checkout: bool,
        token: &Cancel,
    ) -> Result<Self, String> {
        let result = Self::read_options(repo, name, source, checkout, false, token)?;
        if result.previous_tip.is_some() {
            return Err("A branch with this name already exists".into());
        }
        Ok(result)
    }

    pub fn read_options(
        repo: &Repository,
        name: &str,
        source: &str,
        checkout: bool,
        overwrite: bool,
        token: &Cancel,
    ) -> Result<Self, String> {
        // Do not accept revision shortcuts such as @{-1} as a branch name.
        if name.is_empty()
            || name.trim() != name
            || name.starts_with('-')
            || name.eq_ignore_ascii_case("HEAD")
        {
            return Err("Invalid branch name".into());
        }
        let reference = format!("refs/heads/{name}");
        repo.text(&["check-ref-format", &reference], token)?;
        let existing = repo.text(
            &["for-each-ref", "--format=%(refname)", "refs/heads/"],
            token,
        )?;
        if existing.lines().any(|r| {
            r.starts_with(&format!("{reference}/")) || reference.starts_with(&format!("{r}/"))
        }) {
            return Err("A branch or branch directory with this name already exists".into());
        }
        let remotes = repo.text(
            &["for-each-ref", "--format=%(refname)", "refs/remotes/"],
            token,
        )?;
        if remotes
            .lines()
            .any(|r| r.strip_prefix("refs/remotes/") == Some(name))
        {
            return Err("Branch name clashes with a remote branch".into());
        }
        let target = resolve(repo, source, token)?;
        let head = resolve(repo, "HEAD", token)?;
        let symbolic_head = repo
            .text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?
            .trim()
            .to_owned();
        let previous_tip = existing
            .lines()
            .any(|r| r == reference)
            .then(|| resolve(repo, &reference, token))
            .transpose()?;
        if previous_tip.is_some() {
            let worktrees = repo.text(&["worktree", "list", "--porcelain", "-z"], token)?;
            let occupied = worktrees
                .split('\0')
                .filter(|v| v.strip_prefix("branch ") == Some(reference.as_str()))
                .count();
            if occupied > usize::from(symbolic_head == reference) {
                return Err("This branch is checked out in another worktree".into());
            }
        }
        let displaced_commits = if let Some(tip) = &previous_tip {
            repo.text(
                &[
                    "log",
                    "--max-count=101",
                    "--format=%h %s",
                    &format!("{target}..{tip}"),
                    "--",
                ],
                token,
            )?
            .lines()
            .map(str::to_owned)
            .collect()
        } else {
            Vec::new()
        };
        let state = repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {
            return Err("Finish the current Git operation before creating a branch".into());
        }
        Ok(Self {
            repository: repo.clone(),
            directory: repo.editor_directory(token)?,
            name: name.into(),
            source: source.into(),
            target,
            head,
            symbolic_head,
            checkout,
            overwrite,
            previous_tip,
            displaced_commits,
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
    .map(|s| s.trim().to_owned())
}
pub struct Report {
    pub command: Result<Outcome, String>,
    pub verified: bool,
    pub refresh_error: Option<String>,
    pub state: Option<crate::operation_state::State>,
}
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    execute_confirmed(repo, prepared, false, token, progress)
}

/// `discard_commits` must only follow explicit confirmation of the prepared list.
pub fn execute_confirmed(
    repo: &Repository,
    prepared: &Prepared,
    discard_commits: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    execute_choice(
        repo,
        prepared,
        if discard_commits {
            Choice::Discard
        } else {
            Choice::Create
        },
        token,
        progress,
    )
}

pub fn execute_choice(
    repo: &Repository,
    prepared: &Prepared,
    choice: Choice,
    token: &Cancel,
    progress: &Progress,
) -> Result<Report, String> {
    let mut session = crate::update_session::Session::begin(repo, token)?;
    if repo.editor_directory(token)? != prepared.directory {
        return Err("Repository changed; review the branch starting point again".into());
    }
    let current = Prepared::read_options(
        repo,
        &prepared.name,
        &prepared.source,
        prepared.checkout,
        prepared.overwrite,
        token,
    )?;
    if current.head != prepared.head
        || current.symbolic_head != prepared.symbolic_head
        || current.target != prepared.target
        || current.previous_tip != prepared.previous_tip
    {
        return Err("Repository changed; review the branch starting point again".into());
    }
    if !prepared.displaced_commits.is_empty() && choice == Choice::Create {
        return Err("Branch has additional commits; explicit confirmation is required".into());
    }
    match choice {
        Choice::Discard if !prepared.overwrite => return Err("Overwrite was not selected".into()),
        Choice::KeepLocal if prepared.previous_tip.is_none() => {
            return Err("Local branch no longer exists".into());
        }
        Choice::Rebase if !prepared.can_rebase() => {
            return Err("Invalid checkout rebase choice".into());
        }
        _ => {}
    }
    let reference = format!("refs/heads/{}", prepared.name);
    // Git cannot branch -f the current branch. Like Rebased, use transactional
    // checkout -B for that case even when the Checkout checkbox is cleared.
    let checkout = prepared.checkout
        || (prepared.previous_tip.is_some() && prepared.symbolic_head == reference);
    let mut command = repo.operation_command();
    // checkout -b is transactional for overwrite failures: do not create first
    // and then attempt a separate checkout that could leave an unexpected ref.
    if choice == Choice::Rebase {
        command.env("GIT_EDITOR", "true").args([
            "rebase",
            "--no-autostash",
            "--merge",
            &prepared.target,
            &prepared.name,
        ]);
    } else if choice == Choice::KeepLocal {
        if prepared.checkout {
            command.args(["checkout", &prepared.name, "--"]);
        } else {
            command.args(["rev-parse", "--verify", &reference]);
        } // existing branch, no write requested
    } else if checkout {
        command.args([
            "checkout",
            "--no-track",
            if prepared.overwrite { "-B" } else { "-b" },
            &prepared.name,
            &prepared.target,
            "--",
        ]);
    } else if prepared.overwrite {
        command.args([
            "branch",
            "--no-track",
            "-f",
            "--",
            &prepared.name,
            &prepared.target,
        ]);
    } else {
        command.args([
            "branch",
            "--no-track",
            "--",
            &prepared.name,
            &prepared.target,
        ]);
    }
    // Reuse the existing worktree-scoped stash ownership record so Continue /
    // Abort and the saved-changes UI can recover after the window is closed.
    let directory = repo.editor_directory(token)?;
    let dirty = choice == Choice::Rebase
        && !repo
            .text(&["status", "--porcelain", "--untracked-files=no"], token)?
            .is_empty();
    if choice == Choice::Rebase && crate::rebase_saved::pending(repo, token)?.is_some() {
        return Err("Resolve the previously saved local changes before rebasing".into());
    }
    let save = dirty.then(|| session.save("Plico: before branch rebase", token, progress));
    let saved_ok = save.as_ref().is_none_or(|result| {
        result
            .as_ref()
            .is_ok_and(|o| o.status.success() && o.stopped.is_none())
    });
    let record = session
        .saved_oid()
        .map(|stash| crate::rebase_saved::Record {
            head: prepared
                .previous_tip
                .clone()
                .expect("rebase has an existing local branch"),
            base: Some(prepared.target.clone()),
            stash: stash.to_owned(),
            branch: Some(reference.clone()),
        });
    let mut recovery_error = record
        .as_ref()
        .and_then(|r| crate::rebase_saved::create(&directory, r).err());
    let command = if !saved_ok {
        Err("Could not save local changes; rebase was not started. Inspect the stash and working tree".into())
    } else if recovery_error.is_some() {
        Err("Could not record saved local changes; rebase was not started".into())
    } else {
        // Saving can take time; revalidate before allowing a branch rewrite.
        Prepared::read_options(
            repo,
            &prepared.name,
            &prepared.source,
            prepared.checkout,
            prepared.overwrite,
            token,
        )
        .and_then(|now| {
            if now.head != prepared.head
                || now.symbolic_head != prepared.symbolic_head
                || now.previous_tip != prepared.previous_tip
                || now.target != prepared.target
            {
                return Err("Repository changed after saving local changes".into());
            }
            operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string())
        })
    };
    // Cancellation is not rollback: inspect actual state with a fresh token.
    let refresh = Cancel::default();
    if record.is_some() && saved_ok && recovery_error.is_none() {
        let can_restore = (|| -> Result<bool, String> {
            let state = repo.operation_state(&refresh)?;
            if state.in_progress() || !state.conflicts.is_empty() {
                return Ok(false);
            }
            let head = resolve(repo, "HEAD", &refresh)?;
            let symbolic = repo.text(&["rev-parse", "--symbolic-full-name", "HEAD"], &refresh)?;
            let not_started = head == prepared.head && symbolic.trim() == prepared.symbolic_head;
            let completed = command
                .as_ref()
                .is_ok_and(|o| o.status.success() && o.stopped.is_none())
                && symbolic.trim() == reference;
            Ok(not_started || completed)
        })();
        match can_restore {
            Ok(true) => {
                recovery_error = match session.restore_saved(true, progress) {
                    Ok(Some(outcome)) if !outcome.status.success() || outcome.stopped.is_some() => Some("Local changes could not be restored; inspect the retained stash and working tree".into()),
                    Err(error) => Some(error),
                    _ => None,
                };
                if session.saved_oid().is_none() {
                    recovery_error = recovery_error.or_else(|| {
                        crate::rebase_saved::remove(&directory, record.as_ref().unwrap()).err()
                    });
                }
            }
            Err(error) => recovery_error = Some(error),
            Ok(false) => {}
        }
    }
    if recovery_error.is_none() {
        if let Some(stash) = session.saved_oid() {
            recovery_error = Some(format!(
                "Local changes remain saved in stash {stash}; finish or abort the rebase before restoring them"
            ));
        }
    }
    let state = repo.operation_state(&refresh);
    let checked = (|| -> Result<bool, String> {
        let reference = format!("refs/heads/{}", prepared.name);
        let tip = resolve(repo, &reference, &refresh)?;
        let head = resolve(repo, "HEAD", &refresh)?;
        let symbolic = repo.text(&["rev-parse", "--symbolic-full-name", "HEAD"], &refresh)?;
        let expected = if choice == Choice::KeepLocal {
            prepared.previous_tip.as_ref().unwrap()
        } else {
            &prepared.target
        };
        let target_matches = if choice == Choice::Rebase {
            repo.text(
                &[
                    "rev-list",
                    "--max-count=1",
                    &format!("{head}..{}", prepared.target),
                    "--",
                ],
                &refresh,
            )?
            .trim()
            .is_empty()
        } else {
            &tip == expected
        };
        let attached = if choice == Choice::KeepLocal && !prepared.checkout {
            head == prepared.head && symbolic.trim() == prepared.symbolic_head
        } else if checkout {
            head == tip && symbolic.trim() == reference
        } else {
            head == prepared.head && symbolic.trim() == prepared.symbolic_head
        };
        Ok(target_matches
            && attached
            && state
                .as_ref()
                .is_ok_and(|s| !s.in_progress() && s.conflicts.is_empty()))
    })();
    let verified = recovery_error.is_none()
        && session.saved_oid().is_none()
        && command
            .as_ref()
            .is_ok_and(|c| c.status.success() && c.stopped.is_none())
        && checked.as_ref().is_ok_and(|v| *v);
    Ok(Report {
        command,
        verified,
        refresh_error: recovery_error
            .or_else(|| checked.err().or_else(|| state.as_ref().err().cloned())),
        state: state.ok(),
    })
}

/// GitRefNameValidator.cleanUpBranchNameOnTyping, translated from Rebased
/// 4aec352 (JetBrains and contributors, Apache-2.0). Keep a trailing slash
/// while typing so nested branch names can still be entered.
pub fn clean_name_on_typing(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut dropped = String::with_capacity(name.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '@' && next == Some('{') {
            i += 2;
            continue;
        }
        let invalid = (i == 0 && matches!(c, '.' | '-' | '/'))
            || matches!(c, '~' | ':' | '^' | '?' | '*' | '"' | '[' | '\\')
            || (c == '/' && next == Some('/'))
            || (c == '.' && (matches!(next, Some('.' | '/')) || (i > 0 && chars[i - 1] == '/')));
        if !invalid {
            dropped.push(c);
        }
        i += 1;
    }
    let mut result = String::with_capacity(dropped.len());
    let mut space = false;
    for c in dropped.chars() {
        if c == ' ' {
            if !space {
                result.push('-');
            }
            space = true;
        } else {
            space = false;
            result.push(if c <= '\u{1f}' || c == '\u{7f}' {
                '-'
            } else {
                c
            });
        }
    }
    result
}

pub fn clean_name(name: &str) -> String {
    let mut name = clean_name_on_typing(name);
    if name.ends_with(".lock") {
        name.truncate(name.len() - 5);
    } else if name.ends_with(['.', '/']) {
        name.pop();
    }
    name
}

#[cfg(test)]
mod name_tests {
    use super::*;
    #[test]
    fn cleans_typing_without_removing_directory_separator() {
        for (input, expected) in [
            ("task  name", "task-name"),
            ("task\t\nname", "task--name"),
            ("/task//.fix..bug/", "task/fix.bug/"),
            ("-task~:^?*\"[\\@{fix", "taskfix"),
            ("功能/修复", "功能/修复"),
            ("task.lock", "task.lock"),
        ] {
            assert_eq!(clean_name_on_typing(input), expected, "{input:?}");
        }
        assert_eq!(clean_name("task.lock"), "task");
        assert_eq!(clean_name("task/"), "task");
        assert_eq!(clean_name("task."), "task");
        assert_eq!(clean_name("HEAD"), "HEAD"); // validation, not cleanup
    }
}
