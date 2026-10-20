//! Read-only profiling of the real detail/refresh/reset-preflight paths.
//! Optional second argument is a per-path latency budget in milliseconds.
use plico::{
    commit_changes,
    git::{Cancel, Repository},
    reset_operation,
};
use std::time::Instant;
fn main() {
    let repo = Repository::new(std::env::args_os().nth(1).expect("repository"));
    let budget = std::env::args()
        .nth(2)
        .map(|v| v.parse::<u128>().expect("milliseconds"));
    for n in 0..5 {
        let start = Instant::now();
        let output = std::process::Command::new("git")
            .arg("--version")
            .output()
            .unwrap();
        assert!(output.status.success());
        let raw = start.elapsed();
        let start = Instant::now();
        let mut cmd = std::process::Command::new("git");
        cmd.arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let mut tree = plico::process_tree::ProcessTree::spawn(&mut cmd).unwrap();
        let spawn = start.elapsed();
        assert!(tree.child.wait().unwrap().success());
        drop(tree);
        println!(
            "process {n}: raw={}ms owned-spawn={}ms owned-total={}ms",
            raw.as_millis(),
            spawn.as_millis(),
            start.elapsed().as_millis()
        );
    }
    let token = Cancel::default();
    let start = Instant::now();
    repo.operation_state(&token).unwrap();
    let history = plico::history::read(&repo, None, 0, 1000, None, None, &token).unwrap();
    let model = plico::graph_print::Model::new(&history.rows, &token).unwrap();
    model.project(false, &token).unwrap();
    println!(
        "log refresh (1000 commits): {}ms",
        start.elapsed().as_millis()
    );
    let snapshot = history.snapshot;
    let commit = snapshot.commits.first().expect("nonempty repository");
    let mut failed = false;
    for n in 0..5 {
        let start = Instant::now();
        repo.details(commit, &token).unwrap();
        let details = start.elapsed();
        let start = Instant::now();
        commit_changes::Snapshot::load(&repo, &token).unwrap();
        let refresh = start.elapsed();
        let start = Instant::now();
        reset_operation::Prepared::prepare(&repo, commit.id.clone(), &token).unwrap();
        let reset = start.elapsed();
        println!(
            "{n}: details={}ms refresh={}ms reset-prepare={}ms",
            details.as_millis(),
            refresh.as_millis(),
            reset.as_millis()
        );
        if budget.is_some_and(|b| [details, refresh, reset].iter().any(|d| d.as_millis() > b)) {
            failed = true;
        }
    }
    let mut cache = plico::detail_cache::Cache::default();
    let details = std::sync::Arc::new(repo.details(commit, &token).unwrap());
    cache.insert(&repo.path, commit, details);
    let start = Instant::now();
    for _ in 0..1000 {
        assert!(cache.get(&repo.path, commit).is_some());
    }
    println!(
        "1000 detail cache lookups: {}us",
        start.elapsed().as_micros()
    );
    if failed {
        std::process::exit(1);
    }
}
