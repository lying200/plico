//! Each comparison range owns its lazily created, disposable Diff preview.
use super::*;

impl Pane {
    pub(super) fn sync_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.show_preview {
            if let Some(preview) = self.preview.take() {
                preview.update(cx, |p, cx| p.deactivate(cx));
            }
            return;
        }
        let retained = self.preview.as_ref().map(|p| p.read(cx).target.clone());
        let Some((target, chain)) = self.navigation_selection(retained.as_ref()) else {
            if let Some(preview) = self.preview.take() {
                preview.update(cx, |p, cx| p.deactivate(cx));
            }
            return;
        };
        let same_target = retained.is_some_and(|old| old.identity() == target.identity());
        if same_target
            && self.preview_group_files == self.group_files
            && self.preview_folder == self.selected_folder
        {
            return;
        }
        self.preview_group_files = self.group_files;
        self.preview_folder = self.selected_folder.clone();
        if same_target {
            self.preview
                .as_ref()
                .unwrap()
                .update(cx, |p, cx| p.set_navigation_chain(chain, cx));
            return;
        }
        if let Some(preview) = self.preview.take() {
            preview.update(cx, |p, cx| p.deactivate(cx));
        }
        let preview = cx.new(|_| DiffTab::preview(target, chain).with_file_navigation());
        self.track_diff_navigation(&preview, cx);
        preview.update(cx, |p, cx| p.activate_with_focus(false, window, cx));
        self.preview = Some(preview);
    }
    /// The same tree selection drives embedded and independent previews.
    pub(super) fn navigation_selection(
        &self,
        retained: Option<&diff_target::Target>,
    ) -> Option<(diff_target::Target, diff_target::Chain)> {
        let mut chain = self.chain.clone()?;
        let order = self.file_navigation_order();
        let retained = retained
            .and_then(|target| chain.fork_for(target))
            .map(|c| c.index());
        let index = self.selected_file.or_else(|| {
            if let Some(folder) = &self.selected_folder {
                retained
                    .filter(|&i| chain.changes()[i].path.starts_with(folder))
                    .or_else(|| {
                        order
                            .iter()
                            .copied()
                            .find(|&i| chain.changes()[i].path.starts_with(folder))
                    })
            } else {
                retained.or_else(|| order.first().copied())
            }
        })?;
        let target = chain.select(index)?;
        let selected = self.selected_folder.as_ref().map(|folder| {
            order
                .iter()
                .copied()
                .filter(|&i| chain.changes()[i].path.starts_with(folder))
                .collect::<Vec<_>>()
        });
        let order = selected
            .filter(|indices| indices.len() > 1)
            .unwrap_or(order);
        Some((target, chain.subset(&order)?))
    }

    pub(super) fn track_diff_navigation(&self, tab: &Entity<DiffTab>, cx: &mut Context<Self>) {
        cx.subscribe(tab, |s, _, event: &crate::diff_tab::FileChanged, cx| {
            s.select_diff_target(&event.0, cx);
        })
        .detach();
    }

    fn select_diff_target(&mut self, target: &diff_target::Target, cx: &mut Context<Self>) {
        if let Some(chain) = self.chain.as_ref().and_then(|c| c.fork_for(target)) {
            let keep_folder = self.selected_folder.as_ref().is_some_and(|folder| {
                target.change().path.starts_with(folder)
                    && chain
                        .changes()
                        .iter()
                        .filter(|c| c.path.starts_with(folder))
                        .take(2)
                        .count()
                        > 1
            });
            self.chain = Some(chain.clone());
            if keep_folder {
                // Navigating within a multi-file selection must not replace it.
                cx.notify();
                return;
            }
            self.selected_file = Some(chain.index());
            self.selected_folder = None;
            for parent in target.change().path.ancestors().skip(1) {
                self.collapsed_files.remove(parent);
            }
            self.rebuild_file_rows();
            if let Some(row) = self.active_file_row() {
                self.files_scroll.scroll_to_item(row, ScrollStrategy::Top);
            }
            cx.notify();
        }
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;

    #[gpui::test]
    fn independent_preview_is_reused_reopened_and_closed_with_source(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                s.open_branch_comparison(
                    Pair {
                        first_ref: "topic".into(),
                        second_ref: "main".into(),
                        first: "a".repeat(40),
                        second: "b".repeat(40),
                    },
                    window,
                    cx,
                );
            })
        });
        cx.run_until_parked();
        let source = review.read_with(cx, |s, _| s.branch_compare_tabs[0].clone());
        let pane = source.read_with(cx, |s, _| s.panes[0].clone());
        pane.update(cx, |s, _| {
            use plico::combined_changes::{Change, Revision};
            let changes = ["a.txt", "b.txt"].map(|path| Change {
                before: None,
                after: Some(Revision {
                    revision: "a".repeat(40),
                    path: path.into(),
                }),
            });
            s.chain = Some(
                diff_target::Chain::combined(s.repo.clone(), changes.to_vec(), 0)
                    .unwrap()
                    .1,
            );
            s.rebuild_file_rows();
        });
        pane.update(cx, |s, cx| s.open_file(0, cx));
        cx.run_until_parked();
        let first = review.read_with(cx, |s, _| {
            assert_eq!(s.tabs.len(), 1);
            s.tabs[0].entity_id()
        });
        pane.update(cx, |s, cx| s.open_file(1, cx));
        cx.run_until_parked();
        review.read_with(cx, |s, cx| {
            assert_eq!(s.tabs.len(), 1);
            assert_eq!(s.tabs[0].entity_id(), first);
            assert_eq!(
                s.tabs[0].read(cx).target.change().path,
                PathBuf::from("b.txt")
            );
        });
        cx.update(|window, cx| {
            review.update(cx, |s, cx| s.close_diff_tab_from_header(0, window, cx))
        });
        pane.update(cx, |s, cx| s.open_file(0, cx));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert_eq!(s.tabs.len(), 1);
            assert_ne!(s.tabs[0].entity_id(), first);
        });
        cx.update(|window, cx| {
            review.update(cx, |s, cx| {
                let id = source.entity_id();
                s.close_editor_ids(vec![id], Some(id), window, cx);
                assert!(s.branch_compare_tabs.is_empty());
                assert!(s.tabs.is_empty());
                assert!(s.editor_tabs.order().is_empty());
            })
        });
    }

    #[gpui::test]
    fn folder_navigation_keeps_selection_and_excludes_siblings(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut pane = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| {
                Pane::new(
                    Repository::new("missing-folder-preview-fixture"),
                    Pair {
                        first_ref: "topic".into(),
                        second_ref: "main".into(),
                        first: "a".repeat(40),
                        second: "b".repeat(40),
                    },
                    Side::First,
                    window,
                    cx,
                )
            });
            pane = Some(view.clone());
            Root::new(view, window, cx)
        });
        let pane = pane.unwrap();
        let preview = cx.update(|window, cx| {
            pane.update(cx, |s, cx| {
                use plico::combined_changes::{Change, Revision};
                let changes = ["outside.txt", "nested/a.txt", "nested/b.txt"].map(|path| Change {
                    before: None,
                    after: Some(Revision {
                        revision: "a".repeat(40),
                        path: path.into(),
                    }),
                });
                s.chain = Some(
                    diff_target::Chain::combined(s.repo.clone(), changes.to_vec(), 0)
                        .unwrap()
                        .1,
                );
                s.selected_file = None;
                s.selected_folder = Some(PathBuf::from("nested"));
                s.collapsed_files.insert(PathBuf::from("nested"));
                s.rebuild_file_rows();
                s.show_preview = true;
                s.sync_preview(window, cx);
                let preview = s.preview.as_ref().unwrap().clone();
                let tab = preview.read(cx);
                assert_eq!(tab.target.change().path, PathBuf::from("nested/a.txt"));
                assert_eq!(tab.fork_chain(&tab.target).unwrap().len(), 2);
                preview
            })
        });
        cx.update(|window, cx| preview.update(cx, |p, cx| p.move_file(true, window, cx)));
        cx.run_until_parked();
        pane.read_with(cx, |s, cx| {
            assert_eq!(
                s.selected_folder.as_deref(),
                Some(std::path::Path::new("nested"))
            );
            assert!(s.selected_file.is_none());
            assert!(s.collapsed_files.contains(std::path::Path::new("nested")));
            let tab = preview.read(cx);
            assert_eq!(tab.target.change().path, PathBuf::from("nested/b.txt"));
            assert!(tab.fork_chain(&tab.target).unwrap().step(true).is_none());
        });
        cx.update(|window, cx| {
            pane.update(cx, |s, cx| {
                s.sync_preview(window, cx);
                assert_eq!(s.preview.as_ref().unwrap().entity_id(), preview.entity_id());
                s.selected_folder = None;
                s.selected_file = Some(2);
                s.sync_preview(window, cx);
                let tab = preview.read(cx);
                assert_eq!(tab.target.change().path, PathBuf::from("nested/b.txt"));
                let mut chain = tab.fork_chain(&tab.target).unwrap();
                assert_eq!(chain.len(), 3);
                assert_eq!(
                    chain.step(true).unwrap().change().path,
                    PathBuf::from("outside.txt")
                );
            })
        });

        // Exercise the OpenDiff event, not a separately constructed navigation chain.
        let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
        let captured = opened.clone();
        pane.update(cx, |s, cx| {
            cx.subscribe(&cx.entity(), move |s, _, event: &OpenDiff, cx| {
                let tab = cx.new(|_| DiffTab::new(event.0.clone(), event.1.clone()));
                s.track_diff_navigation(&tab, cx);
                *captured.borrow_mut() = Some(tab);
            })
            .detach();
            s.selected_folder = None;
            s.selected_file = Some(0);
            s.collapsed_files.insert(PathBuf::from("nested"));
            s.open_file(0, cx);
        });
        cx.run_until_parked();
        let tab = opened.borrow().clone().expect("OpenDiff must create a tab");
        tab.read_with(cx, |p, _| {
            assert_eq!(p.target.change().path, PathBuf::from("outside.txt"));
            assert_eq!(p.fork_chain(&p.target).unwrap().index(), 2);
        });
        cx.update(|window, cx| tab.update(cx, |p, cx| p.move_file(false, window, cx)));
        cx.run_until_parked();
        pane.read_with(cx, |s, cx| {
            assert_eq!(s.selected_file, Some(2));
            assert!(s.selected_folder.is_none());
            assert!(!s.collapsed_files.contains(std::path::Path::new("nested")));
            assert_eq!(
                tab.read(cx).target.change().path,
                PathBuf::from("nested/b.txt")
            );
        });
        cx.update(|window, cx| {
            let selection = pane.update(cx, |s, cx| {
                s.selected_file = Some(1);
                s.selected_folder = None;
                s.navigation_selection(Some(&tab.read(cx).target))
            });
            tab.update(cx, |t, cx| {
                t.sync_source_selection(selection, false, window, cx)
            });
        });
        tab.read_with(cx, |t, _| {
            assert_eq!(t.target.change().path, PathBuf::from("nested/a.txt"));
            assert_eq!(t.fork_chain(&t.target).unwrap().len(), 3);
        });
        cx.update(|window, cx| {
            let selection = pane.update(cx, |s, cx| {
                s.selected_file = None;
                s.selected_folder = Some(PathBuf::from("nested"));
                s.navigation_selection(Some(&tab.read(cx).target))
            });
            tab.update(cx, |t, cx| {
                t.sync_source_selection(selection, false, window, cx)
            });
        });
        tab.read_with(cx, |t, _| {
            assert_eq!(t.target.change().path, PathBuf::from("nested/a.txt"));
            assert_eq!(t.fork_chain(&t.target).unwrap().len(), 2);
        });
        let old_target = tab.read_with(cx, |p, _| p.target.clone());
        pane.update(cx, |s, cx| {
            use plico::combined_changes::{Change, Revision};
            let replacement = Change {
                before: None,
                after: Some(Revision {
                    revision: "b".repeat(40),
                    path: "nested/b.txt".into(),
                }),
            };
            s.chain = Some(
                diff_target::Chain::combined(s.repo.clone(), vec![replacement], 0)
                    .unwrap()
                    .1,
            );
            s.selected_file = Some(0);
            s.select_diff_target(&old_target, cx);
            assert_eq!(
                s.selected_file,
                Some(0),
                "old commit events must not alter the replacement selection"
            );
            assert_eq!(s.chain.as_ref().unwrap().len(), 1);
        });
    }
}
