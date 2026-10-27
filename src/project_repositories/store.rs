//! Per-user project mappings. Git metadata and working trees remain untouched.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read, Write},
    path::Path,
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct Store {
    file: PathBuf,
    project: String,
}
impl Store {
    /// Explicit file location makes persistence independently testable.
    pub fn new(file: PathBuf, primary: &Path) -> Self {
        Self {
            file,
            project: crate::recent_projects::identity(primary),
        }
    }

    /// Computes a path only; no filesystem access on the UI thread.
    pub fn for_project(primary: &Path) -> Option<Self> {
        if !primary.is_absolute() {
            return None;
        }
        #[cfg(windows)]
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        #[cfg(not(windows))]
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
        let key = crate::recent_projects::identity(primary);
        let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
        Some(Self::new(
            base?
                .join("GitReview/project-roots")
                .join(format!("{hash}.json")),
            primary,
        ))
    }

    fn read(&self) -> io::Result<Vec<Repository>> {
        let file = match File::open(&self.file) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid());
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if value["version"] != 1 || value["project"].as_str() != Some(&self.project) {
            return Err(invalid());
        }
        let roots = value["roots"].as_array().ok_or_else(invalid)?;
        if roots.len() > 4096 {
            return Err(invalid());
        }
        roots
            .iter()
            .map(|value| {
                let path = PathBuf::from(value.as_str().ok_or_else(invalid)?);
                if !path.is_absolute() || path.as_os_str().to_string_lossy().contains('\0') {
                    return Err(invalid());
                }
                Ok(Repository::new(path))
            })
            .collect()
    }

    /// Reload on each scan so independent windows can learn each other's roots.
    pub(super) fn load_into(&self, mappings: &mut Mappings) -> io::Result<()> {
        let roots = self.read()?;
        merge(&mut mappings.registered, roots)
    }

    /// Merge under a cross-process lock, then atomically replace the record.
    /// Never overwrite unreadable/corrupt records or truncate an oversized set.
    pub(super) fn save(&self, mappings: &Mappings, token: &Cancel) -> io::Result<()> {
        let parent = self.file.parent().ok_or_else(invalid)?;
        fs::create_dir_all(parent)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.file.with_extension("lock"))?;
        let start = Instant::now();
        loop {
            if token.cancelled() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"));
            }
            match lock.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) if start.elapsed() < Duration::from_secs(2) => {
                    std::thread::sleep(Duration::from_millis(25))
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "Project mappings are busy",
                    ));
                }
                Err(TryLockError::Error(error)) => return Err(error),
            }
        }
        let old = self.read()?;
        let mut roots = old.clone();
        merge(&mut roots, mappings.registered.clone())?;
        if roots
            .iter()
            .map(|r| &r.path)
            .eq(old.iter().map(|r| &r.path))
        {
            return Ok(());
        }
        let paths = roots
            .iter()
            .map(|r| r.path.to_str().ok_or_else(invalid))
            .collect::<io::Result<Vec<_>>>()?;
        let bytes = serde_json::to_vec(
            &serde_json::json!({"version": 1, "project": self.project, "roots": paths}),
        )?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid());
        }
        let mut random = [0; 16];
        getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
        let temporary = parent.join(format!(".roots-{:032x}.tmp", u128::from_le_bytes(random)));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            if token.cancelled() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"));
            }
            fs::rename(&temporary, &self.file)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid project repository mappings",
    )
}
fn merge(target: &mut Vec<Repository>, roots: Vec<Repository>) -> io::Result<()> {
    let mut result = target.clone();
    let mut keys = result
        .iter()
        .map(|r| crate::recent_projects::identity(&r.path))
        .collect::<BTreeSet<_>>();
    for root in roots {
        if keys.insert(crate::recent_projects::identity(&root.path)) {
            result.push(root);
        }
    }
    if result.len() > 4096 {
        return Err(invalid());
    }
    *target = result;
    Ok(())
}
