use plico::{git::Cancel, operation_process::{self, Progress, StopReason}};
use std::{io::{Read, Write}, process::Command, thread, time::{Duration, Instant}};

#[test]
fn operation_fixture() {
    let Ok(mode) = std::env::var("GITUI_OPERATION_FIXTURE") else { return; };
    if mode == "input" {
        // Fill both output pipes before reading, forcing all three transports
        // to run concurrently. A synchronous stdin write would deadlock here.
        std::io::stdout().write_all(&[b'o'; 128 * 1024]).unwrap();
        std::io::stderr().write_all(&[b'e'; 128 * 1024]).unwrap();
        let mut buffer = [0; 8192];
        let mut count = 0usize;
        let mut checksum = 0u64;
        loop {
            let n = std::io::stdin().read(&mut buffer).unwrap();
            if n == 0 { break; }
            count += n;
            checksum += buffer[..n].iter().map(|b| u64::from(*b)).sum::<u64>();
        }
        print!("received:{count}:{checksum}");
        std::process::exit(0);
    }
    if mode == "reject-input" {
        eprintln!("input rejected by fixture");
        std::process::exit(7);
    }
    if mode == "output" {
        std::io::stdout().write_all(b"nothing to commit\n").unwrap();
        std::io::stderr().write_all(b"error: gpg failed to sign the data\n").unwrap();
        std::io::stdout().write_all(b"Automatic merge failed; fix conflicts and then commit the result\n").unwrap();
        std::io::stderr().write_all(b"CONFLICTS IN INDEX\n").unwrap();
        for _ in 0..64 {
            std::io::stdout().write_all(&[b'o'; 8192]).unwrap();
            std::io::stderr().write_all(&[b'e'; 8192]).unwrap();
        }
        std::io::stdout().write_all(b"output-end").unwrap();
        std::io::stderr().write_all(b"error-end").unwrap();
        std::process::exit(7);
    }
    eprintln!("ready");
    thread::sleep(Duration::from_secs(20));
}
fn fixture(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", "operation_fixture", "--nocapture"])
        .env("GITUI_OPERATION_FIXTURE", mode);
    command
}
#[test]
fn drains_both_streams_with_bounded_memory_and_preserves_failure() {
    let progress = Progress::default();
    let result = operation_process::run(&mut fixture("output"), &Cancel::default(), &progress, Some(Duration::from_secs(10))).unwrap();
    assert_eq!(result.status.code(), Some(7));
    assert_eq!(result.stopped, None);
    assert_eq!(result.stdout.bytes().len(), 65536);
    assert_eq!(result.stderr.bytes().len(), 65536);
    assert!(result.stdout.truncated && result.stderr.truncated);
    assert!(result.stdout.bytes().ends_with(b"output-end"));
    assert!(result.stderr.bytes().ends_with(b"error-end"));
    assert_eq!(progress.snapshot().bytes(), result.stderr.bytes());
    assert!(result.commit_signals.empty && result.commit_signals.signing && result.commit_signals.pinentry);
    use plico::operation_events::Event;
    assert!(result.events.contains(Event::MergeConflict));
    assert!(result.events.contains(Event::UnstashIndexConflict));
    assert!(!result.events.contains(Event::AlreadyUpToDate));
    assert!(!result.stdout.bytes().windows(9).any(|w| w == b"Automatic"));
}
#[test]
fn cancels_running_operation_after_progress() {
    let cancel = Cancel::default();
    let progress = Progress::default();
    let worker_cancel = cancel.clone();
    let worker_progress = progress.clone();
    let worker = thread::spawn(move || operation_process::run(&mut fixture("wait"), &worker_cancel, &worker_progress, Some(Duration::from_secs(10))));
    let start = Instant::now();
    while !progress.snapshot().bytes().windows(5).any(|w| w == b"ready") {
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(10));
    }
    cancel.cancel();
    let result = worker.join().unwrap().unwrap();
    assert_eq!(result.stopped, Some(StopReason::Cancelled));
    assert!(start.elapsed() < Duration::from_secs(5));
}
#[test]
fn deadline_and_pre_cancel_are_distinct() {
    let cancel = Cancel::default();
    let result = operation_process::run(&mut fixture("wait"), &cancel, &Progress::default(), Some(Duration::from_millis(100))).unwrap();
    assert_eq!(result.stopped, Some(StopReason::TimedOut));
    cancel.cancel();
    let error = operation_process::run(&mut Command::new("nonexistent-pre-cancel"), &cancel, &Progress::default(), None).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
}

#[test]
fn input_and_both_outputs_flow_without_deadlock_and_eof_is_delivered() {
    let input: Vec<u8> = (0..2 * 1024 * 1024).map(|i| (i % 256) as u8).collect();
    let result = operation_process::run_with_input(&mut fixture("input"), &Cancel::default(),
        &Progress::default(), Some(Duration::from_secs(10)), &input).unwrap();
    assert!(result.status.success());
    assert!(result.input_error.is_none());
    assert_eq!(result.stopped, None);
    let expected = format!("received:{}:{}", input.len(), input.iter().map(|b| u64::from(*b)).sum::<u64>());
    assert!(result.stdout.bytes().ends_with(expected.as_bytes()));
    assert!(result.stdout.truncated && result.stderr.truncated);
    let empty = operation_process::run_with_input(&mut fixture("input"), &Cancel::default(),
        &Progress::default(), Some(Duration::from_secs(10)), b"").unwrap();
    assert!(empty.status.success() && empty.input_error.is_none());
    assert!(empty.stdout.bytes().ends_with(b"received:0:0"));
}

#[test]
fn blocked_input_writer_is_joined_on_cancel_or_timeout() {
    for timeout in [false, true] {
        let cancel = Cancel::default();
        let progress = Progress::default();
        let worker_cancel = cancel.clone();
        let worker_progress = progress.clone();
        let start = Instant::now();
        let worker = thread::spawn(move || operation_process::run_with_input(&mut fixture("wait"),
            &worker_cancel, &worker_progress, Some(Duration::from_millis(if timeout { 500 } else { 10000 })),
            &vec![b'x'; 4 * 1024 * 1024]));
        if !timeout {
            while !progress.snapshot().bytes().windows(5).any(|w| w == b"ready") {
                assert!(start.elapsed() < Duration::from_secs(5));
                thread::sleep(Duration::from_millis(10));
            }
            cancel.cancel();
        }
        let result = worker.join().unwrap().unwrap();
        assert_eq!(result.stopped, Some(if timeout { StopReason::TimedOut } else { StopReason::Cancelled }));
        assert!(result.input_error.is_some());
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn early_input_rejection_preserves_diagnostics_and_pre_cancel_does_not_spawn() {
    let cancel = Cancel::default();
    let result = operation_process::run_with_input(&mut fixture("reject-input"), &cancel,
        &Progress::default(), Some(Duration::from_secs(5)), &vec![0; 4 * 1024 * 1024]).unwrap();
    assert!(result.input_error.is_some());
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr.bytes()).contains("input rejected by fixture"));
    cancel.cancel();
    let error = operation_process::run_with_input(&mut Command::new("must-not-start"), &cancel,
        &Progress::default(), None, b"private input").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
}
