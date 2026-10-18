//! Rebased GitRewordedCommitMessageProvider (4aec352, Apache-2.0):
//! match rebase onto and original message before consuming a reword mapping.
//! Local storage follows the owning rebase-merge directory's lifecycle.
use std::{fs::{self, OpenOptions}, io::{self, Read, Write}, path::Path};
use crate::{editor::Document, git::is_oid};
const NAME: &str = "gitui-squash-message";
const USED: &str = "gitui-squash-message-used";
const MAGIC: &[u8] = b"GitReview squash message v1\n";
const LIMIT: usize = 1024 * 1024;
struct Record { head: String, onto: String, original: String, replacement: String }
fn invalid(message: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message) }
fn identity(directory: &Path, name: &str) -> io::Result<String> {
    let mut bytes = Vec::new(); fs::File::open(directory.join(name))?.take(128).read_to_end(&mut bytes)?;
    let text = String::from_utf8(bytes).map_err(|_| invalid("Invalid rebase identity"))?;
    let text = text.trim_end_matches(['\r', '\n']);
    if !is_oid(text) { return Err(invalid("Invalid rebase identity")); }
    Ok(text.into())
}
fn matches(directory: &Path, record: &Record) -> io::Result<bool> {
    Ok(identity(directory, "onto")? == record.onto && identity(directory, "orig-head")? == record.head)
}
fn create_file(path: &Path) -> io::Result<fs::File> {
    let mut options = OpenOptions::new(); options.create_new(true).write(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    options.open(path)
}
/// Called during the sequence-editor request, before releasing Git to replay.
/// Never replaces an existing mapping, and never creates a rebase directory.
/// Git removes the directory (and mapping) on successful completion or abort.
pub fn install(git_dir: &Path, head: &str, onto: &str, original: &str, replacement: &str) -> io::Result<()> {
    if !is_oid(head) || !is_oid(onto) || original.len() > LIMIT || replacement.len() > LIMIT
        || original.contains('\0') || replacement.contains('\0') || replacement.trim().is_empty() {
        return Err(invalid("Invalid Squash message mapping"));
    }
    let directory = git_dir.join("rebase-merge");
    let record = Record { head: head.into(), onto: onto.into(), original: original.into(), replacement: replacement.into() };
    if !matches(&directory, &record)? { return Err(invalid("Rebase changed before saving Squash message")); }
    let path = directory.join(NAME);
    let mut file = create_file(&path)?;
    let result = (|| {
        file.write_all(MAGIC)?;
        for value in [head, onto, original, replacement] { file.write_all(&(value.len() as u64).to_le_bytes())?; }
        for value in [head, onto, original, replacement] { file.write_all(value.as_bytes())?; }
        file.sync_all()?;
        if !matches(&directory, &record)? { return Err(invalid("Rebase changed while saving Squash message")); }
        Ok(())
    })();
    drop(file);
    if result.is_err() { let _ = fs::remove_file(path); }
    result
}
fn read(directory: &Path) -> io::Result<Option<Record>> {
    let path = directory.join(NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(value) => value, Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None), Err(e) => return Err(e),
    };
    if !metadata.is_file() { return Err(invalid("Squash mapping is not a regular file")); }
    let mut file = fs::File::open(path)?;
    let mut magic = vec![0; MAGIC.len()]; file.read_exact(&mut magic)?;
    if magic != MAGIC { return Err(invalid("Unknown Squash mapping format")); }
    let mut sizes = [0;4];
    for (index, size) in sizes.iter_mut().enumerate() {
        let mut bytes = [0;8]; file.read_exact(&mut bytes)?;
        *size = usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| invalid("Invalid mapping length"))?;
        if *size > if index < 2 { 64 } else { LIMIT } { return Err(invalid("Squash mapping exceeds limit")); }
    }
    let mut fields = Vec::new();
    for size in sizes {
        let mut bytes = vec![0;size]; file.read_exact(&mut bytes)?;
        fields.push(String::from_utf8(bytes).map_err(|_| invalid("Non-UTF-8 Squash mapping"))?);
    }
    if file.read(&mut [0;1])? != 0 { return Err(invalid("Trailing Squash mapping data")); }
    let record = Record { replacement: fields.pop().unwrap(), original: fields.pop().unwrap(), onto: fields.pop().unwrap(), head: fields.pop().unwrap() };
    if !is_oid(&record.head) || !is_oid(&record.onto) || record.original.contains('\0')
        || record.replacement.contains('\0') || record.replacement.trim().is_empty() { return Err(invalid("Invalid Squash mapping")); }
    Ok(Some(record))
}
/// True means the matching editor request was saved and answered. False leaves
/// the document untouched for the ordinary editor. Corrupt records are errors.
/// A persistent marker prevents reuse after restart. A crash after reserving it
/// falls back to manual editing, never guesses whether an editor already ran.
pub fn apply(document: &mut Document) -> io::Result<bool> {
    if document.sequence { return Ok(false); }
    let directory = document.git_directory().join("rebase-merge");
    let Some(record) = read(&directory)? else { return Ok(false); };
    if record.original.is_empty() || !document.text().starts_with(&record.original) || !matches(&directory, &record)? {
        return Ok(false);
    }
    let path = directory.join(USED);
    let mut marker = match create_file(&path) {
        Ok(file) => file, Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(false), Err(e) => return Err(e),
    };
    let result = (|| {
        marker.write_all(b"reserved\n")?; marker.sync_all()?;
        if !matches(&directory, &record)? { return Err(invalid("Rebase changed before applying Squash message")); }
        document.save(&record.replacement)
    })();
    drop(marker);
    if result.is_err() { let _ = fs::remove_file(path); }
    result.map(|_| true)
}
