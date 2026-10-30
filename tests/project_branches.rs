use plico::{
    branch_popup::project::{Project, Scope},
    git::{Cancel, Repository},
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "plico-popup-roots-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn repository(&self, name: &str) -> Repository {
        let path = self.0.join(name);
        std::fs::create_dir(&path).unwrap();
        git(&path, &["init", "-q", "-b", "main"]);
        git(&path, &["config", "user.name", "Test"]);
        git(&path, &["config", "user.email", "test@example.invalid"]);
        git(&path, &["config", "commit.gpgsign", "false"]);
        git(&path, &["config", "core.hooksPath", "disabled-hooks"]);
        git(&path, &["commit", "--allow-empty", "-qm", name]);
        Repository::new(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
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
#[test]
fn project_popup_preserves_reference_scope_and_repository_local_targets() {
    let f = Fixture::new();
    let first = f.repository("first");
    let second = f.repository("second");
    for repo in [&first, &second] {
        git(&repo.path, &["branch", "shared"]);
        git(
            &repo.path,
            &[
                "-c",
                "tag.gpgsign=false",
                "tag",
                "-a",
                "shared",
                "-m",
                "annotated",
            ],
        );
    }
    git(&first.path, &["branch", "only-first"]);
    let token = Cancel::default();
    let project = Project::read(&[first.clone(), second.clone(), first.clone()], &token).unwrap();
    assert_eq!(project.roots.len(), 2);
    assert_eq!(
        project.scope(Some(&first.path), false, false),
        Scope::Repository(first.path.clone())
    );
    assert_eq!(project.scope(Some(&first.path), true, false), Scope::Common);
    assert_eq!(project.scope(Some(&first.path), false, true), Scope::Search);
    assert_eq!(
        project.scope(Some(Path::new("removed")), false, false),
        Scope::Common
    );
    let common = project.common_references();
    assert!(common.contains("refs/heads/shared") && common.contains("refs/tags/shared"));
    assert!(!common.contains("refs/heads/only-first"));
    let targets = project
        .targets(&Scope::Common, "refs/heads/shared")
        .unwrap();
    assert_eq!(targets.len(), 2);
    assert_ne!(targets[0].commit, targets[1].commit);
    assert_eq!(targets[0].root, first.path);
    assert_eq!(
        project
            .targets(&Scope::Repository(second.path.clone()), "refs/tags/shared")
            .unwrap()[0]
            .commit,
        targets[1].commit
    );
    assert!(
        project
            .targets(&Scope::Common, "refs/heads/only-first")
            .is_err()
    );
    assert!(
        project
            .targets(&Scope::Search, "refs/heads/shared")
            .is_err()
    );
    assert!(
        project
            .roots
            .iter()
            .all(|root| root.references.as_ref().unwrap().commits.is_empty())
    );
    let broken = Project::read(&[first, Repository::new(f.0.join("missing"))], &token).unwrap();
    assert!(broken.roots[1].references.is_err());
    assert!(broken.common_references().is_empty());
    assert!(broken.targets(&Scope::Common, "refs/heads/main").is_err());
    token.cancel();
    assert!(Project::read(&[second], &token).is_err());
    assert_eq!(Project::default().scope(None, false, false), Scope::Empty);
}
#[test]
fn project_history_hands_off_primary_metadata_and_preserves_filtered_scope() {
    use plico::project_log::{self, Query};
    let f = Fixture::new();
    let primary = f.repository("primary");
    let child = f.repository("child");
    let base = git(&primary.path, &["rev-parse", "HEAD"]);
    git(&primary.path, &["branch", "topic"]);
    git(
        &primary.path,
        &["commit", "--allow-empty", "-qm", "new main"],
    );
    git(&primary.path, &["tag", "base", &base]);
    let token = Cancel::default();
    let query = |repository: Repository, branches| Query {
        repository,
        branches,
        filter: None,
    };
    let queries = [
        query(primary.clone(), vec!["refs/heads/topic".into()]),
        query(child, vec![]),
    ];
    let (read, refs) = project_log::read_with_primary(&queries, 10, &primary, &token).unwrap();
    let refs = refs.unwrap();
    let expected = primary.reference_snapshot(&token).unwrap();
    assert!(read.scope.errors.is_empty());
    assert_eq!(read.page.entries.len(), 2);
    assert_eq!(read.scope.roots[0].tips, [base]);
    assert_eq!(refs.head_id, expected.head_id);
    assert_eq!(refs.current_branch, expected.current_branch);
    assert_eq!(refs.ref_titles, expected.ref_titles);
    assert_eq!(refs.tracked_branches, expected.tracked_branches);
    assert_eq!(
        format!("{:?}", refs.branches),
        format!("{:?}", expected.branches)
    );
    assert_eq!(format!("{:?}", refs.tags), format!("{:?}", expected.tags));
    assert!(refs.commits.is_empty() && refs.included_in_head.is_empty());
    assert!(!refs.more);
    let plain = project_log::read(&queries, 10, &token).unwrap();
    assert_eq!(
        read.page
            .entries
            .iter()
            .map(|e| e.identity())
            .collect::<Vec<_>>(),
        plain
            .page
            .entries
            .iter()
            .map(|e| e.identity())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        format!("{:?}", read.page.layout(&token).unwrap()),
        format!("{:?}", plain.page.layout(&token).unwrap())
    );
    let (empty, refs) = project_log::read_with_primary(
        &[Query {
            repository: primary.clone(),
            branches: vec![],
            filter: Some(plico::git::LogFilter {
                authors: Some(vec![]),
                ..Default::default()
            }),
        }],
        10,
        &primary,
        &token,
    )
    .unwrap();
    assert!(empty.page.entries.is_empty() && empty.scope.errors.is_empty());
    assert_eq!(refs.unwrap().head_id, expected.head_id);
    token.cancel();
    assert!(project_log::read_with_primary(&queries, 10, &primary, &token).is_err());
}

#[test]
fn project_primary_references_survive_exclusion_and_history_errors_without_hiding_them() {
    use plico::project_log::{self, Query};
    let f = Fixture::new();
    let primary = f.repository("primary");
    let child = f.repository("child");
    let token = Cancel::default();
    let child_query = Query {
        repository: child,
        branches: vec![],
        filter: None,
    };
    let (read, refs) =
        project_log::read_with_primary(std::slice::from_ref(&child_query), 10, &primary, &token)
            .unwrap();
    assert_eq!(read.page.entries.len(), 1);
    assert_eq!(
        refs.unwrap().head_id.as_deref(),
        Some(git(&primary.path, &["rev-parse", "HEAD"]).as_str())
    );
    let missing_branch = Query {
        repository: primary.clone(),
        branches: vec!["refs/heads/missing".into()],
        filter: None,
    };
    let (read, refs) = project_log::read_with_primary(
        &[missing_branch, child_query.clone()],
        10,
        &primary,
        &token,
    )
    .unwrap();
    assert!(
        refs.is_ok(),
        "Log branch errors must not erase readable repository metadata"
    );
    assert_eq!(read.page.entries.len(), 1);
    assert_eq!(read.scope.errors.len(), 1);
    assert!(read.scope.errors[0].1.contains("Branch no longer exists"));
    let parent = git(&primary.path, &["rev-parse", "HEAD"]);
    git(
        &primary.path,
        &["commit", "--allow-empty", "-qm", "new head"],
    );
    std::fs::remove_file(
        primary
            .path
            .join(".git/objects")
            .join(&parent[..2])
            .join(&parent[2..]),
    )
    .unwrap();
    let (read, refs) = project_log::read_with_primary(
        &[
            Query {
                repository: primary.clone(),
                branches: vec![],
                filter: None,
            },
            child_query.clone(),
        ],
        10,
        &primary,
        &token,
    )
    .unwrap();
    assert!(refs.is_ok());
    assert_eq!(read.scope.errors.len(), 1);
    assert_eq!(read.page.entries.len(), 1);
    let absent = Repository::new(f.0.join("absent"));
    let (read, refs) = project_log::read_with_primary(&[child_query], 10, &absent, &token).unwrap();
    assert!(refs.is_err());
    assert_eq!(read.page.entries.len(), 1);
}

#[test]
fn reference_snapshot_does_not_walk_history_and_supports_unborn_head() {
    let f = Fixture::new();
    let repo = f.repository("history");
    let parent = git(&repo.path, &["rev-parse", "HEAD"]);
    git(&repo.path, &["commit", "--allow-empty", "-qm", "second"]);
    std::fs::remove_file(
        repo.path
            .join(".git/objects")
            .join(&parent[..2])
            .join(&parent[2..]),
    )
    .unwrap();
    let refs = repo.reference_snapshot(&Cancel::default()).unwrap();
    assert_eq!(refs.current_branch.as_deref(), Some("main"));
    assert!(refs.commits.is_empty());
    assert!(
        repo.snapshot(None, 10, None, &Cancel::default()).is_err(),
        "fixture must require missing history for a Log walk"
    );
    let empty = f.0.join("unborn");
    std::fs::create_dir(&empty).unwrap();
    git(&empty, &["init", "-q", "-b", "main"]);
    let unborn = Repository::new(empty);
    let (read, refs) = plico::project_log::read_with_primary(
        &[plico::project_log::Query {
            repository: unborn.clone(),
            branches: vec![],
            filter: None,
        }],
        10,
        &unborn,
        &Cancel::default(),
    )
    .unwrap();
    assert!(read.page.entries.is_empty() && read.scope.errors.is_empty());
    let refs = refs.unwrap();
    assert!(refs.head_id.is_none());
    assert_eq!(refs.current_branch.as_deref(), Some("main"));
}

#[test]
fn upstream_labels_require_agreement_and_remain_reference_scoped() {
    let f = Fixture::new();
    let first = f.repository("tracking-first");
    let second = f.repository("tracking-second");
    let token = Cancel::default();
    for repo in [&first, &second] {
        git(
            &repo.path,
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/repo.git",
            ],
        );
        git(
            &repo.path,
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
        git(
            &repo.path,
            &["update-ref", "refs/remotes/origin/other", "HEAD"],
        );
        git(
            &repo.path,
            &["branch", "--set-upstream-to=origin/main", "main"],
        );
        git(&repo.path, &["branch", "same-commit"]);
    }
    let read = || Project::read(&[first.clone(), second.clone()], &token).unwrap();
    let project = read();
    assert_eq!(
        project.tracked_branch(&Scope::Common, "refs/heads/main"),
        Some("origin/main")
    );
    assert_eq!(
        project.tracked_branch(&Scope::Common, "refs/heads/same-commit"),
        None
    );
    assert!(
        project
            .roots
            .iter()
            .all(|root| root.references.as_ref().unwrap().commits.is_empty())
    );
    git(
        &second.path,
        &["branch", "--set-upstream-to=origin/other", "main"],
    );
    let project = read();
    assert_eq!(
        project.tracked_branch(&Scope::Common, "refs/heads/main"),
        None
    );
    assert_eq!(
        project.tracked_branch(&Scope::Repository(second.path.clone()), "refs/heads/main"),
        Some("origin/other")
    );
    git(&second.path, &["branch", "--unset-upstream", "main"]);
    assert_eq!(
        read().tracked_branch(&Scope::Common, "refs/heads/main"),
        None
    );
    let mut project = read();
    project.roots[1].references = Err("unreadable root".into());
    assert_eq!(
        project.tracked_branch(&Scope::Common, "refs/heads/main"),
        None
    );
}
