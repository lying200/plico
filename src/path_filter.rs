//! Rebased StructureFilterPopupComponent's newline-only path draft for one root.
pub mod project;
pub mod store;

use crate::git::{LogFilter, Result};
use std::{collections::HashSet, path::Path};

pub fn parse(draft: &str, root: &Path) -> Result<Vec<String>> {
    if draft.len() > 64 * 1024 {
        return Err("Paths draft exceeds 64 KiB".into());
    }
    let root = root
        .to_str()
        .ok_or("Repository path is not UTF-8")?
        .replace('\\', "/");
    let root = root.trim_end_matches('/');
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for line in draft.split('\n') {
        let line = line.trim_matches(|c: char| c <= '\u{20}');
        if line.is_empty() {
            continue;
        }
        let normalized = line.replace('\\', "/");
        let mut path = normalized.trim_end_matches('/');
        let absolute = normalized.starts_with('/') || normalized.as_bytes().get(1) == Some(&b':');
        if absolute {
            let prefix = path.get(..root.len());
            let same_root = prefix.is_some_and(|prefix| {
                if cfg!(windows) {
                    prefix.eq_ignore_ascii_case(root)
                } else {
                    prefix == root
                }
            });
            if !same_root || !matches!(path.as_bytes().get(root.len()), None | Some(b'/')) {
                return Err("Select paths inside the current repository".into());
            }
            path = path.get(root.len()..).unwrap_or("").trim_start_matches('/');
            if path.is_empty() {
                path = ".";
            }
        }
        while let Some(relative) = path.strip_prefix("./") {
            path = relative;
        }
        if path.is_empty() {
            path = ".";
        }
        if seen.insert(path.to_owned()) {
            paths.push(path.to_owned());
        }
    }
    LogFilter {
        paths: paths.clone(),
        ..Default::default()
    }
    .validate_paths()?;
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newline_only_paths_normalize_windows_separators_and_keep_literal_names() {
        let root = Path::new("C:/work/repo");
        assert_eq!(
            parse(
                "src\\cache.rs\r\nsrc/\n./src/\nname|part\nC:/work/repo/old file",
                root
            )
            .unwrap(),
            vec!["src/cache.rs", "src", "name|part", "old file"]
        );
        assert_eq!(parse("C:/work/repo/", root).unwrap(), vec!["."]);
        assert!(parse(" \n\r\n", root).unwrap().is_empty());
        for text in [
            "C:/work/repo-other/a",
            "../outside",
            "C:/outside",
            "src/../outside",
        ] {
            assert!(parse(text, root).is_err(), "{text}");
        }
    }
}
