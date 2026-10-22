// Detection phrases: Copyright 2000-2025 JetBrains s.r.o. and contributors.
// Licensed under Apache-2.0; see LICENSE.
//! GitSimpleEventDetector behavior from Rebased 4aec352 (Apache-2.0).
//! Source: plugins/git4idea/backend/src/commands/GitSimpleEventDetector.java.
//! Evidence only: callers must combine events with exit status and repository state.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Events(u16);
#[derive(Clone, Copy, Debug)]
pub enum Event {
    MergeConflict,
    UnstashConflict,
    UnstashIndexConflict,
    AlreadyUpToDate,
    UnmergedPreventingMerge,
    UnmergedPreventingCheckout,
    LocalChangesWouldBeOverwritten,
    UntrackedFilesWouldBeOverwritten,
    RebaseNoChanges,
    ApplyChangesWouldBeOverwritten,
}
// Modern headers from GitLocalChangesWouldBeOverwrittenDetector and
// GitUntrackedFilesOverwrittenByOperationDetector. Stream evidence survives
// bounded output-tail truncation; affected paths are collected separately later.
const PATTERNS: &[(Event, &[u8])] = &[
    // GitLocalChangesConflictDetector: catches revert/cherry-pick headers without a file list.
    (
        Event::ApplyChangesWouldBeOverwritten,
        b"would be overwritten by ",
    ),
    // GitRebaseProblemDetector (apply backend; merge backend no longer emits this).
    (
        Event::RebaseNoChanges,
        b"no changes - did you forget to use 'git add'?",
    ),
    (
        Event::LocalChangesWouldBeOverwritten,
        b"your local changes to the following files would be overwritten by",
    ),
    (
        Event::UntrackedFilesWouldBeOverwritten,
        b"the following untracked working tree files would be overwritten by",
    ),
    (
        Event::MergeConflict,
        b"automatic merge failed; fix conflicts and then commit the result",
    ),
    (Event::UnstashConflict, b"conflict"),
    (Event::UnstashIndexConflict, b"conflicts in index"),
    (Event::AlreadyUpToDate, b"already up-to-date"),
    (Event::AlreadyUpToDate, b"already up to date"),
    (
        Event::UnmergedPreventingMerge,
        b"is not possible because you have unmerged files",
    ),
    (
        Event::UnmergedPreventingCheckout,
        b"you need to resolve your current index first",
    ),
];
impl Events {
    pub fn contains(self, event: Event) -> bool {
        self.0 & (1 << event as u16) != 0
    }
    pub(crate) fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// ASCII detection phrases match upstream case-insensitive substring checks.
/// A fixed suffix handles split pipe reads, unlimited lines and EOF without LF.
/// CR/LF reset the suffix so phrases cannot accidentally span separate lines.
#[derive(Default)]
pub(crate) struct Detector {
    suffix: Vec<u8>,
    pub events: Events,
}
impl Detector {
    pub fn feed(&mut self, bytes: &[u8]) {
        let limit = PATTERNS.iter().map(|(_, s)| s.len()).max().unwrap();
        for &byte in bytes {
            if byte == b'\r' || byte == b'\n' {
                self.suffix.clear();
                continue;
            }
            if self.suffix.len() == limit {
                self.suffix.remove(0);
            }
            self.suffix.push(byte.to_ascii_lowercase());
            for &(event, pattern) in PATTERNS {
                if self.suffix.ends_with(pattern) {
                    self.events.0 |= 1 << event as u16;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_messages_and_line_boundaries() {
        let mut detector = Detector::default();
        for byte in b"CONFLICTS IN INDEX\rAlready up-to-date\n" {
            detector.feed(&[*byte]);
        }
        assert!(detector.events.contains(Event::UnstashIndexConflict));
        assert!(detector.events.contains(Event::UnstashConflict));
        assert!(detector.events.contains(Event::AlreadyUpToDate));
        let mut detector = Detector::default();
        detector.feed(b"Already up\nto date\nconf\rlict");
        assert_eq!(detector.events, Events::default());
        for byte in b"Your local changes to the following files would be overwritten by checkout:\nThe following untracked working tree files would be overwritten by checkout:\n" {
            detector.feed(&[*byte]);
        }
        // A later long diagnostic must not erase previously detected headers.
        detector.feed(&vec![b'x'; 100_000]);
        assert!(
            detector
                .events
                .contains(Event::LocalChangesWouldBeOverwritten)
        );
        assert!(
            detector
                .events
                .contains(Event::UntrackedFilesWouldBeOverwritten)
        );
        let mut detector = Detector::default();
        for byte in b"No changes - did you forget to use 'git add'?" {
            detector.feed(&[*byte]);
        }
        detector.feed(&vec![b'x'; 100_000]);
        assert!(detector.events.contains(Event::RebaseNoChanges));
    }
}
