//! Bounded filesystem evidence for the administrative directories a watcher owns.
use super::git_paths::GitPaths;
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(PartialEq, Eq)]
enum Entry {
    Missing,
    File(Vec<u8>),
    Directory(SystemTime),
}
impl Entry {
    fn read(path: &Path) -> io::Result<Self> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::Missing),
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() {
            return Err(io::Error::other("Indirect repository binding"));
        }
        if metadata.is_dir() {
            // Directory mtime changes on every index.lock replacement. Creation
            // time instead detects replacement; unsupported platforms stay conservative.
            return Ok(Self::Directory(metadata.created()?));
        }
        if !metadata.is_file() || metadata.len() > 8192 {
            return Err(io::Error::other("Unsupported repository binding"));
        }
        let mut bytes = Vec::new();
        fs::File::open(path)?.take(8193).read_to_end(&mut bytes)?;
        if bytes.len() > 8192 {
            return Err(io::Error::other("Repository binding exceeds limit"));
        }
        Ok(Self::File(bytes))
    }
}

pub(super) struct Binding {
    entries: Vec<(PathBuf, Entry)>,
    pub simple_local: bool,
}
impl Binding {
    pub fn read(root: &Path, paths: &GitPaths) -> io::Result<Self> {
        let [directory, common] = paths.directories();
        let mut entries = Vec::new();
        for path in [
            root.to_owned(),
            root.join(".git"),
            directory.to_owned(),
            common.to_owned(),
            directory.join("commondir"),
        ] {
            if !entries.iter().any(|(old, _)| old == &path) {
                entries.push((path.clone(), Entry::read(&path)?));
            }
        }
        let commondir = entries
            .iter()
            .find(|(path, _)| path == &directory.join("commondir"))
            .unwrap();
        let simple_local = matches!(commondir.1, Entry::Missing)
            && entries.iter().any(|(path, entry)| {
                path == &root.join(".git") && !matches!(entry, Entry::Missing)
            })
            && [root, directory, common]
                .iter()
                .all(|path| !super::project::is_network_path(&path.to_string_lossy()));
        Ok(Self {
            entries,
            simple_local,
        })
    }
    pub fn current(&self) -> bool {
        self.entries
            .iter()
            .all(|(path, previous)| Entry::read(path).is_ok_and(|next| &next == previous))
    }
}
