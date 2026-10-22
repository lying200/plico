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
