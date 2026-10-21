use plico::{
    conflicts::{self, Side, Snapshot},
    git::{Cancel, Repository},
};
use std::{fs, path::PathBuf, process::Command};
static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "plico-conflicts-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.hooksPath", "disabled-hooks"],
        ] {
            f.git(&args, true);
        }
        f
    }
    fn git(&self, args: &[&str], success: bool) {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            success,
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn snapshots_merge_and_rebase_versions_and_detects_external_resolution() {
    let f = Fixture::new();
    fs::write(f.0.join("a"), "base\n").unwrap();
    f.git(&["add", "a"], true);
    f.git(&["commit", "-m", "base"], true);
    f.git(&["checkout", "-b", "task"], true);
    fs::write(f.0.join("a"), "task\n").unwrap();
    f.git(&["commit", "-am", "task"], true);
    f.git(&["checkout", "main"], true);
    fs::write(f.0.join("a"), "main\n").unwrap();
    f.git(&["commit", "-am", "main"], true);
    f.git(&["merge", "task"], false);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let snap = Snapshot::read(&repo, &token).unwrap();
    assert!(!snap.reversed);
    let file = &snap.files[0];
    assert_eq!(
        conflicts::content(
            &repo,
            file.side(Side::Yours, snap.reversed).unwrap(),
            &token
        )
        .unwrap(),
        b"main\n"
    );
    assert_eq!(
        conflicts::content(
            &repo,
            file.side(Side::Theirs, snap.reversed).unwrap(),
            &token
        )
        .unwrap(),
        b"task\n"
    );
    snap.revalidate(&repo, file, &token).unwrap();
    fs::write(f.0.join("a"), "resolved manually\n").unwrap();
    let resolved = conflicts::resolve(
        &repo,
        &snap,
        file,
        conflicts::Resolution::MarkResolved,
        &token,
        &plico::operation_process::Progress::default(),
    )
    .unwrap();
    assert!(
        resolved
            .commands
            .iter()
            .all(|r| r.as_ref().is_ok_and(|o| o.status.success()))
    );
    assert!(resolved.snapshot.unwrap().files.is_empty());

    assert!(snap.revalidate(&repo, file, &token).is_err());
    f.git(&["merge", "--abort"], true);
    f.git(&["rebase", "--merge", "main", "task"], false);
    let snap = Snapshot::read(&repo, &token).unwrap();
    assert!(snap.reversed);
    assert_eq!(
        conflicts::content(
            &repo,
            snap.files[0].side(Side::Yours, snap.reversed).unwrap(),
            &token
        )
        .unwrap(),
        b"task\n"
    );
    assert_eq!(
        conflicts::content(
            &repo,
            snap.files[0].side(Side::Theirs, snap.reversed).unwrap(),
            &token
        )
        .unwrap(),
        b"main\n"
    );
    f.git(&["rebase", "--abort"], true);
}

#[test]
fn resolves_binary_and_modify_delete_without_touching_other_paths() {
    use conflicts::{Resolution, resolve};
    use plico::operation_process::Progress;
    for deleted in [false, true] {
        for side in [Side::Yours, Side::Theirs] {
            let f = Fixture::new();
            let path = "[a].bin";
            fs::write(f.0.join(path), b"base\0data").unwrap();
            fs::write(f.0.join("a.bin"), b"unrelated").unwrap();
            f.git(&["add", "."], true);
            f.git(&["commit", "-m", "base"], true);
            f.git(&["checkout", "-b", "task"], true);
            if deleted {
                f.git(&["--literal-pathspecs", "rm", "--", path], true);
                f.git(&["commit", "-m", "delete"], true);
            } else {
                fs::write(f.0.join(path), b"task\0data").unwrap();
                f.git(&["commit", "-am", "task"], true);
            }
            f.git(&["checkout", "main"], true);
            fs::write(f.0.join(path), b"main\0data").unwrap();
            f.git(&["commit", "-am", "main"], true);
            f.git(&["merge", "task"], false);
            fs::write(f.0.join("a.bin"), b"staged unrelated").unwrap();
            f.git(&["add", "a.bin"], true);
            fs::write(f.0.join("a.bin"), b"unstaged unrelated").unwrap();
            let repo = Repository::new(&f.0);
            let token = Cancel::default();
            let snap = Snapshot::read(&repo, &token).unwrap();
            let result = resolve(
                &repo,
                &snap,
                &snap.files[0],
                Resolution::Accept(side),
                &token,
                &Progress::default(),
            )
            .unwrap();
            for command in result.commands {
                let o = command.unwrap();
                assert!(o.status.success(), "{:?}", o.stderr);
                assert!(o.stopped.is_none());
            }
            assert!(result.snapshot.unwrap().files.is_empty());
            if deleted && side == Side::Theirs {
                assert!(!f.0.join(path).exists());
            } else {
                assert_eq!(
                    fs::read(f.0.join(path)).unwrap(),
                    if side == Side::Yours {
                        b"main\0data"
                    } else {
                        b"task\0data"
                    }
                );
            }
            assert_eq!(fs::read(f.0.join("a.bin")).unwrap(), b"unstaged unrelated");
            let staged = Command::new("git")
                .arg("-C")
                .arg(&f.0)
                .args(["show", ":a.bin"])
                .output()
                .unwrap();
            assert_eq!(staged.stdout, b"staged unrelated");
            assert!(
                resolve(
                    &repo,
                    &snap,
                    &snap.files[0],
                    Resolution::MarkResolved,
                    &token,
                    &Progress::default()
                )
                .is_err()
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn external_tool_output_is_isolated_and_rejects_changed_worktree() {
    use plico::external_merge::{Prepared, Tool};
    let f = Fixture::new();
    fs::write(f.0.join("a"), "base\n").unwrap();
    f.git(&["add", "a"], true);
    f.git(&["commit", "-m", "base"], true);
    f.git(&["checkout", "-b", "task"], true);
    fs::write(f.0.join("a"), "task\n").unwrap();
    f.git(&["commit", "-am", "task"], true);
    f.git(&["checkout", "main"], true);
    fs::write(f.0.join("a"), "main\n").unwrap();
    f.git(&["commit", "-am", "main"], true);
    f.git(&["merge", "task"], false);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let snap = Snapshot::read(&repo, &token).unwrap();
    let original = fs::read(f.0.join("a")).unwrap();
    let prepared = Prepared::read(&repo, &snap, &snap.files[0], &token).unwrap();
    let directory = prepared.temporary_directory().to_owned();
    let tool = Tool {
        executable: "/bin/sh".into(),
        arguments: vec![
            "-c".into(),
            "cat \"$1\" > \"$2\"".into(),
            "merge-test".into(),
            "%2".into(),
            "%4".into(),
        ],
    };
    let result = prepared.run(&tool, &token, &Default::default()).unwrap();
    assert!(result.status.success());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), original);
    assert!(!Snapshot::read(&repo, &token).unwrap().files.is_empty());
    // A separate failed result must survive closing its owner, without staging
    // or overwriting an intervening edit. Normal successful sessions still clean up.
    let recovery = Prepared::read(&repo, &snap, &snap.files[0], &token).unwrap();
    fs::write(recovery.output(), b"valuable resolved result\n").unwrap();
    fs::write(f.0.join("a"), "external edit\n").unwrap();
    assert!(recovery.apply(&repo, &token).is_err());
    let retained = recovery.retain_output().to_owned();
    let recovery_directory = recovery.temporary_directory().to_owned();
    drop(recovery);
    assert_eq!(fs::read(&retained).unwrap(), b"valuable resolved result\n");
    assert!(!Snapshot::read(&repo, &token).unwrap().files.is_empty());
    fs::remove_dir_all(recovery_directory).unwrap();

    assert!(prepared.apply(&repo, &token).is_err());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"external edit\n");
    fs::write(f.0.join("a"), original).unwrap();
    prepared.apply(&repo, &token).unwrap();
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"task\n");
    // Applying output is not staging; the caller explicitly marks it resolved.
    assert!(!Snapshot::read(&repo, &token).unwrap().files.is_empty());
    conflicts::resolve(
        &repo,
        prepared.snapshot(),
        prepared.file(),
        conflicts::Resolution::MarkResolved,
        &token,
        &Default::default(),
    )
    .unwrap();
    assert!(Snapshot::read(&repo, &token).unwrap().files.is_empty());
    assert!(prepared.apply(&repo, &token).is_err());
    drop(prepared);
    assert!(!directory.exists());
}

#[test]
fn external_merge_streams_large_binary_versions_and_result() {
    use plico::external_merge::Prepared;
    let f = Fixture::new();
    let size = 9 * 1024 * 1024 + 17;
    let mut bytes = vec![0u8; size];
    fs::write(f.0.join("large.bin"), &bytes).unwrap();
    f.git(&["add", "large.bin"], true);
    f.git(&["commit", "-m", "base"], true);
    f.git(&["checkout", "-b", "task"], true);
    bytes[size - 1] = 1;
    fs::write(f.0.join("large.bin"), &bytes).unwrap();
    f.git(&["commit", "-am", "task"], true);
    f.git(&["checkout", "main"], true);
    bytes[size - 1] = 2;
    fs::write(f.0.join("large.bin"), &bytes).unwrap();
    f.git(&["commit", "-am", "main"], true);
    f.git(&["merge", "task"], false);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let snapshot = Snapshot::read(&repo, &token).unwrap();
    let prepared = Prepared::read(&repo, &snapshot, &snapshot.files[0], &token).unwrap();
    for (name, last) in [("base.bin", 0), ("yours.bin", 2), ("theirs.bin", 1)] {
        let content = fs::read(prepared.temporary_directory().join(name)).unwrap();
        assert_eq!(content.len(), size);
        assert_eq!(content[size - 1], last);
    }
    bytes[size - 1] = 3;
    fs::write(prepared.output(), &bytes).unwrap();
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(prepared.apply(&repo, &cancelled).is_err());
    assert_eq!(fs::read(f.0.join("large.bin")).unwrap()[size - 1], 2);
    prepared.apply(&repo, &token).unwrap();
    assert_eq!(fs::read(f.0.join("large.bin")).unwrap(), bytes);
    assert_eq!(Snapshot::read(&repo, &token).unwrap().files.len(), 1);
    assert!(fs::read_dir(&f.0).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".plico-save-")
    }));
}

#[test]
fn resolves_conflicts_outside_sparse_checkout() {
    use conflicts::Resolution;
    for resolution in [
        Resolution::Accept(Side::Yours),
        Resolution::Accept(Side::Theirs),
        Resolution::MarkResolved,
    ] {
        let f = Fixture::new();
        fs::create_dir(f.0.join("included")).unwrap();
        fs::create_dir(f.0.join("excluded")).unwrap();
        fs::write(f.0.join("included/keep"), "unchanged\n").unwrap();
        fs::write(f.0.join("excluded/conflict"), "base\n").unwrap();
        f.git(&["add", "."], true);
        f.git(&["commit", "-m", "base"], true);
        f.git(&["checkout", "-b", "task"], true);
        f.git(&["rm", "excluded/conflict"], true);
        f.git(&["commit", "-m", "delete"], true);
        f.git(&["checkout", "main"], true);
        fs::write(f.0.join("excluded/conflict"), "main\n").unwrap();
        f.git(&["commit", "-am", "modify"], true);
        f.git(&["sparse-checkout", "set", "included"], true);
        assert!(!f.0.join("excluded/conflict").exists());
        f.git(&["merge", "task"], false);
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let snap = Snapshot::read(&repo, &token).unwrap();
        if matches!(resolution, Resolution::MarkResolved) {
            fs::write(f.0.join("excluded/conflict"), "manual\n").unwrap();
        }
        let result = conflicts::resolve(
            &repo,
            &snap,
            &snap.files[0],
            resolution,
            &token,
            &Default::default(),
        )
        .unwrap();
        for output in result.commands {
            let output = output.unwrap();
            assert!(output.status.success(), "{:?}", output.stderr);
            assert!(output.stopped.is_none());
        }
        assert!(result.snapshot.unwrap().files.is_empty());
        match resolution {
            Resolution::Accept(Side::Theirs) => assert!(!f.0.join("excluded/conflict").exists()),
            _ => assert_eq!(
                fs::read_to_string(f.0.join("excluded/conflict")).unwrap(),
                if matches!(resolution, Resolution::MarkResolved) {
                    "manual\n"
                } else {
                    "main\n"
                }
            ),
        }
        assert_eq!(fs::read(f.0.join("included/keep")).unwrap(), b"unchanged\n");
    }
}

#[test]
fn explicit_stash_apply_keeps_current_worktree_on_yours_side() {
    let f = Fixture::new();
    fs::write(f.0.join("a"), "base\n").unwrap();
    f.git(&["add", "."], true);
    f.git(&["commit", "-m", "base"], true);
    fs::write(f.0.join("a"), "stash\n").unwrap();
    f.git(&["stash", "push", "-m", "draft"], true);
    fs::write(f.0.join("a"), "current\n").unwrap();
    f.git(&["commit", "-am", "current"], true);
    // A stale context from an already completed automatic restore must not
    // affect an explicit apply. It is cleared before the new operation.
    fs::write(
        f.0.join(".git/plico-conflict-saved-changes"),
        b"old context",
    )
    .unwrap();
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let entry = plico::stash::list(&repo, 1, &token)
        .unwrap()
        .entries
        .remove(0);
    let report = plico::stash_operation::execute(
        &repo,
        &entry,
        plico::stash_operation::Action::Apply,
        false,
        &token,
        &Default::default(),
    )
    .unwrap();
    assert_eq!(report.status, plico::stash_operation::Status::Conflicts);
    let snapshot = Snapshot::read(&repo, &token).unwrap();
    assert!(!snapshot.reversed);
    assert_eq!(
        conflicts::content(
            &repo,
            snapshot.files[0].side(Side::Yours, false).unwrap(),
            &token
        )
        .unwrap(),
        b"current\n"
    );
    assert!(!f.0.join(".git/plico-conflict-saved-changes").exists());
}
