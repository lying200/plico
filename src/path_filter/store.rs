//! Per-project Log path/root preferences, independent of repository metadata.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

const MAX_BYTES: u64 = 512 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredPath {
    pub path: PathBuf,
    pub directory: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub roots: Option<BTreeSet<PathBuf>>,
    pub paths: Vec<StoredPath>,
}
impl State {
    /// Rebased drops unavailable saved roots on restore, and falls back to all
    /// if none remain. Do not run this on a live user's explicitly empty set.
    pub fn restore_roots(&mut self, available: &[PathBuf]) {
        if let Some(selected) = &mut self.roots {
            let identities = selected
                .iter()
                .map(|path| crate::recent_projects::identity(path))
                .collect::<BTreeSet<_>>();
            // Use the current catalog's spelling as query keys. Saved Windows
            // paths may differ in casing or use a verbatim path prefix.
            *selected = available
                .iter()
                .filter(|path| identities.contains(&crate::recent_projects::identity(path)))
                .cloned()
                .collect();
            if selected.is_empty() {
                self.roots = None;
            }
        }
    }
}
#[derive(Clone)]
pub struct Store {
    file: PathBuf,
    project: String,
    revision: Arc<AtomicU64>,
    writer: Arc<Mutex<()>>,
}
impl Store {
    pub fn new(file: PathBuf, primary: &Path) -> Self {
        Self {
            file,
            project: crate::recent_projects::identity(primary),
            revision: Arc::new(AtomicU64::new(0)),
            writer: Arc::new(Mutex::new(())),
        }
    }
    /// Path computation only: safe on the UI thread.
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
                .join("GitReview/log-filters")
                .join(format!("{hash}.json")),
            primary,
        ))
    }
    pub fn load(&self) -> io::Result<State> {
        let file = match File::open(&self.file) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(State::default()),
            Err(error) => return Err(error),
        };
        let mut bytes = vec![];
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid());
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if value["version"] != 1 || value["project"].as_str() != Some(self.project.as_str()) {
            return Err(invalid());
        }
        let roots = if value["roots"].is_null() {
            None
        } else {
            let values = value["roots"].as_array().ok_or_else(invalid)?;
            if values.len() > 4096 {
                return Err(invalid());
            }
            Some(
                values
                    .iter()
                    .map(path)
                    .collect::<io::Result<BTreeSet<_>>>()?,
            )
        };
        let values = value["paths"].as_array().ok_or_else(invalid)?;
        if values.len() > 128 {
            return Err(invalid());
        }
        let paths = values
            .iter()
            .map(|value| {
                Ok(StoredPath {
                    path: path(&value["path"])?,
                    directory: value["directory"].as_bool().ok_or_else(invalid)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let state = State { roots, paths };
        validate(&state)?;
        Ok(state)
    }
    /// Reserve synchronously with the UI edit, then perform save in a worker.
    /// Older workers cannot overwrite a newer requested selection.
    pub fn reserve(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst) + 1
    }
    pub fn save(&self, revision: u64, state: &State) -> io::Result<bool> {
        validate(state)?;
        let _guard = self
            .writer
            .lock()
            .map_err(|_| io::Error::other("Log filter writer failed"))?;
        if self.revision.load(Ordering::SeqCst) != revision {
            return Ok(false);
        }
        let roots = state
            .roots
            .as_ref()
            .map(|roots| {
                roots
                    .iter()
                    .map(|p| p.to_str().ok_or_else(invalid))
                    .collect::<io::Result<Vec<_>>>()
            })
            .transpose()?;
        let paths = state
            .paths
            .iter()
            .map(|entry| {
                Ok(json!({
                    "path": entry.path.to_str().ok_or_else(invalid)?,
                    // Retain known directory identity even if it has been deleted.
                    "directory": entry.directory || entry.path.is_dir(),
                }))
            })
            .collect::<io::Result<Vec<_>>>()?;
        let bytes = serde_json::to_vec(
            &json!({"version": 1, "project": self.project, "roots": roots, "paths": paths}),
        )?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid());
        }
        let parent = self.file.parent().ok_or_else(invalid)?;
        fs::create_dir_all(parent)?;
        let mut random = [0; 16];
        getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
        let temporary = parent.join(format!(".filters-{:032x}.tmp", u128::from_le_bytes(random)));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            if self.revision.load(Ordering::SeqCst) != revision {
                return Ok(false);
            }
            fs::rename(&temporary, &self.file)?;
            Ok(true)
        })();
        if !matches!(result, Ok(true)) {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid saved Log path filter")
}
fn path(value: &Value) -> io::Result<PathBuf> {
    let path = PathBuf::from(value.as_str().ok_or_else(invalid)?);
    valid_path(&path)?;
    Ok(path)
}
fn valid_path(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.to_str().is_none_or(|value| value.contains('\0'))
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(invalid());
    }
    Ok(())
}
fn validate(state: &State) -> io::Result<()> {
    if state.roots.as_ref().is_some_and(|roots| roots.len() > 4096)
        || state.paths.len() > 128
        || (state.roots.is_some() && !state.paths.is_empty())
    {
        return Err(invalid());
    }
    for root in state.roots.iter().flatten() {
        valid_path(root)?;
    }
    for entry in &state.paths {
        valid_path(&entry.path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                    "plico-log-filter-store-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )))
        }
        fn store(&self) -> Store {
            Store::new(self.0.join("filters.json"), &self.0.join("project"))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn saved_structure_survives_missing_directory_and_wrong_project_is_rejected() {
        let fixture = Fixture::new();
        let store = fixture.store();
        assert_eq!(store.load().unwrap(), State::default());
        let state = State {
            roots: None,
            paths: vec![StoredPath {
                path: fixture.0.join("deleted"),
                directory: true,
            }],
        };
        assert!(store.save(store.reserve(), &state).unwrap());
        assert_eq!(store.load().unwrap(), state);
        let other = Store::new(
            fixture.0.join("filters.json"),
            &fixture.0.join("other-project"),
        );
        assert!(other.load().is_err());
    }
    #[test]
    fn restore_prunes_roots_but_does_not_change_live_empty_selection() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let a = fixture.0.join("a");
        let b = fixture.0.join("b");
        let state = State {
            roots: Some([a.clone(), b.clone()].into()),
            paths: vec![],
        };
        store.save(store.reserve(), &state).unwrap();
        let mut restored = store.load().unwrap();
        restored.restore_roots(&[b.clone()]);
        assert_eq!(restored.roots, Some([b].into()));
        restored.restore_roots(&[]);
        assert!(restored.roots.is_none());
        let empty = State {
            roots: Some(BTreeSet::new()),
            paths: vec![],
        };
        store.save(store.reserve(), &empty).unwrap();
        let mut restored = store.load().unwrap();
        assert_eq!(restored, empty);
        restored.restore_roots(&[a]);
        assert!(restored.roots.is_none());
    }
    #[test]
    fn restored_roots_use_platform_identity_and_current_catalog_spelling() {
        let available = PathBuf::from("C:/Projects/Repo");
        let mut state = State {
            roots: Some([PathBuf::from("c:/projects/repo")].into()),
            paths: vec![],
        };
        state.restore_roots(&[available.clone()]);
        if cfg!(windows) {
            assert_eq!(state.roots, Some([available].into()));
        } else {
            assert!(state.roots.is_none());
        }
    }
    #[test]
    fn obsolete_and_invalid_writes_preserve_the_latest_complete_record() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let old = store.reserve();
        let latest = store.reserve();
        let state = State {
            roots: None,
            paths: vec![StoredPath {
                path: fixture.0.join("file"),
                directory: false,
            }],
        };
        assert!(store.save(latest, &state).unwrap());
        assert!(!store.save(old, &State::default()).unwrap());
        let mut invalid_state = state.clone();
        invalid_state.roots = Some(BTreeSet::new());
        assert!(store.save(store.reserve(), &invalid_state).is_err());
        assert_eq!(store.load().unwrap(), state);
        fs::write(&store.file, b"{truncated").unwrap();
        assert!(store.load().is_err());
        fs::write(&store.file, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        assert!(store.load().is_err());
    }
}
