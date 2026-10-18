//! Application-supplied, bounded three-column source alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffTint {
    Inserted,
    Deleted,
    Modified,
    Conflict,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeWayRow {
    /// Zero-based source line in each column; None is non-selectable padding.
    pub lines: [Option<usize>; 3],
    pub tints: [Option<DiffTint>; 3],
}

/// Sparse, UTF-8 source-byte ranges keyed by real source line.
pub type ThreeWayInner = [std::collections::BTreeMap<usize, Vec<std::ops::Range<usize>>>; 3];
