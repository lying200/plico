use plico::{
    git::{Cancel, Repository},
    submodules::{self, IndexState},
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
// Native Windows runs many real Git processes before the first conflict and
// between replies. Keep the deadlock watchdog bounded without applying Unix
// process-startup deadlines to the integration harness (product waits unchanged).
fn operation_timeout(seconds: u64) -> std::time::Duration {
    std::time::Duration::from_secs(seconds * if cfg!(windows) { 4 } else { 1 })
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let f = Self(std::env::temp_dir().join(format!(
            "plico-submodules-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&f.0).unwrap();
        f.git(&["init", "-b", "main"]);
        for (key, value) in [
            ("user.name", "Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "disabled-hooks"),
        ] {
            f.git(&["config", key, value]);
        }
        fs::write(f.0.join("base"), "base").unwrap();
        f.git(&["add", "base"]);
        f.git(&["commit", "-m", "base"]);
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
    fn declare(&self, name: &str, path: &str) {
        self.git(&[
            "config",
            "-f",
            ".gitmodules",
            &format!("submodule.{name}.path"),
            path,
        ]);
        self.git(&[
            "config",
            "-f",
            ".gitmodules",
            &format!("submodule.{name}.url"),
            "../child.git",
        ]);
    }
    fn list(&self) -> Vec<submodules::Module> {
        submodules::list(&Repository::new(&self.0), &Cancel::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn inventory_preserves_paths_and_index_without_initializing_modules() {
    let f = Fixture::new();
    assert!(f.list().is_empty());
    let path = "modules/with space";
    f.declare("named.child", path);
    f.declare("not-staged", "another");
    let oid = f.git(&["rev-parse", "HEAD"]);
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{oid},{path}"),
    ]);
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    let modules = f.list();
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0].name, "named.child");
    assert_eq!(modules[0].path, PathBuf::from(path));
    assert_eq!(modules[0].index, IndexState::Commit(oid));
    assert_eq!(modules[1].index, IndexState::NotTracked);
    assert!(!f.0.join(path).exists());
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    // Calling from a subdirectory still finds the repository-level declarations.
    fs::create_dir(f.0.join("inside")).unwrap();
    let nested =
        submodules::list(&Repository::new(f.0.join("inside")), &Cancel::default()).unwrap();
    assert_eq!(nested.len(), 2);
    assert_eq!(nested[0].index, modules[0].index);
}
#[test]
fn invalid_declarations_are_not_operation_targets() {
    let f = Fixture::new();
    f.git(&[
        "config",
        "-f",
        ".gitmodules",
        "submodule.partial.path",
        "only-path",
    ]);
    assert!(f.list().is_empty());
    f.declare("outside", "../outside");
    assert!(submodules::list(&Repository::new(&f.0), &Cancel::default()).is_err());
    f.git(&[
        "config",
        "-f",
        ".gitmodules",
        "--remove-section",
        "submodule.outside",
    ]);
    f.declare("a", "same");
    f.declare("b", "same");
    assert!(submodules::list(&Repository::new(&f.0), &Cancel::default()).is_err());
}
#[cfg(unix)]
#[test]
fn inventory_does_not_split_tab_or_newline_in_path() {
    let f = Fixture::new();
    let path = "modules/tab\tand\nnewline";
    f.declare("special", path);
    let oid = f.git(&["rev-parse", "HEAD"]);
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{oid},{path}"),
    ]);
    let modules = f.list();
    assert_eq!(modules[0].path, PathBuf::from(path));
    assert_eq!(modules[0].index, IndexState::Commit(oid));
}

#[test]
fn inventory_reports_unmerged_gitlinks_without_choosing_a_stage() {
    use std::{io::Write, process::Stdio};
    let f = Fixture::new();
    f.declare("child", "modules/child");
    let oid = f.git(&["rev-parse", "HEAD"]);
    let mut child = Command::new("git")
        .arg("-C")
        .arg(&f.0)
        .args(["update-index", "--index-info"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            format!("160000 {oid} 1\tmodules/child\n160000 {oid} 2\tmodules/child\n").as_bytes(),
        )
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(f.list()[0].index, IndexState::Conflict);
}

#[test]
fn probe_distinguishes_missing_occupied_and_broken_children_from_parent() {
    use submodules::Checkout;
    let f = Fixture::new();
    let path = std::path::Path::new("child");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    assert_eq!(
        submodules::inspect(&repo, path, &token).unwrap(),
        Checkout::Uninitialized { occupied: false }
    );
    fs::create_dir(f.0.join(path)).unwrap();
    fs::write(f.0.join(path).join("local"), "keep").unwrap();
    assert_eq!(
        submodules::inspect(&repo, path, &token).unwrap(),
        Checkout::Uninitialized { occupied: true }
    );
    fs::write(f.0.join(path).join(".git"), "gitdir: nonexistent\n").unwrap();
    assert!(submodules::inspect(&repo, path, &token).is_err());
    assert_eq!(fs::read(f.0.join(path).join("local")).unwrap(), b"keep");
    token.cancel();
    assert!(submodules::inspect(&repo, path, &token).is_err());
}

#[test]
fn probe_reads_real_submodule_branch_detached_and_local_changes() {
    use submodules::Checkout;
    let parent = Fixture::new();
    let source = Fixture::new();
    parent.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        source.0.to_str().unwrap(),
        "modules/child",
    ]);
    let path = std::path::Path::new("modules/child");
    let child = parent.0.join(path);
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let oid = source.git(&["rev-parse", "HEAD"]);
    assert_eq!(
        submodules::inspect(&repo, path, &token).unwrap(),
        Checkout::Initialized {
            head: oid.clone(),
            branch: Some("refs/heads/main".into()),
            tracked_changes: false,
            untracked: false,
            conflicts: false
        }
    );
    let status = Command::new("git")
        .arg("-C")
        .arg(&child)
        .args(["checkout", "--detach"])
        .output()
        .unwrap();
    assert!(status.status.success());
    fs::write(child.join("base"), "dirty").unwrap();
    fs::write(child.join("untracked"), "keep").unwrap();
    let index_path = parent.0.join(".git/modules/modules/child/index");
    let index = fs::read(&index_path).unwrap();
    assert_eq!(
        submodules::inspect(&repo, path, &token).unwrap(),
        Checkout::Initialized {
            head: oid,
            branch: None,
            tracked_changes: true,
            untracked: true,
            conflicts: false
        }
    );
    assert_eq!(fs::read(index_path).unwrap(), index);
    assert_eq!(fs::read(child.join("base")).unwrap(), b"dirty");
}

#[cfg(unix)]
#[test]
fn probe_rejects_linked_directory_ancestors() {
    let f = Fixture::new();
    let other = Fixture::new();
    std::os::unix::fs::symlink(&other.0, f.0.join("linked")).unwrap();
    assert!(
        submodules::inspect(
            &Repository::new(&f.0),
            std::path::Path::new("linked"),
            &Cancel::default()
        )
        .is_err()
    );
}

#[test]
fn probe_reports_unmerged_files_inside_child() {
    use std::{io::Write, process::Stdio};
    let f = Fixture::new();
    let source = Fixture::new();
    f.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        source.0.to_str().unwrap(),
        "child",
    ]);
    let blob = source.git(&["rev-parse", "HEAD:base"]);
    let mut process = Command::new("git")
        .arg("-C")
        .arg(f.0.join("child"))
        .args(["update-index", "--index-info"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(
            format!(
                "0 {}\tbase\n100644 {blob} 1\tbase\n100644 {blob} 2\tbase\n",
                "0".repeat(40)
            )
            .as_bytes(),
        )
        .unwrap();
    assert!(process.wait().unwrap().success());
    assert!(matches!(
        submodules::inspect(
            &Repository::new(&f.0),
            std::path::Path::new("child"),
            &Cancel::default()
        )
        .unwrap(),
        submodules::Checkout::Initialized {
            conflicts: true,
            tracked_changes: true,
            ..
        }
    ));
}

fn child_git(parent: &Fixture, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(parent.0.join("child"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn update_fixture() -> (Fixture, Fixture, String, String) {
    let parent = Fixture::new();
    let source = Fixture::new();
    fs::write(source.0.join("stable"), "stable\n").unwrap();
    source.git(&["add", "stable"]);
    source.git(&["commit", "-m", "stable file"]);
    let before = source.git(&["rev-parse", "HEAD"]);
    fs::write(source.0.join("base"), "second\n").unwrap();
    source.git(&["commit", "-am", "second"]);
    let after = source.git(&["rev-parse", "HEAD"]);
    parent.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        source.0.to_str().unwrap(),
        "child",
    ]);
    child_git(&parent, &["checkout", "--detach", &before]);
    (parent, source, before, after)
}
#[test]
fn clean_update_follows_gitlink_without_modifying_parent_index_or_head() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let (parent, _source, _before, after) = update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    let head = parent.git(&["rev-parse", "HEAD"]);
    let request = Update::prepare(&repo, std::path::Path::new("child"), &token).unwrap();
    let result = request
        .execute_clean(&token, &Progress::default(), None)
        .unwrap();
    assert!(result.outcome.status.success(), "{:?}", result.outcome);
    assert!(
        matches!(result.checkout.unwrap(), submodules::Checkout::Initialized { head, branch: None, .. } if head == after)
    );
    assert_eq!(fs::read(parent.0.join("child/base")).unwrap(), b"second\n");
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);
    assert_eq!(parent.git(&["rev-parse", "HEAD"]), head);
    // A stale request cannot replay after the first execution changed HEAD.
    assert!(
        request
            .execute_clean(&token, &Progress::default(), None)
            .is_err()
    );
}
#[test]
fn clean_update_rechecks_state_and_rejects_dirty_attached_or_cancelled_requests() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let (parent, _source, before, _after) = update_fixture();
    let repo = Repository::new(&parent.0);
    let path = std::path::Path::new("child");
    let token = Cancel::default();
    let request = Update::prepare(&repo, path, &token).unwrap();
    fs::write(parent.0.join("child/base"), "keep local\n").unwrap();
    assert!(
        request
            .execute_clean(&token, &Progress::default(), None)
            .is_err()
    );
    // Even a freshly prepared dirty snapshot cannot bypass preservation.
    let dirty = Update::prepare(&repo, path, &token).unwrap();
    assert!(
        dirty
            .execute_clean(&token, &Progress::default(), None)
            .is_err()
    );
    assert_eq!(
        fs::read(parent.0.join("child/base")).unwrap(),
        b"keep local\n"
    );
    child_git(&parent, &["restore", "base"]);
    child_git(&parent, &["checkout", "-b", "local-topic"]);
    let attached = Update::prepare(&repo, path, &token).unwrap();
    assert!(
        attached
            .execute_clean(&token, &Progress::default(), None)
            .is_err()
    );
    assert_eq!(
        child_git(&parent, &["symbolic-ref", "HEAD"]),
        "refs/heads/local-topic"
    );
    child_git(&parent, &["checkout", "--detach"]);
    let cancelled = Update::prepare(&repo, path, &token).unwrap();
    token.cancel();
    assert!(
        cancelled
            .execute_clean(&token, &Progress::default(), None)
            .is_err()
    );
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), before);
}
#[test]
fn failed_submodule_update_keeps_command_result_and_checkout_for_recovery() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let (parent, _source, before, _) = update_fixture();
    let missing = "f".repeat(40);
    parent.git(&[
        "update-index",
        "--cacheinfo",
        &format!("160000,{missing},child"),
    ]);
    child_git(
        &parent,
        &[
            "remote",
            "set-url",
            "origin",
            parent.0.join("missing-remote.git").to_str().unwrap(),
        ],
    );
    let token = Cancel::default();
    let update = Update::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    let result = update
        .execute_clean(&token, &Progress::default(), None)
        .unwrap();
    assert!(!result.outcome.status.success());
    assert!(!result.outcome.stderr.bytes().is_empty());
    assert!(
        matches!(result.checkout.unwrap(), submodules::Checkout::Initialized { head, .. } if head == before)
    );
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), before);
}

#[test]
fn recursive_update_distinguishes_gitlink_drift_from_nested_file_edits() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let parent = Fixture::new();
    let source = Fixture::new();
    let nested = Fixture::new();
    let before = nested.git(&["rev-parse", "HEAD"]);
    fs::write(nested.0.join("base"), "nested second\n").unwrap();
    nested.git(&["commit", "-am", "nested second"]);
    let after = nested.git(&["rev-parse", "HEAD"]);
    source.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        nested.0.to_str().unwrap(),
        "nested",
    ]);
    source.git(&["commit", "-am", "nested"]);
    parent.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        source.0.to_str().unwrap(),
        "child",
    ]);
    child_git(&parent, &["checkout", "--detach"]);
    child_git(
        &parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
        ],
    );
    child_git(&parent, &["-C", "nested", "checkout", "--detach", &before]);
    let token = Cancel::default();
    let repo = Repository::new(&parent.0);
    let path = std::path::Path::new("child");
    fs::write(parent.0.join("child/nested/base"), "keep nested edit\n").unwrap();
    let dirty = Update::prepare(&repo, path, &token).unwrap();
    assert!(
        dirty
            .execute_clean(&token, &Progress::default(), None)
            .is_err()
    );
    assert_eq!(
        fs::read(parent.0.join("child/nested/base")).unwrap(),
        b"keep nested edit\n"
    );
    child_git(&parent, &["-C", "nested", "restore", "base"]);
    let clean = Update::prepare(&repo, path, &token).unwrap();
    let result = clean
        .execute_clean(&token, &Progress::default(), None)
        .unwrap();
    assert!(result.outcome.status.success(), "{:?}", result.outcome);
    assert_eq!(
        child_git(&parent, &["-C", "nested", "rev-parse", "HEAD"]),
        after
    );
    assert_eq!(
        fs::read(parent.0.join("child/nested/base")).unwrap(),
        b"nested second\n"
    );

    // Saving a nested draft must not turn its parent's gitlink dirt into a
    // second stash, but independent parent edits still need their own stash.
    fs::write(parent.0.join("child/local-parent"), "parent draft\n").unwrap();
    child_git(&parent, &["add", "local-parent"]);
    fs::write(parent.0.join("child/nested/local-child"), "nested draft\n").unwrap();
    child_git(&parent, &["-C", "nested", "add", "local-child"]);
    let update = Update::prepare(&repo, path, &token).unwrap();
    let report = update
        .execute_preserving(&token, &Progress::default(), None)
        .unwrap();
    assert!(report.succeeded(), "{:?}", report.error);
    assert_eq!(report.saved.len(), 2);
    assert_eq!(
        child_git(&parent, &["show", ":local-parent"]),
        "parent draft"
    );
    assert_eq!(
        child_git(&parent, &["-C", "nested", "show", ":local-child"]),
        "nested draft"
    );
    assert!(child_git(&parent, &["stash", "list"]).is_empty());
    assert!(child_git(&parent, &["-C", "nested", "stash", "list"]).is_empty());

    // If a later root cannot save, the earlier nested stash must remain
    // recoverable and no updater may start or pop that stash automatically.
    let update = Update::prepare(&repo, path, &token).unwrap();
    let child_head = child_git(&parent, &["rev-parse", "HEAD"]);
    let lock = parent.0.join(".git/modules/child/index.lock");
    fs::write(&lock, "fixture lock").unwrap();
    let failed = update
        .execute_preserving(&token, &Progress::default(), None)
        .unwrap();
    assert!(failed.update.is_none());
    assert_eq!(failed.saved.len(), 2);
    assert!(failed.saved[0].save.as_ref().unwrap().status.success());
    let nested_stash = failed.saved[0].saved_oid.as_ref().unwrap();
    assert_eq!(
        child_git(&parent, &["-C", "nested", "rev-parse", "refs/stash"]),
        *nested_stash
    );
    assert!(failed.saved.iter().all(|root| root.restore.is_none()));
    assert!(failed.saved[1].saved_oid.is_none());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), child_head);
    assert_eq!(
        fs::read(parent.0.join("child/local-parent")).unwrap(),
        b"parent draft\n"
    );
    fs::remove_file(lock).unwrap();
}

#[test]
fn preserving_update_restores_index_and_worktree_edits_and_leaves_untracked_files() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let (parent, _source, _, after) = update_fixture();
    let child = parent.0.join("child");
    fs::write(child.join("stable"), "staged edit\n").unwrap();
    child_git(&parent, &["add", "stable"]);
    fs::write(child.join("stable"), "working edit\n").unwrap();
    fs::write(child.join("untracked"), "keep untracked\n").unwrap();
    let parent_index = fs::read(parent.0.join(".git/index")).unwrap();
    let token = Cancel::default();
    let update = Update::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    let report = update
        .execute_preserving(&token, &Progress::default(), None)
        .unwrap();
    assert!(report.succeeded(), "error: {:?}", report.error);
    assert_eq!(report.saved.len(), 1);
    assert!(report.saved[0].saved_oid.is_none());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), after);
    assert_eq!(child_git(&parent, &["show", ":stable"]), "staged edit");
    assert_eq!(fs::read(child.join("stable")).unwrap(), b"working edit\n");
    assert_eq!(
        fs::read(child.join("untracked")).unwrap(),
        b"keep untracked\n"
    );
    assert!(child_git(&parent, &["stash", "list"]).is_empty());
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), parent_index);
}
#[test]
fn preserving_update_failure_retains_recoverable_stash_without_auto_pop() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let (parent, _source, before, _) = update_fixture();
    let missing = "f".repeat(40);
    parent.git(&[
        "update-index",
        "--cacheinfo",
        &format!("160000,{missing},child"),
    ]);
    child_git(
        &parent,
        &[
            "remote",
            "set-url",
            "origin",
            parent.0.join("missing.git").to_str().unwrap(),
        ],
    );
    fs::write(parent.0.join("child/stable"), "recover me\n").unwrap();
    let token = Cancel::default();
    let update = Update::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    let report = update
        .execute_preserving(&token, &Progress::default(), None)
        .unwrap();
    assert!(!report.succeeded());
    assert!(!report.update.unwrap().unwrap().status.success());
    let saved = report.saved[0].saved_oid.as_ref().unwrap();
    assert_eq!(child_git(&parent, &["rev-parse", "refs/stash"]), *saved);
    assert_eq!(
        child_git(&parent, &["show", &format!("{saved}:stable")]),
        "recover me"
    );
    assert!(report.saved[0].restore.is_none());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"stable\n"
    );
}
#[test]
fn restoring_submodule_edits_reports_conflicts_and_keeps_saved_identity() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let (parent, _source, _, after) = update_fixture();
    fs::write(parent.0.join("child/base"), "conflicting local edit\n").unwrap();
    let token = Cancel::default();
    let update = Update::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    let report = update
        .execute_preserving(&token, &Progress::default(), None)
        .unwrap();
    assert!(!report.succeeded());
    assert!(report.update.unwrap().unwrap().status.success());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), after);
    let saved = report.saved[0].saved_oid.as_ref().unwrap();
    assert_eq!(child_git(&parent, &["rev-parse", "refs/stash"]), *saved);
    assert!(
        report.saved[0]
            .restore
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .as_ref()
            .is_some_and(|outcome| !outcome.status.success())
    );
    assert!(matches!(
        report.checkout.unwrap(),
        submodules::Checkout::Initialized {
            conflicts: true,
            ..
        }
    ));
}

#[test]
fn untracked_collision_is_not_stashed_or_overwritten_by_preserving_update() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let (parent, source, before, _) = update_fixture();
    fs::write(source.0.join("collision"), "remote content\n").unwrap();
    source.git(&["add", "collision"]);
    source.git(&["commit", "-m", "remote collision"]);
    let target = source.git(&["rev-parse", "HEAD"]);
    child_git(
        &parent,
        &["-c", "protocol.file.allow=always", "fetch", "origin"],
    );
    parent.git(&[
        "update-index",
        "--cacheinfo",
        &format!("160000,{target},child"),
    ]);
    fs::write(parent.0.join("child/collision"), "local untracked\n").unwrap();
    fs::write(parent.0.join("child/stable"), "saved edit\n").unwrap();
    let token = Cancel::default();
    let request = Update::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    let report = request
        .execute_preserving(&token, &Progress::default(), None)
        .unwrap();
    assert!(!report.succeeded());
    assert!(!report.update.unwrap().unwrap().status.success());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        fs::read(parent.0.join("child/collision")).unwrap(),
        b"local untracked\n"
    );
    assert!(report.saved[0].saved_oid.is_some());
    assert!(report.saved[0].restore.is_none());
}

#[test]
fn failed_submodule_save_never_starts_the_update() {
    use plico::{operation_process::Progress, submodule_update::Update};
    let (parent, _source, before, _) = update_fixture();
    fs::write(parent.0.join("child/stable"), "locked draft\n").unwrap();
    let token = Cancel::default();
    let request = Update::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    let lock = parent.0.join(".git/modules/child/index.lock");
    fs::write(&lock, "fixture lock").unwrap();
    let report = request
        .execute_preserving(&token, &Progress::default(), None)
        .unwrap();
    assert!(!report.succeeded());
    assert!(report.update.is_none());
    assert!(report.error.is_some());
    assert_eq!(report.saved.len(), 1);
    assert!(report.saved[0].saved_oid.is_none());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"locked draft\n"
    );
    fs::remove_file(lock).unwrap();
}

#[test]
fn inventory_handles_more_paths_than_one_windows_command_line() {
    use std::io::Write;
    let parent = Fixture::new();
    let oid = parent.git(&["rev-parse", "HEAD"]);
    let mut config = String::new();
    let paths: Vec<_> = (0..400)
        .map(|i| format!("modules/{i:04}-{}", "long component ".repeat(6).trim_end()))
        .collect();
    assert!(paths.iter().map(|p| p.len() + 3).sum::<usize>() > 32_767);
    for (i, path) in paths.iter().enumerate() {
        config.push_str(&format!(
            "[submodule \"module-{i:04}\"]\npath = \"{path}\"\nurl = ../child.git\n"
        ));
    }
    fs::write(parent.0.join(".gitmodules"), config).unwrap();
    let mut command = Command::new("git")
        .arg("-C")
        .arg(&parent.0)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = command.stdin.take().unwrap();
    for path in &paths {
        writeln!(input, "160000 {oid}\t{path}").unwrap();
    }
    drop(input);
    assert!(command.wait().unwrap().success());
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    let modules = parent.list();
    assert_eq!(modules.len(), paths.len());
    for (module, path) in modules.iter().zip(paths) {
        assert_eq!(module.path, PathBuf::from(path));
        assert_eq!(module.index, IndexState::Commit(oid.clone()));
    }
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);
}

#[test]
fn attached_submodule_update_uses_own_upstream_and_preserves_parent_index() {
    use plico::{published_commits::Protection, submodule_update::routing, update_method::Method};
    let (parent, _source, before, after) = update_fixture();
    child_git(&parent, &["checkout", "-b", "topic", &before]);
    child_git(
        &parent,
        &["branch", "--set-upstream-to=origin/main", "topic"],
    );
    // Parent explicitly keeps the old gitlink: updating this branch must follow
    // its upstream rather than moving back to the parent's selected commit.
    parent.git(&[
        "update-index",
        "--cacheinfo",
        &format!("160000,{before},child"),
    ]);
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    let token = Cancel::default();
    let request = routing::Request::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    assert!(matches!(request, routing::Request::Branch { .. }));
    let report = request
        .execute(
            Method::Merge,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| panic!("Merge must not ask rebase questions"),
            |_| false,
        )
        .unwrap();
    assert!(report.complete());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), after);
    assert_eq!(
        child_git(&parent, &["symbolic-ref", "HEAD"]),
        "refs/heads/topic"
    );
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);
}

#[test]
fn attached_submodule_without_upstream_never_falls_back_to_gitlink() {
    let (parent, _source, before, _) = update_fixture();
    child_git(
        &parent,
        &["checkout", "--no-track", "-b", "untracked-branch", &before],
    );
    assert!(
        plico::submodule_update::routing::Request::prepare(
            &Repository::new(&parent.0),
            std::path::Path::new("child"),
            &Cancel::default()
        )
        .is_err()
    );
    assert_eq!(
        child_git(&parent, &["symbolic-ref", "HEAD"]),
        "refs/heads/untracked-branch"
    );
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), before);
}

#[test]
fn attached_submodule_rebase_keeps_branch_and_replays_local_commit() {
    use plico::{published_commits::Protection, submodule_update::routing, update_method::Method};
    let (parent, _source, before, after) = update_fixture();
    child_git(&parent, &["checkout", "-b", "topic", &before]);
    child_git(
        &parent,
        &["branch", "--set-upstream-to=origin/main", "topic"],
    );
    child_git(&parent, &["config", "user.name", "Test"]);
    child_git(&parent, &["config", "user.email", "test@example.invalid"]);
    child_git(&parent, &["config", "commit.gpgsign", "false"]);
    child_git(&parent, &["config", "core.hooksPath", "disabled-hooks"]);
    fs::write(parent.0.join("child/local-file"), "local commit\n").unwrap();
    child_git(&parent, &["add", "local-file"]);
    child_git(&parent, &["commit", "-m", "local topic"]);
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    let token = Cancel::default();
    let request = routing::Request::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    let report = request
        .execute(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
        )
        .unwrap();
    assert!(report.complete());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD^"]), after);
    assert_eq!(
        child_git(&parent, &["symbolic-ref", "HEAD"]),
        "refs/heads/topic"
    );
    assert_eq!(
        fs::read(parent.0.join("child/local-file")).unwrap(),
        b"local commit\n"
    );
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);
}

#[test]
fn recursive_update_skips_uninitialized_nested_modules_and_keeps_placeholder_files() {
    use plico::submodule_update::Update;
    let (parent, source, _, _) = update_fixture();
    let nested = Fixture::new();
    source.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        nested.0.to_str().unwrap(),
        "nested",
    ]);
    source.git(&["commit", "-am", "declare nested"]);
    child_git(&parent, &["fetch", "origin"]);
    child_git(&parent, &["checkout", "--detach", "origin/main"]);
    let before = child_git(&parent, &["rev-parse", "HEAD"]);
    fs::write(parent.0.join("child/nested/local"), "keep placeholder\n").unwrap();
    fs::write(source.0.join("stable"), "next stable\n").unwrap();
    source.git(&["commit", "-am", "advance initialized module"]);
    let after = source.git(&["rev-parse", "HEAD"]);
    child_git(&parent, &["fetch", "origin"]);
    parent.git(&[
        "update-index",
        "--cacheinfo",
        &format!("160000,{after},child"),
    ]);
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    let token = Cancel::default();
    let update = Update::prepare(
        &Repository::new(&parent.0),
        std::path::Path::new("child"),
        &token,
    )
    .unwrap();
    let report = update
        .execute_preserving(&token, &Default::default(), None)
        .unwrap();
    assert!(report.succeeded());
    assert_ne!(before, after);
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), after);
    assert!(!parent.0.join("child/nested/.git").exists());
    assert_eq!(
        fs::read(parent.0.join("child/nested/local")).unwrap(),
        b"keep placeholder\n"
    );
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);
}

#[test]
fn project_update_discovers_branch_and_detached_routes_without_initializing() {
    use plico::project_update::roots::{Roots, SkipReason};
    let (parent, _source, before, _) = update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let roots = Roots::discover(&repo, &token).unwrap();
    assert!(roots.branches.is_empty());
    assert_eq!(roots.detached.len(), 1);
    assert_eq!(roots.detached[0].path, PathBuf::from("child"));
    child_git(&parent, &["checkout", "--no-track", "-b", "topic", &before]);
    let roots = Roots::discover(&repo, &token).unwrap();
    assert!(roots.detached.is_empty());
    assert!(roots.branches.is_empty());
    assert_eq!(roots.skipped[0].reason, SkipReason::NoUpstream);
    child_git(
        &parent,
        &["branch", "--set-upstream-to=origin/main", "topic"],
    );
    let roots = Roots::discover(&repo, &token).unwrap();
    assert!(roots.detached.is_empty());
    assert_eq!(roots.branches.len(), 1);
    assert_eq!(roots.branches[0].request.branch, "topic");
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), before);
    parent.declare("missing", "missing");
    parent.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{before},missing"),
    ]);
    let roots = Roots::discover(&repo, &token).unwrap();
    assert_eq!(roots.skipped.len(), 1);
    assert_eq!(roots.skipped[0].reason, SkipReason::Uninitialized);
    let inventory = plico::project_repositories::Repositories::discover(&repo, &token).unwrap();
    assert_eq!(
        inventory.roots.len(),
        2,
        "uninitialized roots must not become readable members"
    );
    let missing = inventory
        .submodules
        .iter()
        .find(|module| module.path == std::path::Path::new("missing"))
        .unwrap();
    assert!(!missing.initialized);
    assert_eq!(missing.index, IndexState::Commit(before.clone()));
    assert!(!parent.0.join("missing").exists());
}

#[test]
fn discovery_schedules_nested_detached_repository_only_through_its_parent() {
    use plico::project_update::roots::Roots;
    let (parent, source, _, _) = update_fixture();
    let nested = Fixture::new();
    source.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        nested.0.to_str().unwrap(),
        "nested",
    ]);
    source.git(&["commit", "-am", "nested"]);
    child_git(&parent, &["fetch", "origin"]);
    child_git(&parent, &["checkout", "--detach", "origin/main"]);
    child_git(
        &parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
            "--recursive",
        ],
    );
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let roots = Roots::discover(&repo, &token).unwrap();
    assert_eq!(roots.detached.len(), 1);
    assert_eq!(roots.detached[0].path, PathBuf::from("child"));
    assert!(roots.branches.is_empty());
    child_git(
        &parent,
        &["checkout", "-b", "topic", "--track", "origin/main"],
    );
    let roots = Roots::discover(&repo, &token).unwrap();
    assert_eq!(roots.branches.len(), 1);
    assert_eq!(roots.detached.len(), 1);
    assert_eq!(
        plico::recent_projects::identity(&roots.detached[0].parent.path),
        plico::recent_projects::identity(&parent.0.join("child")),
    );
    assert_eq!(roots.detached[0].path, PathBuf::from("nested"));
}

fn group_update_fixture(target_missing: bool) -> (Fixture, Fixture, Fixture, String) {
    let (parent, source, before, after) = update_fixture();
    parent.git(&[
        "update-index",
        "--cacheinfo",
        &format!("160000,{before},child"),
    ]);
    parent.git(&["commit", "-am", "record child"]);
    let remote = Fixture::new();
    let bare = remote.0.join("remote.git");
    parent.git(&["init", "--bare", bare.to_str().unwrap()]);
    parent.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    parent.git(&["push", "-u", "origin", "main"]);
    let writer = Fixture(remote.0.join("writer"));
    parent.git(&[
        "clone",
        "-b",
        "main",
        bare.to_str().unwrap(),
        writer.0.to_str().unwrap(),
    ]);
    writer.git(&["config", "user.name", "Test"]);
    writer.git(&["config", "user.email", "test@example.invalid"]);
    writer.git(&["config", "commit.gpgsign", "false"]);
    writer.git(&["config", "core.hooksPath", "disabled-hooks"]);
    let target = if target_missing {
        "1111111111111111111111111111111111111111"
    } else {
        &after
    };
    writer.git(&[
        "update-index",
        "--cacheinfo",
        &format!("160000,{target},child"),
    ]);
    if target_missing {
        fs::write(writer.0.join("base"), "remote parent\n").unwrap();
    }
    writer.git(&["commit", "-am", "advance child pointer"]);
    writer.git(&["push", "origin", "main"]);
    (parent, source, remote, after)
}

#[test]
fn group_merge_advances_parent_then_child_and_restores_child_edits() {
    use plico::project_update::{self, group::Group};
    let (parent, _source, _remote, after) = group_update_fixture(false);
    fs::write(parent.0.join("child/stable"), "child draft\n").unwrap();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let request = project_update::prepare(&repo, &token).unwrap();
    let group = Group::prepare(&repo, request, &token).unwrap();
    let report = group
        .execute_merge(
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
        )
        .unwrap();
    assert!(report.complete(), "{:?}", report.error);
    assert_eq!(report.roots.len(), 2);
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), after);
    assert_eq!(
        parent.git(&["rev-parse", "HEAD"]),
        parent.git(&["rev-parse", "origin/main"])
    );
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"child draft\n"
    );
    assert!(child_git(&parent, &["stash", "list"]).is_empty());
}

#[test]
fn group_update_preserves_documents_saved_after_confirmation_snapshot() {
    use plico::project_update::Prepared;
    let (parent, _source, _remote, after) = group_update_fixture(false);
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, &token).unwrap();
    // The options dialog has already captured clean worktrees. Its OK handler
    // saves an editor document before invoking the group, as VcsUpdateTask does.
    fs::write(parent.0.join("child/stable"), "saved editor draft\n").unwrap();
    let report = prepared
        .group
        .execute_merge(
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
        )
        .unwrap();
    assert!(report.complete(), "{:?}", report.diagnostic());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), after);
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"saved editor draft\n"
    );
    assert_eq!(report.saved.first().unwrap().path, parent.0.join("child"));
    assert!(report.saved.iter().all(|root| root.saved_oid.is_none()));
    assert!(parent.git(&["stash", "list"]).is_empty());
    assert!(child_git(&parent, &["stash", "list"]).is_empty());
}

#[test]
fn group_update_does_not_recapture_changed_child_identity_after_confirmation() {
    use plico::project_update::Prepared;
    for change in ["head", "branch", "gitlink"] {
        let (parent, _source, _remote, after) = group_update_fixture(false);
        let token = Cancel::default();
        let prepared = Prepared::read(&Repository::new(&parent.0), &token).unwrap();
        let parent_head = parent.git(&["rev-parse", "HEAD"]);
        match change {
            "head" => {
                child_git(&parent, &["checkout", "--detach", &after]);
            }
            "branch" => {
                child_git(&parent, &["switch", "-c", "changed-after-dialog"]);
            }
            "gitlink" => {
                parent.git(&[
                    "update-index",
                    "--cacheinfo",
                    &format!("160000,{after},child"),
                ]);
            }
            _ => unreachable!(),
        }
        fs::write(parent.0.join("child/stable"), "saved editor draft\n").unwrap();
        let child_head = child_git(&parent, &["rev-parse", "HEAD"]);
        let index = fs::read(parent.0.join(".git/index")).unwrap();
        let result = prepared.group.execute_merge(
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
        );
        assert!(
            result.is_err(),
            "changed {change} must fail before local writes"
        );
        assert_eq!(parent.git(&["rev-parse", "HEAD"]), parent_head);
        assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), child_head);
        assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);
        assert_eq!(
            fs::read(parent.0.join("child/stable")).unwrap(),
            b"saved editor draft\n"
        );
        assert!(parent.git(&["stash", "list"]).is_empty());
        assert!(child_git(&parent, &["stash", "list"]).is_empty());
    }
}

#[test]
fn failed_child_update_retains_saved_parent_and_child_changes() {
    use plico::project_update::{self, group::Group};
    let (parent, _source, _remote, _) = group_update_fixture(true);
    fs::write(parent.0.join("base"), "parent draft\n").unwrap();
    fs::write(parent.0.join("child/stable"), "child draft\n").unwrap();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let request = project_update::prepare(&repo, &token).unwrap();
    let group = Group::prepare(&repo, request, &token).unwrap();
    let report = group
        .execute_merge(
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
        )
        .unwrap();
    assert!(!report.complete());
    assert!(report.has_saved_changes());
    assert_eq!(report.result(), plico::update_session::UpdateResult::Error);
    assert!(report.diagnostic().unwrap().contains("child"));
    assert_eq!(report.roots.len(), 2);
    assert_eq!(
        report.roots[0].result,
        plico::update_session::UpdateResult::Success
    );
    assert_eq!(report.saved.len(), 2);
    assert!(
        report
            .saved
            .iter()
            .all(|saved| saved.saved_oid.is_some() && saved.restore.is_none())
    );
    assert_eq!(parent.git(&["show", "refs/stash:base"]), "parent draft");
    assert_eq!(
        child_git(&parent, &["show", "refs/stash:stable"]),
        "child draft"
    );
    assert_eq!(fs::read(parent.0.join("base")).unwrap(), b"remote parent\n");
    // Reopening recovery discovers the actual repositories; no in-memory
    // update report is needed to locate the saved work.
    let recovered = plico::project_repositories::Repositories::discover(&repo, &token).unwrap();
    assert!(recovered.errors.is_empty());
    assert_eq!(recovered.roots.len(), 2);
    let stack = plico::stash::project::load(&repo, &token).unwrap();
    assert!(stack.grouped && stack.errors.is_empty() && !stack.more);
    assert_eq!(stack.entries.len(), 2);
    assert_ne!(stack.entries[0].identity(), stack.entries[1].identity());
    for entry in &stack.entries {
        let saved = report
            .saved
            .iter()
            .find(|s| {
                plico::recent_projects::identity(&s.path)
                    == plico::recent_projects::identity(&entry.repository.path)
            })
            .unwrap();
        assert_eq!(Some(&entry.oid), saved.saved_oid.as_ref());
        assert!(plico::stash::contents(&entry.repository, entry, &token).is_ok());
    }
    for root in &recovered.roots {
        let stack = plico::stash::list(root, 10, &token).unwrap();
        assert_eq!(stack.entries.len(), 1);
        let saved = report
            .saved
            .iter()
            .find(|s| {
                plico::recent_projects::identity(&s.path)
                    == plico::recent_projects::identity(&root.path)
            })
            .unwrap();
        assert_eq!(Some(&stack.entries[0].oid), saved.saved_oid.as_ref());
    }
    let child = recovered
        .roots
        .iter()
        .find(|r| r.path.ends_with("child"))
        .unwrap();
    let stack = plico::stash::list(child, 10, &token).unwrap();
    let restored = plico::stash_operation::execute(
        child,
        &stack.entries[0],
        plico::stash_operation::Action::Pop,
        true,
        &token,
        &Default::default(),
    )
    .unwrap();
    assert_eq!(restored.status, plico::stash_operation::Status::Success);
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"child draft\n"
    );
    assert!(!parent.git(&["stash", "list"]).is_empty());
    assert!(child_git(&parent, &["stash", "list"]).is_empty());
}

fn attach_local_topic(parent: &Fixture) {
    child_git(parent, &["checkout", "-b", "topic"]);
    child_git(
        parent,
        &["branch", "--set-upstream-to=origin/main", "topic"],
    );
    for (key, value) in [
        ("user.name", "Test"),
        ("user.email", "test@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "disabled-hooks"),
    ] {
        child_git(parent, &["config", key, value]);
    }
    fs::write(parent.0.join("child/local-topic"), "local commit\n").unwrap();
    child_git(parent, &["add", "local-topic"]);
    child_git(parent, &["commit", "-m", "local topic"]);
}
#[test]
fn group_rebase_replays_attached_child_and_restores_draft_after_parent_fast_forward() {
    use plico::{
        project_update::{self, group::Group},
        published_commits::Protection,
        update_method::Method,
    };
    let (parent, _source, _remote, after) = group_update_fixture(false);
    attach_local_topic(&parent);
    fs::write(parent.0.join("base"), "parent non-overlapping draft\n").unwrap();
    fs::write(parent.0.join("child/stable"), "child draft\n").unwrap();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let group = Group::prepare(
        &repo,
        project_update::prepare(&repo, &token).unwrap(),
        &token,
    )
    .unwrap();
    let report = group
        .execute(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| panic!("Linear topic needs no rebase-over-merge decision"),
            |_| false,
        )
        .unwrap();
    assert!(report.complete(), "{:?}", report.error);
    assert!(
        report.roots[0]
            .fast_forward
            .as_ref()
            .unwrap()
            .outcome
            .status
            .success()
    );
    assert!(
        report.roots[1]
            .rebase
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .outcome
            .status
            .success()
    );
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD^"]), after);
    assert_eq!(
        child_git(&parent, &["symbolic-ref", "HEAD"]),
        "refs/heads/topic"
    );
    assert_eq!(
        fs::read(parent.0.join("base")).unwrap(),
        b"parent non-overlapping draft\n"
    );
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"child draft\n"
    );
    assert!(report.saved.iter().all(|saved| saved.saved_oid.is_none()));
}
#[test]
fn group_rebase_declining_published_commits_retains_saved_child_changes() {
    use plico::{
        project_update::{self, group::Group},
        published_commits::Protection,
        pull::Target,
        update_method::Method,
    };
    let (parent, _source, _remote, _) = group_update_fixture(false);
    attach_local_topic(&parent);
    child_git(&parent, &["push", "origin", "topic"]);
    let before = child_git(&parent, &["rev-parse", "HEAD"]);
    fs::write(parent.0.join("child/stable"), "child draft\n").unwrap();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let group = Group::prepare(
        &repo,
        project_update::prepare(&repo, &token).unwrap(),
        &token,
    )
    .unwrap();
    let targets = [Target {
        remote: "origin".into(),
        branch: "topic".into(),
    }];
    let mut asked = false;
    let report = group
        .execute(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Explicit(&targets),
            |_| Ok(Some(Method::Rebase)),
            |problem| {
                assert!(matches!(problem, Ok(Some(_))));
                asked = true;
                false
            },
        )
        .unwrap();
    assert!(asked);
    assert!(!report.complete());
    assert!(report.cancelled());
    assert!(report.has_saved_changes());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), before);
    assert_eq!(report.saved.len(), 1);
    assert!(report.saved[0].restore.is_none());
    assert!(report.saved[0].saved_oid.is_some());
    assert_eq!(
        child_git(&parent, &["show", "refs/stash:stable"]),
        "child draft"
    );
}

#[test]
fn group_update_routes_child_merge_editor_to_the_captured_child_directory() {
    use plico::{auth_request::Requests, editor, project_update};
    let (parent, _source, _remote, _) = group_update_fixture(false);
    attach_local_topic(&parent);
    child_git(&parent, &["config", "branch.topic.mergeOptions", "--edit"]);
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let directories = prepared.editor_directories;
    let group = prepared.group;
    assert_eq!(directories.len(), 2);
    let helper = PathBuf::from(
        std::env::var_os("GITUI_TEST_EDITOR_EXE")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_plico-editor").into()),
    )
    .canonicalize()
    .unwrap();
    let command = editor::command(&helper, false).unwrap();
    let askpass = PathBuf::from(
        std::env::var_os("GITUI_TEST_ASKPASS_EXE")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_plico-askpass").into()),
    );
    let requests = Requests::new(token.clone());
    std::thread::scope(|scope| {
        let requests_ref = &requests;
        let token_ref = &token;
        let worker = scope.spawn(move || {
            group.execute_merge(
                &command,
                token_ref,
                &Default::default(),
                Some((requests_ref, &askpass)),
            )
        });
        let start = std::time::Instant::now();
        let mut edits = 0;
        while !worker.is_finished() {
            if let Some(pending) = requests.take_pending() {
                let mut document = editor::Document::open_in_roots(pending, &directories).unwrap();
                document
                    .save("merge attached child from group update\n")
                    .unwrap();
                edits += 1;
            }
            if start.elapsed() > operation_timeout(15) {
                token.cancel();
                panic!("Group editor request timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let report = worker.join().unwrap().unwrap();
        assert!(report.complete(), "{:?}", report.error);
        assert_eq!(edits, 1);
    });
    assert_eq!(
        child_git(&parent, &["log", "-1", "--format=%s"]),
        "merge attached child from group update"
    );
}

#[test]
fn stash_discovery_keeps_conflicted_roots_and_isolates_broken_children() {
    use plico::project_repositories::Repositories;
    use std::{io::Write, process::Stdio};
    let (parent, _source, before, _) = update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let git_dir = Repository::new(parent.0.join("child"))
        .editor_directory(&token)
        .unwrap();
    fs::write(git_dir.join("MERGE_HEAD"), format!("{before}\n")).unwrap();
    let mut process = Command::new("git")
        .arg("-C")
        .arg(&parent.0)
        .args(["update-index", "--index-info"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        process.stdin.take().unwrap(),
        "0 {}\tchild\n160000 {before} 1\tchild\n160000 {before} 2\tchild",
        "0".repeat(40)
    )
    .unwrap();
    assert!(process.wait().unwrap().success());
    for path in ["broken", "missing"] {
        parent.declare(path, path);
        parent.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{before},{path}"),
        ]);
    }
    fs::create_dir(parent.0.join("broken")).unwrap();
    fs::write(parent.0.join("broken/.git"), "gitdir: absent\n").unwrap();
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    assert!(plico::project_update::roots::Roots::discover(&repo, &token).is_err());
    let roots = Repositories::discover(&repo, &token).unwrap();
    assert_eq!(roots.roots.len(), 2);
    assert!(roots.roots.iter().any(|r| {
        plico::recent_projects::identity(&r.path)
            == plico::recent_projects::identity(&parent.0.join("child"))
    }));
    assert_eq!(roots.errors.len(), 1);
    assert_eq!(roots.errors[0].0, parent.0.join("broken"));
    assert!(
        roots
            .submodules
            .iter()
            .any(|module| module.path == std::path::Path::new("child")
                && module.initialized
                && module.index == IndexState::Conflict)
    );
    assert!(!parent.0.join("missing").exists());
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);
    assert!(git_dir.join("MERGE_HEAD").is_file());
    // A partially written .gitmodules must not hide the parent's stash view.
    fs::write(parent.0.join(".gitmodules"), "[broken\n").unwrap();
    let roots = Repositories::discover(&repo, &token).unwrap();
    assert_eq!(roots.roots.len(), 1);
    assert_eq!(roots.errors.len(), 1);
    token.cancel();
    assert!(Repositories::discover(&repo, &token).is_err());
}

#[test]
fn group_fetch_failure_reports_not_ready_without_saving_or_updating() {
    use plico::{project_update::Prepared, update_session::UpdateResult};
    let (parent, _source, _remote, _) = group_update_fixture(false);
    fs::write(parent.0.join("base"), "keep local draft\n").unwrap();
    let head = parent.git(&["rev-parse", "HEAD"]);
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = Prepared::read(&repo, &token).unwrap();
    parent.git(&[
        "remote",
        "set-url",
        "origin",
        parent.0.join("missing.git").to_str().unwrap(),
    ]);
    let report = prepared
        .group
        .execute_merge(
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
        )
        .unwrap();
    assert_eq!(report.result(), UpdateResult::NotReady);
    assert!(!report.complete());
    assert!(!report.has_saved_changes());
    assert!(!report.cancelled());
    assert!(report.roots.is_empty());
    assert!(report.diagnostic().unwrap().contains("missing.git"));
    assert_eq!(parent.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read(parent.0.join("base")).unwrap(),
        b"keep local draft\n"
    );
    assert!(parent.git(&["stash", "list"]).is_empty());
}

#[test]
fn group_save_failure_names_the_child_and_preserves_worktrees() {
    use plico::{project_update::Prepared, update_session::UpdateResult};
    let (parent, _source, _remote, _) = group_update_fixture(false);
    fs::write(parent.0.join("child/stable"), "locked child draft\n").unwrap();
    let head = parent.git(&["rev-parse", "HEAD"]);
    let child_head = child_git(&parent, &["rev-parse", "HEAD"]);
    let token = Cancel::default();
    let prepared = Prepared::read(&Repository::new(&parent.0), &token).unwrap();
    let lock = parent.0.join(".git/modules/child/index.lock");
    fs::write(&lock, "fixture lock").unwrap();
    let report = prepared
        .group
        .execute_merge(
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
        )
        .unwrap();
    fs::remove_file(lock).unwrap();
    assert_eq!(
        report.result(),
        UpdateResult::NotReady,
        "{:?}",
        report.diagnostic()
    );
    assert!(!report.complete());
    assert!(
        report
            .roots
            .iter()
            .all(|root| root.merge.is_none() && root.submodule.is_none())
    );
    let diagnostic = report.diagnostic().unwrap();
    assert_eq!(report.saved.len(), 1);
    assert_eq!(
        plico::recent_projects::identity(&report.saved[0].path),
        plico::recent_projects::identity(&parent.0.join("child")),
    );
    assert!(
        diagnostic.contains(&report.saved[0].path.display().to_string()),
        "{diagnostic}"
    );
    let output = report.saved[0].save.as_ref().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr.bytes()).into_owned();
    assert!(!stderr.trim().is_empty());
    assert!(diagnostic.contains("index.lock"), "{diagnostic}");
    assert!(diagnostic.contains("could not write index"), "{diagnostic}");
    assert_eq!(parent.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), child_head);
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"locked child draft\n"
    );
    assert!(parent.git(&["stash", "list"]).is_empty());
    assert!(child_git(&parent, &["stash", "list"]).is_empty());
}

#[test]
fn project_inventory_preserves_independent_roots_and_shared_git_worktrees() {
    use plico::project_repositories::Repositories;
    let first = Fixture::new();
    let second = Fixture::new();
    let linked = first.0.join("linked");
    first.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
    let a = Repository::new(&first.0);
    let b = Repository::new(&second.0);
    let w = Repository::new(&linked);
    let token = Cancel::default();
    assert_eq!(
        first.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
        first.git(&[
            "-C",
            linked.to_str().unwrap(),
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir"
        ])
    );
    let inventory = Repositories::from_registered(&[a.clone(), w, b, a], &token).unwrap();
    assert!(inventory.errors.is_empty(), "{:?}", inventory.errors);
    assert_eq!(inventory.roots.len(), 3);
    assert!(inventory.submodules.is_empty());
    let roots = inventory
        .roots
        .iter()
        .map(|r| r.path.canonicalize().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(roots.contains(&linked.canonicalize().unwrap()));
    assert!(roots.contains(&first.0.canonicalize().unwrap()));
    assert!(roots.contains(&second.0.canonicalize().unwrap()));
    // An unrelated nested repository is not implicitly part of a project.
    assert_eq!(
        Repositories::discover(&Repository::new(&first.0), &token)
            .unwrap()
            .roots
            .len(),
        1
    );
    let missing = Repository::new(first.0.join("absent"));
    let inventory =
        Repositories::from_registered(&[missing, Repository::new(&second.0)], &token).unwrap();
    assert_eq!(inventory.roots.len(), 1);
    assert_eq!(inventory.errors.len(), 1);
    fs::create_dir(first.0.join("plain-directory")).unwrap();
    let inventory =
        Repositories::from_registered(&[Repository::new(first.0.join("plain-directory"))], &token)
            .unwrap();
    assert!(inventory.roots.is_empty());
    assert_eq!(inventory.errors.len(), 1);
    token.cancel();
    assert!(Repositories::from_registered(&[], &token).is_err());
}

#[test]
fn stash_reads_registered_inventory_without_rediscovering_removed_roots() {
    use plico::{project_repositories::Repositories, stash::project};
    let first = Fixture::new();
    let second = Fixture::new();
    for fixture in [&first, &second] {
        fs::write(fixture.0.join("base"), "draft").unwrap();
        fixture.git(&["stash", "push", "-m", "inventory test"]);
    }
    let a = Repository::new(&first.0);
    let b = Repository::new(&second.0);
    let token = Cancel::default();
    let inventory = Repositories::from_registered(&[a.clone(), b.clone()], &token).unwrap();
    let stack = project::load_in(&a, &inventory, &token).unwrap();
    assert!(stack.grouped);
    assert_eq!(stack.entries.len(), 2);
    assert!(
        stack
            .entries
            .iter()
            .any(|entry| entry.repository.path.canonicalize().unwrap()
                == second.0.canonicalize().unwrap())
    );
    // Membership is authoritative even though the primary repository still
    // exists and has a stash. Reading recovery metadata must not re-add it.
    let inventory = Repositories::from_registered(&[b], &token).unwrap();
    let stack = project::load_in(&a, &inventory, &token).unwrap();
    assert!(!stack.grouped);
    assert_eq!(stack.entries.len(), 1);
    assert_eq!(
        stack.entries[0].repository.path.canonicalize().unwrap(),
        second.0.canonicalize().unwrap()
    );
    assert_eq!(
        first.git(&["stash", "list", "--format=%s"]).lines().count(),
        1
    );
}

fn interactive_update_fixture() -> (Fixture, Fixture, Fixture) {
    let (parent, source, remote, _) = group_update_fixture(false);
    let writer = Fixture(remote.0.join("conflict-writer"));
    parent.git(&[
        "clone",
        "-b",
        "main",
        remote.0.join("remote.git").to_str().unwrap(),
        writer.0.to_str().unwrap(),
    ]);
    writer.git(&["config", "user.name", "Test"]);
    writer.git(&["config", "user.email", "test@example.invalid"]);
    writer.git(&["config", "commit.gpgsign", "false"]);
    writer.git(&["config", "core.hooksPath", "disabled-hooks"]);
    fs::write(writer.0.join("base"), "remote parent\n").unwrap();
    writer.git(&["commit", "-am", "remote conflict"]);
    writer.git(&["push", "origin", "main"]);
    fs::write(parent.0.join("base"), "local parent\n").unwrap();
    fs::write(parent.0.join("draft"), "clean draft\n").unwrap();
    parent.git(&["add", "base", "draft"]);
    parent.git(&["commit", "-m", "local parent conflict"]);
    attach_local_topic(&parent);
    fs::write(parent.0.join("child/base"), "local child\n").unwrap();
    child_git(&parent, &["commit", "-am", "local child conflict"]);
    fs::write(parent.0.join("draft"), "parent draft\n").unwrap();
    fs::write(parent.0.join("child/stable"), "child draft\n").unwrap();
    (parent, source, remote)
}

#[test]
fn group_interactive_rebase_resolves_each_root_then_restores_both_drafts() {
    use plico::{
        conflicts::{Resolution, Side},
        project_update::{self, conflicts::Decision},
        published_commits::Protection,
        update_method::Method,
        update_session::UpdateResult,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let mut roots = Vec::new();
    let report = prepared
        .group
        .execute_with_conflicts(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
            |interaction| {
                roots.push(interaction.repository().path.clone());
                // Every root's draft was saved before the first conflict dialog.
                assert_eq!(
                    fs::read(parent.0.join("child/stable")).unwrap(),
                    b"stable\n"
                );
                let snapshot = interaction.snapshot()?;
                for file in &snapshot.files {
                    interaction.resolve(&snapshot, file, Resolution::Accept(Side::Yours))?;
                }
                Ok(Decision::Continue)
            },
        )
        .unwrap();
    assert_eq!(
        roots
            .iter()
            .map(|path| plico::recent_projects::identity(path))
            .collect::<Vec<_>>(),
        [parent.0.clone(), parent.0.join("child")]
            .iter()
            .map(|path| plico::recent_projects::identity(path))
            .collect::<Vec<_>>(),
    );
    assert!(report.complete(), "{:?}", report.diagnostic());
    assert_eq!(report.result(), UpdateResult::ResolvedConflicts);
    assert!(!report.has_saved_changes());
    assert_eq!(fs::read(parent.0.join("draft")).unwrap(), b"parent draft\n");
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"child draft\n"
    );
    assert_eq!(fs::read(parent.0.join("base")).unwrap(), b"local parent\n");
    assert_eq!(
        fs::read(parent.0.join("child/base")).unwrap(),
        b"local child\n"
    );
    assert!(!repo.operation_state(&token).unwrap().in_progress());
    assert!(
        !Repository::new(parent.0.join("child"))
            .operation_state(&token)
            .unwrap()
            .in_progress()
    );
}

#[test]
fn group_interactive_close_continues_other_roots_and_retains_saved_changes() {
    use plico::{
        project_update::{self, conflicts::Decision},
        published_commits::Protection,
        update_method::Method,
        update_session::UpdateResult,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let mut roots = Vec::new();
    let report = prepared
        .group
        .execute_with_conflicts(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
            |interaction| {
                roots.push(interaction.repository().path.clone());
                assert!(!interaction.snapshot()?.files.is_empty());
                Ok(Decision::Close)
            },
        )
        .unwrap();
    assert_eq!(
        roots
            .iter()
            .map(|path| plico::recent_projects::identity(path))
            .collect::<Vec<_>>(),
        [parent.0.clone(), parent.0.join("child")]
            .iter()
            .map(|path| plico::recent_projects::identity(path))
            .collect::<Vec<_>>(),
    );
    assert_eq!(report.result(), UpdateResult::Incomplete);
    assert!(!report.complete());
    assert_eq!(report.saved.len(), 2);
    assert!(
        report
            .saved
            .iter()
            .all(|s| s.saved_oid.is_some() && s.restore.is_none())
    );
    for path in roots {
        let state = Repository::new(path).operation_state(&token).unwrap();
        assert!(state.in_progress() && !state.conflicts.is_empty());
    }
    assert_eq!(
        child_git(&parent, &["show", "refs/stash:stable"]),
        "child draft"
    );
    assert_eq!(parent.git(&["show", "refs/stash:draft"]), "parent draft");
}

#[test]
fn group_interactive_merge_uses_saved_editor_results_and_rejects_wrong_root() {
    use plico::{
        project_update::{self, conflicts::Decision},
        published_commits::Protection,
        update_method::Method,
        update_session::UpdateResult,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let mut previous = None;
    let mut count = 0;
    let report = prepared
        .group
        .execute_with_conflicts(
            Method::Merge,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Merge)),
            |_| false,
            |interaction| {
                count += 1;
                if let Some(wrong) = &previous {
                    assert!(interaction.apply(wrong).is_err());
                    assert!(interaction.stage(wrong).is_err());
                }
                let snapshot = interaction.snapshot()?;
                for file in &snapshot.files {
                    let draft = interaction.prepare(&snapshot, file)?;
                    draft.write_text_result("resolved text\n".into())?;
                    interaction.apply(&draft)?;
                    interaction.stage(&draft)?;
                    previous = Some(draft);
                }
                Ok(Decision::Continue)
            },
        )
        .unwrap();
    assert_eq!(count, 2);
    assert!(report.complete(), "{:?}", report.diagnostic());
    assert_eq!(report.result(), UpdateResult::ResolvedConflicts);
    assert_eq!(fs::read(parent.0.join("base")).unwrap(), b"resolved text\n");
    assert_eq!(
        fs::read(parent.0.join("child/base")).unwrap(),
        b"resolved text\n"
    );
}

#[test]
fn group_interactive_cancel_does_not_continue_or_restore_and_releases_ownership() {
    use plico::{
        conflicts::{Resolution, Side, Snapshot},
        project_update::{self, conflicts::Decision},
        published_commits::Protection,
        update_method::Method,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let child_head = child_git(&parent, &["rev-parse", "HEAD"]);
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let mut count = 0;
    let report = prepared
        .group
        .execute_with_conflicts(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
            |_| {
                count += 1;
                token.cancel();
                // A late Continue must not run Git after cancellation.
                Ok(Decision::Continue)
            },
        )
        .unwrap();
    assert_eq!(count, 1);
    assert!(report.cancelled() && report.has_saved_changes() && !report.complete());
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), child_head);
    assert!(report.saved.iter().all(|saved| saved.restore.is_none()));
    let recovery = Cancel::default();
    let snapshot = Snapshot::read(&repo, &recovery).unwrap();
    assert!(!snapshot.files.is_empty());
    // Ordinary recovery must acquire the released group reservation normally.
    let resolved = plico::conflicts::resolve(
        &repo,
        &snapshot,
        &snapshot.files[0],
        Resolution::Accept(Side::Yours),
        &recovery,
        &Default::default(),
    )
    .unwrap();
    assert!(resolved.snapshot.unwrap().files.is_empty());
}

#[test]
fn group_interactive_continue_rechecks_unresolved_index_instead_of_trusting_ui() {
    use plico::{
        project_update::{self, conflicts::Decision},
        published_commits::Protection,
        update_method::Method,
        update_session::UpdateResult,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let mut count = 0;
    let report = prepared
        .group
        .execute_with_conflicts(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
            |_| {
                count += 1;
                Ok(Decision::Continue)
            },
        )
        .unwrap();
    assert_eq!(count, 2);
    assert_eq!(report.result(), UpdateResult::Error);
    assert!(report.has_saved_changes());
    assert!(report.diagnostic().unwrap().contains("Resolve and stage"));
    for path in [&parent.0, &parent.0.join("child")] {
        let state = Repository::new(path).operation_state(&token).unwrap();
        assert!(state.in_progress() && !state.conflicts.is_empty());
    }
}

#[test]
fn group_interactive_rebase_reopens_resolver_for_the_next_conflicting_commit() {
    use plico::{
        conflicts::{Resolution, Side},
        project_update::{self, conflicts::Decision},
        published_commits::Protection,
        update_method::Method,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    fs::write(parent.0.join("child/base"), "second local child\n").unwrap();
    child_git(&parent, &["add", "base"]);
    child_git(&parent, &["commit", "-m", "second child conflict"]);
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let mut child_interactions = 0;
    let report = prepared
        .group
        .execute_with_conflicts(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
            |interaction| {
                let child = interaction.repository().path.ends_with("child");
                if child {
                    child_interactions += 1;
                }
                let snapshot = interaction.snapshot()?;
                let side = if child && child_interactions == 1 {
                    Side::Theirs
                } else {
                    Side::Yours
                };
                for file in &snapshot.files {
                    interaction.resolve(&snapshot, file, Resolution::Accept(side))?;
                }
                Ok(Decision::Continue)
            },
        )
        .unwrap();
    assert!(report.complete(), "{:?}", report.diagnostic());
    assert_eq!(child_interactions, 2);
    assert_eq!(
        fs::read(parent.0.join("child/base")).unwrap(),
        b"second local child\n"
    );
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"child draft\n"
    );
}

fn wait_update_conflict(
    ui: &plico::project_update::conflicts::channel::Ui,
) -> plico::project_update::conflicts::channel::Pending {
    let deadline = std::time::Instant::now() + operation_timeout(15);
    loop {
        if let Some(pending) = ui.take_pending() {
            return pending;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Update did not publish a conflict"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn group_conflict_channel_runs_editor_commands_on_owner_and_expires_old_root_client() {
    use plico::{
        project_update::{
            self,
            conflicts::{Decision, channel},
        },
        published_commits::Protection,
        update_method::Method,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let (worker, ui) = channel::channel();
    // Cancellation watchdog makes any recursive lock regression fail instead
    // of hanging the test process indefinitely.
    let (done, stopped) = std::sync::mpsc::channel();
    let watchdog_token = token.clone();
    let watchdog = std::thread::spawn(move || {
        if stopped.recv_timeout(operation_timeout(30)).is_err() {
            watchdog_token.cancel();
        }
    });
    let task = std::thread::spawn(move || {
        prepared.group.execute_with_conflicts(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
            |interaction| worker.resolve(interaction),
        )
    });
    let first = wait_update_conflict(&ui);
    assert_eq!(first.client.repository().path, parent.0);
    let file = first.snapshot.files[0].clone();
    let draft = first.client.prepare(first.snapshot.clone(), file).unwrap();
    let automatic = first
        .client
        .resolve_automatically(draft.clone(), None)
        .unwrap()
        .unwrap();
    assert!(!automatic.changed);
    draft.write_text_result("channel result\n".into()).unwrap();
    first.client.apply(draft.clone()).unwrap();
    assert_eq!(
        first.client.saved_text(draft.clone()).unwrap(),
        "channel result\n"
    );
    let staged = first.client.stage_resolved(vec![draft.clone()]);
    assert!(staged.error.is_none());
    assert_eq!(staged.completed.len(), 1);
    assert!(first.client.snapshot().unwrap().files.is_empty());
    assert!(first.client.finish(Decision::Continue));
    let second = wait_update_conflict(&ui);
    assert_eq!(
        plico::recent_projects::identity(&second.client.repository().path),
        plico::recent_projects::identity(&parent.0.join("child")),
    );
    assert!(second.client.apply(draft.clone()).is_err());
    assert!(second.client.stage_resolved(vec![draft]).error.is_some());
    assert!(!first.client.active());
    assert!(first.client.snapshot().is_err());
    assert!(!first.client.finish(Decision::Continue));
    for file in &second.snapshot.files {
        second
            .client
            .resolve(
                second.snapshot.clone(),
                file.clone(),
                plico::conflicts::Resolution::Accept(plico::conflicts::Side::Yours),
            )
            .unwrap();
    }
    assert!(second.client.finish(Decision::Continue));
    let report = task.join().unwrap().unwrap();
    assert!(report.complete(), "{:?}", report.diagnostic());
    assert_eq!(fs::read(parent.0.join("draft")).unwrap(), b"parent draft\n");
    assert_eq!(
        fs::read(parent.0.join("child/stable")).unwrap(),
        b"child draft\n"
    );
    assert!(!second.client.active());
    done.send(()).unwrap();
    watchdog.join().unwrap();
}

#[test]
fn group_conflict_channel_cancellation_invalidates_pending_dialog_without_writes() {
    use plico::{
        project_update::{
            self,
            conflicts::{Decision, channel},
        },
        published_commits::Protection,
        update_method::Method,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let (worker, ui) = channel::channel();
    let cancel = token.clone();
    let task = std::thread::spawn(move || {
        prepared.group.execute_with_conflicts(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
            |interaction| worker.resolve(interaction),
        )
    });
    let pending = wait_update_conflict(&ui);
    let before = fs::read(parent.0.join("base")).unwrap();
    cancel.cancel();
    assert!(!pending.client.finish(Decision::Continue));
    assert!(
        pending
            .client
            .resolve(
                pending.snapshot.clone(),
                pending.snapshot.files[0].clone(),
                plico::conflicts::Resolution::Accept(plico::conflicts::Side::Theirs)
            )
            .is_err()
    );
    let report = task.join().unwrap().unwrap();
    assert!(report.cancelled() && report.has_saved_changes());
    assert_eq!(fs::read(parent.0.join("base")).unwrap(), before);
}

#[test]
fn group_conflict_channel_dismissal_keeps_each_root_incomplete_without_blocking() {
    use plico::{
        project_update::{self, conflicts::channel},
        published_commits::Protection,
        update_method::Method,
        update_session::UpdateResult,
    };
    let (parent, _source, _remote) = interactive_update_fixture();
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let prepared = project_update::Prepared::read(&repo, &token).unwrap();
    let (worker, ui) = channel::channel();
    let task = std::thread::spawn(move || {
        prepared.group.execute_with_conflicts(
            Method::Rebase,
            std::ffi::OsStr::new("true"),
            &token,
            &Default::default(),
            None,
            Protection::Defaults,
            |_| Ok(Some(Method::Rebase)),
            |_| false,
            |interaction| worker.resolve(interaction),
        )
    });
    let first = wait_update_conflict(&ui);
    drop(first); // Dropping the last dialog handle must release this updater.
    let second = wait_update_conflict(&ui);
    assert_eq!(
        plico::recent_projects::identity(&second.client.repository().path),
        plico::recent_projects::identity(&parent.0.join("child")),
    );
    drop(second);
    drop(ui);
    let report = task.join().unwrap().unwrap();
    assert_eq!(report.result(), UpdateResult::Incomplete);
    assert_eq!(report.roots.len(), 2);
    assert!(report.has_saved_changes());
}

#[test]
fn project_mappings_survive_gitlink_removal_but_revalidate_repository_roots() {
    use plico::project_repositories::Mappings;
    let (parent, _source, _, _) = update_fixture();
    parent.git(&["add", "child", ".gitmodules"]);
    parent.git(&["commit", "-m", "register child"]);
    let token = Cancel::default();
    let mut mappings = Mappings::new(vec![Repository::new(&parent.0)]);
    assert_eq!(mappings.refresh(&token).unwrap().roots.len(), 2);
    let child = parent.0.join("child");
    let child_head = child_git(&parent, &["rev-parse", "HEAD"]);
    fs::write(child.join("base"), "keep my draft\n").unwrap();
    parent.git(&["rm", "--cached", "child"]);
    fs::write(parent.0.join(".gitmodules"), "").unwrap();
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    let inventory = mappings.refresh(&token).unwrap();
    assert_eq!(inventory.roots.len(), 2);
    assert!(inventory.submodules.is_empty());
    assert!(inventory.roots.iter().any(|root| root.path == child));
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), child_head);
    assert_eq!(fs::read(child.join("base")).unwrap(), b"keep my draft\n");
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);

    // A child whose .git is gone must not silently become its parent repository.
    fs::remove_file(child.join(".git")).unwrap();
    let inventory = mappings.refresh(&token).unwrap();
    assert_eq!(inventory.roots.len(), 1);
    assert!(
        inventory.errors.iter().any(|(path, _)| {
            plico::recent_projects::identity(path) == plico::recent_projects::identity(&child)
        }),
        "expected error for {child:?}, got {:?}",
        inventory.errors
    );
    assert_eq!(mappings.registered().len(), 2);
}

#[test]
fn cancelled_project_scan_does_not_register_partial_discovery() {
    use plico::project_repositories::Mappings;
    let (parent, _source, _, _) = update_fixture();
    let mut mappings = Mappings::new(vec![Repository::new(&parent.0)]);
    let token = Cancel::default();
    token.cancel();
    assert!(mappings.refresh(&token).is_err());
    assert_eq!(mappings.registered().len(), 1);
    assert_eq!(mappings.refresh(&Cancel::default()).unwrap().roots.len(), 2);
    assert_eq!(mappings.registered().len(), 2);
}

#[test]
fn persisted_project_mappings_recover_removed_submodule_after_restart() {
    use plico::project_repositories::{Mappings, Store};
    let (parent, _source, _, _) = update_fixture();
    parent.git(&["add", "child", ".gitmodules"]);
    parent.git(&["commit", "-m", "register child"]);
    let settings = Fixture::new();
    let store = Store::new(settings.0.join("project-roots.json"), &parent.0);
    let token = Cancel::default();
    let mut first = Mappings::new(vec![Repository::new(&parent.0)]);
    assert_eq!(
        first.refresh_persisted(&store, &token).unwrap().roots.len(),
        2
    );
    drop(first);
    parent.git(&["rm", "--cached", "child"]);
    fs::write(parent.0.join(".gitmodules"), "").unwrap();
    fs::write(parent.0.join("child/base"), "keep recovery draft\n").unwrap();
    let mut reopened = Mappings::new(vec![Repository::new(&parent.0)]);
    let inventory = reopened.refresh_persisted(&store, &token).unwrap();
    assert!(inventory.errors.is_empty(), "{:?}", inventory.errors);
    assert_eq!(inventory.roots.len(), 2);
    assert!(inventory.submodules.is_empty());
    assert_eq!(
        fs::read(parent.0.join("child/base")).unwrap(),
        b"keep recovery draft\n"
    );
}

#[test]
fn mapping_store_preserves_other_windows_and_reports_corrupt_records() {
    use plico::project_repositories::{Mappings, Store};
    let parent = Fixture::new();
    let other = Fixture::new();
    let settings = Fixture::new();
    let path = settings.0.join("project-roots.json");
    let store = Store::new(path.clone(), &parent.0);
    let token = Cancel::default();
    let mut window_a = Mappings::new(vec![Repository::new(&parent.0)]);
    let mut window_b = Mappings::new(vec![Repository::new(&parent.0), Repository::new(&other.0)]);
    window_b.refresh_persisted(&store, &token).unwrap();
    assert_eq!(
        window_a
            .refresh_persisted(&store, &token)
            .unwrap()
            .roots
            .len(),
        2
    );
    let bytes = fs::read(&path).unwrap();
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(window_a.refresh_persisted(&store, &cancelled).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let wrong_project = Store::new(path.clone(), &other.0);
    assert!(
        !window_a
            .refresh_persisted(&wrong_project, &token)
            .unwrap()
            .errors
            .is_empty()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    fs::write(&path, b"broken").unwrap();
    let inventory = window_a.refresh_persisted(&store, &token).unwrap();
    assert_eq!(inventory.roots.len(), 2);
    assert!(
        inventory
            .errors
            .iter()
            .any(|(_, error)| error.contains("mappings"))
    );
    assert_eq!(fs::read(&path).unwrap(), b"broken");
}

#[test]
fn simultaneous_mapping_writers_merge_instead_of_losing_roots() {
    use plico::project_repositories::{Mappings, Store};
    let parent = Fixture::new();
    let settings = Fixture::new();
    let roots = (0..4).map(|_| Fixture::new()).collect::<Vec<_>>();
    let store = Store::new(settings.0.join("roots.json"), &parent.0);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(roots.len()));
    let threads = roots
        .iter()
        .map(|root| {
            let store = store.clone();
            let barrier = barrier.clone();
            let mut mappings =
                Mappings::new(vec![Repository::new(&parent.0), Repository::new(&root.0)]);
            std::thread::spawn(move || {
                barrier.wait();
                let result = mappings
                    .refresh_persisted(&store, &Cancel::default())
                    .unwrap();
                assert!(result.errors.is_empty(), "{:?}", result.errors);
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    let mut reopened = Mappings::new(vec![Repository::new(&parent.0)]);
    let result = reopened
        .refresh_persisted(&store, &Cancel::default())
        .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.roots.len(), 5);
}

fn ordinary_update_fixture() -> (Fixture, Fixture, String) {
    let source = Fixture::new();
    let local = Fixture::new();
    let bare = source.0.join("remote.git");
    source.git(&["init", "--bare", bare.to_str().unwrap()]);
    source.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    source.git(&["push", "-u", "origin", "main"]);
    local.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    local.git(&["fetch", "origin"]);
    local.git(&["reset", "--hard", "origin/main"]);
    local.git(&["branch", "--set-upstream-to=origin/main"]);
    fs::write(source.0.join("base"), "new remote\n").unwrap();
    source.git(&["commit", "-am", "remote"]);
    source.git(&["push", "origin", "main"]);
    let target = source.git(&["rev-parse", "HEAD"]);
    (local, source, target)
}

#[test]
fn project_update_executes_all_registered_ordinary_roots() {
    use plico::project_update::Prepared;
    let (a, _remote_a, target_a) = ordinary_update_fixture();
    let (b, _remote_b, target_b) = ordinary_update_fixture();
    let registered = [Repository::new(&a.0), Repository::new(&b.0)];
    let token = Cancel::default();
    fs::write(a.0.join("draft"), "keep A").unwrap();
    fs::write(b.0.join("draft"), "keep B").unwrap();
    let prepared = Prepared::read_registered(&registered, &token).unwrap();
    assert_eq!(prepared.editor_directories.len(), 2);
    let report = prepared
        .group
        .execute_merge(
            std::ffi::OsStr::new("false"),
            &token,
            &Default::default(),
            None,
        )
        .unwrap();
    assert!(report.complete(), "{:?}", report.diagnostic());
    assert_eq!(report.roots.len(), 2);
    assert_eq!(a.git(&["rev-parse", "HEAD"]), target_a);
    assert_eq!(b.git(&["rev-parse", "HEAD"]), target_b);
    assert_eq!(fs::read(a.0.join("draft")).unwrap(), b"keep A");
    assert_eq!(fs::read(b.0.join("draft")).unwrap(), b"keep B");
}

#[test]
fn project_update_shared_preflight_revalidates_every_root_before_fetch() {
    use plico::project_update::Prepared;
    for change in ["tracking", "branch", "operation"] {
        let (a, _remote_a, _) = ordinary_update_fixture();
        let (b, _remote_b, _) = ordinary_update_fixture();
        let token = Cancel::default();
        let registered = [Repository::new(&a.0), Repository::new(&b.0)];
        let prepared = Prepared::read_registered(&registered, &token).unwrap();
        let a_head = a.git(&["rev-parse", "HEAD"]);
        let a_fetch = fs::read(a.0.join(".git/FETCH_HEAD")).unwrap();
        let b_head = b.git(&["rev-parse", "HEAD"]);
        fs::write(b.0.join("draft"), "staged\n").unwrap();
        b.git(&["add", "draft"]);
        fs::write(b.0.join("draft"), "unstaged\n").unwrap();
        match change {
            "tracking" => {
                b.git(&["branch", "--unset-upstream"]);
            }
            "branch" => {
                b.git(&["checkout", "-b", "other", "--track", "origin/main"]);
            }
            "operation" => {
                fs::write(b.0.join(".git/MERGE_HEAD"), format!("{b_head}\n")).unwrap();
            }
            _ => unreachable!(),
        }
        let index = fs::read(b.0.join(".git/index")).unwrap();
        let result = prepared.group.execute_merge(
            std::ffi::OsStr::new("false"),
            &token,
            &Default::default(),
            None,
        );
        assert!(
            result.is_err(),
            "stale {change} must reject the whole preflight"
        );
        assert_eq!(a.git(&["rev-parse", "HEAD"]), a_head);
        assert_eq!(fs::read(a.0.join(".git/FETCH_HEAD")).unwrap(), a_fetch);
        assert_eq!(b.git(&["rev-parse", "HEAD"]), b_head);
        assert_eq!(fs::read(b.0.join(".git/index")).unwrap(), index);
        assert_eq!(fs::read(b.0.join("draft")).unwrap(), b"unstaged\n");
    }
}

#[test]
fn project_update_can_skip_primary_without_upstream_or_with_detached_head() {
    use plico::project_update::{Prepared, roots::SkipReason};
    for detached in [false, true] {
        let (a, _remote_a, _) = ordinary_update_fixture();
        let (b, _remote_b, target_b) = ordinary_update_fixture();
        if detached {
            a.git(&["checkout", "--detach"]);
        } else {
            a.git(&["branch", "--unset-upstream"]);
        }
        let before = a.git(&["rev-parse", "HEAD"]);
        let registered = [Repository::new(&a.0), Repository::new(&b.0)];
        let token = Cancel::default();
        let prepared = Prepared::read_registered(&registered, &token).unwrap();
        assert_eq!(prepared.editor_directories.len(), 1);
        let report = prepared
            .group
            .execute_merge(
                std::ffi::OsStr::new("false"),
                &token,
                &Default::default(),
                None,
            )
            .unwrap();
        assert!(report.complete(), "{:?}", report.diagnostic());
        assert_eq!(report.roots.len(), 1);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].path, a.0);
        assert_eq!(
            report.skipped[0].reason,
            if detached {
                SkipReason::DetachedHead
            } else {
                SkipReason::NoUpstream
            }
        );
        assert_eq!(a.git(&["rev-parse", "HEAD"]), before);
        assert_eq!(b.git(&["rev-parse", "HEAD"]), target_b);
        b.git(&["branch", "--unset-upstream"]);
        assert!(Prepared::read_registered(&registered, &token).is_err());
    }
}
