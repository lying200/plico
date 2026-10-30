//! Publishing bounded root-qualified navigation while keeping primary refs intact.
use super::*;
use plico::project_log::{self, CommitId, navigation::Location};

impl Review {
    pub(super) fn navigate_project_commit(
        &mut self,
        name: String,
        target: CommitId,
        clear_filters: bool,
        fresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(scope) = self.current_log_scope() else {
            return;
        };
        let members = self.project_roots.read(cx).snapshot().and_then(Result::ok);
        if clear_filters && members.is_none() {
            self.error = Some(t("Project repository information is not ready").into());
            cx.notify();
            return;
        }
        let target_in_project = members
            .as_ref()
            .is_some_and(|members| members.roots.iter().any(|repo| repo.path == *target.root));
        let limit = self.limit;
        let show_long_edges = self.show_long_edges;
        let (generation, token) = self.log_query.begin(log_query::Kind::Navigation);
        self.error = None;
        self.navigation_notice = None;
        self.navigation_loading = Some(name.clone());
        let query_target = target.clone();
        self.log_query.track(
            generation,
            cx.spawn_in(window, async move |this, wx| {
                let result = wx
                    .background_spawn(async move {
                        let scope = if fresh || clear_filters {
                            let queries = if clear_filters {
                                members
                                    .as_ref()
                                    .unwrap()
                                    .roots
                                    .iter()
                                    .map(|repository| project_log::Query {
                                        repository: repository.clone(),
                                        branches: vec![],
                                        filter: None,
                                    })
                                    .collect::<Vec<_>>()
                            } else {
                                scope
                                    .roots
                                    .iter()
                                    .map(|root| root.query.clone())
                                    .collect::<Vec<_>>()
                            };
                            let mut refreshed = project_log::read(&queries, limit, &token)?.scope;
                            // Failed roots were not in the readable query list. Keep
                            // that uncertainty until project membership is reloaded.
                            Arc::make_mut(&mut refreshed)
                                .errors
                                .extend(if clear_filters {
                                    members.as_ref().unwrap().errors.clone()
                                } else {
                                    scope.errors.clone()
                                });
                            refreshed
                        } else {
                            scope
                        };
                        let located =
                            project_log::navigation::locate(scope, &query_target, limit, &token)?;
                        let (located, model) = match located {
                            Location::Found(mut page) => {
                                let model =
                                    Arc::new(plico::graph_print::Model::new(&page.rows, &token)?);
                                page.rows = model.project(show_long_edges, &token)?;
                                (Location::Found(page), Some(model))
                            }
                            other => (other, None),
                        };
                        Ok::<_, String>((located, model))
                    })
                    .await;
                let _ = wx.update(|window, cx| {
                    let _ = this.update(cx, |s, cx| {
                        if !s.log_query.finish(generation) {
                            return;
                        }
                        s.navigation_loading = None;
                        match result {
                            Ok((Location::Found(page), model)) => {
                                if clear_filters {
                                    s.branch_filter.clear();
                                    s.text_filter = None;
                                    s.date_filter = Default::default();
                                    s.filter_search
                                        .update(cx, |input, cx| input.set_value("", window, cx));
                                    s.user_menu.update(cx, |menu, cx| menu.clear_filter(cx));
                                    s.paths_menu.update(cx, |menu, cx| menu.clear_filter(cx));
                                }
                                let selected = page.selected;
                                s.install_project_window(page, model, cx);
                                s.select_commit(selected, cx);
                                s.history_scroll
                                    .scroll_to_item(selected, ScrollStrategy::Top);
                                s.history_focus.focus(window, cx);
                            }
                            Ok((outcome, _)) => {
                                s.navigation_notice = Some(NavigationNotice {
                                    name,
                                    target,
                                    can_clear: matches!(outcome, Location::Hidden)
                                        || (matches!(outcome, Location::OutsideRoots)
                                            && target_in_project),
                                })
                            }
                            Err(error) => s.error = Some(error),
                        }
                        s.flush_pending_log_details(cx);
                        cx.notify();
                    });
                });
            }),
        );
        cx.notify();
    }
    fn install_project_window(
        &mut self,
        page: project_log::navigation::Window,
        model: Option<Arc<plico::graph_print::Model>>,
        cx: &mut Context<Self>,
    ) {
        self.local = false;
        self.log = log_display::Display::project(page, model);
        self.update_history_count(cx);
    }

    pub(super) fn history_has_more(&self) -> bool {
        self.log.more()
    }

    pub(super) fn reload_project_history(
        &mut self,
        more: bool,
        preserve: bool,
        requested_offset: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        // The first project page can arrive before the primary snapshot. Fresh
        // reads derive their scope from membership, not from a previous page.
        let scope = self.current_log_scope();
        let membership = self.project_roots.read(cx).snapshot();
        let members = match membership {
            None => {
                self.pending_project_reload = Some(preserve || more || requested_offset.is_some());
                self.history_count.invalidate();
                self.log_query.begin(log_query::Kind::History);
                cx.notify();
                return;
            }
            Some(Err(error)) => {
                self.pending_project_reload = None;
                self.log_query.cancel();
                self.error = Some(error);
                cx.notify();
                return;
            }
            Some(Ok(members)) => members,
        };
        self.pending_project_reload = None;
        let next_window = more && self.limit >= git::HISTORY_MAX;
        let offset = requested_offset.unwrap_or_else(|| {
            if next_window {
                self.log.offset().saturating_add(self.log.commits().len())
            } else if more || preserve {
                self.log.offset()
            } else {
                0
            }
        });
        let limit = if requested_offset.is_some() {
            git::HISTORY_MAX
        } else if next_window {
            git::HISTORY_PAGE
        } else if more {
            (self.limit + git::HISTORY_PAGE).min(git::HISTORY_MAX)
        } else {
            self.limit
        };
        let continuation = if next_window && requested_offset.is_none() {
            match self
                .log
                .project_window()
                .as_ref()
                .map(|page| page.continuation())
                .transpose()
            {
                Ok(cursor) => cursor,
                Err(error) => {
                    self.error = Some(error);
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let fresh = scope.is_none() || (!more && requested_offset.is_none());
        let selected_roots = self.paths_menu.read(cx).selected_roots();
        let structure_paths = self.paths_menu.read(cx).absolute_paths();
        let primary_repo = self.repo.clone();
        let filter = self.text_filter.clone();
        let branches = self.branch_filter.clone();
        let preferred = self.preferred_log_file();
        self.reset_log_presentation();
        let show_long_edges = self.show_long_edges;
        let (stamp, token) = self.log_query.begin(log_query::Kind::History);
        self.log_query.track(
            stamp,
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_spawn(async move {
                        let scope = if fresh {
                            let root_paths = members
                                .roots
                                .iter()
                                .map(|repo| repo.path.clone())
                                .chain(members.errors.iter().map(|(path, _)| path.clone()))
                                .collect::<Vec<_>>();
                            let mapped_paths = if structure_paths.is_empty() {
                                plico::path_filter::project::map(
                                    &root_paths,
                                    &primary_repo.path,
                                    filter
                                        .as_ref()
                                        .map_or(&[], |filter| filter.paths.as_slice()),
                                )?
                            } else {
                                plico::path_filter::project::map_absolute(
                                    &root_paths,
                                    &structure_paths,
                                )?
                            };
                            let queries = members
                                .roots
                                .iter()
                                .filter(|repository| {
                                    mapped_paths.contains_key(&repository.path)
                                        && selected_roots
                                            .as_ref()
                                            .is_none_or(|roots| roots.contains(&repository.path))
                                })
                                .map(|repository| project_log::Query {
                                    repository: repository.clone(),
                                    branches: branches.clone(),
                                    filter: {
                                        let mut per_root = filter.clone().unwrap_or_default();
                                        per_root.paths = mapped_paths[&repository.path].clone();
                                        Some(per_root)
                                    },
                                })
                                .collect::<Vec<_>>();
                            let mut next = project_log::read(&queries, limit, &token)?.scope;
                            Arc::make_mut(&mut next).errors.extend(
                                members
                                    .errors
                                    .iter()
                                    .filter(|(path, _)| {
                                        mapped_paths.contains_key(path)
                                            && selected_roots
                                                .as_ref()
                                                .is_none_or(|roots| roots.contains(path))
                                    })
                                    .cloned(),
                            );
                            next
                        } else {
                            scope.ok_or("Project history scope is unavailable")?
                        };
                        let recovery =
                            fresh.then(|| Self::read_primary_recovery(&primary_repo, &token));
                        let primary =
                            if fresh {
                                Some(primary_repo.snapshot_branches_filtered_at(
                                    &branches, 0, 1, None, None, &token,
                                ))
                            } else {
                                None
                            };
                        let mut page = match continuation {
                            Some(cursor) => cursor.load(limit, &token)?,
                            None => {
                                project_log::navigation::window_at(scope, offset, limit, &token)?
                            }
                        };
                        let model = Arc::new(plico::graph_print::Model::new(&page.rows, &token)?);
                        page.rows = model.project(show_long_edges, &token)?;
                        Ok::<_, String>((page, model, primary, recovery))
                    })
                    .await;
                let _ = this.update(cx, |s, cx| {
                    if !s.log_query.finish(stamp) {
                        return;
                    }
                    match result {
                        Ok((page, model, primary, recovery)) => {
                            if let Some((merge, revert, saved)) = recovery {
                                s.merge_recovery_available = merge;
                                s.revert_recovery_available = revert;
                                s.revert_saved_available = saved;
                            }
                            if let Some(primary) = primary {
                                match primary {
                                    Ok(mut snapshot) => {
                                        snapshot.commits.clear();
                                        s.snapshot = Some(snapshot);
                                        s.record_recent_project();
                                        s.rebuild_branches(cx);
                                    }
                                    Err(error) => {
                                        s.snapshot = None;
                                        s.error = Some(error);
                                    }
                                }
                            }
                            if (preserve || more) && page.offset == s.log.offset() {
                                let after = page
                                    .read
                                    .page
                                    .entries
                                    .iter()
                                    .map(project_log::Entry::identity)
                                    .collect::<Vec<_>>();
                                super::log_viewport::restore(
                                    &s.history_scroll,
                                    &s.commit_ids(),
                                    &after,
                                    page.offset,
                                    log_row_height(cx),
                                );
                            } else if page.offset != s.log.offset() {
                                s.history_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            }
                            s.limit = limit;
                            s.install_project_window(page, Some(model), cx);
                            s.pending_preview_file = preferred;
                            s.reconcile_log_details(cx);
                        }
                        Err(error) => s.error = Some(error),
                    }
                    s.flush_pending_log_details(cx);
                    cx.notify();
                });
            }),
        );
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use gpui::component::Root;
    #[gpui::test]
    fn initial_membership_loads_all_roots_without_primary_snapshot(cx: &mut gpui::TestAppContext) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-initial-project-log-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        let paths = [fixture.0.join("parent"), fixture.0.join("child")];
        for (i, path) in paths.iter().enumerate() {
            std::fs::create_dir_all(path).unwrap();
            for args in [
                vec!["init", "-q", "-b", "main"],
                vec![
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "--allow-empty",
                    "-qm",
                    if i == 0 { "parent-only" } else { "child-only" },
                ],
            ] {
                let out = std::process::Command::new("git")
                    .arg("-C")
                    .arg(path)
                    .args(args)
                    .output()
                    .unwrap();
                assert!(out.status.success(), "{out:?}");
            }
        }
        cx.update(gpui::component::init);
        let catalog = cx.new(|cx| {
            super::super::project_roots::ProjectRoots::new(
                paths.iter().map(Repository::new).collect(),
                cx,
            )
        });
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            view.update(cx, |s, cx| {
                s.repo = Repository::new(&paths[0]);
                s.project_roots = catalog.clone();
                s.bind_project_roots(cx);
                assert!(s.snapshot.is_none());
                s.merge_recovery_available = true;
                s.revert_recovery_available = true;
                s.revert_saved_available = true;
                // Match startup: discovery is asynchronous and may finish before
                // the primary history query has returned a usable snapshot.
                s.project_roots.update(cx, |roots, cx| roots.refresh(cx));
            });
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert!(s.log.project_window().is_some());
            assert_eq!(s.log.commits().len(), 2);
            assert!(s.recent_recorded);
            assert!(!s.merge_recovery_available);
            assert!(!s.revert_recovery_available);
            assert!(!s.revert_saved_available);
            for path in &paths {
                assert!(s.commit_ids().iter().any(|id| id.root.as_ref() == path));
            }
        });
        review.update(cx, |s, cx| {
            s.select_log_row(0, false, false, false, cx);
            s.select_log_row(1, true, false, false, cx);
        });
        cx.run_until_parked();
        let (files, cards) = review.read_with(cx, |s, _| {
            (
                s.files.clone(),
                s.selection_details
                    .iter()
                    .map(|(_, _, view)| view.entity_id())
                    .collect::<Vec<_>>(),
            )
        });
        assert_eq!(cards.len(), 2);
        review.update(cx, |s, cx| s.refresh_impl(false, true, None, cx));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(
                Arc::ptr_eq(&files, &s.files),
                "multi-root refresh must retain the combined file model"
            );
            assert_eq!(
                cards,
                s.selection_details
                    .iter()
                    .map(|(_, _, view)| view.entity_id())
                    .collect::<Vec<_>>()
            );
        });
        // A structure path is anchored in the primary repository, not copied
        // into every sibling root. Selecting that root must retain empty commits.
        review.update(cx, |s, cx| {
            s.text_filter = Some(git::LogFilter {
                paths: vec![".".into()],
                ..Default::default()
            });
            s.refresh_log_filter(cx);
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.log.commits().len(), 1);
            assert_eq!(s.log_repository(0).unwrap().path, paths[0]);
            assert_eq!(s.log.commits()[0].subject, "parent-only");
            assert_eq!(s.history_count.total, Some(1));
        });
        review.update(cx, |s, cx| {
            s.text_filter = Some(git::LogFilter {
                text: "child-only".into(),
                ..Default::default()
            });
            s.refresh_log_filter(cx);
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.log.commits().len(), 1);
            assert_eq!(s.log_repository(0).unwrap().path, paths[1]);
            assert_eq!(s.log.commits()[0].subject, "child-only");
        });
        // The path editor/tree emits absolute selections, including sibling
        // repositories outside the primary root. Clearing text must not lose it.
        review.update(cx, |s, cx| {
            s.paths_menu
                .update(cx, |menu, cx| menu.set_paths(vec![paths[1].clone()], cx));
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.log.commits().len(), 1);
            assert_eq!(s.log_repository(0).unwrap().path, paths[1]);
            assert_eq!(s.history_count.total, Some(1));
        });
        // Exercise the menu's production event route, including exact empty
        // selection. Counts must use the same root scope as visible commits.
        review.update(cx, |s, cx| {
            s.paths_menu
                .update(cx, |menu, cx| menu.choose_root(paths[1].clone(), true, cx));
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert_eq!(s.log.commits().len(), 1);
            assert_eq!(s.log_repository(0).unwrap().path, paths[1]);
            assert_eq!(s.history_count.total, Some(1));
        });
        review.update(cx, |s, cx| {
            s.paths_menu
                .update(cx, |menu, cx| menu.choose_root(paths[1].clone(), false, cx));
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert!(s.log.commits().is_empty());
            assert_eq!(s.history_count.total, Some(0));
            assert!(!s.history_has_more());
            assert_eq!(s.empty_diff_message(), "Select commits to view changes");
        });
        review.update(cx, |s, cx| {
            s.paths_menu.update(cx, |menu, cx| menu.clear_filter(cx));
            s.apply_text_filter(cx);
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert_eq!(s.log.commits().len(), 2);
            assert_eq!(s.history_count.total, Some(2));
        });
    }

    #[gpui::test]
    fn unloaded_child_target_keeps_identity_and_primary_repository_state(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let name = format!(
            "plico-project-navigation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let fixture = Fixture(std::env::temp_dir().join(name));
        let parent = fixture.0.join("parent");
        let child = fixture.0.join("child");
        std::fs::create_dir_all(&parent).unwrap();
        let git = |path: &std::path::Path, args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
        };
        git(&parent, &["init", "-q"]);
        std::fs::write(parent.join("file.txt"), "shared\n").unwrap();
        git(&parent, &["add", "file.txt"]);
        git(
            &parent,
            &[
                "-c",
                "user.name=Navigation",
                "-c",
                "user.email=navigation@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "shared",
            ],
        );
        git(
            &fixture.0,
            &[
                "clone",
                "-q",
                parent.to_str().unwrap(),
                child.to_str().unwrap(),
            ],
        );
        git(&child, &["branch", "-m", "child-review"]);
        let queries = [&parent, &child].map(|path| project_log::Query {
            repository: Repository::new(path),
            branches: vec![],
            filter: None,
        });
        let token = Cancel::default();
        let read = project_log::read(&queries, 1, &token).unwrap();
        let scope = read.scope.clone();
        let primary = queries[0]
            .repository
            .snapshot(None, 1, None, &token)
            .unwrap();
        let head = primary.head_id.clone();
        // A clone shares hashes but owns different reference names. Keeping
        // only the primary hash map would silently label the child incorrectly.
        let parent_titles = &scope
            .roots
            .iter()
            .find(|r| r.query.repository.path == parent)
            .unwrap()
            .ref_titles;
        let child_titles = &scope
            .roots
            .iter()
            .find(|r| r.query.repository.path == child)
            .unwrap()
            .ref_titles;
        let hash = head.as_ref().unwrap();
        assert_ne!(parent_titles.get(hash), child_titles.get(hash));
        assert!(child_titles.get(hash).unwrap().contains("child-review"));
        let target = CommitId {
            root: Arc::new(child.clone()),
            hash: head.clone().unwrap(),
        };
        cx.update(gpui::component::init);
        let catalog = cx.new(|cx| {
            super::super::project_roots::ProjectRoots::new(
                queries
                    .iter()
                    .map(|query| query.repository.clone())
                    .collect(),
                cx,
            )
        });
        cx.run_until_parked();
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 1, None, window, cx));
            view.update(cx, |s, cx| {
                s.repo = queries[0].repository.clone();
                s.project_roots = catalog.clone();
                s.bind_project_roots(cx);
                s.log = log_display::Display::fixture_single(&s.repo.path, primary.commits.clone());

                s.snapshot = Some(primary);
                s.log = log_display::Display::fixture(
                    s.log
                        .commits()
                        .iter()
                        .cloned()
                        .map(|commit| plico::project_log::Entry {
                            root: Arc::new(s.repo.path.clone()),
                            commit,
                            display_parents: None,
                        })
                        .collect(),
                    Some(scope),
                );
                s.navigate_log_commit("child".into(), target.clone(), false, false, window, cx);
            });
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.repo.path, parent);
            assert_eq!(s.snapshot.as_ref().unwrap().head_id, head);
            assert_eq!(s.log.offset(), 1);
            assert_eq!(s.selected, Some(0));
            assert_eq!(s.commit_ids(), vec![target.clone()]);
            assert_eq!(s.log_repository(0).unwrap().path, child);
            assert!(s.log.project_window().is_some());
            assert_eq!(s.files.len(), 1);
            assert_eq!(s.files[0].path, std::path::Path::new("file.txt"));
        });
        // Invalidation arriving during unsuccessful navigation must reach the
        // retained child selection through the actual async completion route.
        let orphan = std::process::Command::new("git")
            .arg("-C")
            .arg(&child)
            .args([
                "-c",
                "user.name=Navigation",
                "-c",
                "user.email=navigation@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit-tree",
                "HEAD^{tree}",
                "-m",
                "unreachable",
            ])
            .output()
            .unwrap();
        assert!(orphan.status.success(), "{orphan:?}");
        let orphan = String::from_utf8(orphan.stdout).unwrap().trim().to_owned();
        for outside in [true, false] {
            let old_files = review.read_with(cx, |s, _| s.files.clone());
            cx.update(|window, cx| {
                review.update(cx, |s, cx| {
                    let missing = CommitId {
                        root: Arc::new(if outside {
                            child.clone()
                        } else {
                            fixture.0.join("outside")
                        }),
                        hash: if outside {
                            orphan.clone()
                        } else {
                            "1111111111111111111111111111111111111111".into()
                        },
                    };
                    s.navigate_project_commit("missing".into(), missing, false, false, window, cx);
                    let stamp = s.log_query.stamp();
                    s.details_cache.invalidate_repository(&child);
                    s.invalidate_loaded_details(&child);
                    s.refresh_diff_configuration_details(&child, cx);
                    assert!(s.pending_log_details_refresh);
                    assert!(s.log_query.is_current(stamp));
                });
            });
            cx.run_until_parked();
            review.read_with(cx, |s, _| {
                assert!(!s.pending_log_details_refresh);
                assert!(!s.log_query.running());
                assert!(s.loaded_log_selection.is_some());
                assert_eq!(s.commit_ids(), vec![target.clone()]);
                assert_eq!(s.selected, Some(0));
                assert_eq!(s.files.len(), 1);
                assert!(
                    !Arc::ptr_eq(&old_files, &s.files),
                    "details must have reloaded"
                );
                if outside {
                    assert!(s.error.is_none(), "{:?}", s.error);
                    assert_eq!(s.navigation_notice.as_ref().unwrap().name, "missing");
                } else {
                    assert_eq!(
                        s.error.as_deref(),
                        Some("Commit repository is not in the loaded history")
                    );
                }
            });
        }
        // Failure keeps the child page and selection; retry uses the fixed scope.
        std::fs::rename(child.join(".git"), child.join(".git-saved")).unwrap();
        review.update(cx, |s, cx| s.previous_window(cx));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_some());
            assert_eq!(s.commit_ids(), vec![target.clone()]);
            assert_eq!(s.log.offset(), 1);
        });
        std::fs::rename(child.join(".git-saved"), child.join(".git")).unwrap();
        git(
            &parent,
            &[
                "-c",
                "user.name=Navigation",
                "-c",
                "user.email=navigation@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-qm",
                "new after scope",
            ],
        );
        review.update(cx, |s, cx| s.previous_window(cx));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.log.offset(), 0);
            assert_eq!(
                s.log.commits().len(),
                2,
                "previous window retains fixed tips"
            );
            assert_eq!(s.commit_ids()[1], target);
            assert!(!s.history_has_more());
        });
        review.update(cx, |s, cx| s.refresh(false, cx));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(
                s.log.commits().len(),
                3,
                "explicit refresh resolves new tips"
            );
            assert_ne!(s.snapshot.as_ref().unwrap().head_id, head);
            assert_eq!(s.log.scope().as_ref().unwrap().roots.len(), 2);
        });
        review.update(cx, |s, cx| {
            s.limit = 1;
            s.reload_project_history(false, false, None, cx);
        });
        cx.run_until_parked();
        review.update(cx, |s, cx| {
            assert_eq!(s.log.commits().len(), 1);
            assert!(s.history_has_more());
            s.refresh(true, cx);
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.log.commits().len(), 3);
            assert!(!s.history_has_more());
            assert!(s.commit_ids().contains(&target));
            assert_eq!(s.log.scope().as_ref().unwrap().roots.len(), 2);
        });
        let (completed_rows, completed_scope, completed_total) = review.read_with(cx, |s, _| {
            assert!(!s.log_query.running());
            (
                s.log.commits().clone(),
                s.log.scope().clone().unwrap(),
                s.history_count.total,
            )
        });
        catalog.update(cx, |roots, cx| roots.refresh(cx));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(
                Arc::ptr_eq(&s.log.commits(), &completed_rows),
                "unchanged membership must not reload completed rows"
            );
            assert!(Arc::ptr_eq(
                s.log.scope().as_ref().unwrap(),
                &completed_scope
            ));
            assert_eq!(s.history_count.total, completed_total);
            assert!(s.pending_project_reload.is_none());
            assert!(!s.log_query.running());
        });
        let (stamp, old_token) =
            review.update(cx, |s, _| s.log_query.begin(log_query::Kind::Navigation));
        catalog.update(cx, |roots, cx| roots.refresh(cx));
        cx.run_until_parked();
        assert!(old_token.cancelled());
        review.update(cx, |s, _| {
            assert!(!s.log_query.finish(stamp));
            assert!(s.pending_project_reload.is_none());
            assert!(
                !Arc::ptr_eq(&s.log.commits(), &completed_rows),
                "interrupted query needs a replacement"
            );
        });
        std::fs::rename(child.join(".git"), child.join(".git-saved")).unwrap();
        review.update(cx, |s, cx| {
            s.refresh(true, cx);
            s.project_roots.update(cx, |roots, cx| roots.refresh(cx));
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.pending_project_reload.is_none());
            assert!(s.commit_ids().iter().all(|id| *id.root == parent));
            assert_eq!(s.log.scope().as_ref().unwrap().roots.len(), 1);
            assert_eq!(s.log.scope().as_ref().unwrap().errors.len(), 1);
        });
        std::fs::rename(child.join(".git-saved"), child.join(".git")).unwrap();
        catalog.update(cx, |roots, cx| roots.refresh(cx));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert_eq!(s.log.scope().as_ref().unwrap().roots.len(), 2);
            assert!(s.log.scope().as_ref().unwrap().errors.is_empty());
            assert_eq!(s.log.commits().len(), 3);
            assert!(s.commit_ids().contains(&target));
        });
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.limit = 1;
                s.navigate_log_commit("child".into(), target.clone(), false, true, window, cx);
            });
        });
        cx.run_until_parked();
        review.update(cx, |s, cx| {
            assert!(s.log.offset() > 0);
            s.project_roots.update(cx, |roots, cx| roots.refresh(cx));
            s.text_filter = Some(git::LogFilter {
                text: "shared".into(),
                ..Default::default()
            });
            s.refresh_log_filter(cx);
            assert_eq!(s.pending_project_reload, Some(false));
        });
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(
                s.log.offset(),
                0,
                "filter change during discovery must start at the first matching row"
            );
            assert_eq!(s.log.commits().len(), 1);
            assert_eq!(s.log.commits()[0].subject, "shared");
            assert!(s.pending_project_reload.is_none());
        });
    }
}
