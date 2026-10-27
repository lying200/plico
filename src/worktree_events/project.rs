//! One bounded observation lifetime per registered project repository.
use super::{Batch, Events, Schedule};
use crate::git::{Cancel, Repository};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Instant,
};

struct Observed {
    events: Option<Events>,
    schedule: Schedule,
}
#[derive(Default)]
pub struct ProjectEvents {
    roots: BTreeMap<PathBuf, Observed>,
}
pub struct Notice {
    pub root: PathBuf,
    pub batch: Batch,
    pub poll_documents: bool,
}
impl ProjectEvents {
    /// Run off the UI thread. Retain unchanged watchers, release removed ones,
    /// and use polling for roots whose watcher could not be installed.
    pub fn reconcile(&mut self, repositories: &[Repository], token: &Cancel) -> Result<(), String> {
        let wanted: BTreeSet<_> = repositories.iter().map(|repo| repo.path.clone()).collect();
        self.roots.retain(|root, _| wanted.contains(root));
        for root in wanted {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            if self.roots.contains_key(&root) {
                continue;
            }
            let events = Events::for_repository(&Repository::new(&root), token).ok();
            let fallback = events.is_none()
                || is_network_path(&root.to_string_lossy())
                || root.join(".git").is_file();
            self.roots.insert(
                root,
                Observed {
                    events,
                    schedule: Schedule::new(Instant::now(), fallback),
                },
            );
        }
        Ok(())
    }
    pub fn take(&mut self, now: Instant) -> Vec<Notice> {
        self.roots
            .iter_mut()
            .filter_map(|(root, observed)| {
                let mut batch = observed
                    .events
                    .as_ref()
                    .map(Events::take)
                    .unwrap_or_default();
                let poll_documents = observed.schedule.advance(&mut batch, now);
                (batch.changed() || poll_documents).then(|| Notice {
                    root: root.clone(),
                    batch,
                    poll_documents,
                })
            })
            .collect()
    }
}

// Git may return a canonical Windows path with a verbatim prefix. That
// prefix is not itself a network share: local roots keep native watching.
fn is_network_path(path: &str) -> bool {
    let path = path.replace('\\', "/");
    if let Some(rest) = path.strip_prefix("//?/") {
        return rest
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC/"));
    }
    path.starts_with("//") && !path.starts_with("//./")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_verbatim_paths_are_not_network_shares() {
        assert!(!is_network_path(r"\\?\C:\work\child"));
        assert!(!is_network_path("C:/work/child"));
        assert!(!is_network_path(r"\\?\Volume{volume-id}\child"));
        for path in [
            r"\\server\share\repo",
            r"\\wsl.localhost\Ubuntu\home\repo",
            r"\\?\UNC\server\share\repo",
            "//server/share/repo",
            "//?/UNC/server/share/repo",
        ] {
            assert!(is_network_path(path), "{path}");
        }
    }
}
