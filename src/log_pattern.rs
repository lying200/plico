//! Conservative syntax gate for Rebased's Pattern.compile → literal fallback.
//!
//! Git still owns matching. Rust and Java regex dialects differ, so only shared
//! structural errors establish invalidity; unsupported syntax and parser limits
//! must reach Git unchanged, never silently become a different literal query.
use regex_syntax::ast::{ErrorKind, parse::Parser};

pub(crate) fn use_regex(text: &str) -> bool {
    if !text.contains([
        '(', ')', '[', ']', '{', '}', '.', '*', '?', '+', '^', '$', '\\', '|',
    ]) {
        return false;
    }
    // Bound validation allocation, without imposing a new limit on Git queries.
    if text.len() > 16 * 1024 {
        return true;
    }
    let Err(error) = Parser::new().parse(text) else {
        return true;
    };
    !matches!(
        error.kind(),
        ErrorKind::ClassUnclosed
            | ErrorKind::ClassRangeInvalid
            | ErrorKind::EscapeUnexpectedEof
            | ErrorKind::GroupUnclosed
            | ErrorKind::GroupUnopened
            | ErrorKind::RepetitionCountInvalid
            | ErrorKind::RepetitionCountDecimalEmpty
            | ErrorKind::RepetitionCountUnclosed
            | ErrorKind::RepetitionMissing
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_invalid_syntax_falls_back_to_literal() {
        for text in [
            "[", "(", "a)", "\\", "*", "a{", "a{foo}", "a{2", "a{2,1}", "[z-a]",
        ] {
            assert!(!use_regex(text), "{text:?}");
        }
        assert!(!use_regex("plain text"));
    }

    #[test]
    fn valid_and_java_specific_patterns_are_left_to_git() {
        for text in [
            "a.*b",
            "^(old|recent) ",
            "[a-z]",
            r"\[",
            "(?=marker)",
            "(?<=marker)",
            r"(?<name>a)\k<name>",
            r"\Q[\E",
            "a++",
            "(?U)a",
            "[a-z&&[^b]]",
            r"\p{javaLowerCase}",
            r"\1",
        ] {
            assert!(use_regex(text), "{text:?}");
        }
    }

    #[test]
    fn validation_limits_are_not_syntax_errors() {
        assert!(use_regex(&format!(
            "{}a{}",
            "(".repeat(512),
            ")".repeat(512)
        )));
        assert!(use_regex(&"[".repeat(16 * 1024 + 1)));
    }
}
