//! Rebased RangeExclusionState: whole blocks, or independent deleted/added lines.
use crate::line_changes::{self, Block};
use std::{ops::Range, path::PathBuf, sync::Arc};
#[derive(Clone, Debug)]
pub enum Included {
    All(bool),
    Lines {
        deleted: Vec<bool>,
        added: Vec<bool>,
    },
}
#[derive(Clone, Debug)]
pub struct Selection {
    pub original: Arc<str>,
    pub current: Arc<str>,
    pub blocks: Vec<Block>,
    pub included: Vec<Included>,
}
#[derive(Clone, Debug)]
pub struct File {
    pub path: PathBuf,
    pub selection: Selection,
}
impl Selection {
    pub fn new(original: Arc<str>, current: Arc<str>, included: bool) -> Self {
        let blocks = line_changes::blocks(&original, &current);
        let states = vec![Included::All(included); blocks.len()];
        Self {
            original,
            current,
            blocks,
            included: states,
        }
    }
    pub fn counts(&self) -> (usize, usize) {
        self.blocks
            .iter()
            .zip(&self.included)
            .fold((0, 0), |(n, t), (b, s)| {
                let total = b.original.len() + b.current.len();
                (
                    n + match s {
                        Included::All(v) => {
                            if *v {
                                total
                            } else {
                                0
                            }
                        }
                        Included::Lines { deleted, added } => {
                            deleted.iter().chain(added).filter(|v| **v).count()
                        }
                    },
                    t + total,
                )
            })
    }
    /// Visible differences count whole chunks until a chunk is split into lines.
    pub fn visible_counts(&self) -> (usize, usize) {
        self.included.iter().fold((0, 0), |(n, total), state| {
            let (included, count) = match state {
                Included::All(value) => (usize::from(*value), 1),
                Included::Lines { deleted, added } => (
                    deleted.iter().chain(added).filter(|v| **v).count(),
                    deleted.len() + added.len(),
                ),
            };
            (n + included, total + count)
        })
    }
    /// Whether Include / Exclude can change the selected tracker lines.
    pub fn selected_actions(&self, original: bool, lines: Range<usize>) -> (bool, bool) {
        let affected: Vec<_> = self
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                let r = if original { &b.original } else { &b.current };
                (r.start < lines.end && r.end > lines.start).then_some((i, r))
            })
            .collect();
        let partial = affected
            .iter()
            .any(|(_, r)| lines.start > r.start || lines.end < r.end);
        let mut result = (false, false);
        for (index, range) in affected {
            let (n, total) = if partial {
                match &self.included[index] {
                    Included::All(value) => (usize::from(*value), 1),
                    Included::Lines { deleted, added } => {
                        let mask = if original { deleted } else { added };
                        let selected = &mask[lines.start.max(range.start) - range.start
                            ..lines.end.min(range.end) - range.start];
                        (selected.iter().filter(|v| **v).count(), selected.len())
                    }
                }
            } else {
                self.block_counts(index)
            };
            result.0 |= n < total;
            result.1 |= n > 0;
        }
        result
    }
    pub fn block_counts(&self, index: usize) -> (usize, usize) {
        let b = &self.blocks[index];
        let total = b.original.len() + b.current.len();
        (
            match &self.included[index] {
                Included::All(v) => {
                    if *v {
                        total
                    } else {
                        0
                    }
                }
                Included::Lines { deleted, added } => {
                    deleted.iter().chain(added).filter(|v| **v).count()
                }
            },
            total,
        )
    }
    pub fn set_all(&mut self, included: bool) {
        self.included.fill(Included::All(included));
    }
    pub fn toggle_block(&mut self, index: usize) {
        let (n, t) = self.block_counts(index);
        self.included[index] = Included::All(n != t);
    }
    pub fn set_selected_lines(&mut self, original: bool, lines: Range<usize>, included: bool) {
        let affected = self
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                let r = if original { &b.original } else { &b.current };
                (r.start < lines.end && r.end > lines.start).then_some((i, r.clone()))
            })
            .collect::<Vec<_>>();
        if affected
            .iter()
            .any(|(_, r)| lines.start > r.start || lines.end < r.end)
        {
            self.set_lines(original, lines, included);
        } else {
            for (i, _) in affected {
                self.included[i] = Included::All(included);
            }
        }
    }
    pub fn set_lines(&mut self, original: bool, lines: Range<usize>, included: bool) {
        for (block, state) in self.blocks.iter().zip(&mut self.included) {
            let range = if original {
                &block.original
            } else {
                &block.current
            };
            if range.start >= lines.end || range.end <= lines.start {
                continue;
            }
            // Upstream splitting a whole block starts the opposite side excluded.
            if let Included::All(previous) = state {
                let mut deleted = vec![false; block.original.len()];
                let mut added = vec![false; block.current.len()];
                if *previous && !included {
                    if original {
                        deleted.fill(true)
                    } else {
                        added.fill(true)
                    }
                }
                *state = Included::Lines { deleted, added };
            }
            if let Included::Lines { deleted, added } = state {
                let mask = if original { deleted } else { added };
                for line in range.start.max(lines.start)..range.end.min(lines.end) {
                    mask[line - range.start] = included;
                }
            }
        }
    }
    pub fn compose(&self) -> Result<String, String> {
        if self.blocks != line_changes::blocks(&self.original, &self.current)
            || self.blocks.len() != self.included.len()
        {
            return Err("Change range is stale".into());
        }
        let old: Vec<_> = self.original.split_inclusive('\n').collect();
        let new: Vec<_> = self.current.split_inclusive('\n').collect();
        let mut result = String::new();
        let mut cursor = 0;
        for (block, state) in self.blocks.iter().zip(&self.included) {
            result.extend(old[cursor..block.original.start].iter().copied());
            match state {
                Included::All(true) => result.extend(new[block.current.clone()].iter().copied()),
                Included::All(false) => result.extend(old[block.original.clone()].iter().copied()),
                Included::Lines { deleted, added } => {
                    if deleted.len() != block.original.len() || added.len() != block.current.len() {
                        return Err("Invalid partial line selection".into());
                    }
                    result.extend(
                        old[block.original.clone()]
                            .iter()
                            .zip(deleted)
                            .filter_map(|(s, v)| (!v).then_some(*s)),
                    );
                    for line in new[block.current.clone()]
                        .iter()
                        .zip(added)
                        .filter_map(|(s, v)| v.then_some(*s))
                    {
                        if !result.is_empty() && !result.ends_with('\n') {
                            result.push('\n');
                        }
                        result.push_str(line);
                    }
                }
            }
            cursor = block.original.end;
        }
        result.extend(old[cursor..].iter().copied());
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_counts_track_chunks_until_split() {
        let mut s = Selection::new("a\nb\n".into(), "A\nB\n".into(), false);
        assert_eq!(s.visible_counts(), (0, 1));
        s.set_all(true);
        assert_eq!(s.visible_counts(), (1, 1));
        s.set_lines(false, 0..1, false);
        assert_eq!(s.visible_counts(), (1, 4));
        s.set_all(true);
        assert_eq!(s.visible_counts(), (1, 1));
    }
    #[test]
    fn menu_actions_follow_selected_changed_lines_and_whole_blocks() {
        let mut s = Selection::new(
            "same\na\nb\nend\n".into(),
            "same\nA\nB\nend\n".into(),
            false,
        );
        assert_eq!(s.selected_actions(false, 0..1), (false, false));
        assert_eq!(s.selected_actions(false, 1..2), (true, false));
        s.set_selected_lines(false, 1..2, true);
        assert_eq!(s.selected_actions(false, 1..2), (false, true));
        assert_eq!(s.selected_actions(false, 2..3), (true, false));
        // Selecting a whole block also considers excluded deletions on the other side.
        assert_eq!(s.selected_actions(false, 1..3), (true, true));
        s.set_selected_lines(false, 1..3, true);
        assert_eq!(s.selected_actions(true, 1..3), (false, true));
        assert_eq!(s.compose().unwrap(), s.current.as_ref());
        let added = Selection::new("".into(), "new\n".into(), false);
        assert_eq!(added.selected_actions(true, 0..1), (false, false));
        assert_eq!(added.selected_actions(false, 0..1), (true, false));
        let deleted = Selection::new("old\n".into(), "".into(), true);
        assert_eq!(deleted.selected_actions(false, 0..1), (false, false));
        assert_eq!(deleted.selected_actions(true, 0..1), (false, true));
    }
    #[test]
    fn split_from_included_excludes_opposite_side_like_tracker() {
        let mut s = Selection::new("a\nb\n".into(), "A\nB\n".into(), true);
        s.set_lines(false, 0..1, false);
        assert_eq!(s.compose().unwrap(), "a\nb\nB\n");
        assert_eq!(s.counts(), (1, 4));
        s.toggle_block(0);
        assert_eq!(s.compose().unwrap(), "A\nB\n");
        s.toggle_block(0);
        assert_eq!(s.compose().unwrap(), "a\nb\n");
    }
    #[test]
    fn additions_deletions_empty_and_eof() {
        let mut eof = Selection::new("old".into(), "new".into(), false);
        eof.set_lines(false, 0..1, true);
        assert_eq!(eof.compose().unwrap(), "old\nnew");
        let mut s = Selection::new("a\nb".into(), "".into(), false);
        s.set_lines(true, 1..2, true);
        assert_eq!(s.compose().unwrap(), "a\n");
        let mut s = Selection::new("".into(), "a\nb".into(), false);
        s.set_lines(false, 1..2, true);
        assert_eq!(s.compose().unwrap(), "b");
        s.set_all(true);
        assert_eq!(s.compose().unwrap(), "a\nb");
    }
}
