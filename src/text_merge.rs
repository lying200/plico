//! Three-way text decisions, independent of editors and Git writes.
//! Rebased 4aec352: MergeConflictModel seeds the base and applies individual sides.
use crate::line_changes::{self, Block};
use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub enum Choice {
    Yours,
    Theirs,
    Both,
    Base,
}
#[derive(Clone, Copy, Debug)]
pub enum Side {
    Yours,
    Theirs,
}
impl Side {
    pub fn index(self) -> usize {
        match self {
            Self::Yours => 0,
            Self::Theirs => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Added,
    Deleted,
    Modified,
    Conflict,
}

#[derive(Debug)]
pub struct Change {
    pub result: Range<usize>,
    pub yours: String,
    pub yours_range: Range<usize>,
    pub theirs_range: Range<usize>,
    manual: bool,
    pub theirs: String,
    pub base: String,
    pub resolved: bool,
    pub sides: [bool; 2],
    pub appended: bool,
    /// A manual edit spanning multiple changes makes individual replacement ambiguous.
    pub ambiguous: bool,
}
impl Change {
    pub fn conflict(&self) -> bool {
        self.yours != self.base && self.theirs != self.base && self.yours != self.theirs
    }
    pub fn changed(&self, side: Side) -> bool {
        match side {
            Side::Yours => self.yours != self.base,
            Side::Theirs => self.theirs != self.base,
        }
    }
    pub fn kind(&self) -> Kind {
        if self.conflict() {
            Kind::Conflict
        } else if self.base.is_empty() {
            Kind::Added
        } else if (self.changed(Side::Yours) && self.yours.is_empty())
            || (self.changed(Side::Theirs) && self.theirs.is_empty())
        {
            Kind::Deleted
        } else {
            Kind::Modified
        }
    }
    pub fn available(&self, side: Side) -> bool {
        !self.ambiguous
            && !self.resolved
            && !self.sides[side.index()]
            && match side {
                Side::Yours => self.yours != self.base,
                Side::Theirs => self.theirs != self.base,
            }
    }
}
/// Undo stores only mutable block metadata, never copies of the source files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State(Vec<ChangeState>);
#[derive(Clone, Debug, PartialEq, Eq)]
struct ChangeState {
    result: Range<usize>,
    manual: bool,
    resolved: bool,
    sides: [bool; 2],
    appended: bool,
    ambiguous: bool,
}
pub struct Merge {
    pub changes: Vec<Change>,
    text: String,
}
impl Merge {
    pub fn new(base: &str, yours: &str, theirs: &str) -> Self {
        let left = line_changes::blocks(base, yours);
        let right = line_changes::blocks(base, theirs);
        let mut ranges: Vec<_> = left
            .iter()
            .chain(&right)
            .map(|b| b.original.clone())
            .collect();
        ranges.sort_by_key(|r| (r.start, r.end));
        let mut groups: Vec<Range<usize>> = Vec::new();
        for range in ranges {
            // Adjacent replacements remain independent. Insertions on a shared
            // boundary are grouped conservatively rather than silently reordered.
            if let Some(last) = groups.last_mut()
                && (range.start < last.end
                    || (range.start == last.end && (range.is_empty() || last.start == last.end)))
            {
                last.end = last.end.max(range.end);
                continue;
            }
            groups.push(range);
        }
        let offsets = line_offsets(base);
        let yours_offsets = line_offsets(yours);
        let theirs_offsets = line_offsets(theirs);
        let (mut left_index, mut right_index) = (0, 0);
        let (mut left_delta, mut right_delta) = (0, 0);
        let changes = groups
            .into_iter()
            .map(|range| {
                let result = offsets[range.start]..offsets[range.end];
                let yours_range = side_range(
                    &yours_offsets,
                    &left,
                    &range,
                    &mut left_index,
                    &mut left_delta,
                );
                let theirs_range = side_range(
                    &theirs_offsets,
                    &right,
                    &range,
                    &mut right_index,
                    &mut right_delta,
                );
                Change {
                    base: base[result.clone()].to_owned(),
                    result,
                    yours: yours[yours_range.clone()].to_owned(),
                    theirs: theirs[theirs_range.clone()].to_owned(),
                    yours_range,
                    theirs_range,
                    manual: false,
                    resolved: false,
                    sides: [false; 2],
                    appended: false,
                    ambiguous: false,
                }
            })
            .collect();
        Self {
            changes,
            text: base.to_owned(),
        }
    }
    pub fn state(&self) -> State {
        State(
            self.changes
                .iter()
                .map(|c| ChangeState {
                    result: c.result.clone(),
                    manual: c.manual,
                    resolved: c.resolved,
                    sides: c.sides,
                    appended: c.appended,
                    ambiguous: c.ambiguous,
                })
                .collect(),
        )
    }
    pub fn restore(&mut self, state: &State, text: &str) {
        assert_eq!(self.changes.len(), state.0.len());
        for (c, saved) in self.changes.iter_mut().zip(&state.0) {
            c.result = saved.result.clone();
            c.manual = saved.manual;
            c.resolved = saved.resolved;
            c.sides = saved.sides;
            c.appended = saved.appended;
            c.ambiguous = saved.ambiguous;
        }
        self.text = text.to_owned();
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn remaining(&self) -> usize {
        self.changes.iter().filter(|c| !c.resolved).count()
    }
    /// Synchronize an editor transaction without overwriting manual text.
    pub fn edited(&mut self, text: &str) {
        if text == self.text {
            return;
        }
        let prefix = self
            .text
            .chars()
            .zip(text.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let suffix = self.text[prefix..]
            .chars()
            .rev()
            .zip(text[prefix..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let old_end = self.text.len() - suffix;
        let new_end = text.len() - suffix;
        let touched = self
            .changes
            .iter()
            .filter(|c| c.result.start <= old_end && c.result.end >= prefix)
            .count();
        for c in &mut self.changes {
            if c.result.end < prefix {
                continue;
            }
            if c.result.start > old_end {
                c.result = (c.result.start - old_end + new_end)..(c.result.end - old_end + new_end);
            } else {
                c.result.start = c.result.start.min(prefix);
                c.result.end = if c.result.end >= old_end {
                    c.result.end - old_end + new_end
                } else {
                    new_end
                };
                c.ambiguous |= touched > 1;
                c.manual = true;
                // Document edits move ranges, but do not undo merge decisions.
            }
        }
        self.text = text.to_owned();
    }
    /// Rebased distinguishes ignoring one side from restoring the base.
    pub fn ignore(&mut self, index: usize, side: Side, whole: bool) {
        let Some(c) = self.changes.get_mut(index) else {
            return;
        };
        if !c.available(side) {
            return;
        }
        if whole || !c.conflict() {
            c.sides = [true; 2];
        } else {
            c.sides[side.index()] = true;
        }
        c.resolved = c.sides.iter().all(|v| *v);
    }
    pub fn accept(
        &mut self,
        index: usize,
        side: Side,
        whole: bool,
    ) -> Option<(Range<usize>, String)> {
        let c = self.changes.get(index)?;
        if !c.available(side) {
            return None;
        }
        let (source, other) = match side {
            Side::Yours => (&c.yours, &c.theirs),
            Side::Theirs => (&c.theirs, &c.yours),
        };
        let conflict = c.conflict();
        let mut sides = c.sides;
        if whole || !conflict || other.is_empty() {
            sides = [true; 2];
        } else {
            sides[side.index()] = true;
        }
        let text = if conflict && c.appended {
            let current = &self.text[c.result.clone()];
            format!(
                "{}{}{}",
                current,
                if !current.is_empty() && !source.is_empty() && !current.ends_with('\n') {
                    "\n"
                } else {
                    ""
                },
                source
            )
        } else {
            source.clone()
        };
        let result = self.replace(index, text);
        let c = &mut self.changes[index];
        c.sides = sides;
        c.resolved = sides.iter().all(|v| *v);
        c.appended = conflict;
        Some(result)
    }
    fn replace(&mut self, index: usize, text: String) -> (Range<usize>, String) {
        let range = self.changes[index].result.clone();
        self.text.replace_range(range.clone(), &text);
        let end = range.start + text.len();
        for c in self.changes.iter_mut().skip(index + 1) {
            c.result = (c.result.start - range.end + end)..(c.result.end - range.end + end);
        }
        self.changes[index].result = range.start..end;
        (range, text)
    }
    /// Returns the editor replacement, keeping it in the editor's undo history.
    pub fn choose(&mut self, index: usize, choice: Choice) -> Option<(Range<usize>, String)> {
        let c = self.changes.get(index)?;
        if c.ambiguous {
            return None;
        }
        let text = match choice {
            Choice::Yours => c.yours.clone(),
            Choice::Theirs => c.theirs.clone(),
            Choice::Base => c.base.clone(),
            Choice::Both if c.yours == c.theirs || c.theirs == c.base => c.yours.clone(),
            Choice::Both if c.yours == c.base => c.theirs.clone(),
            Choice::Both => format!(
                "{}{}{}",
                c.yours,
                if !c.yours.is_empty() && !c.theirs.is_empty() && !c.yours.ends_with('\n') {
                    "\n"
                } else {
                    ""
                },
                c.theirs
            ),
        };
        let replacement = self.replace(index, text);
        self.changes[index].resolved = true;
        self.changes[index].sides = [true; 2];
        Some(replacement)
    }
    pub fn automatic_choice(&self, index: usize) -> Option<Choice> {
        let c = &self.changes[index];
        if c.resolved || c.ambiguous || c.manual {
            None
        } else if c.yours == c.theirs || c.theirs == c.base {
            Some(Choice::Yours)
        } else if c.yours == c.base {
            Some(Choice::Theirs)
        } else {
            None
        }
    }
}
fn line_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for line in text.split_inclusive('\n') {
        offsets.push(offsets.last().unwrap() + line.len());
    }
    offsets
}
/// Walk each side's ordered edits once. Do not rescan an entire source for
/// every conflict: generated files can contain many thousands of small edits.
fn side_range(
    offsets: &[usize],
    edits: &[Block],
    range: &Range<usize>,
    next: &mut usize,
    delta: &mut isize,
) -> Range<usize> {
    let start = (range.start as isize + *delta) as usize;
    while let Some(edit) = edits.get(*next) {
        if edit.original.start < range.start || edit.original.end > range.end {
            break;
        }
        *delta += edit.current.len() as isize - edit.original.len() as isize;
        *next += 1;
    }
    let end = (range.end as isize + *delta) as usize;
    offsets[start]..offsets[end]
}
#[cfg(test)]
mod tests {
    #[test]
    fn manual_edit_preserves_accepted_side_and_append() {
        let mut m = Merge::new("base\n", "left\n", "right\n");
        m.accept(0, Side::Yours, false).unwrap();
        m.edited("left edited\n");
        assert_eq!(m.changes[0].sides, [true, false]);
        m.accept(0, Side::Theirs, false).unwrap();
        assert_eq!(m.text(), "left edited\nright\n");
        assert_eq!(m.remaining(), 0);
    }
    #[test]
    fn semantic_types_and_changed_sides_survive_resolution() {
        for (base, left, right, kind, sides) in [
            ("", "a\n", "", Kind::Added, [true, false]),
            ("", "", "a\n", Kind::Added, [false, true]),
            ("", "a\n", "a\n", Kind::Added, [true, true]),
            ("a\n", "", "a\n", Kind::Deleted, [true, false]),
            ("a\n", "", "", Kind::Deleted, [true, true]),
            ("a\n", "b\n", "a\n", Kind::Modified, [true, false]),
            ("a\n", "b\n", "b\n", Kind::Modified, [true, true]),
            ("a\n", "", "b\n", Kind::Conflict, [true, true]),
            ("", "a\n", "b\n", Kind::Conflict, [true, true]),
        ] {
            let mut merge = Merge::new(base, left, right);
            assert_eq!(merge.changes.len(), 1);
            assert_eq!(merge.changes[0].kind(), kind);
            assert_eq!(
                [
                    merge.changes[0].changed(Side::Yours),
                    merge.changes[0].changed(Side::Theirs)
                ],
                sides
            );
            merge.choose(0, Choice::Yours);
            assert_eq!(merge.changes[0].kind(), kind);
        }
    }
    use super::*;
    #[test]
    fn sides_accept_in_click_order_and_ignore_without_restoring_base() {
        for (first, second, expected) in [
            (Side::Yours, Side::Theirs, "left\nright\n"),
            (Side::Theirs, Side::Yours, "right\nleft\n"),
        ] {
            let mut m = Merge::new("base\n", "left\n", "right\n");
            m.accept(0, first, false).unwrap();
            assert_eq!(m.remaining(), 1);
            assert!(!m.changes[0].available(first));
            assert!(m.changes[0].available(second));
            m.accept(0, second, false).unwrap();
            assert_eq!(m.text(), expected);
            assert_eq!(m.remaining(), 0);
        }
        let mut m = Merge::new("base\n", "left\n", "right\n");
        m.accept(0, Side::Yours, false).unwrap();
        m.ignore(0, Side::Theirs, false);
        assert_eq!(m.text(), "left\n");
        assert_eq!(m.remaining(), 0);
        let mut m = Merge::new("base\n", "left\n", "right\n");
        m.ignore(0, Side::Yours, false);
        assert_eq!(m.remaining(), 1);
        m.ignore(0, Side::Theirs, false);
        assert_eq!(m.text(), "base\n");
        assert_eq!(m.remaining(), 0);
    }
    #[test]
    fn side_operations_handle_ctrl_delete_and_manual_edits() {
        let mut m = Merge::new("base\n", "left\n", "right\n");
        m.accept(0, Side::Theirs, true).unwrap();
        assert_eq!(m.remaining(), 0);
        assert!(m.accept(0, Side::Yours, false).is_none());
        m.edited("manual\n");
        assert!(!m.changes[0].available(Side::Yours));
        assert_eq!(m.remaining(), 0);
        assert_eq!(m.text(), "manual\n");
        let mut m = Merge::new("base\n", "left\n", "");
        m.accept(0, Side::Yours, false).unwrap();
        assert_eq!(m.remaining(), 0);
        let mut m = Merge::new("base\n", "left\n", "base\n");
        m.ignore(0, Side::Yours, false);
        assert_eq!(m.remaining(), 0);
        assert_eq!(m.text(), "base\n");
    }
    #[test]
    fn choosing_every_block_reconstructs_each_side_and_ranges() {
        let samples = [
            "",
            "a\n",
            "b\n",
            "a\nb\n",
            "x\na\n",
            "a\nx\nb",
            "a\nb\nx\n",
            "a\n中\nb\n",
        ];
        for base in samples {
            for yours in samples {
                for theirs in samples {
                    for (choice, expected) in [(Choice::Yours, yours), (Choice::Theirs, theirs)] {
                        let mut m = Merge::new(base, yours, theirs);
                        for c in &m.changes {
                            assert_eq!(&yours[c.yours_range.clone()], c.yours);
                            assert_eq!(&theirs[c.theirs_range.clone()], c.theirs);
                        }
                        for i in 0..m.changes.len() {
                            m.choose(i, choice).unwrap();
                        }
                        assert_eq!(m.text(), expected, "{base:?} / {yours:?} / {theirs:?}");
                    }
                }
            }
        }
    }
    #[test]
    fn accepting_both_does_not_duplicate_equal_or_unchanged_sides() {
        for (base, a, b, expected) in [
            ("old\n", "new\n", "new\n", "new\n"),
            ("old\n", "new\n", "old\n", "new\n"),
            ("", "left", "right", "left\nright"),
        ] {
            let mut m = Merge::new(base, a, b);
            m.choose(0, Choice::Both).unwrap();
            assert_eq!(m.text(), expected);
        }
    }
    #[test]
    fn automatic_application_does_not_overwrite_manual_changes() {
        let mut m = Merge::new("a\n", "A\n", "a\n");
        assert!(m.automatic_choice(0).is_some());
        m.edited("manual\n");
        assert!(m.automatic_choice(0).is_none());
        m.choose(0, Choice::Theirs).unwrap();
        assert_eq!(m.text(), "a\n");
    }
    #[test]
    fn independent_changes_and_conflicts_preserve_all_text() {
        let mut m = Merge::new(
            "a\nx\nb\ny\nc\nd\n",
            "A\nx\nb\ny\n左\nd\n",
            "a\nx\nB\ny\n右\nd\n",
        );
        for i in 0..m.changes.len() {
            if let Some(c) = m.automatic_choice(i) {
                m.choose(i, c).unwrap();
            }
        }
        assert_eq!(m.text(), "A\nx\nB\ny\nc\nd\n");
        assert_eq!(m.remaining(), 1);
        m.choose(2, Choice::Both).unwrap();
        assert_eq!(m.text(), "A\nx\nB\ny\n左\n右\nd\n");
    }
    #[test]
    fn insert_delete_add_add_and_missing_final_newline() {
        for (base, a, b, expected) in [
            ("", "左", "右", "左"),
            ("x\ny\n", "y\n", "x\ny\n", "y\n"),
            ("x\n", "x\ny", "x\ny", "x\ny"),
        ] {
            let mut m = Merge::new(base, a, b);
            for i in 0..m.changes.len() {
                m.choose(i, Choice::Yours).unwrap();
            }
            assert_eq!(m.text(), expected);
        }
    }
    #[test]
    fn manual_edits_shift_later_choices_without_losing_text() {
        let mut m = Merge::new("a\nb\nc\n", "A\nb\nC\n", "Z\nb\nD\n");
        m.edited("手写\nb\nc\n");
        m.changes[0].resolved = true;
        m.choose(1, Choice::Theirs).unwrap();
        assert_eq!(m.text(), "手写\nb\nD\n");
        m.edited("whole document rewritten");
        assert!(m.choose(0, Choice::Yours).is_none());
        assert_eq!(m.text(), "whole document rewritten");
    }
}
