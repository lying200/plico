//! Rebased GitRebaseEditorApp / GitSimpleEditorHandler bridge (4aec352, Apache-2.0).
//! Helper sends a file path; the application edits it and returns an exit code.
use crate::auth_request::{Kind, Pending};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};
const LIMIT: u64 = 1024 * 1024;

pub fn command(helper: &Path, sequence: bool) -> io::Result<OsString> {
    if !helper.is_absolute() || !helper.is_file() {
        return Err(io::ErrorKind::NotFound.into());
    }
    let path = helper.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    #[cfg(windows)]
    let path = path.replace('\\', "/");
    let quoted = format!(
        "'{}'{}",
        path.replace('\'', "'\\''"),
        if sequence { " --sequence" } else { "" }
    );
    Ok(quoted.into())
}

pub fn helper(path: &Path, sequence: bool) -> io::Result<i32> {
    let path = path.canonicalize()?;
    let path = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    match crate::askpass::request(if sequence { 4 } else { 3 }, path)?.as_deref() {
        Some("0") => Ok(0),
        Some("1") | None => Ok(1),
        _ => Err(io::ErrorKind::InvalidData.into()),
    }
}

/// Dropping the editor cancels editing (exit 1), not the entire operation token.
/// The caller then handles Git's resulting failure/state as Rebased does.
pub struct Document {
    pending: Option<Pending>,
    path: PathBuf,
    git_directory: PathBuf,
    original: String,
    pub sequence: bool,
}
impl Document {
    pub fn open(pending: Pending, git_dir: &Path) -> io::Result<Self> {
        Self::open_in_roots(pending, std::slice::from_ref(&git_dir.to_path_buf()))
    }
    /// Route an operation-scoped request only to its captured Git directories.
    /// Prefix containment alone is insufficient: parent .git/modules/... must
    /// not implicitly authorize every descendant repository or arbitrary file.
    pub fn open_in_roots(pending: Pending, git_dirs: &[PathBuf]) -> io::Result<Self> {
        let mut document = Self {
            sequence: pending.kind == Kind::SequenceEditor,
            path: PathBuf::new(),
            git_directory: PathBuf::new(),
            original: String::new(),
            pending: Some(pending),
        };
        let pending = document.pending.as_ref().unwrap();
        if !matches!(pending.kind, Kind::CommitEditor | Kind::SequenceEditor) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if git_dirs.len() > 4096 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let path = Path::new(&pending.prompt).canonicalize()?;
        let mut selected = None;
        for directory in git_dirs {
            let root = directory.canonicalize()?;
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            let allowed = if document.sequence {
                relative == Path::new("rebase-merge/git-rebase-todo")
            } else {
                [
                    "COMMIT_EDITMSG",
                    "MERGE_MSG",
                    "SQUASH_MSG",
                    "rebase-merge/message",
                ]
                .iter()
                .any(|p| relative == Path::new(p))
            };
            if allowed {
                selected = Some(root);
                break;
            }
        }
        document.git_directory = selected.ok_or(io::ErrorKind::PermissionDenied)?;
        document.original = read_bounded(&path)?;
        document.path = path;
        Ok(document)
    }
    pub(crate) fn git_directory(&self) -> &Path {
        &self.git_directory
    }
    pub fn text(&self) -> &str {
        &self.original
    }
    /// Forward an unmatched automatic editor request to the ordinary UI without
    /// writing the stale text originally read by this document back over it.
    pub(crate) fn forward(
        mut self,
        requests: &crate::auth_request::Requests,
        stop: &crate::git::Cancel,
    ) -> io::Result<()> {
        let pending = self.pending.take().ok_or(io::ErrorKind::BrokenPipe)?;
        let reply = requests.ask_until(pending.kind, pending.prompt.clone(), stop);
        let accepted = reply.as_deref() == Ok("0");
        pending
            .answer(if accepted { "0" } else { "1" }.into())
            .map_err(|_| io::ErrorKind::Interrupted)?;
        if accepted {
            Ok(())
        } else {
            Err(io::ErrorKind::Interrupted.into())
        }
    }
    pub fn save(&mut self, text: &str) -> io::Result<()> {
        if self.pending.is_none() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if self.pending.as_ref().unwrap().is_cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if text.len() as u64 > LIMIT || text.contains('\0') {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut file = OpenOptions::new().read(true).write(true).open(&self.path)?;
        let mut current = String::new();
        (&mut file).take(LIMIT + 1).read_to_string(&mut current)?;
        if current != self.original {
            return Err(io::Error::other("Editor file changed while editing"));
        }
        file.rewind()?;
        file.write_all(text.as_bytes())?;
        file.set_len(text.len() as u64)?;
        file.flush()?;
        self.pending
            .take()
            .unwrap()
            .answer("0".into())
            .map_err(|_| io::ErrorKind::Interrupted.into())
    }
}
impl Drop for Document {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take() {
            let _ = pending.answer("1".into());
        }
    }
}
fn read_bounded(path: &Path) -> io::Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(io::ErrorKind::InvalidData.into());
    }
    String::from_utf8(bytes).map_err(|_| io::ErrorKind::InvalidData.into())
}
