//! Read-only Git access. Commands are bounded, cancellable, and never invoke external diffs.
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, String>;
pub const HISTORY_PAGE: usize = 1000;
pub const HISTORY_MAX: usize = 10_000;
pub const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
/// A raw Git author identity (before mailmap substitution).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Author {
    pub name: String,
    pub email: String,
}
impl Author {
    pub fn exact_name(&self) -> String {
        match (self.name.is_empty(), self.email.is_empty()) {
            (true, _) => self.email.clone(),
            (_, true) => self.name.clone(),
            _ => format!("{} <{}>", self.name, self.email),
        }
    }
}

/// Rebased GitLogProvider text, date, resolved author and structure filters. Hash lookup has priority.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogFilter {
    pub text: String,
    pub regex: bool,
    pub match_case: bool,
    /// Fixed Unix-second bounds; shared by metadata and graph queries.
    pub after: Option<i64>,
    pub before: Option<i64>,
    /// None means no User filter; Some(empty) means no resolved identities.
    pub authors: Option<Vec<Author>>,
    /// Literal repository-relative paths. Multiple entries form a union.
    pub paths: Vec<String>,
}
impl LogFilter {
    /// Rebased fromHash: every token must be a 7–64 digit hex prefix.
    pub fn hashes(&self) -> Option<Vec<String>> {
        let words: Vec<_> = self.text.split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|s| !s.is_empty()).collect();
        if words.is_empty() || words.iter().any(|s| !(7..=64).contains(&s.len()) || !s.bytes().all(|b| b.is_ascii_hexdigit())) {
            return None;
        }
        Some(words.into_iter().map(str::to_ascii_lowercase).collect())
    }
    fn arguments(&self) -> Vec<String> {
        let mut args = Vec::new();
        if !self.paths.is_empty() { args.extend(["--full-history".into(), "--simplify-merges".into()]); }
        if let Some(after) = self.after { args.push(format!("--after=@{after}")); }
        if let Some(before) = self.before { args.push(format!("--before=@{before}")); }
        let has_text = !self.text.trim().is_empty();
        if !has_text && self.authors.is_none() { return args; }
        // Git shares regexp/case flags between --grep and --author, as does
        // Rebased GitLogProvider. Without TEXT_FILTER it uses extended regex.
        let regexp = !has_text || self.regex;
        if has_text { args.push(format!("--grep={}", self.text)); }
        args.push(if regexp {
                "--extended-regexp"
            } else {
                "--fixed-strings"
            }
            .into());
        if !has_text || !self.match_case {
            args.push("--regexp-ignore-case".into());
        }
        if let Some(authors) = &self.authors {
            for author in authors {
                let exact = author.exact_name();
                let pattern = if regexp {
                    // UserNameRegex.EXTENDED_INSTANCE: anchor the whole identity
                    // and escape user data, never interpret an author's regexp.
                    let mut pattern = String::from("^");
                    for ch in exact.chars() {
                        if "\\.^$*+-?()[]{}|".contains(ch) { pattern.push('\\'); }
                        pattern.push(ch);
                    }
                    pattern.push('$');
                    pattern
                } else { exact };
                args.push(format!("--author={pattern}"));
            }
        }
        args
    }

    pub(crate) fn validate_paths(&self) -> Result<()> {
        if self.paths.len() > 128 || self.paths.iter().map(|p| p.encode_utf16().count() + 3).sum::<usize>() > 8192 {
            return Err("Paths filter exceeds 128 paths / 8192 UTF-16 units".into());
        }
        for path in &self.paths {
            if path == "." { continue; }
            if path.is_empty() || path.contains(['\0', '\\', ':']) || path.starts_with('/')
                || path.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
                return Err("Paths must be repository-relative paths separated by /".into());
            }
        }
        Ok(())
    }
    fn matches_no_authors(&self) -> bool { self.authors.as_ref().is_some_and(Vec::is_empty) }
}

#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);
impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
/// Graph-only data: hidden commits do not need subjects, people or refs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopologyCommit {
    pub id: String,
    pub parents: Vec<String>,
}
pub struct TopologyPage {
    pub commits: Vec<TopologyCommit>,
    pub more: bool,
}
#[derive(Clone, Debug)]
pub struct Commit {
    pub id: String,
    pub parents: Vec<String>,
    pub shallow_boundary: bool,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub refs: Vec<RefLabel>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefLabel {
    pub kind: RefKind,
    pub name: String,
}
#[derive(Clone, Debug)]
pub struct Branch {
    pub name: String,
    pub id: String,
}
#[derive(Clone, Debug)]
pub struct Tag {
    pub name: String,
    /// Peeled commit ID; tags pointing to trees or blobs are excluded.
    pub id: String,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub head_id: Option<String>,
    pub included_in_head: Arc<HashSet<String>>,
    pub current_branch: Option<String>,
    pub ref_titles: Arc<HashMap<String, String>>,
    pub commits: Vec<Commit>,
    pub branches: Vec<Branch>,
    pub tags: Vec<Tag>,
    pub tips: Vec<String>,
    pub more: bool,
    pub shallow: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Area {
    Commit,
    WorkingTree,
    Unstaged,
    Staged,
    Untracked,
    Conflict,
}
impl Area {
    pub fn label(self) -> &'static str {
        match self {
            Self::Commit => "Commit",
            Self::WorkingTree => "Changes",
            Self::Unstaged => "Unstaged",
            Self::Staged => "Staged",
            Self::Untracked => "Untracked",
            Self::Conflict => "Conflict",
        }
    }
}
/// Net HEAD-to-working-tree metadata for the non-staging Commit workflow.
/// Kept separate from index/working-tree review areas until the Commit UI migrates.
#[derive(Clone, Debug)]
pub struct WorkingTreeChange {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: String,
}
#[derive(Clone, Debug)]
pub struct Change {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: String,
    pub area: Area,
}
impl Change {
    pub fn label(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}
#[derive(Debug)]
pub struct Details {
    pub message: String,
    pub author: String,
    pub authored: String,
    pub committer: String,
    pub committed: String,
    pub base: String,
    pub files: Vec<Change>,
}
#[derive(Clone, Debug)]
pub struct Repository {
    pub path: PathBuf,
}

fn utf8(data: Vec<u8>) -> Result<String> {
    String::from_utf8(data).map_err(|_| "Non-UTF-8 text cannot be displayed safely".into())
}
pub fn is_oid(id: &str) -> bool {
    matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(crate) fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
    }
    #[cfg(not(unix))]
    {
        Ok(PathBuf::from(
            std::str::from_utf8(bytes).map_err(|_| "Non-UTF-8 path is unsupported on Windows")?,
        ))
    }
}
impl Repository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    /// Pull dialog inventory from local remote-tracking refs; no network IO.
    /// GitOutgoingCommitsProvider: existing target difference, or new target
    /// excluding this remote's refs with the upstream 1,000-commit ceiling.
    /// Existing targets are paged locally rather than retained in one large list.
    pub fn outgoing_commits(&self, source_oid: &str, target: &crate::push_target::Target,
        offset: usize, limit: usize, token: &Cancel) -> Result<crate::push_target::Outgoing> {
        self.outgoing_commits_checked(source_oid, target, offset, limit, None, token)
    }
    /// Repository-level Push preview needs the entire outgoing range, not the
    /// current UI page. Keep the same remote-ref snapshot throughout traversal.
    /// A local memory budget fails explicitly rather than returning a partial
    /// range disguised as complete. New-branch previews retain upstream's 1000
    /// commit ceiling, exposed through `capped`.
    pub fn outgoing_preview_commits(&self, source_oid: &str, target: &crate::push_target::Target,
        expected: Option<&str>, max_commits: usize, token: &Cancel) -> Result<crate::push_target::Outgoing> {
        let mut commits = Vec::new();
        let mut references: Option<std::sync::Arc<str>> = expected.map(Into::into);
        loop {
            if token.cancelled() { return Err("Cancelled".into()); }
            let remaining = max_commits.saturating_sub(commits.len());
            // Even at the budget boundary query one item, distinguishing a
            // genuinely empty range from an omitted non-empty remainder.
            let page = self.outgoing_commits_checked(source_oid, target, commits.len(),
                remaining.clamp(1, 100), references.as_deref(), token)?;
            if page.commits.len() > remaining {
                return Err("Outgoing preview exceeds the commit limit".into());
            }
            references = Some(page.references.clone());
            commits.extend(page.commits);
            if !page.more {
                return Ok(crate::push_target::Outgoing {
                    commits, more: false, capped: page.capped, references: page.references,
                });
            }
            if remaining == 0 || commits.len() >= max_commits {
                return Err("Outgoing preview exceeds the commit limit".into());
            }
        }
    }
    /// Reject pagination across changed remote refs, including changes during
    /// the log command. Retain only ref metadata, not the full outgoing history.
    pub fn outgoing_commits_checked(&self, source_oid: &str, target: &crate::push_target::Target,
        offset: usize, limit: usize, expected: Option<&str>, token: &Cancel) -> Result<crate::push_target::Outgoing> {
        let inventory = || -> Result<String> {
            let mut refs = self.text(&["for-each-ref", "--format=%(refname)%09%(objectname)%09%(symref)",
                &format!("refs/remotes/{}/", target.remote)], token)?;
            if target.special && !target.new_branch {
                refs.push_str(&self.text(&["rev-parse", "--verify", "--end-of-options",
                    &format!("{}^{{commit}}", target.branch)], token)?);
            }
            Ok(refs)
        };
        let references = inventory()?;
        if expected.is_some_and(|previous| previous != references) {
            return Err("Push references changed; reopen the push dialog".into());
        }
        if !is_oid(source_oid) { return Err("Invalid push source object ID".into()); }
        if !self.fetch_specs(token)?.iter().any(|s| s.remote() == target.remote) {
            return Err("Push remote is no longer available".into());
        }
        let resolved = self.text(&["rev-parse", "--verify", &format!("{source_oid}^{{commit}}")], token)?;
        if resolved.trim() != source_oid { return Err("Push source is not a commit".into()); }
        if target.new_branch && offset >= 1000 {
            return Ok(crate::push_target::Outgoing { commits: Vec::new(), more: false, capped: true, references: references.into() });
        }
        let limit = limit.clamp(1, 1000).min(if target.new_branch { 1000-offset } else { 1000 });
        let count = if target.new_branch { (limit+1).min(1000-offset) } else { limit+1 };
        let mut args = vec!["-c".to_owned(), "log.mailmap=false".into(), "log".into(),
            "--no-patch".into(), "--no-notes".into(), "--no-decorate".into(), "--no-show-signature".into(),
            "--encoding=UTF-8".into(), "--date=iso-strict".into(), "-z".into(),
            "--format=%H%x00%P%x00%an%x00%ad%x00%s".into(),
            format!("--max-count={count}"), format!("--skip={offset}")];
        if target.new_branch {
            args.extend([source_oid.to_owned(), "--not".into(), format!("--remotes={}", target.remote)]);
        } else {
            let reference = if target.special { target.branch.clone() }
                else { format!("refs/remotes/{}/{}", target.remote, target.branch) };
            // Resolve names before constructing a revision expression. An invalid
            // or disappeared target is an error, never an empty outgoing list.
            let base = self.text(&["rev-parse", "--verify", "--end-of-options", &format!("{reference}^{{commit}}")], token)?;
            let base = base.trim();
            if !is_oid(base) { return Err("Invalid push destination object ID".into()); }
            args.push(format!("{base}..{source_oid}"));
        }
        args.push("--".into());
        let mut commits = parse_history(&self.text(&args.iter().map(String::as_str).collect::<Vec<_>>(), token)?)?;
        let more = commits.len() > limit;
        commits.truncate(limit);
        let capped = target.new_branch && offset + commits.len() >= 1000;
        if inventory()? != references {
            return Err("Push references changed; reopen the push dialog".into());
        }
        Ok(crate::push_target::Outgoing { commits, more, capped, references: references.into() })
    }
    pub fn pull_targets(&self, token: &Cancel) -> Result<crate::pull::Targets> {
        let remotes = self.fetch_specs(token)?.into_iter().map(|s| s.remote().to_owned()).collect::<Vec<_>>();
        let refs = self.text(&["for-each-ref", "--format=%(refname)%09%(symref)", "refs/remotes/"], token)?;
        let mut branches = Vec::new();
        let mut known_refs = HashSet::new();
        for line in refs.lines() {
            let (name, symref) = line.split_once('\t').ok_or("Malformed remote branch record")?;
            if !symref.is_empty() { continue; }
            if let Some(target) = crate::pull::parse_remote_ref(name, &remotes) {
                known_refs.insert(name);
                branches.push(target);
            }
        }
        let current_branch = self.text(&["symbolic-ref", "--quiet", "--short", "HEAD"], token).ok()
            .map(|s| s.trim_end_matches(['\r', '\n']).to_owned());
        let tracked = if let Some(branch) = &current_branch {
            let reference = format!("refs/heads/{branch}");
            let upstream = self.text(&["for-each-ref", "--format=%(refname)%09%(upstream)", &reference], token)?;
            upstream.lines().filter_map(|line| line.split_once('\t'))
                .find(|(name, _)| *name == reference).and_then(|(_, upstream)| {
                    known_refs.contains(upstream).then(|| crate::pull::parse_remote_ref(upstream, &remotes)).flatten()
                })
        } else { None };
        if token.cancelled() { return Err("Cancelled".into()); }
        Ok(crate::pull::Targets { remotes, branches, current_branch, tracked })
    }
    /// Read per-worktree operation markers and unmerged index paths, without
    /// inferring conflict state from a failed command's translated stderr.
    pub fn operation_state(&self, token: &Cancel) -> Result<crate::operation_state::State> {
        use crate::operation_state::{Operation, State};
        let directory = self.read(&[OsStr::new("rev-parse"), OsStr::new("--absolute-git-dir")], token)?;
        let directory = directory.strip_suffix(b"\n").unwrap_or(&directory);
        let directory = directory.strip_suffix(b"\r").unwrap_or(directory);
        let directory = path_from_bytes(directory)?;
        let mut operation = Operation::None;
        for (marker, state) in [
            ("MERGE_HEAD", Operation::Merge), ("rebase-apply", Operation::RebaseApply),
            ("rebase-merge", Operation::RebaseMerge), ("CHERRY_PICK_HEAD", Operation::CherryPick),
            ("REVERT_HEAD", Operation::Revert), ("sequencer", Operation::Sequencer),
        ] {
            if directory.join(marker).try_exists().map_err(|e| e.to_string())? { operation = state; break; }
        }
        let rebase_dir = match operation {
            Operation::RebaseApply => Some("rebase-apply"), Operation::RebaseMerge => Some("rebase-merge"), _ => None,
        };
        let rebase_branch = if let Some(rebase_dir) = rebase_dir {
            match std::fs::File::open(directory.join(rebase_dir).join("head-name")) {
                Ok(file) => {
                    let mut value = String::new();
                    file.take(4097).read_to_string(&mut value).map_err(|e| e.to_string())?;
                    if value.len() > 4096 { return Err("Rebase branch name exceeds limit".into()); }
                    value.trim_end_matches(['\r', '\n']).strip_prefix("refs/heads/").map(str::to_owned)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.to_string()),
            }
        } else { None };
        let detached = self.text(&["symbolic-ref", "--quiet", "HEAD"], token).is_err();
        let unmerged = self.read(&[OsStr::new("ls-files"), OsStr::new("--unmerged"), OsStr::new("-z")], token)?;
        let mut conflicts = std::collections::BTreeSet::new();
        for record in unmerged.split(|byte| *byte == 0).filter(|record| !record.is_empty()) {
            let separator = record.iter().position(|byte| *byte == b'\t').ok_or("Malformed unmerged index entry")?;
            conflicts.insert(path_from_bytes(&record[separator + 1..])?);
        }
        if token.cancelled() { return Err("Cancelled".into()); }
        Ok(State { operation, detached, rebase_branch, conflicts: conflicts.into_iter().collect() })
    }
    /// Rebased updater default resolution; read errors fall back as upstream does,
    /// but cancellation must propagate instead of silently selecting merge.
    pub fn default_update_method(&self, token: &Cancel) -> Result<crate::update_method::Method> {
        let branch = self.text(&["symbolic-ref", "--quiet", "--short", "HEAD"], token).ok();
        let branch_value = branch.as_ref().and_then(|branch| {
            self.text(&["config", "--get", &format!("branch.{}.rebase", branch.trim_end_matches(['\r', '\n']))], token).ok()
        });
        let pull_value = self.text(&["config", "--get", "pull.rebase"], token).ok();
        if token.cancelled() { return Err("Cancelled".into()); }
        Ok(crate::update_method::resolve(
            branch_value.as_deref().map(|v| v.trim_end_matches(['\r', '\n'])),
            pull_value.as_deref().map(|v| v.trim_end_matches(['\r', '\n'])),
        ))
    }
    /// Inventory for explicit remote operations. No URLs, authentication or network IO.
    pub fn fetch_specs(&self, token: &Cancel) -> Result<Vec<crate::fetch::Spec>> {
        let text = self.text(&["remote"], token)?;
        let mut specs = Vec::new();
        for remote in text.lines().filter(|name| !name.is_empty()) {
            if specs.len() >= 4096 { return Err("Remote count exceeds the 4096 limit".into()); }
            specs.push(crate::fetch::Spec::new(remote.to_owned())?);
        }
        Ok(specs)
    }
    pub(crate) fn command(&self) -> Command {
        let mut c = self.operation_command();
        c.env("GIT_NO_LAZY_FETCH", "1").env("GIT_OPTIONAL_LOCKS", "0");
        c
    }
    /// Explicit operation command. System credential helpers remain configured;
    /// app-owned AskPass/authentication policy must be added before UI exposure.
    pub(crate) fn operation_command(&self) -> Command {
        let mut c = Command::new("git");
        c.args([
            "--no-pager",
            "--literal-pathspecs",
            "-c",
            "core.fsmonitor=false",
            "-C",
        ])
        .arg(&self.path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_NO_LAZY_FETCH")
        .env_remove("GIT_OPTIONAL_LOCKS");
        // The selected repository, not the launching terminal's Git scope,
        // determines every command's worktree/index/object database.
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_COMMON_DIR",
        ] {
            c.env_remove(key);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000);
        }
        c
    }
    /// Backend Fetch; callers must refresh real repository state on every outcome.
    /// This does not yet provide Rebased's app-owned HTTP/SSH authentication UI.
    pub fn fetch(&self, spec: &crate::fetch::Spec, token: &Cancel,
        progress: &crate::operation_process::Progress) -> Result<crate::operation_process::Outcome> {
        self.fetch_inner(spec, token, progress, None)
    }
    /// Explicit HTTP AskPass path. `requests` must belong to this operation/token.
    /// SSH authentication and HTTP credential reuse/UI remain separate layers.
    pub fn fetch_authenticated(&self, spec: &crate::fetch::Spec, token: &Cancel,
        progress: &crate::operation_process::Progress, requests: &crate::auth_request::Requests,
        helper: &std::path::Path) -> Result<crate::operation_process::Outcome> {
        if !helper.is_absolute() || !helper.is_file() {
            return Err("Authentication helper is unavailable".into());
        }
        self.fetch_inner(spec, token, progress, Some((requests, helper)))
    }
    fn fetch_inner(&self, spec: &crate::fetch::Spec, token: &Cancel,
        progress: &crate::operation_process::Progress,
        auth: Option<(&crate::auth_request::Requests, &std::path::Path)>) -> Result<crate::operation_process::Outcome> {
        let _guard = self.operation_guard(token)?;
        // Only configured remote names are accepted, never a caller-provided URL.
        if !self.fetch_specs(token)?.iter().any(|known| known.remote() == spec.remote()) {
            return Err("Fetch remote is no longer configured".into());
        }
        let mut command = self.operation_command();
        let server = if let Some((requests, helper)) = auth {
            let server = crate::askpass::Server::start(requests.clone()).map_err(|e| e.to_string())?;
            server.configure(&mut command);
            command.env("GIT_ASKPASS", helper).env_remove("GITUI_ASKPASS_SSH");
            Some(server)
        } else { None };
        let result = crate::operation_process::run(command.args(spec.args()?), token, progress, None)
            .map_err(|e| e.to_string());
        drop(server);
        result
    }
    pub(crate) fn operation_guard(&self, token: &Cancel) -> Result<crate::operation_queue::Guard> {
        let common = self.read(&[OsStr::new("rev-parse"), OsStr::new("--path-format=absolute"),
            OsStr::new("--git-common-dir")], token)?;
        let common = common.strip_suffix(b"\n").unwrap_or(&common);
        let common = common.strip_suffix(b"\r").unwrap_or(common);
        let common = path_from_bytes(common)?.canonicalize().map_err(|e| e.to_string())?;
        crate::operation_queue::acquire(common, token)
    }
    pub fn editor_directory(&self, token: &Cancel) -> Result<PathBuf> {
        let bytes = self.read(&[OsStr::new("rev-parse"), OsStr::new("--absolute-git-dir")], token)?;
        let bytes = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
        path_from_bytes(bytes.strip_suffix(b"\r").unwrap_or(bytes))
    }
    pub(crate) fn stash_head(&self, token: &Cancel) -> Result<Option<String>> {
        let bytes = self.read(&[OsStr::new("for-each-ref"), OsStr::new("--format=%(refname) %(objectname)"), OsStr::new("refs/stash")], token)?;
        for line in String::from_utf8_lossy(&bytes).lines() {
            if let Some(id) = line.strip_prefix("refs/stash ") {
                if !is_oid(id) { return Err("Invalid stash object identity".into()); }
                return Ok(Some(id.into()));
            }
        }
        Ok(None)
    }
    pub(crate) fn chooser_index_children(&self, prefix: &str, token: &Cancel) -> Result<std::collections::BTreeSet<String>> {
        let query = if prefix.is_empty() { "." } else { prefix };
        let owned_prefix = prefix.to_owned();
        self.read_stream(&[OsStr::new("ls-files"), OsStr::new("--cached"), OsStr::new("-z"), OsStr::new("--"), OsStr::new(query)],
            token, move |out, _| crate::path_tree::index_children(out, &owned_prefix))
    }
    pub(crate) fn read(&self, args: &[&OsStr], token: &Cancel) -> Result<Vec<u8>> {
        self.read_stream(args, token, |out, flag| drain(out, OUTPUT_LIMIT, flag))
    }
    fn read_stream<T: Send + 'static>(
        &self, args: &[&OsStr], token: &Cancel,
        consume: impl FnOnce(std::process::ChildStdout, Arc<AtomicBool>) -> std::io::Result<T> + Send + 'static,
    ) -> Result<T> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot start Git: {e}"))?;
        let out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        let overflow = Arc::new(AtomicBool::new(false));
        let flag = overflow.clone();
        let stdout = thread::spawn(move || {
            let result = consume(out, flag.clone());
            if result.is_err() { flag.store(true, Ordering::Relaxed); }
            result
        });
        let flag = overflow.clone();
        let stderr = thread::spawn(move || drain(err, 64 * 1024, flag));
        let start = Instant::now();
        let mut aborted = None;
        let status = loop {
            if token.cancelled()
                || overflow.load(Ordering::Relaxed)
                || start.elapsed() > Duration::from_secs(30)
            {
                aborted = Some(if token.cancelled() {
                    "Cancelled"
                } else if overflow.load(Ordering::Relaxed) {
                    "Git output exceeds the 8 MiB limit; choose a smaller scope"
                } else {
                    "Git timed out after 30 seconds"
                });
                let _ = child.kill();
                break child.wait();
            }
            match child.try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) => thread::sleep(Duration::from_millis(5)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(e);
                }
            }
        };
        // Join both readers even if one fails; no detached pipe reader survives
        // completion or cancellation of the owning request.
        let output = stdout.join();
        let error = stderr.join();
        if token.cancelled() { return Err("Cancelled".into()); }
        let output = output.map_err(|_| "Output reader failed")?.map_err(|e| e.to_string())?;
        let error = error.map_err(|_| "Error reader failed")?.map_err(|e| e.to_string())?;
        if let Some(message) = aborted {
            return Err(message.into());
        }
        if overflow.load(Ordering::Relaxed) {
            return Err("Git output limit exceeded".into());
        }
        if !status.map_err(|e| e.to_string())?.success() {
            return Err(String::from_utf8_lossy(&error).trim().to_owned());
        }
        Ok(output)
    }
    pub(crate) fn text(&self, args: &[&str], token: &Cancel) -> Result<String> {
        utf8(self.read(&args.iter().map(OsStr::new).collect::<Vec<_>>(), token)?)
    }
    /// Complete raw author identities reachable from the supplied immutable
    /// history roots. Call with ALL log roots, not the selected branch's roots.
    /// Stream/deduplicate records rather than retaining per-commit output.
    pub fn author_catalog(&self, roots: &[String], token: &Cancel) -> Result<crate::authors::Catalog> {
        if token.cancelled() { return Err("Cancelled".into()); }
        if roots.is_empty() { return Ok(crate::authors::Catalog::new([])); }
        if roots.iter().any(|id| !matches!(id.len(), 40 | 64) || !id.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err("Author catalogue requires full commit IDs".into());
        }
        let mut args = vec!["-c", "log.mailmap=false", "log", "--no-patch", "--no-notes",
            "--no-show-signature", "--encoding=UTF-8", "-z", "--format=%an%x00%ae"];
        args.extend(roots.iter().map(String::as_str));
        args.push("--");
        self.read_stream(&args.iter().map(OsStr::new).collect::<Vec<_>>(), token,
            |out, _| crate::authors::read_catalog(out))
    }
    /// GitUserRegistry uses effective Git config, not the process's author env.
    /// --default distinguishes missing values from command/read failures without
    /// swallowing cancellation, malformed config, or a missing Git executable.
    pub fn current_author(&self, token: &Cancel) -> Result<Option<Author>> {
        let value = |key| -> Result<String> {
            let output = self.text(&["config", "--null", "--default=", "--get", key], token)?;
            output.strip_suffix('\0').map(str::to_owned).ok_or_else(|| "Invalid Git configuration output".into())
        };
        let name = value("user.name")?;
        if name.is_empty() { return Ok(None); }
        Ok(Some(Author { name, email: value("user.email")? }))
    }
    /// Read a bounded graph-only page from immutable root IDs. This avoids
    /// repeating ref discovery or loading hidden commits' presentation metadata.
    /// Git intentionally omits shallow parents; the snapshot boundary set still
    /// governs whether those rows may be compared against an empty tree.
    /// Position in the same date-ordered, unfiltered history as snapshot_at.
    /// Immutable full-OID roots prevent ref movement from shifting the result.
    /// Reads the entire ID stream with bounded memory; the process supervisor
    /// retains its cancellation and timeout guarantees and reaps both readers.
    pub fn commit_position(&self, roots: &[String], target: &str, token: &Cancel) -> Result<Option<usize>> {
        if token.cancelled() { return Err("Cancelled".into()); }
        if !is_oid(target) || roots.iter().any(|id| !is_oid(id)) { return Err("Commit navigation requires full commit IDs".into()); }
        if roots.is_empty() { return Ok(None); }
        let mut args = vec!["log", "--date-order", "--no-patch", "--no-notes", "--no-show-signature", "--no-decorate", "--format=%H"];
        args.extend(roots.iter().map(String::as_str));
        args.push("--");
        let target = target.to_owned();
        self.read_stream(&args.iter().map(OsStr::new).collect::<Vec<_>>(), token,
            move |out, _| crate::history_position::read_position(out, &target))
    }
    pub fn topology_page(
        &self,
        roots: &[String],
        offset: usize,
        limit: usize,
        token: &Cancel,
    ) -> Result<TopologyPage> {
        self.topology_filtered_page(roots, offset, limit, None, token)
    }
    /// With a structure filter, parents reflect Git's simplified path history.
    /// Use these rows only for matching IDs; graph edges and Diff targets must
    /// come from unfiltered topology / snapshot_commits.
    pub fn topology_filtered_page(
        &self,
        roots: &[String],
        offset: usize,
        limit: usize,
        filter: Option<&LogFilter>,
        token: &Cancel,
    ) -> Result<TopologyPage> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if roots.iter().any(|id| !is_oid(id)) {
            return Err("Invalid history root".into());
        }
        if let Some(filter) = filter { filter.validate_paths()?; }
        if roots.is_empty() || filter.is_some_and(LogFilter::matches_no_authors) {
            return Ok(TopologyPage {
                commits: Vec::new(),
                more: false,
            });
        }
        let limit = limit.clamp(1, HISTORY_PAGE);
        let count = format!("--max-count={}", limit + 1);
        let skip = format!("--skip={offset}");
        let mut args = vec![
            "-c", "log.mailmap=false",
            "log",
            "--date-order",
            "--no-show-signature",
            "--no-decorate",
            "--format=%H %P",
            &count,
            &skip,
        ];
        let filter_args = filter.map(LogFilter::arguments).unwrap_or_default();
        args.extend(filter_args.iter().map(String::as_str));
        args.extend(roots.iter().map(String::as_str));
        args.push("--");
        if let Some(filter) = filter { args.extend(filter.paths.iter().map(String::as_str)); }
        let output = self.text(&args, token)?;
        let mut commits = Vec::with_capacity(limit + 1);
        for line in output.lines() {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let mut fields = line.split_whitespace();
            let id = fields.next().ok_or("Missing graph commit ID")?;
            if !is_oid(id) {
                return Err("Invalid graph commit ID".into());
            }
            let mut parents = Vec::new();
            for parent in fields {
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                if !is_oid(parent) {
                    return Err("Invalid graph parent ID".into());
                }
                parents.push(parent.to_owned());
            }
            commits.push(TopologyCommit {
                id: id.to_owned(),
                parents,
            });
            if commits.len() > limit + 1 {
                return Err("Unexpected graph page length".into());
            }
        }
        let more = commits.len() > limit;
        commits.truncate(limit);
        Ok(TopologyPage { commits, more })
    }
    pub fn snapshot(
        &self,
        branch: Option<&str>,
        limit: usize,
        fixed_tips: Option<&[String]>,
        token: &Cancel,
    ) -> Result<Snapshot> {
        self.snapshot_at(branch, 0, limit, fixed_tips, token)
    }

    /// A bounded window of one fixed set of history roots. Skipping only
    /// changes returned rows; callers carry their graph frontier separately.
    pub fn snapshot_at(
        &self,
        branch: Option<&str>,
        offset: usize,
        limit: usize,
        fixed_tips: Option<&[String]>,
        token: &Cancel,
    ) -> Result<Snapshot> {
        self.snapshot_filtered_at(branch, offset, limit, fixed_tips, None, token)
    }

    /// Query the complete reachable history, applying message filtering before
    /// pagination. Parent IDs remain real parents, including hidden commits;
    /// callers must resolve hidden ancestry before rendering a filtered graph.
    pub fn snapshot_filtered_at(
        &self,
        branch: Option<&str>,
        offset: usize,
        limit: usize,
        fixed_tips: Option<&[String]>,
        text_filter: Option<&LogFilter>,
        token: &Cancel,
    ) -> Result<Snapshot> {
        let mut snapshot = self.snapshot_query(branch, offset, limit, fixed_tips, text_filter, None, token)?;
        if text_filter.is_some_and(|filter| !filter.paths.is_empty()) && !snapshot.commits.is_empty() {
            let ids = snapshot.commits.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
            let original = self.snapshot_commits(&ids, &snapshot.tips, token)?;
            snapshot.commits = original.commits;
        }
        Ok(snapshot)
    }
    /// Read a bounded set of full commit IDs, preserving its requested order.
    pub fn snapshot_commits(&self, ids: &[String], roots: &[String], token: &Cancel) -> Result<Snapshot> {
        if ids.len() > HISTORY_MAX || ids.iter().any(|id| !is_oid(id)) {
            return Err("Invalid selected history IDs".into());
        }
        self.snapshot_query(None, 0, ids.len().max(1), Some(roots), None, Some(ids), token)
    }
    fn snapshot_query(
        &self, branch: Option<&str>, offset: usize, limit: usize,
        fixed_tips: Option<&[String]>, text_filter: Option<&LogFilter>,
        selected: Option<&[String]>, token: &Cancel,
    ) -> Result<Snapshot> {
        if let Some(filter) = text_filter { filter.validate_paths()?; }
        self.text(&["rev-parse", "--git-dir"], token)?;
        let refs = self.text(
            &[
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00%(*objectname)%00%(upstream)%00%(upstream:remotename)%00%(objecttype)%00%(*objecttype)",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ],
            token,
        )?;
        let remotes = self.text(&["remote"], token)?;
        let remotes: Vec<_> = remotes.lines().collect();
        let mut upstreams = HashMap::new();
        let mut decorations: HashMap<String, Vec<RefLabel>> = HashMap::new();
        let mut branches = vec![];
        let mut tags = vec![];
        let mut tips = vec![];
        for line in refs.lines() {
            let fields = line.split('\0').collect::<Vec<_>>();
            if fields.len() != 7 {
                return Err("Invalid refs record".into());
            }
            if let (Some(local), Some(remote)) = (
                fields[0].strip_prefix("refs/heads/"),
                fields[3].strip_prefix("refs/remotes/"),
            ) {
                upstreams.insert(local.to_owned(), (remote.to_owned(), fields[4].to_owned()));
            }
            if fields[0].starts_with("refs/heads/") || fields[0].starts_with("refs/remotes/") {
                let (kind, name) = if let Some(name) = fields[0].strip_prefix("refs/heads/") {
                    (RefKind::Local, name)
                } else {
                    (
                        RefKind::Remote,
                        fields[0].strip_prefix("refs/remotes/").unwrap(),
                    )
                };
                decorations
                    .entry(fields[1].into())
                    .or_default()
                    .push(RefLabel {
                        kind,
                        name: name.into(),
                    });
                branches.push(Branch {
                    name: fields[0].into(),
                    id: fields[1].into(),
                });
            }
            // for-each-ref already supplies direct and dereferenced tag
            // targets. Fall back only if a dereferenced target is still a tag.
            // Tree/blob targets must never become history roots.
            if fields[0].starts_with("refs/tags/") {
                let id = match (fields[5], fields[6]) {
                    ("commit", _) => Some(fields[1].to_owned()),
                    ("tag", "commit") => Some(fields[2].to_owned()),
                    ("tag", "tag") => self
                        .text(
                            &[
                                "rev-parse",
                                "--verify",
                                &format!("{}^{{commit}}", fields[0]),
                            ],
                            token,
                        )
                        .ok()
                        .map(|id| id.trim().to_owned()),
                    _ => None,
                };
                if let Some(id) = id {
                    tags.push(Tag { name: fields[0].into(), id: id.clone() });
                    decorations.entry(id.clone()).or_default().push(RefLabel {
                        kind: RefKind::Tag,
                        name: fields[0].strip_prefix("refs/tags/").unwrap().into(),
                    });
                    tips.push(id);
                }
            } else {
                tips.push(fields[1].into());
            }
        }
        let mut head_id = None;
        if let Ok(head) = self.text(&["rev-parse", "--verify", "HEAD"], token) {
            let id = head.trim().to_owned();
            head_id = Some(id.clone());
            decorations.entry(id.clone()).or_default().push(RefLabel {
                kind: RefKind::Head,
                name: "HEAD".into(),
            });
            tips.push(id);
        }
        if let Some(name) = branch.filter(|_| fixed_tips.is_none()) {
            tips = if name == "HEAD" {
                head_id.iter().cloned().collect()
            } else {
                vec![branches.iter().find(|b| b.name == name)
                    .ok_or("Branch no longer exists; refresh")?.id.clone()]
            };
        }

        if let Some(fixed) = fixed_tips {
            tips = fixed.to_vec();
        }
        tips.sort();
        tips.dedup();
        if tips.iter().any(|s| !is_oid(s)) {
            return Err("Invalid history root".into());
        }
        let (shallow, boundaries) = self.shallow_boundaries(token)?;
        let mut commits = vec![];
        if !tips.is_empty() && !text_filter.is_some_and(LogFilter::matches_no_authors) {
            let count = format!("--max-count={}", limit.clamp(1, HISTORY_MAX) + 1);
            let skip = format!("--skip={offset}");
            let mut args = vec![
                "-c", "log.mailmap=false",
                "log",
                // Rebased's default Normal graph uses date order while
                // keeping children before parents (IntelliSort is separate).
                "--date-order",
                "--no-show-signature",
                "--encoding=UTF-8",
                "--date=iso-strict",
                "-z",
                &count,
                &skip,
                "--format=%H%x00%P%x00%an%x00%ad%x00%s",
            ];
            let filter_args = text_filter.map(LogFilter::arguments).unwrap_or_default();
            args.extend(filter_args.iter().map(String::as_str));
            if let Some(ids) = selected {
                // Keep command lines below Windows' limit; graph order is restored below.
                let base = args.clone();
                for chunk in ids.chunks(64) {
                    let mut query = base.clone();
                    query.push("--no-walk=unsorted");
                    query.extend(chunk.iter().map(String::as_str));
                    query.push("--");
                    commits.extend(parse_history(&self.text(&query, token)?)?);
                }
                let mut by_id: HashMap<_, _> = commits.into_iter().map(|c| (c.id.clone(), c)).collect();
                commits = ids.iter().map(|id| by_id.remove(id).ok_or_else(|| "Selected commit disappeared".to_owned())).collect::<Result<_>>()?;
            } else {
                args.extend(tips.iter().map(String::as_str));
                args.push("--");
                if let Some(filter) = text_filter { args.extend(filter.paths.iter().map(String::as_str)); }
                commits = parse_history(&self.text(&args, token)?)?;
            }
        }

        let more = commits.len() > limit.clamp(1, HISTORY_MAX);
        commits.truncate(limit.clamp(1, HISTORY_MAX));
        let current_branch = self
            .text(&["symbolic-ref", "--quiet", "HEAD"], token)
            .ok()
            .and_then(|name| {
                name.trim_end()
                    .strip_prefix("refs/heads/")
                    .map(str::to_owned)
            });
        let ref_titles = Arc::new(
            decorations
                .iter()
                .map(|(id, refs)| {
                    (
                        id.clone(),
                        crate::ref_labels::compact_title(
                            refs,
                            current_branch.as_deref(),
                            &upstreams,
                            &remotes,
                        ),
                    )
                })
                .collect(),
        );
        for commit in &mut commits {
            commit.shallow_boundary = boundaries.contains(&commit.id);
            if let Some(mut labels) = decorations.remove(&commit.id) {
                labels.sort_by(|a, b| crate::ref_labels::rank(a, current_branch.as_deref()).cmp(&crate::ref_labels::rank(b, current_branch.as_deref())).then_with(|| crate::natural_order::compare(&a.name, &b.name)));
                commit.refs = labels;
            }
        }
        let included_in_head = Arc::new(self.contained_in_head(head_id.as_deref(), &commits, token)?);
        Ok(Snapshot {
            head_id,
            included_in_head,
            current_branch,
            ref_titles,
            commits,
            branches,
            tags,
            tips,
            more,
            shallow,
        })
    }
    /// Keep membership only for this displayed window; page through immutable HEAD.
    fn contained_in_head(&self, head: Option<&str>, commits: &[Commit], token: &Cancel) -> Result<HashSet<String>> {
        let Some(head) = head else { return Ok(HashSet::new()); };
        let mut remaining: HashSet<_> = commits.iter().map(|c| c.id.clone()).collect();
        let mut included = HashSet::new();
        let roots = vec![head.to_owned()];
        let mut offset = 0;
        while !remaining.is_empty() {
            let page = self.topology_page(&roots, offset, HISTORY_PAGE, token)?;
            offset += page.commits.len();
            for commit in page.commits {
                if token.cancelled() { return Err("Cancelled".into()); }
                if remaining.remove(&commit.id) { included.insert(commit.id); }
            }
            if !page.more { break; }
        }
        Ok(included)
    }
    /// Branches whose tips contain this immutable commit, independent of the
    /// loaded history window. Only query the selected commit; retain no cache.
    /// HEAD is included when reachable, matching Rebased's graph-backed list.
    pub fn containing_branches(&self, id: &str, token: &Cancel) -> Result<Vec<String>> {
        if !is_oid(id) { return Err("Invalid commit ID".into()); }
        let args = ["branch", "--all", "--no-color", "--no-column", "--contains", id,
            "--format=%(HEAD)%00%(refname)%00%(symref)"];
        let output = self.read_stream(&args.iter().map(OsStr::new).collect::<Vec<_>>(), token,
            |out, flag| drain(out, 1024 * 1024, flag))?;
        let output = utf8(output)?;
        let mut branches = Vec::new();
        let mut head = false;
        for record in output.lines() {
            let fields: Vec<_> = record.split('\0').collect();
            if fields.len() != 3 || !matches!(fields[0], " " | "*") {
                return Err("Invalid containing branches record".into());
            }
            let current = fields[0] == "*";
            head |= current;
            let (rank, name) = if let Some(name) = fields[1].strip_prefix("refs/heads/") {
                (if current { 1 } else if matches!(name, "main" | "master") { 2 } else { 4 }, name)
            } else if let Some(name) = fields[1].strip_prefix("refs/remotes/") {
                if !fields[2].is_empty() { continue; }
                (if matches!(name, "origin/main" | "origin/master") { 3 } else { 5 }, name)
            } else if current {
                // `git branch` emits a synthetic current entry for detached HEAD.
                continue;
            } else {
                return Err("Unexpected containing branch reference".into());
            };
            branches.push((rank, name.to_owned()));
        }
        branches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| crate::natural_order::compare(&a.1, &b.1)));
        let mut names = Vec::with_capacity(branches.len() + usize::from(head));
        if head { names.push("HEAD".into()); }
        names.extend(branches.into_iter().map(|(_, name)| name));
        Ok(names)
    }

    pub(crate) fn shallow_boundaries(&self, token: &Cancel) -> Result<(bool, HashSet<String>)> {
        let shallow = self
            .text(&["rev-parse", "--is-shallow-repository"], token)?
            .trim()
            == "true";
        // Git hides parents of shallow commits in log output. Keep the explicit
        // boundary IDs so those rows cannot be mistaken for genuine roots.
        let mut boundaries = HashSet::new();
        if shallow {
            let path = self.text(&["rev-parse", "--git-path", "shallow"], token)?;
            let mut bytes = Vec::new();
            std::fs::File::open(self.path.join(path.trim_end()))
                .map_err(|e| format!("Cannot read shallow boundaries: {e}"))?
                .take(OUTPUT_LIMIT as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Cannot read shallow boundaries: {e}"))?;
            if bytes.len() > OUTPUT_LIMIT {
                return Err("Shallow boundary list exceeds the read limit".into());
            }
            let text = std::str::from_utf8(&bytes).map_err(|_| "Invalid shallow boundary list")?;
            for id in text.lines() {
                if !is_oid(id) {
                    return Err("Invalid shallow boundary ID".into());
                }
                boundaries.insert(id.to_owned());
            }
        }
        if token.cancelled() { return Err("Cancelled".into()); }
        Ok((shallow, boundaries))
    }

    /// Push uses GitCommitRequirements.DEFAULT: root changes, rename detection,
    /// and combined merge statuses, rather than the Log's first-parent details.
    pub fn push_commit_changes(&self, commit: &Commit, token: &Cancel) -> Result<Vec<crate::combined_changes::Change>> {
        if !is_oid(&commit.id) || commit.parents.iter().any(|p| !is_oid(p)) {
            return Err("Invalid commit ID".into());
        }
        if commit.shallow_boundary {
            return Err("Push preview is unavailable at shallow history boundary".into());
        }
        let bytes = self.read(&["diff-tree", "--root", "--no-commit-id", "-r", "-c",
            "--name-status", "-z", "--find-renames", "--ignore-submodules=none",
            "--no-ext-diff", "--no-textconv", &commit.id, "--"].map(OsStr::new), token)?;
        let files = if commit.parents.len() <= 1 {
            parse_names(&bytes)?
        } else {
            // Without --combined-all-paths Git emits one destination path per
            // combined status, including renames. Rebased takes status[0] and
            // falls back to that path when MOVED has no second path.
            let mut fields = bytes.split(|b| *b == 0);
            let mut files = Vec::new();
            while let Some(status) = fields.next() {
                if status.is_empty() {
                    if fields.next().is_some() { return Err("Malformed combined status".into()); }
                    break;
                }
                if status.len() != commit.parents.len() || !status.iter().all(|c| b"AMDTCR".contains(c)) {
                    return Err("Unsupported combined status".into());
                }
                let path = path_from_bytes(fields.next().filter(|p| !p.is_empty()).ok_or("Missing combined path")?)?;
                files.push(Change { old_path: matches!(status[0], b'R' | b'C').then(|| path.clone()),
                    path, status: String::from_utf8(vec![status[0]]).unwrap(), area: Area::Commit });
            }
            files
        };
        files.iter().map(|file| crate::combined_changes::from_file(file, &commit.id,
            commit.parents.first().map(String::as_str))).collect()
    }

    /// Log details use per-parent diffs and VcsStatusMerger, not Push's -c parser.
    pub fn log_commit_changes(&self, commit: &Commit, token: &Cancel) -> Result<Vec<crate::combined_changes::Change>> {
        if !is_oid(&commit.id) || commit.parents.iter().any(|p| !is_oid(p)) {
            return Err("Invalid commit ID".into());
        }
        if commit.shallow_boundary { return Err("Log preview is unavailable at shallow history boundary".into()); }
        let mut groups = Vec::new();
        let mut count = 0usize;
        for parent in if commit.parents.is_empty() { vec![None] } else { commit.parents.iter().map(Some).collect() } {
            if token.cancelled() { return Err("Cancelled".into()); }
            let mut args = vec!["diff-tree", "--no-commit-id", "-r", "--name-status", "-z",
                "--find-renames", "--ignore-submodules=none", "--no-ext-diff", "--no-textconv"];
            if let Some(parent) = parent { args.push(parent); } else { args.push("--root"); }
            args.extend([commit.id.as_str(), "--"]);
            let files = parse_names(&self.read(&args.iter().map(OsStr::new).collect::<Vec<_>>(), token)?)?;
            if files.len() > crate::combined_changes::MAX_INPUT_CHANGES.saturating_sub(count) {
                return Err("Log preview exceeds the change limit".into());
            }
            count += files.len(); groups.push(files);
        }
        let files = crate::log_changes::merge_parents(&groups)?;
        if token.cancelled() { return Err("Cancelled".into()); }
        files.iter().map(|file| crate::combined_changes::from_file(file, &commit.id,
            commit.parents.first().map(String::as_str))).collect()
    }

    /// Read commit text only. `files` is intentionally unloaded, not an empty diff.
    pub fn commit_metadata(&self, commit: &Commit, token: &Cancel) -> Result<Details> {
        if !is_oid(&commit.id) {
            return Err("Invalid commit ID".into());
        }
        let details = self.text(
            &[
                "show",
                "--no-patch",
                "--no-show-signature",
                "--encoding=UTF-8",
                "--format=%an <%ae>%x00%aI%x00%cn <%ce>%x00%cI%x00%B",
                &commit.id,
                "--",
            ],
            token,
        )?;
        let mut fields = details.splitn(5, '\0');
        let author = fields.next().unwrap_or_default().to_owned();
        let authored = fields.next().ok_or("Missing author date")?.to_owned();
        let committer = fields.next().ok_or("Missing committer")?.to_owned();
        let committed = fields.next().ok_or("Missing commit date")?.to_owned();
        let message = fields.next().ok_or("Missing commit message")?.to_owned();
        Ok(Details { message, author, authored, committer, committed,
            base: if commit.shallow_boundary { "Unavailable at shallow history boundary".into() }
                else { commit.parents.first().cloned().unwrap_or_else(|| "Empty tree".into()) },
            files: Vec::new() })
    }

    pub fn details(&self, commit: &Commit, token: &Cancel) -> Result<Details> {
        let mut details = self.commit_metadata(commit, token)?;
        if commit.shallow_boundary { return Ok(details); }
        let base = commit.parents.first().cloned().unwrap_or_default();
        let args = if base.is_empty() {
            vec![
                "diff-tree",
                "--root",
                "--no-commit-id",
                "-r",
                "--name-status",
                "--ignore-submodules=none",
                "-z",
                "--find-renames",
                &commit.id,
                "--",
            ]
        } else {
            vec![
                "diff",
                "--name-status",
                "--ignore-submodules=none",
                "-z",
                "--find-renames",
                &base,
                &commit.id,
                "--",
            ]
        };
        let files =
            parse_names(&self.read(&args.iter().map(OsStr::new).collect::<Vec<_>>(), token)?)?;
        details.files = files;
        Ok(details)
    }
    /// GitMergeUpdater.isSaveNeeded / GitUtil.getPathsDiffBetweenRefs.
    /// Read-only, after fetch. Failures conservatively require saving, except
    /// cancellation; the actual save/operation still validates repository state.
    pub fn merge_update_save_needed(&self, target: &crate::pull::Target, token: &Cancel) -> Result<bool> {
        let inspect = || -> Result<bool> {
            let targets = self.pull_targets(token)?;
            let branch = targets.current_branch.as_ref().ok_or("Merge update requires an attached branch")?;
            if !targets.contains(&target.remote, &target.branch) { return Err("Merge update target disappeared".into()); }
            let staged = self.read(&[OsStr::new("diff"), OsStr::new("--cached"), OsStr::new("--name-only"),
                OsStr::new("--no-renames"), OsStr::new("-z"), OsStr::new("--")], token)?;
            if !staged.is_empty() { return Ok(true); }
            // Untracked files are not part of ChangeListManager affected paths;
            // avoid enumerating them merely to decide whether to save tracked work.
            let local = parse_status(&self.read(&[OsStr::new("status"), OsStr::new("--porcelain=v1"),
                OsStr::new("-z"), OsStr::new("--untracked-files=no"), OsStr::new("--ignore-submodules=none")], token)?)?;
            let changed: Vec<_> = local.iter().filter(|change| change.area != Area::Untracked)
                .flat_map(|change| std::iter::once(&change.path).chain(change.old_path.iter())).collect();
            if changed.is_empty() { return Ok(false); }
            let range = format!("refs/heads/{branch}..refs/remotes/{}/{}", target.remote, target.branch);
            let bytes = self.read(&[OsStr::new("diff"), OsStr::new("--no-ext-diff"), OsStr::new("--no-textconv"),
                OsStr::new("--name-only"), OsStr::new("-z"), OsStr::new(&range), OsStr::new("--")], token)?;
            if !bytes.is_empty() && !bytes.ends_with(&[0]) { return Err("Incomplete changed-path record".into()); }
            for raw in bytes.split(|b| *b == 0).filter(|s| !s.is_empty()) {
                let remote = path_from_bytes(raw)?;
                if changed.iter().any(|local| {
                    #[cfg(windows)] { local.to_string_lossy().to_lowercase() == remote.to_string_lossy().to_lowercase() }
                    #[cfg(not(windows))] { **local == remote }
                }) { return Ok(true); }
            }
            Ok(false)
        };
        let decision = inspect();
        if token.cancelled() { return Err("Cancelled".into()); }
        Ok(decision.unwrap_or(true))
    }
    /// GitRebaseOverMergeProblem: only merge commits with combined changes in
    /// base..HEAD trigger the warning. A merge node alone is insufficient.
    /// Read failures remain unknown; callers must not treat them as proof of safety.
    pub fn rebase_over_merge_problem(&self, target: &crate::pull::Target, token: &Cancel) -> Result<bool> {
        let known = self.pull_targets(token)?;
        if known.current_branch.is_none() || !known.contains(&target.remote, &target.branch) {
            return Err("Rebase requires an attached branch and an existing remote branch".into());
        }
        let reference = format!("refs/remotes/{}/{}^{{commit}}", target.remote, target.branch);
        let base = self.text(&["rev-parse", "--verify", &reference], token)?;
        let head = self.text(&["rev-parse", "--verify", "HEAD^{commit}"], token)?;
        let range = format!("{}..{}", base.trim(), head.trim());
        // GitCommitRequirements.DEFAULT = -M -c. No commit messages or headers
        // are needed for the existence query; NUL output keeps path bytes opaque.
        let bytes = self.read(&[OsStr::new("log"), OsStr::new("--format="),
            OsStr::new("--name-only"), OsStr::new("-z"), OsStr::new("-c"), OsStr::new("-M"),
            OsStr::new("--no-ext-diff"), OsStr::new("--no-textconv"),
            OsStr::new("--merges"), OsStr::new(&range), OsStr::new("--")], token)?;
        Ok(!bytes.is_empty())
    }

    /// Tracked changes only: Rebase updater save/fast-forward decision excludes untracked files.
    pub fn has_tracked_changes(&self, token: &Cancel) -> Result<bool> {
        let bytes = self.read(&[OsStr::new("status"), OsStr::new("--porcelain=v1"),
            OsStr::new("-z"), OsStr::new("--untracked-files=no"),
            OsStr::new("--ignore-submodules=none")], token)?;
        Ok(!parse_status(&bytes)?.is_empty())
    }

    pub fn local_changes(&self, token: &Cancel) -> Result<Vec<Change>> {
        let bytes = self.read(
            &[
                OsStr::new("status"),
                OsStr::new("--porcelain=v1"),
                OsStr::new("-z"),
                OsStr::new("--untracked-files=all"),
                OsStr::new("--ignore-submodules=none"),
            ],
            token,
        )?;
        parse_status(&bytes)
    }
    fn working_tree_base(&self, token: &Cancel) -> Result<String> {
        match crate::commit_operation::head(self, token)? {
            Some(head) => Ok(head),
            // Compute the repository-format empty tree without writing an object.
            None => Ok(utf8(self.read(&[OsStr::new("hash-object"), OsStr::new("-t"), OsStr::new("tree"), OsStr::new("--stdin")],token)?)?.trim().to_owned()),
        }
    }
    /// Compare two immutable revision trees (Rebased GitChangeUtils.getDiff,
    /// two-dot semantics). Neither ancestry nor the current HEAD is implied.
    pub fn revision_changes(&self, before: &str, after: &str, paths: &[PathBuf], token: &Cancel)
        -> Result<Vec<crate::combined_changes::Change>> {
        use std::path::Component;
        if !is_oid(before) || !is_oid(after) { return Err("Version comparison requires full commit IDs".into()); }
        if paths.len() > 10_000 || paths.iter().any(|p| p.as_os_str().is_empty()
            || p.components().any(|part| !matches!(part, Component::Normal(_)))) {
            return Err("Invalid comparison path filter".into());
        }
        let mut args = vec![OsStr::new("--literal-pathspecs"), OsStr::new("diff"), OsStr::new("--name-status"),
            OsStr::new("-z"), OsStr::new("--no-ext-diff"), OsStr::new("--no-textconv"),
            OsStr::new("--find-renames"), OsStr::new("--ignore-submodules=none"),
            OsStr::new(before), OsStr::new(after), OsStr::new("--")];
        args.extend(paths.iter().map(|p| p.as_os_str()));
        let changes = parse_names(&self.read(&args, token)?)?;
        if changes.len() > 100_000 { return Err("Version comparison exceeds the file limit".into()); }
        if token.cancelled() { return Err("Cancelled".into()); }
        changes.iter().map(|change| crate::combined_changes::from_file(change, after, Some(before))).collect()
    }

    /// Net tracked changes, including staged and unstaged edits, with no index writes.
    /// Unversioned files and conflicts are obtained separately from local_changes.
    pub fn working_tree_changes(&self, token: &Cancel) -> Result<Vec<WorkingTreeChange>> {
        let base=self.working_tree_base(token)?;
        let bytes=self.read(&[OsStr::new("diff"),OsStr::new("--name-status"),OsStr::new("-z"),
            OsStr::new("--no-ext-diff"),OsStr::new("--no-textconv"),OsStr::new("--find-renames"),
            OsStr::new("--ignore-submodules=none"),OsStr::new(&base),OsStr::new("--")],token)?;
        if self.working_tree_base(token)? != base { return Err("HEAD changed; refresh local changes".into()); }
        Ok(parse_names(&bytes)?.into_iter().map(|c|WorkingTreeChange{path:c.path,old_path:c.old_path,status:c.status}).collect())
    }
    pub fn working_tree_patch(&self, change: &WorkingTreeChange, full: bool, token: &Cancel) -> Result<String> {
        if change.status.starts_with('U') { return Err("Resolve conflicts before reviewing this change".into()); }
        let c=Change{path:change.path.clone(),old_path:change.old_path.clone(),status:change.status.clone(),area:Area::Unstaged};
        self.patch_context(None,&c,full,true,token)
    }
    pub fn patch(
        &self,
        commit: Option<&Commit>,
        change: &Change,
        token: &Cancel,
    ) -> Result<String> {
        self.patch_context(commit, change, false, change.area == Area::WorkingTree, token)
    }
    /// Full-file context for the editor-tab viewer, with the same output bound.
    pub fn full_patch(&self, commit: Option<&Commit>, change: &Change, token: &Cancel) -> Result<String> {
        self.patch_context(commit, change, true, change.area == Area::WorkingTree, token)
    }
    fn patch_context(&self, commit: Option<&Commit>, change: &Change, full: bool, whole_worktree: bool, token: &Cancel) -> Result<String> {
        let context = if full { "--unified=2147483647" } else { "--unified=20" };
        if commit.is_some_and(|c| c.shallow_boundary) {
            return Err("Parent history is unavailable at this shallow boundary. Deepen the clone with your Git tool, then refresh.".into());
        }
        if change.area == Area::Conflict {
            return Err("Unmerged index entry. Resolve with your Git tool, then refresh. This viewer does not modify conflicts.".into());
        }
        if change.area == Area::Untracked {
            let mut path = self.path.clone();
            if change.path.as_os_str().is_empty() {
                return Err("Invalid untracked file path; refresh".into());
            }
            for component in change.path.components() {
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                let std::path::Component::Normal(name) = component else {
                    return Err(
                        "Untracked file path must stay inside the repository; refresh".into(),
                    );
                };
                path.push(name);
                let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                let linked = metadata.file_type().is_symlink();
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt;
                    // Junctions and other reparse points may not be reported
                    // as symlinks. Do not traverse them for a text preview.
                    linked || metadata.file_attributes() & 0x400 != 0
                };
                if linked {
                    return Err("Untracked preview does not follow linked files or directories; refresh after checking the path".into());
                }
            }
            let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err("Only regular untracked files can be previewed".into());
            }
            if meta.len() > 5 * 1024 * 1024 {
                return Err("File exceeds the 5 MiB preview limit".into());
            }
            let mut bytes = vec![];
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(5 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 5 * 1024 * 1024 {
                return Err("File grew beyond the preview limit; refresh".into());
            }
            if bytes.contains(&0) {
                return Err("Binary file — text preview unavailable".into());
            }
            return utf8(bytes);
        }
        let working_base=if whole_worktree {Some(self.working_tree_base(token)?)} else {None};
        let mut args = vec![
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            context,
            "--submodule=short",
            "--ignore-submodules=none",
        ];
        if change.area == Area::Commit {
            let c = commit.ok_or("Select a commit")?;
            if !is_oid(&c.id) {
                return Err("Invalid commit ID".into());
            }
            args = vec![
                "show",
                "--format=",
                "--no-show-signature",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--diff-merges=first-parent",
                "--find-renames",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                context,
                "--submodule=short",
                "--ignore-submodules=none",
                &c.id,
            ];
        } else if change.area == Area::Staged {
            args.push("--cached");
        }
        if let Some(base)=&working_base { args.push(base); }
        args.push("--");
        let mut args = args.iter().map(OsStr::new).collect::<Vec<_>>();
        args.push(change.path.as_os_str());
        if let Some(old) = &change.old_path {
            args.push(old.as_os_str());
        }
        let mut patch = utf8(self.read(&args, token)?)?;
        // Git omits text for identical blobs (rename or executable-bit change).
        // The editor still needs both complete sides, as Rebased revision content does.
        let metadata_only = !patch.lines().any(|l| l.starts_with("@@") || l.starts_with("index ") || l.starts_with("Binary "))
            && (patch.lines().any(|l| l == "similarity index 100%")
                || patch.lines().any(|l| l.starts_with("old mode 100")));
        if full && metadata_only {
            let mut spec = std::ffi::OsString::from(working_base.as_deref().unwrap_or_else(||commit.map(|c| c.id.as_str()).unwrap_or("")));
            spec.push(":");
            spec.push(if whole_worktree {change.old_path.as_ref().unwrap_or(&change.path)} else {&change.path});
            // cat-file requires a blob: never interpret trees or gitlinks as text.
            let bytes = self.read(&[OsStr::new("cat-file"), OsStr::new("blob"), &spec], token)?;
            if bytes.contains(&0) { return Err("Binary file — text preview unavailable".into()); }
            let content = utf8(bytes)?;
            if !content.is_empty() {
                let count = content.split_inclusive('\n').count();
                let required = patch.len().saturating_add(content.len()).saturating_add(count).saturating_add(1024);
                if required > OUTPUT_LIMIT { return Err("Full file exceeds the preview output limit".into()); }
                fn quoted(prefix: &str, path: &std::path::Path) -> Result<String> {
                    let path = path.to_str().ok_or("File path is not UTF-8")?;
                    let mut out = format!("\"{prefix}");
                    for b in path.bytes() {
                        match b {
                            b'"' | b'\\' => { out.push('\\'); out.push(b as char); }
                            32..=126 => out.push(b as char),
                            _ => out.push_str(&format!("\\{b:03o}")),
                        }
                    }
                    out.push('"'); Ok(out)
                }
                patch.push_str(&format!("--- {}\n+++ {}\n@@ -1,{count} +1,{count} @@\n",
                    quoted("a/", change.old_path.as_deref().unwrap_or(&change.path))?, quoted("b/", &change.path)?));
                for line in content.split_inclusive('\n') {
                    if token.cancelled() { return Err("Cancelled".into()); }
                    patch.push(' '); patch.push_str(line);
                }
                if !content.ends_with('\n') { patch.push_str("\n\\ No newline at end of file\n"); }
            }
        }
        if let Some(base)=working_base {
            if self.working_tree_base(token)? != base {return Err("HEAD changed; refresh local changes".into());}
        }
        Ok(patch)
    }
}
fn parse_history(output: &str) -> Result<Vec<Commit>> {
    if output.is_empty() {
        return Ok(vec![]);
    }
    let fields = output
        .strip_suffix('\0')
        .unwrap_or(output)
        .split('\0')
        .collect::<Vec<_>>();
    if fields.len() % 5 != 0 {
        return Err("Incomplete Git history record".into());
    }
    fields
        .chunks_exact(5)
        .map(|f| {
            let parents = f[1]
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if !is_oid(f[0]) || parents.iter().any(|p| !is_oid(p)) {
                return Err("Invalid object ID".into());
            }
            Ok(Commit {
                id: f[0].into(),
                parents,
                author: f[2].into(),
                date: f[3].into(),
                subject: f[4].into(),
                refs: Vec::new(),
                shallow_boundary: false,
            })
        })
        .collect()
}
fn parse_names(bytes: &[u8]) -> Result<Vec<Change>> {
    let mut fields = bytes.split(|b| *b == 0).filter(|b| !b.is_empty());
    let mut result = vec![];
    while let Some(status) = fields.next() {
        let status = String::from_utf8_lossy(status).into_owned();
        let first = path_from_bytes(fields.next().ok_or("Missing file path")?)?;
        let (path, old_path) = if status.starts_with(['R', 'C']) {
            (
                path_from_bytes(fields.next().ok_or("Missing renamed path")?)?,
                Some(first),
            )
        } else {
            (first, None)
        };
        result.push(Change {
            path,
            old_path,
            status,
            area: Area::Commit,
        });
    }
    Ok(result)
}
fn parse_status(bytes: &[u8]) -> Result<Vec<Change>> {
    let mut fields = bytes.split(|b| *b == 0).filter(|b| !b.is_empty());
    let mut result = vec![];
    while let Some(record) = fields.next() {
        if record.len() < 4 || record[2] != b' ' {
            return Err("Invalid status record".into());
        }
        let xy = &record[..2];
        let path = path_from_bytes(&record[3..])?;
        let old_path = if xy.contains(&b'R') || xy.contains(&b'C') {
            Some(path_from_bytes(
                fields.next().ok_or("Missing rename source")?,
            )?)
        } else {
            None
        };
        let area = if xy == b"??" {
            Some(Area::Untracked)
        } else if xy.contains(&b'U') || xy == b"AA" || xy == b"DD" {
            Some(Area::Conflict)
        } else {
            None
        };
        if let Some(area) = area {
            result.push(Change {
                path,
                old_path,
                status: String::from_utf8_lossy(xy).into(),
                area,
            });
        } else {
            for (i, area) in [(0, Area::Staged), (1, Area::Unstaged)] {
                if xy[i] != b' ' {
                    result.push(Change {
                        path: path.clone(),
                        old_path: old_path.clone(),
                        status: (xy[i] as char).to_string(),
                        area,
                    });
                }
            }
        }
    }
    result.sort_by_key(|f| {
        (
            match f.area {
                Area::Conflict => 0,
                Area::Unstaged => 1,
                Area::Staged => 2,
                _ => 3,
            },
            f.path.clone(),
        )
    });
    Ok(result)
}

fn drain(
    mut pipe: impl Read,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut buf = [0; 8192];
    loop {
        let n = pipe.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if result.len() + n > limit {
            overflow.store(true, Ordering::Relaxed);
            break;
        }
        result.extend_from_slice(&buf[..n]);
    }
    Ok(result)
}
