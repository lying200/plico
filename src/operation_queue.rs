//! Rebased GitRemoteOperationQueueImpl-style cancellable ownership.
//! Local adaptation: serialize by canonical common Git directory, including linked
//! worktrees, since the planned write operations can share refs and object storage.
use crate::git::Cancel;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};
fn busy() -> &'static Mutex<HashSet<PathBuf>> {
    static BUSY: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    BUSY.get_or_init(Mutex::default)
}
pub(crate) struct Guard(Vec<PathBuf>);
pub(crate) fn acquire(common_dir: PathBuf, cancel: &Cancel) -> Result<Guard, String> {
    acquire_many(vec![common_dir], cancel)
}
/// Reserve the entire set atomically: never wait while holding a subset of
/// repositories (recursive submodule operations can overlap in either order).
pub(crate) fn acquire_many(
    mut directories: Vec<PathBuf>,
    cancel: &Cancel,
) -> Result<Guard, String> {
    directories.sort();
    directories.dedup();
    loop {
        if cancel.cancelled() {
            return Err("Cancelled".into());
        }
        {
            let mut busy = busy().lock().unwrap_or_else(|e| e.into_inner());
            if directories
                .iter()
                .all(|directory| !busy.contains(directory))
            {
                busy.extend(directories.iter().cloned());
                return Ok(Guard(directories));
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let mut busy = busy().lock().unwrap_or_else(|e| e.into_inner());
        for directory in &self.0 {
            busy.remove(directory);
        }
    }
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
    #[test]
    fn overlapping_groups_release_together_and_waiting_groups_are_cancellable() {
        let a = PathBuf::from("queue-group-a");
        let b = PathBuf::from("queue-group-b");
        let group =
            acquire_many(vec![b.clone(), a.clone(), a.clone()], &Cancel::default()).unwrap();
        let token = Cancel::default();
        let pending_token = token.clone();
        let pending_a = a.clone();
        let worker = thread::spawn(move || acquire(pending_a, &pending_token).is_err());
        token.cancel();
        assert!(worker.join().unwrap());
        let available = PathBuf::from("queue-group-available");
        let pending = Cancel::default();
        let worker_token = pending.clone();
        let worker_available = available.clone();
        let worker_b = b.clone();
        let worker = thread::spawn(move || {
            acquire_many(vec![worker_available, worker_b], &worker_token).is_err()
        });
        // An overlapping wait must not reserve the unrelated member.
        let independent = acquire(available, &Cancel::default()).unwrap();
        pending.cancel();
        assert!(worker.join().unwrap());
        drop(independent);
        drop(group);
        let _reversed = acquire_many(vec![a, b], &Cancel::default()).unwrap();
    }
}
