//! Main-window log selection coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

/// Content identity excludes mutable refs, but includes shallow parent semantics.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Content {
    id: plico::project_log::CommitId,
    parents: Vec<String>,
    shallow: bool,
}

impl Review {
    fn selection_content(&self) -> Vec<Content> {
        let rows = self.commit_ids();
        let ordered = self.log_selection.ordered(&rows);
        let mut selected = ordered.into_iter().peekable();
        rows.iter()
            .zip(self.log.commits().iter())
            .filter_map(|(id, commit)| {
                if selected.peek().is_some_and(|next| *next == id) {
                    selected.next();
                    Some(Content {
                        id: id.clone(),
                        parents: commit.parents.clone(),
                        shallow: commit.shallow_boundary,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
    /// A new page may move the same selection to another row. Only reload
    /// immutable details when its content identity changed or loading failed.
    pub(super) fn reconcile_log_details(&mut self, cx: &mut Context<Self>) {
        let ids = self.commit_ids();
        self.log_selection.reconcile(&ids);
        let content = self.selection_content();
        if self.loaded_log_selection.as_ref() != Some(&content) || self.local {
            self.load_log_selection(cx);
            return;
        }
        self.selected = if content.len() == 1 {
            ids.iter().position(|id| id == &content[0].id)
        } else {
            None
        };
        self.pending_preview_file = None;
        if let Some(index) = self.selected {
            let commit = &self.log.commits()[index];
            let repo = self.log_repository(index).unwrap();
            self.reword_gate =
                Some(cx.new(|cx| super::reword::Gate::new(repo.clone(), commit.id.clone(), cx)));
            if let Some(branches) = &self.containing_branches {
                branches.update(cx, |s, cx| s.refresh(repo, commit.id.clone(), cx));
            }
        } else {
            // Cards follow the selection's row order, independent of row indices.
            let mut cards = Vec::new();
            for (entry, (refs, _, view)) in content.iter().zip(&mut self.selection_details) {
                let index = ids.iter().position(|id| id == &entry.id).unwrap();
                *refs = self.log.commits()[index].refs.clone();
                cards.push((
                    Repository::new(entry.id.root.as_ref()),
                    entry.id.hash.clone(),
                    view.downgrade(),
                ));
            }
            self.selection_branches =
                Some(cx.new(|cx| crate::containing_branches::Batch::from_roots(cards, cx)));
        }
    }
    pub(super) fn log_details_loading(&self) -> bool {
        self.log_query.busy() && self.loaded_log_selection.is_none()
    }

    pub(super) fn log_repository(&self, index: usize) -> Option<Repository> {
        self.log.repository(index)
    }
    pub(super) fn log_files_repository(&self) -> Option<Repository> {
        if self.local {
            Some(self.repo.clone())
        } else {
            self.selected.and_then(|index| self.log_repository(index))
        }
    }
    pub(super) fn current_log_scope(&self) -> Option<Arc<plico::project_log::Scope>> {
        self.log.scope()
    }

    pub(super) fn select_commit(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.log_query.history_loading() {
            return;
        }
        self.activate_log_preview();
        let ids = self.commit_ids();
        self.log_selection.select(&ids, index, false, false);
        self.load_log_selection(cx);
    }
    pub(super) fn commit_ids(&self) -> Vec<plico::project_log::CommitId> {
        self.log.ids().as_ref().clone()
    }
    pub(super) fn select_log_row(
        &mut self,
        index: usize,
        toggle: bool,
        extend: bool,
        context: bool,
        cx: &mut Context<Self>,
    ) {
        if self.log_query.history_loading() {
            return;
        }
        self.activate_log_preview();
        let ids = self.commit_ids();
        let before = self
            .log_selection
            .ordered(&ids)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if context {
            self.log_selection.context(&ids, index);
        } else {
            self.log_selection.select(&ids, index, toggle, extend);
        }
        if before.iter().collect::<Vec<_>>() != self.log_selection.ordered(&ids) {
            self.load_log_selection(cx);
        }
        cx.notify();
    }
    pub(super) fn load_log_selection(&mut self, cx: &mut Context<Self>) {
        self.loaded_log_selection = None;
        self.reword_gate = None;
        let rows = self.commit_ids();
        let ids = self
            .log_selection
            .ordered(&rows)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let preferred = self.preferred_log_file();
        self.aggregate_chain = None;
        self.selection_details.clear();
        self.selection_branches = None;
        self.local_preview = None;
        if ids.len() == 1 {
            if let Some(index) = rows.iter().position(|id| id == &ids[0]) {
                self.select_commit_with_file(index, preferred, cx);
            }
            return;
        }
        self.reset_log_presentation();
        let (generation, token) = self.log_query.begin(log_query::Kind::Details);
        self.selected = None;
        self.selected_file = None;
        self.files = Arc::default();
        self.rebuild_files(cx);
        self.message.clear();
        self.commit_metadata = Default::default();
        self.containing_branches = None;
        self.base = None;
        self.pending_preview_file = preferred.clone();
        if ids.is_empty() {
            self.log_query.finish(generation);
            self.pending_preview_file = None;
            cx.notify();
            return;
        }
        self.log_query.track(
            generation,
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_spawn(async move {
                        let snapshot = plico::project_log::selection::load(&ids, &token)?;
                        let mut presentations = Vec::new();
                        let mut presentation_bytes = 0usize;
                        for entry in snapshot.commits.iter().take(50) {
                            let repo = Repository::new(entry.root.as_ref());
                            let commit = &entry.commit;
                            let details = repo.commit_metadata(commit, &token)?;
                            let presentation = commit_presentation::build(&details, Some(commit));
                            presentation_bytes = presentation_bytes
                                .saturating_add(presentation.message.len())
                                .saturating_add(
                                    presentation.metadata.iter().map(String::len).sum::<usize>(),
                                );
                            if presentation_bytes > git::OUTPUT_LIMIT {
                                return Err("Log details exceed the preview output limit".into());
                            }
                            presentations.push((
                                repo,
                                commit.refs.clone(),
                                presentation,
                                commit.id.clone(),
                            ));
                        }
                        let chain = if snapshot.changes.iter().all(|group| group.files.is_empty()) {
                            None
                        } else {
                            let mut groups = snapshot
                                .changes
                                .into_iter()
                                .map(|group| (Repository::new(group.root.as_ref()), group.files))
                                .collect::<Vec<_>>();
                            groups.sort_by(|(a, _), (b, _)| {
                                a.path
                                    .file_name()
                                    .cmp(&b.path.file_name())
                                    .then(a.path.cmp(&b.path))
                            });
                            Some(diff_target::Chain::combined_roots(groups, 0)?.1)
                        };
                        Ok::<_, String>((chain, presentations))
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if !this.log_query.finish(generation) {
                        return;
                    }
                    this.pending_preview_file = None;
                    match result {
                        Ok((chain, presentations)) => {
                            this.files = Arc::new(
                                chain
                                    .as_ref()
                                    .map(|c| c.changes().to_vec())
                                    .unwrap_or_default(),
                            );
                            this.aggregate_chain = chain;
                            this.loaded_log_selection = Some(this.selection_content());
                            let mut cards = Vec::new();
                            this.selection_details = presentations
                                .into_iter()
                                .map(|(repo, refs, presentation, id)| {
                                    let view = cx.new(
                                        crate::containing_branches::ContainingBranches::pending,
                                    );
                                    cards.push((repo, id, view.downgrade()));
                                    (refs, presentation, view)
                                })
                                .collect();
                            this.selection_branches = Some(cx.new(|cx| {
                                crate::containing_branches::Batch::from_roots(cards, cx)
                            }));
                            this.prune_directory_state();
                            this.rebuild_files(cx);
                            if let Some(index) = this.preferred_file_index(preferred.as_ref()) {
                                this.load_file(index, cx);
                            }
                        }
                        Err(error) => this.error = Some(error),
                    }
                    cx.notify();
                });
            }),
        );
        cx.notify();
    }
    pub(super) fn select_commit_with_file(
        &mut self,
        index: usize,
        preferred: Option<(PathBuf, Change)>,
        cx: &mut Context<Self>,
    ) {
        if self.log_query.history_loading() {
            return;
        }
        let Some(commit) = self.log.commits().get(index).cloned() else {
            return;
        };
        let Some(repo) = self.log_repository(index) else {
            return;
        };
        self.loaded_log_selection = None;
        self.reword_gate =
            Some(cx.new(|cx| super::reword::Gate::new(repo.clone(), commit.id.clone(), cx)));
        self.reset_log_presentation();
        let (generation, token) = self.log_query.begin(log_query::Kind::Details);
        self.aggregate_chain = None;
        self.selection_details.clear();
        self.selected = Some(index);
        self.pending_preview_file = preferred.clone();
        self.selected_file = None;
        self.files = Arc::default();
        self.rebuild_files(cx);
        self.message.clear();
        self.commit_metadata = Default::default();
        self.containing_branches = None;
        self.base = None;
        self.containing_branches = Some(cx.new(|cx| {
            crate::containing_branches::ContainingBranches::new(repo.clone(), commit.id.clone(), cx)
        }));
        if let Some(details) = self.details_cache.get(&repo.path, &commit) {
            self.log_query.finish(generation);
            self.apply_commit_details(&details, preferred.as_ref(), cx);
            cx.notify();
            return;
        }
        let repository_path = repo.path.clone();
        self.log_query.track(
            generation,
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_spawn(async move { repo.details(&commit, &token) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if !this.log_query.finish(generation) {
                        return;
                    }
                    match result {
                        Ok(d) => {
                            let d = Arc::new(d);
                            if let Some(commit) =
                                this.selected.and_then(|i| this.log.commits().get(i))
                            {
                                this.details_cache
                                    .insert(&repository_path, commit, d.clone());
                            }
                            this.apply_commit_details(&d, preferred.as_ref(), cx);
                        }
                        Err(e) => this.error = Some(e),
                    }
                    cx.notify();
                });
            }),
        );
        cx.notify();
    }
    fn apply_commit_details(
        &mut self,
        d: &git::Details,
        preferred: Option<&(PathBuf, Change)>,
        cx: &mut Context<Self>,
    ) {
        self.loaded_log_selection = Some(self.selection_content());
        let commit = self.selected.and_then(|i| self.log.commits().get(i));
        let presentation = commit_presentation::build(d, commit);
        self.message = presentation.message;
        self.commit_metadata = presentation.metadata;
        self.base = Some(if commit.is_some_and(|commit| commit.shallow_boundary) {
            i18n::ComparisonTitle::Label("Shallow boundary · Parent comparison unavailable")
        } else {
            i18n::ComparisonTitle::Parent {
                revision: d.base.clone(),
                merge: commit.is_some_and(|commit| commit.parents.len() > 1),
            }
        });
        self.files = Arc::new(d.files.clone());
        self.prune_directory_state();
        self.rebuild_files(cx);
        self.pending_preview_file = None;
        if let Some(index) = self.preferred_file_index(preferred) {
            self.load_file(index, cx);
        }
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod repository_tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;

    #[gpui::test]
    fn child_details_cache_preview_and_diff_tab_use_the_selected_repository(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-log-root-routing-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        let parent = fixture.0.join("parent");
        let child = fixture.0.join("child");
        for (path, name) in [(&parent, "parent.txt"), (&child, "child.txt")] {
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(path.join(name), name).unwrap();
            for args in [
                vec!["init", "-q"],
                vec!["add", name],
                vec![
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-qm",
                    name,
                ],
            ] {
                let out = std::process::Command::new("git")
                    .arg("-C")
                    .arg(path)
                    .args(args)
                    .output()
                    .unwrap();
                assert!(
                    out.status.success(),
                    "{}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
        let child_repo = Repository::new(&child);
        let commit = child_repo
            .snapshot_at(None, 0, 10, None, &Cancel::default())
            .unwrap()
            .commits
            .remove(0);
        let child_key = plico::project_log::CommitId {
            root: Arc::new(child.clone()),
            hash: commit.id.clone(),
        };
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        review.update(cx, |s, cx| {
            s.repo = Repository::new(&parent);
            s.local = false;
            s.welcome = false;
            let page = plico::project_log::Page {
                entries: [&parent, &child]
                    .into_iter()
                    .map(|root| plico::project_log::Entry {
                        display_parents: None,
                        root: Arc::new(root.clone()),
                        commit: commit.clone(),
                    })
                    .collect(),
                more: false,
            };
            s.log = log_display::Display::fixture(page.entries, None);
            // A parent cache entry for the same hash must not satisfy child details.
            s.details_cache.insert(
                &parent,
                &commit,
                Arc::new(git::Details {
                    message: "wrong parent cache".into(),
                    author: String::new(),
                    authored: String::new(),
                    committer: String::new(),
                    committed: String::new(),
                    base: String::new(),
                    files: vec![],
                }),
            );
            s.select_commit(1, cx);
            s.refresh_reword_gate(cx);
            assert_eq!(s.log_selection.ordered(&s.commit_ids()), vec![&child_key]);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                assert!(s.error.is_none(), "{:?}", s.error);
                assert!(!s.log_query.busy());
                assert!(s.message.contains("child.txt"), "{}", s.message);
                assert!(s.can_reword(&commit.id, cx));
                let prepared = s
                    .reword_gate
                    .as_ref()
                    .unwrap()
                    .read(cx)
                    .result
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unwrap();
                assert_eq!(prepared.repository().path, child);
                let ids = s.commit_ids();
                s.log_selection.select(&ids, 0, false, false);
                assert!(
                    !s.can_reword(&commit.id, cx),
                    "a same-hash parent row cannot reuse a child gate"
                );
                s.log_selection.select(&ids, 1, false, false);
                assert_eq!(s.files[0].path, PathBuf::from("child.txt"));
                assert!(s.details_cache.get(&child, &commit).is_some());
                assert_eq!(
                    s.details_cache.get(&parent, &commit).unwrap().message,
                    "wrong parent cache"
                );
                let (target, _) = s.log_preview_target(cx).unwrap();
                assert_eq!(target.identity().repository, child);
                s.open_diff_tab(window, cx);
                assert_eq!(s.tabs.len(), 1);
                assert_eq!(s.tabs[0].read(cx).target.identity().repository, child);
                assert_eq!(
                    s.repo.path, parent,
                    "reading a child must not switch the parent project"
                );
                // Identical object IDs in two roots remain two selectable rows.
                let parent_key = plico::project_log::CommitId {
                    root: Arc::new(parent.clone()),
                    hash: commit.id.clone(),
                };
                let child_key = plico::project_log::CommitId {
                    root: Arc::new(child.clone()),
                    hash: commit.id.clone(),
                };
                s.log = log_display::Display::fixture_single(
                    &s.repo.path,
                    vec![commit.clone(), commit.clone()],
                );
                s.log = s
                    .log
                    .fixture_identified(vec![parent_key.clone(), child_key.clone()]);
                let ids = s.commit_ids();
                s.log_selection.select(&ids, 1, false, false);
                assert_eq!(s.log_selection.ordered(&ids), vec![&child_key]);
                s.log_selection.select(&ids, 0, true, false);
                assert_eq!(s.log_selection.ordered(&ids), vec![&parent_key, &child_key]);
                s.log_selection.context(&ids, 1);
                assert_eq!(s.log_selection.ordered(&ids), vec![&parent_key, &child_key]);
                s.log_selection.reconcile(std::slice::from_ref(&child_key));
                assert_eq!(s.log_selection.ordered(&ids), vec![&child_key]);
                s.log = s.log.fixture_identified(vec![]);
                assert!(s.commit_ids().is_empty());
                assert!(
                    s.log_preview_target(cx).is_none(),
                    "missing row ownership must not fall back to parent"
                );
                s.log = log_display::Display::fixture_single(&s.repo.path, vec![commit.clone()]);
                s.selected = Some(0);
                assert!(s.log.sources().is_none());
            })
        });
        cx.run_until_parked();
        let parent_commit = Repository::new(&parent)
            .snapshot_at(None, 0, 10, None, &Cancel::default())
            .unwrap()
            .commits
            .remove(0);
        review.update(cx, |s, cx| {
            s.log = log_display::Display::fixture_single(
                &s.repo.path,
                vec![parent_commit.clone(), commit.clone()],
            );
            s.log = s.log.fixture_identified(vec![
                plico::project_log::CommitId {
                    root: Arc::new(parent.clone()),
                    hash: parent_commit.id.clone(),
                },
                child_key.clone(),
            ]);
            let ids = s.commit_ids();
            s.log_selection.select(&ids, 0, false, false);
            s.log_selection.select(&ids, 1, true, false);
            s.load_log_selection(cx);
        });
        cx.run_until_parked();
        review.update(cx, |s, cx| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert!(!s.log_query.busy());
            assert_eq!(s.selection_details.len(), 2);
            assert_eq!(s.files.len(), 2);
            for (root, name) in [(&parent, "parent.txt"), (&child, "child.txt")] {
                let index = s
                    .files
                    .iter()
                    .position(|f| f.path == PathBuf::from(name))
                    .unwrap();
                s.load_file(index, cx);
                let (target, _) = s.log_preview_target(cx).unwrap();
                assert_eq!(&target.identity().repository, root);
                assert_eq!(
                    target
                        .read_contents(&Cancel::default())
                        .unwrap()
                        .unwrap()
                        .after
                        .as_deref(),
                    Some(name)
                );
            }
            assert_eq!(s.repo.path, parent);
        });
    }
}
