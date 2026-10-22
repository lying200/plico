//! Working-tree document state, independent of the editor and filesystem worker.
//! Saving is serialized per document. Only a successful worker acknowledgement
//! advances the disk baseline; edits made during IO remain dirty.
use std::sync::Arc;

use crate::project_files::FILE_LIMIT;

const BOM: &[u8] = b"\xef\xbb\xbf";

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    Binary,
    UnsupportedEncoding,
    SaveInProgress,
}

#[derive(Debug)]
pub struct Document {
    disk: Arc<[u8]>,
    text: String,
    separator: &'static str,
    bom: bool,
    saving: Option<Arc<[u8]>>,
}

/// The worker must compare the actual file bytes with `expected` before writing.
/// A missing file is a conflict, not permission to recreate it. This state layer
/// does not itself provide an atomic compare-and-write filesystem operation.
pub struct Save {
    pub expected: Arc<[u8]>,
    pub bytes: Arc<[u8]>,
}

fn decode(bytes: &[u8]) -> Result<(&str, bool), Error> {
    if bytes.len() > FILE_LIMIT {
        return Err(Error::TooLarge);
    }
    if bytes.starts_with(b"\xff\xfe") || bytes.starts_with(b"\xfe\xff") {
        return Err(Error::UnsupportedEncoding);
    }
    if bytes.contains(&0) {
        return Err(Error::Binary);
    }
    let bom = bytes.starts_with(BOM);
    let text = std::str::from_utf8(if bom { &bytes[3..] } else { bytes })
        .map_err(|_| Error::UnsupportedEncoding)?;
    Ok((text, bom))
}

fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

impl Document {
    pub fn new(bytes: Vec<u8>) -> Result<Self, Error> {
        let (text, bom) = decode(&bytes)?;
        // As with a document-level line separator, newly edited text uses the
        // first separator. An unchanged mixed-ending file is never rewritten.
        let separator = match text.find(['\r', '\n']) {
            Some(i) if text[i..].starts_with("\r\n") => "\r\n",
            Some(i) if text.as_bytes()[i] == b'\r' => "\r",
            _ => "\n",
        };
        let text = normalized(text);
        Ok(Self {
            disk: bytes.into(),
            text,
            separator,
            bom,
            saving: None,
        })
    }

    pub fn disk(&self) -> Arc<[u8]> {
        self.disk.clone()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Compare physical text without BOM. Keep exact mixed separators for an
    /// untouched document; edited text uses the same separator as saving.
    pub fn comparison_text(&self, editor_text: &str) -> String {
        if editor_text == self.text && !self.dirty() {
            decode(&self.disk)
                .expect("validated disk baseline")
                .0
                .to_owned()
        } else {
            editor_text.replace('\n', self.separator)
        }
    }

    pub fn edit(&mut self, text: String) -> Result<(), Error> {
        if text.len() > FILE_LIMIT {
            return Err(Error::TooLarge);
        }
        if text.contains('\0') {
            return Err(Error::Binary);
        }
        let text = normalized(&text);
        let encoded_len = text.len()
            + text.bytes().filter(|b| *b == b'\n').count() * (self.separator.len() - 1)
            + if self.bom { BOM.len() } else { 0 };
        if encoded_len > FILE_LIMIT {
            return Err(Error::TooLarge);
        }
        self.text = text;
        Ok(())
    }

    pub fn dirty(&self) -> bool {
        let (disk, _) = decode(&self.disk).expect("validated disk baseline");
        // Compare without allocating another full-file string on every edit.
        let mut bytes = disk.bytes().peekable();
        let normalized = std::iter::from_fn(move || {
            bytes.next().map(|b| {
                if b == b'\r' {
                    if bytes.peek() == Some(&b'\n') {
                        bytes.next();
                    }
                    b'\n'
                } else {
                    b
                }
            })
        });
        !self.text.bytes().eq(normalized)
    }

    pub fn saving(&self) -> bool {
        self.saving.is_some()
    }

    /// No-op saves do not rewrite bytes, preserving mixed endings and timestamps.
    pub fn begin_save(&mut self) -> Result<Option<Save>, Error> {
        if self.saving() {
            return Err(Error::SaveInProgress);
        }
        if !self.dirty() {
            return Ok(None);
        }
        let mut bytes = Vec::with_capacity(self.text.len() + 3);
        if self.bom {
            bytes.extend_from_slice(BOM);
        }
        bytes.extend_from_slice(self.text.replace('\n', self.separator).as_bytes());
        let bytes: Arc<[u8]> = bytes.into();
        self.saving = Some(bytes.clone());
        Ok(Some(Save {
            expected: self.disk.clone(),
            bytes,
        }))
    }

    /// An acknowledgement belongs only to its originating save allocation.
    /// A stale acknowledgement cannot clear a newer save or another document.
    pub fn finish_save(&mut self, save: Save, succeeded: bool) -> bool {
        if !self
            .saving
            .as_ref()
            .is_some_and(|bytes| Arc::ptr_eq(bytes, &save.bytes))
        {
            return false;
        }
        if succeeded {
            self.disk = save.bytes;
        }
        self.saving = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_bom_crlf_and_no_final_newline() {
        let mut d = Document::new(b"\xef\xbb\xbfalpha\r\nbeta".to_vec()).unwrap();
        assert_eq!(d.text(), "alpha\nbeta");
        d.edit("alpha\ngamma".into()).unwrap();
        let save = d.begin_save().unwrap().unwrap();
        assert_eq!(&*save.expected, b"\xef\xbb\xbfalpha\r\nbeta");
        assert_eq!(&*save.bytes, b"\xef\xbb\xbfalpha\r\ngamma");
        assert!(d.finish_save(save, true));
        assert!(!d.dirty());
    }

    #[test]
    fn undo_to_original_does_not_rewrite_mixed_endings() {
        let mut d = Document::new(b"a\r\nb\nc\r".to_vec()).unwrap();
        d.edit("different".into()).unwrap();
        d.edit("a\nb\nc\n".into()).unwrap();
        assert!(d.begin_save().unwrap().is_none());
    }

    #[test]
    fn edits_during_io_are_not_marked_saved() {
        let mut d = Document::new(b"old".to_vec()).unwrap();
        d.edit("first".into()).unwrap();
        let first = d.begin_save().unwrap().unwrap();
        d.edit("second".into()).unwrap();
        assert!(matches!(d.begin_save(), Err(Error::SaveInProgress)));
        assert!(d.finish_save(first, true));
        assert!(d.dirty());
        let second = d.begin_save().unwrap().unwrap();
        assert_eq!(&*second.expected, b"first");
        assert_eq!(&*second.bytes, b"second");
        d.finish_save(second, true);
        assert!(!d.dirty());
    }

    #[test]
    fn failure_retains_draft_and_original_baseline() {
        let mut d = Document::new(b"old".to_vec()).unwrap();
        d.edit("draft".into()).unwrap();
        let save = d.begin_save().unwrap().unwrap();
        d.finish_save(save, false);
        assert_eq!(d.text(), "draft");
        assert!(d.dirty());
        let retry = d.begin_save().unwrap().unwrap();
        assert_eq!(&*retry.expected, b"old");
    }

    #[test]
    fn another_document_cannot_acknowledge_save() {
        let mut a = Document::new(vec![]).unwrap();
        let mut b = Document::new(vec![]).unwrap();
        a.edit("same".into()).unwrap();
        b.edit("same".into()).unwrap();
        let save_a = a.begin_save().unwrap().unwrap();
        let save_b = b.begin_save().unwrap().unwrap();
        assert!(!a.finish_save(save_b, true));
        assert!(a.saving());
        assert!(a.finish_save(save_a, true));
    }

    #[test]
    fn unsupported_inputs_and_oversized_edits_preserve_content() {
        assert!(matches!(
            Document::new(vec![0xff]),
            Err(Error::UnsupportedEncoding)
        ));
        assert!(matches!(
            Document::new(vec![0xff, 0xfe, 0, 0]),
            Err(Error::UnsupportedEncoding)
        ));
        assert!(matches!(Document::new(vec![0]), Err(Error::Binary)));
        let mut d = Document::new(b"a\r\n".to_vec()).unwrap();
        assert_eq!(
            d.edit("\n".repeat(FILE_LIMIT / 2 + 1)),
            Err(Error::TooLarge)
        );
        assert_eq!(d.text(), "a\n");
        assert!(!d.dirty());
    }
}
