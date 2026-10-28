//! Sequential automatic resolution under the existing operation/session owner.
use super::*;

#[derive(Default)]
pub(super) struct Automatic {
    pub progress: Option<(usize, usize)>,
    pub attempted: bool,
    pub counts: std::collections::BTreeMap<Key, (usize, usize)>,
    pub message: Option<String>,
}
impl Automatic {
    pub fn invalidate(&mut self, path: &Key) {
        self.attempted = false;
        self.message = None;
        self.counts.remove(path);
    }
}
impl ConflictList {
    pub(super) fn automatic_message(&self, changed: usize) -> String {
        if changed == 0 {
            return t("No changes were resolved automatically").into();
        }
        let mut files = 0;
        let mut remaining = 0;
        let mut known = true;
        for entry in &self.entries {
            let count = self
                .drafts
                .get(entry.key())
                .map(|d| d.decisions.progress())
                .or_else(|| self.automatic.counts.get(entry.key()).copied());
            match count {
                Some((done, total)) if done == total => {}
                Some((done, total)) => {
                    remaining += total - done;
                    files += 1;
                }
                None => {
                    known = false;
                    files += 1;
                }
            }
        }
        if files == 0 {
            t("All changes have been resolved").into()
        } else {
            plico::i18n::automatic_merge_result(changed, known.then_some(remaining), files)
        }
    }
}
impl Review {
    pub(super) fn resolve_conflicts_automatically(
        &mut self,
        view: Entity<ConflictList>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_conflict_session(&view, cx) || view.read(cx).busy {
            return;
        }
        let files = view
            .read(cx)
            .entries
            .iter()
            .filter(|entry| {
                !view
                    .read(cx)
                    .drafts
                    .get(entry.key())
                    .is_some_and(SavedResolution::resolved)
            })
            .cloned()
            .collect::<Vec<_>>();
        if files.is_empty() {
            return;
        }
        let Some((id, token)) =
            self.begin_conflict_operation(plico::active_operation::Phase::Executing, cx)
        else {
            return;
        };
        let total = files.len();
        view.update(cx, |s, cx| {
            s.busy = true;
            s.error = None;
            s.automatic.message = None;
            s.automatic.progress = Some((1, total));
            cx.notify();
        });
        let access = id.access.clone();
        self.conflict_task = cx.spawn_in(window, async move |this, wx| {
            let mut changed = 0;
            let mut touched = std::collections::BTreeMap::new();
            let mut failure = None;
            for (index, entry) in files.into_iter().enumerate() {
                let snapshot = entry.snapshot().clone();
                let file = entry.file().clone();
                let repo = snapshot.repository();
                if token.cancelled() {
                    break;
                }
                let saved = this
                    .update_in(wx, |s, _, cx| {
                        if !s.owns_conflict_session(&view, cx) {
                            return None;
                        }
                        view.update(cx, |v, cx| {
                            v.automatic.progress = Some((index + 1, total));
                            cx.notify();
                            Some(v.drafts.get(entry.key()).cloned())
                        })
                    })
                    .ok()
                    .flatten();
                let Some(saved) = saved else {
                    break;
                };
                // Binary, gitlink, symlink and modify/delete conflicts do not
                // support the iterative text resolver. They remain in the list.
                if file
                    .stages
                    .iter()
                    .flatten()
                    .any(|v| v.mode == "120000" || v.mode == "160000")
                    || file.side(Side::Yours, snapshot.reversed).is_none()
                    || file.side(Side::Theirs, snapshot.reversed).is_none()
                {
                    continue;
                }
                let repository = repo.clone();
                let snapshot = snapshot.clone();
                let stop = token.clone();
                let path = Key::new(&snapshot.repository(), &file.path);
                let access = access.clone();
                let result = wx
                    .background_spawn(async move {
                        let prepared = match &saved {
                            Some(saved) => saved.prepared.clone(),
                            None => access.prepare(&repository, &snapshot, &file, &stop)?,
                        };
                        let outcome = access.automatic(
                            &prepared,
                            &repository,
                            saved.as_ref().map(|s| &s.decisions),
                            &stop,
                        )?;
                        Ok::<_, String>(outcome.map(|outcome| {
                            let before = saved.as_ref().map_or(0, |s| s.decisions.progress().0);
                            (prepared, outcome, before)
                        }))
                    })
                    .await;
                match result {
                    Ok(Some((prepared, outcome, before))) => {
                        let count = outcome.decisions.progress();
                        if outcome.changed {
                            changed += count.0 - before;
                            touched.insert(repo.path.clone(), repo.clone());
                        }
                        // A cancellation request after a successful write does
                        // not erase that write or its resumable block decisions.
                        let accepted = this
                            .update_in(wx, |s, _, cx| {
                                if !s.owns_conflict_session(&view, cx) {
                                    return false;
                                }
                                view.update(cx, |v, cx| {
                                    v.automatic.counts.insert(path.clone(), count);
                                    if outcome.changed {
                                        v.drafts.insert(
                                            path,
                                            SavedResolution {
                                                prepared,
                                                decisions: outcome.decisions,
                                                reviewed: false,
                                            },
                                        );
                                    }
                                    cx.notify();
                                });
                                true
                            })
                            .unwrap_or(false);
                        if !accepted {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        if !token.cancelled() {
                            failure = Some(error);
                        }
                        break;
                    }
                }
            }
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_conflict_operation(id, cx) else {
                    return;
                };
                if !s.owns_conflict_session(&view, cx) {
                    return;
                }
                view.update(cx, |v, cx| {
                    v.busy = false;
                    v.automatic.progress = None;
                    v.automatic.attempted = !cancelled && failure.is_none();
                    v.automatic.message = Some(if cancelled {
                        t("Automatic resolution cancelled; completed changes have been kept").into()
                    } else {
                        v.automatic_message(changed)
                    });
                    v.error = failure;
                    cx.notify();
                });
                for repo in touched.values() {
                    s.sync_repository_worktree(
                        &repo.path,
                        plico::worktree_events::Batch {
                            all: true,
                            index: true,
                            ..Default::default()
                        },
                        true,
                        w,
                        cx,
                    );
                }
                cx.notify();
            });
        });
    }
}
