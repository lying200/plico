//! Unified presentation and byte mapping for a working-tree editor.
//! Local mapped edits advance the projection immediately. External source edits
//! must invalidate/rebuild it before accepting more input.
use std::{ops::Range, num::NonZeroU32};
use super::{DiffFile, DiffSide};

#[derive(Clone, Debug)]
struct Writable {
    display: Range<usize>,
    source: Range<usize>,
}

/// Deleted text belongs only to this presentation. Never save `text()` to disk;
/// map a validated edit to the shared working document instead.
#[derive(Clone, Debug)]
pub struct UnifiedEditProjection {
    text: String,
    writable: Vec<Writable>,
    deleted: Vec<Range<usize>>,
    deleted_source: Vec<Range<usize>>,
    line_numbers: Vec<[Option<NonZeroU32>; 2]>,
    working_side: DiffSide,
    working_line_count: usize,
    fixed_line_count: usize,
}

impl UnifiedEditProjection {
    /// Requires complete source documents, as supplied by `DiffFile::from_sources`.
    pub fn new(file: &DiffFile) -> Self {
        Self::for_side(file, DiffSide::Modified)
    }

    /// Preserve old-then-new presentation while selecting the writable document.
    /// The legacy `original_*` accessors describe the fixed, readonly fragments.
    pub fn for_side(file: &DiffFile, working_side: DiffSide) -> Self {
        let original = file.source(DiffSide::Original);
        let modified = file.source(DiffSide::Modified);
        let old_lines = boundaries(original);
        let new_lines = boundaries(modified);
        let left = working_side == DiffSide::Original;
        let working = if left { original } else { modified };
        let mut result = Self { text: String::new(), writable: Vec::new(), deleted: Vec::new(), deleted_source: Vec::new(), line_numbers: Vec::new(), working_side, working_line_count: boundaries(working).len(), fixed_line_count: boundaries(if left { modified } else { original }).len() };
        let mut working_end = 0;
        for (old, new) in file.change_line_ranges() {
            let old_range = boundary(&old_lines, old.start, original.len())
                ..boundary(&old_lines, old.end, original.len());
            let new_range = boundary(&new_lines, new.start, modified.len())
                ..boundary(&new_lines, new.end, modified.len());
            let range = if left { old_range.clone() } else { new_range.clone() };
            result.append_working(working, working_end..range.start);
            if left {
                result.append_working(original, old_range);
                // A working final line still needs a presentation-only separator
                // before the fixed row. Never map that separator to disk bytes.
                if !new_range.is_empty() { result.append_separator(new_range.start); }
                result.append_fixed(modified, new_range);
            } else {
                result.append_fixed(original, old_range);
                result.append_working(modified, new_range);
            }
            working_end = range.end;
        }
        result.append_working(working, working_end..working.len());
        if result.writable.last().is_none_or(|p| p.display.end != result.text.len()) {
            result.writable.push(Writable { display: result.text.len()..result.text.len(), source: working.len()..working.len() });
        }
        result.line_numbers = file.patch_lines().iter().map(|row| {
            [DiffSide::Original, DiffSide::Modified].map(|side|
                row.line(side).and_then(|line| number(file.line_number(side, line))))
        }).collect();
        let display_lines = result.text.bytes().filter(|&b| b == b'\n').count() + 1;
        while result.line_numbers.len() < display_lines {
            result.line_numbers.push([original, modified].map(|source|
                (source.is_empty() || source.ends_with('\n')).then(|| number(boundaries(source).len())).flatten()));
        }
        result
    }

    /// One-based source line numbers for each display row; absent on the other
    /// side's changed rows. Local edits invalidate touched fixed-side mappings.
    pub fn line_numbers(&self, row: usize) -> Option<[Option<usize>; 2]> {
        self.line_numbers.get(row).map(|numbers| numbers.map(|n| n.map(|n| n.get() as usize)))
    }
    pub fn max_line_number(&self) -> usize { self.working_line_count.max(self.fixed_line_count) }

    fn append_separator(&mut self, fixed_offset: usize) {
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            let start = self.text.len();
            self.text.push('\n');
            self.deleted.push(start..self.text.len());
            self.deleted_source.push(fixed_offset..fixed_offset);
        }
    }

    fn append_fixed(&mut self, source: &str, range: Range<usize>) {
        if range.is_empty() { return; }
        let start = self.text.len();
        self.text.push_str(&source[range.clone()]);
        if !self.text.ends_with('\n') { self.text.push('\n'); }
        self.deleted.push(start..self.text.len());
        self.deleted_source.push(range);
    }

    fn append_working(&mut self, source: &str, range: Range<usize>) {
        if range.is_empty() { return; }
        let start = self.text.len();
        self.text.push_str(&source[range.clone()]);
        if let Some(last) = self.writable.last_mut().filter(|last|
            last.display.end == start && last.source.end == range.start)
        {
            last.display.end = self.text.len();
            last.source.end = range.end;
        } else {
            self.writable.push(Writable { display: start..self.text.len(), source: range });
        }
    }

    pub fn text(&self) -> &str { &self.text }
    pub fn deleted_ranges(&self) -> &[Range<usize>] { &self.deleted }

    /// Rejects deleted/synthetic bytes, selections spanning a deleted block,
    /// invalid UTF-8 boundaries, and out-of-bounds positions. Endpoints
    /// on a real working row are allowed; no approximate mapping is used.
    pub fn working_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        if range.start > range.end || !self.text.is_char_boundary(range.start)
            || !self.text.is_char_boundary(range.end) { return None; }
        if range.is_empty() && self.deleted.iter().any(|r| r.contains(&range.start)) { return None; }
        self.writable.iter().rev().find_map(|part| {
            if range.start >= part.display.start && range.end <= part.display.end {
                Some((part.source.start + range.start - part.display.start)
                    ..(part.source.start + range.end - part.display.start))
            } else { None }
        })
    }

    /// Advance the presentation after the owner accepts this exact edit in the
    /// shared working document. Returns the pre-edit source range. Rejection
    /// leaves all state unchanged. Do this synchronously with the source edit;
    /// background rediff may replace the presentation later, but a second input
    /// must already see the new coordinates and shifted readonly ranges.
    pub fn apply_edit(&mut self, range: Range<usize>, replacement: &str) -> Option<Range<usize>> {
        let mapped = self.working_range(range.clone())?;
        let index = self.writable.iter().rposition(|part|
            range.start >= part.display.start && range.end <= part.display.end)?;
        // All adjusted endpoints are at/after the replaced range, hence these
        // subtractions cannot underflow. Check final length before mutation.
        self.text.len().checked_sub(range.len())?.checked_add(replacement.len())?;
        let shift = |offset: usize| offset - range.len() + replacement.len();
        // Rebased LineNumberConvertor: edit the working-side mapping synchronously;
        // touched fixed-side rows lose exact correspondence until the next diff.
        let start_line = self.text[..range.start].bytes().filter(|&b| b == b'\n').count();
        let old_lines = self.text[range.clone()].bytes().filter(|&b| b == b'\n').count() + 1;
        let new_lines = replacement.bytes().filter(|&b| b == b'\n').count() + 1;
        let side = self.working_side as usize;
        let source_line = self.line_numbers[start_line][side].map(|n| n.get() as usize).unwrap_or(self.working_line_count);
        for row in &mut self.line_numbers[start_line + old_lines..] {
            if let Some(value) = row[side] { row[side] = number(value.get() as usize - old_lines + new_lines); }
        }
        self.line_numbers.splice(start_line..start_line + old_lines, (0..new_lines).map(|offset| {
            let mut row = [None, None];
            row[side] = number(source_line + offset);
            row
        }));
        self.working_line_count = self.working_line_count - old_lines + new_lines;
        self.text.replace_range(range.clone(), replacement);
        let current = &mut self.writable[index];
        current.display.end = shift(current.display.end);
        current.source.end = shift(current.source.end);
        for part in &mut self.writable[index + 1..] {
            part.display = shift(part.display.start)..shift(part.display.end);
            part.source = shift(part.source.start)..shift(part.source.end);
        }
        for deleted in &mut self.deleted {
            if deleted.start >= range.end {
                *deleted = shift(deleted.start)..shift(deleted.end);
            }
        }
        Some(mapped)
    }

    /// Display fragments belonging to a working-source range, excluding deleted
    /// blocks even when a range endpoint is also a deletion boundary.
    pub fn display_ranges(&self, source: Range<usize>) -> Vec<Range<usize>> {
        self.writable.iter().filter_map(|part| {
            let start=source.start.max(part.source.start);
            let end=source.end.min(part.source.end);
            (start<end).then(||(part.display.start+start-part.source.start)..(part.display.start+end-part.source.start))
        }).collect()
    }

    /// Display fragments belonging to the original (deleted) source.
    pub fn original_display_ranges(&self, range: Range<usize>) -> Vec<Range<usize>> {
        self.deleted.iter().zip(&self.deleted_source).filter_map(|(shown,source)| {
            let start=range.start.max(source.start); let end=range.end.min(source.end);
            (start<end).then(|| shown.start+start-source.start..shown.start+end-source.start)
        }).collect()
    }

    /// Historical source position for a caret inside a deleted display row.
    pub fn original_offset(&self, display: usize) -> Option<usize> {
        if !self.text.is_char_boundary(display) { return None; }
        self.deleted.iter().zip(&self.deleted_source).find_map(|(shown,source)| {
            shown.contains(&display).then(||source.start+(display-shown.start).min(source.len()))
        })
    }

    /// Map caret/selection from the shared working document back to the view.
    /// Prefer the following working row at a deletion boundary, so navigation
    /// doesn't place the caret inside the readonly deleted block.
    pub fn display_offset(&self, source_offset: usize) -> Option<usize> {
        self.writable.iter().rev().find_map(|part| {
            if source_offset >= part.source.start && source_offset <= part.source.end {
                let offset = part.display.start + source_offset - part.source.start;
                self.text.is_char_boundary(offset).then_some(offset)
            } else { None }
        })
    }
}

fn boundaries(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(i,_)|i+1)).collect()
}
fn boundary(lines: &[usize], line: usize, end: usize) -> usize {
    lines.get(line).copied().unwrap_or(end)
}

// File previews are bounded well below u32::MAX lines; compact optional numbers
// take eight bytes per displayed row rather than two full Option<usize> values.
fn number(value: usize) -> Option<NonZeroU32> {
    NonZeroU32::new(u32::try_from(value).expect("source line exceeds u32 range"))
}
