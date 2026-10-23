//! Read-only recovery candidates. Rebased has no dedicated reflog browser in
//! reference 4aec352; branch recovery delegates to the existing branch workflow.
use crate::git::{Cancel, Repository, is_oid};
use std::ffi::OsStr;

pub const PAGE_SIZE: usize = 200;
pub const MAX_ENTRIES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Stable identity for a subsequent branch operation. Never use a reflog
    /// ordinal (which changes whenever another operation runs) for writes.
    pub commit: String,
    pub reference: String,
    pub timestamp: i64,
    pub action: String,
    pub author: String,
    pub email: String,
}
#[derive(Debug)]
pub struct Page {
    pub entries: Vec<Entry>,
    pub has_more: bool,
}

/// Re-read a bounded prefix rather than append by offset: concurrent reflog
/// entries would otherwise shift offsets and duplicate/omit recovery candidates.
/// Includes HEAD and local/remote ref logs, retaining distinct events for the
/// same commit. Does not expire logs, scan unreachable objects, or run GC.
pub fn read(repo: &Repository, limit: usize, token: &Cancel) -> Result<Page, String> {
    if !(1..=MAX_ENTRIES).contains(&limit) {
        return Err("Invalid reflog page size".into());
    }
    let count = (limit + 1).to_string();
    let args = [
        "reflog",
        "show",
        "--all",
        "--date=unix",
        "--no-abbrev",
        "--format=%H%x00%gD%x00%gs%x00%gn%x00%ge",
        "-z",
        "-n",
        &count,
    ];
    let output = repo.read(&args.map(OsStr::new), token)?;
    let mut entries = parse(&output)?;
    let has_more = entries.len() > limit;
    entries.truncate(limit);
    Ok(Page { entries, has_more })
}
fn parse(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let bytes = bytes
        .strip_suffix(b"\0")
        .ok_or("Incomplete reflog response")?;
    let fields: Vec<_> = bytes.split(|b| *b == 0).collect();
    if fields.len() % 5 != 0 {
        return Err("Malformed reflog response".into());
    }
    fields
        .as_chunks::<5>()
        .0
        .iter()
        .map(|row| {
            let commit = std::str::from_utf8(row[0]).map_err(|_| "Invalid reflog commit")?;
            if !is_oid(commit) {
                return Err("Invalid reflog commit".into());
            }
            let selector = std::str::from_utf8(row[1]).map_err(|_| "Invalid reflog selector")?;
            let (reference, date) = selector
                .rsplit_once("@{")
                .ok_or("Invalid reflog selector")?;
            let timestamp = date
                .strip_suffix('}')
                .ok_or("Invalid reflog timestamp")?
                .parse::<i64>()
                .map_err(|_| "Invalid reflog timestamp")?;
            if reference.is_empty() {
                return Err("Invalid reflog reference".into());
            }
            Ok(Entry {
                commit: commit.to_owned(),
                reference: reference.to_owned(),
                timestamp,
                action: String::from_utf8_lossy(row[2]).into_owned(),
                author: String::from_utf8_lossy(row[3]).into_owned(),
                email: String::from_utf8_lossy(row[4]).into_owned(),
            })
        })
        .collect()
}
