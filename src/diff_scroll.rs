//! Two-editor Diff scroll mapping. The viewport's upper third is the anchor,
//! mapped through changed-block boundaries (including wrapped visual rows).
//! Behavior follows Rebased BaseSyncScrollable and SyncScrollSupport; see
//! docs/REBASED-REFERENCE.md for the pinned Apache-2.0 source reference.
use std::ops::Range;
use gpui_kit::{px, Pixels};
use gpui_kit::component::input::EditorState;

type Pair = (usize, usize);

fn enclosing(anchors: impl IntoIterator<Item = Pair>, line: usize) -> (Pair, Pair) {
    let mut previous = (0, 0);
    for next in anchors {
        if line <= next.0 { return (previous, next); }
        previous = next;
    }
    (previous, previous)
}

fn transfer(line: usize, start: Pair, end: Pair) -> usize {
    if line == start.0 { start.1 }
    else if line == end.0 { end.1 }
    else if line > end.0 { end.1 + (line - end.0) }
    else { (start.1 + line.saturating_sub(start.0)).min(end.1) }
}

pub fn vertical_offset(master: &EditorState, slave: &EditorState,
    changes: &[(Range<usize>, Range<usize>)], original: bool, offset: Pixels,
) -> Option<Pixels> {
    let height = master.line_height()?;
    let peer_height = slave.line_height()?;
    if height <= px(0.) || peer_height <= px(0.) { return None; }
    let anchor = master.input_bounds().size.height / 3.;
    let position = (-offset + anchor).max(px(0.));
    let row = (position / height).floor() as usize;
    let line = master.source_line_at_row(row);
    let endpoints = changes.iter().flat_map(|(a,b)| {
        if original { [(a.start,b.start),(a.end,b.end)] }
        else { [(b.start,a.start),(b.end,a.end)] }
    }).chain(std::iter::once((master.source_line_count(),slave.source_line_count())));
    let (start,end) = enclosing(endpoints,line);
    let visual = |(a,b)| (master.source_boundary_row(a),slave.source_boundary_row(b));
    let start=visual(start); let end=visual(end);
    let peer_row=transfer(row,start,end);
    let phase=position-height*row as f32;
    let desired=-(peer_height*peer_row as f32-anchor+phase);
    // At insertions/deletions several master rows map to the same peer row.
    // Suppress sub-line corrections that would make the peer jitter backwards.
    let current=slave.scroll_offset().y;
    let delta=current-desired;
    if (phase < height/2. && row>0 && transfer(row-1,start,end)==peer_row && delta<px(0.) && delta>-peer_height)
        || (phase > height/2. && transfer(row+1,start,end)==peer_row && delta>px(0.) && delta<peer_height) {
        return Some(current);
    }
    Some(desired.min(px(0.))) // editor clamps the lower document edge
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_crlf_coordinates_preserve_eol_changes() {
        use gpui_kit::component::diff::{DiffFile,DiffSide};
        let file=DiffFile::from_sources(Some(("a.txt","one\r\ntwo\r\n")),Some(("a.txt","one\ntwo\n"))).unwrap();
        let normalized=file.editor_coordinates();
        assert_eq!(normalized.change_line_ranges(),file.change_line_ranges());
        assert_eq!(normalized.source(DiffSide::Original),"one\ntwo\n");
        assert!(!normalized.change_line_ranges().is_empty());
        for side in [DiffSide::Original,DiffSide::Modified] {
            for (range,_,_) in normalized.editor_change_highlights(side) {
                assert!(range.end<=normalized.source(side).len());
            }
        }
    }
    #[test]
    fn insertion_and_deletion_preserve_unchanged_offsets() {
        let anchors=[(0,0),(10,10),(10,14),(30,34)];
        for (line,want) in [(0,0),(9,9),(10,10),(11,15),(30,34)] {
            let (a,b)=enclosing(anchors,line);assert_eq!(transfer(line,a,b),want);
        }
        let reverse=anchors.map(|(a,b)|(b,a));
        for (line,want) in [(10,10),(11,10),(13,10),(14,10),(15,11),(34,30)] {
            let (a,b)=enclosing(reverse,line);assert_eq!(transfer(line,a,b),want);
        }
    }
    #[test]
    fn unequal_replacements_use_rows_not_document_percentage() {
        assert_eq!(transfer(12,(10,20),(15,22)),22);
        assert_eq!(transfer(15,(10,20),(15,22)),22);
        assert_eq!(transfer(16,(10,20),(15,22)),23);
        // Wrapped boundaries use the same rule after converting to visual rows.
        assert_eq!(transfer(35,(30,40),(38,44)),44);
    }
    #[test]
    fn start_end_and_empty_documents_are_valid_boundaries() {
        for (anchors,line,want) in [(vec![(0,0),(0,3),(5,8)],1,4),
            (vec![(0,0),(5,5),(5,8)],5,5),(vec![(0,0)],0,0)] {
            let(a,b)=enclosing(anchors,line); assert_eq!(transfer(line,a,b),want);
        }
    }
}
