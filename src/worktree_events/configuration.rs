//! Effective configuration, including Git's includeIf and worktree precedence.
//! Retain only a digest, never configuration values (which may contain secrets).
use crate::git::{Cancel, Repository};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct Configuration {
    fingerprint: Option<([u8; 32], [u8; 32])>,
    next: Option<Instant>,
    pending: bool,
    requested: bool,
    broad: bool,
    ignore_rules: super::ignore_rules::IgnoreRules,
}
impl Configuration {
    pub fn due(&self, now: Instant) -> bool {
        self.requested || self.next.is_none_or(|next| now >= next)
    }
    pub fn request(&mut self) {
        self.requested = true;
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn diff_only(&self) -> bool {
        self.pending && !self.broad
    }
    pub fn status_rules_pending(&self) -> bool {
        self.ignore_rules.pending()
    }
    pub fn acknowledge_status_rules(&mut self) {
        self.ignore_rules.acknowledge();
    }
    pub fn acknowledge(&mut self) {
        self.pending = false;
        self.broad = false;
    }
    pub fn poll(&mut self, repo: &Repository, token: &Cancel, now: Instant) -> Result<(), String> {
        // One eligible root per interval, not per selection. Resolve external
        // ignore paths only when config changed or resolution previously failed.
        let result = repo.read(
            &[
                OsStr::new("config"),
                OsStr::new("--null"),
                OsStr::new("--list"),
                OsStr::new("--includes"),
            ],
            token,
        );
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let fingerprint = result.ok().map(|bytes| {
            let mut other = Sha256::new();
            for record in bytes.split(|b| *b == 0).filter(|r| !r.is_empty()) {
                let key = record.split(|b| *b == b'\n').next().unwrap();
                if !key.starts_with(b"diff.") {
                    other.update(record);
                    other.update([0]);
                }
            }
            (Sha256::digest(bytes).into(), other.finalize().into())
        });
        self.ignore_rules.poll(
            repo,
            token,
            fingerprint.is_none() || self.fingerprint != fingerprint,
        )?;
        // Errors are unknown, not evidence that the old interpretation is valid.
        self.pending |= fingerprint.is_none() || self.fingerprint != fingerprint;
        self.broad |= fingerprint.is_none()
            || self.fingerprint.is_none()
            || self.fingerprint.as_ref().map(|f| &f.1) != fingerprint.as_ref().map(|f| &f.1);
        self.fingerprint = fingerprint;
        self.requested = false;
        self.next = Some(now + Duration::from_secs(30));
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    pub(in crate::worktree_events) struct Fixture(pub(in crate::worktree_events) PathBuf);
    impl Fixture {
        pub(in crate::worktree_events) fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "plico-config-observation-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            let f = Self(root);
            f.git(&["init", "-b", "main"]);
            f.git(&["config", "include.path", "observed-include"]);
            f
        }
        pub(in crate::worktree_events) fn git(&self, args: &[&str]) {
            let output = Command::new("git")
                .arg("-C")
                .arg(&self.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn diff_only_changes_stay_narrow_but_unknown_and_unacknowledged_changes_do_not() {
        let fixture = Fixture::new();
        let repo = Repository::new(&fixture.0);
        let token = Cancel::default();
        let now = Instant::now();
        let mut state = Configuration::default();
        state.poll(&repo, &token, now).unwrap();
        assert!(state.pending() && !state.diff_only());
        state.acknowledge();
        fixture.git(&["config", "diff.renameLimit", "1"]);
        state.poll(&repo, &token, now).unwrap();
        assert!(state.diff_only());
        // A wider change arriving before delivery upgrades the pending scope.
        fixture.git(&["config", "core.abbrev", "12"]);
        state.poll(&repo, &token, now).unwrap();
        assert!(state.pending() && !state.diff_only());
        fixture.git(&["config", "diff.renameLimit", "1000"]);
        state.poll(&repo, &token, now).unwrap();
        assert!(
            !state.diff_only(),
            "a later narrow change cannot erase broad pending work"
        );
        state.acknowledge();
        fixture.git(&["config", "--unset", "diff.renameLimit"]);
        state.poll(&repo, &token, now).unwrap();
        assert!(
            state.diff_only(),
            "removing a diff setting is also a narrow change"
        );
        state.acknowledge();
        state.poll(&repo, &token, now).unwrap();
        assert!(!state.pending());
    }
    #[test]
    fn configuration_observes_missing_include_creation_errors_and_delivery() {
        let fixture = Fixture::new();
        let repo = Repository::new(&fixture.0);
        let token = Cancel::default();
        let now = Instant::now();
        let mut observation = Configuration::default();
        observation.poll(&repo, &token, now).unwrap();
        assert!(observation.pending());
        assert!(!observation.due(now + Duration::from_secs(29)));
        assert!(observation.due(now + Duration::from_secs(30)));
        observation.acknowledge();
        observation.poll(&repo, &token, now).unwrap();
        assert!(!observation.pending());
        let common_before = fs::read(fixture.0.join(".git/config")).unwrap();
        fs::write(
            fixture.0.join(".git/observed-include"),
            "[diff]\nrenameLimit = 1\n",
        )
        .unwrap();
        observation.poll(&repo, &token, now).unwrap();
        assert!(observation.pending());
        // Re-reading unchanged config must not acknowledge a refused delivery.
        observation.poll(&repo, &token, now).unwrap();
        assert!(observation.pending());
        assert_eq!(
            common_before,
            fs::read(fixture.0.join(".git/config")).unwrap()
        );
        observation.acknowledge();
        fs::write(fixture.0.join(".git/observed-include"), "invalid config\n").unwrap();
        observation.poll(&repo, &token, now).unwrap();
        assert!(observation.pending());
        observation.acknowledge();
        observation.poll(&repo, &token, now).unwrap();
        assert!(
            observation.pending(),
            "unreadable configuration remains unknown"
        );
        observation.acknowledge();
        fs::remove_file(fixture.0.join(".git/observed-include")).unwrap();
        observation.poll(&repo, &token, now).unwrap();
        assert!(observation.pending(), "recovery must invalidate too");
        observation.acknowledge();
        let fingerprint = observation.fingerprint;
        let deadline = observation.next;
        token.cancel();
        assert!(observation.poll(&repo, &token, now).is_err());
        assert_eq!(observation.fingerprint, fingerprint);
        assert_eq!(observation.next, deadline);
        assert!(!observation.pending());
    }
}
