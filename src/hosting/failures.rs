//! Rebased AccountUrlAuthenticationFailuresHolder: reject an account only for
//! the failed URL until its credential changes or the application restarts.
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, sync::Mutex};
static FAILURES: Mutex<Failures> = Mutex::new(Failures(VecDeque::new()));
pub fn clear_account(account: &str) {
    FAILURES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .0
        .retain(|v| v.account != account);
}
#[derive(Clone)]
pub(crate) struct Attempt {
    pub target: String,
    pub account: String,
    fingerprint: [u8; 32],
}
impl Attempt {
    pub fn new(target: &str, account: &str, secret: &str) -> Self {
        Self {
            target: target.into(),
            account: account.into(),
            fingerprint: Sha256::digest(secret.as_bytes()).into(),
        }
    }
    pub fn rejected(&self) -> bool {
        FAILURES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(self)
    }
    pub fn reject(self) {
        FAILURES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(self);
    }
}
struct Failures(VecDeque<Attempt>);
impl Failures {
    fn contains(&self, attempt: &Attempt) -> bool {
        self.0.iter().any(|v| {
            v.target == attempt.target
                && v.account == attempt.account
                && v.fingerprint == attempt.fingerprint
        })
    }
    fn insert(&mut self, attempt: Attempt) {
        self.0
            .retain(|v| v.target != attempt.target || v.account != attempt.account);
        self.0.push_back(attempt);
        if self.0.len() > 256 {
            self.0.pop_front();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failures_are_scoped_and_new_credentials_are_eligible() {
        let mut cache = Failures(VecDeque::new());
        cache.insert(Attempt::new("https://git/a", "alice", "old"));
        assert!(cache.contains(&Attempt::new("https://git/a", "alice", "old")));
        for attempt in [
            Attempt::new("https://git/a", "alice", "new"),
            Attempt::new("https://git/b", "alice", "old"),
            Attempt::new("https://git/a", "bob", "old"),
        ] {
            assert!(!cache.contains(&attempt));
        }
        for i in 0..300 {
            cache.insert(Attempt::new(&format!("https://git/{i}"), "alice", "old"));
        }
        assert_eq!(cache.0.len(), 256);
    }
}
