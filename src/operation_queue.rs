//! Rebased GitRemoteOperationQueueImpl-style cancellable ownership.
//! Local adaptation: serialize by canonical common Git directory, including linked
//! worktrees, since the planned write operations can share refs and object storage.
use crate::git::Cancel;
use std::{collections::HashSet, path::PathBuf, sync::{Mutex, OnceLock}, thread, time::Duration};
fn busy() -> &'static Mutex<HashSet<PathBuf>> {
    static BUSY: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    BUSY.get_or_init(Mutex::default)
}
pub(crate) struct Guard(PathBuf);
pub(crate) fn acquire(common_dir: PathBuf, cancel: &Cancel) -> Result<Guard, String> {
    loop {
        if cancel.cancelled() { return Err("Cancelled".into()); }
        if busy().lock().unwrap_or_else(|e| e.into_inner()).insert(common_dir.clone()) {
            return Ok(Guard(common_dir));
        }
        thread::sleep(Duration::from_millis(50));
    }
}
impl Drop for Guard {
    fn drop(&mut self) { busy().lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0); }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_and_release_do_not_block_other_repositories() {
        let key = PathBuf::from("queue-test-shared");
        let guard = acquire(key.clone(), &Cancel::default()).unwrap();
        let token = Cancel::default();
        let worker_token = token.clone();
        let worker_key = key.clone();
        let worker = thread::spawn(move || acquire(worker_key, &worker_token).is_err());
        let _other = acquire(PathBuf::from("queue-test-other"), &Cancel::default()).unwrap();
        token.cancel();
        assert!(worker.join().unwrap());
        drop(guard);
        let _next = acquire(key, &Cancel::default()).unwrap();
    }
}
