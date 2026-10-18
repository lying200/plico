//! Unified presentation and byte mapping for a working-tree editor.
//! Local mapped edits advance the projection immediately. External source edits
//! must invalidate/rebuild it before accepting more input.
use std::ops::Range;
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
}

impl UnifiedEditProjection {
    /// Requires complete source documents, as supplied by `DiffFile::from_sources`.
    pub fn new(file: &DiffFile) -> Self {
        let original = file.source(DiffSide::Original);
        let modified = file.source(DiffSide::Modified);
        let old_lines = boundaries(original);
        let new_lines = boundaries(modified);
        let mut result = Self { text: String::new(), writable: Vec::new(), deleted: Vec::new(), deleted_source: Vec::new() };
        let mut new_end = 0;
        for (old, new) in file.change_line_ranges() {
            let start = boundary(&new_lines, new.start, modified.len());
            result.append_working(modified, new_end..start);
            let old_start = boundary(&old_lines, old.start, original.len());
            let old_end = boundary(&old_lines, old.end, original.len());
            if old_start < old_end {
                let start = result.text.len();
                result.text.push_str(&original[old_start..old_end]);
                // A deleted final line without a newline still occupies its own
                // display row. This separator is never a working-file byte.
                if !result.text.ends_with('\n') { result.text.push('\n'); }
                result.deleted.push(start..result.text.len());
                result.deleted_source.push(old_start..old_end);
            }
            new_end = boundary(&new_lines, new.end, modified.len());
            result.append_working(modified, start..new_end);
        }
        result.append_working(modified, new_end..modified.len());
        if result.writable.last().is_none_or(|p| p.display.end != result.text.len()) {
            result.writable.push(Writable { display: result.text.len()..result.text.len(), source: modified.len()..modified.len() });
        }
        result
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
