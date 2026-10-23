//! Explicit Project-tree navigation; never opens an editor or scans closed siblings.
use super::*;
impl ProjectView {
    pub(super) fn open_repository(&mut self, relative: PathBuf, cx: &mut Context<Self>) {
        self.cancel_reveal();
        self.cancel_preview();
        self.token.cancel();
        self.token = Cancel::default();
        self.generation += 1;
        self.loading = Some(relative.clone());
        self.error = None;
        let (root, token, generation) = (self.root.clone(), self.token.clone(), self.generation);
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    Project::new(root)
                        .map_err(|e| format!("{e:?}"))?
                        .repository_at(&relative, &token)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.generation != generation {
                    return;
                }
                s.loading = None;
                match result {
                    Ok(path) => cx.emit(OpenRepository(path)),
                    Err(error) => s.error = Some(Notice::Io(error)),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    pub(crate) fn set_project_showing(&mut self, showing: bool) {
        if self.project_showing == showing {
            return;
        }
        self.project_showing = showing;
        self.cancel_reveal();
        self.follow_pending = showing && self.auto_follow;
    }
    pub(super) fn toggle_auto_follow(&mut self, cx: &mut Context<Self>) {
        let enabled = !self.auto_follow;
        if let Err(error) = plico::layout_preferences::save_project_auto_follow(enabled) {
            self.error = Some(Notice::Io(format!(
                "{}: {error}",
                t("Settings could not be saved")
            )));
        } else {
            self.cancel_reveal();
            self.auto_follow = enabled;
            self.follow_pending = enabled && self.project_showing;
        }
        cx.notify();
    }
    pub(super) fn schedule_follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.follow_pending) {
            return;
        }
        if !self.auto_follow || !self.project_showing || self.focus.contains_focused(window, cx) {
            return;
        }
        self.follow_task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let _ = this.update_in(cx, |s, window, cx| {
                if !s.auto_follow || !s.project_showing || s.focus.contains_focused(window, cx) {
                    return;
                }
                let Some(path) = s.current_source.as_ref() else {
                    return;
                };
                if let Some(index) = s
                    .rows
                    .iter()
                    .position(|(e, _, more)| !more && &e.path == path)
                {
                    s.active = index;
                    s.restore_selection = None;
                    s.scroll.scroll_to_item(index, ScrollStrategy::Center);
                    cx.notify();
                } else {
                    s.reveal_source(false, window, cx);
                }
            });
        });
    }

    pub(crate) fn set_current_source(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.current_source != path {
            self.cancel_reveal();
            self.current_source = path;
            self.follow_pending = self.auto_follow && self.project_showing;
            cx.notify();
        }
    }
    pub(super) fn cancel_reveal(&mut self) {
        self.follow_pending = false;
        self.follow_task = Task::ready(());
        if self.revealing {
            self.revealing = false;
            self.token.cancel();
            self.generation += 1;
            self.task = Task::ready(());
            self.loading = None;
        }
    }
    pub(super) fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.cancel_reveal();
        self.cancel_preview();
        self.token.cancel();
        self.generation += 1;
        self.task = Task::ready(());
        self.loading = None;
        self.refresh_restore = None;
        self.restore_selection = None;
        self.error = None;
        // Keep the project root open, retaining its paging continuation.
        self.directories
            .retain(|path, _| path.as_os_str().is_empty());
        self.cursors.retain(|path, _| path.as_os_str().is_empty());
        self.rebuild();
        self.scroll.scroll_to_item(self.active, ScrollStrategy::Top);
        cx.notify();
    }
    pub(super) fn reveal_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_reveal();
        self.reveal_source(true, window, cx);
    }
    fn reveal_source(&mut self, manual: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.current_source.clone() else {
            return;
        };
        self.cancel_preview();
        self.token.cancel();
        self.generation += 1;
        self.token = Cancel::default();
        self.revealing = true;
        self.refresh_restore = None;
        self.restore_selection = None;
        self.loading = Some(path.clone());
        self.error = None;
        if manual {
            self.focus.focus(window, cx);
        }
        let (root, token, generation) = (self.root.clone(), self.token.clone(), self.generation);
        self.task = cx.spawn_in(window, async move |this, cx| {
            let query = path.clone();
            let result = cx
                .background_spawn(async move { Project::new(root)?.reveal_file(&query, &token) })
                .await;
            let _ = this.update_in(cx, |s, window, cx| {
                if generation != s.generation {
                    return;
                }
                s.revealing = false;
                s.loading = None;
                if !manual
                    && (!s.auto_follow
                        || !s.project_showing
                        || s.focus.contains_focused(window, cx))
                {
                    if std::mem::take(&mut s.external_refresh_pending) {
                        s.refresh(cx);
                    }
                    cx.notify();
                    return;
                }
                match result {
                    Ok(mut tree) => {
                        // Preserve an already longer cached page when it contains
                        // the required child; locating must not shrink that directory.
                        tree.directories.retain(|parent, entries| {
                            let keep_old = s.directories.get(parent).is_some_and(|old| {
                                old.len() > entries.len()
                                    && old.iter().any(|e| {
                                        e.path == path
                                            || (e.kind == Kind::Directory
                                                && path.starts_with(&e.path))
                                    })
                            });
                            !keep_old
                        });
                        let size: usize = s
                            .directories
                            .iter()
                            .filter(|(p, _)| !tree.directories.contains_key(*p))
                            .map(|(_, e)| e.len())
                            .sum::<usize>()
                            + tree.directories.values().map(Vec::len).sum::<usize>();
                        if size > project_files::DISPLAY_NODE_LIMIT {
                            s.error = Some("Collapse folders before opening more files".into());
                        } else {
                            for (parent, entries) in tree.directories {
                                s.cursors.remove(&parent);
                                if let Some(cursor) = tree.cursors.remove(&parent) {
                                    s.cursors.insert(parent.clone(), cursor);
                                }
                                s.directories.insert(parent, entries);
                            }
                            s.rebuild();
                            s.restore_path(Some((path, false)));
                            s.scroll.scroll_to_item(s.active, ScrollStrategy::Center);
                        }
                    }
                    Err(project_files::Error::InvalidPath | project_files::Error::NotFile) => {
                        s.error = Some("File is not available in the project tree".into());
                    }
                    Err(error) => s.error = Some(directory_error_text(error)),
                }
                if std::mem::take(&mut s.external_refresh_pending) {
                    s.refresh(cx);
                }
                cx.notify();
            });
        });
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    #[gpui::test]
    fn auto_follow_uses_cached_rows_without_focus_or_io_and_respects_manual_selection(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut view = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let project = cx.new(|cx| ProjectView::new(PathBuf::from("/missing-follow-test"), cx));
            view = Some(project.clone());
            gpui::component::Root::new(project, window, cx)
        });
        cx.update(|window, cx| {
            view.as_ref().unwrap().update(cx, |s, cx| {
                s.token.cancel();
                s.task = Task::ready(());
                s.loading = None;
                s.directories.insert(
                    PathBuf::new(),
                    vec![Entry {
                        path: "file.rs".into(),
                        kind: Kind::File,
                    }],
                );
                s.rebuild();
                s.active = 0;
                s.auto_follow = true;
                s.set_project_showing(true);
                // A separate editor holds focus.
                cx.focus_handle().focus(window, cx);
                s.set_current_source(Some("file.rs".into()), cx);
                s.schedule_follow(window, cx);
            });
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(150));
        cx.run_until_parked();
        cx.update(|window, cx| {
            view.as_ref().unwrap().update(cx, |s, cx| {
                assert_eq!(s.rows[s.active].0.path, Path::new("file.rs"));
                assert!(!s.focus.is_focused(window));
                assert!(s.loading.is_none());
                s.set_current_source(Some("other.rs".into()), cx);
                s.schedule_follow(window, cx);
                s.select(0);
                assert!(!s.follow_pending);
            });
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(150));
        cx.run_until_parked();
        view.as_ref().unwrap().read_with(cx, |s, _| {
            assert_eq!(s.active, 0);
            assert!(s.loading.is_none());
        });
        cx.update(|window, cx| {
            view.unwrap().update(cx, |s, cx| {
                s.focus.focus(window, cx);
                s.set_current_source(Some("file.rs".into()), cx);
                s.schedule_follow(window, cx);
                assert!(!s.follow_pending);
                assert_eq!(s.active, 0);
                s.set_project_showing(false);
            });
        });
    }
    #[gpui::test]
    fn collapse_and_manual_selection_cancel_pending_reveal(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut view = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let project = cx.new(|cx| ProjectView::new(PathBuf::from("/missing-test-project"), cx));
            view = Some(project.clone());
            gpui::component::Root::new(project, window, cx)
        });
        cx.update(|window, cx| {
            view.as_ref().unwrap().update(cx, |s, cx| {
                s.token.cancel();
                s.task = Task::ready(());
                s.loading = None;
                s.directories.insert(
                    PathBuf::new(),
                    vec![Entry {
                        path: "src".into(),
                        kind: Kind::Directory,
                    }],
                );
                s.directories.insert(
                    "src".into(),
                    vec![Entry {
                        path: "src/file.rs".into(),
                        kind: Kind::File,
                    }],
                );
                s.rebuild();
                s.active = 2;
                s.set_current_source(Some("src/file.rs".into()), cx);
                s.reveal_current(window, cx);
                let old = s.token.clone();
                s.collapse_all(cx);
                assert!(old.cancelled());
                assert!(!s.revealing);
                assert_eq!(s.directories.len(), 1);
                assert_eq!(s.rows[s.active].0.path, Path::new("src"));
                s.reveal_current(window, cx);
                let generation = s.generation;
                s.select(0);
                assert!(s.generation > generation);
                assert!(!s.revealing);
                assert!(s.loading.is_none());
            })
        });
        cx.run_until_parked();
        view.unwrap().read_with(cx, |s, _| assert_eq!(s.active, 0));
    }
}
