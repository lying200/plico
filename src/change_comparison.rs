//! Compare the displayed committed changes with local files (Rebased ShowDiffWithLocalAction).
use crate::{
    diff_target::{Chain, Target},
    git::{Cancel, Repository, Result},
    working_comparison::{Change, Endpoint},
};

pub fn with_local(targets: &[Target], token: &Cancel) -> Result<(Target, Chain)> {
    let root = &targets
        .first()
        .ok_or("No selected changes")?
        .identity()
        .repository;
    let repo = Repository::new(root);
    let project = crate::project_files::Project::new(root).map_err(|e| format!("{e:?}"))?;
    let mut changes = Vec::new();
    for target in targets {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        if &target.identity().repository != root {
            return Err("Select changes in one repository".into());
        }
        let versions = target
            .committed_endpoints()
            .ok_or("Select committed changes")?;
        let mut local = None;
        // Rebased falls back to the other side for renames.
        for version in versions.after.iter().chain(versions.before.iter()) {
            let path = project
                .resolve_missing(&version.path, token)
                .map_err(|e| format!("{e:?}"))?;
            match std::fs::metadata(path) {
                Ok(metadata) if metadata.is_file() => {
                    local = Some(Endpoint::WorkingTree {
                        path: version.path.clone(),
                    });
                    break;
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        let before = versions.after.map(|r| Endpoint::Revision {
            commit: r.revision,
            path: r.path,
        });
        if before.is_some() || local.is_some() {
            changes.push(Change {
                before,
                after: local,
            });
        }
    }
    Chain::working_comparison(repo, changes, 0)
}
