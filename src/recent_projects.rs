//! A bounded recent-repository list; no repository scanning at welcome startup.
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};
const LIMIT: usize = 20;
const MAX_BYTES: usize = 65536;
fn settings_file() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    base.map(|p| p.join("GitReview").join("recent-projects"))
}
/// UI/list identity only; never use this key to open a repository. Reading
/// recent entries must not canonicalize or probe disconnected network paths.
pub fn identity(path: &Path) -> String {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    {
        let plain = if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{}", unc)
        } else {
            text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
        };
        plain.replace('\\', "/").to_lowercase()
    }
    #[cfg(not(windows))]
    {
        text.into_owned()
    }
}
fn decode(bytes: &[u8]) -> Option<Vec<PathBuf>> {
    let mut bytes = bytes.strip_prefix(b"GitReview-recent-v1\0")?;
    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while !bytes.is_empty() {
        let size = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
        bytes = &bytes[4..];
        let text = std::str::from_utf8(bytes.get(..size)?).ok()?;
        let path = PathBuf::from(text);
        if !path.is_absolute() {
            return None;
        }
        if seen.insert(identity(&path)) {
            if paths.len() == LIMIT {
                return None;
            }
            paths.push(path);
        }
        bytes = &bytes[size..];
    }
    Some(paths)
}
fn read(file: &Path) -> Vec<PathBuf> {
    let mut bytes = Vec::new();
    if std::fs::File::open(file)
        .and_then(|f| f.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes))
        .is_err()
        || bytes.len() > MAX_BYTES
    {
        return vec![];
    }
    decode(&bytes).unwrap_or_default()
}
pub fn load() -> Vec<PathBuf> {
    settings_file().map(|p| read(&p)).unwrap_or_default()
}
fn save(file: &Path, paths: &[PathBuf]) -> io::Result<()> {
    let mut bytes = b"GitReview-recent-v1\0".to_vec();
    for path in paths.iter().take(LIMIT) {
        let Some(text) = path.to_str() else {
            continue;
        };
        if bytes.len() + 4 + text.len() > MAX_BYTES {
            break;
        }
        bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
        bytes.extend_from_slice(text.as_bytes());
    }
    std::fs::create_dir_all(file.parent().unwrap())?;
    std::fs::write(file, bytes)
}
pub fn opened(path: &Path) -> io::Result<Vec<PathBuf>> {
    let Some(file) = settings_file() else {
        return Ok(vec![]);
    };
    let path = path.canonicalize()?;
    let key = identity(&path);
    let mut paths = read(&file);
    paths.retain(|p| identity(p) != key);
    paths.insert(0, path);
    paths.truncate(LIMIT);
    save(&file, &paths)?;
    Ok(paths)
}
pub fn remove(path: &Path) -> io::Result<Vec<PathBuf>> {
    let Some(file) = settings_file() else {
        return Ok(vec![]);
    };
    let key = identity(path);
    let mut paths = read(&file);
    paths.retain(|p| identity(p) != key);
    save(&file, &paths)?;
    Ok(paths)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_recent_file_coalesces_normal_and_verbatim_paths() {
        let paths = [
            PathBuf::from(r"C:\Users\review\repo"),
            PathBuf::from(r"\\?\C:\Users\review\repo"),
            PathBuf::from(r"c:/users/review/REPO"),
            PathBuf::from(r"\\server\share\other"),
            PathBuf::from(r"\\?\UNC\SERVER\share\other"),
        ];
        let mut bytes = b"GitReview-recent-v1\0".to_vec();
        for path in &paths {
            let text = path.to_str().unwrap();
            bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
        assert_eq!(
            decode(&bytes).unwrap(),
            vec![paths[0].clone(), paths[3].clone()]
        );
    }
    #[test]
    fn roundtrip_unicode_and_linebreak_paths_and_reject_truncation() {
        let dir = std::env::temp_dir().join(format!("gitui-recent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recent");
        let paths = vec![dir.join("中文 folder"), dir.join("other")];
        save(&file, &paths).unwrap();
        assert_eq!(read(&file), paths);
        let bytes = std::fs::read(&file).unwrap();
        for end in 1..bytes.len() {
            if end != b"GitReview-recent-v1\0".len()
                && end != b"GitReview-recent-v1\0".len() + 4 + paths[0].to_str().unwrap().len()
            {
                assert!(decode(&bytes[..end]).is_none());
            }
        }
        std::fs::write(&file, vec![0; MAX_BYTES + 1]).unwrap();
        assert!(read(&file).is_empty());
        let many: Vec<_> = (0..30).map(|i| dir.join(i.to_string())).collect();
        save(&file, &many).unwrap();
        assert_eq!(read(&file), many[..20]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
