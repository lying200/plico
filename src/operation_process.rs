//! Bounded transport for explicit Git operations; command/auth policy belongs to callers.
//! Rebased GitFetchSupportImpl.doFetch consumes stderr progress separately from results.
//! We retain bounded tails rather than accumulating every progress update in memory.
use crate::{git::Cancel, process_tree::ProcessTree};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    process::{Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const TAIL_LIMIT: usize = 64 * 1024;
#[derive(Clone, Debug, Default)]
pub struct Tail {
    bytes: VecDeque<u8>,
    pub truncated: bool,
}
impl Tail {
    fn append(&mut self, bytes: &[u8]) {
        let excess = (self.bytes.len() + bytes.len()).saturating_sub(TAIL_LIMIT);
        if excess > 0 {
            self.truncated = true;
            self.bytes.drain(..excess.min(self.bytes.len()));
        }
        self.bytes
            .extend(bytes.iter().skip(bytes.len().saturating_sub(TAIL_LIMIT)));
    }
    /// Raw bytes are intentionally not logged. Consumers must redact before displaying.
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }
}
#[derive(Clone, Default)]
pub struct Progress(Arc<Mutex<Tail>>);
impl Progress {
    pub fn snapshot(&self) -> Tail {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum StopReason {
    Cancelled,
    TimedOut,
}
#[derive(Debug)]
pub struct Outcome {
    pub status: ExitStatus,
    pub stopped: Option<StopReason>,
    pub stdout: Tail,
    pub stderr: Tail,
    pub events: crate::operation_events::Events,
    /// A successful exit alone is insufficient when stdin delivery failed.
    /// Kept separate so cancellation and Git's stderr are not lost to BrokenPipe.
    pub input_error: Option<io::Error>,
    pub commit_signals: crate::commit_result::Signals,
}
fn drain(
    mut pipe: impl Read,
    progress: Option<Progress>,
) -> io::Result<(
    Tail,
    crate::operation_events::Events,
    crate::commit_result::Signals,
)> {
    let mut tail = Tail::default();
    let mut detector = crate::operation_events::Detector::default();
    let mut commit_detector = crate::commit_result::Detector::new(progress.is_some());
    let mut buffer = [0; 8192];
    loop {
        let n = match pipe.read(&mut buffer) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if n == 0 {
            return Ok((tail, detector.events, commit_detector.signals));
        }
        detector.feed(&buffer[..n]);
        commit_detector.feed(&buffer[..n]);
        tail.append(&buffer[..n]);
        if let Some(progress) = &progress {
            progress
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .append(&buffer[..n]);
        }
    }
}
/// Blocking worker API; never call on the UI thread. No shell, logging, or retry.
/// `timeout` is an explicit caller policy (network operations do not inherit the
/// read layer's 30-second deadline). Cancellation can leave partial repository writes.
/// Authentication must be provided by command configuration, not this null stdin.
pub fn run(
    command: &mut Command,
    cancel: &Cancel,
    progress: &Progress,
    timeout: Option<Duration>,
) -> io::Result<Outcome> {
    run_inner(command, cancel, progress, timeout, None, None)
}

/// Deliver bounded, already-prepared input without copying it. The caller owns
/// the input budget and must check `input_error`, `stopped`, and exit status.
/// The writer runs alongside both readers; cancellation terminates the owned
/// process tree before joining the writer, including when Git never reads stdin.
pub fn run_with_input(
    command: &mut Command,
    cancel: &Cancel,
    progress: &Progress,
    timeout: Option<Duration>,
    input: &[u8],
) -> io::Result<Outcome> {
    run_inner(command, cancel, progress, timeout, Some(input), None)
}

/// Stream binary stdout directly to a file; stderr and cancellation retain the
/// regular operation lifecycle. Callers must reject unsuccessful/partial output.
pub fn run_to_file(
    command: &mut Command,
    cancel: &Cancel,
    progress: &Progress,
    output: std::fs::File,
) -> io::Result<Outcome> {
    run_inner(command, cancel, progress, None, None, Some(output))
}

fn run_inner(
    command: &mut Command,
    cancel: &Cancel,
    progress: &Progress,
    timeout: Option<Duration>,
    input: Option<&[u8]>,
    output: Option<std::fs::File>,
) -> io::Result<Outcome> {
    use crate::performance_trace::{Phase, Span};
    let mut trace = Span::new(
        "operation",
        crate::performance_trace::command_name(command.get_args()),
        crate::performance_trace::command_root(command),
    );
    trace.outcome("error");
    if cancel.cancelled() {
        trace.outcome("cancelled");
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Cancelled before start",
        ));
    }
    *progress.0.lock().unwrap_or_else(|e| e.into_inner()) = Tail::default();
    trace.phase(Phase::Spawn);
    let mut tree = ProcessTree::spawn(
        command
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(output.map(Stdio::from).unwrap_or_else(Stdio::piped))
            .stderr(Stdio::piped()),
    )?;
    trace.phase(Phase::Run);
    let out = tree.child.stdout.take();
    let err = tree.child.stderr.take().unwrap();
    let stdin = tree.child.stdin.take();
    let write_failed = AtomicBool::new(false);
    // Scoped workers are always joined, including process/wait errors.
    thread::scope(|scope| {
        let failed = &write_failed;
        let writer = input.zip(stdin).map(|(input, mut pipe)| {
            scope.spawn(move || {
                let result = pipe.write_all(input);
                if result.is_err() {
                    failed.store(true, Ordering::Release);
                }
                result // pipe is closed here, delivering EOF before joining
            })
        });
        let stdout = scope.spawn(move || match out {
            Some(out) => drain(out, None),
            None => Ok(Default::default()),
        });
        let stderr_progress = progress.clone();
        let stderr = scope.spawn(move || drain(err, Some(stderr_progress)));
        let started = Instant::now();
        let mut stopped = None;
        let status = loop {
            if cancel.cancelled() {
                stopped = Some(StopReason::Cancelled);
            } else if timeout.is_some_and(|limit| started.elapsed() >= limit) {
                stopped = Some(StopReason::TimedOut);
            }
            if stopped.is_some() || write_failed.load(Ordering::Acquire) {
                break tree.terminate().and_then(|()| tree.child.wait());
            }
            match tree.child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => break Err(error),
            }
        };
        // Even on successful leader exit, helpers may still hold inherited pipes.
        // Drop closes the owned tree before waiting for EOF from either reader.
        trace.phase(Phase::Drain);
        drop(tree);
        let out = stdout
            .join()
            .map_err(|_| io::Error::other("Output reader panicked"));
        let err = stderr
            .join()
            .map_err(|_| io::Error::other("Error reader panicked"));
        let input_error = writer
            .map(|w| {
                w.join()
                    .unwrap_or_else(|_| Err(io::Error::other("Input writer panicked")))
            })
            .and_then(Result::err);
        // AskPass dismissal can race with Git exiting after its empty answer.
        // Preserve user cancellation even when try_wait observed that exit first.
        if cancel.cancelled() {
            stopped = Some(StopReason::Cancelled);
        }
        let (stdout, stdout_events, stdout_commit) = out??;
        let (stderr, stderr_events, stderr_commit) = err??;
        trace.outcome(match stopped {
            Some(StopReason::Cancelled) => "cancelled",
            Some(StopReason::TimedOut) => "timeout",
            None if status.as_ref().is_ok_and(|s| s.success()) && input_error.is_none() => "ok",
            None => "error",
        });
        Ok(Outcome {
            status: status?,
            stopped,
            stdout,
            stderr,
            events: stdout_events.union(stderr_events),
            input_error,
            commit_signals: stdout_commit.union(stderr_commit),
        })
    })
}
