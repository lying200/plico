//! Owns local-change query scheduling and feedback; no tree or commit draft state.
use gpui_kit::{App, AppContext, Context, EventEmitter, Task};
use plico::{
    commit_changes::Snapshot,
    commit_recovery,
    git::{Cancel, Repository},
};
use std::{rc::Rc, time::Duration};

type Result = std::result::Result<(Snapshot, bool, Vec<plico::git::Change>), String>;
type Loader = Rc<dyn Fn(Repository, Cancel, bool, &mut App) -> Task<Result>>;

pub enum Update {
    Started,
    Loaded(Snapshot, bool, Vec<plico::git::Change>),
    Failed(String),
}

struct Flight {
    token: Cancel,
    pending: bool,
}
impl Drop for Flight {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

pub struct ChangesRefresh {
    repository: Repository,
    loader: Loader,
    flight: Option<Flight>,
    revision: u64,
    loaded: bool,
    show_ignored: bool,
    feedback: bool,
    query: Task<()>,
    delay: Task<()>,
}
impl EventEmitter<Update> for ChangesRefresh {}
impl ChangesRefresh {
    pub fn new(repository: Repository) -> Self {
        Self::with_loader(
            repository,
            Rc::new(|repo, token, show_ignored, cx| {
                cx.background_spawn(async move {
                    let snapshot = Snapshot::load(&repo, &token)?;
                    let recovery =
                        commit_recovery::pending(&repo.editor_directory(&token)?)?.is_some();
                    let ignored = if show_ignored {
                        plico::local_changes::ignored(&repo, &token)?
                    } else {
                        vec![]
                    };
                    Ok((snapshot, recovery, ignored))
                })
            }),
        )
    }
    pub(crate) fn with_loader(repository: Repository, loader: Loader) -> Self {
        Self {
            repository,
            loader,
            flight: None,
            revision: 0,
            loaded: false,
            show_ignored: false,
            feedback: false,
            query: Task::ready(()),
            delay: Task::ready(()),
        }
    }
    pub fn set_show_ignored(&mut self, value: bool, cx: &mut Context<Self>) {
        self.show_ignored = value;
        self.request(cx);
    }
    pub fn busy(&self) -> bool {
        self.flight.is_some()
    }
    pub fn loaded(&self) -> bool {
        self.loaded
    }
    pub fn feedback(&self) -> bool {
        self.feedback
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Keep the current query and coalesce any number of invalidations into one follow-up.
    pub fn request(&mut self, cx: &mut Context<Self>) {
        if let Some(flight) = &mut self.flight {
            flight.pending = true;
            return;
        }
        let token = Cancel::default();
        self.flight = Some(Flight {
            token: token.clone(),
            pending: false,
        });
        self.revision = self.revision.wrapping_add(1);
        self.feedback = false;
        cx.emit(Update::Started);
        self.delay = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.busy() {
                    s.feedback = true;
                    cx.notify();
                }
            });
        });
        let work = (self.loader)(self.repository.clone(), token, self.show_ignored, cx);
        self.query = cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |s, cx| {
                let pending = s.flight.take().is_some_and(|flight| flight.pending);
                s.delay = Task::ready(());
                s.feedback = false;
                match result {
                    Ok((snapshot, recovery, ignored)) => {
                        s.loaded = true;
                        cx.emit(Update::Loaded(snapshot, recovery, ignored));
                    }
                    Err(error) => cx.emit(Update::Failed(error)),
                }
                if pending {
                    s.request(cx);
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
    use gpui::AppContext;
    use std::cell::{Cell, RefCell};

    fn snapshot() -> Snapshot {
        Snapshot {
            head: None,
            files: vec![],
            review_versions: vec![],
        }
    }

    #[gpui::test]
    fn coalesces_queries_delays_feedback_and_retains_success_after_failure(
        cx: &mut gpui::TestAppContext,
    ) {
        let starts = Rc::new(Cell::new(0));
        let tokens = Rc::new(RefCell::new(Vec::new()));
        let events = Rc::new(RefCell::new(Vec::new()));
        let loader: Loader = {
            let starts = starts.clone();
            let tokens = tokens.clone();
            Rc::new(move |_, token, _, cx| {
                let n = starts.get() + 1;
                starts.set(n);
                tokens.borrow_mut().push(token);
                let timer = cx
                    .background_executor()
                    .timer(Duration::from_millis(if n == 1 { 500 } else { 100 }));
                cx.background_spawn(async move {
                    timer.await;
                    if n == 2 {
                        Err("read failed".into())
                    } else {
                        Ok((snapshot(), false, vec![]))
                    }
                })
            })
        };
        let query = cx.new(|_| ChangesRefresh::with_loader(Repository::new("unused"), loader));
        cx.update(|cx| {
            let events = events.clone();
            cx.subscribe(&query, move |_, event, _| {
                events.borrow_mut().push(match event {
                    Update::Started => "start",
                    Update::Loaded(..) => "loaded",
                    Update::Failed(..) => "failed",
                })
            })
            .detach();
            query.update(cx, |s, cx| {
                for _ in 0..10 {
                    s.request(cx);
                }
            });
        });
        cx.run_until_parked();
        assert_eq!(starts.get(), 1);
        assert!(!tokens.borrow()[0].cancelled());
        cx.executor().advance_clock(Duration::from_millis(299));
        cx.run_until_parked();
        assert!(!query.read_with(cx, |s, _| s.feedback()));
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert!(query.read_with(cx, |s, _| s.feedback()));
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert_eq!(starts.get(), 2);
        assert!(query.read_with(cx, |s, _| s.busy() && s.loaded() && !s.feedback()));
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        assert!(query.read_with(cx, |s, _| !s.busy() && s.loaded() && !s.feedback()));
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert_eq!(&*events.borrow(), &["start", "loaded", "start", "failed"]);
        assert_eq!(starts.get(), 2);
        assert!(!query.read_with(cx, |s, _| s.feedback()));
        query.update(cx, |s, cx| s.request(cx));
        assert_eq!(starts.get(), 3);
        drop(query);
        cx.update(|_| {});
        cx.run_until_parked();
        assert!(
            tokens.borrow()[2].cancelled(),
            "dropping owner cancels its in-flight Git query"
        );
    }
}
