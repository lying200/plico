//! Owns the main Log's mutually replacing history, details and navigation jobs.
//! Presentation and Git queries stay with their callers; request lifetime does not.
use gpui_kit::Task;
use plico::git::Cancel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Stamp(u64);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    History,
    Details,
    Navigation,
}

pub(super) struct Query {
    stamp: Stamp,
    active: Option<Kind>,
    token: Cancel,
    task: Task<()>,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            stamp: Stamp(0),
            active: None,
            token: Cancel::default(),
            task: Task::ready(()),
        }
    }
}
impl Drop for Query {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Query {
    pub fn begin(&mut self, kind: Kind) -> (Stamp, Cancel) {
        self.cancel();
        self.token = Cancel::default();
        self.active = Some(kind);
        (self.stamp, self.token.clone())
    }
    pub fn cancel(&mut self) {
        self.token.cancel();
        self.task = Task::ready(());
        self.stamp = Stamp(
            self.stamp
                .0
                .checked_add(1)
                .expect("Log request identity exhausted"),
        );
        self.active = None;
    }
    pub fn track(&mut self, stamp: Stamp, task: Task<()>) {
        if self.is_current(stamp) && self.active.is_some() {
            self.task = task;
        }
    }
    /// Must succeed before applying a result, including errors. A stale result
    /// cannot dismiss the current request's loading feedback.
    pub fn finish(&mut self, stamp: Stamp) -> bool {
        if !self.is_current(stamp) || self.active.is_none() {
            return false;
        }
        self.active = None;
        true
    }
    /// Remains current after completion, for menus tied to the displayed result.
    pub fn stamp(&self) -> Stamp {
        self.stamp
    }
    pub fn is_current(&self, stamp: Stamp) -> bool {
        self.stamp == stamp
    }
    pub fn history_loading(&self) -> bool {
        self.active == Some(Kind::History)
    }
    /// Includes navigation, whose results also depend on repository membership.
    pub fn running(&self) -> bool {
        self.active.is_some()
    }
    /// Navigation retains the existing comparison and has its own progress text.
    pub fn busy(&self) -> bool {
        matches!(self.active, Some(Kind::History | Kind::Details))
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    #[test]
    fn replacement_cancels_old_work_and_rejects_late_completion() {
        let mut query = Query::default();
        let (old, cancelled) = query.begin(Kind::History);
        let (new, current) = query.begin(Kind::History);
        assert!(cancelled.cancelled());
        assert!(!current.cancelled());
        assert!(!query.finish(old));
        assert!(query.history_loading());
        assert!(query.finish(new));
        assert!(!query.busy());
        assert!(query.is_current(new));
        assert!(!query.finish(new));
    }
    #[test]
    fn navigation_and_cached_details_have_distinct_loading_lifetimes() {
        let mut query = Query::default();
        let (history, token) = query.begin(Kind::History);
        let (navigation, _) = query.begin(Kind::Navigation);
        assert!(token.cancelled());
        assert!(!query.finish(history));
        assert!(!query.history_loading());
        assert!(!query.busy());
        assert!(query.finish(navigation));
        let (details, token) = query.begin(Kind::Details);
        assert!(!query.is_current(navigation));
        assert!(query.busy());
        assert!(query.finish(details));
        assert!(!query.busy());
        drop(query);
        assert!(token.cancelled());
    }

    extern crate gpui_kit as gpui;
    use super::super::Review;
    use gpui::{AppContext as _, component::Root};
    use std::{path::PathBuf, sync::Arc};

    #[gpui::test]
    fn clearing_an_empty_filter_does_not_select_an_unrelated_commit(cx: &mut gpui::TestAppContext) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-log-selection-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        std::fs::create_dir(&fixture.0).unwrap();
        for args in [
            vec!["init", "-q"],
            vec![
                "-c",
                "user.name=Audit",
                "-c",
                "user.email=audit@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-qm",
                "base",
            ],
        ] {
            let result = std::process::Command::new("git")
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(fixture.0.clone(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        review.update(cx, |s, cx| s.refresh(false, cx));
        cx.run_until_parked();
        review.update(cx, |s, cx| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.log.commits().len(), 1);
            assert_eq!(s.selected, Some(0), "retain existing initial-open behavior");
            s.select_commit(0, cx);
            s.refresh(false, cx);
        });
        cx.run_until_parked();
        let membership = review.read_with(cx, |s, cx| {
            s.project_roots.read(cx).snapshot().unwrap().unwrap()
        });
        review.update(cx, |s, cx| {
            assert_eq!(s.selected, Some(0), "refresh preserves visible selection");
            s.text_filter = Some(plico::git::LogFilter {
                text: "absent-audit-selection".into(),
                ..Default::default()
            });
            s.refresh_log_filter(cx);
            let current = s.project_roots.read(cx).snapshot().unwrap().unwrap();
            assert!(Arc::ptr_eq(&membership, &current));
        });
        cx.run_until_parked();
        review.update(cx, |s, cx| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert!(s.log.commits().is_empty());
            assert!(s.selected.is_none());
            assert!(s.message.is_empty());
            s.text_filter = None;
            s.refresh_log_filter(cx);
        });
        cx.run_until_parked();
        review.read_with(cx, |s, cx| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.log.commits().len(), 1);
            assert!(
                s.selected.is_none(),
                "clearing filters must preserve empty selection"
            );
            let current = s.project_roots.read(cx).snapshot().unwrap().unwrap();
            assert!(
                Arc::ptr_eq(&membership, &current),
                "filtering must not rediscover project roots"
            );
            assert!(s.message.is_empty());
            assert!(s.files.is_empty());
        });
    }

    #[gpui::test]
    fn opening_diff_during_refresh_releases_log_loading_state(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        cx.update(|window, cx| {
            review.unwrap().update(cx, |s, cx| {
                use plico::{
                    diff_target::{Chain, Target},
                    git::{Area, Change},
                };
                s.refresh(false, cx);
                let old = s.log_query.stamp();
                assert!(s.log_query.history_loading());
                let change = Change {
                    area: Area::Untracked,
                    path: "missing.txt".into(),
                    old_path: None,
                    status: "?".into(),
                };
                let target = Target::new(s.repo.clone(), None, change.clone()).unwrap();
                let chain = Chain::new(&target, &[change]).unwrap();
                let tab = cx.new(|_| crate::diff_tab::DiffTab::new(target, chain));
                s.insert_editor_tab(tab.entity_id());
                s.tabs.push(tab);
                s.activate_diff_tab(0, window, cx);
                assert!(
                    !s.log_query.history_loading(),
                    "cancelled Log work must not leave loading stuck"
                );
                assert!(!s.log_query.busy());
                assert!(!s.log_query.finish(old));
                s.refresh(false, cx);
                assert!(s.log_query.history_loading());
                assert!(!s.log_query.finish(old));
                assert!(
                    s.log_query.history_loading(),
                    "a stale result cannot clear the replacement spinner"
                );
            })
        });
    }

    #[gpui::test]
    fn cached_and_empty_selection_complete_without_background_loading(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        cx.update(|_, cx| {
            review.unwrap().update(cx, |s, cx| {
                let commit = plico::git::Commit {
                    id: "a".repeat(40),
                    parents: vec![],
                    shallow_boundary: false,
                    committer_timestamp: 0,
                    author: "author".into(),
                    date: String::new(),
                    subject: "cached".into(),
                    refs: vec![],
                };
                let details = Arc::new(plico::git::Details {
                    message: "cached message".into(),
                    author: "author".into(),
                    authored: String::new(),
                    committer: String::new(),
                    committed: String::new(),
                    base: String::new(),
                    files: vec![],
                });
                s.details_cache.insert(&s.repo.path, &commit, details);
                s.log = crate::ui::log_display::Display::fixture_single(&s.repo.path, vec![commit]);
                s.select_commit(0, cx);
                assert!(!s.log_query.busy());
                assert!(s.message.contains("cached message"));
                let cached = s.log_query.stamp();
                s.log_selection = Default::default();
                s.load_log_selection(cx);
                assert!(!s.log_query.busy());
                assert!(!s.log_query.is_current(cached));
                assert!(s.message.is_empty());
            })
        });
    }
}
