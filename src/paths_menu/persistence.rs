//! Background persistence owned by this menu, never by a loaded Log page.
use super::*;
use plico::path_filter::store::{State, Store, StoredPath};

impl PathsMenu {
    pub(crate) fn enable_persistence(&mut self, cx: &mut Context<Self>) {
        self.persistent = true;
        self.restore_version += 1;
        self.pending_restore = None;
        self.persistence_error = None;
        self.store = Store::for_project(&self.root);
        self.start_restore(cx);
    }
    fn start_restore(&mut self, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        let version = self.restore_version;
        self.restore_task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { store.load() }).await;
            let _ = this.update(cx, |s, cx| {
                if version != s.restore_version {
                    return;
                }
                match result {
                    Ok(state) => {
                        s.pending_restore = Some(state);
                        s.apply_restored(cx);
                    }
                    Err(error) => {
                        s.persistence_error =
                            Some(format!("{}: {error}", t("Could not restore Log filters")));
                        cx.notify();
                    }
                }
            });
        });
    }
    pub(super) fn apply_restored(&mut self, cx: &mut Context<Self>) {
        // Discovery is authoritative, not a filtered or partially loaded Log.
        if self.roots.entries.is_empty() {
            return;
        }
        let Some(mut state) = self.pending_restore.take() else {
            return;
        };
        state.restore_roots(
            &self
                .roots
                .entries
                .iter()
                .map(|(p, _)| p.clone())
                .collect::<Vec<_>>(),
        );
        let paths = state
            .paths
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        let changed = self.paths != paths || self.roots.selected != state.roots;
        self.directory_paths = state
            .paths
            .iter()
            .filter(|entry| entry.directory)
            .map(|entry| entry.path.clone())
            .collect();
        self.paths = paths;
        self.roots.selected = state.roots;
        if changed {
            cx.emit(Changed);
        }
        cx.notify();
    }
    pub(super) fn persist(&mut self, cx: &mut Context<Self>) {
        // An explicit edit wins even if the old read has not returned yet.
        self.restore_version += 1;
        self.pending_restore = None;
        self.persistence_error = None;
        let Some(store) = self.store.clone() else {
            return;
        };
        let state = State {
            roots: self.roots.selected.clone(),
            paths: self
                .paths
                .iter()
                .map(|path| StoredPath {
                    path: path.clone(),
                    directory: self.directory_paths.contains(path),
                })
                .collect(),
        };
        let revision = store.reserve();
        let version = self.restore_version;
        // Detached so closing the view does not discard an accepted preference.
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { store.save(revision, &state) })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |s, cx| {
                    if s.restore_version == version {
                        s.persistence_error =
                            Some(format!("{}: {error}", t("Could not save Log filters")));
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;
    #[gpui::test]
    fn restoration_waits_for_membership_and_user_edits_win(cx: &mut gpui::TestAppContext) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(
            format!("plico-filter-restore-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
        ));
        let primary = fixture.0.join("main");
        let child = fixture.0.join("child");
        let store = Store::new(fixture.0.join("filter.json"), &primary);
        let state = State {
            roots: Some([child.clone()].into()),
            paths: vec![],
        };
        store.save(store.reserve(), &state).unwrap();
        cx.update(gpui::component::init);
        let mut menu = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| PathsMenu::new(primary.clone(), cx));
            view.update(cx, |s, cx| {
                s.store = Some(store.clone());
                s.start_restore(cx);
            });
            menu = Some(view.clone());
            Root::new(view, window, cx)
        });
        let menu = menu.unwrap();
        cx.run_until_parked();
        menu.read_with(cx, |s, _| {
            assert!(s.pending_restore.is_some());
            assert!(!s.has_filter());
        });
        menu.update(cx, |s, cx| {
            s.set_roots(vec![(primary.clone(), 0), (child.clone(), 1)], cx)
        });
        menu.read_with(cx, |s, _| assert_eq!(s.selected_roots(), state.roots));
        // Live empty selection is saved literally, but restoring it falls back to all.
        menu.update(cx, |s, cx| s.choose_root(child.clone(), false, cx));
        cx.run_until_parked();
        assert_eq!(store.load().unwrap().roots, Some(Default::default()));
        menu.update(cx, |s, cx| s.start_restore(cx));
        cx.run_until_parked();
        menu.read_with(cx, |s, _| assert!(s.selected_roots().is_none()));
        // An accepted path edit invalidates a restore queued in the same turn.
        let latest = child.join("deleted.txt");
        menu.update(cx, |s, cx| {
            s.start_restore(cx);
            s.set_paths(vec![latest.clone()], cx);
        });
        cx.run_until_parked();
        menu.read_with(cx, |s, _| assert_eq!(s.absolute_paths(), [latest.clone()]));
        assert_eq!(store.load().unwrap().paths[0].path, latest);
        menu.update(cx, |s, cx| s.clear_filter(cx));
        cx.run_until_parked();
        assert_eq!(store.load().unwrap(), State::default());
        // Switching project while a restore is pending cannot restore the old project.
        menu.update(cx, |s, cx| {
            s.persistent = true;
            s.start_restore(cx);
            s.reset(fixture.0.join("new-project"), cx);
        });
        cx.run_until_parked();
        menu.read_with(cx, |s, _| assert!(!s.has_filter()));
    }
}
