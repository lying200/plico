//! Word-level three-way resolution, without Git or editor side effects.
//!
//! Rules and regression cases adapted from Rebased 4aec352 / JetBrains
//! `platform/util/diff/src/com/intellij/diff/comparison/MergeResolveUtil.kt`
//! and `platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/MergeResolveUtilTest.kt`.
//! Copyright 2000-2024 JetBrains s.r.o. and contributors, Apache-2.0.
//! The upstream default uses the simple resolver, NOT its greedy deletion union.

use std::{
    ops::Range,
    time::{Duration, Instant},
};

const MAX_BYTES: usize = 256 * 1024;
const MAX_TOKENS: usize = 32 * 1024;

struct Edit {
    base: Range<usize>,
    side: Range<usize>,
}

/// Failure leaves the entire block unresolved; never return a partial guess.
pub(super) fn resolve(base: &str, yours: &str, theirs: &str) -> Option<String> {
    if [base, yours, theirs].iter().any(|s| s.len() > MAX_BYTES) {
        return None;
    }
    if yours == theirs || theirs == base {
        return Some(yours.to_owned());
    }
    if yours == base {
        return Some(theirs.to_owned());
    }
    let deadline = Instant::now() + Duration::from_millis(50);
    let left = edits(base, yours, deadline)?;
    let right = edits(base, theirs, deadline)?;
    merge(base, yours, theirs, &left, &right, false)
        .or_else(|| merge(base, yours, theirs, &left, &right, true))
}

/// Keep identifiers whole: character diff would incorrectly merge `foo` ->
/// `Foo` and `foo` -> `foO`. Punctuation and continuous-script characters can
/// provide independent anchors, including the dots in version numbers.
fn tokens(text: &str) -> Option<(Vec<&str>, Vec<usize>)> {
    let mut boundaries = Vec::new();
    let mut in_word = false;
    for (offset, ch) in text.char_indices() {
        let code = ch as u32;
        // TrimUtil.isAlpha/isContinuousScript. Alphabetic scripts (including
        // Hangul) remain words; ideographs and continuous scripts are separate.
        let punctuation = ch != '_' && ch.is_ascii_punctuation();
        let ideographic = matches!(code,
            0x3006..=0x3007 | 0x3021..=0x3029 | 0x3038..=0x303a |
            0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff);
        let continuous = code >= 128
            && !ch.is_numeric()
            && (code > 0xffff
                || ideographic
                || !ch.is_alphabetic()
                || matches!(code, 0x0e00..=0x0e7f | 0x3040..=0x30ff | 0xa980..=0xa9df));
        let word = !whitespace(ch) && !punctuation && !continuous;
        if !word || !in_word {
            boundaries.push(offset);
            if boundaries.len() > MAX_TOKENS {
                return None;
            }
        }
        in_word = word;
    }
    boundaries.push(text.len());
    let tokens = boundaries.windows(2).map(|r| &text[r[0]..r[1]]).collect();
    Some((tokens, boundaries))
}

fn edits(base: &str, side: &str, deadline: Instant) -> Option<Vec<Edit>> {
    let (a, a_offsets) = tokens(base)?;
    let (b, b_offsets) = tokens(side)?;
    fn anchors<'a>(tokens: Vec<&'a str>, offsets: &[usize]) -> Vec<(&'a str, Range<usize>)> {
        tokens
            .into_iter()
            .enumerate()
            .filter(|(_, token)| !token.chars().all(|c| matches!(c, ' ' | '\t')))
            .map(|(i, token)| (token, offsets[i]..offsets[i + 1]))
            .collect::<Vec<_>>()
    }
    let a = anchors(a, &a_offsets);
    let b = anchors(b, &b_offsets);
    let a_tokens: Vec<_> = a.iter().map(|t| t.0).collect();
    let b_tokens: Vec<_> = b.iter().map(|t| t.0).collect();
    let ops = matching_words(&a_tokens, &b_tokens, deadline)?;
    // Never make a resolution decision using a timed-out coarse diff.
    if Instant::now() >= deadline {
        return None;
    }
    let mut result = Vec::new();
    let (mut old, mut new) = (0, 0);
    let matches = optimize_matches(&a, &b, base, side, &ops, deadline)?;
    for (x, y) in matches {
        for (i, j) in x.zip(y) {
            let (x, y) = (&a[i].1, &b[j].1);
            if old != x.start || new != y.start {
                result.push(Edit {
                    base: old..x.start,
                    side: new..y.start,
                });
            }
            old = x.end;
            new = y.end;
        }
    }
    if old != base.len() || new != side.len() {
        result.push(Edit {
            base: old..base.len(),
            side: new..side.len(),
        });
    }
    Some(result)
}

// Bounded LCS table: at most 1 MiB, disposed before resolution. Work and memory
// limits are deterministic; expiration never supplies an approximate match.
fn matching_words(a: &[&str], b: &[&str], deadline: Instant) -> Option<Vec<similar::DiffOp>> {
    let width = b.len() + 1;
    let cells = (a.len() + 1).checked_mul(width)?;
    if cells > 512 * 1024 {
        return None;
    }
    let mut lengths = vec![0u16; cells];
    for i in (0..a.len()).rev() {
        if Instant::now() >= deadline {
            return None;
        }
        for j in (0..b.len()).rev() {
            lengths[i * width + j] = if a[i] == b[j] {
                lengths[(i + 1) * width + j + 1] + 1
            } else {
                lengths[(i + 1) * width + j].max(lengths[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut matches = Vec::new();
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            matches.push(similar::DiffOp::Equal {
                old_index: i,
                new_index: j,
                len: 1,
            });
            i += 1;
            j += 1;
        } else if lengths[(i + 1) * width + j] >= lengths[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    Some(matches)
}

// ChunkOptimizer.WordChunkOptimizer: coalesce adjacent matching chunks and
// place ambiguous matches at whitespace boundaries, rather than choosing a
// character alignment that splices an identifier or moves a newline.
fn optimize_matches(
    a: &[(&str, Range<usize>)],
    b: &[(&str, Range<usize>)],
    base: &str,
    side: &str,
    ops: &[similar::DiffOp],
    deadline: Instant,
) -> Option<Vec<(Range<usize>, Range<usize>)>> {
    let mut ranges: Vec<(Range<usize>, Range<usize>)> = Vec::new();
    let separated = |words: &[(&str, Range<usize>)], text: &str, index: usize| {
        words[index - 1].0 == "\n"
            || words[index].0 == "\n"
            || text[words[index - 1].1.end..words[index].1.start]
                .chars()
                .any(whitespace)
    };
    for op in ops.iter().filter(|op| op.tag() == similar::DiffTag::Equal) {
        ranges.push((op.old_range(), op.new_range()));
        loop {
            if Instant::now() >= deadline {
                return None;
            }
            if ranges.len() < 2 {
                break;
            }
            let (x, y) = ranges[ranges.len() - 2].clone();
            let (u, v) = ranges[ranges.len() - 1].clone();
            if x.end != u.start && y.end != v.start {
                break;
            }
            let forward = (0..u.len())
                .take_while(|i| a[x.end + i].0 == b[y.end + i].0)
                .count();
            let backward = (0..x.len())
                .take_while(|i| a[u.start - i - 1].0 == b[v.start - i - 1].0)
                .count();
            if forward == u.len() {
                ranges.truncate(ranges.len() - 2);
                ranges.push((x.start..x.end + u.len(), y.start..y.end + v.len()));
                continue;
            }
            if backward == x.len() {
                ranges.truncate(ranges.len() - 2);
                ranges.push((u.start - x.len()..u.end, v.start - y.len()..v.end));
                continue;
            }
            let (words, text, start) = if x.end == u.start {
                (a, base, u.start)
            } else {
                (b, side, v.start)
            };
            if separated(words, text, start) {
                break;
            }
            let shift = (0..forward)
                .find(|i| separated(words, text, start + i + 1))
                .map(|i| (i + 1) as isize)
                .or_else(|| {
                    (0..backward)
                        .find(|i| separated(words, text, start - i - 1))
                        .map(|i| -((i + 1) as isize))
                })
                .unwrap_or(0);
            if shift != 0 {
                ranges.truncate(ranges.len() - 2);
                ranges.push((
                    x.start..x.end.checked_add_signed(shift)?,
                    y.start..y.end.checked_add_signed(shift)?,
                ));
                ranges.push((
                    u.start.checked_add_signed(shift)?..u.end,
                    v.start.checked_add_signed(shift)?..v.end,
                ));
            }
            break;
        }
    }
    Some(ranges)
}

fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n')
}
fn equal(a: &str, b: &str, ignore_whitespace: bool) -> bool {
    a == b
        || (ignore_whitespace
            && a.chars()
                .filter(|c| !whitespace(*c))
                .eq(b.chars().filter(|c| !whitespace(*c))))
}

fn materialize(base: &str, side: &str, range: Range<usize>, edits: &[Edit]) -> String {
    let mut out = String::new();
    let mut offset = range.start;
    for edit in edits {
        out.push_str(&base[offset..edit.base.start]);
        out.push_str(&side[edit.side.clone()]);
        offset = edit.base.end;
    }
    out.push_str(&base[offset..range.end]);
    out
}

fn merge(
    base: &str,
    yours: &str,
    theirs: &str,
    left: &[Edit],
    right: &[Edit],
    ignore_whitespace: bool,
) -> Option<String> {
    let (mut l, mut r, mut offset) = (0, 0, 0);
    let mut out = String::new();
    while l < left.len() || r < right.len() {
        let start = left
            .get(l)
            .map_or(usize::MAX, |e| e.base.start)
            .min(right.get(r).map_or(usize::MAX, |e| e.base.start));
        out.push_str(&base[offset..start]);
        let (begin_l, begin_r) = (l, r);
        let mut end = start;
        loop {
            if let Some(e) = left.get(l).filter(|e| e.base.start <= end) {
                end = end.max(e.base.end);
                l += 1;
            } else if let Some(e) = right.get(r).filter(|e| e.base.start <= end) {
                end = end.max(e.base.end);
                r += 1;
            } else {
                break;
            }
        }
        let a = materialize(base, yours, start..end, &left[begin_l..l]);
        let b = materialize(base, theirs, start..end, &right[begin_r..r]);
        let original = &base[start..end];
        append_range(&mut out, original, &a, &b, ignore_whitespace)?;
        offset = end;
    }
    out.push_str(&base[offset..]);
    Some(out)
}

// The default comparison trims only whitespace common to all three inputs.
// The fallback trims each input, preserving the excluded whitespace as ordinary
// (exact-comparison) surrounding content, like SimpleHelper.appendBase.
fn append_range(out: &mut String, base: &str, a: &str, b: &str, ignore: bool) -> Option<()> {
    if !ignore {
        out.push_str(select(base, a, b, false)?);
        return Some(());
    }
    if equal(base, a, true) && equal(base, b, true) {
        out.push_str(select_surrounding(base, a, b));
        return Some(());
    }
    let range = |s: &str| {
        let trimmed = s.trim_matches(whitespace);
        let start = s.len() - s.trim_start_matches(whitespace).len();
        start..start + trimmed.len()
    };
    let (br, ar, cr) = (range(base), range(a), range(b));
    out.push_str(select_surrounding(
        &base[..br.start],
        &a[..ar.start],
        &b[..cr.start],
    ));
    out.push_str(select(
        &base[br.clone()],
        &a[ar.clone()],
        &b[cr.clone()],
        true,
    )?);
    out.push_str(select_surrounding(
        &base[br.end..],
        &a[ar.end..],
        &b[cr.end..],
    ));
    Some(())
}
fn select<'a>(base: &'a str, a: &'a str, b: &'a str, ignore: bool) -> Option<&'a str> {
    if equal(a, b, ignore) || equal(b, base, ignore) {
        Some(a)
    } else if equal(a, base, ignore) {
        Some(b)
    } else {
        None
    }
}
fn select_surrounding<'a>(base: &'a str, a: &'a str, b: &'a str) -> &'a str {
    if a != base {
        a
    } else if b != base {
        b
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebased_simple_resolver_cases() {
        // Upstream testSimple, testSameModification, testNonConflictingChanges,
        // testFailure, and IDEA-200696. Run both side orders as well.
        for (base, left, right, expected) in [
            ("", "", "", Some("")),
            ("x x x", "x x x", "x x x", Some("x x x")),
            ("x x x", "x Y x", "x x x", Some("x Y x")),
            ("x x", "x x", "x Y x", Some("x Y x")),
            ("x X x", "x x", "x X x", Some("x x")),
            ("x x x", "x Y x", "x Y x", Some("x Y x")),
            ("x x", "x Y x", "x Y x", Some("x Y x")),
            ("x X x", "x x", "x x", Some("x x")),
            ("x x x", "x Y x x", "x x Z x", Some("x Y x Z x")),
            ("x", "x Y", "Z x", Some("Z x Y")),
            ("x x", "x", "Z x x", Some("Z x")),
            ("x x x", "x Y x", "x Z x", None),
            ("x x", "x Y x", "x Z x", None),
            (
                "version: 1.0.0",
                "version: 2.0.0",
                "version: 1.0.4",
                Some("version: 2.0.4"),
            ),
            (
                "alpha beta gamma\n",
                "ALPHA beta gamma\n",
                "alpha beta GAMMA\n",
                Some("ALPHA beta GAMMA\n"),
            ),
        ] {
            for (a, b) in [(left, right), (right, left)] {
                assert_eq!(
                    resolve(base, a, b).as_deref(),
                    expected,
                    "{base:?} / {a:?} / {b:?}"
                );
            }
        }
    }

    #[test]
    fn rebased_whitespace_regression() {
        // Whitespace-only fallback follows the left-side preference upstream;
        // unlike independent content edits, it is not side-order symmetric.
        assert_eq!(resolve("i\n", "i", "\ni").as_deref(), Some("i"));
        assert_eq!(
            resolve("Y X Y", "Y C\nX\nC Y", "Y \nX\n Y").as_deref(),
            Some("Y \nC\nX\nC Y")
        );
    }

    #[test]
    fn rebased_indentation_regression() {
        assert_eq!(
            resolve(
                r#"
  public static class ChangeData {
    @NotNull public final ChangeKind kind;
    public final int otherPath;

    public ChangeData(@NotNull ChangeKind kind, int otherPath) {
      this.kind = kind;
      this.otherPath = otherPath;
    }

    public boolean isRename() {
      return kind.equals(ChangeKind.RENAMED_FROM) || kind.equals(ChangeKind.RENAMED_TO);
    }

    @Override
    public int hashCode() {
      return Objects.hash(kind, otherPath);
    }
  }

  private enum ChangeKind {
    MODIFIED((byte)0),
    RENAMED_FROM((byte)1),
    RENAMED_TO((byte)2);"#,
                r#"
  public enum ChangeKind {
    NOT_CHANGED((byte)-1),
    MODIFIED((byte)0),
    ADDED((byte)1),
    REMOVED((byte)2);"#,
                r#"
  public static class ChangeData {
  @NotNull public final ChangeKind kind;
  public final int otherPath;

  public ChangeData(@NotNull ChangeKind kind, int otherPath) {
    this.kind = kind;
    this.otherPath = otherPath;
  }

  public boolean isRename() {
    return kind.equals(ChangeKind.RENAMED_FROM) || kind.equals(ChangeKind.RENAMED_TO);
  }

  @Override
  public int hashCode() {
    return Objects.hash(kind, otherPath);
  }
}

private enum ChangeKind {
  MODIFIED((byte)0),
  RENAMED_FROM((byte)1),
  RENAMED_TO((byte)2);"#
            )
            .as_deref(),
            Some(
                r#"
  public enum ChangeKind {
    NOT_CHANGED((byte)-1),
  MODIFIED((byte)0),
  ADDED((byte)1),
  REMOVED((byte)2);"#
            )
        );
    }

    #[test]
    fn no_greedy_reordering_or_identifier_splicing() {
        for (base, a, b) in [
            ("x X x", "x x", "x Y x"),
            ("foo", "Foo", "foO"),
            ("", "left", "right"),
        ] {
            assert_eq!(resolve(base, a, b), None);
        }
    }

    #[test]
    fn unicode_and_size_limits() {
        assert!(!equal("a\u{a0}b", "ab", true));
        assert!(!equal("a\rb", "ab", true));
        assert!(equal("a \n\tb", "ab", true));
        assert_eq!(resolve("가나다", "카나다", "가나타"), None);

        assert_eq!(
            resolve("甲乙丙", "丁乙丙", "甲乙戊").as_deref(),
            Some("丁乙戊")
        );
        assert_eq!(
            resolve("a 😀 b", "A 😀 b", "a 😀 B").as_deref(),
            Some("A 😀 B")
        );
        assert!(resolve(&"x".repeat(MAX_BYTES + 1), "y", "z").is_none());
        assert!(resolve(&".".repeat(MAX_TOKENS + 1), "y", "z").is_none());
        let a = vec!["a"; 1024];
        let b = vec!["b"; 1024];
        assert!(matching_words(&a, &b, Instant::now() + Duration::from_secs(1)).is_none());
        assert!(edits("old text", "new text", Instant::now()).is_none());
    }

    #[test]
    fn repeated_anchors_and_unicode_ranges_do_not_panic() {
        // Exercise equal chunks touching on one side, both directions, empty
        // ranges and newline anchors used by the upstream chunk optimizer.
        let samples = [
            "",
            "x",
            "x x",
            "x y x",
            "x\nx",
            "\nx",
            "x\n",
            "中 x 中",
            "x.x",
            "x😀x",
            "\t x ",
        ];
        for base in samples {
            for a in samples {
                for b in samples {
                    let result = resolve(base, a, b);
                    if a == b || b == base {
                        assert_eq!(result.as_deref(), Some(a));
                    } else if a == base {
                        assert_eq!(result.as_deref(), Some(b));
                    }
                }
            }
        }
    }
}
