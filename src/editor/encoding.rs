//! GitConfigUtil encoding precedence; reject lossy conversions before writing.
use crate::git::{Cancel, Repository};
use std::{borrow::Cow, io};

#[derive(Clone, Copy)]
pub(super) enum Encoding {
    Utf8,
    Ascii,
    Latin1,
    Other(&'static encoding_rs::Encoding),
}
impl Encoding {
    pub(super) fn configured(repo: &Repository, sequence: bool, stop: &Cancel) -> io::Result<Self> {
        let commit = repo
            .text(
                &[
                    "config",
                    "--get",
                    "--default",
                    "UTF-8",
                    "i18n.commitEncoding",
                ],
                stop,
            )
            .map_err(io::Error::other)?;
        let name = if sequence {
            repo.text(
                &[
                    "config",
                    "--get",
                    "--default",
                    commit.trim(),
                    "i18n.logOutputEncoding",
                ],
                stop,
            )
            .map_err(io::Error::other)?
        } else {
            commit
        };
        Self::named(name.trim())
    }
    fn named(name: &str) -> io::Result<Self> {
        match name.to_ascii_lowercase().as_str() {
            "utf-8" | "utf8" => Ok(Self::Utf8),
            "ascii" | "us-ascii" | "ansi_x3.4-1968" => Ok(Self::Ascii),
            "iso-8859-1" | "iso8859-1" | "iso_8859-1" | "latin1" | "latin-1" | "l1" => {
                Ok(Self::Latin1)
            }
            _ => {
                // WHATWG treats several ISO labels as Windows code pages. Do
                // not silently substitute those aliases for Java/Git charsets.
                let encoding = encoding_rs::Encoding::for_label(name.as_bytes())
                    .filter(|e| e.is_ascii_compatible() && e.name().eq_ignore_ascii_case(name))
                    .ok_or_else(|| {
                        io::Error::other(format!("Unsupported Git editor encoding: {name}"))
                    })?;
                Ok(Self::Other(encoding))
            }
        }
    }
    pub(super) fn decode(self, bytes: &[u8]) -> io::Result<String> {
        let text = match self {
            Self::Utf8 => std::str::from_utf8(bytes).ok().map(str::to_owned),
            Self::Ascii => bytes
                .is_ascii()
                .then(|| String::from_utf8(bytes.to_vec()).unwrap()),
            Self::Latin1 => Some(bytes.iter().map(|&b| char::from(b)).collect()),
            Self::Other(e) => e
                .decode_without_bom_handling_and_without_replacement(bytes)
                .map(Cow::into_owned),
        };
        text.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid bytes for Git editor encoding",
            )
        })
    }
    pub(super) fn encode(self, text: &str) -> io::Result<Vec<u8>> {
        let bytes = match self {
            Self::Utf8 => Some(text.as_bytes().to_vec()),
            Self::Ascii => text.is_ascii().then(|| text.as_bytes().to_vec()),
            Self::Latin1 => text
                .chars()
                .map(|c| u8::try_from(u32::from(c)).ok())
                .collect(),
            Self::Other(e) => {
                let (bytes, _, errors) = e.encode(text);
                (!errors).then(|| bytes.into_owned())
            }
        }
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Message cannot be represented in Git editor encoding",
            )
        })?;
        if self.decode(&bytes)? != text {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Git editor encoding would change the message",
            ));
        }
        Ok(bytes)
    }
}
