//! Rebased ClonePathProvider/FilePathDocumentChildPathHandle, 4aec352.
//! Suggestions stop permanently after a user edit; no repository probing.
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};
const LIMIT: usize = 32768;
fn file() -> Option<PathBuf> {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    root.map(|r| r.join("GitReview/clone-parent"))
}
fn read(file: &Path) -> Option<PathBuf> {
    let mut text = String::new();
    fs_read(file, &mut text).ok()?;
    let path = PathBuf::from(text);
    path.is_absolute().then_some(path)
}
fn fs_read(file: &Path, text: &mut String) -> io::Result<()> {
    std::fs::File::open(file)?
        .take(LIMIT as u64 + 1)
        .read_to_string(text)?;
    if text.len() > LIMIT || text.contains('\0') {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(())
}
pub fn parent() -> PathBuf {
    file()
        .and_then(|p| read(&p))
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
        })
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}
pub fn remember(destination: &Path) -> io::Result<()> {
    let Some(parent) = destination.parent().filter(|p| p.is_absolute()) else {
        return Ok(());
    };
    let Some(file) = file() else {
        return Ok(());
    };
    let text = parent.to_str().ok_or(io::ErrorKind::InvalidData)?;
    if text.len() > LIMIT {
        return Err(io::ErrorKind::InvalidData.into());
    }
    std::fs::create_dir_all(file.parent().unwrap())?;
    std::fs::write(file, text)
}
pub fn child(source: &str) -> Option<String> {
    let source = source.trim();
    let source = source.strip_prefix("git clone ").unwrap_or(source).trim();
    let parsed = url::Url::parse(source)
        .ok()
        .filter(|u| source.contains("://") && !u.path().is_empty());
    let path = parsed
        .as_ref()
        .map(|u| u.path())
        .unwrap_or(source)
        .trim_end_matches(['/', '\\']);
    let last = path.rsplit(['/', '\\', ':']).next()?;
    let mut decoded = Vec::new();
    let mut bytes = last.as_bytes().iter().copied();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let a = bytes.next()?;
            let b = bytes.next()?;
            let a = (a as char).to_digit(16)?;
            let b = (b as char).to_digit(16)?;
            decoded.push((a * 16 + b) as u8);
        } else {
            decoded.push(b);
        }
    }
    let decoded = String::from_utf8(decoded).ok()?;
    let name = decoded.strip_suffix(".git").unwrap_or(&decoded);
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
    {
        return None;
    }
    Some(name.into())
}
pub struct Suggestion {
    parent: PathBuf,
    last: PathBuf,
    manual: bool,
}
impl Suggestion {
    pub fn new(parent: PathBuf) -> Self {
        Self {
            last: parent.clone(),
            parent,
            manual: false,
        }
    }
    pub fn current(&self) -> &Path {
        &self.last
    }
    pub fn edited(&mut self, value: &Path) {
        if value != self.last {
            self.manual = true;
        }
    }
    pub fn select_directory(&mut self) {
        self.manual = true;
    }
    pub fn from_url(&mut self, source: &str) -> Option<PathBuf> {
        if self.manual {
            return None;
        }
        let path = self.parent.join(child(source).unwrap_or_default());
        self.last = path.clone();
        Some(path)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suggestions_understand_sources_and_never_escape_parent() {
        for (url, want) in [
            ("git clone https://example.invalid/team/repo.git", "repo"),
            ("git@host:team/repo.git", "repo"),
            ("https://host/hello%20world.git?token=x", "hello world"),
            (r"C:\code\repo.git", "repo"),
        ] {
            assert_eq!(child(url).as_deref(), Some(want));
        }
        for url in [
            "https://host/%2e%2e.git",
            "https://host/a%2fb.git",
            "https://host/a%5cb.git",
            "https://host/a%00b.git",
        ] {
            assert!(child(url).is_none());
        }
    }
    #[test]
    fn manual_destination_survives_future_url_changes() {
        let parent = std::env::temp_dir();
        let mut s = Suggestion::new(parent.clone());
        let auto = s.from_url("https://host/first.git").unwrap();
        s.edited(&auto);
        assert_eq!(
            s.from_url("https://host/second.git").unwrap(),
            parent.join("second")
        );
        s.edited(&parent.join("mine"));
        assert!(s.from_url("https://host/third.git").is_none());
    }
    #[test]
    fn chooser_marks_even_identical_path_as_explicit() {
        let mut s = Suggestion::new(std::env::temp_dir());
        s.select_directory();
        assert!(s.from_url("https://host/repo.git").is_none());
    }
}
