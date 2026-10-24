use plico::{
    git::{Cancel, Repository},
    worktrees,
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let f = Self(std::env::temp_dir().join(format!(
            "plico-branch-create-{}-{}",
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
        fs::write(f.0.join("a"), "base\n").unwrap();
        f.git(&["add", "a"]);
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
    fn repo(&self) -> Repository {
        Repository::new(&self.0)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn create_preserves_current_edits_and_confirmed_removal_discards_target_changes() {
    let f = Fixture::new();
    let token = Cancel::default();
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let plan = worktrees::Create::prepare(
        &f.repo(),
        f.0.join("linked"),
        "refs/heads/main".into(),
        Some("task".into()),
        &token,
    )
    .unwrap();
    let report = worktrees::create(&f.repo(), &plan, &token, &Default::default()).unwrap();
    assert!(report.succeeded(), "{}", report.diagnostic());
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "draft\n");
    assert_eq!(f.git(&["branch", "--show-current"]), "main");
    let trees = report.trees.unwrap();
    assert!(trees[0].main && trees[0].current);
    let linked = trees
        .iter()
        .find(|t| t.branch.as_deref() == Some("refs/heads/task"))
        .unwrap();
    let local = worktrees::list(&Repository::new(&linked.path), &token).unwrap();
    assert!(local.iter().any(|t| t.current && !t.main));
    fs::write(linked.path.join("untracked"), "disposable").unwrap();
    fs::write(linked.path.join("a"), "changed").unwrap();
    assert!(
        worktrees::remove(
            &Repository::new(&linked.path),
            local.iter().find(|t| t.current).unwrap(),
            &token,
            &Default::default()
        )
        .is_err()
    );
    let removed = worktrees::remove(&f.repo(), linked, &token, &Default::default()).unwrap();
    assert!(removed.succeeded(), "{}", removed.diagnostic());
    assert!(!linked.path.exists());
    assert_eq!(removed.trees.unwrap().len(), 1);
    assert!(worktrees::remove(&f.repo(), &trees[0], &token, &Default::default()).is_err());
}

#[test]
fn existing_remote_and_tag_worktrees_have_expected_branch_identity() {
    let f = Fixture::new();
    let token = Cancel::default();
    f.git(&["branch", "other"]);
    f.git(&["tag", "v1"]);
    f.git(&["remote", "add", "origin", "."]);
    f.git(&["update-ref", "refs/remotes/origin/topic", "HEAD"]);
    for (reference, folder, branch) in [
        ("refs/heads/other", "local", Some("refs/heads/other")),
        (
            "refs/remotes/origin/topic",
            "remote",
            Some("refs/heads/topic"),
        ),
        ("refs/tags/v1", "tag", None),
    ] {
        let path = f.0.join(folder);
        let plan =
            worktrees::Create::prepare(&f.repo(), path.clone(), reference.into(), None, &token)
                .unwrap();
        let result = worktrees::create(&f.repo(), &plan, &token, &Default::default()).unwrap();
        assert!(result.succeeded(), "{}", result.diagnostic());
        assert_eq!(
            result
                .trees
                .unwrap()
                .iter()
                .find(|t| t.path == path)
                .unwrap()
                .branch
                .as_deref(),
            branch
        );
    }
    assert_eq!(f.git(&["config", "branch.topic.remote"]), "origin");
    assert_eq!(f.git(&["config", "branch.topic.merge"]), "refs/heads/topic");
    assert!(
        worktrees::Create::prepare(
            &f.repo(),
            f.0.join("duplicate"),
            "refs/heads/other".into(),
            None,
            &token
        )
        .is_err()
    );
}

#[test]
fn refuses_stale_source_existing_destination_locked_worktree_and_cancellation() {
    let f = Fixture::new();
    let token = Cancel::default();
    let plan = worktrees::Create::prepare(
        &f.repo(),
        f.0.join("linked"),
        "refs/heads/main".into(),
        Some("task".into()),
        &token,
    )
    .unwrap();
    f.git(&["commit", "--allow-empty", "-m", "advance"]);
    assert!(worktrees::create(&f.repo(), &plan, &token, &Default::default()).is_err());
    assert!(!plan.path.exists());
    let plan = worktrees::Create::prepare(
        &f.repo(),
        plan.path,
        plan.reference,
        plan.new_branch,
        &token,
    )
    .unwrap();
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(worktrees::create(&f.repo(), &plan, &cancelled, &Default::default()).is_err());
    let result = worktrees::create(&f.repo(), &plan, &token, &Default::default()).unwrap();
    assert!(result.succeeded());
    f.git(&["worktree", "lock", plan.path.to_str().unwrap()]);
    let trees = worktrees::list(&f.repo(), &token).unwrap();
    let linked = trees.iter().find(|t| t.path == plan.path).unwrap();
    assert!(linked.locked);
    assert!(
        !worktrees::remove(&f.repo(), linked, &token, &Default::default())
            .unwrap()
            .succeeded()
    );
    assert!(
        worktrees::Create::prepare(
            &f.repo(),
            plan.path,
            "refs/heads/main".into(),
            Some("new".into()),
            &token
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn nul_list_keeps_newlines_in_paths() {
    let f = Fixture::new();
    let path = f.0.join("linked\nwith spaces");
    f.git(&["worktree", "add", "-b", "special", path.to_str().unwrap()]);
    assert!(
        worktrees::list(&f.repo(), &Cancel::default())
            .unwrap()
            .iter()
            .any(|t| t.path == path && t.branch.as_deref() == Some("refs/heads/special"))
    );
}

#[test]
fn prune_revalidates_confirmation_and_preserves_live_and_locked_worktrees() {
    let f = Fixture::new();
    let token = Cancel::default();
    for name in ["missing", "live", "locked"] {
        f.git(&[
            "worktree",
            "add",
            "--detach",
            f.0.join(name).to_str().unwrap(),
            "HEAD",
        ]);
    }
    f.git(&["worktree", "lock", f.0.join("locked").to_str().unwrap()]);
    fs::remove_dir_all(f.0.join("missing")).unwrap();
    fs::remove_dir_all(f.0.join("locked")).unwrap();
    fs::write(f.0.join("live/a"), "draft").unwrap();
    let paths: Vec<_> = worktrees::list(&f.repo(), &token)
        .unwrap()
        .into_iter()
        .filter(|t| t.prunable)
        .map(|t| t.path)
        .collect();
    assert_eq!(paths, vec![f.0.join("missing")]);
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(worktrees::prune(&f.repo(), &paths, &cancelled, &Default::default()).is_err());
    assert!(worktrees::prune(&f.repo(), &[], &token, &Default::default()).is_err());
    // Restore the original linked checkout: the old confirmation must not prune it.
    fs::create_dir(f.0.join("missing")).unwrap();
    fs::write(
        f.0.join("missing/.git"),
        format!("gitdir: {}\n", f.0.join(".git/worktrees/missing").display()),
    )
    .unwrap();
    assert!(worktrees::prune(&f.repo(), &paths, &token, &Default::default()).is_err());
    fs::remove_dir_all(f.0.join("missing")).unwrap();
    let result = worktrees::prune(&f.repo(), &paths, &token, &Default::default()).unwrap();
    assert!(result.succeeded(), "{}", result.diagnostic());
    let trees = result.trees.unwrap();
    assert_eq!(trees.len(), 3);
    assert!(trees.iter().any(|t| t.locked));
    assert!(!trees.iter().any(|t| t.path == f.0.join("missing")));
    assert_eq!(fs::read_to_string(f.0.join("live/a")).unwrap(), "draft");
}

#[test]
fn failed_and_interrupted_commands_retain_diagnostics_without_git_output() {
    use plico::operation_process::{self, StopReason};
    let f = Fixture::new();
    fs::write(f.0.join("a"), "modified\n").unwrap();
    let mut command = Command::new("git");
    command.arg("-C").arg(&f.0).args(["diff", "--quiet"]);
    let outcome =
        operation_process::run(&mut command, &Cancel::default(), &Default::default(), None)
            .unwrap();
    assert!(!outcome.status.success());
    assert!(outcome.stdout.bytes().is_empty());
    assert!(outcome.stderr.bytes().is_empty());
    let mut report = worktrees::Report {
        command: Ok(outcome),
        trees: Ok(vec![]),
    };
    assert!(!report.succeeded());
    assert!(report.diagnostic().contains("Git exited"));
    report.command.as_mut().unwrap().stopped = Some(StopReason::Cancelled);
    assert!(report.diagnostic().contains("cancelled"));
    report.command.as_mut().unwrap().stopped = Some(StopReason::TimedOut);
    report.trees = Err("Refresh failed".into());
    assert!(report.diagnostic().contains("timed out"));
    assert!(report.diagnostic().contains("Refresh failed"));
}
