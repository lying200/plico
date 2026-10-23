//! Selected-file rollback translated from Rebased GitRollbackEnvironment (4aec352).
//! Preparation is read-only; execution revalidates and then completes without cancellation.
use crate::{
    commit_changes::{File, Snapshot},
    git::{Area, Cancel, Change, Repository},
    operation_process::{self, Progress},
};
use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    fs,
    hash::Hasher,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub struct Prepared {
    pub head: Option<String>,
    pub changes: Vec<Change>,
    fingerprints: Vec<Result<Vec<(PathBuf, Fingerprint)>, String>>,
    index: Vec<u8>,
}
#[derive(PartialEq, Eq)]
enum Fingerprint {
    Missing,
    File(u64, u64),
    Link(PathBuf),
}

pub fn prepare(repo: &Repository, cancel: &Cancel) -> Result<Prepared, String> {
    check_state(repo, cancel)?;
    let snapshot = Snapshot::load(repo, cancel)?;
    let changes: Vec<_> = snapshot
        .files
        .iter()
        .filter_map(|f| match f {
            File::Tracked(_) => Some(f.change()),
            _ => None,
        })
        .collect();
    if changes.len() > 4096 {
        return Err("Too many changes to roll back; choose a smaller repository scope".into());
    }
    let mut fingerprints = Vec::new();
    for change in &changes {
        // The dialog contains all candidates, not only the eventual selection.
        // Retain a candidate's error without blocking unrelated valid files.
        let values = paths(change)
            .into_iter()
            .map(|path| fingerprint(repo, &path, cancel).map(|value| (path, value)))
            .collect();
        if cancel.cancelled() {
            return Err("Cancelled".into());
        }
        fingerprints.push(values);
    }
    let index = index_state(repo, &changes, cancel)?;
    Ok(Prepared {
        head: snapshot.head,
        changes,
        fingerprints,
        index,
    })
}
fn check_state(repo: &Repository, cancel: &Cancel) -> Result<(), String> {
    let state = repo.operation_state(cancel)?;
    if state.in_progress() || !state.conflicts.is_empty() {
        return Err("Finish the current Git operation before rolling back changes".into());
    }
    if crate::commit_recovery::pending(&repo.editor_directory(cancel)?)?.is_some() {
        return Err("Restore staged changes before rolling back".into());
    }
    Ok(())
}
fn paths(change: &Change) -> BTreeSet<PathBuf> {
    change
        .old_path
        .iter()
        .cloned()
        .chain(std::iter::once(change.path.clone()))
        .collect()
}
fn validate(repo: &Repository, path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || path
            .components()
            .any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case(".git"))
    {
        return Err("Invalid rollback path".into());
    }
    let mut full = repo.path.clone();
    for component in path.components() {
        full.push(component.as_os_str());
        if full == repo.path.join(path) {
            break;
        }
        match fs::symlink_metadata(&full) {
            Ok(m) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return Err("Rollback path crosses a reparse point".into());
                    }
                }
                if m.file_type().is_symlink() || !m.is_dir() {
                    return Err("Rollback path crosses a non-directory or symbolic link".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(full)
}
fn fingerprint(repo: &Repository, path: &Path, cancel: &Cancel) -> Result<Fingerprint, String> {
    let full = validate(repo, path)?;
    let m = match fs::symlink_metadata(&full) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Fingerprint::Missing),
        Err(e) => return Err(e.to_string()),
    };
    if m.file_type().is_symlink() {
        return fs::read_link(full)
            .map(Fingerprint::Link)
            .map_err(|e| e.to_string());
    }
    if !m.is_file() {
        return Err(format!(
            "Directory/submodule rollback is not supported: {}",
            path.display()
        ));
    }
    let mut file = fs::File::open(full).map_err(|e| e.to_string())?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let mut len = 0;
    let mut buf = [0; 65536];
    loop {
        if cancel.cancelled() {
            return Err("Cancelled".into());
        }
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.write(&buf[..n]);
        len += n as u64;
    }
    Ok(Fingerprint::File(len, hash.finish()))
}
fn index_state(repo: &Repository, changes: &[Change], cancel: &Cancel) -> Result<Vec<u8>, String> {
    if changes.is_empty() {
        return Ok(vec![]);
    }
    let paths: BTreeSet<_> = changes.iter().flat_map(paths).collect();
    let prefix = ["--literal-pathspecs", "ls-files", "--stage", "-z", "--"]
        .map(OsString::from)
        .to_vec();
    let mut args = prefix.clone();
    let mut size = 0;
    let mut result = Vec::new();
    for path in paths {
        let length = path.as_os_str().len();
        if size > 0 && (size + length > 8000 || args.len() >= prefix.len() + 64) {
            read_index_chunk(repo, &args, cancel, &mut result)?;
            args = prefix.clone();
            size = 0;
        }
        args.push(path.into_os_string());
        size += length + 3;
    }
    if args.len() > prefix.len() {
        read_index_chunk(repo, &args, cancel, &mut result)?;
    }
    Ok(result)
}
fn read_index_chunk(
    repo: &Repository,
    args: &[OsString],
    cancel: &Cancel,
    result: &mut Vec<u8>,
) -> Result<(), String> {
    let bytes = repo.read(
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        cancel,
    )?;
    if result.len() + bytes.len() > 8 * 1024 * 1024 {
        return Err("Rollback index metadata exceeds its budget".into());
    }
    result.extend(bytes);
    Ok(())
}
/// Any error may follow partial writes: caller must refresh both index and documents.
/// Only indices explicitly checked in the confirmation dialog are acted on.
pub fn execute(
    repo: &Repository,
    prepared: &Prepared,
    selected: &BTreeSet<usize>,
    delete_added: bool,
) -> Result<(), String> {
    if selected.is_empty() || selected.iter().any(|i| *i >= prepared.changes.len()) {
        return Err("Select changes to roll back".into());
    }
    let token = Cancel::default();
    let _guard = repo.operation_guard(&token)?;
    check_state(repo, &token)?;
    if crate::commit_operation::head(repo, &token)? != prepared.head {
        return Err("HEAD changed; reopen Rollback Changes".into());
    }
    if index_state(repo, &prepared.changes, &token)? != prepared.index {
        return Err("Index changed; reopen Rollback Changes".into());
    }
    let mut unindex = BTreeSet::new();
    let mut revert = BTreeSet::new();
    let mut moved = BTreeSet::new();
    let mut added = BTreeSet::new();
    for i in selected {
        let change = &prepared.changes[*i];
        let fingerprints = prepared.fingerprints[*i].as_ref().map_err(Clone::clone)?;
        for (path, expected) in fingerprints {
            if fingerprint(repo, path, &token)? != *expected {
                return Err(format!(
                    "File changed; reopen Rollback Changes: {}",
                    path.display()
                ));
            }
        }
        if change.area != Area::WorkingTree {
            return Err("Only tracked changes can be rolled back".into());
        }
        match change.status.as_str() {
            "A" => {
                unindex.insert(change.path.clone());
                if delete_added {
                    added.insert(change.path.clone());
                }
            }
            "R" => {
                let old = change.old_path.clone().ok_or("Rename source is missing")?;
                if old != change.path && fs::symlink_metadata(repo.path.join(&old)).is_ok() {
                    return Err(
                        "Rename source was recreated; roll back manually to preserve it".into(),
                    );
                }
                revert.insert(old);
                unindex.insert(change.path.clone());
                moved.insert(change.path.clone());
            }
            "M" | "T" | "D" => {
                revert.insert(change.path.clone());
            }
            _ => return Err("Unsupported rollback change".into()),
        }
    }
    if !revert.is_empty() && prepared.head.is_none() {
        return Err("Rollback base is missing".into());
    }
    run_paths(
        repo,
        &[
            "rm",
            "--cached",
            "-f",
            "--ignore-unmatch",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
        &unindex,
        &token,
    )?;
    for path in moved {
        remove(repo, &path)?;
    }
    if let Some(head) = &prepared.head {
        run_paths(
            repo,
            &[
                "checkout",
                head,
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            &revert,
            &token,
        )?;
    }
    // Match RollbackWorker: delete newly added files only after Git succeeded.
    for path in added {
        remove(repo, &path)?;
    }
    Ok(())
}
fn remove(repo: &Repository, path: &Path) -> Result<(), String> {
    let full = validate(repo, path)?;
    match fs::remove_file(full) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
fn run_paths(
    repo: &Repository,
    args: &[&str],
    paths: &BTreeSet<PathBuf>,
    token: &Cancel,
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut input = Vec::new();
    for path in paths {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            input.extend_from_slice(path.as_os_str().as_bytes());
        }
        #[cfg(not(unix))]
        {
            input.extend_from_slice(path.to_str().ok_or("Non-Unicode rollback path")?.as_bytes());
        }
        input.push(0);
    }
    let out = operation_process::run_with_input(
        repo.operation_command()
            .arg(OsStr::new("--literal-pathspecs"))
            .args(args),
        token,
        &Progress::default(),
        None,
        &input,
    )
    .map_err(|e| e.to_string())?;
    if !out.status.success() || out.stopped.is_some() || out.input_error.is_some() {
        return Err(format!(
            "Rollback failed: {}{}",
            String::from_utf8_lossy(&out.stderr.bytes()),
            out.input_error.map(|e| e.to_string()).unwrap_or_default()
        ));
    }
    Ok(())
}
