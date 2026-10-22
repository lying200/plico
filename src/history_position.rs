//! Scan Git's ordered IDs without retaining the preceding history.
use std::io::{self, Read};

pub(crate) fn read_position(mut input: impl Read, target: &str) -> io::Result<Option<usize>> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid history position record",
        )
    };
    let mut chunk = [0u8; 8192];
    let mut record = [0u8; 64];
    let (mut length, mut index, mut found) = (0usize, 0usize, None);
    loop {
        let count = input.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        for &byte in &chunk[..count] {
            if byte == b'\n' {
                if !matches!(length, 40 | 64) {
                    return Err(invalid());
                }
                if record[..length].eq_ignore_ascii_case(target.as_bytes()) && found.is_none() {
                    found = Some(index);
                }
                index = index.checked_add(1).ok_or_else(invalid)?;
                length = 0;
            } else {
                if length == record.len() || !byte.is_ascii_hexdigit() {
                    return Err(invalid());
                }
                record[length] = byte;
                length += 1;
            }
        }
    }
    // Git emits a newline after every %H, including its last record.
    if length != 0 {
        return Err(invalid());
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_records_cross_chunks_and_exceed_text_output_budget() {
        let target = "b".repeat(40);
        // Over 8 MiB, generated as a stream rather than allocated in the reader.
        struct Repeated {
            remaining: usize,
            offset: usize,
        }
        impl Read for Repeated {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                let count = output.len().min(self.remaining).min(37);
                for byte in &mut output[..count] {
                    *byte = if self.offset % 41 == 40 { b'\n' } else { b'a' };
                    self.offset += 1;
                }
                self.remaining -= count;
                Ok(count)
            }
        }
        let input = Repeated {
            remaining: 41 * 210_000,
            offset: 0,
        }
        .chain(io::Cursor::new(format!("{target}\n")));
        assert_eq!(read_position(input, &target).unwrap(), Some(210_000));
        assert_eq!(
            read_position(
                io::Cursor::new(format!("{}\n", "C".repeat(64))),
                &"c".repeat(64)
            )
            .unwrap(),
            Some(0)
        );
        assert_eq!(read_position(io::empty(), &target).unwrap(), None);
        assert_eq!(
            read_position(io::Cursor::new(format!("{}\n", "a".repeat(40))), &target).unwrap(),
            None
        );
    }
    #[test]
    fn rejects_malformed_or_truncated_records_even_after_match() {
        let target = "b".repeat(40);
        for tail in [
            "\n".to_owned(),
            "a".repeat(65),
            format!("{}\n", "x".repeat(40)),
            "a".repeat(40),
        ] {
            assert!(read_position(io::Cursor::new(format!("{target}\n{tail}")), &target).is_err());
        }
    }
}
