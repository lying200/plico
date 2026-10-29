//! Bounded Push change batches. See docs/PERFORMANCE-AUDIT-20261029.md.
use super::*;
use std::io::{self, BufRead, BufReader};

impl Repository {
    /// Preserve caller order (oldest first for zipChanges), without walking ancestors.
    /// 128 SHA-256 IDs fit well below Windows' command line limit. Output is streamed
    /// one commit at a time, retaining the existing per-commit byte/change limits.
    pub fn push_changes_batch(
        &self,
        commits: &[Commit],
        token: &Cancel,
    ) -> Result<Vec<crate::combined_changes::Change>> {
        if commits
            .iter()
            .any(|c| !is_oid(&c.id) || c.parents.iter().any(|p| !is_oid(p)))
        {
            return Err("Invalid commit ID".into());
        }
        if commits.iter().any(|c| c.shallow_boundary) {
            return Err("Push preview is unavailable at shallow history boundary".into());
        }
        let mut changes = Vec::new();
        for chunk in commits.chunks(128) {
            let mut args = vec![
                "show",
                "--no-walk=unsorted",
                "--format=%x00%H",
                "--root",
                "-c",
                "--name-status",
                "-z",
                "--find-renames",
                "--ignore-submodules=none",
                "--no-ext-diff",
                "--no-textconv",
                "--no-notes",
                "--no-decorate",
                "--no-show-signature",
            ];
            args.extend(chunk.iter().map(|c| c.id.as_str()));
            args.push("--");
            let expected = chunk.to_vec();
            let remaining = crate::combined_changes::MAX_INPUT_CHANGES - changes.len();
            let batch = self.read_stream(
                &args.iter().map(OsStr::new).collect::<Vec<_>>(),
                token,
                move |out, _| read_batch(BufReader::new(out), &expected, remaining),
            )?;
            changes.extend(batch);
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(changes)
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

// Empty NUL fields separate records; paths cannot be empty. Consume paths as raw
// bytes, so newlines, header-looking names and non-UTF-8 paths remain unambiguous.
fn field(reader: &mut impl BufRead, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    let n = reader.take(limit as u64 + 1).read_until(0, &mut bytes)?;
    if n == 0 {
        return Ok(None);
    }
    if n > limit {
        return Err(invalid("Git output exceeds the per-commit 8 MiB limit"));
    }
    if bytes.pop() != Some(0) {
        return Err(invalid("Truncated push changes"));
    }
    Ok(Some(bytes))
}

fn read_batch(
    mut reader: impl BufRead,
    commits: &[Commit],
    max_changes: usize,
) -> io::Result<Vec<crate::combined_changes::Change>> {
    if field(&mut reader, 1)? != Some(Vec::new()) {
        return Err(invalid("Missing push changes header"));
    }
    let mut result = Vec::new();
    for commit in commits {
        if field(&mut reader, 65)?.as_deref() != Some(commit.id.as_bytes()) {
            return Err(invalid("Push changes commit order changed"));
        }
        // Combined diff uses an extra NUL instead of the ordinary diff's newline,
        // including clean merges with no combined paths.
        if commit.parents.len() > 1 && field(&mut reader, 1)? != Some(Vec::new()) {
            return Err(invalid("Missing combined changes separator"));
        }
        let mut bytes = Vec::new();
        // Framing (record delimiter / first-status newline) is not part of the
        // old diff-tree byte budget, including exactly-at-limit records.
        while let Some(mut value) = field(&mut reader, OUTPUT_LIMIT - bytes.len() + 2)? {
            if value.is_empty() {
                break;
            }
            // Git inserts one newline between the pretty header and first status.
            // Only strip it at this boundary, never from a pathname.
            if bytes.is_empty() && commit.parents.len() <= 1 {
                if value.first() != Some(&b'\n') {
                    return Err(invalid("Missing push changes separator"));
                }
                value.remove(0);
            }
            if value.len() + 1 > OUTPUT_LIMIT - bytes.len() {
                return Err(invalid("Git output exceeds the per-commit 8 MiB limit"));
            }
            bytes.extend(value);
            bytes.push(0);
        }
        let changes = parse(commit, &bytes).map_err(invalid)?;
        if changes.len() > max_changes.saturating_sub(result.len()) {
            return Err(invalid("Combined preview exceeds the change limit"));
        }
        result.extend(changes);
    }
    if !reader.fill_buf()?.is_empty() {
        return Err(invalid("Unexpected push changes record"));
    }
    Ok(result)
}

pub(super) fn parse(commit: &Commit, bytes: &[u8]) -> Result<Vec<crate::combined_changes::Change>> {
    let files = if commit.parents.len() <= 1 {
        parse_names(bytes)?
    } else {
        // Without --combined-all-paths Git emits one destination path per
        // combined status, including renames. Rebased takes status[0] and
        // falls back to that path when MOVED has no second path.
        let mut fields = bytes.split(|b| *b == 0);
        let mut files = Vec::new();
        while let Some(status) = fields.next() {
            if status.is_empty() {
                if fields.next().is_some() {
                    return Err("Malformed combined status".into());
                }
                break;
            }
            if status.len() != commit.parents.len() || !status.iter().all(|c| b"AMDTCR".contains(c))
            {
                return Err("Unsupported combined status".into());
            }
            let path = path_from_bytes(
                fields
                    .next()
                    .filter(|p| !p.is_empty())
                    .ok_or("Missing combined path")?,
            )?;
            files.push(Change {
                old_path: matches!(status[0], b'R' | b'C').then(|| path.clone()),
                path,
                status: String::from_utf8(vec![status[0]]).unwrap(),
                area: Area::Commit,
            });
        }
        files
    };
    files
        .iter()
        .map(|file| {
            crate::combined_changes::from_file(
                file,
                &commit.id,
                commit.parents.first().map(String::as_str),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(digit: char) -> Commit {
        Commit {
            id: digit.to_string().repeat(40),
            parents: vec![],
            shallow_boundary: false,
            committer_timestamp: 0,
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            refs: vec![],
        }
    }

    #[test]
    fn push_batch_empty_records_and_literal_paths() {
        let commits = [commit('a'), commit('b'), commit('c')];
        let bytes = format!(
            "\0{}\0\0{}\0\nA\0M\0\0{}\0",
            commits[0].id, commits[1].id, commits[2].id
        );
        let changes = read_batch(bytes.as_bytes(), &commits, 10).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].after.as_ref().unwrap().path, PathBuf::from("M"));
        assert_eq!(changes[0].after.as_ref().unwrap().revision, commits[1].id);
        assert!(read_batch(bytes.as_bytes(), &commits, 0).is_err());
        assert!(read_batch(bytes.as_bytes(), &commits[..2], 10).is_err());
        assert!(read_batch(bytes.as_bytes(), &[commit('d')], 10).is_err());
    }

    #[test]
    fn push_batch_rejects_truncation_and_bounds_fields() {
        let c = commit('a');
        let bytes = format!("\0{}\0\nA\0file", c.id);
        assert!(read_batch(bytes.as_bytes(), &[c], 10).is_err());
        assert!(field(&mut &b"abcd\0"[..], 4).is_err());
        assert_eq!(
            field(&mut &b"abcd\0"[..], 5).unwrap(),
            Some(b"abcd".to_vec())
        );
    }

    #[test]
    fn push_batch_byte_limit_excludes_record_framing() {
        let commits = [commit('a'), commit('b')];
        // A\0<path>\0 fills exactly the existing per-commit byte allowance.
        // The following empty commit must not make that valid record overflow.
        let path = "x".repeat(OUTPUT_LIMIT - 3);
        let bytes = format!("\0{}\0\nA\0{path}\0\0{}\0", commits[0].id, commits[1].id);
        let result = read_batch(bytes.as_bytes(), &commits, 1).unwrap();
        assert_eq!(result.len(), 1);
        drop(result);
        let oversized = format!("\0{}\0\nA\0{path}x\0", commits[0].id);
        assert!(read_batch(oversized.as_bytes(), &commits[..1], 1).is_err());
    }
}
