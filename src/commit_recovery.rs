//! Durable metadata for Rebased-style excluded-index restoration.
//! Local addition: upstream's saver retains this information only in memory.
//! Reading a record does NOT authorize replay against a possibly changed index.
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const NAME: &str = "gitui-commit-index-recovery";
const MAGIC: &[u8] = b"GitReview commit index recovery v1\n";
const LIMIT: usize = 16 * 1024 * 1024;

#[derive(PartialEq, Eq)]
pub struct Record {
    pub before_head: Option<String>,
    pub prepared: Vec<u8>,
    pub restore: Vec<u8>,
}

/// Explicit caller action only. Never called automatically on startup. The app
/// queue does not exclude other Git clients; avoid concurrent external writers.
pub fn restore(
    repo: &crate::git::Repository,
    token: &crate::git::Cancel,
    progress: &crate::operation_process::Progress,
) -> Result<(), String> {
    use crate::{git::Cancel, operation_process};
    let _guard = repo.operation_guard(token)?;
    let directory = repo.editor_directory(token)?;
    let record = read(&directory)?;
    let prepared = index_info(&record.prepared)?;
    let original = index_info(&record.restore)?;
    if original.is_empty() || !original.keys().eq(prepared.keys()) {
        return Err("Recovery path sets do not match".into());
    }
    if repo.operation_state(token)?.in_progress() {
        return Err("Finish the current Git operation before index recovery".into());
    }
    let current = live_index(repo, token)?;
    for (path, expected) in &original {
        let live = current.get(path).cloned();
        if &live != expected && &live != prepared.get(path).unwrap() {
            return Err(
                "Affected index content changed after the interrupted commit; recovery stopped"
                    .into(),
            );
        }
    }
    // update-index can replace file/directory prefixes. Never let a replay
    // remove newly staged descendants or ancestors outside the recorded set.
    for path in current.keys().filter(|p| !original.contains_key(*p)) {
        let ancestor = path
            .iter()
            .enumerate()
            .filter(|(_, b)| **b == b'/')
            .any(|(i, _)| original.contains_key(&path[..i]));
        let descendant = original
            .range(path.clone()..)
            .next()
            .is_some_and(|(affected, _)| nested(affected, path));
        if ancestor || descendant {
            return Err("A new staged path overlaps commit recovery paths".into());
        }
    }
    if read(&directory)? != record {
        return Err("Commit recovery record changed".into());
    }
    let outcome = operation_process::run_with_input(
        repo.operation_command()
            .args(["update-index", "-z", "--index-info"]),
        token,
        progress,
        Some(std::time::Duration::from_secs(30)),
        &record.restore,
    )
    .map_err(|e| e.to_string())?;
    if !outcome.status.success() || outcome.stopped.is_some() || outcome.input_error.is_some() {
        return Err(format!(
            "Index recovery did not complete: {:?} {}{}",
            outcome.stopped,
            String::from_utf8_lossy(&outcome.stderr.bytes()),
            outcome
                .input_error
                .map(|e| e.to_string())
                .unwrap_or_default()
        ));
    }
    // Do not discard the record if post-write verification fails or is cancelled.
    let current = live_index(repo, &Cancel::default())?;
    if original
        .iter()
        .any(|(path, expected)| current.get(path).cloned() != *expected)
    {
        return Err("Index recovery verification failed; record retained".into());
    }
    if read(&directory)? != record {
        return Err("Commit recovery record changed; record retained".into());
    }
    fs::remove_file(path(&directory))
        .map_err(|e| format!("Index recovered but record could not be removed: {e}"))
}
fn nested(child: &[u8], parent: &[u8]) -> bool {
    child.starts_with(parent) && child.get(parent.len()) == Some(&b'/')
}
type Identity = (Vec<u8>, Vec<u8>);
type Entries = std::collections::BTreeMap<Vec<u8>, Option<Identity>>;
fn valid_path(path: &[u8]) -> bool {
    #[cfg(windows)]
    if path.contains(&b'\\') || path.contains(&b':') || std::str::from_utf8(path).is_err() {
        return false;
    }
    !path.is_empty()
        && !path
            .split(|b| *b == b'/')
            .any(|p| p.is_empty() || p == b"." || p == b".." || p.eq_ignore_ascii_case(b".git"))
        && !path.contains(&0)
}
fn identity(mode: &[u8], oid: &[u8]) -> Result<Identity, String> {
    if !matches!(mode, b"100644" | b"100755" | b"120000" | b"160000")
        || !matches!(oid.len(), 40 | 64)
        || !oid.iter().all(u8::is_ascii_hexdigit)
        || oid.iter().all(|b| *b == b'0')
    {
        return Err("Invalid recovery index identity".into());
    }
    Ok((mode.to_vec(), oid.to_vec()))
}
fn records(bytes: &[u8]) -> Result<impl Iterator<Item = &[u8]>, String> {
    if bytes.len() > LIMIT || bytes.last() != Some(&0) {
        return Err("Invalid recovery index records".into());
    }
    Ok(bytes[..bytes.len() - 1].split(|b| *b == 0))
}
fn index_info(bytes: &[u8]) -> Result<Entries, String> {
    let mut result = Entries::new();
    let mut adding = false;
    for record in records(bytes)? {
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or("Invalid recovery record separator")?;
        let path = &record[tab + 1..];
        if !valid_path(path) {
            return Err("Invalid recovery path".into());
        }
        let fields: Vec<_> = record[..tab].split(|b| *b == b' ').collect();
        if fields.len() == 2 && fields[0] == b"0" {
            if adding
                || !matches!(fields[1].len(), 40 | 64)
                || !fields[1].iter().all(|b| *b == b'0')
                || result.insert(path.to_vec(), None).is_some()
            {
                return Err("Invalid recovery removal".into());
            }
        } else if fields.len() == 3 && fields[2] == b"0" {
            adding = true;
            let entry = result
                .get_mut(path)
                .ok_or("Recovery addition lacks removal record")?;
            if entry.is_some() {
                return Err("Duplicate recovery addition".into());
            }
            *entry = Some(identity(fields[0], fields[1])?);
        } else {
            return Err("Invalid recovery index record".into());
        }
    }
    Ok(result)
}
fn live_index(
    repo: &crate::git::Repository,
    token: &crate::git::Cancel,
) -> Result<std::collections::BTreeMap<Vec<u8>, Identity>, String> {
    use std::ffi::OsStr;
    let bytes = repo.read(&["ls-files", "--stage", "-z"].map(OsStr::new), token)?;
    let mut result = std::collections::BTreeMap::new();
    if bytes.is_empty() {
        return Ok(result);
    }
    for record in records(&bytes)? {
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or("Invalid index record")?;
        let fields: Vec<_> = record[..tab].split(|b| *b == b' ').collect();
        if fields.len() != 3 || fields[2] != b"0" {
            return Err("Resolve index conflicts before recovery".into());
        }
        let path = &record[tab + 1..];
        if !valid_path(path)
            || result
                .insert(path.to_vec(), identity(fields[0], fields[1])?)
                .is_some()
        {
            return Err("Invalid or duplicate live index path".into());
        }
    }
    Ok(result)
}
pub fn path(git_directory: &Path) -> PathBuf {
    git_directory.join(NAME)
}
pub fn pending(git_directory: &Path) -> Result<Option<PathBuf>, String> {
    let path = path(git_directory);
    match fs::symlink_metadata(&path) {
        Ok(_) => Ok(Some(path)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub(crate) fn create(
    git_directory: &Path,
    before_head: Option<&str>,
    prepared: &[u8],
    restore: &[u8],
) -> Result<PathBuf, String> {
    let head = before_head.unwrap_or("").as_bytes();
    if head.len() > 64 || prepared.len() > LIMIT || restore.len() > LIMIT {
        return Err("Recovery record exceeds limit".into());
    }
    let path = path(git_directory);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| format!("Cannot create commit recovery record: {e}"))?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(MAGIC)?;
        for bytes in [head, prepared, restore] {
            file.write_all(&(bytes.len() as u64).to_le_bytes())?;
        }
        for bytes in [head, prepared, restore] {
            file.write_all(bytes)?;
        }
        file.sync_all()?;
        #[cfg(unix)]
        fs::File::open(git_directory)?.sync_all()?;
        Ok(())
    })();
    drop(file);
    if let Err(error) = result {
        // No Git mutation has started. Failure to remove still leaves a marker
        // that blocks subsequent commits instead of overwriting partial data.
        let _ = fs::remove_file(&path);
        return Err(format!("Cannot persist commit recovery record: {error}"));
    }
    Ok(path)
}
/// Bounded inspection. Use `restore` for replay with current-index checks;
/// never blindly feed data loaded here to Git.
pub fn read(git_directory: &Path) -> Result<Record, String> {
    let path = path(git_directory);
    if !fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("Recovery record is not a regular file".into());
    }
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut magic = vec![0; MAGIC.len()];
    file.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if magic != MAGIC {
        return Err("Unknown commit recovery format".into());
    }
    let mut lengths = [0usize; 3];
    for (i, len) in lengths.iter_mut().enumerate() {
        let mut raw = [0u8; 8];
        file.read_exact(&mut raw).map_err(|e| e.to_string())?;
        *len = usize::try_from(u64::from_le_bytes(raw)).map_err(|_| "Invalid recovery length")?;
        if *len > if i == 0 { 64 } else { LIMIT } {
            return Err("Recovery record exceeds limit".into());
        }
    }
    let mut fields = Vec::new();
    for len in lengths {
        let mut bytes = vec![0; len];
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        fields.push(bytes);
    }
    if file.read(&mut [0u8; 1]).map_err(|e| e.to_string())? != 0 {
        return Err("Trailing commit recovery data".into());
    }
    let restore = fields.pop().unwrap();
    let prepared = fields.pop().unwrap();
    let head = String::from_utf8(fields.pop().unwrap()).map_err(|e| e.to_string())?;
    if !head.is_empty() && !crate::git::is_oid(&head) {
        return Err("Invalid recovery HEAD identity".into());
    }
    Ok(Record {
        before_head: (!head.is_empty()).then_some(head),
        prepared,
        restore,
    })
}
