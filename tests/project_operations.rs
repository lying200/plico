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
        plico::operation_inspection::Inspection::read(&child, &token)
            .state
            .as_ref()
            .unwrap(),
        state[1].state.as_ref().unwrap().as_ref()
    );
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
    assert!(
        plico::operation_inspection::Inspection::read(&child, &token)
            .state
            .unwrap()
            .conflicts
            .is_empty()
    );
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

#[test]
fn operation_observer_does_not_hide_config_errors_behind_unchanged_markers() {
    use plico::operation_inspection::Inspection;
    for mode in ["local", "include", "conditional-include"] {
        let fixture = Fixture::new();
        let healthy = fixture.repo("healthy");
        let affected = fixture.repo("affected");
        let config = affected.path.join(".git/config");
        let changed = if mode == "local" {
            config.clone()
        } else {
            let external = fixture.0.join("external config");
            git(
                &affected.path,
                &[
                    "config",
                    if mode == "include" {
                        "include.path"
                    } else {
                        "includeIf.onbranch:main.path"
                    },
                    external.to_str().unwrap(),
                ],
                true,
            );
            external
        };
        let original = std::fs::read(&changed).ok();
        let local_config = std::fs::read(&config).unwrap();
        let head = std::fs::read(affected.path.join(".git/HEAD")).unwrap();
        let index = std::fs::read(affected.path.join(".git/index")).unwrap();
        let roots = [healthy, affected.clone()];
        let token = Cancel::default();
        let mut observer = Observer::default();
        let initial = observer.poll(&roots, &token).unwrap().unwrap();
        assert!(initial.iter().all(|entry| entry.state.is_ok()));
        assert!(observer.poll(&roots, &token).unwrap().is_none());

        // Missing includes becoming malformed must invalidate too, even though
        // they never appeared as a successfully loaded configuration origin.
        std::fs::write(&changed, "[broken\n").unwrap();
        let invalid = observer.poll(&roots, &token).unwrap().unwrap();
        assert_eq!(invalid[0].state, initial[0].state, "{mode}");
        assert!(invalid[1].state.is_err(), "{mode}");
        let inspection = Inspection::read(&affected, &token);
        assert!(inspection.state.is_err(), "{mode}");
        assert_eq!(inspection.recovery.flags(), (true, true, true));
        assert_eq!(
            std::fs::read(affected.path.join(".git/HEAD")).unwrap(),
            head
        );
        assert_eq!(
            std::fs::read(affected.path.join(".git/index")).unwrap(),
            index
        );
        if mode != "local" {
            assert_eq!(std::fs::read(&config).unwrap(), local_config);
        }

        if let Some(bytes) = original {
            std::fs::write(&changed, bytes).unwrap();
        } else {
            std::fs::remove_file(&changed).unwrap();
        }
        let cancelled = Cancel::default();
        cancelled.cancel();
        assert!(observer.poll(&roots, &cancelled).is_err());
        let recovered = observer.poll(&roots, &token).unwrap().unwrap();
        assert_eq!(recovered[1].state, initial[1].state, "{mode}");
        assert!(observer.poll(&roots, &token).unwrap().is_none());
        assert!(
            invalid[1].state.is_err(),
            "published errors remain immutable"
        );
    }
}

#[test]
fn operation_inspection_uses_each_worktree_and_reresolves_changed_git_file() {
    use plico::operation_inspection::{Inspection, RecoveryPresence};
    let fixture = Fixture::new();
    let main = fixture.repo("main");
    let first = Repository::new(fixture.0.join("first"));
    let second = Repository::new(fixture.0.join("second"));
    for (name, repo) in [("first", &first), ("second", &second)] {
        git(
            &main.path,
            &["worktree", "add", "-b", name, repo.path.to_str().unwrap()],
            true,
        );
    }
    let token = Cancel::default();
    let main_dir = main.editor_directory(&token).unwrap();
    let first_dir = first.editor_directory(&token).unwrap();
    let second_dir = second.editor_directory(&token).unwrap();
    // A corrupt record in the common/main directory must not leak into linked
    // worktree chrome. A corrupt own record remains conservatively present.
    std::fs::write(main_dir.join("plico-merge-saved"), "broken").unwrap();
    std::fs::write(first_dir.join("plico-revert-queue"), "broken").unwrap();
    std::fs::write(second_dir.join("plico-revert-saved"), "broken").unwrap();
    let head = git(&main.path, &["rev-parse", "HEAD"], true);
    std::fs::write(second_dir.join("MERGE_HEAD"), format!("{head}\n")).unwrap();
    assert_eq!(
        RecoveryPresence::read(&main, &token).flags(),
        (true, false, false)
    );
    let initial = Inspection::read(&first, &token);
    assert_eq!(initial.recovery.flags(), (false, true, false));
    assert_eq!(initial.state.unwrap().operation, Operation::None);
    let (previous, _) = plico::operation_state::observe(&first, None, &token)
        .unwrap()
        .unwrap();
    let original_git_file = std::fs::read(first.path.join(".git")).unwrap();
    std::fs::write(
        first.path.join(".git"),
        format!("gitdir: {}\n", second_dir.display()),
    )
    .unwrap();
    let redirected = Inspection::read(&first, &token);
    assert_eq!(redirected.recovery.flags(), (false, false, true));
    assert_eq!(redirected.state.unwrap().operation, Operation::Merge);
    let (_, state) = plico::operation_state::observe(&first, Some(&previous), &token)
        .unwrap()
        .unwrap();
    assert_eq!(state.operation, Operation::Merge);
    std::fs::write(first.path.join(".git"), original_git_file).unwrap();
    assert_eq!(
        Inspection::read(&first, &token).recovery.flags(),
        (false, true, false)
    );
}

#[test]
fn operation_inspection_preserves_unknown_and_cancellation_guards() {
    use plico::operation_inspection::{Inspection, RecoveryPresence};
    let fixture = Fixture::new();
    let repo = fixture.repo("repo");
    let token = Cancel::default();
    let cancelled = Cancel::default();
    cancelled.cancel();
    let cancelled_inspection = Inspection::read(&repo, &cancelled);
    assert!(cancelled_inspection.state.is_err());
    assert_eq!(cancelled_inspection.recovery.flags(), (true, true, true));
    assert_eq!(
        RecoveryPresence::read(&repo, &cancelled).flags(),
        (true, true, true)
    );
    std::fs::rename(repo.path.join(".git"), repo.path.join("saved-git")).unwrap();
    let missing = Inspection::read(&repo, &token);
    assert!(missing.state.is_err());
    assert_eq!(missing.recovery.flags(), (true, true, true));
    std::fs::rename(repo.path.join("saved-git"), repo.path.join(".git")).unwrap();
    let directory = repo.editor_directory(&token).unwrap();
    std::fs::create_dir(directory.join("plico-merge-saved")).unwrap();
    let corrupt = Inspection::read(&repo, &token);
    assert_eq!(corrupt.state.unwrap().operation, Operation::None);
    assert_eq!(corrupt.recovery.flags(), (true, false, false));
}

#[test]
fn project_conflicts_keep_same_named_files_and_root_specific_write_guards() {
    use plico::conflicts::project;
    let fixture = Fixture::new();
    let parent = fixture.repo("parent");
    let child = fixture.repo("child");
    for repo in [&parent, &child] {
        git(&repo.path, &["checkout", "-qb", "topic"], true);
        std::fs::write(repo.path.join("a"), "theirs\n").unwrap();
        git(&repo.path, &["commit", "-qam", "theirs"], true);
        git(&repo.path, &["checkout", "main"], true);
        std::fs::write(repo.path.join("a"), "ours\n").unwrap();
        git(&repo.path, &["commit", "-qam", "ours"], true);
        git(&repo.path, &["merge", "topic"], false);
    }
    let token = Cancel::default();
    let entries = project::read(&[parent.clone(), child.clone(), parent.clone()], &token).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].file().path, entries[1].file().path);
    assert_ne!(entries[0].key(), entries[1].key());
    assert!(entries[0].full_path() < entries[1].full_path());
    for entry in &entries {
        let own = entry.snapshot().repository();
        let other = if own.path == parent.path {
            &child
        } else {
            &parent
        };
        entry
            .snapshot()
            .revalidate(&own, entry.file(), &token)
            .unwrap();
        assert!(
            entry
                .snapshot()
                .revalidate(other, entry.file(), &token)
                .is_err()
        );
    }
    git(&child.path, &["merge", "--abort"], true);
    let remaining = project::read(&[parent.clone(), child], &token).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].snapshot().repository().path, parent.path);
    assert!(
        project::read(
            &[parent.clone(), Repository::new(fixture.0.join("missing"))],
            &token
        )
        .is_err()
    );
    token.cancel();
    assert!(project::read(&[parent], &token).is_err());
}

#[test]
fn finalizing_drafts_uses_each_root_and_preserves_partial_success() {
    use plico::{
        conflicts::Snapshot,
        merge_result::{Prepared, stage_resolved},
    };
    use std::sync::Arc;
    let fixture = Fixture::new();
    let first = fixture.repo("first");
    let second = fixture.repo("second");
    let third = fixture.repo("third");
    let token = Cancel::default();
    let mut drafts = Vec::new();
    for repo in [&first, &second, &third] {
        git(&repo.path, &["checkout", "-qb", "topic"], true);
        std::fs::write(repo.path.join("a"), "theirs\n").unwrap();
        git(&repo.path, &["commit", "-qam", "theirs"], true);
        git(&repo.path, &["checkout", "main"], true);
        std::fs::write(repo.path.join("a"), "ours\n").unwrap();
        git(&repo.path, &["commit", "-qam", "ours"], true);
        git(&repo.path, &["merge", "topic"], false);
        let snapshot = Snapshot::read(repo, &token).unwrap();
        let prepared =
            Arc::new(Prepared::read(repo, &snapshot, &snapshot.files[0], &token).unwrap());
        prepared.write_text_result("resolved\n".into()).unwrap();
        prepared.apply(repo, &token).unwrap();
        drafts.push(prepared);
    }
    std::fs::write(second.path.join("a"), "external edit\n").unwrap();
    let result = stage_resolved(&drafts, &token);
    assert!(result.error.is_some());
    assert_eq!(result.completed.len(), 1);
    assert_eq!(result.completed[0].repository.path, first.path);
    assert_eq!(result.snapshots.len(), 1);
    assert_eq!(result.touched.len(), 2);
    assert!(git(&first.path, &["ls-files", "-u"], true).is_empty());
    for repo in [&second, &third] {
        assert_eq!(
            git(&repo.path, &["ls-files", "-u"], true).lines().count(),
            3
        );
    }
    assert_eq!(
        std::fs::read_to_string(second.path.join("a")).unwrap(),
        "external edit\n"
    );
    // Resume only the untouched third root; the failure never staged it implicitly.
    let resumed = stage_resolved(&drafts[2..], &token);
    assert!(resumed.error.is_none(), "{:?}", resumed.error);
    assert_eq!(resumed.completed[0].repository.path, third.path);
    assert_ne!(result.completed[0].key, resumed.completed[0].key);
    assert!(git(&third.path, &["ls-files", "-u"], true).is_empty());
    token.cancel();
    let cancelled = stage_resolved(&drafts[1..2], &token);
    assert!(cancelled.error.is_some());
    assert!(cancelled.completed.is_empty());
    assert!(cancelled.touched.is_empty());
}
