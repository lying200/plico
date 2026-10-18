//! Rebased 4aec352 KeyboardLayoutUtil / FixingLayoutMatcher.
//! JetBrains and contributors, Apache-2.0. Physical-key observations are supplied
//! by the platform owner; this module does not infer a layout from locale.
use std::collections::HashMap;
#[derive(Default)]
pub struct Layout {
    observed: HashMap<char, char>,
}
impl Layout {
    /// `letter_index` is the physical key's A..Z offset, not the typed character.
    /// First observation wins, as in KeyboardLayoutUtil.storeAsciiForChar.
    pub fn observe(&mut self, letter_index: u8, typed: char) {
        if typed as u32 > 0xffff
            || letter_index >= 26
            || typed.is_ascii_alphabetic()
            || self.observed.contains_key(&typed)
        {
            return;
        }
        let mut mapped = (b'a' + letter_index) as char;
        if crate::search_unicode::upper(typed as u32) {
            mapped = mapped.to_ascii_uppercase();
        }
        self.observed.insert(typed, mapped);
    }
    pub fn convert(&self, typed: char, linux: bool) -> Option<char> {
        if let Some(&mapped) = self.observed.get(&typed) {
            return Some(mapped);
        }
        if !self.observed.is_empty() && !linux {
            return None;
        }
        if typed as u32 > 0xffff {
            return None;
        }
        let lower = char::from_u32(crate::search_unicode::simple_case(typed as u16).0 as u32)?;
        let mapped = russian_key(lower)?;
        Some(if lower == typed {
            mapped
        } else {
            mapped.to_ascii_uppercase()
        })
    }
    /// The caller supplies JVM-compatible Char.isLetter semantics. Keeping this
    /// explicit avoids silently treating Rust Alphabetic (which includes some
    /// combining marks) as the same Unicode property as Java Letter.
    pub fn alternate(
        &self,
        pattern: &str,
        linux: bool,
        is_letter: impl Fn(char) -> bool,
    ) -> Option<String> {
        let mut has_letters = false;
        for c in pattern.chars() {
            if is_letter(c) {
                if c.is_ascii() {
                    return None;
                }
                has_letters = true;
            }
        }
        if !has_letters {
            return None;
        }
        let alternate: String = pattern
            .chars()
            .map(|c| self.convert(c, linux).unwrap_or(c))
            .collect();
        (alternate != pattern).then_some(alternate)
    }
}
fn russian_key(c: char) -> Option<char> {
    const SOURCE: &str = "йцукенгшщзхъфывапролджэячсмитьбю.";
    const TARGET: &[u8] = b"qwertyuiop[]asdfghjkl;'zxcvbnm,./";
    SOURCE
        .chars()
        .position(|key| key == c)
        .map(|i| TARGET[i] as char)
}
#[cfg(test)]
mod tests {
    use super::*;
    // Restricted to the scripts exercised here. Production wiring must use the
    // complete Java Letter property, not this test predicate.
    fn letter(c: char) -> bool {
        c.is_ascii_alphabetic() || matches!(c, 'А'..='я' | 'Ё' | 'ё' | '中' | '文')
    }
    #[test]
    fn fallback_only_when_all_letters_are_non_ascii() {
        let map = Layout::default();
        assert_eq!(
            map.alternate("*ыек", false, letter).as_deref(),
            Some("*str")
        );
        assert_eq!(map.alternate("Пше", false, letter).as_deref(), Some("Git"));
        assert_eq!(
            map.alternate("*ыек.123", false, letter).as_deref(),
            Some("*str/123")
        );
        for pattern in ["main", "ыекA", "Aыек", "123.*", "中文"] {
            assert_eq!(map.alternate(pattern, false, letter), None, "{pattern}");
        }
        let alternate = map.alternate("*ыек", false, letter).unwrap();
        assert_eq!(
            crate::branch_matcher::AsciiMatcher::new(&alternate)
                .unwrap()
                .matches("String"),
            Some(true)
        );
    }
    #[test]
    fn runtime_mapping_wins_and_disables_hardcoded_windows_fallback() {
        let mut map = Layout::default();
        map.observe(0, 'ж');
        map.observe(5, 'ж');
        assert_eq!(map.convert('ж', false), Some('a'));
        assert_eq!(map.convert('ы', false), None);
        assert_eq!(map.convert('ы', true), Some('s'));
        map.observe(6, 'Ж');
        assert_eq!(map.convert('Ж', false), Some('G'));
        map.observe(26, '中');
        assert_eq!(map.convert('中', false), None);
        let mut unchanged = Layout::default();
        unchanged.observe(0, 'q');
        assert_eq!(unchanged.convert('ы', false), Some('s'));
    }
}
