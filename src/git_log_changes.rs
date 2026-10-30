//! Explicit per-parent comparisons in bounded Git processes, including empty diffs.
use crate::git::{Cancel, Change, Commit, Repository, Result};
use std::ffi::OsStr;

struct Query<'a> {
    commit: &'a Commit,
    parent: Option<&'a str>,
    last: bool,
}

impl Repository {
    /// Caller order is retained. Parent records are never inferred from absent output.
    pub fn log_commit_changes_batch(
        &self,
        commits: &[&Commit],
        token: &Cancel,
    ) -> Result<Vec<Vec<crate::combined_changes::Change>>> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if commits.len() > crate::git::HISTORY_MAX {
            return Err("Log selection exceeds the commit limit".into());
        }
        let mut queries = Vec::new();
        for commit in commits {
            if !crate::git::is_oid(&commit.id)
                || commit.parents.iter().any(|p| !crate::git::is_oid(p))
            {
                return Err("Invalid commit ID".into());
            }
            if commit.shallow_boundary {
                return Err("Log preview is unavailable at shallow history boundary".into());
            }
            if commit.parents.len().max(1) > 100_000usize.saturating_sub(queries.len()) {
                return Err("Log parent comparisons exceed the preview limit".into());
            }
            if commit.parents.is_empty() {
                queries.push(Query {
                    commit,
                    parent: None,
                    last: true,
                });
            } else {
                for (index, parent) in commit.parents.iter().enumerate() {
                    queries.push(Query {
                        commit,
                        parent: Some(parent),
                        last: index + 1 == commit.parents.len(),
                    });
                }
            }
        }
        let mut result = Vec::with_capacity(commits.len());
        let mut groups = Vec::new();
        let mut parent_count = 0usize;
        let mut total = 0usize;
        // A batch may exceed the output cap even when each individual comparison
        // fits. Bisect only that read, preserving order and the existing per-read
        // limit instead of rejecting a previously valid selection.
        let mut pending: Vec<_> = (0..queries.len())
            .step_by(32)
            .map(|start| (start, (start + 32).min(queries.len())))
            .collect();
        pending.reverse();
        while let Some((start, end)) = pending.pop() {
            let chunk = &queries[start..end];
            let input: String = chunk
                .iter()
                .map(|q| match q.parent {
                    Some(parent) => format!("{} {parent}\n", q.commit.id),
                    None => format!("{}\n", q.commit.id),
                })
                .collect();
            let bytes = self.read_with_input(
                &[
                    "diff-tree",
                    "--stdin",
                    "--always",
                    "--root",
                    "-r",
                    "--name-status",
                    "-z",
                    "--find-renames",
                    "--ignore-submodules=none",
                    "--no-ext-diff",
                    "--no-textconv",
                ]
                .map(OsStr::new),
                input.into_bytes(),
                token,
            );
            let bytes = match bytes {
                Ok(bytes) => bytes,
                Err(error)
                    if end - start > 1
                        && error
                            == "Git output exceeds the 8 MiB limit; choose a smaller scope" =>
                {
                    let middle = start + (end - start) / 2;
                    pending.push((middle, end));
                    pending.push((start, middle));
                    continue;
                }
                Err(error) => return Err(error),
            };
            let mut remaining = bytes.as_slice();
            for query in chunk {
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                let files = read_record(&mut remaining, &query.commit.id)?;
                parent_count = parent_count.saturating_add(files.len());
                if parent_count > crate::combined_changes::MAX_INPUT_CHANGES {
                    return Err("Log preview exceeds the change limit".into());
                }
                groups.push(files);
                if query.last {
                    let merged = crate::log_changes::merge_parents(&groups)?;
                    total = total.saturating_add(merged.len());
                    if total > crate::combined_changes::MAX_INPUT_CHANGES {
                        return Err("Log preview exceeds the change limit".into());
                    }
                    result.push(
                        merged
                            .iter()
                            .map(|file| {
                                crate::combined_changes::from_file(
                                    file,
                                    &query.commit.id,
                                    query.commit.parents.first().map(String::as_str),
                                )
                            })
                            .collect::<Result<Vec<_>>>()?,
                    );
                    groups.clear();
                    parent_count = 0;
                }
            }
            if !remaining.is_empty() {
                return Err("Unexpected extra parent comparison".into());
            }
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(result)
    }
}

fn field<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8]> {
    let end = bytes
        .iter()
        .position(|&b| b == 0)
        .ok_or("Incomplete parent comparison")?;
    let value = &bytes[..end];
    *bytes = &bytes[end + 1..];
    Ok(value)
}

fn read_record(bytes: &mut &[u8], id: &str) -> Result<Vec<Change>> {
    if !field(bytes)?.eq_ignore_ascii_case(id.as_bytes()) {
        return Err("Unexpected parent comparison identity".into());
    }
    let start = *bytes;
    let mut count = 0usize;
    while !bytes.is_empty() {
        let mut probe = *bytes;
        let status = field(&mut probe)?;
        if std::str::from_utf8(status).is_ok_and(crate::git::is_oid) {
            break;
        }
        if !matches!(
            status.first(),
            Some(b'A' | b'D' | b'M' | b'T' | b'R' | b'C')
        ) {
            return Err("Invalid parent comparison status".into());
        }
        field(&mut probe)?;
        if matches!(status.first(), Some(b'R' | b'C')) {
            field(&mut probe)?;
        }
        count += 1;
        if count > crate::combined_changes::MAX_INPUT_CHANGES {
            return Err("Log preview exceeds the change limit".into());
        }
        *bytes = probe;
    }
    crate::git::parse_names(&start[..start.len() - bytes.len()])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parent_protocol_keeps_hex_paths_and_empty_records_separate() {
        let id = "1".repeat(40);
        let raw = format!("{id}\0R100\0{id}\0new\nname\0{id}\0");
        let mut bytes = raw.as_bytes();
        let files = read_record(&mut bytes, &id).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].old_path.as_ref().unwrap().to_str(),
            Some(id.as_str())
        );
        assert_eq!(files[0].path.to_str(), Some("new\nname"));
        assert!(read_record(&mut bytes, &id).unwrap().is_empty());
        assert!(bytes.is_empty());
        for bad in [
            format!("{id}\0R100\0old\0"),
            format!("{id}\0M\0path"),
            format!("{id}\0?\0path\0"),
            format!("{}\0", "2".repeat(40)),
        ] {
            assert!(read_record(&mut bad.as_bytes(), &id).is_err());
        }
    }
}
