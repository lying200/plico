use plico::process_tree::ProcessTree;
use std::{io::{Read, Write}, process::{Command, Stdio}, sync::mpsc, thread, time::{Duration, Instant}};

// Invoked as a separate test process, never through a shell or a network endpoint.
#[test]
fn child_fixture() {
    let Ok(mode) = std::env::var("GITUI_PROCESS_TREE_FIXTURE") else { return; };
    if mode == "descendant" {
        println!("descendant-ready"); std::io::stdout().flush().unwrap();
        thread::sleep(Duration::from_secs(20));
    } else {
        let _child = fixture("descendant").stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn().unwrap();
        if mode == "parent-exits" { return; }
        println!("parent-ready"); std::io::stdout().flush().unwrap();
        thread::sleep(Duration::from_secs(20));
    }
}
fn fixture(mode: &str) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", "child_fixture", "--nocapture"]).env("GITUI_PROCESS_TREE_FIXTURE", mode).stdin(Stdio::null());
    c
}
fn verify_cleanup(mode: &str, explicit: bool) {
    let mut unrelated = fixture("descendant").stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let mut tree = ProcessTree::spawn(fixture(mode).stdout(Stdio::piped()).stderr(Stdio::null())).unwrap();
    let mut out = tree.child.stdout.take().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut seen = Vec::new(); let mut buffer = [0; 1024]; let mut ready = false;
        loop {
            let n = out.read(&mut buffer).unwrap(); if n == 0 { break; }
            seen.extend_from_slice(&buffer[..n]);
            if !ready && String::from_utf8_lossy(&seen).contains("descendant-ready") {
                ready_tx.send(()).unwrap(); ready = true;
            }
        }
        done_tx.send(seen).unwrap();
    });
    ready_rx.recv_timeout(Duration::from_secs(10)).expect("descendant started");
    if mode == "parent-exits" {
        let deadline = Instant::now() + Duration::from_secs(5);
        while tree.child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "parent did not exit");
            thread::sleep(Duration::from_millis(10));
        }
    }
    let started = Instant::now();
    if explicit { tree.terminate().unwrap(); tree.terminate().unwrap(); }
    drop(tree);
    let unrelated_alive = unrelated.try_wait().unwrap().is_none();
    let _ = unrelated.kill();
    let _ = unrelated.wait();
    assert!(unrelated_alive, "operation cleanup killed an unrelated process");
    let output = done_rx.recv_timeout(Duration::from_secs(5)).expect("tree cleanup closed inherited pipe");
    reader.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(String::from_utf8_lossy(&output).contains("descendant-ready"));
}
#[test]
fn terminate_closes_descendant_pipes_and_is_idempotent() { verify_cleanup("parent-waits", true); }
#[test]
fn drop_cleans_descendants_even_after_leader_exits() { verify_cleanup("parent-exits", false); }
#[test]
fn spawn_failure_returns_without_a_child() {
    assert!(ProcessTree::spawn(&mut Command::new("gitui-nonexistent-process-fixture-2026")).is_err());
}
