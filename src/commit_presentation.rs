//! Read-only Log details presentation; metadata is separate from the editor-font message.
use crate::{
    git::{Commit, Details},
    i18n::{Message, render_message},
};
use chrono::{DateTime, Local};

pub struct Presentation {
    pub message: String,
    /// Only the selected commit is retained; switching language requires no Git reload.
    pub metadata: [String; 2],
}

pub fn build(details: &Details, commit: Option<&Commit>) -> Presentation {
    let mut lines = details.message.trim_end().lines();
    let mut message = format!(
        "<p><strong>{}</strong></p>",
        message_line(lines.next().unwrap_or_default())
    );
    for line in lines {
        message.push_str("<p>");
        message.push_str(&if line.is_empty() {
            "&nbsp;".into()
        } else {
            message_line(line)
        });
        message.push_str("</p>");
    }
    let metadata = [
        metadata(details, commit, false),
        metadata(details, commit, true),
    ];
    Presentation { message, metadata }
}

fn metadata(details: &Details, commit: Option<&Commit>, chinese: bool) -> String {
    let hash = commit
        .map(|c| c.id.chars().take(8).collect::<String>())
        .unwrap_or_default();
    let author = identity(&details.author);
    let authored = date(&details.authored, chinese);
    let mut metadata = format!(
        "<p>{} {}</p>",
        escape(&hash),
        render_message(
            Message::AuthorDate {
                author: &author,
                date: &authored
            },
            chinese
        )
    );
    let different_person = details.committer != details.author;
    let different_time = match (
        DateTime::parse_from_rfc3339(&details.authored),
        DateTime::parse_from_rfc3339(&details.committed),
    ) {
        (Ok(a), Ok(c)) => a != c,
        _ => details.authored != details.committed,
    };
    if different_person || different_time {
        let person = different_person.then(|| identity(&details.committer));
        let committed = different_time.then(|| date(&details.committed, chinese));
        metadata.push_str(&format!(
            "<p>{}</p>",
            render_message(
                Message::Committer {
                    person: person.as_deref(),
                    date: committed.as_deref()
                },
                chinese
            )
        ));
    }
    if commit.is_some_and(|c| c.shallow_boundary) {
        metadata.push_str(&format!(
            "<p>{}</p>",
            render_message(Message::ShallowParents, chinese)
        ));
    }
    metadata
}

fn message_line(value: &str) -> String {
    let escaped = escape(value);
    let mut chars = escaped.chars().peekable();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            ' ' if chars.peek() == Some(&' ') => result.push_str("&nbsp;"),
            '\t' => result.push_str("&nbsp;&nbsp;&nbsp;&nbsp;"),
            _ => result.push(ch),
        }
    }
    result
}

fn date(value: &str, chinese: bool) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|time| {
            time.with_timezone(&Local)
                .format(if chinese {
                    "%Y/%m/%d %H:%M"
                } else {
                    "%Y/%m/%d at %H:%M"
                })
                .to_string()
        })
        .unwrap_or_else(|_| escape(value))
}

fn identity(value: &str) -> String {
    let Some((name, email)) = value.strip_suffix('>').and_then(|v| v.rsplit_once(" <")) else {
        return escape(value);
    };
    // Encode the address as data; repository-controlled quotes, delimiters or
    // newlines must not turn into HTML attributes or mailto query parameters.
    let mut address = String::new();
    for byte in email.bytes() {
        if byte.is_ascii_alphanumeric() || b"@._+-".contains(&byte) {
            address.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(&mut address, "%{byte:02X}").unwrap();
        }
    }
    format!(
        "{} <a href=\"mailto:{}\">&lt;{}&gt;</a>",
        escape(name),
        address,
        escape(email)
    )
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn details() -> Details {
        Details {
            message: "Review <cache>\n\nPreserve & explain\n  indentation\n".into(),
            author: "Alex Chen <alex@example.invalid>".into(),
            authored: "2026-10-15T15:57:00+08:00".into(),
            committer: "Alex Chen <alex@example.invalid>".into(),
            committed: "2026-10-15T07:57:00Z".into(),
            base: String::new(),
            files: vec![],
        }
    }
    #[test]
    fn preserves_message_lines_and_avoids_duplicate_metadata_for_equal_instants() {
        let p = build(&details(), None);
        assert!(p.message.contains("Review &lt;cache&gt;"));
        assert!(
            p.message
                .contains("<p>&nbsp;</p><p>Preserve &amp; explain</p><p>&nbsp; indentation")
        );
        assert!(!p.metadata[0].contains("committed"));
        assert!(p.metadata[0].contains("href=\"mailto:alex@example.invalid\""));
    }
    #[test]
    fn localized_metadata_preserves_data_and_handles_committer_variants() {
        let mut d = details();
        let original_message = build(&d, None).message;
        for (person, time) in [(false, false), (true, false), (false, true), (true, true)] {
            d.committer = if person {
                "Other <evil\"?subject=x&body=y@example.invalid>".into()
            } else {
                d.author.clone()
            };
            d.committed = if time {
                "invalid <date>".into()
            } else {
                "2026-10-15T07:57:00Z".into()
            };
            let p = build(&d, None);
            assert_eq!(p.message, original_message);
            let zh = &p.metadata[1];
            assert!(zh.contains("Alex Chen"));
            assert!(!zh.contains(" on "));
            assert!(!zh.contains(" at "));
            assert_eq!(zh.contains("提交"), person || time);
            assert_eq!(zh.contains("由 Other"), person);
            assert_eq!(zh.contains("invalid &lt;date&gt;"), time);
            if person {
                assert!(zh.contains("mailto:evil%22%3Fsubject%3Dx%26body%3Dy@example.invalid"));
            }
            assert!(!zh.contains("href=\"mailto:evil\"?"));
        }
    }
    #[test]
    fn metadata_is_escaped_and_committers_remain_visible() {
        let mut d = details();
        d.committer = "Other <evil\"?subject=x&body=y@example.invalid>".into();
        d.committed = "invalid <date>".into();
        let p = build(&d, None);
        assert!(p.metadata[0].contains("committed by Other"));
        assert!(p.metadata[0].contains("mailto:evil%22%3Fsubject%3Dx%26body%3Dy@example.invalid"));
        assert!(p.metadata[0].contains("invalid &lt;date&gt;"));
        assert!(!p.metadata[0].contains("href=\"mailto:evil\"?"));
    }
}
