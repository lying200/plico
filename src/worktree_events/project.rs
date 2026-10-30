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
    configuration: super::configuration::Configuration,
}
#[derive(Default)]
pub struct ProjectEvents {
    roots: BTreeMap<PathBuf, Observed>,
    configuration_cursor: Option<PathBuf>,
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
        if repositories.len() > 4096 {
            return Err("Project repository count exceeds the supported size".into());
        }
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
                    configuration: Default::default(),
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
                batch.configuration |=
                    observed.configuration.pending() && !observed.configuration.diff_only();
                batch.diff_configuration |= observed.configuration.diff_only();
                batch.status_rules |= observed.configuration.status_rules_pending();
                (batch.changed() || poll_documents).then(|| Notice {
                    root: root.clone(),
                    batch,
                    poll_documents,
                })
            })
            .collect()
    }
    pub fn configuration_due(&self, notices: &[Notice], now: Instant) -> bool {
        self.roots.values().any(|root| root.configuration.due(now))
            || notices.iter().any(|notice| {
                notice.batch.configuration
                    && self
                        .roots
                        .get(&notice.root)
                        .is_some_and(|root| !root.configuration.pending())
            })
    }
    /// Off-thread, at most one eligible root per call. Round-robin roots so
    /// a continuously dirty root cannot starve another root's periodic check.
    /// Queued reads and unacknowledged deliveries have independent lifetimes.
    pub fn poll_configuration(
        &mut self,
        notices: &mut Vec<Notice>,
        now: Instant,
        token: &Cancel,
    ) -> Result<(), String> {
        for notice in notices.iter() {
            if notice.batch.configuration {
                if let Some(observed) = self.roots.get_mut(&notice.root) {
                    if !observed.configuration.pending() {
                        observed.configuration.request();
                    }
                }
            }
        }
        let eligible = |root: &PathBuf, observed: &Observed| {
            observed.configuration.due(now)
                && self
                    .configuration_cursor
                    .as_ref()
                    .is_none_or(|cursor| root > cursor)
        };
        let next = self
            .roots
            .iter()
            .find(|(root, observed)| eligible(root, observed))
            .or_else(|| {
                self.roots
                    .iter()
                    .find(|(_, observed)| observed.configuration.due(now))
            })
            .map(|(root, _)| root.clone());
        if let Some(root) = next {
            self.roots.get_mut(&root).unwrap().configuration.poll(
                &Repository::new(&root),
                token,
                now,
            )?;
            self.configuration_cursor = Some(root);
        } else if token.cancelled() {
            return Err("Cancelled".into());
        }
        for (root, observed) in &self.roots {
            let notice = notices.iter().position(|notice| &notice.root == root);
            if let Some(i) = notice {
                notices[i].batch.configuration =
                    observed.configuration.pending() && !observed.configuration.diff_only();
                notices[i].batch.diff_configuration = observed.configuration.diff_only();
                notices[i].batch.status_rules |= observed.configuration.status_rules_pending();
            } else if observed.configuration.pending()
                || observed.configuration.status_rules_pending()
            {
                notices.push(Notice {
                    root: root.clone(),
                    batch: Batch {
                        configuration: observed.configuration.pending()
                            && !observed.configuration.diff_only(),
                        diff_configuration: observed.configuration.diff_only(),
                        status_rules: observed.configuration.status_rules_pending(),
                        ..Default::default()
                    },
                    poll_documents: false,
                });
            }
        }
        Ok(())
    }
    pub fn acknowledge_configuration(&mut self, roots: &[PathBuf]) {
        for root in roots {
            if let Some(observed) = self.roots.get_mut(root) {
                observed.configuration.acknowledge();
                observed.configuration.acknowledge_status_rules();
            }
        }
    }
    pub fn acknowledge_status_rules(&mut self, roots: &[PathBuf]) {
        for root in roots {
            if let Some(observed) = self.roots.get_mut(root) {
                observed.configuration.acknowledge_status_rules();
            }
        }
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
    fn ignore_content_delivery_does_not_reload_history_or_disappear_before_acknowledgement() {
        use crate::worktree_events::configuration::tests::Fixture;
        use std::{fs, time::Duration};
        let fixture = Fixture::new();
        fixture.git(&["config", "core.excludesFile", "rules"]);
        let now = Instant::now();
        let token = Cancel::default();
        let mut project = ProjectEvents::default();
        project.roots.insert(
            fixture.0.clone(),
            Observed {
                events: None,
                schedule: Schedule::new(now, false),
                configuration: Default::default(),
            },
        );
        let mut initial = project.take(now);
        project
            .poll_configuration(&mut initial, now, &token)
            .unwrap();
        project.acknowledge_configuration(std::slice::from_ref(&fixture.0));
        fs::write(fixture.0.join("rules"), "hidden\n").unwrap();
        let later = now + Duration::from_secs(31);
        let mut notices = Vec::new();
        project
            .poll_configuration(&mut notices, later, &token)
            .unwrap();
        assert_eq!(notices.len(), 1);
        assert!(notices[0].batch.status_rules);
        assert!(!notices[0].batch.history_changed());
        let retry = project.take(later);
        assert!(retry[0].batch.status_rules && !retry[0].batch.history_changed());
        assert!(!project.configuration_due(&retry, later));
        project.acknowledge_status_rules(std::slice::from_ref(&fixture.0));
        assert!(project.take(later).is_empty());
    }
    #[test]
    fn configuration_delivery_is_per_root_and_survives_rejection_and_cancellation() {
        use crate::worktree_events::configuration::tests::Fixture;
        use std::{fs, time::Duration};
        let a = Fixture::new();
        let b = Fixture::new();
        let token = Cancel::default();
        let now = Instant::now();
        let mut project = ProjectEvents::default();
        // Deterministic delivery test: no OS event timing, real Git reads.
        for root in [&a.0, &b.0] {
            project.roots.insert(
                root.clone(),
                Observed {
                    events: None,
                    schedule: Schedule::new(now, false),
                    configuration: Default::default(),
                },
            );
        }
        let mut notices = project.take(now);
        project
            .poll_configuration(&mut notices, now, &token)
            .unwrap();
        assert_eq!(notices.iter().filter(|n| n.batch.configuration).count(), 1);
        project
            .poll_configuration(&mut notices, now, &token)
            .unwrap();
        assert_eq!(notices.iter().filter(|n| n.batch.configuration).count(), 2);
        project.acknowledge_configuration(std::slice::from_ref(&a.0));
        let retried = project.take(now + Duration::from_millis(100));
        assert_eq!(retried.len(), 1);
        assert_eq!(retried[0].root, b.0);
        assert!(retried[0].batch.configuration);
        assert!(
            !project.configuration_due(&retried, now + Duration::from_millis(100)),
            "retry delivery directly; an already sampled invalidation needs no background query task"
        );
        project.acknowledge_configuration(std::slice::from_ref(&b.0));
        fs::write(a.0.join(".git/observed-include"), "[diff]\nrenameLimit=7\n").unwrap();
        let later = now + Duration::from_secs(30);
        let cancelled = Cancel::default();
        cancelled.cancel();
        assert!(
            project
                .poll_configuration(&mut Vec::new(), later, &cancelled)
                .is_err()
        );
        let mut notices = Vec::new();
        project
            .poll_configuration(&mut notices, later, &token)
            .unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].root, a.0);
        // A read is not delivery: another poll must retain the invalidation.
        let mut retry = Vec::new();
        project
            .poll_configuration(&mut retry, later, &token)
            .unwrap();
        assert_eq!(retry.len(), 1);
        assert_eq!(retry[0].root, a.0);
        // Unregistered roots must release pending state as well as watchers.
        project.reconcile(&[Repository::new(&b.0)], &token).unwrap();
        assert!(!project.roots.contains_key(&a.0));
        assert!(
            project
                .take(later + Duration::from_millis(100))
                .iter()
                .all(|n| !n.batch.configuration)
        );
    }
    #[test]
    fn configuration_round_robin_retains_deferred_events_and_does_not_starve_roots() {
        use crate::worktree_events::configuration::tests::Fixture;
        let fixtures = [Fixture::new(), Fixture::new(), Fixture::new()];
        let token = Cancel::default();
        let now = Instant::now();
        let mut project = ProjectEvents::default();
        for fixture in &fixtures {
            project.roots.insert(
                fixture.0.clone(),
                Observed {
                    events: None,
                    schedule: Schedule::new(now, false),
                    configuration: Default::default(),
                },
            );
        }
        let paths: Vec<_> = project.roots.keys().cloned().collect();
        let mut notices = Vec::new();
        project
            .poll_configuration(&mut notices, now, &token)
            .unwrap();
        assert_eq!(project.configuration_cursor.as_ref(), Some(&paths[0]));
        project.acknowledge_configuration(&paths);
        // Root zero keeps changing, but roots one and two must still be read.
        for expected in &paths[1..] {
            let mut notices = vec![Notice {
                root: paths[0].clone(),
                batch: Batch {
                    configuration: true,
                    ..Default::default()
                },
                poll_documents: false,
            }];
            project
                .poll_configuration(&mut notices, now, &token)
                .unwrap();
            assert_eq!(project.configuration_cursor.as_ref(), Some(expected));
            assert!(project.roots[&paths[0]].configuration.due(now));
            project.acknowledge_configuration(&paths);
        }
        // No event need be sent again: the deferred dirty root remains queued.
        assert!(project.configuration_due(&[], now));
        project
            .poll_configuration(&mut Vec::new(), now, &token)
            .unwrap();
        assert_eq!(project.configuration_cursor.as_ref(), Some(&paths[0]));
        assert!(!project.configuration_due(&[], now));
        // Removing the cursor root cannot stop the remaining roots' next cycle.
        project
            .reconcile(
                &[Repository::new(&paths[1]), Repository::new(&paths[2])],
                &token,
            )
            .unwrap();
        project
            .poll_configuration(
                &mut Vec::new(),
                now + std::time::Duration::from_secs(31),
                &token,
            )
            .unwrap();
        assert_eq!(project.configuration_cursor.as_ref(), Some(&paths[1]));
    }
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
