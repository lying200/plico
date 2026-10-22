//! Boolean ASCII core of Rebased 4aec352 MinusculeMatcherImpl / NameUtilCore.
//! JetBrains and contributors, Apache-2.0. Not yet the full NameUtil builder:
//! Unicode, layout fallback, pinyin and match scoring are pending.
use std::collections::HashMap;

pub struct AsciiMatcher {
    pattern: Vec<u8>,
    meaningful: Vec<u8>,
    mixed: bool,
    dots: bool,
}
impl AsciiMatcher {
    /// Raw NameUtil pattern, including any leading wildcard supplied by caller.
    /// Unsupported input is explicit, never silently treated as a non-match.
    pub fn new(pattern: &str) -> Option<Self> {
        let pattern = pattern.strip_suffix("* ").unwrap_or(pattern);
        if !pattern.is_ascii() {
            return None;
        }
        let mut seen = false;
        let mut lower = false;
        let mut upper = false;
        for c in pattern.bytes() {
            lower |= c.is_ascii_lowercase();
            upper |= seen && c.is_ascii_uppercase();
            seen |= !wild(c);
        }
        Some(Self {
            pattern: pattern.as_bytes().to_vec(),
            meaningful: pattern.bytes().filter(|c| !wild(*c)).collect(),
            mixed: lower && upper,
            dots: pattern.contains('.'),
        })
    }
    pub fn matches(&self, name: &str) -> Option<bool> {
        if !name.is_ascii() {
            return None;
        }
        if name.len() < self.meaningful.len() {
            return Some(false);
        }
        if self.pattern.len() > 100 {
            return Some(self.substring(name.as_bytes()));
        }
        // MinusculeMatcher.nameContainsAllMeaningfulCharsInOrder: cheaply
        // reject impossible candidates before allocating the recursive memo.
        let mut remaining = name.as_bytes();
        for expected in &self.meaningful {
            let Some(index) = remaining
                .iter()
                .position(|c| expected.eq_ignore_ascii_case(c))
            else {
                return Some(false);
            };
            remaining = &remaining[index + 1..];
        }
        Some(self.at(name.as_bytes(), 0, 0, &mut HashMap::new()))
    }
    // Upstream long-prefix mode removes pattern wildcards, while allowing
    // wildcard characters in the candidate to be skipped. It reports consumed
    // candidate length, even if candidate end interrupts the meaningful prefix.
    // Preserve that behavior rather than replacing it with a generic glob.
    fn substring(&self, name: &[u8]) -> bool {
        let match_at = |start: usize| {
            let mut meaningful = 0;
            let mut pos = start;
            while pos < name.len() && meaningful < self.meaningful.len() {
                if self.meaningful[meaningful].eq_ignore_ascii_case(&name[pos]) {
                    meaningful += 1;
                } else if !wild(name[pos]) {
                    return false;
                }
                pos += 1;
            }
            pos != start
        };
        if self.pattern.first() == Some(&b'*') {
            (0..=name.len() - self.meaningful.len()).any(match_at)
        } else {
            match_at(0)
        }
    }
    fn at(
        &self,
        n: &[u8],
        p: usize,
        start: usize,
        memo: &mut HashMap<(usize, usize), bool>,
    ) -> bool {
        if let Some(&result) = memo.get(&(p, start)) {
            return result;
        }
        let result = if p == self.pattern.len() {
            true
        } else if self.is_wild(p) {
            let mut next = p;
            while self.is_wild(next) {
                next += 1;
            }
            if next == self.pattern.len() {
                !self.pattern.ends_with(b" ")
                    || start == n.len()
                    || (next >= 2
                        && self.pattern[next - 2].is_ascii_alphanumeric()
                        && !self.pattern[next - 2].is_ascii_lowercase())
                    || n.get(start..).is_some_and(|s| s.contains(&b' '))
            } else {
                self.skip(n, next, start, true, memo)
            }
        } else {
            self.fragment(n, p, start, memo)
        };
        memo.insert((p, start), result);
        result
    }
    fn is_wild(&self, p: usize) -> bool {
        self.pattern.get(p).is_some_and(|c| wild(*c))
    }
    fn equals(&self, p: usize, c: u8) -> bool {
        self.pattern[p].eq_ignore_ascii_case(&c)
    }
    fn occurrence(&self, n: &[u8], p: usize, start: usize) -> Option<usize> {
        let unrestricted = p > 0 && self.pattern[p - 1] == b'*' || separator(self.pattern[p]);
        if !unrestricted
            && self.mixed
            && self.pattern[p].is_ascii_lowercase()
            && !(p > 0 && separator(self.pattern[p - 1]))
        {
            return None;
        }
        (start..n.len()).find(|&i| {
            self.equals(p, n[i])
                && (unrestricted || !self.pattern[p].is_ascii_alphanumeric() || word_start(n, i))
        })
    }
    fn skip(
        &self,
        n: &[u8],
        p: usize,
        start: usize,
        allow_dots: bool,
        memo: &mut HashMap<(usize, usize), bool>,
    ) -> bool {
        let mut from = start;
        while let Some(i) = self.occurrence(n, p, from) {
            if !allow_dots
                && self.dots
                && self.pattern.get(p.wrapping_sub(1)) != Some(&b'.')
                && n[from..i].contains(&b'.')
            {
                return false;
            }
            let valid_start = !self.pattern[p].is_ascii_uppercase()
                || n[i].is_ascii_uppercase()
                || word_start(n, i)
                || !self.mixed;
            if valid_start && self.at(n, p, i, memo) {
                return true;
            }
            from = i + 1;
        }
        false
    }
    fn fragment(
        &self,
        n: &[u8],
        p: usize,
        start: usize,
        memo: &mut HashMap<(usize, usize), bool>,
    ) -> bool {
        if start >= n.len() || !self.equals(p, n[start]) {
            return false;
        }
        let mut length = 1;
        while start + length < n.len() && p + length < self.pattern.len() {
            if !self.equals(p + length, n[start + length]) {
                if self.pattern[p + length].is_ascii_digit()
                    && self.pattern[p + length - 1].is_ascii_digit()
                    && n[start + length].is_ascii_digit()
                {
                    return false;
                }
                break;
            }
            length += 1;
        }
        if p + length == self.pattern.len() {
            return true;
        }
        let middle = p > 0
            && self.pattern[p - 1] == b'*'
            && !self.is_wild(p + 1)
            && n[start].is_ascii_alphanumeric()
            && !word_start(n, start);
        let minimum = if middle { 3 } else { 1 };
        for length in (1..=length).rev() {
            if length < minimum && !self.is_wild(p + length) {
                continue;
            }
            if self.is_wild(p + length) {
                if self.at(n, p + length, start + length, memo) {
                    return true;
                }
            } else if let Some(next) = self.occurrence(n, p + length, start + length + 1) {
                if self.dots
                    && self.pattern[p + length - 1] != b'.'
                    && n[start + length..next].contains(&b'.')
                {
                    continue;
                }
                if self.skip(n, p + length, next, false, memo) {
                    return true;
                }
            }
        }
        false
    }
}
fn wild(c: u8) -> bool {
    c == b' ' || c == b'*'
}
fn separator(c: u8) -> bool {
    c.is_ascii_whitespace() || matches!(c, b'_' | b'-' | b':' | b'+' | b'.')
}
fn word_start(n: &[u8], i: usize) -> bool {
    let c = n[i];
    if c.is_ascii_uppercase() {
        return i == 0
            || !n[i - 1].is_ascii_uppercase()
            || n.get(i + 1).is_some_and(u8::is_ascii_lowercase);
    }
    if c.is_ascii_digit() {
        return true;
    }
    c.is_ascii_alphabetic() && (i == 0 || !n[i - 1].is_ascii_alphanumeric())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_simple_humps_and_word_skipping() {
        for pattern in ["N", "NU", "NUT", "NaUT", "NaUtT", "NaUtTe", "nt", "replmap"] {
            let name = if pattern == "replmap" {
                "ReplacePathToMacroMap"
            } else {
                "NameUtilTest"
            };
            assert_eq!(
                AsciiMatcher::new(pattern).unwrap().matches(name),
                Some(true),
                "{pattern}"
            );
        }
        for (pattern, name) in [
            ("NeUT", "NameUtilTest"),
            ("NaUTa", "NameUtilTest"),
            ("N", "anameUtilTest"),
            ("ABCD", "AbstractButton.DISABLED_ICON_CHANGED_PROPERTY"),
            ("alias imple", "alias simple"),
            ("alias nother", "alias another"),
        ] {
            assert_eq!(
                AsciiMatcher::new(pattern).unwrap().matches(name),
                Some(false),
                "{pattern} {name}"
            );
        }
        for (pattern, name) in [
            ("templipa", "template_impl_template_list_panel"),
            ("na te", "name_util_test"),
            ("repl map", "ReplacePathToMacroMap"),
            ("AACl", "AAClass"),
        ] {
            assert_eq!(
                AsciiMatcher::new(pattern).unwrap().matches(name),
                Some(true),
                "{pattern}"
            );
        }
    }
    #[test]
    fn unsupported_modes_are_explicit_and_wildcard_can_find_middle_text() {
        assert!(AsciiMatcher::new("中文").is_none());
        assert_eq!(AsciiMatcher::new("*").unwrap().matches("中文"), None);
        assert_eq!(
            AsciiMatcher::new("*cache")
                .unwrap()
                .matches("feature/cache-limit"),
            Some(true)
        );
        assert_eq!(
            AsciiMatcher::new("*ft").unwrap().matches("feature/topic"),
            Some(true)
        );
        assert_eq!(
            AsciiMatcher::new("*13").unwrap().matches("version123"),
            Some(false)
        );
    }
    #[test]
    fn long_prefix_follows_upstream_threshold_and_non_glob_fallback() {
        let short = format!("{}B", "a".repeat(99));
        assert_eq!(
            AsciiMatcher::new(&short)
                .unwrap()
                .matches(&format!("{}somethingB", "a".repeat(99))),
            Some(true)
        );
        let long = format!("{}B", "a".repeat(100));
        assert_eq!(
            AsciiMatcher::new(&long)
                .unwrap()
                .matches(&format!("{}somethingB", "a".repeat(100))),
            Some(false)
        );
        assert_eq!(
            AsciiMatcher::new(&long)
                .unwrap()
                .matches(&long.to_uppercase()),
            Some(true)
        );
        assert_eq!(
            AsciiMatcher::new(&long)
                .unwrap()
                .matches(&format!("prefix/{long}")),
            Some(false)
        );
        assert_eq!(
            AsciiMatcher::new(&format!("*{long}"))
                .unwrap()
                .matches(&format!("prefix/{long}")),
            Some(true)
        );
        // More than 100 wildcard characters don't mean a normal glob match.
        assert_eq!(
            AsciiMatcher::new(&"*".repeat(101)).unwrap().matches("main"),
            Some(false)
        );
        let spaced = format!("{}a", "*".repeat(101));
        assert_eq!(
            AsciiMatcher::new(&spaced).unwrap().matches(" a"),
            Some(true)
        );
        // This surprising partial-at-end result is present in meaningfulCharsMatchAt.
        let partial = format!("{}ab", "*".repeat(101));
        assert_eq!(
            AsciiMatcher::new(&partial).unwrap().matches(" a"),
            Some(true)
        );
        assert_eq!(
            AsciiMatcher::new(&long).unwrap().matches("short"),
            Some(false)
        );
    }
}

#[cfg(test)]
mod upstream_corpus {
    #[test]
    fn literal_ascii_boolean_assertions() {
        for line in include_str!("../tests/fixtures/minuscule-ascii.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let mut fields = line.split('\t');
            let expected = fields.next().unwrap() == "1";
            let pattern = fields.next().unwrap();
            let name = fields.next().unwrap();
            assert_eq!(
                super::AsciiMatcher::new(pattern).unwrap().matches(name),
                Some(expected),
                "pattern={pattern:?}, name={name:?}"
            );
        }
    }
}
