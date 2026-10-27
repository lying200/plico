//! Project observation owns per-root listeners; documents own reload safety.
use super::*;
use plico::worktree_events::{Batch, ProjectEvents};
use std::time::{Duration, Instant};

impl Review {
    pub(super) fn start_worktree_observation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let root = self.repo.path.clone();
        let token = self.updates.token.clone();
        self.updates.worktree = cx.spawn_in(window, async move |this, wx| {
            let mut events = ProjectEvents::default();
            let mut watched: Option<Arc<Vec<Repository>>> = None;
            let mut operations = plico::project_operations::Observer::default();
            let mut next_operations = Instant::now();
            loop {
                if token.cancelled() {
                    break;
                }
                let ready = this
                    .read_with(wx, |s, cx| {
                        (!s.welcome
                            && !s.operation.busy()
                            && !s.documents.locked()
                            && s.repo.path == root)
                            .then(|| s.project_roots.read(cx).observation_roots())
                    })
                    .ok()
                    .flatten();
                if let Some(roots) = ready {
                    let roots_changed =
                        !watched.as_ref().is_some_and(|old| Arc::ptr_eq(old, &roots));
                    if roots_changed {
                        let repositories = roots.clone();
                        let query_token = token.clone();
                        let (next, result) = wx
                            .background_spawn(async move {
                                let result = events.reconcile(&repositories, &query_token);
                                (events, result)
                            })
                            .await;
                        events = next;
                        if result.is_err() || token.cancelled() {
                            break;
                        }
                        watched = Some(roots.clone());
                    }
                    let notices = events.take(Instant::now());
                    let poll_operations = roots_changed
                        || Instant::now() >= next_operations
                        || notices
                            .iter()
                            .any(|n| n.batch.all || n.batch.index || n.batch.refs);
                    if !notices.is_empty() {
                        if this
                            .update_in(wx, |s, window, cx| {
                                if token.cancelled() || s.documents.locked() || s.operation.busy() {
                                    return;
                                }
                                let primary = plico::recent_projects::identity(&s.repo.path);
                                let refresh_shallow =
                                    notices.iter().any(|notice| notice.batch.shallow);
                                let refresh_project_log = s.project_window.is_some()
                                    && !s.local
                                    && notices.iter().any(|notice| {
                                        notice.batch.history_changed()
                                            && plico::recent_projects::identity(&notice.root)
                                                != primary
                                    });
                                for notice in notices {
                                    s.sync_repository_worktree(
                                        &notice.root,
                                        notice.batch,
                                        notice.poll_documents,
                                        window,
                                        cx,
                                    );
                                }
                                if refresh_shallow {
                                    // The primary ref poll cannot detect unchanged tips
                                    // with new ancestry. Refresh the catalog, rows and
                                    // count together, once for all affected roots.
                                    s.refresh_impl(false, true, None, cx);
                                } else if refresh_project_log {
                                    // One refresh for this whole batch, including events
                                    // arriving while an older Log query was in flight.
                                    s.reload_project_history(false, true, None, cx);
                                }
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    if poll_operations {
                        let repositories = roots.clone();
                        let query_token = token.clone();
                        let (observer, result) = wx
                            .background_spawn(async move {
                                let result = operations.poll(&repositories, &query_token);
                                (operations, result)
                            })
                            .await;
                        operations = observer;
                        if token.cancelled() {
                            break;
                        }
                        next_operations = Instant::now() + Duration::from_secs(5);
                        if let Ok(Some(entries)) = result {
                            let accepted = this
                                .update_in(wx, |s, _, cx| {
                                    if s.repo.path != root
                                        || s.operation.busy()
                                        || s.documents.locked()
                                    {
                                        return false;
                                    }
                                    s.project_roots.update(cx, |project, cx| {
                                        project.publish_operations(&roots, entries, cx)
                                    })
                                })
                                .unwrap_or(false);
                            if !accepted {
                                // Retry delivery as well as reading: an unchanged
                                // observer result is not proof the UI received it.
                                operations = plico::project_operations::Observer::default();
                                next_operations = Instant::now();
                            }
                        }
                    }
                }
                wx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }
        });
    }
    pub(super) fn sync_worktree(
        &mut self,
        batch: Batch,
        poll_open_documents: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_repository_worktree(
            &self.repo.path.clone(),
            batch,
            poll_open_documents,
            window,
            cx,
        );
    }
    pub(super) fn sync_repository_worktree(
        &mut self,
        root: &std::path::Path,
        batch: Batch,
        poll_open_documents: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.documents
            .observe_disk(root, &batch, poll_open_documents, window, cx);
        let primary = plico::recent_projects::identity(root)
            == plico::recent_projects::identity(&self.repo.path);
        if !batch.changed() {
            return;
        }
        if self
            .project_roots
            .read(cx)
            .affected_by_worktree(root, &batch)
        {
            self.project_roots.update(cx, |roots, cx| roots.refresh(cx));
        }
        if batch.history_changed() && self.repository_menu_open {
            self.dismiss_repository_child(cx);
            self.load_repository_recents(cx);
        }
        // Primary references already flow through the existing auto-refresh
        // controller. Child roots need their own notification even before the
        // aggregate project Log is enabled.
        if !primary && batch.history_changed() {
            for view in &self.branch_compare_tabs {
                view.update(cx, |view, cx| view.repository_changed(root, &batch, cx));
            }
        }
        if let Some(view) = &self.comparison {
            view.update(cx, |view, cx| view.repository_changed(root, &batch, cx));
        }
        if primary {
            if let Some(view) = &self.local_view {
                view.update(cx, |v, cx| v.refresh(cx));
            }
            if let Some(view) = &self.project_view {
                view.update(cx, |v, cx| v.refresh_external(cx));
            }
        }
        for tab in self.tabs.iter().chain(self.local_preview.iter()) {
            tab.update(cx, |tab, cx| {
                if plico::recent_projects::identity(&tab.target.identity().repository)
                    == plico::recent_projects::identity(root)
                {
                    tab.disk_changed(&batch, window, cx);
                }
            });
        }
        cx.notify();
    }
}
