//! Compact Character tables from the explicitly recorded reference runtime.
//! Word boundaries translated from Rebased 4aec352 NameUtilCore / CodePoint
//! (JetBrains and contributors, Apache-2.0). See assets/search/UNICODE-SOURCE.txt.
const PROPERTIES: &[u8] = include_bytes!("../assets/search/unicode-properties.bin");
const CASES: &[u8] = include_bytes!("../assets/search/unicode-char-case.bin");
fn u16_at(data: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([data[i], data[i + 1]])
}
fn u32_at(data: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(data[i..i + 4].try_into().unwrap())
}
pub fn flags(cp: u32) -> u8 {
    if cp > 0x10ffff {
        return 0;
    }
    let mut lo = 0;
    let mut hi = PROPERTIES.len() / 9;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let at = mid * 9;
        if cp < u32_at(PROPERTIES, at) {
            hi = mid;
        } else if cp >= u32_at(PROPERTIES, at + 4) {
            lo = mid + 1;
        } else {
            return PROPERTIES[at + 8];
        }
    }
    0
}
pub fn upper(cp: u32) -> bool {
    flags(cp) & 1 != 0
}
pub fn lower(cp: u32) -> bool {
    flags(cp) & 2 != 0
}
pub fn digit(cp: u32) -> bool {
    flags(cp) & 4 != 0
}
pub fn letter(cp: u32) -> bool {
    flags(cp) & 8 != 0
}
pub fn letter_or_digit(cp: u32) -> bool {
    flags(cp) & 12 != 0
}
pub fn whitespace(cp: u32) -> bool {
    flags(cp) & 32 != 0
}
/// Java/Kotlin Char.isLetter works on UTF-16 units, not supplementary scalars.
pub fn char_letter(c: char) -> bool {
    (c as u32) <= 0xffff && letter(c as u32)
}
pub fn simple_case(c: u16) -> (u16, u16) {
    let mut lo = 0;
    let mut hi = CASES.len() / 6;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let at = mid * 6;
        let found = u16_at(CASES, at);
        match c.cmp(&found) {
            std::cmp::Ordering::Less => hi = mid,
            std::cmp::Ordering::Greater => lo = mid + 1,
            std::cmp::Ordering::Equal => return (u16_at(CASES, at + 2), u16_at(CASES, at + 4)),
        }
    }
    (c, c)
}
fn pair(a: u16, b: u16) -> Option<u32> {
    ((0xd800..=0xdbff).contains(&a) && (0xdc00..=0xdfff).contains(&b))
        .then(|| 0x10000 + ((a as u32 - 0xd800) << 10) + (b as u32 - 0xdc00))
}
pub fn code_point_at(text: &[u16], i: usize) -> Option<u32> {
    let &c = text.get(i)?;
    Some(
        text.get(i + 1)
            .and_then(|&next| pair(c, next))
            .unwrap_or(c as u32),
    )
}
fn before(text: &[u16], i: usize) -> Option<u32> {
    let &c = text.get(i.checked_sub(1)?)?;
    Some(
        i.checked_sub(2)
            .and_then(|p| pair(text[p], c))
            .unwrap_or(c as u32),
    )
}
pub fn word_start(text: &[u16], i: usize) -> bool {
    let Some(cur) = code_point_at(text, i) else {
        return false;
    };
    let prev = before(text, i);
    if upper(cur) {
        if prev.is_some_and(upper) {
            return code_point_at(text, i + if cur >= 0x10000 { 2 } else { 1 }).is_some_and(lower);
        }
        return true;
    }
    if digit(cur) {
        return true;
    }
    if !letter(cur) {
        return false;
    }
    if flags(cur) & 16 != 0 {
        return true;
    }
    i == 0 || !letter_or_digit(text[i - 1] as u32) || prev.is_some_and(|prev| kana_break(cur, prev))
}
/// NameUtilCore.nextWord, with UTF-16 offsets and codepoint classification.
pub fn next_word(text: &[u16], start: usize) -> usize {
    let Some(ch) = code_point_at(text, start) else {
        return text.len();
    };
    let width = |cp| if cp >= 0x10000 { 2 } else { 1 };
    let ch_len = width(ch);
    if !letter_or_digit(ch) {
        return start + ch_len;
    }
    let mut i = start;
    while let Some(cp) = code_point_at(text, i) {
        if !digit(cp) {
            break;
        }
        i += width(cp);
    }
    if i > start {
        return i;
    }
    while let Some(cp) = code_point_at(text, i) {
        if !upper(cp) {
            break;
        }
        i += width(cp);
    }
    if i > start + ch_len {
        if i == text.len() || !code_point_at(text, i).is_some_and(letter) {
            return i;
        }
        return i - width(before(text, i).unwrap());
    }
    if i == start {
        i += ch_len;
    }
    while let Some(cp) = code_point_at(text, i) {
        if !letter(cp) || word_start(text, i) {
            break;
        }
        i += width(cp);
    }
    i
}
fn kana_break(cur: u32, prev: u32) -> bool {
    let maybe = |cp| (0x3040..=0x3358).contains(&cp) || (0xff66..=0xff9d).contains(&cp);
    if !maybe(cur) && !maybe(prev) {
        return false;
    }
    let a = flags(cur) >> 6;
    let b = flags(prev) >> 6;
    a != b && (matches!(a, 1 | 2) || matches!(b, 1 | 2)) && a != 0 && b != 0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simple_case_and_char_properties_do_not_use_full_unicode_folding() {
        assert_eq!(simple_case('ß' as u16), ('ß' as u16, 'ß' as u16));
        assert_eq!(simple_case('İ' as u16), ('i' as u16, 'İ' as u16));
        assert_eq!(simple_case('ς' as u16), ('ς' as u16, 'Σ' as u16));
        assert!(!char_letter('\u{345}'));
        assert!(digit('٣' as u32));
        assert!(whitespace('\u{a0}' as u32));
        assert!(upper(0x10400));
        assert!(!char_letter('\u{10400}'));
        assert_eq!(simple_case(0xd801), (0xd801, 0xd801));
    }
    #[test]
    fn boundaries_follow_codepoints_but_offsets_remain_utf16() {
        let text: Vec<_> = "😀中文/漢あア".encode_utf16().collect();
        assert_eq!(code_point_at(&text, 0), Some(0x1f600));
        assert!(word_start(&text, 2));
        assert!(word_start(&text, 3));
        assert!(word_start(&text, 5));
        assert!(word_start(&text, 6));
        assert!(word_start(&text, 7));
        let text: Vec<_> = "XMLParser".encode_utf16().collect();
        assert!(word_start(&text, 0));
        assert!(!word_start(&text, 1));
        assert!(!word_start(&text, 2));
        assert!(word_start(&text, 3));
        assert!(!word_start(&text, text.len()));
    }
    #[test]
    fn generated_tables_are_sorted_contiguous_and_bounded() {
        assert_eq!(PROPERTIES.len() % 9, 0);
        let mut end = 0;
        for row in PROPERTIES.chunks_exact(9) {
            assert_eq!(u32_at(row, 0), end);
            end = u32_at(row, 4);
            assert!(end > u32_at(row, 0));
        }
        assert_eq!(end, 0x110000);
        assert_eq!(CASES.len() % 6, 0);
        let mut previous = None;
        for row in CASES.chunks_exact(6) {
            let cp = u16_at(row, 0);
            assert!(previous.is_none_or(|p| cp > p));
            previous = Some(cp);
        }
    }
}
