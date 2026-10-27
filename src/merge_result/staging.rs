//! Finalize already-resolved drafts without borrowing a dialog's primary root.
use super::*;
use crate::conflicts::project::Key;
use std::{collections::BTreeMap, sync::Arc};

pub struct StagedFile {
    pub key: Key,
    pub repository: Repository,
    pub path: PathBuf,
}
#[derive(Default)]
pub struct StageResult {
    pub completed: Vec<StagedFile>,
    pub snapshots: BTreeMap<PathBuf, Snapshot>,
    /// Includes the failing root: Git may have changed its index before failing.
    pub touched: BTreeMap<PathBuf, Repository>,
    pub error: Option<String>,
}
/// Caller supplies only fully resolved drafts. Stop on the first failure and
/// retain completed outcomes; cancellation is not an index rollback.
pub fn stage_resolved(drafts: &[Arc<Prepared>], token: &Cancel) -> StageResult {
    stage_resolved_with(drafts, token, |draft, repository| {
        draft.stage(repository, token, &Progress::default())
    })
}

pub(crate) fn stage_resolved_with(
    drafts: &[Arc<Prepared>],
    token: &Cancel,
    mut stage: impl FnMut(&Prepared, &Repository) -> Result<conflicts::ResolutionResult, String>,
) -> StageResult {
    let mut result = StageResult::default();
    for draft in drafts {
        if token.cancelled() {
            result.error = Some("Conflict resolution interrupted".into());
            break;
        }
        let repository = draft.snapshot().repository();
        let path = draft.file().path.clone();
        result
            .touched
            .insert(repository.path.clone(), repository.clone());
        let staged = stage(draft, &repository).and_then(|report| checked_report(report, &path));
        match staged {
            Ok(snapshot) => {
                result.completed.push(StagedFile {
                    key: Key::new(&repository, &path),
                    repository: repository.clone(),
                    path,
                });
                result.snapshots.insert(repository.path, snapshot);
            }
            Err(error) => {
                draft.retain_output();
                result.error = Some(error);
                break;
            }
        }
    }
    result
}

pub fn checked_report(
    report: conflicts::ResolutionResult,
    path: &std::path::Path,
) -> Result<Snapshot, String> {
    for command in report.commands {
        let output = command?;
        if !output.status.success() || output.stopped.is_some() || output.input_error.is_some() {
            let detail = String::from_utf8_lossy(&output.stderr.bytes())
                .trim()
                .to_owned();
            return Err(if detail.is_empty() {
                "Conflict resolution interrupted".into()
            } else {
                detail
            });
        }
    }
    let snapshot = report.snapshot?;
    if snapshot.files.iter().any(|file| file.path == path) {
        return Err("The file is still unresolved".into());
    }
    Ok(snapshot)
}
