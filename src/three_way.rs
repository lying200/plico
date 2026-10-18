// Copyright 2000-2024 JetBrains s.r.o. and contributors.
// Apache-2.0 adaptation of ComparisonMergeUtil.FairMergeBuilder and
// MergeRangeUtil.getMergeType / SimpleThreesideTextDiffProvider.MERGE_RESULT.
//! Bounded three-way rows. The center is a merge result, never a merge base.
use crate::git::{Cancel, Result};
use std::{
    ops::Range,
    time::{Duration, Instant},
};
pub const MAX_ROWS: usize = 100_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Inserted,
    Deleted,
    Modified,
    Conflict,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub lines: [Option<usize>; 3],
    pub tints: [Option<Tint>; 3],
}
struct Equal {
    center: Range<usize>,
    side: Range<usize>,
}

pub fn align(texts: [&str; 3], token: &Cancel) -> Result<Vec<Row>> {
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    if texts
        .iter()
        .any(|s| s.len() > crate::file_history::TEXT_LIMIT)
    {
        return Err("File exceeds the 5 MiB preview limit".into());
    }
    let started = Instant::now();
    let lines = texts.map(|s| {
        s.split_inclusive('\n')
            .take(MAX_ROWS + 1)
            .collect::<Vec<_>>()
    });
    if lines.iter().any(|s| s.len() > MAX_ROWS) {
        return Err("Three-way comparison exceeds the 100000-row budget".into());
    }
    let equal = |side: usize| -> Result<Vec<Equal>> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let phase = Instant::now();
        let diff = similar::TextDiff::configure()
            .algorithm(similar::Algorithm::Patience)
            .timeout(Duration::from_millis(450))
            .diff_slices(&lines[1], &lines[side]);
        if phase.elapsed() > Duration::from_millis(430)
            || started.elapsed() > Duration::from_millis(850)
        {
            return Err("Three-way comparison exceeded the time budget".into());
        }
        Ok(diff
            .ops()
            .iter()
            .filter_map(|op| {
                let (tag, center, side) = op.as_tag_tuple();
                (tag == similar::DiffTag::Equal).then_some(Equal { center, side })
            })
            .collect())
    };
    let left = equal(0)?;
    let right = equal(2)?;
    let mut rows = Vec::new();
    let mut cursor = [0; 3];
    let (mut l, mut r) = (0, 0);
    // Intersect unchanged center ranges, exactly the FairMergeBuilder seam.
    // Gaps between joint equal ranges are indivisible three-way fragments.
    while let (Some(a), Some(b)) = (left.get(l), right.get(r)) {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let start = a.center.start.max(b.center.start);
        let end = a.center.end.min(b.center.end);
        if start < end {
            let starts = [
                a.side.start + start - a.center.start,
                start,
                b.side.start + start - b.center.start,
            ];
            append(&mut rows, &lines, cursor, starts, true)?;
            let ends = starts.map(|s| s + end - start);
            append(&mut rows, &lines, starts, ends, false)?;
            cursor = ends;
        }
        if a.center.end <= b.center.end {
            l += 1;
        } else {
            r += 1;
        }
    }
    append(
        &mut rows,
        &lines,
        cursor,
        lines.each_ref().map(|s| s.len()),
        true,
    )?;
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(rows)
}
fn append(
    rows: &mut Vec<Row>,
    lines: &[Vec<&str>; 3],
    starts: [usize; 3],
    ends: [usize; 3],
    changed: bool,
) -> Result<()> {
    let lengths = std::array::from_fn::<_, 3, _>(|i| ends[i] - starts[i]);
    let height = lengths.into_iter().max().unwrap();
    if rows.len().saturating_add(height) > MAX_ROWS {
        return Err("Three-way comparison exceeds the 100000-row budget".into());
    }
    let chunks = std::array::from_fn::<_, 3, _>(|i| &lines[i][starts[i]..ends[i]]);
    let tint = changed.then(|| merge_result_tint(chunks)).flatten();
    let active = [chunks[0] != chunks[1], true, chunks[2] != chunks[1]];
    for row in 0..height {
        rows.push(Row {
            lines: std::array::from_fn(|i| (row < lengths[i]).then_some(starts[i] + row)),
            tints: std::array::from_fn(|i| if active[i] { tint } else { None }),
        });
    }
    Ok(())
}
fn merge_result_tint(chunks: [&[&str]; 3]) -> Option<Tint> {
    use Tint::*;
    let [left, center, right] = chunks;
    if left == center && right == center {
        return None;
    }
    // MergeRangeUtil's insertion/deletion meaning is reversed for MERGE_RESULT.
    Some(if center.is_empty() {
        if left.is_empty() || right.is_empty() || left == right {
            Deleted
        } else {
            Conflict
        }
    } else if left.is_empty() && right.is_empty() {
        Inserted
    } else if left == center {
        if right.is_empty() { Inserted } else { Modified }
    } else if right == center {
        if left.is_empty() { Inserted } else { Modified }
    } else if left == right {
        Modified
    } else {
        Conflict
    })
}

/// Sparse source-byte ranges for word emphasis, never virtual padding offsets.
pub type InnerRanges = [std::collections::BTreeMap<usize, Vec<Range<usize>>>; 3];

/// Translate DiffUtil.compareThreesideInner: compare whole changed chunks,
/// union both comparisons in the result, and omit unchanged/empty sides.
/// Fine emphasis is optional: oversized or timed-out chunks retain line colors.
pub fn inner_ranges(texts: [&str; 3], rows: &[Row], token: &Cancel) -> Result<InnerRanges> {
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    if texts
        .iter()
        .any(|t| t.len() > crate::file_history::TEXT_LIMIT)
        || rows.len() > MAX_ROWS
    {
        return Err("Three-way emphasis exceeds the source budget".into());
    }
    let sources = texts.map(|s| {
        s.split_inclusive('\n')
            .take(MAX_ROWS + 1)
            .collect::<Vec<_>>()
    });
    let mut output: InnerRanges = Default::default();
    let started = Instant::now();
    let mut processed = 0;
    let mut count = 0;
    let mut start = 0;
    while start < rows.len() {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if rows[start].tints.iter().all(Option::is_none) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < rows.len() && rows[end].tints.iter().any(Option::is_some) {
            end += 1;
        }
        if started.elapsed() > Duration::from_millis(250)
            || processed >= 512 * 1024
            || count >= 100_000
        {
            break;
        }
        let indices: [Vec<usize>; 3] = std::array::from_fn(|col| {
            rows[start..end]
                .iter()
                .filter(|r| r.tints[col].is_some())
                .filter_map(|r| r.lines[col])
                .collect()
        });
        let mut chunks: [Option<String>; 3] = Default::default();
        let mut oversized = false;
        for col in 0..3 {
            if indices[col].is_empty() {
                continue;
            }
            let mut size = 0usize;
            for &line in &indices[col] {
                let text = sources[col]
                    .get(line)
                    .ok_or("Invalid three-way source line")?;
                size = size.saturating_add(text.len());
            }
            if size > 32 * 1024 {
                oversized = true;
                break;
            }
            let mut text = indices[col]
                .iter()
                .map(|&i| sources[col][i])
                .collect::<String>();
            if text.ends_with('\n') {
                text.pop();
                if text.ends_with('\r') {
                    text.pop();
                }
            }
            processed += size;
            chunks[col] = Some(text);
        }
        if !oversized {
            let active = (0..3).filter(|&i| chunks[i].is_some()).collect::<Vec<_>>();
            let pairs: Vec<_> = if active.len() == 3 {
                vec![(1, 0), (1, 2)]
            } else if active.len() == 2 {
                vec![(active[0], active[1])]
            } else {
                vec![]
            };
            let mut ranges: [Vec<Range<usize>>; 3] = Default::default();
            let mut timed_out = false;
            for (a, b) in pairs {
                if token.cancelled() {
                    return Err("Cancelled".into());
                }
                let phase = Instant::now();
                let diff = similar::TextDiff::configure()
                    .timeout(Duration::from_millis(20))
                    .diff_unicode_words(chunks[a].as_ref().unwrap(), chunks[b].as_ref().unwrap());
                if phase.elapsed() >= Duration::from_millis(18)
                    || started.elapsed() > Duration::from_millis(250)
                {
                    timed_out = true;
                    break;
                }
                let (mut aa, mut bb) = (0, 0);
                for change in diff.iter_all_changes() {
                    let length = change.value().len();
                    match change.tag() {
                        similar::ChangeTag::Equal => {
                            aa += length;
                            bb += length;
                        }
                        similar::ChangeTag::Delete => {
                            ranges[a].push(aa..aa + length);
                            aa += length;
                        }
                        similar::ChangeTag::Insert => {
                            ranges[b].push(bb..bb + length);
                            bb += length;
                        }
                    }
                }
            }
            if !timed_out {
                for col in 0..3 {
                    ranges[col].sort_by_key(|r| (r.start, r.end));
                    let mut merged: Vec<Range<usize>> = Vec::new();
                    for range in ranges[col].drain(..) {
                        if let Some(last) = merged.last_mut()
                            && last.end >= range.start
                        {
                            last.end = last.end.max(range.end);
                        } else {
                            merged.push(range);
                        }
                    }
                    let mut offset = 0;
                    let mut range_index = 0;
                    for &line in &indices[col] {
                        let source = sources[col][line];
                        let visible = source.trim_end_matches('\n').trim_end_matches('\r').len();
                        while range_index < merged.len() && merged[range_index].end <= offset {
                            range_index += 1;
                        }
                        for range in merged[range_index..]
                            .iter()
                            .take_while(|r| r.start < offset + visible)
                        {
                            let from = range.start.max(offset) - offset;
                            let to = range.end.min(offset + visible) - offset;
                            if from < to && count < 100_000 {
                                output[col].entry(line).or_default().push(from..to);
                                count += 1;
                            }
                        }
                        offset += source.len();
                    }
                }
            }
        }
        start = end;
    }
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    Ok(output)
}
