//! Commit-message editing: Rebased GitRewordAction and in-memory reword flow.
//! Objects are prepared first; only a compare-and-swap reference update publishes them.
use crate::{
    git::{Cancel, HISTORY_MAX, Repository, is_oid},
    operation_process::{self, Progress},
    operation_state::Operation,
    published_commits::Protection,
    squash_check,
};
use std::{collections::HashMap, path::PathBuf, process::Command};

#[derive(Debug)]
pub struct Prepared {
    pub head: String,
    pub target: String,
    pub message: String,
    pub author: String,
    pub initial: bool,
    branch: Option<String>,
    directory: PathBuf,
    repository: PathBuf,
}
pub struct Completed {
    pub target: String,
    pub warning: Option<String>,
}
impl Prepared {
    /// Keep the operation's repository attached to the prepared action, including
    /// while dialogs or asynchronous checks outlive the current Log selection.
    pub fn repository(&self) -> Repository {
        Repository::new(&self.repository)
    }
    pub fn prepare(repo: &Repository, target: String, token: &Cancel) -> Result<Self, String> {
        if !is_oid(&target) {
            return Err("Invalid commit".into());
        }
        let head = crate::commit_operation::head(repo, token)?.ok_or("Cannot find HEAD")?;
        let branch = squash_check::symbolic_head(repo, token)?;
        let directory = repo.editor_directory(token)?;
        let raw = repo.text(
            &[
                "show",
                "-s",
                "--no-show-signature",
                "--encoding=UTF-8",
                "--format=%P%x00%an%x00%B",
                &target,
                "--",
            ],
            token,
        )?;
        let fields: Vec<_> = raw.splitn(3, '\0').collect();
        if fields.len() != 3 {
            return Err("Invalid commit metadata".into());
        }
        if target != head && fields[0].split_whitespace().count() > 1 {
            return Err("Merge commits can only be edited at HEAD".into());
        }
        let value = Self {
            head,
            target,
            branch,
            directory,
            repository: repo.path.clone(),
            initial: fields[0].trim().is_empty(),
            author: fields[1].into(),
            message: fields[2].strip_suffix('\n').unwrap_or(fields[2]).into(),
        };
        if value.message.len() > 1024 * 1024 {
            return Err("Commit message exceeds editor limit".into());
        }
        value.validate(repo, token)?;
        Ok(value)
    }
    pub fn validate(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        if repo.editor_directory(token)? != self.directory
            || crate::commit_operation::head(repo, token)?.as_deref() != Some(&self.head)
            || squash_check::symbolic_head(repo, token)? != self.branch
        {
            return Err("Cannot edit selected commits: repository state was changed".into());
        }
        let ancestry = operation_process::run(
            repo.command()
                .args(["merge-base", "--is-ancestor", &self.target, &self.head]),
            token,
            &Progress::default(),
            Some(std::time::Duration::from_secs(30)),
        )
        .map_err(|e| e.to_string())?;
        if ancestry.stopped.is_some() || !ancestry.status.success() {
            return Err("Commit does not belong to HEAD history".into());
        }
        if let Some(branch) =
            squash_check::protected_containing(repo, &self.target, Protection::Defaults, token)?
        {
            return Err(format!(
                "Cannot edit selected commits: pushed to protected branch: {branch}"
            ));
        }
        let state = repo.operation_state(token)?;
        let head_during_rebase = self.target == self.head
            && matches!(
                state.operation,
                Operation::RebaseApply | Operation::RebaseMerge
            );
        if !state.conflicts.is_empty() || (state.in_progress() && !head_during_rebase) {
            return Err(
                "Cannot edit selected commits: another Git operation is in progress".into(),
            );
        }
        if crate::commit_recovery::pending(&self.directory)?.is_some()
            || crate::rebase_saved::read(&self.directory)?.is_some()
        {
            return Err(
                "Cannot edit selected commits: recover the previous operation first".into(),
            );
        }
        Ok(())
    }
    pub fn execute(
        &self,
        repo: &Repository,
        message: &str,
        token: &Cancel,
        progress: &Progress,
    ) -> Result<Completed, String> {
        if message.trim().is_empty() || message.len() > 1024 * 1024 || message.contains('\0') {
            return Err("Commit message must be nonempty and at most 1 MiB".into());
        }
        let _guard = repo.operation_guard(token)?;
        self.validate(repo, token)?;
        let encoding = repo.text(
            &[
                "config",
                "--get",
                "--default",
                "UTF-8",
                "i18n.commitEncoding",
            ],
            token,
        )?;
        if !matches!(
            encoding.trim().to_ascii_lowercase().as_str(),
            "utf8" | "utf-8"
        ) {
            return Err("Commit message encoding is not yet supported".into());
        }
        let cleanup = repo.text(
            &[
                "config",
                "--get",
                "--default",
                "whitespace",
                "commit.cleanup",
            ],
            token,
        )?;
        let cleaned = if cleanup.trim() == "verbatim" {
            message.to_owned()
        } else {
            let mut cmd = repo.command();
            cmd.arg("stripspace");
            if cleanup.trim() == "strip" {
                cmd.arg("--strip-comments");
            }
            output(&mut cmd, message.as_bytes(), token, progress)?
        };
        if cleaned.trim().is_empty() {
            return Err("Commit message is empty after cleanup".into());
        }
        // Only descendants of the target need new IDs. Preserve every tree and
        // parent edge, including merges above the selected non-merge commit.
        let range = format!("{}..{}", self.target, self.head);
        let max = format!("--max-count={}", HISTORY_MAX + 1);
        let descendants = repo.text(
            &[
                "rev-list",
                "--reverse",
                "--topo-order",
                "--ancestry-path",
                &max,
                &range,
                "--",
            ],
            token,
        )?;
        let mut ids = vec![self.target.clone()];
        ids.extend(descendants.lines().map(str::to_owned));
        if ids.len() > HISTORY_MAX || ids.iter().any(|s| !is_oid(s)) {
            return Err("Commit rewrite exceeds history limit".into());
        }
        let shallow = repo.shallow_boundaries(token)?;
        if ids.iter().any(|id| shallow.1.contains(id)) {
            return Err("Cannot rewrite a shallow history boundary".into());
        }
        let sign = repo
            .text(
                &[
                    "config",
                    "--type=bool",
                    "--get",
                    "--default",
                    "false",
                    "commit.gpgSign",
                ],
                token,
            )?
            .trim()
            == "true";
        let mut mapped = HashMap::<String, String>::new();
        let mut mappings = String::new();
        for id in &ids {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let raw = repo.text(
                &[
                    "show",
                    "-s",
                    "--no-show-signature",
                    "--encoding=UTF-8",
                    "--format=%T%x00%P%x00%an%x00%ae%x00%aI%x00%B",
                    id,
                    "--",
                ],
                token,
            )?;
            let f: Vec<_> = raw.splitn(6, '\0').collect();
            if f.len() != 6 || !is_oid(f[0]) {
                return Err("Invalid rewrite metadata".into());
            }
            let mut cmd = repo.operation_command();
            cmd.args(["commit-tree", f[0]]);
            for parent in f[1].split_whitespace() {
                if !is_oid(parent) {
                    return Err("Invalid commit parent".into());
                }
                cmd.args([
                    "-p",
                    mapped.get(parent).map(String::as_str).unwrap_or(parent),
                ]);
            }
            if sign {
                cmd.arg("-S");
            }
            cmd.args(["-F", "-"])
                .env("GIT_AUTHOR_NAME", f[2])
                .env("GIT_AUTHOR_EMAIL", f[3])
                .env("GIT_AUTHOR_DATE", f[4]);
            let body = if id == &self.target {
                cleaned.as_str()
            } else {
                f[5].strip_suffix('\n').unwrap_or(f[5])
            };
            let new = output(&mut cmd, body.as_bytes(), token, progress)?
                .trim()
                .to_owned();
            if !is_oid(&new) {
                return Err("Invalid rewritten commit ID".into());
            }
            mappings.push_str(&format!("{id} {new}\n"));
            mapped.insert(id.clone(), new);
        }
        self.validate(repo, token)?;
        let new_head = mapped.get(&self.head).ok_or("Rewrite did not reach HEAD")?;
        // Cancellation ends before the atomic publication. No reset, stash, index
        // update or checkout is needed because all trees are unchanged.
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let completion = Cancel::default();
        let reference = self.branch.as_deref().unwrap_or("HEAD");
        let mut update = repo.operation_command();
        update.args([
            "update-ref",
            "--no-deref",
            "-m",
            "reword: edit commit message",
            reference,
            new_head,
            &self.head,
        ]);
        output(&mut update, b"", &completion, progress)?;
        let mut warnings = Vec::new();
        if let Err(e) = output(
            repo.operation_command()
                .args(["notes", "copy", "--for-rewrite=rebase"]),
            mappings.as_bytes(),
            &completion,
            progress,
        ) {
            warnings.push(e);
        }
        // Use Git's hook runner (respects core.hooksPath); unsupported older Git
        // reports a post-publication warning rather than retrying the rewrite.
        let path = self.directory.join(format!(
            "plico-post-rewrite-{}-{}",
            std::process::id(),
            new_head
        ));
        let mut created = false;
        let hook = (|| -> Result<(), String> {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| e.to_string())?;
            created = true;
            file.write_all(mappings.as_bytes())
                .map_err(|e| e.to_string())?;
            drop(file);
            let mut cmd = repo.operation_command();
            cmd.args(["hook", "run", "--ignore-missing", "--to-stdin"])
                .arg(&path)
                .args(["post-rewrite", "--", "rebase"]);
            output(&mut cmd, b"", &completion, progress).map(|_| ())
        })();
        if created {
            let _ = std::fs::remove_file(&path);
        }
        if let Err(e) = hook {
            warnings.push(e);
        }
        Ok(Completed {
            target: mapped[&self.target].clone(),
            warning: (!warnings.is_empty()).then(|| warnings.join("\n")),
        })
    }
}
fn output(
    cmd: &mut Command,
    input: &[u8],
    token: &Cancel,
    progress: &Progress,
) -> Result<String, String> {
    let out = operation_process::run_with_input(cmd, token, progress, None, input)
        .map_err(|e| e.to_string())?;
    if out.stopped.is_some()
        || !out.status.success()
        || out.input_error.is_some()
        || out.stdout.truncated
    {
        return Err(format!(
            "Git message rewrite failed: {}",
            String::from_utf8_lossy(&out.stderr.bytes())
        ));
    }
    String::from_utf8(out.stdout.bytes()).map_err(|_| "Invalid Git output".into())
}
