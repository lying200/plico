use plico::worktree_events::Events;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let path = std::env::temp_dir().join(format!("plico-watch-{:x?}", random));
        fs::create_dir_all(path.join(".git")).unwrap();
        Self(path)
    }
    fn wait(&self, events: &Events, path: &str) {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if events.take().affects(std::path::Path::new(path)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("missing change for {path}");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn watches_create_atomic_replace_rename_and_delete() {
    let f = Fixture::new();
    let events = Events::new(f.0.clone()).unwrap();
    fs::write(f.0.join("a.txt"), "first").unwrap();
    f.wait(&events, "a.txt");
    fs::write(f.0.join("replacement"), "second").unwrap();
    fs::remove_file(f.0.join("a.txt")).unwrap();
    fs::rename(f.0.join("replacement"), f.0.join("a.txt")).unwrap();
    f.wait(&events, "a.txt");
    fs::rename(f.0.join("a.txt"), f.0.join("b.txt")).unwrap();
    f.wait(&events, "b.txt");
    fs::remove_file(f.0.join("b.txt")).unwrap();
    f.wait(&events, "b.txt");
}
#[test]
fn file_reads_do_not_generate_refresh_loops() {
    let f = Fixture::new();
    fs::write(f.0.join("a.txt"), "first").unwrap();
    let events = Events::new(f.0.clone()).unwrap();
    for _ in 0..10 {
        fs::read(f.0.join("a.txt")).unwrap();
    }
    std::thread::sleep(Duration::from_millis(400));
    assert!(!events.take().changed());
}

fn git(root: &std::path::Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn wait_git_change(events: &Events) -> plico::worktree_events::Batch {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        let batch = events.take();
        if batch.index || batch.refs {
            return batch;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("missing actual Git index/ref notification");
}
#[test]
fn observes_external_git_directory_index() {
    let f = Fixture::new();
    let root = f.0.join("work");
    let admin = f.0.join("admin");
    fs::create_dir(&root).unwrap();
    git(
        &root,
        &["init", "-q", "--separate-git-dir", admin.to_str().unwrap()],
    );
    let events = Events::for_repository(
        &plico::git::Repository::new(&root),
        &plico::git::Cancel::default(),
    )
    .unwrap();
    fs::write(root.join("file.txt"), "staged\n").unwrap();
    git(&root, &["add", "file.txt"]);
    let batch = wait_git_change(&events);
    assert!(batch.membership, "index changes may change gitlinks");
}
#[test]
fn linked_worktree_ignores_main_index_but_observes_own_index_and_shared_refs() {
    let f = Fixture::new();
    git(&f.0, &["init", "-q", "-b", "main"]);
    git(&f.0, &["config", "user.name", "Test"]);
    git(&f.0, &["config", "user.email", "test@example.invalid"]);
    git(
        &f.0,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            "base",
        ],
    );
    let root = f.0.join("linked");
    git(
        &f.0,
        &["worktree", "add", "-qb", "topic", root.to_str().unwrap()],
    );
    let events = Events::for_repository(
        &plico::git::Repository::new(&root),
        &plico::git::Cancel::default(),
    )
    .unwrap();
    fs::write(f.0.join("main-only.txt"), "main\n").unwrap();
    git(&f.0, &["add", "main-only.txt"]);
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        !events.take().changed(),
        "another worktree's index is not ours"
    );
    fs::write(root.join("own.txt"), "topic\n").unwrap();
    git(&root, &["add", "own.txt"]);
    let staged = wait_git_change(&events);
    assert!(
        staged.membership && staged.index && !staged.history_changed(),
        "{staged:?}"
    );
    git(&f.0, &["branch", "new-shared-ref"]);
    let references = wait_git_change(&events);
    assert!(references.refs && !references.index && references.history_changed());
    assert!(
        !references.membership,
        "refs must not invalidate project membership"
    );
}

#[test]
fn project_observers_keep_root_identity_retain_unchanged_and_release_removed_roots() {
    use plico::{
        git::{Cancel, Repository},
        worktree_events::ProjectEvents,
    };
    let f = Fixture::new();
    let first = f.0.join("first");
    let second = f.0.join("second");
    for root in [&first, &second] {
        fs::create_dir(root).unwrap();
        git(root, &["init", "-q"]);
    }
    let first_repo = Repository::new(&first);
    let second_repo = Repository::new(&second);
    let token = Cancel::default();
    let mut events = ProjectEvents::default();
    events
        .reconcile(&[first_repo.clone(), second_repo.clone()], &token)
        .unwrap();
    assert_eq!(events.take(Instant::now()).len(), 2);
    events
        .reconcile(&[first_repo.clone(), second_repo], &token)
        .unwrap();
    assert!(
        events.take(Instant::now()).is_empty(),
        "unchanged watchers must not be recreated"
    );
    fs::write(first.join("same.txt"), "first\n").unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let notices = events.take(Instant::now());
    assert!(
        notices
            .iter()
            .any(|n| n.root == first && n.batch.affects(std::path::Path::new("same.txt")))
    );
    assert!(
        !notices
            .iter()
            .any(|n| n.root == second && n.batch.changed())
    );
    fs::write(second.join("same.txt"), "queued before removal\n").unwrap();
    events.reconcile(&[first_repo], &token).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(!events.take(Instant::now()).iter().any(|n| n.root == second));
    events.reconcile(&[], &token).unwrap();
    assert!(
        events
            .take(Instant::now() + Duration::from_secs(60))
            .is_empty()
    );
    token.cancel();
    assert!(events.reconcile(&[Repository::new(first)], &token).is_err());
}
