//! File-history Get, translated from Rebased 4aec352 GetVersionAction.
//! Restore historical bytes to the current file path, preserving HEAD and index.
use crate::{
    file_document::Save,
    file_history::Revision,
    git::{Cancel, Repository},
    project_files::Project,
};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

pub struct Prepared {
    root: PathBuf,
    pub path: PathBuf,
    pub revision: String,
    pub modified: bool,
    expected: Option<Arc<[u8]>>,
    bytes: Arc<[u8]>,
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
fn disk(project: &Project, path: &Path, token: &Cancel) -> Result<Option<Arc<[u8]>>, String> {
    let full = project
        .resolve_missing(path, token)
        .map_err(|e| format!("{e:?}"))?;
    match fs::symlink_metadata(full) {
        Ok(_) => project
            .read_bytes(path, token)
            .map(|b| Some(b.into()))
            .map_err(|e| format!("{e:?}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
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
        let expected = disk(&project, path, token)?;
        let mut spec = OsString::from(&revision.id);
        spec.push(":");
        spec.push(&revision.path);
        let size = repo.read(&[OsStr::new("cat-file"), OsStr::new("-s"), &spec], token)?;
        let size: usize = std::str::from_utf8(&size)
            .map_err(|e| e.to_string())?
            .trim()
            .parse()
            .map_err(|_| "Invalid blob size")?;
        if size > crate::project_files::FILE_LIMIT {
            return Err("File exceeds the 5 MiB restore limit".into());
        }
        let bytes = repo.read(&[OsStr::new("cat-file"), OsStr::new("blob"), &spec], token)?;
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
            bytes: bytes.into(),
            head,
            index,
        })
    }
    /// Call only after any modified-file confirmation and document-save barrier.
    /// Failure does not change the index or refs. Disk races use the same
    /// optimistic revalidation guarantees as ordinary editor saves.
    pub fn execute(&self, repo: &Repository, token: &Cancel) -> Result<(), String> {
        let _guard = repo.operation_guard(token)?;
        let project = Project::new(&repo.path).map_err(|e| format!("{e:?}"))?;
        if project.root() != self.root
            || crate::commit_operation::head(repo, token)? != self.head
            || index(repo, &self.path, token)? != self.index
            || disk(&project, &self.path, token)? != self.expected
        {
            return Err("File or repository changed; reopen Get Revision".into());
        }
        match &self.expected {
            Some(expected) => crate::file_save::save(
                &project,
                &self.path,
                &Save {
                    expected: expected.clone(),
                    bytes: self.bytes.clone(),
                },
                token,
            ),
            None => crate::file_save::create(&project, &self.path, &self.bytes, token),
        }
        .map_err(|e| format!("Could not restore file revision: {e:?}"))
    }
}
