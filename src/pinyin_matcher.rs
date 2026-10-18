//! Rebased 4aec352 PinyinMatcher, JetBrains and contributors, Apache-2.0.
//! A fallback matcher, not a replacement for ordinary reference-name matching.
use std::ops::Range;
const BASE: u32 = 0x3400;
const INDICES: &[u8] = include_bytes!("../assets/search/pinyin-indices.bin");
const MASKS: &[u8] = include_bytes!("../assets/search/pinyin-masks.bin");

pub struct PinyinMatcher<'a> { initials: &'a [u8] }
impl<'a> PinyinMatcher<'a> {
    /// Upstream strips exactly one leading '*', and enables only lowercase
    /// initials excluding i/u/v. It does not normalize case or full pinyin.
    pub fn new(pattern: &'a str) -> Option<Self> {
        let pattern = pattern.strip_prefix('*').unwrap_or(pattern);
        (!pattern.is_empty() && pattern.bytes().all(|c| c.is_ascii_lowercase() && !matches!(c,b'i'|b'u'|b'v')))
            .then_some(Self { initials: pattern.as_bytes() })
    }
    /// The returned range uses UTF-16 offsets, matching the upstream API.
    /// Scan without building a romanized string or allocating per-name tables.
    pub fn find(&self, name: &str) -> Option<Range<usize>> {
        let mut utf16 = 0;
        for (byte, first) in name.char_indices() {
            if mask(first) & bit(self.initials[0]) != 0 {
                let mut chars = name[byte..].chars();
                if self.initials.iter().all(|&initial| chars.next().is_some_and(|c| mask(c) & bit(initial) != 0)) {
                    return Some(utf16..utf16+self.initials.len());
                }
            }
            utf16 += first.len_utf16();
        }
        None
    }
    pub fn matching_degree(&self, name: &str, fragment: Option<&Range<usize>>) -> i32 {
        let Some(fragment) = fragment else { return i32::MIN; };
        let len = fragment.len() as i32;
        if fragment.start == 0 { return 500 + len; }
        if (fragment.start == 3 && (name.starts_with("get") || name.starts_with("set")))
            || (fragment.start == 2 && name.starts_with("is")) { return 200 + len; }
        len
    }
}
fn bit(initial: u8) -> u32 { 1 << (initial - b'a') }
fn mask(c: char) -> u32 {
    let Some(index) = (c as u32).checked_sub(BASE).map(|i| i as usize * 2) else { return 0; };
    let Some(bytes) = INDICES.get(index..index+2) else { return 0; };
    let code = u16::from_le_bytes([bytes[0],bytes[1]]);
    let Some(index) = code.checked_sub(33).map(|i| i as usize * 4) else { return 0; };
    let Some(bytes) = MASKS.get(index..index+4) else { return 0; };
    u32::from_le_bytes(bytes.try_into().unwrap())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_examples_and_polyphonic_initials() {
        let nh = PinyinMatcher::new("*nh").unwrap();
        assert_eq!(nh.find("你好"),Some(0..2));
        assert_eq!(nh.find("get你好"),Some(3..5));
        assert_eq!(PinyinMatcher::new("*gh").unwrap().find("角色"),None);
        assert_eq!(PinyinMatcher::new("*g").unwrap().find("角色"),Some(0..1));
        assert_eq!(PinyinMatcher::new("*j").unwrap().find("角色"),Some(0..1));
    }
    #[test]
    fn enabling_contiguity_utf16_offsets_and_scoring() {
        for pattern in ["", "*", "**nh", "NH", "*Nh", "nihao", "ui", "v", "nh*", "n h", "你好"] {
            assert!(PinyinMatcher::new(pattern).is_none(),"{pattern}");
        }
        let matcher = PinyinMatcher::new("nh").unwrap();
        assert_eq!(matcher.find("你/好"),None);
        assert_eq!(matcher.find("你x好"),None);
        assert_eq!(matcher.find("feature/你好"),Some(8..10));
        assert_eq!(matcher.find("😀你好"),Some(2..4));
        for (name,score) in [("你好",502),("get你好",202),("is你好",202),("set你好",202),("Get你好",2),("feature/你好",2),("nothing",i32::MIN)] {
            assert_eq!(matcher.matching_degree(name,matcher.find(name).as_ref()),score,"{name}");
        }
        assert_eq!(mask('\u{33ff}'),0); assert_eq!(mask('\u{a000}'),0); assert_eq!(mask('𠀀'),0);
    }
    #[test]
    fn generated_table_indices_are_in_range() {
        assert_eq!(INDICES.len(),27648*2); assert_eq!(MASKS.len(),579*4);
        for entry in INDICES.chunks_exact(2) {
            let index = u16::from_le_bytes([entry[0],entry[1]]);
            assert!(index < 33 || (index as usize - 33) < MASKS.len()/4);
        }
    }
}
