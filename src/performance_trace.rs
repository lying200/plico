//! Opt-in bounded backend timings. No command arguments, paths, output or errors are logged.
//! Set PLICO_PERF_TRACE to a new file before process startup. Trace failures never fail Git.
//! Spans belong to blocking workers: the !Send guard must not cross an async suspension.
use std::{
    cell::Cell,
    collections::hash_map::RandomState,
    ffi::OsStr,
    fs::{File, OpenOptions},
    hash::BuildHasher,
    io::Write,
    marker::PhantomData,
    path::Path,
    rc::Rc,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const LIMIT: usize = 16 * 1024 * 1024;
struct Output {
    file: File,
    remaining: usize,
    failed: bool,
}
struct Recorder {
    output: Mutex<Output>,
    sequence: AtomicU64,
    roots: RandomState,
    start: Instant,
}
impl Recorder {
    fn open(path: &Path, limit: usize) -> std::io::Result<Arc<Self>> {
        let file = OpenOptions::new().create_new(true).write(true).open(path)?;
        Ok(Arc::new(Self {
            output: Mutex::new(Output {
                file,
                remaining: limit,
                failed: false,
            }),
            sequence: AtomicU64::new(1),
            roots: RandomState::new(),
            start: Instant::now(),
        }))
    }
    fn record(&self, event: serde_json::Value) {
        let Ok(mut bytes) = serde_json::to_vec(&event) else {
            return;
        };
        bytes.push(b'\n');
        let mut output = self.output.lock().unwrap_or_else(|e| e.into_inner());
        if output.failed || bytes.len() > output.remaining {
            output.failed = true;
            return;
        }
        output.remaining -= bytes.len();
        if output.file.write_all(&bytes).is_err() {
            output.failed = true;
        }
    }
}
fn recorder() -> Option<Arc<Recorder>> {
    static TRACE: OnceLock<Option<Arc<Recorder>>> = OnceLock::new();
    TRACE
        .get_or_init(|| {
            std::env::var_os("PLICO_PERF_TRACE")
                .filter(|p| !p.is_empty())
                .and_then(|p| Recorder::open(Path::new(&p), LIMIT).ok())
        })
        .clone()
}

#[derive(Clone, Copy, Default)]
struct Context {
    id: u64,
    root: Option<u64>,
}
thread_local! { static CURRENT: Cell<Context> = Cell::new(Context::default()); }

/// Explicit diagnostic identity transported between UI and blocking workers.
/// It installs thread-local context only for the synchronous `scope` call.
#[derive(Clone, Default)]
pub struct RequestContext(Option<Arc<RequestData>>);
struct RequestData {
    recorder: Arc<Recorder>,
    context: Context,
    parent: u64,
    name: &'static str,
    start: Instant,
}
impl RequestContext {
    pub fn scope<T>(&self, work: impl FnOnce() -> T) -> T {
        let Some(data) = &self.0 else { return work() };
        struct Restore(Context);
        impl Drop for Restore {
            fn drop(&mut self) {
                CURRENT.set(self.0);
            }
        }
        let _restore = Restore(CURRENT.replace(data.context));
        work()
    }
    /// A milestone is an observation, not proof that a frame was painted.
    pub fn mark(&self, name: &'static str) {
        let Some(data) = &self.0 else { return };
        data.recorder.record(serde_json::json!({
            "schema":1, "pid":std::process::id(),
            "id":data.recorder.sequence.fetch_add(1, Ordering::Relaxed),
            "parent":data.context.id, "root":data.context.root.map(|n|format!("{n:016x}")),
            "kind":"milestone", "name":name,
            "start_us":micros(data.recorder.start.elapsed()),
            "elapsed_us":micros(data.start.elapsed()),
        }));
    }
}

/// Async request lifetime with explicit ownership; never leaves TLS installed
/// while suspended. Dropping without `finish` records an abandoned request.
pub struct Request {
    context: RequestContext,
    outcome: &'static str,
}
impl Request {
    pub fn new(name: &'static str, root: Option<&Path>) -> Self {
        Self::begin(recorder(), name, root)
    }
    fn begin(recorder: Option<Arc<Recorder>>, name: &'static str, root: Option<&Path>) -> Self {
        let data = recorder.map(|recorder| {
            let parent = CURRENT.get();
            let context = Context {
                id: recorder.sequence.fetch_add(1, Ordering::Relaxed),
                root: root.map(|p| recorder.roots.hash_one(p)).or(parent.root),
            };
            Arc::new(RequestData {
                recorder,
                context,
                parent: parent.id,
                name,
                start: Instant::now(),
            })
        });
        Self {
            context: RequestContext(data),
            outcome: "abandoned",
        }
    }
    pub fn context(&self) -> RequestContext {
        self.context.clone()
    }
    pub fn finish(mut self, outcome: &'static str) {
        self.outcome = outcome;
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        let Some(data) = &self.context.0 else { return };
        data.recorder.record(serde_json::json!({
            "schema":1, "pid":std::process::id(), "id":data.context.id,
            "parent":data.parent, "root":data.context.root.map(|n|format!("{n:016x}")),
            "kind":"request", "name":data.name,
            "start_us":micros(data.start.duration_since(data.recorder.start)),
            "total_us":micros(data.start.elapsed()), "outcome":self.outcome,
        }));
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Work = 0,
    Queue = 1,
    Spawn = 2,
    Run = 3,
    Drain = 4,
}

/// Backend request or command span. Drop records early returns as well as success.
pub(crate) struct Span {
    active: Option<Active>,
    _worker: PhantomData<Rc<()>>,
}
struct Active {
    recorder: Arc<Recorder>,
    id: u64,
    parent: Context,
    root: Option<u64>,
    kind: &'static str,
    name: &'static str,
    start: Instant,
    checkpoint: Instant,
    phase: Phase,
    times: [u64; 5],
    output_us: Option<u64>,
    outcome: &'static str,
}
impl Span {
    pub(crate) fn new(kind: &'static str, name: &'static str, root: Option<&Path>) -> Self {
        Self::begin(recorder(), kind, name, root)
    }
    fn begin(
        recorder: Option<Arc<Recorder>>,
        kind: &'static str,
        name: &'static str,
        root: Option<&Path>,
    ) -> Self {
        let active = recorder.map(|recorder| {
            let parent = CURRENT.get();
            let id = recorder.sequence.fetch_add(1, Ordering::Relaxed);
            let root = root.map(|p| recorder.roots.hash_one(p)).or(parent.root);
            CURRENT.set(Context { id, root });
            let start = Instant::now();
            Active {
                recorder,
                id,
                parent,
                root,
                kind,
                name,
                start,
                checkpoint: start,
                phase: Phase::Work,
                times: [0; 5],
                output_us: None,
                outcome: "unfinished",
            }
        });
        Self {
            active,
            _worker: PhantomData,
        }
    }
    pub(crate) fn enabled(&self) -> bool {
        self.active.is_some()
    }
    pub(crate) fn phase(&mut self, next: Phase) {
        if let Some(a) = &mut self.active {
            a.times[a.phase as usize] += micros(a.checkpoint.elapsed());
            a.checkpoint = Instant::now();
            a.phase = next;
        }
    }
    pub(crate) fn outcome(&mut self, outcome: &'static str) {
        if let Some(a) = &mut self.active {
            a.outcome = outcome;
        }
    }
    /// Output consumption runs concurrently with Git; this is read+parse wall time,
    /// not CPU-only parsing and not an extra sequential phase to add to total_us.
    pub(crate) fn output_time(&mut self, time: Option<Duration>) {
        if let Some(a) = &mut self.active {
            a.output_us = time.map(micros);
        }
    }
}
fn micros(time: Duration) -> u64 {
    time.as_micros().min(u64::MAX as u128) as u64
}
impl Drop for Span {
    fn drop(&mut self) {
        let Some(mut a) = self.active.take() else {
            return;
        };
        a.times[a.phase as usize] += micros(a.checkpoint.elapsed());
        CURRENT.set(a.parent);
        a.recorder.record(serde_json::json!({
            "schema":1, "pid":std::process::id(), "id":a.id, "parent":a.parent.id,
            "root":a.root.map(|n| format!("{n:016x}")), "kind":a.kind, "name":a.name,
            "start_us":micros(a.start.duration_since(a.recorder.start)),
            "total_us":micros(a.start.elapsed()), "work_us":a.times[0], "queue_us":a.times[1],
            "spawn_us":a.times[2], "run_us":a.times[3], "drain_us":a.times[4],
            "output_read_parse_us":a.output_us, "outcome":a.outcome,
        }));
    }
}

/// Static allowlist only: even an unknown command or alias may contain private text.
pub(crate) fn command_name<'a>(args: impl IntoIterator<Item = &'a OsStr>) -> &'static str {
    for arg in args {
        match arg.to_str() {
            Some("rev-parse") => return "rev-parse",
            Some("for-each-ref") => return "for-each-ref",
            Some("symbolic-ref") => return "symbolic-ref",
            Some("status") => return "status",
            Some("log") => return "log",
            Some("show") => return "show",
            Some("diff") => return "diff",
            Some("diff-tree") => return "diff-tree",
            Some("ls-files") => return "ls-files",
            Some("ls-tree") => return "ls-tree",
            Some("cat-file") => return "cat-file",
            Some("config") => return "config",
            Some("remote") => return "remote",
            Some("rev-list") => return "rev-list",
            Some("fetch") => return "fetch",
            Some("push") => return "push",
            Some("merge") => return "merge",
            Some("rebase") => return "rebase",
            Some("checkout") => return "checkout",
            Some("commit") => return "commit",
            Some("reset") => return "reset",
            Some("stash") => return "stash",
            Some("update-index") => return "update-index",
            _ => {}
        }
    }
    "other"
}

pub(crate) fn command_root(command: &std::process::Command) -> Option<&Path> {
    let mut args = command.get_args();
    while let Some(arg) = args.next() {
        if arg == "-C" {
            return args.next().map(Path::new);
        }
    }
    command.get_current_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self(
                std::env::temp_dir()
                    .join(format!("plico-perf-{}-{nonce}.jsonl", std::process::id())),
            )
        }
        fn events(&self) -> Vec<serde_json::Value> {
            std::fs::read_to_string(&self.0)
                .unwrap()
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn nested_queries_keep_identity_without_serializing_sensitive_inputs() {
        let fixture = Fixture::new();
        let recorder = Recorder::open(&fixture.0, LIMIT).unwrap();
        {
            let mut request = Span::begin(
                Some(recorder.clone()),
                "request",
                "changes",
                Some(Path::new("private-repository")),
            );
            let mut query = Span::begin(
                Some(recorder.clone()),
                "query",
                command_name([OsStr::new("status"), OsStr::new("secret.txt")]),
                None,
            );
            query.phase(Phase::Spawn);
            query.phase(Phase::Run);
            query.output_time(Some(Duration::from_micros(12)));
            query.outcome("ok");
            drop(query);
            request.outcome("ok");
        }
        let events = fixture.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["parent"], events[1]["id"]);
        assert_eq!(events[0]["root"], events[1]["root"]);
        assert_eq!(events[1]["parent"], 0);
        assert_eq!(events[0]["output_read_parse_us"], 12);
        let text = std::fs::read_to_string(&fixture.0).unwrap();
        assert!(!text.contains("private-repository"));
        assert!(!text.contains("secret.txt"));
        assert_eq!(command_name([OsStr::new("private-alias")]), "other");
        assert_eq!(CURRENT.get().id, 0);
    }

    #[test]
    fn panic_restores_context_and_threads_do_not_inherit_unrelated_requests() {
        let fixture = Fixture::new();
        let recorder = Recorder::open(&fixture.0, LIMIT).unwrap();
        let outer = Span::begin(Some(recorder.clone()), "request", "outer", None);
        let outer_id = CURRENT.get().id;
        let _ = std::panic::catch_unwind(|| {
            let _inner = Span::begin(Some(recorder.clone()), "request", "inner", None);
            panic!("test unwind");
        });
        assert_eq!(CURRENT.get().id, outer_id);
        let other = recorder.clone();
        std::thread::spawn(move || {
            let _span = Span::begin(Some(other), "query", "status", None);
        })
        .join()
        .unwrap();
        drop(outer);
        let events = fixture.events();
        assert_eq!(events[0]["parent"], outer_id);
        assert_eq!(events[1]["parent"], 0);
        assert_eq!(CURRENT.get().id, 0);
    }

    #[test]
    fn async_request_context_is_explicit_and_restored_after_worker_unwinds() {
        let fixture = Fixture::new();
        let recorder = Recorder::open(&fixture.0, LIMIT).unwrap();
        let request = Request::begin(Some(recorder.clone()), "changes-refresh", None);
        let context = request.context();
        assert_eq!(CURRENT.get().id, 0);
        let worker_recorder = recorder.clone();
        std::thread::spawn(move || {
            let unrelated = Span::begin(Some(worker_recorder.clone()), "request", "other", None);
            let previous = CURRENT.get().id;
            let result = std::panic::catch_unwind(|| {
                context.scope(|| {
                    context.mark("worker-start");
                    let _query =
                        Span::begin(Some(worker_recorder.clone()), "query", "status", None);
                    panic!("worker failure");
                })
            });
            assert!(result.is_err());
            assert_eq!(CURRENT.get().id, previous);
            drop(unrelated);
        })
        .join()
        .unwrap();
        request.context().mark("result-event");
        request.finish("error");
        let events = fixture.events();
        let request = events
            .iter()
            .find(|e| e["name"] == "changes-refresh")
            .unwrap();
        assert_eq!(request["outcome"], "error");
        for event in events.iter().filter(|e| {
            matches!(
                e["name"].as_str(),
                Some("worker-start" | "status" | "result-event")
            )
        }) {
            assert_eq!(event["parent"], request["id"]);
        }
        assert_eq!(CURRENT.get().id, 0);
        drop(Request::begin(Some(recorder), "abandoned-refresh", None));
        assert_eq!(fixture.events().last().unwrap()["outcome"], "abandoned");
    }

    #[test]
    fn bounded_trace_never_overwrites_an_existing_file_or_emits_partial_records() {
        let fixture = Fixture::new();
        let recorder = Recorder::open(&fixture.0, 20).unwrap();
        assert!(Recorder::open(&fixture.0, LIMIT).is_err());
        recorder.record(serde_json::json!({"n":1}));
        recorder.record(serde_json::json!({"long":"exceeds remaining capacity"}));
        recorder.record(serde_json::json!({"n":2}));
        assert_eq!(fixture.events(), vec![serde_json::json!({"n":1})]);
        let current = CURRENT.get().id;
        drop(Span::begin(None, "query", "status", None));
        assert_eq!(CURRENT.get().id, current);
    }
}
