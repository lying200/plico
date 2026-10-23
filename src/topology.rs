//! Bounded streaming topology for filtered Log queries. No commit messages.
use crate::git::{OUTPUT_LIMIT, TopologyCommit, is_oid};
use std::io::{self, BufRead, BufReader, Read};

pub(crate) const COMMIT_LIMIT: usize = 100_000;
const EDGE_LIMIT: usize = 500_000;
const BUDGET_ERROR: &str = "Filtered graph exceeds the temporary 100,000-commit / 500,000-edge budget; no partial history is shown.";

pub(crate) fn read(input: impl Read) -> io::Result<Vec<TopologyCommit>> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    let mut reader = BufReader::new(input);
    let mut line = Vec::new();
    let mut commits = Vec::new();
    let mut edges = 0usize;
    loop {
        line.clear();
        let size = (&mut reader)
            .take((OUTPUT_LIMIT + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if size == 0 {
            break;
        }
        if size > OUTPUT_LIMIT {
            return Err(invalid("Graph record exceeds 8 MiB"));
        }
        if commits.len() == COMMIT_LIMIT {
            return Err(invalid(BUDGET_ERROR));
        }
        let text = std::str::from_utf8(&line).map_err(|_| invalid("Invalid graph encoding"))?;
        let mut fields = text.split_whitespace();
        let id = fields
            .next()
            .ok_or_else(|| invalid("Missing graph commit ID"))?;
        if !is_oid(id) {
            return Err(invalid("Invalid graph commit ID"));
        }
        let mut parents = Vec::new();
        for parent in fields {
            if !is_oid(parent) {
                return Err(invalid("Invalid graph parent ID"));
            }
            edges += 1;
            if edges > EDGE_LIMIT {
                return Err(invalid(BUDGET_ERROR));
            }
            parents.push(parent.to_owned());
        }
        commits.push(TopologyCommit {
            id: id.to_owned(),
            parents,
        });
    }
    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commit_budget_accepts_exact_limit_and_rejects_extra_row() {
        // Exceeds the general 8 MiB command-output cap, but stays within the
        // explicit graph budgets. Streaming must not require a whole buffer.
        let row = format!("{} {}\n", "a".repeat(64), "b".repeat(64));
        let input = row.repeat(COMMIT_LIMIT);
        assert!(input.len() > OUTPUT_LIMIT);
        assert_eq!(read(input.as_bytes()).unwrap().len(), COMMIT_LIMIT);
        let excess = input + &row;
        assert!(
            read(excess.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("no partial history")
        );
    }
    #[test]
    fn edge_budget_rejects_excess_without_returning_a_prefix() {
        let row = format!(
            "{}{}\n",
            "a".repeat(40),
            format!(" {}", "b".repeat(40)).repeat(6)
        );
        let input = row.repeat(EDGE_LIMIT / 6 + 1);
        assert!(
            read(input.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("no partial history")
        );
    }
    #[test]
    fn malformed_identity_and_truncated_io_are_errors() {
        assert!(read(b"not-an-oid\n".as_slice()).is_err());
        assert!(read(format!("{} bad-parent\n", "a".repeat(40)).as_bytes()).is_err());
        struct Failed;
        impl Read for Failed {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "fixture failure",
                ))
            }
        }
        assert!(read(Failed).is_err());
    }
}
