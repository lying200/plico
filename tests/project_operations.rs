use plico::{
    git::{Cancel, Repository},
    operation_state::Operation,
    project_operations::Observer,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!("plico-project-operations-{nonce:x?}"));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn repo(&self, name: &str) -> Repository {
        let root = self.0.join(name);
        std::fs::create_dir(&root).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.hooksPath", "disabled-hooks"],
        ] {
            git(&root, &args, true);
        }
        std::fs::write(root.join("a"), "base\n").unwrap();
        git(&root, &["add", "."], true);
        git(&root, &["commit", "-qm", "base"], true);
        Repository::new(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn git(root: &Path, args: &[&str], success: bool) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(out.status.success(), success, "{args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().into()
}
#[test]
fn external_child_merge_resolution_and_abort_are_observed_without_ref_changes() {
    let fixture = Fixture::new();
    let parent = fixture.repo("parent");
    let child = fixture.repo("child");
    git(&child.path, &["checkout", "-qb", "topic"], true);
    std::fs::write(child.path.join("a"), "theirs\n").unwrap();
    git(&child.path, &["commit", "-qam", "theirs"], true);
    git(&child.path, &["checkout", "main"], true);
    std::fs::write(child.path.join("a"), "ours\n").unwrap();
    git(&child.path, &["commit", "-qam", "ours"], true);
    let roots = [parent.clone(), child.clone()];
    let token = Cancel::default();
    let mut observer = Observer::default();
    let initial = observer.poll(&roots, &token).unwrap().unwrap();
    assert!(
        initial
            .iter()
            .all(|e| !e.state.as_ref().unwrap().in_progress())
    );
    assert!(observer.poll(&roots, &token).unwrap().is_none());
    let head = git(&child.path, &["rev-parse", "HEAD"], true);
    git(&child.path, &["merge", "topic"], false);
    let state = observer.poll(&roots, &token).unwrap().unwrap();
    assert_eq!(state[1].state.as_ref().unwrap().operation, Operation::Merge);
    assert_eq!(
        state[1].state.as_ref().unwrap().conflicts,
        vec![PathBuf::from("a")]
    );
    assert!(!state[0].state.as_ref().unwrap().in_progress());
    let reopened = Observer::default().poll(&roots, &token).unwrap().unwrap();
    assert_eq!(
        reopened[1].state, state[1].state,
        "no in-process operation required"
    );
    git(&child.path, &["checkout", "--ours", "--", "a"], true);
    git(&child.path, &["add", "a"], true);
    let resolved = observer.poll(&roots, &token).unwrap().unwrap();
    assert!(resolved[1].state.as_ref().unwrap().conflicts.is_empty());
    assert_eq!(
        resolved[1].state.as_ref().unwrap().operation,
        Operation::Merge
    );
    assert_eq!(git(&child.path, &["rev-parse", "HEAD"], true), head);
    git(&child.path, &["merge", "--abort"], true);
    assert!(
        !observer.poll(&roots, &token).unwrap().unwrap()[1]
            .state
            .as_ref()
            .unwrap()
            .in_progress()
    );
    assert_eq!(
        state[1].state.as_ref().unwrap().conflicts.len(),
        1,
        "immutable prior snapshot"
    );
}
#[test]
fn membership_errors_and_cancellation_never_turn_unknown_into_idle() {
    let fixture = Fixture::new();
    let first = fixture.repo("first");
    let second = fixture.repo("second");
    let roots = [first.clone(), second.clone()];
    let mut observer = Observer::default();
    let token = Cancel::default();
    observer.poll(&roots, &token).unwrap().unwrap();
    std::fs::rename(second.path.join(".git"), second.path.join("saved-git")).unwrap();
    let failed = observer.poll(&roots, &token).unwrap().unwrap();
    assert!(failed[1].state.is_err());
    assert!(failed[0].state.is_ok());
    std::fs::rename(second.path.join("saved-git"), second.path.join(".git")).unwrap();
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(observer.poll(&roots, &cancelled).is_err());
    assert!(
        observer.poll(&roots, &token).unwrap().unwrap()[1]
            .state
            .is_ok()
    );
    assert_eq!(
        observer.poll(&roots[..1], &token).unwrap().unwrap().len(),
        1
    );
    assert!(observer.poll(&roots[..1], &token).unwrap().is_none());
    assert!(observer.poll(&[], &cancelled).is_err());
    assert!(observer.poll(&[], &token).unwrap().unwrap().is_empty());
}
