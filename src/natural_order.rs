// Adapted from JetBrains NaturalComparator.java (Apache-2.0).
// Copyright 2000-2022 JetBrains s.r.o. and contributors.
//! Natural reference-name order: ASCII digit runs, UTF-16 character order,
//! case-insensitive primary comparison with a case-sensitive tie break.
use std::cmp::Ordering;

pub fn compare(a: &str, b: &str) -> Ordering {
    if a.is_ascii() && b.is_ascii() {
        return compare_units(a.as_bytes(), b.as_bytes(), true).then_with(|| compare_units(a.as_bytes(), b.as_bytes(), false));
    }
    let a: Vec<_> = a.encode_utf16().collect();
    let b: Vec<_> = b.encode_utf16().collect();
    compare_units(&a, &b, true).then_with(|| compare_units(&a, &b, false))
}
fn compare_units<T: Copy + Ord + Into<u16>>(a: &[T], b: &[T], ignore_case: bool) -> Ordering {
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let (x, y) = (a[i].into(), b[j].into());
        if (digit(x) || x == 32) && (digit(y) || y == 32) {
            let start = |s: &[T], mut n: usize| {
                while n < s.len() && s[n].into() == 32 { n += 1; }
                while n < s.len() && s[n].into() == 48 { n += 1; }
                n
            };
            let end = |s: &[T], mut n: usize| {
                while n < s.len() && digit(s[n].into()) { n += 1; }
                n
            };
            let (sa, sb) = (start(a, i), start(b, j));
            let (ea, eb) = (end(a, sa), end(b, sb));
            let order = (ea - sa).cmp(&(eb - sb))
                .then_with(|| a[sa..ea].cmp(&b[sb..eb]))
                .then_with(|| (ea - i).cmp(&(eb - j)))
                .then_with(|| a[i..sa].cmp(&b[j..sb]));
            if order != Ordering::Equal { return order; }
            i = ea; j = eb;
        } else {
            let order = if x == 32 && y > 32 && y < 48 { Ordering::Greater }
                else if y == 32 && x > 32 && x < 48 { Ordering::Less }
                else if x == y || !ignore_case { x.cmp(&y) }
                else {
                    let (u, v) = (upper(x), upper(y));
                    if u == v { Ordering::Equal } else { lower(u).cmp(&lower(v)) }
                };
            if order != Ordering::Equal { return order; }
            i += 1; j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j)).then_with(|| a.len().cmp(&b.len()))
}
fn digit(c: u16) -> bool { (48..=57).contains(&c) }
// Keep supplementary UTF-16 units unchanged, as Java char case conversion does.
// Rust/JDK Unicode table versions can differ; no multi-character expansion.
fn upper(c: u16) -> u16 {
    let Some(ch) = char::from_u32(c as u32) else { return c; };
    let mut mapped = ch.to_uppercase();
    let first = mapped.next().unwrap();
    if mapped.next().is_none() && first.len_utf16() == 1 { first as u16 } else { c }
}
fn lower(c: u16) -> u16 {
    if c == 0x130 { return b'i' as u16; }
    char::from_u32(c as u32).and_then(|ch| ch.to_lowercase().next())
        .filter(|ch| ch.len_utf16() == 1).map_or(c, |ch| ch as u16)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_runs_and_case_ties() {
        let mut names = ["feature/10", "feature/02", "feature/2", "Feature/2", "feature/1", "feature/0002"];
        names.sort_by(|a,b| compare(a,b));
        assert_eq!(names, ["feature/1", "Feature/2", "feature/2", "feature/02", "feature/0002", "feature/10"]);
        assert_eq!(compare("x99999999999999999999", "x100000000000000000000"), Ordering::Less);
        assert_eq!(compare("x0", "x00"), Ordering::Less);
        assert_eq!(compare("x2", "x2a"), Ordering::Less);
    }
    #[test]
    fn punctuation_unicode_and_comparator_laws() {
        let names = ["", "a", "A", "a#", "a ", "a0", "a00", "a2", "a10", "é2", "É10", "功能/2", "功能/10", "😀", "\u{e000}"];
        assert_eq!(compare("😀", "\u{e000}"), Ordering::Less);
        for a in names { for b in names {
            assert_eq!(compare(a,b), compare(b,a).reverse());
            for c in names {
                if compare(a,b).is_le() && compare(b,c).is_le() { assert!(compare(a,c).is_le(), "{a:?} {b:?} {c:?}"); }
            }
        }}
    }
}
