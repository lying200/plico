//! Owns local-change query scheduling and feedback; no tree or commit draft state.
use gpui_kit::{App, AppContext, Context, EventEmitter, Task};
use plico::{
    changes_scope::Scope,
    commit_changes::Snapshot,
    commit_recovery,
    git::{Cancel, Repository},
    performance_trace::{Request, RequestContext},
};
use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

type Result = std::result::Result<(Snapshot, bool, Vec<plico::git::Change>), String>;
pub(crate) struct Read {
    previous: Option<Arc<Snapshot>>,
    scope: Scope,
}
type Loader = Rc<dyn Fn(Repository, Cancel, bool, Read, RequestContext, &mut App) -> Task<Result>>;
const CACHE_BYTES: usize = 2 * 1024 * 1024;

pub enum Update {
    Started,
    Loaded(Arc<Snapshot>, bool, Vec<plico::git::Change>),
    Failed(String),
}

struct Flight {
    token: Cancel,
    scope: Scope,
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
    pending: Scope,
    snapshot: Option<Arc<Snapshot>>,
    last_full: Option<Instant>,
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
            Rc::new(|repo, token, show_ignored, read, trace, cx| {
                cx.background_spawn(async move {
                    trace.mark("worker-start");
                    let result = trace.scope(|| {
                        let snapshot = match read.previous {
                            Some(previous) => previous.refresh(&repo, &read.scope, &token)?,
                            None => Snapshot::load(&repo, &token)?,
                        };
                        let recovery =
                            commit_recovery::pending(&repo.editor_directory(&token)?)?.is_some();
                        let ignored = if show_ignored {
                            plico::local_changes::ignored(&repo, &token)?
                        } else {
                            vec![]
                        };
                        Ok((snapshot, recovery, ignored))
                    });
                    trace.mark("worker-finished");
                    result
                })
            }),
        )
    }
    pub(crate) fn with_loader(repository: Repository, loader: Loader) -> Self {
        Self {
            repository,
            loader,
            flight: None,
            pending: Scope::empty(),
            snapshot: None,
            last_full: None,
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
        self.request_scope(Scope::full(), cx);
    }
    pub(crate) fn request_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        self.pending.merge(&scope);
        if self.flight.is_some() || self.pending.is_empty() {
            return;
        }
        let mut scope = std::mem::replace(&mut self.pending, Scope::empty());
        // No background polling is added here. A new invalidation after this
        // interval must re-establish a complete baseline before partial reads.
        if self.snapshot.is_none()
            || self
                .last_full
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(30))
            || self.show_ignored
        {
            scope = Scope::full();
        }
        let read = Read {
            previous: (!scope.is_full()).then(|| self.snapshot.clone()).flatten(),
            scope: scope.clone(),
        };
        let token = Cancel::default();
        let trace = Request::new("changes-refresh", Some(&self.repository.path));
        let trace_context = trace.context();
        self.flight = Some(Flight {
            token: token.clone(),
            scope,
        });
        self.revision = self.revision.wrapping_add(1);
        self.feedback = false;
        cx.emit(Update::Started);
        trace_context.mark("started-event");
        let feedback_trace = trace_context.clone();
        self.delay = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.busy() {
                    s.feedback = true;
                    feedback_trace.mark("feedback-notify");
                    cx.notify();
                }
            });
        });
        let work = (self.loader)(
            self.repository.clone(),
            token,
            self.show_ignored,
            read,
            trace_context.clone(),
            cx,
        );
        self.query = cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |s, cx| {
                let Some(flight) = s.flight.take() else {
                    return;
                };
                let followup = !s.pending.is_empty();
                s.delay = Task::ready(());
                s.feedback = false;
                let outcome = if result.is_ok() { "ok" } else { "error" };
                match result {
                    Ok((snapshot, recovery, ignored)) => {
                        s.loaded = true;
                        if flight.scope.is_full() {
                            s.last_full = Some(Instant::now());
                        }
                        let snapshot = Arc::new(snapshot);
                        s.snapshot = (snapshot.retained_bytes() + 2 * std::mem::size_of::<usize>()
                            <= CACHE_BYTES)
                            .then(|| snapshot.clone());
                        cx.emit(Update::Loaded(snapshot, recovery, ignored));
                    }
                    Err(error) => {
                        s.pending.merge(&flight.scope);
                        cx.emit(Update::Failed(error));
                    }
                }
                trace_context.mark("result-event");
                trace.finish(outcome);
                // Retry immediately only when new dirt arrived during this
                // flight. A failed query alone must not create a busy retry loop.
                if followup {
                    s.request_scope(Scope::empty(), cx);
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
    fn partial_refresh_retries_union_without_spinning_and_bounds_snapshot_ownership(
        cx: &mut gpui::TestAppContext,
    ) {
        let reads = Rc::new(RefCell::new(Vec::<Read>::new()));
        let loader: Loader = {
            let reads = reads.clone();
            Rc::new(move |_, _, _, read, _, cx| {
                reads.borrow_mut().push(read);
                let n = reads.borrow().len();
                let timer = cx.background_executor().timer(Duration::from_millis(100));
                cx.background_spawn(async move {
                    timer.await;
                    if n == 2 || n == 4 {
                        return Err("read failed".into());
                    }
                    let mut value = snapshot();
                    value.head = Some(if n == 6 {
                        "x".repeat(CACHE_BYTES)
                    } else {
                        format!("snapshot-{n}")
                    });
                    Ok((value, false, vec![]))
                })
            })
        };
        fn finish(cx: &mut gpui::TestAppContext) {
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(100));
            cx.run_until_parked();
        }
        let scope = |path: &str| Scope::paths([path.into()]);
        let query = cx.new(|_| ChangesRefresh::with_loader(Repository::new("owned-root"), loader));
        query.update(cx, |s, cx| s.request_scope(scope("initial"), cx));
        finish(cx);
        assert!(reads.borrow()[0].scope.is_full());
        query.update(cx, |s, cx| {
            s.request_scope(scope("a"), cx);
            s.request_scope(scope("b"), cx);
        });
        finish(cx);
        assert_eq!(reads.borrow().len(), 3);
        assert_eq!(
            reads.borrow()[2].scope,
            Scope::paths(["a".into(), "b".into()])
        );
        assert!(Arc::ptr_eq(
            reads.borrow()[1].previous.as_ref().unwrap(),
            reads.borrow()[2].previous.as_ref().unwrap(),
        ));
        finish(cx);
        assert!(query.read_with(cx, |s, _| !s.busy() && s.pending.is_empty()));
        query.update(cx, |s, cx| s.request_scope(scope("c"), cx));
        finish(cx);
        assert_eq!(reads.borrow().len(), 4);
        assert!(query.read_with(cx, |s, _| !s.busy() && s.pending == scope("c")));
        assert_eq!(
            query.read_with(cx, |s, _| s.snapshot.as_ref().unwrap().head.clone()),
            Some("snapshot-3".into())
        );
        finish(cx);
        assert_eq!(
            reads.borrow().len(),
            4,
            "failure must wait for another request"
        );
        query.update(cx, |s, cx| s.request_scope(scope("d"), cx));
        assert_eq!(
            reads.borrow()[4].scope,
            Scope::paths(["c".into(), "d".into()])
        );
        finish(cx);
        query.update(cx, |s, cx| {
            s.last_full = Some(Instant::now() - Duration::from_secs(31));
            s.request_scope(scope("e"), cx);
        });
        assert!(reads.borrow()[5].scope.is_full());
        assert!(reads.borrow()[5].previous.is_none());
        finish(cx);
        assert!(
            query.read_with(cx, |s, _| s.loaded() && s.snapshot.is_none()),
            "large snapshots are delivered but not retained"
        );
        query.update(cx, |s, cx| s.request_scope(scope("f"), cx));
        assert!(reads.borrow()[6].scope.is_full());
        assert!(reads.borrow()[6].previous.is_none());
        finish(cx);
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
            Rc::new(move |_, token, _, _, _, cx| {
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
