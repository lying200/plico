//! Request-scoped metadata shared by history pages and their refresh baseline.
//! Rebased 2450423: GitRepositoryImpl repoInfo and GitLogProvider readFirstBlock.
//! Never reuse this input for writes or a later refresh request.
use super::{Cancel, Repository, Result};
use std::{collections::HashSet, path::PathBuf};

pub(crate) struct HistoryInputs {
    pub path: PathBuf,
    pub refs: String,
    pub remotes: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub shallow: (bool, HashSet<String>),
}
impl HistoryInputs {
    pub fn observation(&self) -> Result<crate::branch_updates::HistoryObservation> {
        // Normalize to the lightweight observer's existing comparison format.
        // Its timer does not need to pay for tag peeling or remote discovery.
        let mut refs = String::new();
        for line in self.refs.lines() {
            let fields = line.split('\0').collect::<Vec<_>>();
            if fields.len() != 10 {
                return Err("Invalid refs record".into());
            }
            refs.push_str(
                &[
                    fields[0], fields[1], fields[7], fields[3], fields[4], fields[8], fields[9],
                ]
                .join("\t"),
            );
            refs.push('\n');
        }
        Ok(crate::branch_updates::HistoryObservation::from_parts(
            crate::branch_updates::References::from_records(refs, self.head.clone()),
            self.shallow.clone(),
        ))
    }
}
impl Repository {
    pub(crate) fn history_inputs(&self, token: &Cancel) -> Result<HistoryInputs> {
        self.text(&["rev-parse", "--git-dir"], token)?;
        let refs = self.text(&[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)%00%(*objectname)%00%(upstream)%00%(upstream:remotename)%00%(objecttype)%00%(*objecttype)%00%(HEAD)%00%(upstream:remoteref)%00%(push)",
            "refs/heads", "refs/remotes", "refs/tags", "refs/replace",
        ], token)?;
        let remotes = self.text(&["remote"], token)?;
        let head = self
            .text(&["rev-parse", "--verify", "HEAD"], token)
            .ok()
            .map(|v| v.trim().to_owned());
        // No HEAD=* row exists on an unborn branch. Preserve symbolic HEAD as
        // an independent input rather than misclassifying it as detached.
        let branch = self
            .text(&["symbolic-ref", "--quiet", "HEAD"], token)
            .ok()
            .and_then(|v| v.trim_end().strip_prefix("refs/heads/").map(str::to_owned));
        let shallow = self.shallow_boundaries(token)?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(HistoryInputs {
            path: self.path.clone(),
            refs,
            remotes,
            head,
            branch,
            shallow,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "plico-history-inputs-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            let f = Self(path);
            f.git(&["init", "-b", "main"]);
            f.git(&["config", "user.name", "Test"]);
            f.git(&["config", "user.email", "test@example.invalid"]);
            f.git(&["commit", "--allow-empty", "-m", "base"]);
            f
        }
        fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .arg("-C")
                .arg(&self.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap().trim().into()
        }
        fn repo(&self) -> Repository {
            Repository::new(&self.0)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn history_input_stays_bound_to_its_root_and_pre_read_refs() {
        let f = Fixture::new();
        let repo = f.repo();
        let token = Cancel::default();
        f.git(&["tag", "-a", "v1", "-m", "annotated"]);
        let inputs = repo.history_inputs(&token).unwrap();
        let old_head = inputs.head.clone().unwrap();
        let before = inputs.observation().unwrap();
        assert_eq!(
            before,
            crate::branch_updates::HistoryObservation::read(&repo, &token).unwrap()
        );
        f.git(&["commit", "--allow-empty", "-m", "external"]);
        f.git(&["branch", "external-branch"]);
        f.git(&["tag", "-f", "v1"]);
        let snapshot = repo
            .snapshot_branches_with_inputs(&inputs, &[], 0, 20, None, None, &token)
            .unwrap();
        assert_eq!(snapshot.head_id.as_ref(), Some(&old_head));
        assert_eq!(snapshot.commits.len(), 1);
        assert_eq!(snapshot.commits[0].id, old_head);
        assert!(
            !snapshot
                .branches
                .iter()
                .any(|b| b.name == "refs/heads/external-branch")
        );
        assert_eq!(snapshot.tags[0].id, old_head);
        assert!(!before.unchanged(&repo, &token));
        let other = Fixture::new();
        assert!(
            other
                .repo()
                .snapshot_branches_with_inputs(&inputs, &[], 0, 20, None, None, &token)
                .is_err()
        );
        token.cancel();
        assert!(
            repo.snapshot_branches_with_inputs(&inputs, &[], 0, 20, None, None, &token)
                .is_err()
        );
    }

    #[test]
    fn history_observation_matches_lightweight_poll_for_head_and_tracking_states() {
        let f = Fixture::new();
        let repo = f.repo();
        let token = Cancel::default();
        let compare = || {
            let inputs = repo.history_inputs(&token).unwrap();
            assert_eq!(
                inputs.observation().unwrap(),
                crate::branch_updates::HistoryObservation::read(&repo, &token).unwrap()
            );
            inputs
        };
        f.git(&["tag", "-a", "annotated", "-m", "tag"]);
        f.git(&["tag", "tree", &f.git(&["rev-parse", "HEAD^{tree}"])]);
        compare();
        f.git(&["checkout", "--orphan", "unborn"]);
        let unborn = compare();
        assert_eq!(unborn.branch.as_deref(), Some("unborn"));
        assert!(unborn.head.is_none());
        f.git(&["checkout", "--detach", "main"]);
        assert!(compare().branch.is_none());
        f.git(&["checkout", "main"]);
        f.git(&["branch", "upstream"]);
        f.git(&["branch", "--set-upstream-to=upstream"]);
        compare();
        f.git(&["remote", "add", "origin", "../unused-origin.git"]);
        f.git(&["remote", "add", "fork", "../unused-fork.git"]);
        let head = f.git(&["rev-parse", "HEAD"]);
        f.git(&["update-ref", "refs/remotes/origin/main", &head]);
        f.git(&["update-ref", "refs/remotes/fork/main", &head]);
        f.git(&["branch", "--set-upstream-to=origin/main"]);
        f.git(&["config", "remote.pushDefault", "fork"]);
        f.git(&["config", "push.default", "current"]);
        f.git(&["commit", "--allow-empty", "-m", "outgoing"]);
        let observation = compare().observation().unwrap();
        let counts = observation.references.counts(&repo, &token).unwrap();
        assert_eq!((counts.incoming, counts.outgoing), (0, 1));
    }
}
