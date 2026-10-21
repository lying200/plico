//! Unmerged index identities, based on Rebased 4aec352 GitMergeProvider and
//! GitMergeUtil (JetBrains and contributors, Apache-2.0).
use crate::{
    git::{self, Cancel, Repository},
    operation_state::Operation,
};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Component, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub mode: String,
    pub oid: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Yours,
    Theirs,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub path: PathBuf,
    /// Index stages 1 (base), 2 (ours), 3 (theirs). Absence means deletion,
    /// except an absent base in an add/add conflict.
    pub stages: [Option<Version>; 3],
}
impl File {
    pub fn side(&self, side: Side, reversed: bool) -> Option<&Version> {
        let ours = (side == Side::Yours) != reversed;
        self.stages[if ours { 1 } else { 2 }].as_ref()
    }
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub files: Vec<File>,
    pub operation: Operation,
    pub head: Option<String>,
    pub reversed: bool,
}
impl Snapshot {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        let head = crate::commit_operation::head(repo, token)?;
        let state = repo.operation_state(token)?;
        let data = repo.read(
            &[
                OsStr::new("ls-files"),
                OsStr::new("--unmerged"),
                OsStr::new("-z"),
            ],
            token,
        )?;
        let files = parse(&data)?;
        let after = repo.operation_state(token)?;
        if crate::commit_operation::head(repo, token)? != head || after.operation != state.operation
        {
            return Err("Repository changed while reading conflicts; refresh and retry".into());
        }
        let saved_changes = state.operation == Operation::None
            && saved_changes_context(repo, head.as_deref(), &files, token)?;
        Ok(Self {
            files,
            head,
            operation: state.operation,
            reversed: saved_changes
                || matches!(
                    state.operation,
                    Operation::RebaseApply | Operation::RebaseMerge
                ),
        })
    }
    /// Check the selected file's exact index versions again before a resolution.
    /// Other files may have been resolved independently in the meantime.
    pub fn revalidate(&self, repo: &Repository, file: &File, token: &Cancel) -> Result<(), String> {
        if !self.files.contains(file) {
            return Err("File does not belong to this conflict snapshot".into());
        }
        let current = Self::read(repo, token)?;
        if current.head != self.head
            || current.operation != self.operation
            || current.reversed != self.reversed
            || current.files.iter().find(|f| f.path == file.path) != Some(file)
        {
            return Err("Conflict changed; refresh before resolving it".into());
        }
        Ok(())
    }
}

// Context belongs to this worktree, not to the position-dependent stash selector.
// Only automatic saved-change restoration creates it. Bind the display sides to
// exact HEAD and index versions, allowing other listed files to be resolved.
const SAVED_CONTEXT: &str = "plico-conflict-saved-changes";
const CONTEXT_LIMIT: u64 = 8 * 1024 * 1024 + 128;
pub(crate) fn clear_saved_context(repo: &Repository, token: &Cancel) -> Result<(), String> {
    let path = repo.editor_directory(token)?.join(SAVED_CONTEXT);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
pub(crate) fn record_saved_context(repo: &Repository) -> Result<(), String> {
    use std::io::Write;
    let token = Cancel::default();
    let head = crate::commit_operation::head(repo, &token)?;
    let state = repo.operation_state(&token)?;
    if state.operation != Operation::None || state.conflicts.is_empty() {
        return Ok(());
    }
    let data = repo.read(
        &[
            OsStr::new("ls-files"),
            OsStr::new("--unmerged"),
            OsStr::new("-z"),
        ],
        &token,
    )?;
    if parse(&data)?.is_empty() {
        return Ok(());
    }
    if crate::commit_operation::head(repo, &token)? != head {
        return Err("Repository changed while recording saved-change conflicts".into());
    }
    let path = repo.editor_directory(&token)?.join(SAVED_CONTEXT);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let result = writeln!(file, "{}", head.as_deref().unwrap_or(""))
        .and_then(|_| file.write_all(&data))
        .and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(path);
        return Err(error.to_string());
    }
    Ok(())
}
fn saved_changes_context(
    repo: &Repository,
    head: Option<&str>,
    files: &[File],
    token: &Cancel,
) -> Result<bool, String> {
    use std::io::Read;
    if files.is_empty() {
        return Ok(false);
    }
    let path = repo.editor_directory(token)?.join(SAVED_CONTEXT);
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("Invalid saved-conflict context".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("Invalid saved-conflict context".into());
        }
    }
    let mut data = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(CONTEXT_LIMIT + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > CONTEXT_LIMIT {
        return Err("Saved-conflict context is too large".into());
    }
    let newline = data
        .iter()
        .position(|b| *b == b'\n')
        .ok_or("Invalid saved-conflict context")?;
    if &data[..newline] != head.unwrap_or("").as_bytes() {
        return Ok(false);
    }
    let saved = parse(&data[newline + 1..])?;
    Ok(files.iter().all(|file| {
        saved
            .binary_search_by(|candidate| candidate.path.cmp(&file.path))
            .is_ok_and(|index| saved[index] == *file)
    }))
}

fn parse(data: &[u8]) -> Result<Vec<File>, String> {
    if !data.is_empty() && !data.ends_with(&[0]) {
        return Err("Truncated unmerged index".into());
    }
    let mut files: BTreeMap<PathBuf, File> = BTreeMap::new();
    for entry in data.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let tab = entry
            .iter()
            .position(|b| *b == b'\t')
            .ok_or("Malformed unmerged index entry")?;
        let header = std::str::from_utf8(&entry[..tab]).map_err(|e| e.to_string())?;
        let fields: Vec<_> = header.split(' ').collect();
        if fields.len() != 3
            || !git::is_oid(fields[1])
            || !matches!(fields[0], "100644" | "100755" | "120000" | "160000")
        {
            return Err("Invalid unmerged index identity".into());
        }
        let stage = match fields[2] {
            "1" => 0,
            "2" => 1,
            "3" => 2,
            _ => return Err("Invalid conflict stage".into()),
        };
        let path = git::path_from_bytes(&entry[tab + 1..])?;
        if path.as_os_str().is_empty()
            || !path
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err("Invalid conflict path".into());
        }
        let file = files.entry(path.clone()).or_insert_with(|| File {
            path,
            stages: Default::default(),
        });
        if file.stages[stage]
            .replace(Version {
                mode: fields[0].into(),
                oid: fields[1].into(),
            })
            .is_some()
        {
            return Err("Duplicate conflict stage".into());
        }
    }
    Ok(files.into_values().collect())
}

/// Read by immutable object identity rather than :stage:path, which can change
/// while a preview loads. Gitlinks are commits, not editable file contents.
pub fn content(repo: &Repository, version: &Version, token: &Cancel) -> Result<Vec<u8>, String> {
    if !git::is_oid(&version.oid) || version.mode == "160000" {
        return Err("This conflict version is not a file blob".into());
    }
    repo.read(
        &[
            OsStr::new("cat-file"),
            OsStr::new("blob"),
            OsStr::new(&version.oid),
        ],
        token,
    )
}

#[derive(Clone, Copy, Debug)]
pub enum Resolution {
    Accept(Side),
    /// The caller must save the editor document before staging its disk contents.
    MarkResolved,
}

pub struct ResolutionResult {
    pub commands: Vec<Result<crate::operation_process::Outcome, String>>,
    /// Always refreshed after writes, including cancellation or partial failure.
    pub snapshot: Result<Snapshot, String>,
}

/// Worker-only, one exact path at a time. Accepting a version and staging it are
/// separate Git commands, as in GitMergeUtil. A failed second command must not
/// be represented as a successful resolution or retried automatically.
pub fn resolve(
    repo: &Repository,
    snapshot: &Snapshot,
    file: &File,
    resolution: Resolution,
    token: &Cancel,
    progress: &crate::operation_process::Progress,
) -> Result<ResolutionResult, String> {
    let _guard = repo.operation_guard(token)?;
    snapshot.revalidate(repo, file, token)?;
    validate_resolution_path(repo, file)?;
    let sparse = supports_sparse_conflict_paths(&repo.text(&["--version"], token)?)?;
    let mut commands = Vec::new();
    let mut run = |args: &[&str]| {
        let mut command = repo.operation_command();
        command.arg("--literal-pathspecs").args(args);
        if sparse && matches!(args.first(), Some(&"add" | &"rm")) {
            command.arg("--sparse");
        }
        command.arg("--").arg(&file.path);
        let outcome = crate::operation_process::run(&mut command, token, progress, None)
            .map_err(|e| e.to_string());
        let success = outcome
            .as_ref()
            .is_ok_and(|o| o.status.success() && o.stopped.is_none() && o.input_error.is_none());
        commands.push(outcome);
        success
    };
    match resolution {
        Resolution::Accept(side) if file.side(side, snapshot.reversed).is_none() => {
            run(&["rm"]);
        }
        Resolution::Accept(side) => {
            let ours = (side == Side::Yours) != snapshot.reversed;
            if run(&["checkout", if ours { "--ours" } else { "--theirs" }]) {
                run(&["add", "-A", "-f"]);
            }
        }
        Resolution::MarkResolved => {
            run(&["add", "-A", "-f"]);
        }
    }
    let mut refreshed = Snapshot::read(repo, &Cancel::default());
    if refreshed.as_ref().is_ok_and(|s| s.files.is_empty()) {
        if let Err(error) = clear_saved_context(repo, &Cancel::default()) {
            refreshed = Err(error);
        }
    }
    Ok(ResolutionResult {
        commands,
        snapshot: refreshed,
    })
}

// GitVersionSpecialty.ADD_REJECTS_SPARSE_FILES_FOR_CONFLICTS starts at 2.34.
// Accept vendor suffixes such as .windows.1 and (Apple Git-...), not malformed
// versions: silently guessing could select unsupported flags for a write.
fn supports_sparse_conflict_paths(version: &str) -> Result<bool, String> {
    let version = version
        .trim()
        .strip_prefix("git version ")
        .ok_or("Unrecognized Git version")?;
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or("Unrecognized Git version")?;
    let minor = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or("Unrecognized Git version")?;
    Ok((major, minor) >= (2, 34))
}

fn validate_resolution_path(repo: &Repository, file: &File) -> Result<(), String> {
    if file.path.as_os_str().is_empty()
        || file.path.components().any(|c| {
            !matches!(c, Component::Normal(_))
                || c.as_os_str().to_string_lossy().eq_ignore_ascii_case(".git")
        })
    {
        return Err("Invalid conflict path".into());
    }
    if file.stages.iter().flatten().any(|v| v.mode == "160000") {
        return Err(
            "Resolve submodule conflicts in the submodule before staging them externally".into(),
        );
    }
    let mut full = repo.path.clone();
    let count = file.path.components().count();
    for (index, component) in file.path.components().enumerate() {
        full.push(component.as_os_str());
        match std::fs::symlink_metadata(&full) {
            Ok(meta) => {
                let parent = index + 1 < count;
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if parent && meta.file_attributes() & 0x400 != 0 {
                        return Err("Conflict path crosses a reparse point".into());
                    }
                }
                if (parent && (!meta.is_dir() || meta.file_type().is_symlink()))
                    || (!parent && meta.is_dir())
                {
                    return Err("Conflict path is a directory or crosses a symbolic link".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    const OID: &str = "1234567890123456789012345678901234567890";
    #[test]
    fn parses_stage_presence_and_preserves_path_delimiters() {
        let data = format!(
            "100644 {OID} 1\ta\tb\n.txt\0\
            100755 {OID} 2\ta\tb\n.txt\0\
            100644 {OID} 3\tadded.txt\0"
        );
        let files = parse(data.as_bytes()).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, Path::new("a\tb\n.txt"));
        assert!(files[0].side(Side::Theirs, false).is_none());
        assert_eq!(files[0].side(Side::Theirs, true).unwrap().mode, "100755");
        assert!(files[1].stages[0].is_none());
    }
    #[test]
    fn rejects_truncation_duplicate_stages_and_unsafe_paths() {
        let line = format!("100644 {OID} 2\ta\0");
        for data in [
            line.trim_end_matches('\0').to_owned(),
            format!("{line}{line}"),
            format!("100644 {OID} 0\ta\0"),
            format!("100644 {OID} 2\t../a\0"),
            format!("100644 {OID} 2\t/a\0"),
        ] {
            assert!(parse(data.as_bytes()).is_err(), "{data:?}");
        }
    }
}

#[cfg(test)]
mod version_tests {
    use super::supports_sparse_conflict_paths as supported;
    #[test]
    fn sparse_conflicts_version_boundary_and_vendor_builds() {
        for version in ["git version 2.33.9", "git version 1.9.5"] {
            assert!(!supported(version).unwrap());
        }
        for version in [
            "git version 2.34.0",
            "git version 2.47.1.windows.2\n",
            "git version 2.39.5 (Apple Git-154)",
            "git version 3.0.0",
        ] {
            assert!(supported(version).unwrap());
        }
        assert!(supported("unknown").is_err());
        assert!(supported("git version 2.bad.0").is_err());
    }
}
