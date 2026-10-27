//! Ordered reword mappings, following GitRewordedCommitMessageProvider.
//! Stored inside rebase-merge so completion/abort removes the state with Git's.
use crate::{
    editor::Document,
    git::{HISTORY_MAX, OUTPUT_LIMIT, is_oid},
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
const NAME: &str = "plico-interactive-messages-v1.json";
const MESSAGE_LIMIT: usize = 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Mapping {
    pub original: String,
    pub replacement: String,
}

struct Record {
    head: String,
    onto: String,
    mappings: Vec<Mapping>,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn identity(directory: &Path, name: &str) -> io::Result<String> {
    let mut value = String::new();
    fs::File::open(directory.join(name))?
        .take(128)
        .read_to_string(&mut value)?;
    let value = value.trim_end_matches(['\r', '\n']);
    if !is_oid(value) {
        return Err(invalid("Invalid rebase identity"));
    }
    Ok(value.into())
}
fn current(directory: &Path, record: &Record) -> io::Result<bool> {
    Ok(identity(directory, "orig-head")? == record.head
        && identity(directory, "onto")? == record.onto)
}
fn validate(record: &Record) -> io::Result<()> {
    if !is_oid(&record.head) || !is_oid(&record.onto) || record.mappings.len() > HISTORY_MAX {
        return Err(invalid("Invalid rebase message mappings"));
    }
    let mut total = 0usize;
    for mapping in &record.mappings {
        for text in [&mapping.original, &mapping.replacement] {
            total = total.saturating_add(text.len());
            if text.len() > MESSAGE_LIMIT || text.contains('\0') || total > OUTPUT_LIMIT {
                return Err(invalid("Rebase message mappings exceed limits"));
            }
        }
    }
    Ok(())
}
fn create(path: &Path) -> io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
/// Install only while Git's sequence editor is waiting. Never create a rebase
/// directory or replace a mapping owned by an earlier attempt.
pub fn install(gitdir: &Path, head: &str, mappings: Vec<Mapping>) -> io::Result<()> {
    let directory = gitdir.join("rebase-merge");
    let record = Record {
        head: head.into(),
        onto: identity(&directory, "onto")?,
        mappings,
    };
    validate(&record)?;
    if !current(&directory, &record)? {
        return Err(invalid("Rebase changed before saving messages"));
    }
    let bytes = serde_json::to_vec(&serde_json::json!({
        "head": record.head, "onto": record.onto,
        "mappings": record.mappings.iter().map(|m| serde_json::json!({"original": m.original, "replacement": m.replacement})).collect::<Vec<_>>()
    }))?;
    if bytes.len() > OUTPUT_LIMIT {
        return Err(invalid("Serialized rebase messages exceed limit"));
    }
    let path = directory.join(NAME);
    let mut file = create(&path)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        if !current(&directory, &record)? {
            return Err(invalid("Rebase changed while saving messages"));
        }
        Ok(())
    })();
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}
fn read(directory: &Path) -> io::Result<Option<Record>> {
    let path = directory.join(NAME);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
        Ok(meta) if !meta.is_file() || meta.len() > OUTPUT_LIMIT as u64 => {
            return Err(invalid("Invalid rebase mapping file"));
        }
        _ => {}
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(OUTPUT_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > OUTPUT_LIMIT {
        return Err(invalid("Rebase mapping file exceeds limit"));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("Invalid message record"))?;
    if object.len() != 3 {
        return Err(invalid("Unknown message record fields"));
    }
    let string = |value: Option<&serde_json::Value>| -> io::Result<String> {
        value
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or_else(|| invalid("Missing message record field"))
    };
    let values = object
        .get("mappings")
        .and_then(|v| v.as_array())
        .ok_or_else(|| invalid("Missing message mappings"))?;
    if values.len() > HISTORY_MAX {
        return Err(invalid("Too many message mappings"));
    }
    let mut mappings = Vec::with_capacity(values.len());
    for value in values {
        let item = value
            .as_object()
            .ok_or_else(|| invalid("Invalid message mapping"))?;
        if item.len() != 2 {
            return Err(invalid("Unknown message mapping fields"));
        }
        mappings.push(Mapping {
            original: string(item.get("original"))?,
            replacement: string(item.get("replacement"))?,
        });
    }
    let record = Record {
        head: string(object.get("head"))?,
        onto: string(object.get("onto"))?,
        mappings,
    };
    validate(&record)?;
    Ok(Some(record))
}
struct Reservation {
    replacement: String,
    marker: PathBuf,
}
impl Reservation {
    fn finish(self) -> io::Result<()> {
        // A non-complete marker always forces manual editing after a crash.
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(self.marker)?;
        file.write_all(b"applied\n")?;
        file.sync_all()
    }
}
fn claim(gitdir: &Path, original: &str) -> io::Result<Option<Reservation>> {
    let directory = gitdir.join("rebase-merge");
    let Some(record) = read(&directory)? else {
        return Ok(None);
    };
    if !current(&directory, &record)? {
        return Ok(None);
    }
    for (index, mapping) in record.mappings.iter().enumerate() {
        let marker = directory.join(format!("plico-interactive-message-{index}.used"));
        match fs::symlink_metadata(&marker) {
            Ok(meta) => {
                if !meta.is_file() || meta.len() != 8 {
                    return Ok(None);
                }
                let mut bytes = Vec::new();
                fs::File::open(&marker)?.take(9).read_to_end(&mut bytes)?;
                if bytes == b"applied\n" {
                    continue;
                }
                return Ok(None);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        // Do not skip unmatched mappings to find a later matching message.
        if mapping.original.is_empty() || !original.starts_with(&mapping.original) {
            return Ok(None);
        }
        let mut file = match create(&marker) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(None),
            Err(e) => return Err(e),
        };
        file.write_all(b"reserved\n")?;
        file.sync_all()?;
        if !current(&directory, &record)? {
            return Err(invalid("Rebase changed before applying message"));
        }
        return Ok(Some(Reservation {
            replacement: mapping.replacement.clone(),
            marker,
        }));
    }
    Ok(None)
}
/// False leaves the request open for the ordinary message editor. Reserving
/// before answering Git prevents accidental reuse after an interrupted write.
pub fn apply(document: &mut Document) -> io::Result<bool> {
    if document.sequence {
        return Ok(false);
    }
    let Some(reservation) = claim(document.git_directory(), document.text())? else {
        return Ok(false);
    };
    document.save(&reservation.replacement)?;
    reservation.finish()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "plico-rebase-messages-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            fs::create_dir(p.join("rebase-merge")).unwrap();
            fs::write(p.join("rebase-merge/orig-head"), "a".repeat(40)).unwrap();
            fs::write(p.join("rebase-merge/onto"), "b".repeat(40)).unwrap();
            Self(p)
        }
        fn install(&self) {
            install(
                &self.0,
                &"a".repeat(40),
                vec![
                    Mapping {
                        original: "same".into(),
                        replacement: "first".into(),
                    },
                    Mapping {
                        original: "same".into(),
                        replacement: "second".into(),
                    },
                ],
            )
            .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn ordered_messages_survive_reload_and_match_prefix() {
        let f = Fixture::new();
        f.install();
        assert!(claim(&f.0, "unmatched").unwrap().is_none());
        let first = claim(&f.0, "same\n# git comment").unwrap().unwrap();
        assert_eq!(first.replacement, "first");
        first.finish().unwrap();
        let second = claim(&f.0, "same").unwrap().unwrap();
        assert_eq!(second.replacement, "second");
        second.finish().unwrap();
        assert!(claim(&f.0, "same").unwrap().is_none());
        assert!(install(&f.0, &"a".repeat(40), vec![]).is_err());
    }
    #[test]
    fn interrupted_reservation_never_guesses_next_mapping() {
        let f = Fixture::new();
        f.install();
        drop(claim(&f.0, "same").unwrap().unwrap());
        assert!(claim(&f.0, "same").unwrap().is_none());
    }
    #[test]
    fn changed_identity_and_invalid_records_are_not_applied() {
        let f = Fixture::new();
        f.install();
        fs::write(f.0.join("rebase-merge/onto"), "c".repeat(40)).unwrap();
        assert!(claim(&f.0, "same").unwrap().is_none());
        fs::write(f.0.join("rebase-merge").join(NAME), "truncated").unwrap();
        assert!(claim(&f.0, "same").is_err());
    }
}
