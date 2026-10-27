//! Project-owned repository membership. Views share results, not discovery jobs.
use gpui_kit::*;
use plico::{
    git::{Cancel, Repository},
    project_repositories::{Mappings, Repositories, Store},
};
use std::sync::Arc;

pub(crate) enum Changed {
    Checking,
    Operations,
    Ready { history_changed: bool },
}
pub(crate) struct ProjectRoots {
    shallow: std::collections::BTreeMap<std::path::PathBuf, bool>,
    operations: Arc<Vec<plico::project_operations::Entry>>,
    colors: super::project_colors::RepositoryColors,
    mappings: Mappings,
    store: Option<Store>,
    observed: Arc<Vec<Repository>>,
    snapshot: Option<Result<Arc<Repositories>, String>>,
    previous: Option<Result<Arc<Repositories>, String>>,
    multiple_roots: bool,
    generation: u64,
    discovering: bool,
    pending: bool,
    token: Cancel,
    task: Task<()>,
}
impl EventEmitter<Changed> for ProjectRoots {}
impl Drop for ProjectRoots {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl ProjectRoots {
    pub fn open(primary: std::path::PathBuf, cx: &mut Context<Self>) -> Self {
        // Tests inject an isolated Store through with_store; never use the
        // developer's configuration directory from a GPUI fixture.
        #[cfg(not(test))]
        let store = Store::for_project(&primary);
        #[cfg(test)]
        let store = None;
        let registered = if primary.as_os_str().is_empty() {
            vec![]
        } else {
            vec![Repository::new(primary)]
        };
        Self::with_store(registered, store, cx)
    }
    #[cfg(test)]
    pub fn new(registered: Vec<Repository>, cx: &mut Context<Self>) -> Self {
        Self::with_store(registered, None, cx)
    }
    fn with_store(
        registered: Vec<Repository>,
        store: Option<Store>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut roots = Self {
            shallow: Default::default(),
            operations: Arc::new(Vec::new()),
            colors: super::project_colors::RepositoryColors::new(
                registered.iter().map(|r| r.path.as_path()),
            ),
            observed: Arc::new(registered.clone()),
            mappings: Mappings::new(registered),
            store,
            snapshot: None,
            previous: None,
            multiple_roots: false,
            generation: 0,
            discovering: false,
            pending: false,
            token: Cancel::default(),
            task: Task::ready(()),
        };
        roots.refresh(cx);
        roots
    }
    pub fn operations(&self) -> &Arc<Vec<plico::project_operations::Entry>> {
        &self.operations
    }
    /// A completed query may not cross a membership replacement.
    pub fn publish_operations(
        &mut self,
        roots: &Arc<Vec<Repository>>,
        entries: Arc<Vec<plico::project_operations::Entry>>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !Arc::ptr_eq(roots, &self.observed) {
            return false;
        }
        self.operations = entries;
        cx.emit(Changed::Operations);
        cx.notify();
        true
    }
    pub fn color(&self, root: &std::path::Path) -> Option<u32> {
        self.colors.get(root)
    }
    /// A read-only snapshot; only this owner may publish discovery results.
    pub fn snapshot(&self) -> Option<Result<Arc<Repositories>, String>> {
        self.snapshot.clone()
    }
    pub fn is_shallow(&self, root: &std::path::Path) -> bool {
        self.snapshot.as_ref().is_some_and(Result::is_ok)
            && self.shallow.get(root).copied().unwrap_or(false)
    }
    /// Preserve the last completed observation set while discovery runs.
    pub fn observation_roots(&self) -> Arc<Vec<Repository>> {
        self.observed.clone()
    }
    /// Presentation keeps the last completed membership while a refresh runs.
    /// Query owners must use snapshot(), which invalidates immediately.
    pub fn has_multiple_roots(&self) -> bool {
        self.multiple_roots
    }
    /// Renaming an ancestor of a registered/declared root can arrive without
    /// individual .git events. Ordinary file renames must not rescan membership.
    pub fn affected_by_worktree(
        &self,
        observed_root: &std::path::Path,
        batch: &plico::worktree_events::Batch,
    ) -> bool {
        if batch.all || batch.membership {
            return true;
        }
        let affects = |root: &std::path::Path| batch.affects_root(observed_root, root);
        if self
            .mappings
            .registered()
            .iter()
            .any(|repo| affects(&repo.path))
        {
            return true;
        }
        match &self.snapshot {
            Some(Ok(snapshot)) => {
                snapshot.roots.iter().any(|repo| affects(&repo.path))
                    || snapshot
                        .submodules
                        .iter()
                        .any(|module| affects(&module.root))
            }
            // A scan in flight may discover a root affected by this batch.
            _ => !batch.paths.is_empty(),
        }
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        // A burst of invalidations must not repeatedly kill the same Git scan.
        // Keep one follow-up, because this scan may predate the latest change.
        if self.discovering {
            self.pending = true;
            return;
        }
        self.discovering = true;
        self.token.cancel();
        self.token = Cancel::default();
        self.generation = self
            .generation
            .checked_add(1)
            .expect("Project roots generation exhausted");
        let generation = self.generation;
        let token = self.token.clone();
        let mut mappings = self.mappings.clone();
        let store = self.store.clone();
        if let Some(snapshot) = self.snapshot.take() {
            self.previous = Some(snapshot);
        }
        // Invalidate consumers before the replacement query completes.
        cx.emit(Changed::Checking);
        self.task = cx.spawn(async move |this, cx| {
            let (mappings, result, shallow) = cx
                .background_spawn(async move {
                    let result = match store {
                        Some(store) => mappings.refresh_persisted(&store, &token),
                        None => mappings.refresh(&token),
                    }
                    .map(Arc::new);
                    let shallow = result
                        .as_ref()
                        .ok()
                        .into_iter()
                        .flat_map(|members| members.roots.iter())
                        .filter_map(|repo| {
                            repo.is_shallow(&token)
                                .ok()
                                .map(|value| (repo.path.clone(), value))
                        })
                        .collect();
                    (mappings, result, shallow)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.discovering = false;
                if std::mem::take(&mut this.pending) {
                    // Do not publish membership known to predate an invalidation.
                    this.refresh(cx);
                    return;
                }
                this.mappings = mappings;
                this.shallow = shallow;
                let mut observed = this.mappings.registered().to_vec();
                if let Ok(roots) = &result {
                    for repo in &roots.roots {
                        if !observed.iter().any(|old| old.path == repo.path) {
                            observed.push(repo.clone());
                        }
                    }
                }
                if !this
                    .observed
                    .iter()
                    .map(|repo| &repo.path)
                    .eq(observed.iter().map(|repo| &repo.path))
                {
                    this.colors = super::project_colors::RepositoryColors::new(
                        observed.iter().map(|r| r.path.as_path()),
                    );
                    this.observed = Arc::new(observed);
                    // Removed roots must disappear immediately; new roots remain
                    // unknown until the operation observer has read them.
                    this.operations = Arc::new(
                        this.operations
                            .iter()
                            .filter(|entry| {
                                this.observed
                                    .iter()
                                    .any(|root| root.path == entry.repository.path)
                            })
                            .cloned()
                            .collect(),
                    );
                }
                this.multiple_roots = result.as_ref().is_ok_and(|roots| roots.roots.len() > 1);
                let history_changed = match (&this.previous, &result) {
                    (Some(Ok(old)), Ok(new)) => {
                        old.errors != new.errors
                            || !old
                                .roots
                                .iter()
                                .map(|r| &r.path)
                                .eq(new.roots.iter().map(|r| &r.path))
                    }
                    (Some(Err(old)), Err(new)) => old != new,
                    _ => true,
                };
                this.previous = None;
                this.snapshot = Some(result);
                cx.emit(Changed::Ready { history_changed });
                cx.notify();
            });
        });
    }
}

impl super::Review {
    pub(super) fn bind_project_roots(&mut self, cx: &mut Context<Self>) {
        self.project_roots_subscription = Some(cx.subscribe(
            &self.project_roots,
            |s, roots, event: &Changed, cx| {
                if roots.entity_id() != s.project_roots.entity_id() {
                    return;
                }
                if matches!(event, Changed::Ready { .. }) {
                    let catalog = roots.read(cx);
                    if let Some(Ok(members)) = catalog.snapshot() {
                        let entries = members
                            .roots
                            .iter()
                            .map(|repo| {
                                (
                                    repo.path.clone(),
                                    catalog.color(&repo.path).unwrap_or(0x865762),
                                )
                            })
                            .collect();
                        s.paths_menu
                            .update(cx, |menu, cx| menu.set_roots(entries, cx));
                    }
                }
                if matches!(event, Changed::Operations) {
                    let primary = plico::recent_projects::identity(&s.repo.path);
                    let state = roots
                        .read(cx)
                        .operations()
                        .iter()
                        .find(|entry| {
                            plico::recent_projects::identity(&entry.repository.path) == primary
                        })
                        .and_then(|entry| entry.state.as_ref().ok())
                        .map(|state| state.as_ref().clone());
                    if s.operation_state != state {
                        s.operation_state = state;
                        s.refresh_reword_gate(cx);
                    }
                    cx.notify();
                    return;
                }
                if !s.local
                    && (s.log.project_window().is_some() || roots.read(cx).has_multiple_roots())
                {
                    match event {
                        Changed::Operations => unreachable!(),
                        Changed::Checking => {
                            // In-flight results must not cross a membership check.
                            // Completed rows/counts remain valid until membership
                            // actually changes; staging alone need not reread them.
                            if s.log_query.running() {
                                s.pending_project_reload.get_or_insert(true);
                            }
                            s.log_query.cancel();
                        }
                        Changed::Ready { history_changed } => {
                            if *history_changed || s.log.project_window().is_none() {
                                s.history_count.invalidate();
                                s.pending_project_reload.get_or_insert(true);
                            }
                            if let Some(preserve) = s.pending_project_reload.take() {
                                s.reload_project_history(false, preserve, None, cx);
                            }
                        }
                    }
                }
                if s.repository_menu_open {
                    s.dismiss_repository_child(cx);
                    // Keep the last rendered roots during discovery/reference loading.
                    // Their entry points remain disabled until fresh metadata arrives.
                    if let Some(token) = s.repository_recent_token.take() {
                        token.cancel();
                    }
                    s.repository_recent_generation += 1;
                    s.repository_recent_task = Task::ready(());
                    if matches!(event, Changed::Ready { .. }) {
                        s.load_repository_recents(cx);
                    }
                }
                if let Some(view) = &s.comparison {
                    let roots = roots.read(cx);
                    let color = roots
                        .has_multiple_roots()
                        .then(|| roots.color(view.read(cx).repository_path()))
                        .flatten();
                    view.update(cx, |view, cx| view.set_repository_color(color, cx));
                }
                s.rebuild_files(cx);
                cx.notify();
            },
        ));
        self.rebuild_files(cx);
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;

    #[gpui::test]
    fn unshallow_uses_editor_repository_and_invalidates_unknown_membership(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::ui::Review;
        use gpui::component::Root;
        cx.update(gpui::component::init);
        let parent = std::env::temp_dir().join("plico-context-parent");
        let child = parent.join("child");
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(std::path::PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        review.update_in(cx, |s, window, cx| {
            s.repo = Repository::new(&parent);
            s.project_roots.update(cx, |roots, _| {
                roots.token.cancel();
                roots.task = Task::ready(());
                roots.snapshot = Some(Ok(Arc::new(Repositories {
                    roots: vec![Repository::new(&parent), Repository::new(&child)],
                    ..Default::default()
                })));
                roots.shallow.insert(parent.clone(), false);
                roots.shallow.insert(child.clone(), true);
            });
            assert!(s.unshallow_repository(window, cx).is_none());
            s.history_focus.focus(window, cx);
            let child_scope = plico::project_log::Scope {
                roots: vec![plico::project_log::LoadedRoot {
                    query: plico::project_log::Query {
                        repository: Repository::new(&child),
                        branches: vec![],
                        filter: None,
                    },
                    current_branch: None,
                    ref_titles: Arc::default(),
                    tips: vec![],
                }],
                errors: vec![],
            };
            s.log = crate::ui::log_display::Display::fixture(
                vec![],
                Some(Arc::new(child_scope.clone())),
            );
            assert_eq!(
                s.unshallow_repository(window, cx).unwrap().path,
                child,
                "an empty filtered Log still supplies its repository context"
            );
            let mut partial = child_scope;
            partial.errors.push((parent.clone(), "Unavailable".into()));
            s.log = crate::ui::log_display::Display::fixture(vec![], Some(Arc::new(partial)));
            assert!(
                s.unshallow_repository(window, cx).is_none(),
                "a failed root must not turn a multi-root Log into a child-only context"
            );
            s.log = Default::default();
            let file = cx.new(|_| {
                crate::project_view::FileTab::new(parent.clone(), "child/file.txt".into())
            });
            let id = file.entity_id();
            s.file_tabs.push(file);
            s.editor_tabs.insert(id);
            s.editor_tabs.activate(id);
            assert_eq!(s.unshallow_repository(window, cx).unwrap().path, child);
            let snapshot = s.project_roots.read(cx).snapshot.clone();
            s.project_roots.update(cx, |roots, _| roots.snapshot = None);
            assert!(
                s.unshallow_repository(window, cx).is_none(),
                "never act using a stale membership scan"
            );
            s.project_roots
                .update(cx, |roots, _| roots.snapshot = snapshot);
            s.close_file_tab(0, window, cx);
            assert!(s.file_tabs.is_empty());
            assert!(
                s.unshallow_repository(window, cx).is_none(),
                "closing the child editor must not retain it as an operation target"
            );
        });
    }

    #[gpui::test]
    fn file_groups_follow_project_membership_even_with_no_log_rows(cx: &mut gpui::TestAppContext) {
        use crate::ui::{FileRow, Review};
        use gpui::component::Root;
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!("plico-membership-view-{}-{nonce}", std::process::id());
        let fixture = Fixture(std::env::temp_dir().join(name));
        let first = fixture.0.join("first");
        let second = fixture.0.join("second");
        for path in [&first, &second] {
            std::fs::create_dir_all(path).unwrap();
            let out = std::process::Command::new("git")
                .args(["init", "-q", "-b", "main"])
                .arg(path)
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
        }
        cx.update(gpui::component::init);
        let roots = cx.new(|cx| {
            ProjectRoots::new(vec![Repository::new(&first), Repository::new(&second)], cx)
        });
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(std::path::PathBuf::new(), 100, None, window, cx));
            view.update(cx, |s, cx| {
                s.repo = Repository::new(&first);
                s.project_roots = roots.clone();
                s.local = false;
                s.log_files_options.directory = false;
                s.bind_project_roots(cx);
            });
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        // Install presentation-only fixture data after the automatic initial
        // history read. The empty repositories have no real commit selection.
        let install_file = |s: &mut Review, cx: &mut Context<Review>| {
            // This synthetic file belongs to the primary repository; there is
            // deliberately no selected project-log row in this fixture.
            s.log = crate::ui::log_display::Display::fixture_single(
                &s.repo.path,
                s.log.commits().as_ref().clone(),
            );
            s.files = Arc::new(vec![plico::git::Change {
                area: plico::git::Area::Commit,
                path: "only.txt".into(),
                old_path: None,
                status: "A".into(),
            }]);
            s.rebuild_files(cx);
        };
        review.update(cx, install_file);
        let observed = roots.read_with(cx, |s, _| s.observation_roots());
        let entries = plico::project_operations::Observer::default()
            .poll(&observed, &Cancel::default())
            .unwrap()
            .unwrap();
        review.update(cx, |s, _| s.operation_state = None);
        roots.update(cx, |s, cx| {
            assert!(s.publish_operations(&observed, entries, cx));
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(
                s.operation_state
                    .as_ref()
                    .is_some_and(|state| !state.in_progress()),
                "primary chrome uses the same project observation as child roots"
            );
        });
        // Recovery eligibility is project-wide, including operations started
        // outside this process; unreadable roots must not become actionable.
        let mut recovery = roots.read_with(cx, |s, _| s.operations().as_ref().clone());
        let child = recovery
            .iter_mut()
            .find(|entry| entry.repository.path == second)
            .unwrap();
        let mut merge = child.state.as_ref().unwrap().as_ref().clone();
        merge.operation = plico::operation_state::Operation::Merge;
        merge.conflicts.push("conflict.txt".into());
        child.state = Ok(Arc::new(merge));
        roots.update(cx, |s, cx| {
            assert!(s.publish_operations(&observed, Arc::new(recovery.clone()), cx));
        });
        review.read_with(cx, |s, cx| {
            assert_eq!(
                s.merge_roots(cx)
                    .iter()
                    .map(|r| &r.path)
                    .collect::<Vec<_>>(),
                vec![&second]
            );
            assert!(s.project_has_conflicts(cx));
        });
        recovery
            .iter_mut()
            .find(|entry| entry.repository.path == second)
            .unwrap()
            .state = Err("Unavailable".into());
        roots.update(cx, |s, cx| {
            assert!(s.publish_operations(&observed, Arc::new(recovery), cx));
        });
        review.read_with(cx, |s, cx| {
            assert!(s.merge_roots(cx).is_empty());
        });
        review.read_with(cx, |s, _| {
            assert!(s.log.commits().is_empty());
            assert!(matches!(&s.file_rows[0], FileRow::Folder(root, _, _, label, _, 1, true) if root == &first && label == std::path::Path::new("first")));
        });
        review.update(cx, |s, cx| {
            s.snapshot = Some(s.repo.reference_snapshot(&Cancel::default()).unwrap());
            s.repository_menu_open = true;
            s.load_repository_recents(cx);
        });
        cx.run_until_parked();
        assert_eq!(
            review.read_with(cx, |s, _| s.repository_popup_roots().len()),
            2
        );
        // Removing this owned test repository must remove its membership, even
        // though the Log has no rows whose change could trigger a tree rebuild.
        std::fs::remove_dir_all(second.join(".git")).unwrap();
        review.update_in(cx, |s, window, cx| {
            s.sync_worktree(
                plico::worktree_events::Batch {
                    membership: true,
                    ..Default::default()
                },
                false,
                window,
                cx,
            );
        });
        roots.read_with(cx, |s, _| {
            assert!(s.snapshot().is_none());
            assert!(
                s.has_multiple_roots(),
                "no grouping flicker during a pending scan"
            );
        });
        assert_eq!(
            review.read_with(cx, |s, _| s.repository_popup_roots().len()),
            2,
            "keep the repository rows visible while membership is being checked"
        );
        review.update_in(cx, |s, window, cx| {
            assert!(s.repository_roots_updating(cx));
            s.repository_selection =
                Some(plico::branch_search::Selection::Repository(second.clone()));
            s.repository_search_input
                .update(cx, |input, cx| input.focus(window, cx));
            s.repository_activate_selection(false, window, cx);
            assert!(
                s.repository_child_selection.is_none(),
                "stale roots cannot open an action menu"
            );
        });
        cx.run_until_parked();
        review.update(cx, install_file);
        review.read_with(cx, |s, _| {
            assert_eq!(s.file_rows.len(), 1);
            assert!(
                s.repository_popup_roots().is_empty(),
                "removed root disappears after discovery completes"
            );
            assert!(matches!(s.file_rows[0], FileRow::File(0, 0)));
        });
        roots.read_with(cx, |s, _| {
            assert!(!s.has_multiple_roots());
            assert_eq!(s.snapshot().unwrap().unwrap().errors.len(), 1);
        });
    }

    #[gpui::test]
    fn catalog_publishes_independent_registered_roots_and_retains_root_errors(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = format!("plico-project-catalog-{}-{nonce}", std::process::id());
        let fixture = Fixture(std::env::temp_dir().join(directory));
        std::fs::create_dir_all(&fixture.0).unwrap();
        let mut registered = Vec::new();
        for name in ["first", "second"] {
            let path = fixture.0.join(name);
            let result = std::process::Command::new("git")
                .args(["init", "-q", "-b", "main"])
                .arg(&path)
                .output()
                .unwrap();
            assert!(result.status.success(), "{:?}", result);
            registered.push(Repository::new(path));
        }
        let missing = fixture.0.join("missing");
        registered.push(Repository::new(&missing));
        let expected = registered[..2]
            .iter()
            .map(|r| r.path.clone())
            .collect::<Vec<_>>();
        let roots = cx.new(|cx| ProjectRoots::new(registered, cx));
        assert!(roots.read_with(cx, |s, _| s.snapshot()).is_none());
        cx.run_until_parked();
        let snapshot = roots.read_with(cx, |s, _| s.snapshot()).unwrap().unwrap();
        assert_eq!(
            snapshot
                .roots
                .iter()
                .map(|r| r.path.clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(snapshot.errors.len(), 1);
        assert_eq!(snapshot.errors[0].0, missing);
        let observed = roots.read_with(cx, |s, _| s.observation_roots());
        let entries = plico::project_operations::Observer::default()
            .poll(&observed, &Cancel::default())
            .unwrap()
            .unwrap();
        roots.update(cx, |s, cx| {
            assert!(s.publish_operations(&observed, entries.clone(), cx));
            assert_eq!(s.operations().len(), 3);
            assert!(s.operations()[2].state.is_err());
            let replaced_membership = Arc::new(observed.as_ref().clone());
            assert!(!s.publish_operations(&replaced_membership, Arc::new(vec![]), cx));
            assert_eq!(
                s.operations().len(),
                3,
                "late results cannot replace current root states"
            );
        });
        roots.read_with(cx, |s, _| {
            let batch = |path: &str| plico::worktree_events::Batch {
                paths: [std::path::PathBuf::from(path)].into(),
                ..Default::default()
            };
            assert!(s.affected_by_worktree(&fixture.0, &batch("first")));
            assert!(s.affected_by_worktree(&fixture.0, &batch("missing")));
            assert!(!s.affected_by_worktree(&fixture.0, &batch("first/src/main.rs")));
            assert!(!s.affected_by_worktree(&fixture.0, &batch("unregistered")));
            assert!(!s.affected_by_worktree(
                &fixture.0,
                &plico::worktree_events::Batch {
                    index: true,
                    ..Default::default()
                }
            ));
        });
        let colors = roots.read_with(cx, |s, _| {
            expected.iter().map(|p| s.color(p)).collect::<Vec<_>>()
        });
        assert!(colors.iter().all(Option::is_some));
        roots.update(cx, |s, cx| {
            s.refresh(cx);
            assert!(s.snapshot().is_none());
            assert_eq!(
                expected.iter().map(|p| s.color(p)).collect::<Vec<_>>(),
                colors
            );
        });
        cx.run_until_parked();
        assert_eq!(
            roots.read_with(cx, |s, _| expected
                .iter()
                .map(|p| s.color(p))
                .collect::<Vec<_>>()),
            colors
        );
        let token = roots.read_with(cx, |s, _| s.token.clone());
        drop(roots);
        // GPUI releases dropped entities while flushing an application update.
        cx.update(|_| {});
        cx.run_until_parked();
        assert!(
            token.cancelled(),
            "project disposal must cancel its discovery lifetime"
        );
        assert_eq!(snapshot.roots.len(), 2, "readers own immutable snapshots");
    }

    #[gpui::test]
    fn refresh_coalesces_inflight_invalidations_without_publishing_stale_membership(
        cx: &mut gpui::TestAppContext,
    ) {
        let roots = cx.new(|cx| ProjectRoots::new(vec![], cx));
        let published = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let subscription = cx.update(|cx| {
            let published = published.clone();
            cx.subscribe(&roots, move |roots, _: &Changed, cx| {
                let roots = roots.read(cx);
                if roots.snapshot().is_some() {
                    published.borrow_mut().push(roots.generation);
                }
            })
        });
        let old = roots.read_with(cx, |s, _| s.token.clone());
        roots.update(cx, |s, cx| {
            for _ in 0..100 {
                s.refresh(cx);
            }
            assert!(!old.cancelled(), "keep the in-flight discovery alive");
            assert_eq!(s.generation, 1, "no duplicate discovery jobs");
            assert!(s.snapshot.is_none());
        });
        cx.run_until_parked();
        roots.read_with(cx, |s, _| {
            assert!(
                s.snapshot
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .roots
                    .is_empty()
            );
            assert!(!s.token.cancelled());
            assert_eq!(s.generation, 2, "one follow-up for the entire burst");
            assert!(!s.discovering);
            assert!(!s.pending);
        });
        assert_eq!(*published.borrow(), vec![2], "never publish the stale scan");
        drop(subscription);
        assert!(old.cancelled(), "the finished scan token is retired");
        let token = roots.read_with(cx, |s, _| s.token.clone());
        roots.update(cx, |s, cx| {
            s.refresh(cx);
            assert!(
                s.snapshot.is_none(),
                "old membership must not survive invalidation"
            );
        });
        assert!(token.cancelled());
        cx.run_until_parked();
    }
}
