//! Bounded metadata batches, preserving caller order rather than Git's ordering.
use crate::git::{Cancel, Commit, Details, Repository, Result};
use std::{collections::HashMap, ffi::OsStr};

impl Repository {
    /// Text only; file changes and branch containment are intentionally separate.
    pub fn commit_metadata_batch(
        &self,
        commits: &[&Commit],
        token: &Cancel,
    ) -> Result<Vec<Details>> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if commits.len() > 50 {
            return Err("Commit metadata exceeds the preview count limit".into());
        }
        let mut expected = HashMap::new();
        for (position, commit) in commits.iter().enumerate() {
            if !crate::git::is_oid(&commit.id)
                || expected
                    .insert(commit.id.to_ascii_lowercase(), position)
                    .is_some()
            {
                return Err("Invalid commit metadata selection".into());
            }
        }
        if commits.is_empty() {
            return Ok(vec![]);
        }
        let input = commits
            .iter()
            .map(|c| format!("{}\n", c.id))
            .collect::<String>();
        let bytes = self.read_with_input(
            &[
                "log",
                "--no-walk=unsorted",
                "--no-patch",
                "--no-show-signature",
                "--encoding=UTF-8",
                "-z",
                "--format=%H%x00%an <%ae>%x00%aI%x00%cn <%ce>%x00%cI%x00%B%n",
                "--stdin",
                "--",
            ]
            .map(OsStr::new),
            input.into_bytes(),
            token,
        )?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "Invalid metadata encoding")?;
        let fields: Vec<_> = text
            .strip_suffix('\0')
            .ok_or("Incomplete commit metadata batch")?
            .split('\0')
            .take(commits.len() * 6 + 1)
            .collect();
        if fields.len() != commits.len() * 6 {
            return Err("Incomplete or ambiguous commit metadata batch".into());
        }
        let mut result: Vec<Option<Details>> = (0..commits.len()).map(|_| None).collect();
        for record in fields.chunks_exact(6) {
            let position = *expected
                .get(record[0])
                .ok_or("Unexpected commit metadata")?;
            if result[position].is_some() {
                return Err("Duplicate commit metadata".into());
            }
            let commit = commits[position];
            result[position] = Some(Details {
                author: record[1].into(),
                authored: record[2].into(),
                committer: record[3].into(),
                committed: record[4].into(),
                message: record[5].into(),
                base: if commit.shallow_boundary {
                    "Unavailable at shallow history boundary".into()
                } else {
                    commit
                        .parents
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Empty tree".into())
                },
                files: vec![],
            });
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        result
            .into_iter()
            .map(|value| value.ok_or_else(|| "Missing commit metadata".into()))
            .collect()
    }
}
