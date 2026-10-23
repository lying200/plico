//! Selected-file commit index preservation, following Rebased's
//! GitIndexInfoStagingAreaStateManager (4aec35256d765c217aa0d662b82ac4e860c396fd).
//! This is a plan only: the commit executor must hold repository ownership from
//! capture through restore, check preparation errors, and restore even on cancel.
//! Input: `status --porcelain=2 -z --no-renames --no-ahead-behind --no-show-stash
//! --untracked-files=no --ignored=no`.
use std::collections::HashSet;

const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Debug)]
struct Entry {
    path: Vec<u8>,
    head_mode: Vec<u8>,
    index_mode: Vec<u8>,
    head_oid: Vec<u8>,
    index_oid: Vec<u8>,
}

/// Raw repository-relative path bytes avoid quoting and newline ambiguities.
/// Selected paths must include both ends of a selected rename.
#[derive(Debug, Default)]
pub struct ExcludedIndex {
    entries: Vec<Entry>,
}

impl ExcludedIndex {
    pub fn capture(status: &[u8], selected: &HashSet<Vec<u8>>) -> Result<Self, String> {
        if status.len() > LIMIT {
            return Err("Commit status exceeds the 8 MiB limit".into());
        }
        if !status.is_empty() && status.last() != Some(&0) {
            return Err("Truncated commit status".into());
        }
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        if status.is_empty() {
            return Ok(Self::default());
        }
        for record in status[..status.len() - 1].split(|b| *b == 0) {
            if record.starts_with(b"u ") {
                return Err("Cannot commit with unmerged paths".into());
            }
            // --no-renames makes rename endpoints ordinary deletion/addition records.
            // Reject unexpected formats rather than risk committing an excluded path.
            let fields: Vec<_> = record.splitn(9, |b| *b == b' ').collect();
            if fields.len() != 9 || fields[0] != b"1" || fields[1].len() != 2 {
                return Err("Unexpected commit status record".into());
            }
            let path = fields[8];
            if path.is_empty()
                || path.starts_with(b"/")
                || path
                    .split(|b| *b == b'/')
                    .any(|p| p == b".." || p.is_empty())
                || !seen.insert(path)
            {
                return Err("Invalid or repeated commit status path".into());
            }
            for (mode, oid) in [(fields[3], fields[6]), (fields[4], fields[7])] {
                if !matches!(
                    mode,
                    b"000000" | b"100644" | b"100755" | b"120000" | b"160000"
                ) || !matches!(oid.len(), 40 | 64)
                    || !oid.iter().all(u8::is_ascii_hexdigit)
                    || (mode == b"000000") != oid.iter().all(|b| *b == b'0')
                {
                    return Err("Invalid commit index identity".into());
                }
            }
            if fields[6].len() != fields[7].len() {
                return Err("Mixed commit object formats".into());
            }
            if fields[1][0] == b'.' || selected.contains(path) {
                continue;
            }
            entries.push(Entry {
                path: path.to_vec(),
                head_mode: fields[3].to_vec(),
                index_mode: fields[4].to_vec(),
                head_oid: fields[6].to_vec(),
                index_oid: fields[7].to_vec(),
            });
        }
        Ok(Self { entries })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Feed to `git update-index -z --index-info` before staging selected files.
    pub fn prepare_input(&self) -> Vec<u8> {
        self.input(true)
    }

    /// Feed to the same command after success OR failure. No worktree reads.
    pub fn restore_input(&self) -> Vec<u8> {
        self.input(false)
    }

    fn input(&self, head: bool) -> Vec<u8> {
        let mut out = Vec::new();
        // Remove all endpoints first, so file/directory replacements restore
        // independently of Git's status ordering.
        for entry in &self.entries {
            out.extend_from_slice(b"0 ");
            out.extend(std::iter::repeat_n(b'0', entry.head_oid.len()));
            out.push(b'\t');
            out.extend_from_slice(&entry.path);
            out.push(0);
        }
        for entry in &self.entries {
            let (mode, oid) = if head {
                (&entry.head_mode, &entry.head_oid)
            } else {
                (&entry.index_mode, &entry.index_oid)
            };
            if mode == b"000000" {
                continue;
            }
            out.extend_from_slice(mode);
            out.push(b' ');
            out.extend_from_slice(oid);
            out.extend_from_slice(b" 0\t");
            out.extend_from_slice(&entry.path);
            out.push(0);
        }
        out
    }
}
