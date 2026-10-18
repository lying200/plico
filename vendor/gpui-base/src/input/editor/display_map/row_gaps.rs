//! Display-only blank rows. The source row space already includes wrapping and
//! folding. No source bytes, line numbers or undo records are changed here.
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Gap {
    before: usize,
    display: Range<usize>,
}

/// Sparse projection: storage scales with gaps, not with file length.
#[derive(Debug, Default)]
pub(crate) struct RowGaps {
    gaps: Vec<Gap>,
    source_rows: usize,
    added_rows: usize,
}

impl RowGaps {
    /// Replace the entire projection. Callers supply sorted (boundary, height)
    /// pairs; the last boundary can equal source_rows (padding at EOF).
    /// Invalid input leaves the old projection intact.
    pub(crate) fn replace(
        &mut self,
        source_rows: usize,
        gaps: impl IntoIterator<Item = (usize, usize)>,
    ) -> Result<(), &'static str> {
        let mut projected: Vec<Gap> = Vec::new();
        let mut added = 0usize;
        let mut previous = 0;
        for (before, height) in gaps {
            if before > source_rows || before < previous {
                return Err("alignment boundaries must be ordered and inside the document");
            }
            previous = before;
            let start = before.checked_add(added).ok_or("alignment height overflow")?;
            added = added.checked_add(height).ok_or("alignment height overflow")?;
            let end = before.checked_add(added).ok_or("alignment height overflow")?;
            if height == 0 { continue; }
            if let Some(last) = projected.last_mut().filter(|last| last.before == before) {
                last.display.end = end;
            } else {
                projected.push(Gap { before, display: start..end });
            }
        }
        source_rows.checked_add(added).ok_or("alignment height overflow")?;
        *self = Self { gaps: projected, source_rows, added_rows: added };
        Ok(())
    }

    pub(crate) fn display_count(&self) -> usize { self.source_rows + self.added_rows }

    /// Accepts EOF as a boundary as well as actual rows.
    pub(crate) fn source_to_display(&self, row: usize) -> Option<usize> {
        if row > self.source_rows { return None; }
        let i = self.gaps.partition_point(|gap| gap.before <= row);
        let added = i.checked_sub(1).map_or(0, |i| self.gaps[i].display.end - self.gaps[i].before);
        Some(row + added)
    }

    /// A blank row has no source row; callers must not paint a line number or
    /// allow it to become an editable newline.
    pub(crate) fn display_to_source(&self, row: usize) -> Option<usize> {
        if row >= self.display_count() { return None; }
        let i = self.gaps.partition_point(|gap| gap.display.start <= row);
        let added = if let Some(i) = i.checked_sub(1) {
            let gap = &self.gaps[i];
            if row < gap.display.end { return None; }
            gap.display.end - gap.before
        } else { 0 };
        Some(row - added)
    }

    /// Hit testing/vertical navigation through padding. Direction matters:
    /// upward movement lands before the gap, downward movement after it.
    /// At a document edge, clamp to the only existing adjacent row.
    pub(crate) fn adjacent_source(&self, row: usize, forward: bool) -> Option<usize> {
        if self.source_rows == 0 { return None; }
        let row = row.min(self.display_count().saturating_sub(1));
        if let Some(source) = self.display_to_source(row) { return Some(source); }
        let i = self.gaps.partition_point(|gap| gap.display.start <= row) - 1;
        let boundary = self.gaps[i].before;
        Some(if forward { boundary.min(self.source_rows - 1) } else { boundary.saturating_sub(1) })
    }
}

#[cfg(test)]
mod tests {
    use super::RowGaps;

    #[test]
    fn padding_has_no_source_and_navigation_skips_it() {
        let mut map = RowGaps::default();
        map.replace(4, [(0, 2), (2, 1), (2, 2), (4, 3)]).unwrap();
        assert_eq!(map.display_count(), 12);
        assert_eq!((0..4).map(|r| map.source_to_display(r).unwrap()).collect::<Vec<_>>(), [2,3,7,8]);
        assert_eq!(map.source_to_display(4), Some(12));
        assert_eq!(map.display_to_source(5), None);
        assert_eq!(map.adjacent_source(5, false), Some(1));
        assert_eq!(map.adjacent_source(5, true), Some(2));
        assert_eq!(map.adjacent_source(0, false), Some(0));
        assert_eq!(map.adjacent_source(11, true), Some(3));
        assert_eq!(map.display_to_source(12), None);
        assert_eq!(map.source_to_display(5), None);
    }

    #[test]
    fn matches_expanded_oracle_without_storing_per_line_entries() {
        for count in 0..40 {
            for seed in 0..15 {
                let gaps: Vec<_> = (0..=count).map(|row| (row, (row * 7 + seed) % 4)).collect();
                let mut oracle = Vec::new();
                for &(row, height) in &gaps {
                    oracle.extend(std::iter::repeat_n(None, height));
                    if row < count { oracle.push(Some(row)); }
                }
                let mut map = RowGaps::default();
                map.replace(count, gaps).unwrap();
                assert_eq!(map.display_count(), oracle.len());
                for (display, &source) in oracle.iter().enumerate() {
                    assert_eq!(map.display_to_source(display), source);
                    if let Some(source) = source {
                        assert_eq!(map.source_to_display(source), Some(display));
                    } else if count > 0 {
                        let previous = oracle[..display].iter().rev().flatten().next().copied().unwrap_or(0);
                        let next = oracle[display..].iter().flatten().next().copied().unwrap_or(count - 1);
                        assert_eq!(map.adjacent_source(display, false), Some(previous));
                        assert_eq!(map.adjacent_source(display, true), Some(next));
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_rebuild_preserves_mapping_and_clear_restores_identity() {
        let mut map = RowGaps::default();
        map.replace(3, [(1, 2)]).unwrap();
        for invalid in [vec![(4,1)], vec![(2,1),(1,1)], vec![(0,usize::MAX)]] {
            assert!(map.replace(3, invalid).is_err());
            assert_eq!(map.source_to_display(1), Some(3));
        }
        map.replace(3, []).unwrap();
        assert_eq!(map.display_count(), 3);
        assert_eq!(map.source_to_display(2), Some(2));
        map.replace(0, [(0,2)]).unwrap();
        assert_eq!(map.adjacent_source(0, true), None);
    }
}
