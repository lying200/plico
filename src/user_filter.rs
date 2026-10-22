//! User filter values follow Rebased's MultilinePopupBuilder and
//! VcsLogUserFilterImpl. Draft text is separate from committed selection.
use crate::{
    authors::Catalog,
    git::{Author, Cancel, Repository, Result},
};
use std::collections::HashSet;

/// Rebased TextCompletionProviderBase.getPrefix: start after the most recent
/// pipe/newline before the caret. Spaces are part of this prefix, not trimmed.
/// GPUI adapters must pass UTF-8 byte offsets, not Java's UTF-16 indices.
pub fn completion_range(text: &str, caret: usize) -> Option<std::ops::Range<usize>> {
    let before = text.get(..caret)?;
    let start = before.rfind(['|', '\n']).map_or(0, |i| i + 1);
    Some(start..caret)
}

/// PlainPrefixMatcher uses a case-insensitive substring, not fuzzy initials or
/// a starts-with-only rule. Compare UTF-16 units like StringUtil; do not expand
/// one character into multiple matching characters (ß must not match "ss").
pub fn completion_matches(name: &str, prefix: &str) -> bool {
    fn upper(unit: u16) -> u16 {
        let Some(ch) = char::from_u32(unit as u32) else {
            return unit;
        };
        let mut mapped = ch.to_uppercase();
        let first = mapped.next().unwrap();
        if mapped.next().is_none() && first.len_utf16() == 1 {
            first as u16
        } else {
            unit
        }
    }
    fn lower(unit: u16) -> u16 {
        let Some(ch) = char::from_u32(unit as u32) else {
            return unit;
        };
        // Unicode simple lower mapping for U+0130 is 'i'; Rust's full lower
        // mapping additionally emits combining dot above.
        ch.to_lowercase()
            .next()
            .filter(|c| c.len_utf16() == 1)
            .map_or(unit, |c| c as u16)
    }
    if prefix.is_empty() {
        return true;
    }
    name.char_indices().any(|(offset, _)| {
        let mut tail = name[offset..].encode_utf16();
        prefix.encode_utf16().all(|b| {
            tail.next().is_some_and(|a| {
                let (a_upper, b_upper) = (upper(a), upper(b));
                a == b || a_upper == b_upper || lower(a_upper) == lower(b_upper)
            })
        })
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    values: Vec<String>,
}
impl Selection {
    /// Empty confirmation clears the filter. Commas and spaces inside names
    /// are literal; only pipe and newline delimit values, as in Rebased.
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 64 * 1024 {
            return Err("User filter text exceeds 64 KiB".into());
        }
        let values = text
            .split(['|', '\n'])
            .map(|value| value.trim_matches(|c: char| c <= '\u{20}'))
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if values.len() > 256 {
            return Err("Select at most 256 user names".into());
        }
        Ok(Self { values })
    }
    pub fn values(&self) -> &[String] {
        &self.values
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn draft(&self) -> String {
        self.values.join("\n")
    }
    pub fn resolve(&self, directory: &Directory) -> Option<Vec<Author>> {
        if self.is_empty() {
            return None;
        }
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for value in &self.values {
            let users = if value == "me" {
                directory
                    .catalog
                    .resolve_current(directory.current.as_ref())
            } else {
                directory.catalog.resolve_name(value)
            };
            for user in users {
                if seen.insert(user.clone()) {
                    result.push(user);
                }
            }
        }
        // Some(empty) intentionally remains an active filter with no matches.
        Some(result)
    }
}

/// Owned by one repository request/UI lifetime. No global cache. Always read
/// all Log roots, independently of the active Branch/Text/Date filter.
pub struct Directory {
    pub catalog: Catalog,
    pub current: Option<Author>,
    pub roots: Vec<String>,
}
impl Directory {
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Self> {
        let roots = repo.snapshot(None, 1, None, token)?.tips;
        let current = repo.current_author(token)?;
        let catalog = repo.author_catalog(&roots, token)?;
        Ok(Self {
            catalog,
            current,
            roots,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_matches_middle_text_without_fuzzy_or_expanding_case() {
        assert!(completion_matches("Alice Chen", "ICE ch"));
        assert!(completion_matches("Émilie Smith", "émILIE"));
        assert!(completion_matches("张三", "三"));
        assert!(completion_matches("İpek", "ipe"));
        assert!(completion_matches("ıpek", "IPE"));
        assert!(completion_matches("Σigma", "ςI"));
        assert!(completion_matches("Anything", ""));
        assert!(!completion_matches("Alice Chen", "AC"));
        assert!(!completion_matches("Alice Chen", " Alice"));
        assert!(!completion_matches("Straße", "STRASSE"));
        assert!(!completion_matches("Alice", "Alice Chen"));
        // Java char-based comparison does not fold supplementary case pairs.
        assert!(!completion_matches("\u{10400}", "\u{10428}"));
    }
    #[test]
    fn completion_range_is_caret_local_and_preserves_other_values() {
        let text = "me|张三\n Bob, Jr.|Alice";
        let caret = text.find("Jr.").unwrap() + 2;
        let range = completion_range(text, caret).unwrap();
        assert_eq!(&text[range.clone()], " Bob, Jr");
        let mut changed = text.to_owned();
        changed.replace_range(range, "Bob Smith");
        assert_eq!(changed, "me|张三\nBob Smith.|Alice");
        assert_eq!(completion_range(text, 3), Some(3..3));
        assert_eq!(completion_range(text, 4), None); // inside 张's UTF-8 bytes
        assert_eq!(completion_range(text, text.len() + 1), None);
        assert_eq!(completion_range("", 0), Some(0..0));
        assert_eq!(completion_range("Alice Chen", 5), Some(0..5));
    }
    #[test]
    fn multiline_draft_preserves_names_and_has_explicit_clear() {
        let selection = Selection::parse(" Alice Chen | Bob, Jr.\r\n\tme\n| ").unwrap();
        assert_eq!(selection.values(), &["Alice Chen", "Bob, Jr.", "me"]);
        assert_eq!(Selection::parse(&selection.draft()).unwrap(), selection);
        assert!(Selection::parse(" \n|\r\t").unwrap().is_empty());
        // Java String.trim does not remove non-breaking spaces.
        assert_eq!(
            Selection::parse("\u{a0}Name\u{a0}").unwrap().values(),
            &["\u{a0}Name\u{a0}"]
        );
        assert!(Selection::parse(&"a|".repeat(257)).is_err());
        assert!(Selection::parse(&"a".repeat(65_537)).is_err());
    }
    #[test]
    fn union_me_and_names_deduplicates_without_clearing_unknown_names() {
        let alice = Author {
            name: "Alice Chen".into(),
            email: "alice@example.invalid".into(),
        };
        let bob = Author {
            name: "Bob, Jr.".into(),
            email: "bob@example.invalid".into(),
        };
        let mut directory = Directory {
            catalog: Catalog::new([alice.clone(), bob.clone()]),
            current: Some(alice.clone()),
            roots: vec![],
        };
        let selection = Selection::parse("me|Alice.Chen|Bob, Jr.|me").unwrap();
        assert_eq!(selection.resolve(&directory), Some(vec![alice, bob]));
        assert_eq!(
            Selection::parse("Unknown").unwrap().resolve(&directory),
            Some(vec![])
        );
        assert_eq!(Selection::default().resolve(&directory), None);
        directory.current = None;
        assert_eq!(
            Selection::parse("me").unwrap().resolve(&directory),
            Some(vec![])
        );
    }
}
