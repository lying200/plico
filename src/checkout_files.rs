//! Display-only file lists from Rebased GitMessageWithFilesDetector start/end
//! markers. Never use these human-readable paths as Git command arguments.
//! Reference 4aec35256d765c217aa0d662b82ac4e860c396fd, Apache-2.0.
#[derive(Debug, Default)]
pub struct Files {
    pub paths: Vec<String>,
    pub complete: bool,
}

pub fn parse(bytes: &[u8], truncated: bool, untracked: bool) -> Files {
    let mut result = Files::default();
    let mut active = false;
    let mut found = false;
    let mut valid = !truncated;
    let start = if untracked {
        "The following untracked working tree files would be overwritten by"
    } else {
        "Your local changes to the following files would be overwritten by"
    };
    let end = if untracked {
        "Please move or remove them before"
    } else {
        "commit your changes or stash them before"
    };
    for raw in bytes.split(|b| *b == b'\n') {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        let line = String::from_utf8_lossy(raw);
        if line.contains(start) {
            if active {
                valid = false;
            }
            active = true;
            found = true;
            continue;
        }
        if active
            && (line.contains(end)
                || line.contains("Merge with strategy ") && line.contains(" failed.")
                || line.contains("No merge strategy handled the merge."))
        {
            active = false;
            continue;
        }
        if !active {
            continue;
        }
        // Modern checkout uses one tab-indented Git-quoted path per line.
        // Ambiguous legacy space-separated lists remain explicitly incomplete.
        let Some(path) = raw.strip_prefix(b"\t") else {
            valid = false;
            continue;
        };
        let Some(path) = unquote(path) else {
            valid = false;
            continue;
        };
        if result.paths.len() >= 200 {
            valid = false;
            continue;
        }
        if !result.paths.contains(&path) {
            result.paths.push(path);
        }
    }
    result.complete = found && !active && valid && !result.paths.is_empty();
    result
}

fn unquote(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(b"\"") {
        return String::from_utf8(bytes.to_vec())
            .ok()
            .filter(|s| !s.is_empty());
    }
    let bytes = bytes.strip_prefix(b"\"")?.strip_suffix(b"\"")?;
    let mut output = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        i += 1;
        if byte != b'\\' {
            output.push(byte);
            continue;
        }
        let escaped = *bytes.get(i)?;
        i += 1;
        output.push(match escaped {
            b'a' => 7,
            b'b' => 8,
            b't' => 9,
            b'n' => 10,
            b'v' => 11,
            b'f' => 12,
            b'r' => 13,
            b'\\' => b'\\',
            b'"' => b'"',
            b'0'..=b'3' => {
                let a = *bytes.get(i)?;
                let b = *bytes.get(i + 1)?;
                if !(b'0'..=b'7').contains(&a) || !(b'0'..=b'7').contains(&b) {
                    return None;
                }
                i += 2;
                (escaped - b'0') * 64 + (a - b'0') * 8 + (b - b'0')
            }
            _ => return None,
        });
    }
    String::from_utf8(output).ok().filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoted_names_and_incomplete_lists() {
        let output = b"error: Your local changes to the following files would be overwritten by checkout:\r\n\t leading space \r\n\t\"\\344\\270\\255.txt\"\r\n\t\"tab\\tline\\n\\\".txt\"\r\nPlease commit your changes or stash them before you switch branches.\r\nAborting\n";
        let result = parse(output, false, false);
        assert!(result.complete);
        assert_eq!(
            result.paths,
            [" leading space ", "中.txt", "tab\tline\n\".txt"]
        );
        assert!(!parse(output, true, false).complete);
        assert!(!parse(output, false, true).complete);
        assert!(
            !parse(
                b"Your local changes to the following files would be overwritten by checkout:\n\ta",
                false,
                false
            )
            .complete
        );
        assert!(!parse(b"Your local changes to the following files would be overwritten by checkout:\n  a b\nPlease commit your changes or stash them before checkout", false, false).complete);
        assert!(unquote(b"\"\\999\"").is_none());
    }
}
