//! Rebased GitShowDiffWithBranchPanel: explicit revision/worktree endpoints.
//! Metadata is bounded; file contents are read only when a diff is opened.
use crate::git::{Cancel, Repository, Result, is_oid};
use std::{ffi::OsStr, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Endpoint {
    Revision { commit: String, path: PathBuf },
    WorkingTree { path: PathBuf },
}
impl Endpoint {
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::Revision { path, .. } | Self::WorkingTree { path } => path,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Change {
    pub before: Option<Endpoint>,
    pub after: Option<Endpoint>,
}
/// Display orientation never changes the disk identity or save precondition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkingSide {
    Before,
    After,
}
pub struct EditableContents {
    pub path: PathBuf,
    pub side: WorkingSide,
    pub contents: crate::working_copy::Contents,
}
impl Change {
    pub fn reversed(&self) -> Self {
        Self {
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
    pub fn status(&self) -> &'static str {
        match (&self.before, &self.after) {
            (None, _) => "A",
            (_, None) => "D",
            (Some(a), Some(b)) if a.path() != b.path() => "R",
            _ => "M",
        }
    }
    fn editable_endpoints(&self) -> Result<(&std::path::Path, WorkingSide, Option<&Endpoint>)> {
        match (&self.before, &self.after) {
            (Some(Endpoint::WorkingTree { path }), None | Some(Endpoint::Revision { .. })) => {
                Ok((path, WorkingSide::Before, self.after.as_ref()))
            }
            (None | Some(Endpoint::Revision { .. }), Some(Endpoint::WorkingTree { path })) => {
                Ok((path, WorkingSide::After, self.before.as_ref()))
            }
            _ => Err("Comparison must contain exactly one working-tree endpoint".into()),
        }
    }
    pub fn working_path(&self) -> Option<&std::path::Path> {
        self.editable_endpoints().ok().map(|(path, _, _)| path)
    }
    /// Fixed comparison baseline; index/HEAD refreshes must never replace it.
    pub fn read_baseline(&self, repo: &Repository, token: &Cancel) -> Result<String> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let (_, _, baseline) = self.editable_endpoints()?;
        let text = baseline
            .map(|e| read_endpoint_text(e, repo, token))
            .transpose()?
            .unwrap_or_default();
        Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
    }
    pub fn read_editable(&self, repo: &Repository, token: &Cancel) -> Result<EditableContents> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let (path, side, _) = self.editable_endpoints()?;
        // Keep raw disk bytes as the save baseline, including BOM and line endings.
        let bytes = crate::project_files::Project::new(&repo.path)
            .map_err(|e| format!("{e:?}"))?
            .read_bytes(path, token)
            .map_err(|e| format!("{e:?}"))?;
        let working = crate::file_document::Document::new(bytes).map_err(|e| format!("{e:?}"))?;
        let original = self.read_baseline(repo, token)?;
        if original.len().saturating_add(working.disk().len()) > crate::git::OUTPUT_LIMIT {
            return Err("Comparison exceeds the preview output limit".into());
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(EditableContents {
            path: path.to_path_buf(),
            side,
            contents: crate::working_copy::Contents { original, working },
        })
    }
    pub fn read(
        &self,
        repo: &Repository,
        token: &Cancel,
    ) -> Result<crate::combined_changes::Contents> {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if self.before.is_none() && self.after.is_none() {
            return Err("Change has no endpoints".into());
        }
        let before = self
            .before
            .as_ref()
            .map(|e| read_endpoint_text(e, repo, token))
            .transpose()?;
        let after = self
            .after
            .as_ref()
            .map(|e| read_endpoint_text(e, repo, token))
            .transpose()?;
        if before
            .as_ref()
            .map_or(0, String::len)
            .saturating_add(after.as_ref().map_or(0, String::len))
            > crate::git::OUTPUT_LIMIT
        {
            return Err("Comparison exceeds the preview output limit".into());
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(crate::combined_changes::Contents { before, after })
    }
}

/// Default direction matches Rebased: current working tree on the left.
/// Callers resolve the branch to a full commit ID once, before this query.
pub fn load(repo: &Repository, revision: &str, token: &Cancel) -> Result<Vec<Change>> {
    if !is_oid(revision) {
        return Err("Comparison requires a full commit ID".into());
    }
    let bytes = repo.read(
        &[
            OsStr::new("diff"),
            OsStr::new("--name-status"),
            OsStr::new("-z"),
            OsStr::new("--no-ext-diff"),
            OsStr::new("--no-textconv"),
            OsStr::new("--find-renames"),
            OsStr::new("--ignore-submodules=none"),
            OsStr::new(revision),
            OsStr::new("--"),
        ],
        token,
    )?;
    let changes = crate::git::parse_names(&bytes)?;
    if changes.len() > crate::combined_changes::MAX_INPUT_CHANGES {
        return Err("Comparison exceeds the file limit".into());
    }
    if token.cancelled() {
        return Err("Cancelled".into());
    }
    changes
        .into_iter()
        .map(|c| {
            if c.status.starts_with('U') {
                return Err("Resolve conflicts before reviewing this change".into());
            }
            let old = c.old_path.unwrap_or_else(|| c.path.clone());
            Ok(Change {
                before: (!c.status.starts_with('D'))
                    .then_some(Endpoint::WorkingTree { path: c.path }),
                after: (!c.status.starts_with('A')).then_some(Endpoint::Revision {
                    commit: revision.into(),
                    path: old,
                }),
            })
        })
        .collect()
}

fn read_endpoint_text(endpoint: &Endpoint, repo: &Repository, token: &Cancel) -> Result<String> {
    let path = endpoint.path();
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Invalid comparison path".into());
    }
    let bytes = match endpoint {
        Endpoint::Revision { commit, .. } => {
            if !is_oid(commit) {
                return Err("Invalid comparison revision".into());
            }
            let mut spec = std::ffi::OsString::from(commit);
            spec.push(":");
            spec.push(path);
            repo.read(&[OsStr::new("cat-file"), OsStr::new("blob"), &spec], token)?
        }
        Endpoint::WorkingTree { .. } => crate::project_files::Project::new(&repo.path)
            .map_err(|e| format!("{e:?}"))?
            .read_bytes(path, token)
            .map_err(|e| format!("{e:?}"))?,
    };
    if bytes.contains(&0) {
        return Err("Binary file — text preview unavailable".into());
    }
    String::from_utf8(bytes).map_err(|_| "Non-UTF-8 text cannot be displayed safely".into())
}
