//! External status dependencies that do not change effective config values.
use crate::git::{Cancel, Repository, path_from_bytes};
use sha2::{Digest, Sha256};
use std::{ffi::OsStr, fs, io::Read, path::PathBuf};

#[derive(Default)]
pub(super) struct IgnoreRules {
    paths: Option<Vec<PathBuf>>,
    fingerprint: Option<[u8; 32]>,
    pending: bool,
}
impl IgnoreRules {
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn acknowledge(&mut self) {
        self.pending = false;
    }
    pub fn poll(&mut self, repo: &Repository, token: &Cancel, resolve: bool) -> Result<(), String> {
        let paths = if resolve || self.paths.is_none() {
            resolve_paths(repo, token).ok()
        } else {
            self.paths.clone()
        };
        let fingerprint = paths.as_ref().and_then(|paths| digest(paths, token).ok());
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        self.pending |=
            fingerprint.is_none() || paths != self.paths || fingerprint != self.fingerprint;
        self.paths = paths;
        self.fingerprint = fingerprint;
        Ok(())
    }
}

fn resolve_paths(repo: &Repository, token: &Cancel) -> Result<Vec<PathBuf>, String> {
    let default = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(|value| PathBuf::from(value).join("git/ignore"))
        .unwrap_or_else(|| "~/.config/git/ignore".into());
    // Git owns tilde expansion and config precedence, including an explicitly
    // empty value. NUL termination preserves whitespace and newlines in paths.
    let bytes = repo.read(
        &[
            OsStr::new("config"),
            OsStr::new("--null"),
            OsStr::new("--path"),
            OsStr::new("--default"),
            default.as_os_str(),
            OsStr::new("--get"),
            OsStr::new("core.excludesfile"),
        ],
        token,
    )?;
    let value = bytes
        .strip_suffix(&[0])
        .ok_or("Missing ignore path terminator")?;
    if value.contains(&0) {
        return Err("Unexpected multiple ignore paths".into());
    }
    let mut paths = vec![repo.operation_key(token)?.join("info/exclude")];
    if !value.is_empty() {
        paths.push(repo.path.join(path_from_bytes(value)?));
    }
    Ok(paths)
}

fn digest(paths: &[PathBuf], token: &Cancel) -> Result<[u8; 32], String> {
    let mut hash = Sha256::new();
    for path in paths {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let metadata = match fs::metadata(path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hash.update([0]);
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        const LIMIT: u64 = 1024 * 1024;
        if !metadata.is_file() || metadata.len() > LIMIT {
            return Err("Ignore rules cannot be sampled within the file budget".into());
        }
        let mut file = fs::File::open(path)
            .map_err(|error| error.to_string())?
            .take(LIMIT + 1);
        let mut content = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0; 8192];
        loop {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let n = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > LIMIT {
                return Err("Ignore rules exceed the file budget".into());
            }
            content.update(&buffer[..n]);
        }
        hash.update([1]);
        hash.update(content.finalize());
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worktree_events::configuration::tests::Fixture;

    #[test]
    fn external_rules_detect_same_size_edits_deletion_errors_and_retry_delivery() {
        let fixture = Fixture::new();
        fixture.git(&["config", "core.excludesFile", "rules with spaces"]);
        let repo = Repository::new(&fixture.0);
        let token = Cancel::default();
        let file = fixture.0.join("rules with spaces");
        fs::write(&file, "one\n").unwrap();
        let mut rules = IgnoreRules::default();
        rules.poll(&repo, &token, false).unwrap();
        assert!(rules.pending());
        assert_eq!(rules.paths.as_ref().unwrap()[1], file);
        rules.acknowledge();
        rules.poll(&repo, &token, false).unwrap();
        assert!(!rules.pending());
        fs::write(&file, "two\n").unwrap();
        rules.poll(&repo, &token, false).unwrap();
        assert!(rules.pending());
        rules.poll(&repo, &token, false).unwrap();
        assert!(rules.pending(), "reading is not successful delivery");
        rules.acknowledge();
        fs::remove_file(&file).unwrap();
        rules.poll(&repo, &token, false).unwrap();
        assert!(rules.pending());
        rules.acknowledge();
        rules.poll(&repo, &token, false).unwrap();
        assert!(!rules.pending(), "stable absence is valid");
        fs::create_dir(&file).unwrap();
        for _ in 0..2 {
            rules.poll(&repo, &token, false).unwrap();
            assert!(rules.pending(), "non-file remains unknown");
            rules.acknowledge();
        }
        fs::remove_dir(&file).unwrap();
        fs::write(&file, "x".repeat(1024 * 1024 + 1)).unwrap();
        rules.poll(&repo, &token, false).unwrap();
        assert!(rules.pending());
        rules.acknowledge();
        fs::write(&file, "ok\n").unwrap();
        rules.poll(&repo, &token, false).unwrap();
        assert!(rules.pending(), "recovery also invalidates");
        rules.acknowledge();
        let fingerprint = rules.fingerprint;
        token.cancel();
        assert!(rules.poll(&repo, &token, true).is_err());
        assert_eq!(rules.fingerprint, fingerprint);
        assert!(!rules.pending());
    }

    #[test]
    fn common_excludes_and_empty_or_git_expanded_paths_are_observed() {
        let fixture = Fixture::new();
        fixture.git(&["config", "core.excludesFile", ""]);
        let repo = Repository::new(&fixture.0);
        let token = Cancel::default();
        let mut rules = IgnoreRules::default();
        rules.poll(&repo, &token, true).unwrap();
        assert_eq!(rules.paths.as_ref().unwrap().len(), 1);
        rules.acknowledge();
        let common = rules.paths.as_ref().unwrap()[0].clone();
        fs::create_dir_all(common.parent().unwrap()).unwrap();
        fs::write(&common, "ignored\n").unwrap();
        rules.poll(&repo, &token, false).unwrap();
        assert!(rules.pending());
        fixture.git(&["config", "core.excludesFile", "~/plico-missing-ignore-test"]);
        rules.poll(&repo, &token, true).unwrap();
        let expanded = &rules.paths.as_ref().unwrap()[1];
        assert!(expanded.is_absolute());
        assert!(!expanded.components().any(|c| c.as_os_str() == "~"));
        // Only inspect the default path; never create files in the user's HOME.
        fixture.git(&["config", "--unset", "core.excludesFile"]);
        rules.poll(&repo, &token, true).unwrap();
        assert!(rules.paths.as_ref().unwrap()[1].ends_with("git/ignore"));
    }
}
