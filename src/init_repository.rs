//! Rebased 4aec352 GitInit / GitImpl.init: initialize the selected folder,
//! retaining Git's default branch and templates. No staging or initial commit.
use crate::{
    git::{Cancel, Repository},
    operation_process::{self, Outcome, Progress},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Prepared {
    directory: PathBuf,
    git_marker: Option<PathBuf>,
}
impl Prepared {
    /// Blocking filesystem inspection; call on a worker, including for WSL paths.
    pub fn read(path: &Path, token: &Cancel) -> Result<Self, String> {
        check_cancel(token)?;
        let directory = path.canonicalize().map_err(|e| e.to_string())?;
        if !directory.is_dir() {
            return Err("Choose a directory".into());
        }
        let git_marker = marker(&directory, token)?;
        Ok(Self {
            directory,
            git_marker,
        })
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn needs_confirmation(&self) -> bool {
        self.git_marker.is_some()
    }
}

pub fn execute(
    prepared: &Prepared,
    confirmed_existing: bool,
    token: &Cancel,
    progress: &Progress,
) -> Result<Outcome, String> {
    check_cancel(token)?;
    let current = Prepared::read(&prepared.directory, token)?;
    if current.directory != prepared.directory || current.git_marker != prepared.git_marker {
        return Err("Repository changed; choose the directory again".into());
    }
    if prepared.needs_confirmation() && !confirmed_existing {
        return Err("The selected directory is already under Git".into());
    }
    let repo = Repository::new(&prepared.directory);
    let mut command = repo.operation_command();
    command.arg("init");
    operation_process::run(&mut command, token, progress, None).map_err(|e| e.to_string())
}

fn check_cancel(token: &Cancel) -> Result<(), String> {
    if token.cancelled() {
        Err("Cancelled".into())
    } else {
        Ok(())
    }
}

fn marker(directory: &Path, token: &Cancel) -> Result<Option<PathBuf>, String> {
    for parent in directory.ancestors() {
        check_cancel(token)?;
        let path = parent.join(".git");
        match fs::symlink_metadata(&path) {
            // Also warn for a malformed/dangling marker instead of silently
            // treating an existing Git configuration as a new directory.
            Ok(_) => return Ok(Some(path)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(None)
}
