//! Minimal persisted origin of an app-started Squash, scoped to rebase-merge.
//! Rebased GitRebaseSpec keeps initial HEAD/branch and parameters in its session.
use crate::git::is_oid;
use std::{fs::{self, OpenOptions}, io::{Read, Write}, path::Path};
const NAME: &str = "gitui-squash-origin";
const MAGIC: &str = "GitReview squash origin v1";
pub(crate) struct Origin { pub head: String, pub base: String, pub branch: Option<String> }
fn text(path: &Path) -> Result<String, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() { return Err("Squash origin is not a regular file".into()); }
    let mut value = String::new();
    fs::File::open(path).map_err(|e| e.to_string())?.take(8193).read_to_string(&mut value).map_err(|e| e.to_string())?;
    if value.len() > 8192 { return Err("Squash origin exceeds limit".into()); }
    Ok(value)
}
fn matches(directory: &Path, origin: &Origin) -> Result<(), String> {
    for (file, expected) in [("orig-head", origin.head.as_str()), ("onto", origin.base.as_str()),
        ("head-name", origin.branch.as_deref().unwrap_or("detached HEAD"))] {
        if text(&directory.join(file))?.trim_end_matches(['\r', '\n']) != expected {
            return Err("Squash origin does not match the current rebase".into());
        }
    }
    Ok(())
}
pub(crate) fn read(gitdir: &Path) -> Result<Option<Origin>, String> {
    let directory = gitdir.join("rebase-merge"); let path = directory.join(NAME);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()), Ok(_) => {},
    }
    let data = text(&path)?; let fields = data.split('\n').collect::<Vec<_>>();
    if fields.len() != 5 || fields[0] != MAGIC || !fields[4].is_empty()
        || !is_oid(fields[1]) || !is_oid(fields[2])
        || (!fields[3].is_empty() && (!fields[3].starts_with("refs/heads/") || fields[3].contains(['\r','\0']))) {
        return Err("Invalid Squash origin record".into());
    }
    let origin = Origin { head: fields[1].into(), base: fields[2].into(), branch: (!fields[3].is_empty()).then(|| fields[3].into()) };
    matches(&directory, &origin)?;
    Ok(Some(origin))
}
pub(crate) fn install(gitdir: &Path, head: &str, base: &str, branch: Option<&str>) -> Result<(), String> {
    if !is_oid(head) || !is_oid(base) || branch.is_some_and(|b| !b.starts_with("refs/heads/") || b.len() > 4096 || b.contains(['\n','\r','\0'])) {
        return Err("Invalid Squash origin".into());
    }
    let directory = gitdir.join("rebase-merge");
    let origin = Origin { head: head.into(), base: base.into(), branch: branch.map(str::to_owned) };
    matches(&directory, &origin)?;
    let path = directory.join(NAME); let mut options = OpenOptions::new(); options.create_new(true).write(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(&path).map_err(|e| e.to_string())?;
    let content = format!("{MAGIC}\n{head}\n{base}\n{}\n", branch.unwrap_or(""));
    let result = file.write_all(content.as_bytes()).and_then(|_| file.sync_all()).map_err(|e| e.to_string())
        .and_then(|_| matches(&directory, &origin));
    drop(file);
    if result.is_err() { let _ = fs::remove_file(path); }
    result
}
