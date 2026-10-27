//! Project startup check, following Rebased GitShallowRepositoryCheck/RunOnceUtil.
//! No repository mutation: the one-time marker lives in application preferences.
use crate::git::{Cancel, Repository};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub struct Store(PathBuf);
impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn for_project(project: &Path) -> Option<Self> {
        if !project.is_absolute() {
            return None;
        }
        #[cfg(windows)]
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        #[cfg(not(windows))]
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
        let hash = format!(
            "{:x}",
            Sha256::digest(crate::recent_projects::identity(project).as_bytes())
        );
        Some(Self(base?.join("GitReview/unshallow-checked").join(hash)))
    }

    /// Atomic across windows/processes. Even a complete/no-remote repository
    /// consumes its startup check, as in the upstream project-level RunOnce.
    pub fn check(&self, repository: &Repository, token: &Cancel) -> Result<bool, String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let parent = self.0.parent().ok_or("Invalid startup check path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.0)
        {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
            Err(e) => return Err(e.to_string()),
        }
        if !repository.is_shallow(token)? {
            return Ok(false);
        }
        let targets = repository.pull_targets(token)?;
        Ok(crate::fetch::default_remote(
            &targets.remotes,
            targets
                .tracked
                .as_ref()
                .map(|target| target.remote.as_str()),
        )
        .is_some())
    }
}
