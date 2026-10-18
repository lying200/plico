//! Opt-in, one-shot startup diagnostics; never enabled by normal window creation.
use std::{io::Write, path::PathBuf, sync::atomic::{AtomicBool, Ordering}, time::Instant};

pub struct Probe { started: Instant, path: PathBuf, done: AtomicBool }
#[derive(Clone, Copy)]
pub struct Frame {
    pub welcome: bool,
    pub requested: usize,
    pub commits: usize,
    pub graph_rows: usize,
    pub max_columns: usize,
    pub offset: usize,
    pub more: bool,
    pub filtered: bool,
}
impl Probe {
    pub fn new(path: PathBuf, started: Instant) -> Self { Self { started, path, done: AtomicBool::new(false) } }
    pub fn done(&self) -> bool { self.done.load(Ordering::Relaxed) }
    /// Called from prepaint after the initial batch and graph have been published.
    /// This is not a GPU presentation timestamp; external validation must observe
    /// the actual frame and perform a real operation before calling it interactive.
    pub fn record(&self, frame: Frame) {
        if self.done.swap(true, Ordering::Relaxed) { return; }
        let elapsed = self.started.elapsed().as_secs_f64() * 1000.;
        let stage = if frame.welcome { "welcome-prepaint" } else { "history-prepaint" };
        let json = format!("{{\"format\":1,\"stage\":\"{stage}\",\"rust_elapsed_ms\":{elapsed:.3},\"requested\":{},\"commits\":{},\"graph_rows\":{},\"max_columns\":{},\"offset\":{},\"more\":{},\"filtered\":{}}}\n",
            frame.requested, frame.commits, frame.graph_rows, frame.max_columns, frame.offset, frame.more, frame.filtered);
        let result = std::fs::OpenOptions::new().write(true).create_new(true).open(&self.path)
            .and_then(|mut file| file.write_all(json.as_bytes()));
        if let Err(error) = result { eprintln!("Plico startup report: {error}"); }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_preserves_existing_files_and_records_only_once() {
        let path = std::env::temp_dir().join(format!("plico-startup-{}-{}.json", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let frame = Frame { welcome: false, requested: 1000, commits: 1000, graph_rows: 1000, max_columns: 1, offset: 0, more: true, filtered: false };
        let probe = Probe::new(path.clone(), Instant::now());
        probe.record(frame);
        let first = std::fs::read_to_string(&path).unwrap();
        assert!(first.contains("\"commits\":1000,\"graph_rows\":1000"));
        probe.record(Frame { commits: 1, ..frame });
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
        let another = Probe::new(path.clone(), Instant::now());
        another.record(Frame { commits: 2, ..frame });
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
        std::fs::remove_file(path).unwrap();
    }
}
