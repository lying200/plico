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
fn automatic_merge_preserves_zero_effect_files_index_and_external_edits() {
    let f = Fixture::new();
    let content = |first: &str, last: &str| {
        format!(
            "{first}\n{}\n{last}\n",
            (0..8)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    for path in ["a", "b", "c"] {
        fs::write(f.0.join(path), content("alpha beta gamma", "base end")).unwrap();
    }
    f.git(&["add", "."], true);
    f.git(&["commit", "-qm", "base"], true);
    f.git(&["checkout", "-qb", "topic"], true);
    for path in ["a", "b", "c"] {
        fs::write(
            f.0.join(path),
            content(
                if path == "b" {
                    "theirs"
                } else {
                    "alpha beta GAMMA"
                },
                "theirs end",
            ),
        )
        .unwrap();
    }
    f.git(&["commit", "-qam", "topic"], true);
    f.git(&["checkout", "main"], true);
    for path in ["a", "b", "c"] {
        fs::write(
            f.0.join(path),
            content(
                if path == "b" {
                    "ours"
                } else {
                    "ALPHA beta gamma"
                },
                "ours end",
            ),
        )
        .unwrap();
    }
    f.git(&["commit", "-qam", "main"], true);
    f.git(&["merge", "topic"], false);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let snapshot = Snapshot::read(&repo, &token).unwrap();
    let read = |index| {
        plico::merge_result::Prepared::read(&repo, &snapshot, &snapshot.files[index], &token)
            .unwrap()
    };
    let a = read(0);
    let cancelled = Cancel::default();
    cancelled.cancel();
    let original = fs::read(f.0.join("a")).unwrap();
    assert!(a.resolve_automatically(&repo, None, &cancelled).is_err());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), original);
    let result = a
        .resolve_automatically(&repo, None, &token)
        .unwrap()
        .unwrap();
    assert!(result.changed);
    assert_eq!(result.decisions.progress(), (1, 2));
    assert_eq!(
        fs::read_to_string(f.0.join("a")).unwrap(),
        content("ALPHA beta GAMMA", "base end")
    );
    let again = a
        .resolve_automatically(&repo, Some(&result.decisions), &token)
        .unwrap()
        .unwrap();
    assert!(!again.changed);
    assert_eq!(again.decisions, result.decisions);
    let b = read(1);
    let before = fs::read(f.0.join("b")).unwrap();
    let untouched = b
        .resolve_automatically(&repo, None, &token)
        .unwrap()
        .unwrap();
    assert!(!untouched.changed);
    assert_eq!(untouched.decisions.progress(), (0, 2));
    assert_eq!(fs::read(f.0.join("b")).unwrap(), before);
    let c = read(2);
    fs::write(f.0.join("c"), "external edit\n").unwrap();
    assert!(c.resolve_automatically(&repo, None, &token).is_err());
    assert_eq!(fs::read(f.0.join("c")).unwrap(), b"external edit\n");
    fs::write(f.0.join("a"), "external after auto\n").unwrap();
    assert!(
        a.resolve_automatically(&repo, Some(&result.decisions), &token)
            .is_err()
    );
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"external after auto\n");
    assert_eq!(Snapshot::read(&repo, &token).unwrap().files, snapshot.files);
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
fn merge_output_is_isolated_and_rejects_changed_worktree() {
    use plico::merge_result::Prepared;
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
    prepared.write_text_result("task\n".into()).unwrap();
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
fn merge_result_streams_large_binary_versions_and_result() {
    use plico::merge_result::Prepared;
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

#[test]
fn native_merge_is_isolated_preserves_encoding_and_rejects_stale_worktree() {
    use plico::merge_result::Prepared;
    let f = Fixture::new();
    f.git(&["config", "core.autocrlf", "false"], true);
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
    let original = b"\xef\xbb\xbf<<<<<<< main\r\nmain\r\n=======\r\ntask\r\n>>>>>>> task\r\n";
    fs::write(f.0.join("a"), original).unwrap();
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let snapshot = Snapshot::read(&repo, &token).unwrap();
    let prepared = Prepared::read(&repo, &snapshot, &snapshot.files[0], &token).unwrap();
    assert_eq!(
        prepared.text_inputs().unwrap(),
        ["main\n", "base\n", "task\n"]
    );
    prepared.write_text_result("合并\n完成\n".into()).unwrap();
    assert_eq!(fs::read(f.0.join("a")).unwrap(), original);
    assert_eq!(Snapshot::read(&repo, &token).unwrap().files.len(), 1);
    let temp = prepared.temporary_directory().to_owned();
    drop(prepared);
    assert!(!temp.exists());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), original);
    let prepared = Prepared::read(&repo, &snapshot, &snapshot.files[0], &token).unwrap();
    prepared.write_text_result("合并\n完成\n".into()).unwrap();
    fs::write(f.0.join("a"), "external edit\n").unwrap();
    assert!(prepared.apply(&repo, &token).is_err());
    assert_eq!(
        fs::read_to_string(f.0.join("a")).unwrap(),
        "external edit\n"
    );
    fs::write(f.0.join("a"), original).unwrap();
    prepared.apply(&repo, &token).unwrap();
    assert_eq!(
        fs::read(f.0.join("a")).unwrap(),
        "\u{feff}合并\r\n完成\r\n".as_bytes()
    );
    assert_eq!(Snapshot::read(&repo, &token).unwrap().files.len(), 1);
    conflicts::resolve(
        &repo,
        &snapshot,
        &snapshot.files[0],
        conflicts::Resolution::MarkResolved,
        &token,
        &Default::default(),
    )
    .unwrap();
    assert!(Snapshot::read(&repo, &token).unwrap().files.is_empty());
    assert!(prepared.apply(&repo, &token).is_err());
}

#[test]
fn native_merge_retry_after_staging_failure_keeps_external_edit_guard() {
    use plico::merge_result::Prepared;
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
    let prepared = Prepared::read(&repo, &snap, &snap.files[0], &token).unwrap();

    prepared.write_text_result("resolved\n".into()).unwrap();
    prepared.apply(&repo, &token).unwrap();
    fs::write(f.0.join(".git/index.lock"), "audit").unwrap();
    let report = prepared.stage(&repo, &token, &Default::default());
    let failed = match report {
        Err(_) => true,
        Ok(r) => r
            .commands
            .into_iter()
            .any(|r| r.map_or(true, |c| !c.status.success())),
    };
    assert!(failed);
    fs::remove_file(f.0.join(".git/index.lock")).unwrap();
    prepared.apply(&repo, &token).unwrap();
    prepared
        .write_text_result("revised result\n".into())
        .unwrap();
    prepared.apply(&repo, &token).unwrap();
    assert_eq!(
        fs::read_to_string(f.0.join("a")).unwrap(),
        "revised result\n"
    );
    fs::write(f.0.join("a"), "external edit\n").unwrap();
    assert!(prepared.apply(&repo, &token).is_err());
    assert_eq!(
        fs::read_to_string(f.0.join("a")).unwrap(),
        "external edit\n"
    );
    fs::write(f.0.join("a"), "revised result\n").unwrap();
    prepared.apply(&repo, &token).unwrap();
    let report = prepared.stage(&repo, &token, &Default::default()).unwrap();
    assert!(
        report
            .commands
            .iter()
            .all(|r| r.as_ref().is_ok_and(|c| c.status.success()))
    );
    assert!(Snapshot::read(&repo, &token).unwrap().files.is_empty());
}

#[test]
fn native_merge_classifies_binary_versions_without_changing_files() {
    use plico::merge_result::Prepared;
    for binary_version in 0..4 {
        let f = Fixture::new();
        let content = |index, name: &str| {
            if index == binary_version {
                format!("{name}\0data").into_bytes()
            } else {
                format!("{name}\n").into_bytes()
            }
        };
        fs::write(f.0.join("file"), content(0, "base")).unwrap();
        f.git(&["add", "."], true);
        f.git(&["commit", "-m", "base"], true);
        f.git(&["checkout", "-b", "task"], true);
        fs::write(f.0.join("file"), content(1, "theirs")).unwrap();
        f.git(&["commit", "-am", "theirs"], true);
        f.git(&["checkout", "main"], true);
        fs::write(f.0.join("file"), content(2, "yours")).unwrap();
        f.git(&["commit", "-am", "yours"], true);
        f.git(&["merge", "task"], false);
        let working = content(3, "working");
        fs::write(f.0.join("file"), &working).unwrap();
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let snapshot = Snapshot::read(&repo, &token).unwrap();
        let prepared = Prepared::read(&repo, &snapshot, &snapshot.files[0], &token).unwrap();
        assert!(prepared.native_text_inputs().unwrap().is_none());
        assert!(prepared.text_inputs().unwrap_err().contains("Binary"));
        if binary_version == 0 {
            // Finding a binary base must not mask a later unreadable or
            // unsupported side. Such errors cannot become a valid chooser.
            let theirs = prepared.temporary_directory().join("theirs");
            fs::write(&theirs, [0xff, 0xfe, b'a', 0]).unwrap();
            assert!(
                prepared
                    .native_text_inputs()
                    .unwrap_err()
                    .contains("UnsupportedEncoding")
            );
            fs::write(&theirs, vec![b'x'; plico::project_files::FILE_LIMIT + 1]).unwrap();
            assert!(
                prepared
                    .native_text_inputs()
                    .unwrap_err()
                    .contains("TooLarge")
            );
            fs::remove_file(&theirs).unwrap();
            assert!(prepared.native_text_inputs().is_err());
        }
        drop(prepared); // Cancel does not stage or overwrite the working result.
        assert_eq!(fs::read(f.0.join("file")).unwrap(), working);
        snapshot
            .revalidate(&repo, &snapshot.files[0], &token)
            .unwrap();
    }
}

#[test]
fn operation_observation_detects_merge_quit_without_reference_changes() {
    use plico::operation_state::{Operation, observe};
    let f = Fixture::new();
    fs::write(f.0.join("base"), "base\n").unwrap();
    f.git(&["add", "."], true);
    f.git(&["commit", "-m", "base"], true);
    f.git(&["checkout", "-b", "topic"], true);
    fs::write(f.0.join("added"), "topic\n").unwrap();
    f.git(&["add", "."], true);
    f.git(&["commit", "-m", "topic"], true);
    f.git(&["checkout", "main"], true);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let (initial, state) = observe(&repo, None, &token).unwrap().unwrap();
    assert_eq!(state.operation, Operation::None);
    assert!(observe(&repo, Some(&initial), &token).unwrap().is_none());
    let refs = plico::branch_updates::References::read(&repo, &token).unwrap();
    f.git(&["merge", "--no-ff", "--no-commit", "topic"], true);
    assert_eq!(
        plico::branch_updates::References::read(&repo, &token).unwrap(),
        refs
    );
    let (merging, state) = observe(&repo, Some(&initial), &token).unwrap().unwrap();
    assert_eq!(state.operation, Operation::Merge);
    let index = fs::read(f.0.join(".git/index")).unwrap();
    f.git(&["merge", "--quit"], true);
    assert_eq!(
        plico::branch_updates::References::read(&repo, &token).unwrap(),
        refs
    );
    let (finished, state) = observe(&repo, Some(&merging), &token).unwrap().unwrap();
    assert_eq!(state.operation, Operation::None);
    assert!(observe(&repo, Some(&finished), &token).unwrap().is_none());
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(f.0.join("added")).unwrap(), b"topic\n");
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(observe(&repo, Some(&finished), &cancelled).is_err());
}

#[test]
fn operation_observation_tracks_linked_worktree_markers_and_nested_metadata() {
    use plico::operation_state::{Operation, observe};
    let f = Fixture::new();
    f.git(&["commit", "--allow-empty", "-m", "base"], true);
    f.git(&["worktree", "add", "--detach", "linked"], true);
    let main = Repository::new(&f.0);
    let linked = Repository::new(f.0.join("linked"));
    let token = Cancel::default();
    let (main_key, _) = observe(&main, None, &token).unwrap().unwrap();
    let (mut key, _) = observe(&linked, Some(&main_key), &token).unwrap().unwrap();
    let directory = linked.editor_directory(&token).unwrap();
    for (marker, operation) in [
        ("CHERRY_PICK_HEAD", Operation::CherryPick),
        ("REVERT_HEAD", Operation::Revert),
    ] {
        fs::write(directory.join(marker), "fixture\n").unwrap();
        let (next, state) = observe(&linked, Some(&key), &token).unwrap().unwrap();
        assert_eq!(state.operation, operation);
        fs::remove_file(directory.join(marker)).unwrap();
        (key, _) = observe(&linked, Some(&next), &token).unwrap().unwrap();
    }
    fs::create_dir(directory.join("rebase-merge")).unwrap();
    let (next, state) = observe(&linked, Some(&key), &token).unwrap().unwrap();
    assert_eq!(state.operation, Operation::RebaseMerge);
    fs::write(
        directory.join("rebase-merge/head-name"),
        "refs/heads/topic\n",
    )
    .unwrap();
    let (_, state) = observe(&linked, Some(&next), &token).unwrap().unwrap();
    assert_eq!(state.rebase_branch.as_deref(), Some("topic"));
    fs::remove_dir_all(directory.join("rebase-merge")).unwrap();
    fs::create_dir(directory.join("sequencer")).unwrap();
    let (next, state) = observe(&linked, None, &token).unwrap().unwrap();
    assert_eq!(state.operation, Operation::Sequencer);
    fs::write(directory.join("sequencer/todo"), "pick fixture subject\n").unwrap();
    let (_, state) = observe(&linked, Some(&next), &token).unwrap().unwrap();
    assert_eq!(state.operation, Operation::CherryPick);
    assert!(observe(&main, Some(&main_key), &token).unwrap().is_none());
    fs::remove_dir_all(directory.join("sequencer")).unwrap();
    f.git(&["worktree", "remove", "linked"], true);
}

#[test]
fn identical_conflicts_in_another_repository_or_worktree_reject_the_original_snapshot() {
    let source = Fixture::new();
    fs::write(source.0.join("a"), "base\n").unwrap();
    source.git(&["add", "a"], true);
    source.git(&["commit", "-qm", "base"], true);
    source.git(&["checkout", "-qb", "task"], true);
    fs::write(source.0.join("a"), "task\n").unwrap();
    source.git(&["commit", "-qam", "task"], true);
    source.git(&["checkout", "main"], true);
    fs::write(source.0.join("a"), "main\n").unwrap();
    source.git(&["commit", "-qam", "main"], true);
    source.git(&["merge", "task"], false);
    let token = Cancel::default();
    let source_repo = Repository::new(&source.0);
    let snapshot = Snapshot::read(&source_repo, &token).unwrap();
    let other = Fixture::new();
    other.git(&["fetch", source.0.to_str().unwrap(), "main"], true);
    other.git(&["checkout", "-B", "main", "FETCH_HEAD"], true);
    other.git(&["fetch", source.0.to_str().unwrap(), "task"], true);
    other.git(&["merge", "FETCH_HEAD"], false);
    let linked = source.0.join("linked");
    source.git(
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            "main",
        ],
        true,
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(&linked)
        .args(["merge", "task"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    for path in [&other.0, &linked] {
        let repo = Repository::new(path);
        let same = Snapshot::read(&repo, &token).unwrap();
        assert_eq!(same.head, snapshot.head);
        assert_eq!(same.files, snapshot.files);
        assert_eq!(same.operation, snapshot.operation);
        let before = fs::read(path.join("a")).unwrap();
        assert!(
            conflicts::resolve(
                &repo,
                &snapshot,
                &snapshot.files[0],
                conflicts::Resolution::Accept(Side::Theirs),
                &token,
                &Default::default()
            )
            .is_err()
        );
        assert_eq!(fs::read(path.join("a")).unwrap(), before);
        assert_eq!(Snapshot::read(&repo, &token).unwrap().files, same.files);
        same.revalidate(&repo, &same.files[0], &token).unwrap();
    }
    snapshot
        .revalidate(&source_repo, &snapshot.files[0], &token)
        .unwrap();
}

#[test]
fn old_conflict_snapshot_cannot_resolve_a_different_merge_with_identical_stages() {
    let f = Fixture::new();
    fs::write(f.0.join("a"), "base\n").unwrap();
    f.git(&["add", "a"], true);
    f.git(&["commit", "-qm", "base"], true);
    f.git(&["checkout", "-qb", "first"], true);
    fs::write(f.0.join("a"), "theirs\n").unwrap();
    f.git(&["commit", "-qam", "first"], true);
    f.git(&["checkout", "-qb", "second"], true);
    fs::write(f.0.join("unrelated"), "second merge target\n").unwrap();
    f.git(&["add", "unrelated"], true);
    f.git(&["commit", "-qm", "second"], true);
    f.git(&["checkout", "main"], true);
    fs::write(f.0.join("a"), "ours\n").unwrap();
    f.git(&["commit", "-qam", "ours"], true);
    f.git(&["merge", "first"], false);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let old = Snapshot::read(&repo, &token).unwrap();
    let draft = plico::merge_result::Prepared::read(&repo, &old, &old.files[0], &token).unwrap();
    draft
        .write_text_result("stale merge draft\n".into())
        .unwrap();
    f.git(&["merge", "--abort"], true);
    f.git(&["merge", "second"], false);
    let current = Snapshot::read(&repo, &token).unwrap();
    assert_eq!(old.head, current.head);
    assert_eq!(
        old.files, current.files,
        "file-level revalidation alone cannot distinguish these merges"
    );
    let before = fs::read(f.0.join("a")).unwrap();
    let marker = fs::read(f.0.join(".git/MERGE_HEAD")).unwrap();
    assert!(old.revalidate(&repo, &old.files[0], &token).is_err());
    assert!(draft.apply(&repo, &token).is_err());
    assert!(
        conflicts::resolve(
            &repo,
            &old,
            &old.files[0],
            conflicts::Resolution::Accept(Side::Yours),
            &token,
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(fs::read(f.0.join("a")).unwrap(), before);
    assert_eq!(fs::read(f.0.join(".git/MERGE_HEAD")).unwrap(), marker);
    current
        .revalidate(&repo, &current.files[0], &token)
        .unwrap();
}

#[test]
fn saved_merge_results_stage_only_on_finish_and_reject_intervening_edits() {
    let f = Fixture::new();
    fs::write(f.0.join("a"), "base\n").unwrap();
    f.git(&["add", "a"], true);
    f.git(&["commit", "-qm", "base"], true);
    f.git(&["checkout", "-qb", "topic"], true);
    fs::write(f.0.join("a"), "theirs\n").unwrap();
    f.git(&["commit", "-qam", "theirs"], true);
    f.git(&["checkout", "main"], true);
    fs::write(f.0.join("a"), "ours\n").unwrap();
    f.git(&["commit", "-qam", "ours"], true);
    f.git(&["merge", "topic"], false);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let initial = Snapshot::read(&repo, &token).unwrap();
    let marker = fs::read(f.0.join(".git/MERGE_HEAD")).unwrap();
    let draft =
        plico::merge_result::Prepared::read(&repo, &initial, &initial.files[0], &token).unwrap();
    assert!(draft.saved_text(&repo, &token).is_err());
    assert!(draft.stage(&repo, &token, &Default::default()).is_err());
    draft.write_text_result("reviewed result\n".into()).unwrap();
    draft.apply(&repo, &token).unwrap();
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"reviewed result\n");
    // A subsequent un-applied editor draft must not replace the saved review result.
    draft
        .write_text_result("unsaved next draft\n".into())
        .unwrap();
    assert_eq!(
        draft.saved_text(&repo, &token).unwrap(),
        "reviewed result\n"
    );
    assert_eq!(
        Snapshot::read(&repo, &token).unwrap().files,
        initial.files,
        "saving the chosen text must leave the unmerged index intact"
    );
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(draft.stage(&repo, &cancelled, &Default::default()).is_err());
    fs::write(f.0.join("a"), "external edit\n").unwrap();
    assert!(draft.saved_text(&repo, &token).is_err());
    assert!(draft.stage(&repo, &token, &Default::default()).is_err());
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"external edit\n");
    assert_eq!(Snapshot::read(&repo, &token).unwrap().files, initial.files);
    // Only a return to the exact reviewed bytes permits final staging.
    fs::write(f.0.join("a"), "reviewed result\n").unwrap();
    fs::write(f.0.join("unrelated"), "keep staged\n").unwrap();
    f.git(&["add", "unrelated"], true);
    let report = draft.stage(&repo, &token, &Default::default()).unwrap();
    assert!(report.commands.iter().all(|r| {
        r.as_ref()
            .is_ok_and(|o| o.status.success() && o.stopped.is_none())
    }));
    assert!(report.snapshot.unwrap().files.is_empty());
    assert_eq!(
        fs::read(f.0.join(".git/MERGE_HEAD")).unwrap(),
        marker,
        "finishing conflict resolution must not commit the merge"
    );
    let out = Command::new("git")
        .arg("-C")
        .arg(&f.0)
        .args(["show", ":unrelated"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"keep staged\n");
}
