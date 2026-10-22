//! Rebased VcsLogUserResolverBase / VcsUserUtil identity resolution.
//! The caller supplies the complete raw author catalogue, never just visible rows.
use crate::git::Author;
use std::collections::{BTreeSet, HashMap, HashSet};

/// Budgets apply to unique identities, not commit count. No partial catalogue
/// is returned: that would silently omit users from Select… and me resolution.
pub(crate) fn read_catalog(input: impl std::io::Read) -> std::io::Result<Catalog> {
    use std::io::{self, BufRead, BufReader, Read};
    const FIELD_LIMIT: usize = 8192;
    const USER_LIMIT: usize = 16_384;
    const TEXT_LIMIT: usize = 4 * 1024 * 1024;
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    let mut input = BufReader::new(input);
    let mut users = HashSet::new();
    let mut text_bytes = 0;
    let mut pending_name = None;
    let mut field = Vec::new();
    loop {
        field.clear();
        let size = (&mut input)
            .take((FIELD_LIMIT + 1) as u64)
            .read_until(0, &mut field)?;
        if size == 0 {
            if pending_name.is_some() {
                return Err(invalid("Incomplete Git author identity"));
            }
            break;
        }
        if size > FIELD_LIMIT {
            return Err(invalid("Author identity field exceeds 8 KiB"));
        }
        if field.pop() != Some(0) {
            return Err(invalid("Unterminated Git author identity"));
        }
        let value =
            std::str::from_utf8(&field).map_err(|_| invalid("Author identity is not UTF-8"))?;
        if let Some(name) = pending_name.take() {
            let user = Author {
                name,
                email: value.into(),
            };
            if !users.contains(&user) {
                text_bytes += user.name.len() + user.email.len();
                if users.len() == USER_LIMIT || text_bytes > TEXT_LIMIT {
                    return Err(invalid(
                        "Author catalogue exceeds the 16,384 identity / 4 MiB text budget",
                    ));
                }
                users.insert(user);
            }
        } else {
            pending_name = Some(value.to_owned());
        }
    }
    Ok(Catalog::new(users))
}

pub fn email_name(email: &str) -> Option<&str> {
    let at = email.find('@')?;
    (at > 0).then_some(&email[..at])
}

/// Java's default \w and \p{Punct} are ASCII here. Do not fold arbitrary
/// Unicode names or collapse several separators into one.
fn first_last(name: &str) -> Option<(&str, &str)> {
    let bytes = name.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut nonword = bytes.iter().enumerate().filter(|(_, b)| !word(**b));
    let split = match (nonword.next(), nonword.next()) {
        // Java's default \s includes vertical tab; Rust's ASCII whitespace
        // predicate intentionally excludes it.
        (Some((i, b)), None)
            if b.is_ascii_punctuation() || b.is_ascii_whitespace() || *b == 0x0b =>
        {
            Some(i)
        }
        // The first \w+ group is greedy: a final interior underscore acts as
        // the separator when the whole name is otherwise word characters.
        (None, None) => bytes
            .iter()
            .enumerate()
            .rev()
            .find(|(i, b)| **b == b'_' && *i > 0 && *i + 1 < bytes.len())
            .map(|(i, _)| i),
        _ => None,
    };
    split
        .filter(|i| *i > 0 && *i + 1 < bytes.len())
        .map(|i| (&name[..i], &name[i + 1..]))
}

pub fn standard_name(name: &str) -> String {
    if let Some((first, last)) = first_last(name) {
        return format!(
            "{} {}",
            first.to_ascii_lowercase(),
            last.to_ascii_lowercase()
        );
    }
    if name.bytes().all(|b| (b' '..=b'~').contains(&b)) {
        name.to_ascii_lowercase()
    } else {
        name.into()
    }
}

/// UserFilterPopupComponent.collectUsers uses short presentation and only
/// capitalizes the initial of each ASCII name part, preserving remaining case.
fn presentation(user: &Author) -> String {
    let name = if !user.name.is_empty() {
        &user.name
    } else {
        email_name(&user.email).unwrap_or(&user.email)
    };
    if let Some((first, last)) = first_last(name) {
        let capitalize = |part: &str| format!("{}{}", part[..1].to_ascii_uppercase(), &part[1..]);
        format!("{} {}", capitalize(first), capitalize(last))
    } else {
        name.to_owned()
    }
}

/// Each identity is retained once. Name and email-prefix keys store indices,
/// so aliases do not duplicate author strings. No repository-global cache.
pub struct Catalog {
    users: Vec<Author>,
    by_name: HashMap<String, Vec<usize>>,
}

impl Catalog {
    pub fn new(users: impl IntoIterator<Item = Author>) -> Self {
        let mut users: Vec<_> = users
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        users.sort_by(|a, b| (&a.name, &a.email).cmp(&(&b.name, &b.email)));
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, user) in users.iter().enumerate() {
            let name = (!user.name.is_empty()).then(|| standard_name(&user.name));
            let email = email_name(&user.email).map(standard_name);
            if let Some(key) = &name {
                by_name.entry(key.clone()).or_default().push(i);
            }
            if let Some(key) = email.filter(|key| Some(key) != name.as_ref()) {
                by_name.entry(key).or_default().push(i);
            }
        }
        Self { users, by_name }
    }

    pub fn users(&self) -> &[Author] {
        &self.users
    }

    /// Build once for a popup session. Keep raw identities separate: different
    /// emails with the same displayed name are resolved together, not lost.
    pub fn presentations(&self) -> Vec<String> {
        let mut values = self.users.iter().map(presentation).collect::<Vec<_>>();
        // Java TreeSet<String> ordering is UTF-16 code-unit order.
        values.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        values.dedup();
        values
    }

    pub fn resolve_name(&self, name: &str) -> Vec<Author> {
        self.indices(name)
            .iter()
            .map(|&i| self.users[i].clone())
            .collect()
    }

    pub fn resolve_current(&self, current: Option<&Author>) -> Vec<Author> {
        let Some(current) = current else {
            return Vec::new();
        };
        let mut selected: BTreeSet<_> = self.indices(&current.name).iter().copied().collect();
        if let Some(prefix) = email_name(&current.email) {
            let emails: HashSet<_> = selected
                .iter()
                .map(|&i| self.users[i].email.to_lowercase())
                .collect();
            selected.extend(
                self.indices(prefix)
                    .iter()
                    .copied()
                    .filter(|&i| emails.contains(&self.users[i].email.to_lowercase())),
            );
        }
        selected
            .into_iter()
            .map(|i| self.users[i].clone())
            .collect()
    }

    fn indices(&self, name: &str) -> &[usize] {
        self.by_name
            .get(&standard_name(name))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn user(name: &str, email: &str) -> Author {
        Author {
            name: name.into(),
            email: email.into(),
        }
    }

    #[test]
    fn streaming_catalog_deduplicates_without_retaining_history_output() {
        use std::io::Read;
        // More than the generic Git output limit, but just one identity.
        let record = b"John Smith\0john@example.invalid\0";
        struct Repeated<'a> {
            record: &'a [u8],
            offset: usize,
        }
        impl Read for Repeated<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                for byte in out.iter_mut() {
                    *byte = self.record[self.offset % self.record.len()];
                    self.offset += 1;
                }
                Ok(out.len())
            }
        }
        let input = Repeated { record, offset: 0 }.take((record.len() * 300_000) as u64);
        let catalog = read_catalog(input).unwrap();
        assert_eq!(
            catalog.users(),
            &[user("John Smith", "john@example.invalid")]
        );
    }

    #[test]
    fn streaming_catalog_rejects_incomplete_invalid_and_oversized_inputs() {
        for bytes in [b"name\0".as_slice(), b"name\0email", b"\xff\0mail\0"] {
            assert!(read_catalog(bytes).is_err());
        }
        assert!(read_catalog(std::io::repeat(b'x')).is_err());
        let distinct = (0..16_385)
            .map(|i| format!("User{i}\0mail\0"))
            .collect::<String>();
        assert!(read_catalog(distinct.as_bytes()).is_err());
        let large_names = (0..600)
            .map(|i| format!("{i}{}\0mail\0", "x".repeat(7500)))
            .collect::<String>();
        assert!(read_catalog(large_names.as_bytes()).is_err());
        assert!(read_catalog(b"".as_slice()).unwrap().users().is_empty());
    }

    #[test]
    fn presentation_names_match_reference_without_losing_raw_identities() {
        let catalog = Catalog::new([
            user("john.smith", "one@example.invalid"),
            user("John Smith", "two@example.invalid"),
            user("JOHN.SMITH", "three@example.invalid"),
            user("", "login@example.invalid"),
            user("", "unstructured"),
            user("张 三", "zhang@example.invalid"),
            user("a_b_c", "abc@example.invalid"),
            user("\u{10000}", "supp@example.invalid"),
            user("\u{e000}", "bmp@example.invalid"),
        ]);
        assert_eq!(
            catalog.presentations(),
            vec![
                "A_b C",
                "JOHN SMITH",
                "John Smith",
                "login",
                "unstructured",
                "张 三",
                "\u{10000}",
                "\u{e000}"
            ]
        );
        assert_eq!(catalog.resolve_name("John Smith").len(), 3);
        assert_eq!(catalog.users().len(), 9);
    }

    #[test]
    fn standard_names_follow_ascii_two_word_rule() {
        for name in [
            "John Smith",
            "JOHN.SMITH",
            "john-smith",
            "john_smith",
            "John\tSmith",
            "John\nSmith",
            "John\rSmith",
            "John\x0bSmith",
            "John\x0cSmith",
        ] {
            assert_eq!(standard_name(name), "john smith");
        }
        assert_eq!(standard_name("A_B_C"), "a_b c");
        assert_eq!(standard_name("John..Smith"), "john..smith");
        assert_eq!(standard_name("John  Smith"), "john  smith");
        assert_eq!(standard_name("ÉMILIE Smith"), "ÉMILIE Smith");
        assert_eq!(standard_name("张 三"), "张 三");
    }

    #[test]
    fn current_user_keeps_aliases_but_rejects_unrelated_email_prefix() {
        let john = user("John Smith", "john@work.invalid");
        let home = user("john.smith", "john@home.invalid");
        let alias = user("Old Alias", "JOHN@work.invalid");
        let unrelated = user("Another Person", "john@other.invalid");
        let catalog = Catalog::new([
            john.clone(),
            home.clone(),
            alias.clone(),
            unrelated.clone(),
            john.clone(),
        ]);
        assert_eq!(catalog.users().len(), 4);
        let resolved = catalog.resolve_current(Some(&john));
        assert_eq!(resolved.len(), 3);
        for expected in [john, home, alias] {
            assert!(resolved.contains(&expected));
        }
        assert!(!resolved.contains(&unrelated));
        assert_eq!(catalog.resolve_name("john").len(), 4);
        assert_eq!(catalog.resolve_name("JOHN.SMITH").len(), 2);
        assert!(catalog.resolve_current(None).is_empty());
        assert!(
            catalog
                .resolve_current(Some(&user("Unknown", "john@work.invalid")))
                .is_empty()
        );
        assert!(catalog.resolve_name("unknown").is_empty());
    }

    #[test]
    fn empty_identity_parts_and_non_ascii_names_are_preserved() {
        assert_eq!(
            user("", "only@example.invalid").exact_name(),
            "only@example.invalid"
        );
        assert_eq!(user("Name", "").exact_name(), "Name");
        assert_eq!(user("", "").exact_name(), "");
        assert_eq!(email_name("@invalid"), None);
        let catalog = Catalog::new([
            user("", "login@example.invalid"),
            user("张三", "zhang@example.invalid"),
        ]);
        assert_eq!(catalog.resolve_name("LOGIN").len(), 1);
        assert_eq!(catalog.resolve_name("张三").len(), 1);
        assert!(catalog.resolve_name("张 三").is_empty());
    }
}
