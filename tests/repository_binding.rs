//! Real OS coverage for administrative-directory watcher ownership.
use plico::{
    git::{Cancel, Repository},
    worktree_events::ProjectEvents,
};
use std::{
    fs,
    io::Write,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

fn git(root: &Path, args: &[&str]) {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn failed_binding_resolution_retries_without_membership_change() {
    let mut random = [0; 16];
    getrandom::fill(&mut random).unwrap();
    let fixture = std::env::temp_dir().join(format!("plico-binding-retry-{:x?}", random));
    let work = fixture.join("work");
    let admin = fixture.join("admin");
    fs::create_dir_all(&work).unwrap();
    git(
        &work,
        &["init", "-q", "--separate-git-dir", admin.to_str().unwrap()],
    );
    let rewrite = |target: &Path| {
        let mut pointer = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(work.join(".git"))
            .unwrap();
        writeln!(
            pointer,
            "gitdir: {}",
            target.to_str().unwrap().replace('\\', "/")
        )
        .unwrap();
    };
    rewrite(&fixture.join("missing"));
    let token = Cancel::default();
    let mut observed = ProjectEvents::default();
    observed
        .reconcile(&[Repository::new(&work)], &token)
        .unwrap();
    let now = Instant::now();
    observed.poll_bindings(now, &token).unwrap();
    assert!(
        observed
            .take(now)
            .iter()
            .any(|n| n.batch.all && n.batch.interpretation)
    );
    observed.acknowledge_bindings(std::slice::from_ref(&work));
    assert!(
        !observed.bindings_due(now),
        "installation failure must not busy-loop"
    );
    rewrite(&admin);
    let later = now + Duration::from_secs(31);
    assert!(observed.bindings_due(later));
    observed.poll_bindings(later, &token).unwrap();
    assert!(
        observed
            .take(later)
            .iter()
            .any(|n| n.batch.all && n.batch.interpretation)
    );
    observed.acknowledge_bindings(std::slice::from_ref(&work));
    fs::write(work.join("file.txt"), "base\n").unwrap();
    git(&work, &["add", "file.txt"]);
    let start = Instant::now();
    let mut found = false;
    while start.elapsed() < Duration::from_secs(3) {
        found |= observed
            .take(later)
            .iter()
            .any(|n| n.batch.index && !n.batch.all);
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(observed);
    fs::remove_dir_all(&fixture).unwrap();
    assert!(
        found,
        "recovered watcher must observe the real external index"
    );
}

#[test]
fn background_binding_check_replaces_listener_and_retains_delivery_until_acknowledged() {
    let mut random = [0; 16];
    getrandom::fill(&mut random).unwrap();
    let fixture = std::env::temp_dir().join(format!("plico-binding-probe-{:x?}", random));
    let work = fixture.join("work");
    let other = fixture.join("other");
    let first = fixture.join("admin-a");
    let second = fixture.join("admin-b");
    fs::create_dir_all(&work).unwrap();
    fs::create_dir_all(&other).unwrap();
    git(
        &work,
        &["init", "-q", "--separate-git-dir", first.to_str().unwrap()],
    );
    git(
        &other,
        &["init", "-q", "--separate-git-dir", second.to_str().unwrap()],
    );
    fs::write(work.join("file.txt"), "base\n").unwrap();
    let token = Cancel::default();
    let roots = [Repository::new(&work)];
    let mut observed = ProjectEvents::default();
    observed.reconcile(&roots, &token).unwrap();
    observed.poll_bindings(Instant::now(), &token).unwrap();
    observed.take(Instant::now());
    git(&work, &["add", "file.txt"]);
    let initial_start = Instant::now();
    let mut initial_found = false;
    while initial_start.elapsed() < Duration::from_secs(3) {
        initial_found |= observed
            .take(Instant::now())
            .iter()
            .any(|notice| notice.batch.index);
        if initial_found {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        initial_found,
        "positive control: original external index must be observed"
    );
    let mut pointer = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(work.join(".git"))
        .unwrap();
    writeln!(
        pointer,
        "gitdir: {}",
        second.to_str().unwrap().replace('\\', "/")
    )
    .unwrap();
    drop(pointer);
    std::thread::sleep(Duration::from_millis(500));
    observed.take(Instant::now()); // Drain the pointer edit, not the later index event.
    assert!(observed.bindings_due(Instant::now()));
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(observed.poll_bindings(Instant::now(), &cancelled).is_err());
    assert!(observed.bindings_due(Instant::now()));
    observed.poll_bindings(Instant::now(), &token).unwrap();
    for _ in 0..2 {
        let notice = observed.take(Instant::now());
        assert!(notice.iter().any(|n| n.batch.all && n.batch.interpretation));
    }
    observed.acknowledge_bindings(std::slice::from_ref(&work));
    std::thread::sleep(Duration::from_millis(500));
    observed.take(Instant::now());
    git(&work, &["add", "file.txt"]);
    let start = Instant::now();
    let mut found = false;
    while start.elapsed() < Duration::from_secs(3) {
        found |= observed
            .take(Instant::now())
            .iter()
            .any(|notice| notice.batch.index);
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(found, "new administrative-directory index must be watched");
    std::thread::sleep(Duration::from_millis(500));
    observed.take(Instant::now());
    fs::write(
        first.join("refs/heads/old-only"),
        "0000000000000000000000000000000000000000\n",
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        !observed.take(Instant::now()).iter().any(|n| n.batch.refs),
        "old watcher must be released"
    );
    let later = Instant::now() + Duration::from_secs(31);
    observed.poll_bindings(later, &token).unwrap();
    // Platforms without stable directory creation metadata retain fallback.
    if fs::metadata(&work).unwrap().created().is_ok() {
        assert!(
            !observed.take(later).iter().any(|n| n.batch.all),
            "healthy local gitfile must not periodically invalidate every consumer"
        );
    }
    drop(observed);
    fs::remove_dir_all(&fixture).unwrap();
}
