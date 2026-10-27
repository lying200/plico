// Behavior follows GitInteractiveRebaseFile.java and GitRebaseEntry.kt.
// Reference and deliberate validation differences: docs/REBASED-REFERENCE.md.
//! Bounded parsing of Git's sequence-editor document; no commands are executed.
use crate::git::{HISTORY_MAX, OUTPUT_LIMIT};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Pick,
    Edit,
    Reword,
    Drop,
    Squash,
    Fixup { override_message: bool },
    UpdateRef,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub command: Command,
    /// Commit abbreviation, or the full reference name for update-ref.
    pub target: String,
    pub subject: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Document {
    Noop,
    Entries(Vec<Entry>),
}

/// Unsupported commands require the ordinary text editor, as in Rebased.
/// Malformed input remains an error; callers must not infer fallback from text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Invalid(String),
    Unsupported(String),
}
impl From<&str> for ParseError {
    fn from(message: &str) -> Self {
        Self::Invalid(message.into())
    }
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::Unsupported(command) => {
                write!(f, "Rebase action requires the text editor: {command}")
            }
        }
    }
}
impl std::error::Error for ParseError {}

/// `comment` must be the concrete marker used by Git, not config's `auto`.
pub fn parse(text: &str, comment: &str) -> Result<Document, ParseError> {
    if text.len() > OUTPUT_LIMIT
        || text.contains('\0')
        || comment.is_empty()
        || comment.chars().any(|c| c.is_whitespace() || c == '\0')
    {
        return Err("Invalid or oversized rebase todo".into());
    }
    let mut entries = Vec::new();
    let mut noop = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(comment) {
            continue;
        }
        let (action, rest) = token(line);
        if action == "noop" {
            if !rest.is_empty() && !rest.starts_with(comment) {
                return Err("Unexpected noop arguments".into());
            }
            noop = true;
            continue;
        }
        let mut command = match action {
            "pick" | "p" => Command::Pick,
            "edit" | "e" => Command::Edit,
            "reword" | "r" => Command::Reword,
            "drop" | "d" => Command::Drop,
            "squash" | "s" => Command::Squash,
            "fixup" | "f" => Command::Fixup {
                override_message: false,
            },
            "update-ref" | "u" => Command::UpdateRef,
            _ => return Err(ParseError::Unsupported(action.into())),
        };
        let (mut target, mut subject) = token(rest);
        if matches!(command, Command::Fixup { .. }) && matches!(target, "-c" | "-C") {
            command = Command::Fixup {
                override_message: true,
            };
            (target, subject) = token(subject);
        }
        if command == Command::UpdateRef {
            if !valid_ref(target) {
                return Err("Invalid update-ref target".into());
            }
        } else if !(4..=64).contains(&target.len())
            || !target.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid commit ID in rebase todo".into());
        }
        if entries.len() == HISTORY_MAX {
            return Err("Rebase todo exceeds history limit".into());
        }
        entries.push(Entry {
            command,
            target: target.into(),
            subject: subject
                .split_once(comment)
                .map_or(subject, |(s, _)| s)
                .trim()
                .into(),
        });
    }
    if entries.is_empty() && noop {
        Ok(Document::Noop)
    } else {
        Ok(Document::Entries(entries))
    }
}
fn token(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    text.find(char::is_whitespace)
        .map_or((text, ""), |i| (&text[..i], text[i..].trim_start()))
}
fn valid_ref(name: &str) -> bool {
    name.starts_with("refs/")
        && !name.ends_with('.')
        && !name.contains("..")
        && !name.contains("@{")
        && !name
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || "~^:?*[\\".contains(c))
        && name
            .split('/')
            .all(|s| !s.is_empty() && !s.starts_with('.') && !s.ends_with(".lock"))
}

/// Only commands and validated targets enter Git syntax. Subjects/messages are
/// display data and may contain newlines; never serialize them as commands.
pub fn render(entries: &[Entry]) -> Result<String, String> {
    if entries.len() > HISTORY_MAX {
        return Err("Rebase todo exceeds history limit".into());
    }
    let mut result = String::new();
    for entry in entries {
        let valid = if entry.command == Command::UpdateRef {
            valid_ref(&entry.target)
        } else {
            (4..=64).contains(&entry.target.len())
                && entry.target.bytes().all(|b| b.is_ascii_hexdigit())
        };
        if !valid {
            return Err("Invalid rebase todo target".into());
        }
        let action = match entry.command {
            Command::Pick => "pick",
            Command::Edit => "edit",
            Command::Reword => "reword",
            Command::Drop => "drop",
            Command::Squash => "squash",
            Command::Fixup {
                override_message: false,
            } => "fixup",
            Command::Fixup {
                override_message: true,
            } => "fixup -C",
            Command::UpdateRef => "update-ref",
        };
        if result
            .len()
            .saturating_add(action.len())
            .saturating_add(entry.target.len())
            .saturating_add(2)
            > OUTPUT_LIMIT
        {
            return Err("Rebase todo exceeds preview limit".into());
        }
        result.push_str(action);
        result.push(' ');
        result.push_str(&entry.target);
        result.push('\n');
    }
    Ok(result)
}

/// Rebased compares actions and commit IDs in order, not display subjects.
/// Reference targets must match exactly; commit targets may be abbreviated.
pub fn matches_prepared(prepared: &[Entry], actual: &[Entry]) -> bool {
    prepared.len() == actual.len()
        && prepared.iter().zip(actual).all(|(a, b)| {
            a.command == b.command
                && if a.command == Command::UpdateRef {
                    a.target == b.target
                } else {
                    (4..=64).contains(&a.target.len())
                        && (4..=64).contains(&b.target.len())
                        && a.target.bytes().all(|c| c.is_ascii_hexdigit())
                        && b.target.bytes().all(|c| c.is_ascii_hexdigit())
                        && (a.target.starts_with(&b.target) || b.target.starts_with(&a.target))
                }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entries(text: &str) -> Vec<Entry> {
        let Document::Entries(entries) = parse(text, "#").unwrap() else {
            panic!("entries expected")
        };
        entries
    }
    #[test]
    fn unsupported_commands_request_fallback_without_accepting_invalid_data() {
        assert_eq!(
            parse("pick abcdef ok\nexec echo hello", "#"),
            Err(ParseError::Unsupported("exec".into()))
        );
        assert!(matches!(
            parse("pick nope", "#"),
            Err(ParseError::Invalid(_))
        ));
        assert!(matches!(
            parse("exec echo hello\0", "#"),
            Err(ParseError::Invalid(_))
        ));
    }
    #[test]
    fn rendered_plan_roundtrips_without_subject_injection() {
        let mut plan =
            entries("p abcdef first\nf -C bcdef0 second\nu refs/heads/topic\nd cdef01 dropped");
        plan[0].subject = "subject\nexec arbitrary command".into();
        let rendered = render(&plan).unwrap();
        assert!(!rendered.contains("exec"));
        assert!(matches_prepared(&plan, &entries(&rendered)));
        plan[0].target = "abcdef\nexec arbitrary command".into();
        assert!(render(&plan).is_err());
        assert!(
            render(&[Entry {
                command: Command::UpdateRef,
                target: "refs/heads/a\nexec x".into(),
                subject: String::new()
            }])
            .is_err()
        );
    }
    #[test]
    fn git_commands_aliases_comments_and_fixup_options() {
        let result = entries(
            "# help\r\np abcdef subject # empty\nf -C bcdef0 amend! subject\nf -c cdef01 amend! body\nu refs/heads/topic\ns def012 squash! subject\nr ef0123 subject\ne f01234 subject\nd 012345 subject\n",
        );
        assert_eq!(result.len(), 8);
        assert_eq!(result[0].subject, "subject");
        assert_eq!(
            result[1].command,
            Command::Fixup {
                override_message: true
            }
        );
        assert_eq!(result[2].command, result[1].command);
        assert_eq!(result[3].target, "refs/heads/topic");
        assert_eq!(parse("noop\n# help", "#").unwrap(), Document::Noop);
        assert_eq!(
            parse(";; help\np abcdef subject ;; empty", ";;").unwrap(),
            Document::Entries(vec![result[0].clone()])
        );
    }
    #[test]
    fn malformed_and_unsupported_commands_are_not_silently_dropped() {
        for text in [
            "exec echo anything",
            "label onto",
            "pick",
            "pick ../file",
            "fixup -C -c abcdef",
            "update-ref refs/heads/../main",
            "update-ref refs/heads/a.lock",
            "noop unexpected",
            "pick abcdef\0",
        ] {
            assert!(parse(text, "#").is_err(), "{text}");
        }
        assert!(parse(&"x".repeat(OUTPUT_LIMIT + 1), "#").is_err());
        assert!(parse(&"pick abcdef\n".repeat(HISTORY_MAX + 1), "#").is_err());
    }
    #[test]
    fn validation_checks_actions_order_and_refs_but_not_subjects() {
        let prepared = entries(
            "pick abcdef123456 original\nupdate-ref refs/heads/topic\npick bcdef1234567 next",
        );
        assert!(matches_prepared(
            &prepared,
            &entries("p abcdef different\nu refs/heads/topic\np bcdef1 next")
        ));
        for changed in [
            "pick bcdef1 next\nupdate-ref refs/heads/topic\npick abcdef original",
            "edit abcdef original\nupdate-ref refs/heads/topic\npick bcdef1 next",
            "pick abcdef original\nupdate-ref refs/heads/other\npick bcdef1 next",
            "pick abcdef original",
        ] {
            assert!(!matches_prepared(&prepared, &entries(changed)));
        }
    }
}
