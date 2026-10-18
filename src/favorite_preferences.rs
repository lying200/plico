//! Repository-scoped favorite overrides, based on Rebased 4aec352
//! DvcsBranchManager / BranchStorage (JetBrains, Apache-2.0).
use crate::{branch_popup::Favorites, git::{Cancel, Repository}};
use std::{io::{self, Read, Write}, path::{Path, PathBuf}};
const MAX_BYTES: usize = 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidData, "Invalid favorite preferences") }

/// Like existing update preferences, use private worktree administration data.
/// Loading does not create directories or change Git config/index/worktree.
pub fn for_repository(repo: &Repository, token: &Cancel) -> Result<(PathBuf, Favorites), String> {
    let path = repo.editor_directory(token)?.join("git-review").join("branch-favorites");
    let favorites = load(&path).map_err(|e| e.to_string())?;
    Ok((path, favorites))
}
fn valid_ref(name: &str) -> bool {
    !name.chars().any(|c| c.is_control() || c.is_whitespace())
        && ["refs/heads/", "refs/remotes/", "refs/tags/"].iter()
            .any(|prefix| name.strip_prefix(prefix).is_some_and(|s| !s.is_empty()))
}
fn encode(favorites: &Favorites) -> io::Result<String> {
    let mut entries: Vec<_> = favorites.overrides().collect();
    if entries.len() > MAX_ENTRIES { return Err(invalid()); }
    entries.sort_unstable();
    let mut text = String::from("git-review-favorites-v1\n");
    for (name, value) in entries {
        if !valid_ref(name) { return Err(invalid()); }
        text.push(if value { '+' } else { '-' }); text.push_str(name); text.push('\n');
        if text.len() > MAX_BYTES - 4 { return Err(invalid()); }
    }
    text.push_str("end\n"); Ok(text)
}
fn parse(text: &str) -> io::Result<Favorites> {
    if !text.ends_with("\n") { return Err(invalid()); }
    let mut lines = text.lines();
    if lines.next() != Some("git-review-favorites-v1") { return Err(invalid()); }
    let mut prefs = Favorites::default(); let mut seen = std::collections::HashSet::new();
    loop {
        let line = lines.next().ok_or_else(invalid)?;
        if line == "end" { if lines.next().is_some() { return Err(invalid()); } return Ok(prefs); }
        let (value,name) = match line.as_bytes().first() {
            Some(b'+') => (true, &line[1..]), Some(b'-') => (false, &line[1..]), _ => return Err(invalid()),
        };
        if !valid_ref(name) || !seen.insert(name) || seen.len() > MAX_ENTRIES { return Err(invalid()); }
        prefs.set(name,value);
    }
}
pub fn load(path: &Path) -> io::Result<Favorites> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file, Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Favorites::default()),
        Err(e) => return Err(e),
    };
    let mut text = String::new(); file.take(MAX_BYTES as u64 + 1).read_to_string(&mut text)?;
    if text.len() > MAX_BYTES { return Err(invalid()); }
    parse(&text)
}
/// Write a complete bounded record beside the old file, then replace it. A
/// failure before rename leaves the previous preferences intact. Concurrent
/// independent application instances are last-writer-wins, not merged.
pub fn save(path: &Path, favorites: &Favorites) -> io::Result<()> {
    let text = encode(favorites)?;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut random = [0u8; 16]; getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let temporary = parent.join(format!(".favorites-{:032x}.tmp", u128::from_le_bytes(random)));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    let result = (|| {
        file.write_all(text.as_bytes())?; file.sync_all()?; drop(file);
        std::fs::rename(&temporary,path)
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temporary); }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_exclusions_unicode_and_replacement_roundtrip() {
        let root = std::env::temp_dir().join(format!("gitui-favorites-{}",std::process::id()));
        let path = root.join("one"); let other = root.join("two");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(load(&path).unwrap(),Favorites::default()); assert!(!root.exists());
        let mut prefs = Favorites::default(); prefs.set("refs/heads/main",false); prefs.set("refs/tags/发布",true);
        save(&path,&prefs).unwrap(); assert_eq!(load(&path).unwrap(),prefs);
        assert!(load(&other).unwrap().contains("refs/heads/main"));
        prefs.set("refs/heads/main",true); save(&path,&prefs).unwrap(); assert_eq!(load(&path).unwrap(),prefs);
        let previous = std::fs::read(&path).unwrap(); prefs.set("refs/heads/bad\nname",true);
        assert!(save(&path,&prefs).is_err()); assert_eq!(std::fs::read(&path).unwrap(),previous);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(),1);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_partial_duplicate_and_oversized_records_are_errors() {
        for text in ["", "git-review-favorites-v2\nend\n", "git-review-favorites-v1\n", "git-review-favorites-v1\n+HEAD\nend\n",
            "git-review-favorites-v1\n-refs/heads/main\n+refs/heads/main\nend\n", "git-review-favorites-v1\nend\nextra\n"] {
            assert!(parse(text).is_err(), "{text:?}");
        }
        let text = format!("git-review-favorites-v1\n{}end\n", (0..=MAX_ENTRIES).map(|i| format!("+refs/heads/b{i}\n")).collect::<String>());
        assert!(parse(&text).is_err());
    }
}
