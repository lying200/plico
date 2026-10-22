//! Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd:
//! push/GitPushNativeResultParser.java and GitPushNativeResult.java.
//! Copyright JetBrains s.r.o. and contributors, Apache-2.0.
//! Borrowed, lazy parsing avoids a second retained copy of command output.
//! Callers must distinguish missing/truncated output from a successful push.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Success,
    ForcedUpdate,
    Deleted,
    NewRef,
    Rejected,
    UpToDate,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RefResult<'a> {
    pub kind: Kind,
    pub source: &'a str,
    /// Retained locally for matching the actual result to the requested target.
    pub target: &'a str,
    pub summary: &'a str,
    pub reason: Option<&'a str>,
    pub range: Option<&'a str>,
}
impl RefResult<'_> {
    pub fn is_non_ff_update(&self) -> bool {
        self.kind == Kind::Rejected
            && self.reason.is_some_and(|reason| {
                ["non-fast-forward", "fetch first", "failed to lock"]
                    .iter()
                    .any(|needle| contains_ascii_case(reason, needle))
            })
    }
    pub fn is_stale_info(&self) -> bool {
        self.kind == Kind::Rejected
            && self
                .reason
                .is_some_and(|reason| contains_ascii_case(reason, "stale info"))
    }
}
fn contains_ascii_case(value: &str, needle: &str) -> bool {
    value
        .as_bytes()
        .windows(needle.len())
        .any(|part| part.eq_ignore_ascii_case(needle.as_bytes()))
}
fn is_range(summary: &str) -> bool {
    let Some(start) = summary.find('.') else {
        return false;
    };
    let end = summary[start..].bytes().take_while(|b| *b == b'.').count() + start;
    (2..=3).contains(&(end - start))
        && start > 0
        && end < summary.len()
        && summary[..start]
            .bytes()
            .chain(summary[end..].bytes())
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn parse(output: &str) -> impl Iterator<Item = RefResult<'_>> {
    output.lines().filter_map(parse_line)
}
fn parse_line(line: &str) -> Option<RefResult<'_>> {
    // Upstream accepts a prefix before the flag (e.g. progress output).
    // Search backwards, matching the greedy prefix in its regular expression.
    for (tab, _) in line.rmatch_indices('\t') {
        let Some(flag) = tab
            .checked_sub(1)
            .and_then(|i| line.as_bytes().get(i))
            .copied()
        else {
            continue;
        };
        let kind = match flag {
            b' ' => Kind::Success,
            b'+' => Kind::ForcedUpdate,
            b'-' => Kind::Deleted,
            b'*' => Kind::NewRef,
            b'!' => Kind::Rejected,
            b'=' => Kind::UpToDate,
            _ => continue,
        };
        let Some((refs, description)) = line[tab + 1..].split_once('\t') else {
            continue;
        };
        let Some((source, target)) = refs.rsplit_once(':') else {
            continue;
        };
        // Preserve upstream's nonempty source rule, including its deletion limitation.
        if source.is_empty() || target.is_empty() || refs.bytes().any(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let split = description.find('(').unwrap_or(description.len());
        if split == 0 {
            continue;
        }
        let summary = description[..split].trim();
        let reason = description
            .get(split + 1..)
            .and_then(|rest| rest.rfind(')').map(|end| &rest[..end]))
            .filter(|r| !r.is_empty());
        return Some(RefResult {
            kind,
            source,
            target,
            summary,
            reason,
            range: is_range(summary).then_some(summary),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_parser_cases() {
        use Kind::*;
        let cases = [
            (
                " \trefs/heads/master:refs/heads/master\t3e62822..a537351",
                Success,
                "refs/heads/master",
                Some("3e62822..a537351"),
                None,
            ),
            (
                " \t4915ba17cc8b718d58ae77dd25018fccdce322d6:refs/heads/master\t3e62822..4915ba1",
                Success,
                "4915ba17cc8b718d58ae77dd25018fccdce322d6",
                Some("3e62822..4915ba1"),
                None,
            ),
            (
                "!\trefs/heads/master:refs/heads/master\t[rejected] (non-fast-forward)",
                Rejected,
                "refs/heads/master",
                None,
                Some("non-fast-forward"),
            ),
            (
                "!\trefs/heads/master:refs/heads/master\t[rejected] (fetch first)",
                Rejected,
                "refs/heads/master",
                None,
                Some("fetch first"),
            ),
            (
                "+\trefs/heads/master:refs/heads/master\tb9b3235...23760f8 (forced update)",
                ForcedUpdate,
                "refs/heads/master",
                Some("b9b3235...23760f8"),
                Some("forced update"),
            ),
            (
                "!\trefs/heads/master:refs/heads/master\t[remote rejected] (pre-receive hook declined)",
                Rejected,
                "refs/heads/master",
                None,
                Some("pre-receive hook declined"),
            ),
            (
                "=\trefs/heads/master:refs/heads/master\t[up to date]",
                UpToDate,
                "refs/heads/master",
                None,
                None,
            ),
            (
                "*\trefs/heads/feature:refs/heads/feature2\t[new branch]",
                NewRef,
                "refs/heads/feature",
                None,
                None,
            ),
        ];
        for (line, kind, source, range, reason) in cases {
            let output = format!(
                "Counting objects: 20, done.\nTo http://example.invalid/repo\n{line}\nDone\n"
            );
            let results: Vec<_> = parse(&output).collect();
            assert_eq!(results.len(), 1);
            let result = &results[0];
            assert_eq!(
                (result.kind, result.source, result.range, result.reason),
                (kind, source, range, reason)
            );
        }
        for source in [
            "refs/heads/master",
            "4915ba17cc8b718d58ae77dd25018fccdce322d6",
        ] {
            let output = format!(
                " \t{source}:refs/heads/main\t3e62822..4915ba1\n*\trefs/tags/some_tag:refs/tags/some_tag\t[new tag]"
            );
            let results: Vec<_> = parse(&output).collect();
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].source, source);
            assert_eq!(results[1].kind, NewRef);
            assert_eq!(results[1].target, "refs/tags/some_tag");
        }
    }
    #[test]
    fn rejection_rules_and_unrecognized_output() {
        for (reason, non_ff, stale) in [
            ("NON-FAST-FORWARD", true, false),
            ("fetch first", true, false),
            ("failed to lock", true, false),
            ("stale info", false, true),
            ("pre-receive hook declined", false, false),
        ] {
            for flag in ['!', '+'] {
                let line =
                    format!("{flag}\trefs/heads/main:refs/heads/main\t[rejected] ({reason})");
                let result = parse(&line).next().unwrap();
                assert_eq!(result.is_non_ff_update(), non_ff && flag == '!');
                assert_eq!(result.is_stale_info(), stale && flag == '!');
            }
        }
        assert_eq!(parse("fatal: authentication failed\nDone\n!\tbroken\t[rejected]\n-\t:refs/heads/main\t[deleted]").count(), 0);
        assert_eq!(
            parse("prefix *\trefs/heads/中文:refs/heads/中文\t[new branch]\r\n")
                .next()
                .unwrap()
                .target,
            "refs/heads/中文"
        );
        for value in ["a.b", "a....b", "a..", "..b", "A..b", "z..b"] {
            assert!(!is_range(value));
        }
    }
}
