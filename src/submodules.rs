//! Direct submodule inventory, informed by Rebased 4aec352 GitModulesFileReader
//! and GitSubmodule.getDirectSubmodules (Apache-2.0). Git parses its own config;
//! the index supplies gitlink identities, including unresolved merge stages.
use crate::git::{Cancel, Repository, is_oid, path_from_bytes};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

/// URLs may contain credentials: do not log or display this structure directly.
#[derive(PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub path: PathBuf,
    pub url: String,
    pub index: IndexState,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexState {
    /// A declaration can exist before its gitlink has been staged.
    NotTracked,
    Commit(String),
    Conflict,
}

/// Read direct declarations only. Nested repositories and working contents are
/// deliberately not opened until requested by the operation/UI.
pub fn list(repo: &Repository, token: &Cancel) -> Result<Vec<Module>, String> {
    let root = worktree_root(repo, token)?;
    let config = root.join(".gitmodules");
    let repo = Repository::new(root);
    match std::fs::symlink_metadata(&config) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
            return Err("Invalid .gitmodules file".into());
        }
        Ok(_) => {}
    }
    let raw = repo.read(
        &[
            OsStr::new("config"),
            OsStr::new("--no-includes"),
            OsStr::new("--null"),
            OsStr::new("--list"),
            OsStr::new("--file"),
            config.as_os_str(),
        ],
        token,
    )?;
    let mut declarations: BTreeMap<String, (Option<PathBuf>, Option<String>)> = BTreeMap::new();
    for entry in raw.split(|b| *b == 0).filter(|v| !v.is_empty()) {
        let Some(separator) = entry.iter().position(|b| *b == b'\n') else {
            continue;
        };
        let key = std::str::from_utf8(&entry[..separator])
            .map_err(|_| "Invalid submodule configuration key")?;
        let Some(key) = key.strip_prefix("submodule.") else {
            continue;
        };
        let Some((name, field)) = key.rsplit_once('.') else {
            continue;
        };
        if !matches!(field, "path" | "url") {
            continue;
        }
        if !declarations.contains_key(name) && declarations.len() >= 4096 {
            return Err("Submodule count exceeds the 4096 limit".into());
        }
        let value = &entry[separator + 1..];
        let declaration = declarations.entry(name.into()).or_default();
        if field == "path" {
            let path = path_from_bytes(value)?;
            if !relative(&path) {
                return Err("Invalid submodule path".into());
            }
            declaration.0 = Some(path);
        } else {
            declaration.1 = Some(
                String::from_utf8(value.to_vec()).map_err(|_| "Invalid submodule URL encoding")?,
            );
        }
    }
    let mut modules: Vec<_> = declarations
        .into_iter()
        .filter_map(|(name, (path, url))| {
            Some(Module {
                name,
                path: path?,
                url: url?,
                index: IndexState::NotTracked,
            })
        })
        .collect();
    if modules.is_empty() {
        return Ok(modules);
    }
    let mut paths = BTreeMap::new();
    for (i, module) in modules.iter().enumerate() {
        if paths.insert(module.path.clone(), i).is_some() {
            return Err("Multiple submodules declare the same path".into());
        }
    }
    // Bound both command-line size (notably Windows' 32K limit) and output.
    // Path byte length conservatively bounds UTF-16 units; reserve room for Git
    // flags, quoting and its executable path. Do not load the whole parent index.
    let declared_paths: Vec<_> = paths.keys().collect();
    let mut offset = 0;
    while offset < declared_paths.len() {
        let mut args = vec![
            OsStr::new("ls-files"),
            OsStr::new("--stage"),
            OsStr::new("-z"),
            OsStr::new("--"),
        ];
        let mut size = 0;
        while let Some(path) = declared_paths.get(offset) {
            let cost = path.as_os_str().len().saturating_mul(2).saturating_add(3);
            if size > 0 && size + cost > 8192 {
                break;
            }
            args.push(path.as_os_str());
            size += cost;
            offset += 1;
        }
        let raw = repo.read(&args, token)?;
        for entry in raw.split(|b| *b == 0).filter(|v| !v.is_empty()) {
            let Some(separator) = entry.iter().position(|b| *b == b'\t') else {
                return Err("Invalid index record".into());
            };
            let path = path_from_bytes(&entry[separator + 1..])?;
            let Some(&i) = paths.get(&path) else { continue };
            let header =
                std::str::from_utf8(&entry[..separator]).map_err(|_| "Invalid index record")?;
            let fields: Vec<_> = header.split(' ').collect();
            if fields.len() != 3 || !is_oid(fields[1]) {
                return Err("Invalid gitlink index record".into());
            }
            if fields[2] != "0" {
                modules[i].index = IndexState::Conflict;
            } else if fields[0] == "160000" {
                modules[i].index = IndexState::Commit(fields[1].into());
            }
        }
    }
    Ok(modules)
}
fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

pub(crate) fn worktree_root(repo: &Repository, token: &Cancel) -> Result<PathBuf, String> {
    let raw = repo.read(
        &[OsStr::new("rev-parse"), OsStr::new("--show-toplevel")],
        token,
    )?;
    let root = raw.strip_suffix(b"\n").unwrap_or(&raw);
    path_from_bytes(root.strip_suffix(b"\r").unwrap_or(root))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Checkout {
    /// Occupied directories must not be treated as disposable placeholders.
    Uninitialized { occupied: bool },
    Initialized {
        head: String,
        /// Full ref; None means detached, not an empty/unborn repository.
        branch: Option<String>,
        tracked_changes: bool,
        untracked: bool,
        conflicts: bool,
    },
}

pub(crate) enum Located {
    Uninitialized { occupied: bool },
    Initialized(Repository),
}

/// Probe one selected direct submodule. Errors remain errors: Git must not walk
/// up from an absent/broken child and accidentally report the parent's HEAD.
/// This is a read snapshot, not authorization for a later unchecked write.
pub(crate) fn initialized_repository(
    repo: &Repository,
    path: &Path,
    token: &Cancel,
) -> Result<Located, String> {
    if !relative(path) {
        return Err("Invalid submodule path".into());
    }
    let root = worktree_root(repo, token)?;
    let project = crate::project_files::Project::new(&root).map_err(|e| format!("{e:?}"))?;
    let directory = project
        .resolve_missing(path, token)
        .map_err(|e| format!("{e:?}"))?;
    match std::fs::symlink_metadata(directory.join(".git")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let occupied = match std::fs::read_dir(&directory) {
                Ok(mut entries) => entries
                    .next()
                    .transpose()
                    .map_err(|e| e.to_string())?
                    .is_some(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => return Err(e.to_string()),
            };
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            return Ok(Located::Uninitialized { occupied });
        }
        Err(e) => return Err(e.to_string()),
        Ok(m) if m.file_type().is_symlink() || !(m.is_file() || m.is_dir()) => {
            return Err("Invalid submodule Git directory".into());
        }
        Ok(_) => {}
    }
    let child = Repository::new(&directory);
    let actual = worktree_root(&child, token)?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if actual != directory.canonicalize().map_err(|e| e.to_string())? {
        return Err("Submodule repository root does not match its declared path".into());
    }
    Ok(Located::Initialized(child))
}

pub fn inspect(repo: &Repository, path: &Path, token: &Cancel) -> Result<Checkout, String> {
    let child = match initialized_repository(repo, path, token)? {
        Located::Initialized(child) => child,
        Located::Uninitialized { occupied } => return Ok(Checkout::Uninitialized { occupied }),
    };
    let head = child
        .text(&["rev-parse", "--verify", "HEAD^{commit}"], token)?
        .trim()
        .to_owned();
    if !is_oid(&head) {
        return Err("Invalid submodule HEAD".into());
    }
    let reference = child.text(&["rev-parse", "--symbolic-full-name", "HEAD"], token)?;
    let reference = reference.trim_end_matches(['\r', '\n']);
    let branch = if reference == "HEAD" {
        None
    } else if reference.starts_with("refs/heads/") {
        Some(reference.to_owned())
    } else {
        return Err("Invalid submodule branch".into());
    };
    let changes = child.local_changes(token)?;
    Ok(Checkout::Initialized {
        head,
        branch,
        tracked_changes: changes
            .iter()
            .any(|c| c.area != crate::git::Area::Untracked),
        untracked: changes
            .iter()
            .any(|c| c.area == crate::git::Area::Untracked),
        conflicts: changes.iter().any(|c| c.area == crate::git::Area::Conflict),
    })
}
