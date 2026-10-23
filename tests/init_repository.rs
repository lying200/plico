use plico::{
    git::Cancel,
    init_repository::{self, Prepared},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "plico-init-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn directory(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

#[test]
fn initializes_nonempty_folder_without_staging_or_overriding_git_defaults() {
    let f = Fixture::new();
    let target = f.directory("project with spaces");
    let reference = f.directory("git-defaults");
    fs::write(target.join("README.md"), "existing content\n").unwrap();
    let token = Cancel::default();
    let prepared = Prepared::read(&target, &token).unwrap();
    assert!(!prepared.needs_confirmation());
    let result = init_repository::execute(&prepared, false, &token, &Default::default()).unwrap();
    assert!(result.status.success());
    assert!(result.stopped.is_none());
    git(&reference, &["init"]);
    assert_eq!(
        git(&target, &["symbolic-ref", "HEAD"]),
        git(&reference, &["symbolic-ref", "HEAD"])
    );
    assert_eq!(git(&target, &["ls-files"]), "");
    assert_eq!(git(&target, &["status", "--porcelain"]), "?? README.md");
    assert_eq!(
        fs::read_to_string(target.join("README.md")).unwrap(),
        "existing content\n"
    );
    assert!(!target.join(".git/index").exists());
}

#[test]
fn existing_nested_changed_and_cancelled_destinations_require_fresh_intent() {
    let f = Fixture::new();
    let target = f.directory("project");
    let token = Cancel::default();
    let before = Prepared::read(&target, &token).unwrap();
    git(&target, &["init", "-b", "custom"]);
    assert!(init_repository::execute(&before, false, &token, &Default::default()).is_err());
    let existing = Prepared::read(&target, &token).unwrap();
    assert!(existing.needs_confirmation());
    let head = fs::read(target.join(".git/HEAD")).unwrap();
    assert!(init_repository::execute(&existing, false, &token, &Default::default()).is_err());
    assert!(
        init_repository::execute(&existing, true, &token, &Default::default())
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(target.join(".git/HEAD")).unwrap(), head);
    let nested = target.join("child");
    fs::create_dir(&nested).unwrap();
    let nested_request = Prepared::read(&nested, &token).unwrap();
    assert!(nested_request.needs_confirmation());
    assert!(init_repository::execute(&nested_request, false, &token, &Default::default()).is_err());
    assert!(!nested.join(".git").exists());
    let fresh = f.directory("cancelled");
    let request = Prepared::read(&fresh, &token).unwrap();
    token.cancel();
    assert!(init_repository::execute(&request, false, &token, &Default::default()).is_err());
    assert!(!fresh.join(".git").exists());
    assert!(Prepared::read(&f.0.join("missing"), &Cancel::default()).is_err());
}

#[test]
fn first_commit_remote_and_first_push_use_the_existing_product_operations() {
    use plico::{commit_operation, git::Repository, push, push_target, remote_management};
    let f = Fixture::new();
    let target = f.directory("project");
    let remote = f.directory("remote.git");
    git(&remote, &["init", "--bare"]);
    let token = Cancel::default();
    let progress = Default::default();
    let request = Prepared::read(&target, &token).unwrap();
    assert!(
        init_repository::execute(&request, false, &token, &progress)
            .unwrap()
            .status
            .success()
    );
    for args in [
        ["config", "user.name", "Initialization Test"],
        ["config", "user.email", "init@example.invalid"],
        ["config", "commit.gpgsign", "false"],
        ["config", "core.hooksPath", "disabled-hooks"],
    ] {
        git(&target, &args);
    }
    let repo = Repository::new(&target);
    assert!(
        repo.snapshot(None, 100, None, &token)
            .unwrap()
            .commits
            .is_empty()
    );
    assert!(repo.push_snapshot(&token).unwrap().is_none());
    fs::write(target.join("README.md"), "First project\n").unwrap();
    let commit = commit_operation::Request {
        add: vec!["README.md".into()],
        remove: vec![],
        message: "Initial commit".into(),
        amend: false,
        expected_head: None,
    };
    assert!(
        commit_operation::execute(&repo, &commit, &token, &progress)
            .unwrap()
            .succeeded()
    );
    assert_eq!(
        repo.snapshot(None, 100, None, &token)
            .unwrap()
            .commits
            .len(),
        1
    );
    assert!(
        remote_management::execute(
            &repo,
            &remote_management::Edit::Add {
                name: "origin".into(),
                url: remote.to_string_lossy().into_owned(),
            },
            &token,
            &progress
        )
        .unwrap()
        .succeeded()
    );
    let snapshot = repo.push_snapshot(&token).unwrap().unwrap();
    let branch = snapshot.source_branch.clone();
    let oid = snapshot.source_oid.clone();
    let result = push::execute(
        &repo,
        &push::Spec {
            tags: None,
            force: false,
            source_branch: branch.clone(),
            source_oid: oid.clone(),
            set_upstream: true,
            target: push_target::Target {
                remote: "origin".into(),
                branch: branch.clone(),
                special: false,
                new_branch: true,
                origin: push_target::Origin::Custom,
            },
        },
        &token,
        &progress,
        None,
    )
    .unwrap();
    assert_eq!(result.status, push::Status::NewBranch);
    assert_eq!(
        git(&remote, &["rev-parse", &format!("refs/heads/{branch}")]),
        oid
    );
    assert_eq!(
        git(&target, &["rev-parse", "--abbrev-ref", "@{upstream}"]),
        format!("origin/{branch}")
    );
    assert!(git(&target, &["status", "--porcelain"]).is_empty());
}
