//! GitRepositoryCommitter detectors, Rebased 4aec352 (JetBrains, Apache-2.0).
//! Stream identity and line-start matching matter; evidence alone is not success.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Signals {
    pub signing: bool,
    pub pinentry: bool,
    pub empty: bool,
}
impl Signals {
    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            signing: self.signing || other.signing,
            pinentry: self.pinentry || other.pinentry,
            empty: self.empty || other.empty,
        }
    }
}
pub(crate) struct Detector {
    stderr: bool,
    prefix: Vec<u8>,
    suffix: Vec<u8>,
    pub signals: Signals,
}
impl Detector {
    pub fn new(stderr: bool) -> Self {
        Self {
            stderr,
            prefix: Vec::new(),
            suffix: Vec::new(),
            signals: Signals::default(),
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if matches!(byte, b'\n' | b'\r') {
                self.prefix.clear();
                self.suffix.clear();
                continue;
            }
            if self.stderr {
                if self.suffix.len() == 40 {
                    self.suffix.remove(0);
                }
                self.suffix.push(byte);
                if self.suffix.ends_with(b"gpg failed to sign the data") {
                    self.signals.signing = true;
                    self.signals.pinentry = true;
                }
                if [b"No pinentry".as_slice(), b"pinentry", b"signing failed"]
                    .iter()
                    .any(|p| self.suffix.ends_with(p))
                {
                    self.signals.pinentry = true;
                }
            } else if self.prefix.len() < 40 {
                self.prefix.push(byte.to_ascii_lowercase());
                if [
                    b"no changes".as_slice(),
                    b"no changes added to commit",
                    b"nothing added to commit",
                    b"nothing to commit",
                ]
                .iter()
                .any(|p| self.prefix.as_slice() == *p)
                {
                    self.signals.empty = true;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_reads_and_truncated_output_keep_stream_specific_evidence() {
        let mut err = Detector::new(true);
        for b in b"error: gpg failed to sign the data\n" {
            err.feed(&[*b]);
        }
        err.feed(&vec![b'x'; 100_000]);
        assert!(err.signals.signing && err.signals.pinentry && !err.signals.empty);
        let mut out = Detector::new(false);
        for b in b"NOTHING TO COMMIT" {
            out.feed(&[*b]);
        }
        out.feed(&vec![b'x'; 100_000]);
        assert!(out.signals.empty);
    }
    #[test]
    fn wrong_stream_embedded_text_and_cross_line_matches_are_ignored() {
        let mut out = Detector::new(false);
        out.feed(b"gpg failed to sign the data\nnote: nothing to commit\nno cha\nnges");
        assert_eq!(out.signals, Signals::default());
        let mut err = Detector::new(true);
        err.feed(b"nothing to commit\ngpg failed to sign\r the data");
        assert_eq!(err.signals, Signals::default());
        err.feed(b"No pinentry");
        assert!(err.signals.pinentry && !err.signals.signing);
    }
}
