//! UpdateOptionsDialog's confirmation preference; update method is shared with Push.
//! Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd, Apache-2.0:
//! UpdateOptionsDialog.setToBeShown, GitUpdateOptionsPanel, GitPushOperation.
use crate::{git::{Cancel, Repository}, update_method::Method, update_preferences::{self, Strategy}};
use std::{io::Read, path::{Path, PathBuf}};

/// The dialog owns a draft. Loading never writes; only its OK handler may accept it.
pub struct Preferences {
    pub method: Method,
    pub show_options: bool,
    method_path: PathBuf,
    options_path: PathBuf,
}
impl Preferences {
    pub fn for_repository(repo: &Repository, token: &Cancel) -> Result<Self, String> {
        let (method_path, shared) = update_preferences::for_repository(repo, token)?;
        let method = match shared.strategy {
            Strategy::Merge => Method::Merge,
            Strategy::Rebase => Method::Rebase,
            Strategy::BranchDefault => repo.default_update_method(token)?,
        };
        let options_path = method_path.with_file_name("project-update-options");
        Ok(Self { method, show_options: load_show_options(&options_path), method_path, options_path })
    }

    /// Save the common method without changing Push's automatic-update consent.
    /// Hide the dialog only after the method was successfully saved.
    pub fn accept(&self, method: Method, show_options: bool) -> std::io::Result<()> {
        let mut shared = update_preferences::load(&self.method_path);
        shared.strategy = match method { Method::Merge => Strategy::Merge, Method::Rebase => Strategy::Rebase };
        update_preferences::save(&self.method_path, shared)?;
        save_show_options(&self.options_path, show_options)
    }
}

pub fn load_show_options(path: &Path) -> bool {
    let mut text = String::new();
    if std::fs::File::open(path).and_then(|f| f.take(65).read_to_string(&mut text)).is_err() { return true; }
    // Missing, corrupt, newer-version and truncated records always restore confirmation.
    text != "version=1\nshow-options=false\n"
}
pub fn save_show_options(path: &Path, show: bool) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) { std::fs::create_dir_all(parent)?; }
    std::fs::write(path, format!("version=1\nshow-options={show}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn folder(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("gitui-project-update-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap(); path
    }
    #[test]
    fn project_update_options_only_complete_record_can_skip_confirmation() {
        let directory = folder("invalid"); let path = directory.join("options");
        assert!(load_show_options(&path));
        for bytes in [b"".as_slice(), b"version=1\nshow-options=false", b"version=2\nshow-options=false\n",
            b"version=1\nshow-options=false\nextra", b"\xff\xfe"] {
            std::fs::write(&path, bytes).unwrap(); assert!(load_show_options(&path));
        }
        std::fs::write(&path, "x".repeat(1000)).unwrap(); assert!(load_show_options(&path));
        for show in [false, true] {
            save_show_options(&path, show).unwrap(); assert_eq!(load_show_options(&path), show);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn project_update_method_is_shared_and_push_consent_is_preserved() {
        let directory = folder("shared");
        let method_path = directory.join("push-update-preferences"); let options_path = directory.join("options");
        for auto_update in [false, true] {
            update_preferences::save(&method_path, update_preferences::Preferences { strategy: Strategy::BranchDefault, auto_update }).unwrap();
            let draft = Preferences { method: Method::Merge, show_options: true, method_path: method_path.clone(), options_path: options_path.clone() };
            draft.accept(Method::Rebase, false).unwrap();
            let shared = update_preferences::load(&method_path);
            assert_eq!(shared.strategy, Strategy::Rebase); assert_eq!(shared.auto_update, auto_update);
            assert!(!load_show_options(&options_path));
            // Restoring confirmation must not alter the common method or Push consent.
            save_show_options(&options_path, true).unwrap();
            assert_eq!(update_preferences::load(&method_path), shared);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn project_update_write_failure_does_not_hide_confirmation() {
        let directory = folder("failure"); let blocker = directory.join("file");
        std::fs::write(&blocker, "not a directory").unwrap();
        let options_path = directory.join("options");
        let draft = Preferences { method: Method::Merge, show_options: true, method_path: blocker.join("method"), options_path: options_path.clone() };
        assert!(draft.accept(Method::Rebase, false).is_err());
        assert!(load_show_options(&options_path)); assert!(!options_path.exists());
        assert!(save_show_options(&blocker.join("options"), false).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
