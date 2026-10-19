//! A block replacement in normalized editor text. Offsets are UTF-8 bytes;
//! separators belong to their preceding source line, just like DiffFile.
use std::ops::Range;

pub fn replacement(old: &str, current: &str, a: Range<usize>, b: Range<usize>) -> Option<(Range<usize>, String)> {
    fn offsets(text: &str) -> Vec<usize> {
        let mut offsets = vec![0];
        offsets.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        if offsets.last().copied() != Some(text.len()) { offsets.push(text.len()); }
        offsets
    }
    let old_offsets = offsets(old);
    let current_offsets = offsets(current);
    let source = *old_offsets.get(a.start)?..*old_offsets.get(a.end)?;
    let mut target = *current_offsets.get(b.start)?..*current_offsets.get(b.end)?;
    let mut text = old.get(source)?.to_owned();
    // Inserting after a last line without a separator must start a new line.
    if target.is_empty() && target.start > 0 && !current[..target.start].ends_with('\n') && !text.is_empty() {
        text.insert(0, '\n');
    }
    if text.is_empty() && !target.is_empty() && target.end == current.len()
        && target.start > 0 && !old.ends_with('\n') && current[..target.start].ends_with('\n') {
        target.start -= 1;
    }
    Some((target, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restores_only_selected_block_and_preserves_separators() {
        for (old, current, a, b, expected) in [
            ("a\nold\nz\n", "a\nnew\nz\n", 1..2, 1..2, "a\nold\nz\n"),
            ("a\nz\n", "a\nextra\nz\n", 1..1, 1..2, "a\nz\n"),
            ("a\n中\nz\n", "a\nz\n", 1..2, 1..1, "a\n中\nz\n"),
            ("", "x\n", 0..0, 0..1, ""),
            ("旧", "新", 0..1, 0..1, "旧"),
            ("a\nb", "a", 1..2, 1..1, "a\nb"),
            ("a", "a\nb", 1..1, 1..2, "a"),
            ("a\n", "a\nb\n", 1..1, 1..2, "a\n"),
        ] {
            let (range, text) = replacement(old, current, a, b).unwrap();
            let mut result = current.to_owned(); result.replace_range(range, &text);
            assert_eq!(result, expected);
        }
        assert!(replacement("a", "b", 0..9, 0..1).is_none());
    }
}
