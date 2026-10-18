//! UTF-16 matching core translated from Rebased 4aec352 MinusculeMatcherImpl.
//! JetBrains and contributors, Apache-2.0. IGNORE_CASE, empty hard separators.
//! NameUtil wrappers are not yet composed here.
use crate::search_unicode as unicode;
use std::collections::HashSet;
use std::ops::Range;
type Fragments = Vec<Range<usize>>;

pub struct Utf16Matcher {
    pattern: Vec<u16>,
    meaningful: Vec<u16>,
    mixed: bool,
    dots: bool,
}
impl Utf16Matcher {
    /// Raw NameUtil pattern, including any leading wildcard supplied by caller.
    pub fn new(pattern: &str) -> Self {
        let pattern = pattern.strip_suffix("* ").unwrap_or(pattern);
        let mut seen = false;
        let mut lower = false;
        let mut upper = false;
        for c in pattern.encode_utf16() {
            lower |= unicode::lower(c as u32);
            upper |= seen && unicode::upper(c as u32);
            seen |= !wild(c);
        }
        Self {
            pattern: pattern.encode_utf16().collect(),
            meaningful: pattern.encode_utf16().filter(|c| !wild(*c)).collect(),
            mixed: lower && upper,
            dots: pattern.contains('.'),
        }
    }
    pub fn matches(&self, name: &str) -> bool {
        self.find(name).is_some()
    }
    /// Ordered, coalesced UTF-16 ranges. Empty success differs from no match.
    pub fn find(&self, name: &str) -> Option<Fragments> {
        let name: Vec<u16> = name.encode_utf16().collect();
        if name.len() < self.meaningful.len() {
            return None;
        }
        if self.pattern.len() > 100 {
            return self.substring(&name);
        }
        // MinusculeMatcher.nameContainsAllMeaningfulCharsInOrder: cheaply
        // reject impossible candidates before allocating the recursive memo.
        let mut remaining = name.as_slice();
        for expected in &self.meaningful {
            let Some(index) = remaining
                .iter()
                .position(|c| case_pair_equals(*expected, *c))
            else {
                return None;
            };
            remaining = &remaining[index + 1..];
        }
        self.at(&name, 0, 0, &mut HashSet::new()).map(|mut ranges| {
            ranges.reverse();
            ranges
        })
    }
    /// MinusculeMatcher.calculateHumpedMatchingScore with empty hard separators.
    /// Accepts only ranges obtained from this matcher for this name.
    pub fn matching_degree(
        &self,
        name: &str,
        value_start_case: bool,
        fragments: Option<&[Range<usize>]>,
    ) -> i32 {
        let Some(ranges) = fragments else {
            return i32::MIN;
        };
        let Some(first) = ranges.first() else {
            return 0;
        };
        let n: Vec<u16> = name.encode_utf16().collect();
        let start_match = first.start == 0;
        let valued_start = start_match && value_start_case;
        let mut matching_case = 0;
        let mut pattern_pos: Option<usize> = None;
        let mut skipped_humps = 0;
        let mut next_hump = 0;
        let mut hump_upper = false;
        for (range_index, range) in ranges.iter().enumerate() {
            for i in range.clone() {
                let after_gap = i == range.start && range_index != 0;
                let mut hump_start = false;
                while next_hump <= i {
                    if next_hump == i {
                        hump_start = true;
                    } else if after_gap {
                        skipped_humps += 1;
                    }
                    next_hump = if unicode::digit(n[next_hump] as u32) {
                        next_hump + 1
                    } else {
                        unicode::next_word(&n, next_hump)
                    };
                }
                let c = n[i];
                let from = pattern_pos.map_or(0, |p| p + 1);
                // CharArrayUtil.indexOf uses Kotlin Char.equals for all units,
                // unlike MinusculeMatcherImpl's ASCII-specialized name search.
                pattern_pos = (from..self.pattern.len())
                    .find(|&p| char_equals_ignore_case(self.pattern[p], c));
                let Some(p) = pattern_pos else {
                    break;
                };
                let lower = unicode::lower(self.pattern[p] as u32);
                let upper = unicode::upper(self.pattern[p] as u32);
                if hump_start {
                    hump_upper = c == self.pattern[p] && upper;
                }
                matching_case += if after_gap && hump_start && lower {
                    -10
                } else if c == self.pattern[p] {
                    if upper {
                        50
                    } else if i == 0 && valued_start {
                        150
                    } else if hump_start {
                        1
                    } else {
                        0
                    }
                } else if hump_start || (lower && hump_upper) {
                    -1
                } else {
                    0
                };
            }
        }
        let word_start = first.start == 0
            || (unicode::word_start(&n, first.start) && !unicode::word_start(&n, first.start - 1));
        let final_match = ranges.last().unwrap().end == n.len();
        (if word_start { 1000 } else { 0 }) + matching_case
            - ranges.len() as i32
            - skipped_humps * 10
            + 2
            + i32::from(start_match)
            + i32::from(final_match)
    }
    // Upstream long-prefix mode removes pattern wildcards, while allowing
    // wildcard characters in the candidate to be skipped. It reports consumed
    // candidate length, even if candidate end interrupts the meaningful prefix.
    // Preserve that behavior rather than replacing it with a generic glob.
    fn substring(&self, name: &[u16]) -> Option<Fragments> {
        let match_at = |start: usize| {
            let mut meaningful = 0;
            let mut pos = start;
            while pos < name.len() && meaningful < self.meaningful.len() {
                if case_pair_equals(self.meaningful[meaningful], name[pos]) {
                    meaningful += 1;
                } else if !wild(name[pos]) {
                    return None;
                }
                pos += 1;
            }
            (pos != start).then(|| vec![start..pos])
        };
        if self.pattern.first() == Some(&(b'*' as u16)) {
            (0..=name.len() - self.meaningful.len()).find_map(match_at)
        } else {
            match_at(0)
        }
    }
    fn at(
        &self,
        n: &[u16],
        p: usize,
        start: usize,
        failed: &mut HashSet<(usize, usize)>,
    ) -> Option<Fragments> {
        if failed.contains(&(p, start)) {
            return None;
        }
        let result = if p == self.pattern.len() {
            Some(Vec::new())
        } else if self.is_wild(p) {
            let mut next = p;
            while self.is_wild(next) {
                next += 1;
            }
            if next == self.pattern.len() {
                if self.pattern.last() == Some(&(b' ' as u16))
                    && start != n.len()
                    && !(next >= 2
                        && (unicode::upper(self.pattern[next - 2] as u32)
                            || unicode::digit(self.pattern[next - 2] as u32)))
                {
                    n[start..]
                        .iter()
                        .position(|&c| c == b' ' as u16)
                        .map(|offset| vec![start + offset..start + offset + 1])
                } else {
                    Some(Vec::new())
                }
            } else {
                self.skip(n, next, start, true, failed)
            }
        } else {
            let length = self.max_fragment(n, p, start);
            if length == 0 {
                None
            } else {
                self.inside(n, p, start, length, failed)
            }
        };
        if result.is_none() {
            failed.insert((p, start));
        }
        result
    }
    fn is_wild(&self, p: usize) -> bool {
        self.pattern.get(p).is_some_and(|c| wild(*c))
    }
    fn equals(&self, p: usize, c: u16) -> bool {
        self.pattern[p] == c || case_pair_equals(self.pattern[p], c)
    }
    fn occurrence(&self, n: &[u16], p: usize, start: usize) -> Option<usize> {
        let unrestricted =
            p > 0 && self.pattern[p - 1] == b'*' as u16 || separator(self.pattern[p]);
        if unrestricted {
            return (start..n.len()).find(|&i| occurrence_equals(self.pattern[p], n[i]));
        }
        self.word_occurrence(n, p, start)
    }
    fn word_occurrence(&self, n: &[u16], p: usize, start: usize) -> Option<usize> {
        if self.mixed
            && unicode::lower(self.pattern[p] as u32)
            && !(p > 0 && separator(self.pattern[p - 1]))
        {
            return None;
        }
        (start..n.len()).find(|&i| {
            occurrence_equals(self.pattern[p], n[i])
                && (!unicode::letter_or_digit(self.pattern[p] as u32) || unicode::word_start(n, i))
        })
    }
    fn skip(
        &self,
        n: &[u16],
        p: usize,
        start: usize,
        allow_dots: bool,
        failed: &mut HashSet<(usize, usize)>,
    ) -> Option<Fragments> {
        let mut from = start;
        let mut max_found = 0;
        while let Some(i) = self.occurrence(n, p, from) {
            if !allow_dots && self.blocked_dots(n, p, from, i) {
                return None;
            }
            let valid = !unicode::upper(self.pattern[p] as u32)
                || unicode::upper(n[i] as u32)
                || unicode::word_start(n, i)
                || !self.mixed;
            let length = if valid { self.max_fragment(n, p, i) } else { 0 };
            if length > max_found
                || (i + length == n.len() && self.pattern.last() == Some(&(b' ' as u16)))
            {
                if !self.middle(n, p, i) {
                    max_found = length;
                }
                if let Some(ranges) = self.inside(n, p, i, length, failed) {
                    return Some(ranges);
                }
            }
            from = i + 1;
        }
        None
    }
    fn blocked_dots(&self, n: &[u16], p: usize, start: usize, end: usize) -> bool {
        self.dots
            && self.pattern.get(p.wrapping_sub(1)) != Some(&(b'.' as u16))
            && n[start..end].contains(&(b'.' as u16))
    }
    fn max_fragment(&self, n: &[u16], p: usize, start: usize) -> usize {
        if start >= n.len() || !self.equals(p, n[start]) {
            return 0;
        }
        let mut length = 1;
        while start + length < n.len() && p + length < self.pattern.len() {
            if !self.equals(p + length, n[start + length]) {
                if unicode::digit(self.pattern[p + length] as u32)
                    && unicode::digit(self.pattern[p + length - 1] as u32)
                    && unicode::digit(n[start + length] as u32)
                {
                    return 0;
                }
                break;
            }
            length += 1;
        }
        length
    }
    fn middle(&self, n: &[u16], p: usize, start: usize) -> bool {
        p > 0
            && self.pattern[p - 1] == b'*' as u16
            && !self.is_wild(p + 1)
            && unicode::letter_or_digit(n[start] as u32)
            && !unicode::word_start(n, start)
    }
    fn inside(
        &self,
        n: &[u16],
        p: usize,
        start: usize,
        length: usize,
        failed: &mut HashSet<(usize, usize)>,
    ) -> Option<Fragments> {
        let minimum = if self.middle(n, p, start) { 3 } else { 1 };
        // Prefer a later uppercase hump before accepting a contiguous prefix.
        for offset in minimum..length {
            if unicode::upper(self.pattern[p + offset] as u32)
                && self.pattern[p + offset] != n[start + offset]
            {
                if let Some(next) = self.word_occurrence(n, p + offset, start + offset) {
                    if let Some(ranges) = self.at(n, p + offset, next, failed) {
                        return Some(append(ranges, start, offset));
                    }
                }
            }
        }
        if p + length >= self.pattern.len() {
            return Some(vec![start..start + length]);
        }
        for length in (1..=length).rev() {
            if length < minimum && !self.is_wild(p + length) {
                break;
            }
            let ranges = if self.is_wild(p + length) {
                self.at(n, p + length, start + length, failed)
            } else if let Some(next) = self.occurrence(n, p + length, start + length + 1) {
                if self.blocked_dots(n, p + length, start + length, next) {
                    continue;
                }
                self.skip(n, p + length, next, false, failed)
            } else {
                None
            };
            if let Some(ranges) = ranges {
                return Some(append(ranges, start, length));
            }
        }
        None
    }
}
// Build backwards during recursion; adjacent ranges merge before one final reversal.
fn append(mut ranges: Fragments, start: usize, length: usize) -> Fragments {
    if let Some(last) = ranges.last_mut().filter(|r| r.start == start + length) {
        last.start = start;
    } else {
        ranges.push(start..start + length);
    }
    ranges
}
fn wild(c: u16) -> bool {
    c == b' ' as u16 || c == b'*' as u16
}
fn separator(c: u16) -> bool {
    unicode::whitespace(c as u32)
        || [b'_', b'-', b':', b'+', b'.']
            .iter()
            .any(|&s| c == s as u16)
}
// The precheck and fragments use the pattern's two simple case forms.
fn case_pair_equals(pattern: u16, candidate: u16) -> bool {
    let (lower, upper) = unicode::simple_case(pattern);
    candidate == lower || candidate == upper
}
// indexOf has an ASCII fast path; non-ASCII uses Kotlin Char.equals.
fn occurrence_equals(pattern: u16, candidate: u16) -> bool {
    if pattern < 128 {
        return case_pair_equals(pattern, candidate);
    }
    char_equals_ignore_case(pattern, candidate)
}
fn char_equals_ignore_case(pattern: u16, candidate: u16) -> bool {
    if pattern == candidate {
        return true;
    }
    let (_, a) = unicode::simple_case(pattern);
    let (_, b) = unicode::simple_case(candidate);
    a == b || unicode::simple_case(a).0 == unicode::simple_case(b).0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_ascii_boolean_corpus_remains_valid() {
        for line in include_str!("../tests/fixtures/minuscule-ascii.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let mut fields = line.split('\t');
            let expected = fields.next().unwrap() == "1";
            let pattern = fields.next().unwrap();
            let name = fields.next().unwrap();
            assert_eq!(
                Utf16Matcher::new(pattern).matches(name),
                expected,
                "{pattern:?}, {name:?}"
            );
        }
    }
    #[test]
    fn upstream_fragment_order_coalescing_and_capital_preference() {
        // NameUtilMatchingTest: testMatchingFragments, testMatchingFragmentsSorted,
        // testPreferCapsMatching, testCamelHumpWinsOverConsecutiveCaseMismatch.
        for (pattern, name, ranges) in [
            (
                "ncldfou*ion",
                "NoClassDefFoundException",
                vec![0..1, 2..4, 7..8, 10..13, 21..24],
            ),
            (
                "d*st",
                "doGet(HttpServletRequest, HttpServletResponse):void",
                vec![0..1, 22..24],
            ),
            (
                "doge*st",
                "doGet(HttpServletRequest, HttpServletResponse):void",
                vec![0..4, 22..24],
            ),
            ("_", "_test", vec![0..1]),
            ("_t", "_test", vec![0..2]),
            (
                "SWU*H*R",
                "SWUPGRADEHDLRFSPR7TEST",
                vec![0..3, 9..10, 12..13],
            ),
            ("getCU", "getCurrentUser", vec![0..4, 10..11]),
            ("GEN", "GetExtendedName", vec![0..1, 3..4, 11..12]),
        ] {
            assert_eq!(
                Utf16Matcher::new(pattern).find(name),
                Some(ranges),
                "{pattern}, {name}"
            );
        }
    }
    #[test]
    fn empty_trailing_space_and_unicode_fragment_offsets() {
        assert_eq!(Utf16Matcher::new("").find("main"), Some(vec![]));
        assert_eq!(Utf16Matcher::new("*").find("main"), Some(vec![]));
        assert_eq!(Utf16Matcher::new("missing").find("main"), None);
        assert_eq!(Utf16Matcher::new("foo ").find("foo tail"), Some(vec![0..4]));
        assert_eq!(
            Utf16Matcher::new("foo ").find("fooBar tail"),
            Some(vec![0..3, 6..7])
        );
        assert_eq!(Utf16Matcher::new("foo ").find("fooBar"), None);
        assert_eq!(
            Utf16Matcher::new("*中修").find("😀中文修复"),
            Some(vec![2..3, 4..5])
        );
        assert_eq!(
            Utf16Matcher::new("*😀修").find("feature/😀修复"),
            Some(vec![8..11])
        );
        let long = format!("*{}修", "😀".repeat(50));
        assert_eq!(
            Utf16Matcher::new(&long).find(&format!("前/{long}").replace('*', "")),
            Some(vec![2..103])
        );
    }
    #[test]
    fn upstream_ignore_case_score_preferences() {
        for line in include_str!("../tests/fixtures/minuscule-score.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let fields: Vec<_> = line.split('\t').collect();
            let matcher = Utf16Matcher::new(fields[1]);
            let score = |name| matcher.matching_degree(name, false, matcher.find(name).as_deref());
            let a = score(fields[2]);
            let b = score(fields[3]);
            assert!(
                if fields[0] == "=" { a == b } else { a < b },
                "{line}: {a}, {b}"
            );
        }
        let matcher = Utf16Matcher::new("*pim");
        let score = |name| matcher.matching_degree(name, true, matcher.find(name).as_deref());
        assert!(score("PImageDecoder") < score("posIdMap"));
        assert_eq!(matcher.matching_degree("no", false, None), i32::MIN);
        assert_eq!(
            Utf16Matcher::new("").matching_degree("name", false, Some(&[])),
            0
        );
    }
    #[test]
    fn unicode_boundaries_and_simple_case() {
        for (pattern, name) in [
            ("*中修", "feature/中文修复"),
            ("*漢ア", "漢字あアプリ"),
            ("*😀修", "feature/😀修复"),
            ("*É", "feature/école"),
            ("*ς", "feature/Σ"),
            ("*İ", "feature/i"),
            ("*٣٤", "version٣٤"),
            ("*修\u{a0}复", "修理\u{a0}复查"),
        ] {
            assert!(
                Utf16Matcher::new(pattern).matches(name),
                "{pattern:?}, {name:?}"
            );
        }
        for (pattern, name) in [
            ("*中缺", "feature/中文修复"),
            ("*あう", "あいう"),
            ("*٣٥", "version٣٤٥"),
            ("*ß", "SS"),
            ("*σ", "ς"),
            ("*é", "e\u{301}"),
            // JVM Char matching does not case-fold surrogate pairs.
            ("*\u{10400}", "\u{10428}"),
        ] {
            assert!(
                !Utf16Matcher::new(pattern).matches(name),
                "{pattern:?}, {name:?}"
            );
        }
    }
    #[test]
    fn threshold_counts_utf16_units_not_scalars_or_bytes() {
        let short = format!("{}修", "😀".repeat(49));
        let long = format!("{}修", "😀".repeat(50));
        assert!(Utf16Matcher::new(&short).matches(&format!("{}别的修", "😀".repeat(49))));
        assert!(!Utf16Matcher::new(&long).matches(&format!("{}别的修", "😀".repeat(50))));
        assert!(Utf16Matcher::new(&long).matches(&long));
        assert!(Utf16Matcher::new(&format!("*{long}")).matches(&format!("前缀/{long}")));
        assert!(!Utf16Matcher::new(&"*".repeat(101)).matches("中文"));
    }
}
