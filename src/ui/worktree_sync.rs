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
                    // Binding checks have an independent deadline: unchanged
                    // project membership does not prove unchanged administrative paths.
                    if events.bindings_due(Instant::now()) {
                        let query_token = token.clone();
                        let (next, result) = wx
                            .background_spawn(async move {
                                let result = events.poll_bindings(Instant::now(), &query_token);
                                (events, result)
                            })
                            .await;
                        events = next;
                        if result.is_err() || token.cancelled() {
                            break;
                        }
                    }
                    let mut notices = events.take(Instant::now());
                    if events.configuration_due(&notices, Instant::now()) {
                        let query_token = token.clone();
                        let (next, updated, result) = wx
                            .background_spawn(async move {
                                let result = events.poll_configuration(
                                    &mut notices,
                                    Instant::now(),
                                    &query_token,
                                );
                                (events, notices, result)
                            })
                            .await;
                        events = next;
                        notices = updated;
                        if result.is_err() || token.cancelled() {
                            break;
                        }
                    }
                    let poll_operations = roots_changed
                        || Instant::now() >= next_operations
                        || notices
                            .iter()
                            .any(|n| n.batch.all || n.batch.index || n.batch.refs);
                    if !notices.is_empty() {
                        let binding_roots: Vec<_> = notices
                            .iter()
                            .filter(|notice| notice.batch.all && notice.batch.interpretation)
                            .map(|notice| notice.root.clone())
                            .collect();
                        let configuration_roots: Vec<_> = notices
                            .iter()
                            .filter(|notice| {
                                notice.batch.configuration || notice.batch.diff_configuration
                            })
                            .map(|notice| notice.root.clone())
                            .collect();
                        let status_roots: Vec<_> = notices
                            .iter()
                            .filter(|notice| notice.batch.status_rules)
                            .map(|notice| notice.root.clone())
                            .collect();
                        let accepted = this.update_in(wx, |s, window, cx| {
                            if token.cancelled()
                                || s.documents.locked()
                                || s.operation.busy()
                                || s.repo.path != root
                            {
                                return false;
                            }
                            let primary = plico::recent_projects::identity(&s.repo.path);
                            let refresh_interpretation = notices.iter().any(|notice| {
                                notice.batch.shallow
                                    || notice.batch.configuration
                                    || notice.batch.interpretation
                            });
                            let refresh_project_log = s.log.project_window().is_some()
                                && !s.local
                                && notices.iter().any(|notice| {
                                    notice.batch.history_changed()
                                        && plico::recent_projects::identity(&notice.root) != primary
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
                            if refresh_interpretation {
                                // The primary ref poll cannot detect unchanged tips
                                // with new ancestry. Refresh the catalog, rows and
                                // count together, once for all affected roots.
                                s.refresh_impl(false, true, None, cx);
                            } else if refresh_project_log {
                                // One refresh for this whole batch, including events
                                // arriving while an older Log query was in flight.
                                s.reload_project_history(false, true, None, cx);
                            }
                            // refresh_impl also resolves comparison pairs. Apply the
                            // forced interpretation refresh last so its task cannot be
                            // replaced by a same-pair, labels-only refresh above.
                            for configuration_root in &configuration_roots {
                                for view in &s.branch_compare_tabs {
                                    view.update(cx, |view, cx| {
                                        view.repository_changed(
                                            configuration_root,
                                            &Batch {
                                                configuration: true,
                                                ..Default::default()
                                            },
                                            cx,
                                        )
                                    });
                                }
                            }
                            true
                        });
                        match accepted {
                            Ok(true) => {
                                events.acknowledge_bindings(&binding_roots);
                                events.acknowledge_configuration(&configuration_roots);
                                events.acknowledge_status_rules(&status_roots);
                            }
                            Ok(false) => {}
                            Err(_) => break,
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
        // Capture membership before this event can start rediscovery. A child
        // event refreshes the parent's gitlink, never a path inside that link.
        if let Some(view) = &self.local_view {
            let catalog = self.project_roots.read(cx).snapshot().and_then(Result::ok);
            let scope = plico::changes_scope::Scope::for_event(
                &self.repo.path,
                root,
                &batch,
                catalog.as_deref(),
            );
            view.update(cx, |view, cx| view.refresh_scope(scope, cx));
        }
        // A child history reload does not pass through refresh_impl, whose
        // cache clear protects the primary Log. Invalidate this root's details,
        // including packed replace refs and watcher rescan, without evicting
        // unrelated roots. More precise ordinary-ref retention follows later.
        if batch.history_changed() || batch.diff_configuration {
            self.details_cache.invalidate_repository(root);
        }
        if batch.all
            || batch.shallow
            || batch.interpretation
            || batch.configuration
            || batch.diff_configuration
        {
            self.invalidate_loaded_details(root);
        }
        if batch.diff_configuration {
            self.refresh_diff_configuration_details(root, cx);
        }
        if self
            .project_roots
            .read(cx)
            .affected_by_worktree(root, &batch)
        {
            self.project_roots.update(cx, |roots, cx| roots.refresh(cx));
        }
        if batch.history_changed() && self.branch_popup.open {
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
