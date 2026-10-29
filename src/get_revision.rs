//! File-history Get, translated from Rebased 4aec352 GetVersionAction.
//! Restore historical bytes to the current file path, preserving HEAD and index.
use crate::{
    file_history::Revision,
    git::{Cancel, Repository},
    project_files::Project,
};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Component, Path, PathBuf},
};

pub struct Prepared {
    root: PathBuf,
    pub path: PathBuf,
    pub revision: String,
    pub modified: bool,
    expected: Option<PathBuf>,
    bytes: PathBuf,
    _temporary: Temporary,
    head: Option<String>,
    index: Vec<u8>,
}
fn validate(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.components().any(|c| {
            !matches!(c, Component::Normal(_))
                || c.as_os_str().to_string_lossy().eq_ignore_ascii_case(".git")
        })
    {
        return Err("Invalid history file path".into());
    }
    #[cfg(windows)]
    if path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.contains(':') || s.ends_with('.') || s.ends_with(' ')
    }) {
        return Err("Invalid history file path".into());
    }
    Ok(())
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
impl Temporary {
    fn new() -> Result<Self, String> {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir().join(format!("plico-get-{name}"));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
fn disk_path(project: &Project, path: &Path, token: &Cancel) -> Result<Option<PathBuf>, String> {
    let full = project
        .resolve_missing(path, token)
        .map_err(|e| format!("{e:?}"))?;
    match fs::symlink_metadata(&full) {
        Ok(metadata) if metadata.is_file() => Ok(Some(full)),
        Ok(_) => Err("History restore requires a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}
fn disk_matches(
    project: &Project,
    path: &Path,
    expected: Option<&Path>,
    token: &Cancel,
) -> Result<bool, String> {
    match (disk_path(project, path, token)?, expected) {
        (None, None) => Ok(true),
        (Some(actual), Some(expected)) => {
            crate::file_save::same_file_bytes(&actual, expected, token)
                .map_err(|e| format!("{e:?}"))
        }
        _ => Ok(false),
    }
}
fn index(repo: &Repository, path: &Path, token: &Cancel) -> Result<Vec<u8>, String> {
    repo.read(
        &[
            OsStr::new("--literal-pathspecs"),
            OsStr::new("ls-files"),
            OsStr::new("--stage"),
            OsStr::new("-z"),
            OsStr::new("--"),
            path.as_os_str(),
        ],
        token,
    )
}
impl Prepared {
    pub fn read(
        repo: &Repository,
        path: &Path,
        revision: &Revision,
        token: &Cancel,
    ) -> Result<Self, String> {
        validate(path)?;
        validate(&revision.path)?;
        if revision.deleted || !crate::git::is_oid(&revision.id) {
            return Err("No file in the selected revision".into());
        }
        let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
        let head = crate::commit_operation::head(repo, token)?;
        let temporary = Temporary::new()?;
        let expected = if let Some(source) = disk_path(&project, path, token)? {
            let baseline = temporary.0.join("baseline");
            let mut output = fs::File::create(&baseline).map_err(|e| e.to_string())?;
            crate::file_save::copy_stream(&source, &mut output, token)
                .map_err(|e| format!("{e:?}"))?;
            drop(output);
            if !disk_matches(&project, path, Some(&baseline), token)? {
                return Err("File changed while preparing Get Revision".into());
            }
            Some(baseline)
        } else {
            None
        };
        let mut spec = OsString::from(&revision.id);
        spec.push(":");
        spec.push(&revision.path);
        let bytes = temporary.0.join("revision");
        let output = fs::File::create(&bytes).map_err(|e| e.to_string())?;
        let mut command = repo.operation_command();
        command.args([OsStr::new("cat-file"), OsStr::new("blob"), &spec]);
        let outcome = crate::operation_process::run_to_file(
            &mut command,
            token,
            &crate::operation_process::Progress::default(),
            output,
        )
        .map_err(|e| e.to_string())?;
        if !outcome.status.success() || outcome.stopped.is_some() {
            return Err(format!(
                "Could not read history revision: {:?}: {}",
                outcome.stopped,
                String::from_utf8_lossy(&outcome.stderr.bytes())
            ));
        }
        let index = index(repo, path, token)?;
        let status = repo.read(
            &[
                OsStr::new("--literal-pathspecs"),
                OsStr::new("status"),
                OsStr::new("--porcelain=v1"),
                OsStr::new("-z"),
                OsStr::new("--untracked-files=all"),
                OsStr::new("--ignored=matching"),
                OsStr::new("--"),
                path.as_os_str(),
            ],
            token,
        )?;
        Ok(Self {
            root: project.root().into(),
            path: path.into(),
            revision: revision.id.clone(),
            modified: expected.is_some() && !status.is_empty(),
            expected,
            bytes,
            _temporary: temporary,
            head,
            index,
        })
    }
    /// Call only after any modified-file confirmation and document-save barrier.
    /// Failure does not change the index or refs. Disk races use the same
    /// optimistic revalidation guarantees as ordinary editor saves.
    pub fn execute(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let _guard = repo.operation_guard(token)?;
        self.check_current(repo, token)?;
        self.write(repo, token)
    }
    fn check_current(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
        if project.root() != self.root
            || crate::commit_operation::head(repo, token)? != self.head
            || index(repo, &self.path, token)? != self.index
            || !disk_matches(&project, &self.path, self.expected.as_deref(), token)?
        {
            return Err("File or repository changed; reopen Get Revision".into());
        }
        Ok(())
    }
    fn write(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
        match &self.expected {
            Some(expected) => {
                crate::file_save::save_from_file(&project, &self.path, expected, &self.bytes, token)
            }
            None => crate::file_save::create_from_file(&project, &self.path, &self.bytes, token),
        }
        .map_err(|e| format!("Could not restore file revision: {e:?}"))
    }
}

/// Validate the complete selection before the first write; retain per-file race checks.
/// Like Rebased, this is a sequence of file writes, not an atomic filesystem transaction.
pub fn execute_all(files: &[Prepared], repo: &Repository, token: &Cancel) -> Result<(), String> {
    let _guard = repo.operation_guard(token)?;
    for file in files {
        file.check_current(repo, token)?;
    }
    for (completed, file) in files.iter().enumerate() {
        file.check_current(repo, token)
            .and_then(|_| file.write(repo, token))
            .map_err(|error| {
                format!(
                    "Restored {completed}/{} files; {}: {error}",
                    files.len(),
                    file.path.display()
                )
            })?;
    }
    Ok(())
}
