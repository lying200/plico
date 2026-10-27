//! Rebased 4aec352 GitOperationActionBase: eligibility comes from every project
//! repository's current state, not from the last command launched by this window.
use crate::{
    git::{Cancel, Repository},
    operation_state::{self, Observation, State},
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub struct Entry {
    pub repository: Repository,
    /// An unreadable repository is unknown, never silently treated as idle.
    pub state: Result<Arc<State>, String>,
}
#[derive(Clone)]
struct Cached {
    observation: Option<Observation>,
    entry: Entry,
}
#[derive(Default)]
pub struct Observer {
    cached: BTreeMap<String, Cached>,
    published: Option<Arc<Vec<Entry>>>,
}
impl Observer {
    /// None means the published state is unchanged. Cancellation never publishes
    /// a partial project or acknowledges observations that were not delivered.
    pub fn poll(
        &mut self,
        roots: &[Repository],
        token: &Cancel,
    ) -> Result<Option<Arc<Vec<Entry>>>, String> {
        if roots.len() > 4096 {
            return Err("Project repository count exceeds the supported size".into());
        }
        let mut next = BTreeMap::new();
        let mut entries = Vec::new();
        for repository in roots {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let key = crate::recent_projects::identity(&repository.path);
            if next.contains_key(&key) {
                continue;
            }
            let old = self.cached.get(&key);
            let value = match operation_state::observe(
                repository,
                old.and_then(|c| c.observation.as_ref()),
                token,
            ) {
                Ok(Some((observation, state))) => Cached {
                    observation: Some(observation),
                    entry: Entry {
                        repository: repository.clone(),
                        state: Ok(Arc::new(state)),
                    },
                },
                Ok(None) => old
                    .expect("unchanged observation requires a cached value")
                    .clone(),
                Err(error) => Cached {
                    observation: None,
                    entry: Entry {
                        repository: repository.clone(),
                        state: Err(error),
                    },
                },
            };
            entries.push(value.entry.clone());
            next.insert(key, value);
        }
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let unchanged = self.published.as_ref().is_some_and(|previous| {
            previous.len() == entries.len()
                && previous
                    .iter()
                    .zip(&entries)
                    .all(|(a, b)| a.repository.path == b.repository.path && a.state == b.state)
        });
        self.cached = next;
        if unchanged {
            return Ok(None);
        }
        let entries = Arc::new(entries);
        self.published = Some(entries.clone());
        Ok(Some(entries))
    }
}
