use plico::git::*;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn merge_completion_and_abort_preserve_repository_state() {
    use plico::{merge_operation::{self, Action}, operation_process::Progress, operation_state::Operation};
    for mode in ["complete", "missing-message", "abort", "abort-failure", "commit-failure", "cancel"] {
        let f = Fixture::new();
        f.write("shared.txt", "base\n");
        f.write("local.txt", "original\n");
        f.commit("base");
        f.git(&["checkout", "-b", "topic"]);
        f.write("shared.txt", "topic\n"); f.commit("topic");
        let other = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "main"]);
        f.write("shared.txt", "main\n"); f.commit("main");
        let before = f.git(&["rev-parse", "HEAD"]);
        f.write("local.txt", "valuable local edit\n");
        let status = Command::new("git").arg("-C").arg(&f.0).args(["merge", "topic"]).output().unwrap();
        assert!(!status.status.success());
        let repo = f.repo(); let token = Cancel::default(); let progress = Progress::default();
        assert!(merge_operation::execute(&repo, Action::Commit, &token, &progress).is_err());
        assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
        if mode == "cancel" {
            token.cancel();
            assert!(merge_operation::execute(&repo, Action::Abort, &token, &progress).is_err());
            assert_eq!(repo.operation_state(&Cancel::default()).unwrap().operation, Operation::Merge);
            continue;
        }
        let abort = mode.starts_with("abort");
        if !abort {
            f.write("shared.txt", "resolved\n"); f.git(&["add", "shared.txt"]);
            if mode == "missing-message" { fs::remove_file(f.0.join(".git/MERGE_MSG")).unwrap(); }
            else { fs::write(f.0.join(".git/MERGE_MSG"), "合并完成\n\nReviewed resolution\n").unwrap(); }
        }
        if mode.ends_with("failure") { fs::write(f.0.join(".git/index.lock"), "fixture lock").unwrap(); }
        let result = merge_operation::execute(&repo, if abort { Action::Abort } else { Action::Commit }, &token, &progress).unwrap();
        let outcome = result.command.unwrap();
        assert_eq!(outcome.status.success(), !mode.ends_with("failure"), "{mode}: {:?}", outcome.stderr.bytes());
        assert_eq!(fs::read_to_string(f.0.join("local.txt")).unwrap(), "valuable local edit\n");
        if mode.ends_with("failure") {
            assert_eq!(result.state.unwrap().operation, Operation::Merge);
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
        } else {
            assert_eq!(result.state.unwrap().operation, Operation::None);
            if abort {
                assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
                assert_eq!(fs::read_to_string(f.0.join("shared.txt")).unwrap(), "main\n");
            } else {
                assert_eq!(f.git(&["rev-parse", "HEAD^1"]), before);
                assert_eq!(f.git(&["rev-parse", "HEAD^2"]), other);
                let message = f.git(&["log", "-1", "--format=%B"]);
                if mode == "missing-message" { assert!(message.starts_with("Merge branch 'main' of ")); }
                else { assert_eq!(message, "合并完成\n\nReviewed resolution"); }
            }
            assert!(merge_operation::execute(&repo, Action::Commit, &token, &progress).is_err());
            assert!(merge_operation::execute(&repo, Action::Abort, &token, &progress).is_err());
        }
    }
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "git-review-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "测试 Developer"]);
        f.git(&["config", "user.email", "test@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).trim().into()
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn commit(&self, message: &str) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "-m", message]);
    }
    fn repo(&self) -> Repository {
        Repository::new(self.0.clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn root_unicode_literal_path_rename_and_local_stages() {
    let f = Fixture::new();
    let repo = f.repo();
    let t = Cancel::default();
    assert!(
        repo.snapshot(None, 100, None, &t)
            .unwrap()
            .commits
            .is_empty()
    );
    f.write("中文 [a].txt", "hello\n");
    f.commit("初始 🦀\n\nAuthor: message text, not metadata\n<review> & details");
    let snapshot = repo.snapshot(None, 100, None, &t).unwrap();
    let c = &snapshot.commits[0];
    assert!(c.parents.is_empty());
    let d = repo.details(c, &t).unwrap();
    assert_eq!(d.author, "测试 Developer <test@example.invalid>");
    assert_eq!(d.committer, d.author);
    assert!(!d.authored.is_empty());
    assert!(!d.committed.is_empty());
    assert_eq!(
        d.message.trim_end(),
        "初始 🦀\n\nAuthor: message text, not metadata\n<review> & details"
    );
    assert_eq!(d.files[0].label(), "中文 [a].txt");
    assert!(
        repo.patch(Some(c), &d.files[0], &t)
            .unwrap()
            .contains("+hello")
    );
    f.git(&["mv", "中文 [a].txt", "renamed.txt"]);
    f.commit("rename");
    let s = repo.snapshot(None, 100, None, &t).unwrap();
    let d = repo.details(&s.commits[0], &t).unwrap();
    assert_eq!(
        d.files[0].old_path.as_ref().unwrap().to_str().unwrap(),
        "中文 [a].txt"
    );
    f.write("renamed.txt", "staged\n");
    f.git(&["add", "renamed.txt"]);
    f.write("renamed.txt", "unstaged\n");
    f.write("new.txt", "untracked\n");
    let files = repo.local_changes(&t).unwrap();
    assert_eq!(files.len(), 3);
    for file in &files {
        let patch = repo.patch(None, file, &t).unwrap();
        match file.area {
            Area::Staged => assert!(patch.contains("+staged")),
            Area::Unstaged => assert!(patch.contains("+unstaged")),
            Area::Untracked => assert_eq!(patch, "untracked\n"),
            _ => panic!(),
        }
    }
}
#[test]
fn ref_labels_keep_types_names_and_peeled_tag_targets() {
    let f = Fixture::new();
    f.write("root.txt", "root\n");
    f.commit("root");
    f.git(&["branch", "origin/main"]);
    f.git(&["branch", "feature,review"]);
    f.git(&["branch", "feature/10"]);
    f.git(&["branch", "feature/2"]);
    f.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    f.git(&["remote", "add", "origin", "."]);
    f.git(&["config", "branch.main.remote", "origin"]);
    f.git(&["config", "branch.main.merge", "refs/heads/main"]);
    f.git(&["tag", "main"]);
    f.git(&["tag", "v1,stable"]);
    f.git(&["tag", "-a", "release", "-m", "annotated"]);
    f.git(&["tag", "-a", "nested", "release", "-m", "nested annotated"]);
    let blob = f.git(&["rev-parse", "HEAD:root.txt"]);
    f.git(&["tag", "blob-only", &blob]);
    f.git(&["tag", "-a", "annotated-blob", &blob, "-m", "blob"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    f.git(&["tag", "tree-only", &tree]);
    f.git(&["tag", "-a", "annotated-tree", &tree, "-m", "tree"]);
    f.git(&[
        "tag",
        "-a",
        "nested-blob",
        "annotated-blob",
        "-m",
        "nested blob",
    ]);
    let repo = f.repo();
    let token = Cancel::default();
    let snapshot = repo.snapshot(None, 20, None, &token).unwrap();
    assert_eq!(snapshot.commits.len(), 1);
    assert_eq!(snapshot.current_branch.as_deref(), Some("main"));
    assert_eq!(
        snapshot.ref_titles[&snapshot.commits[0].id],
        "origin & main"
    );
    let labels = &snapshot.commits[0].refs;
    assert_eq!(
        labels
            .iter()
            .map(|r| (r.kind, r.name.as_str()))
            .collect::<Vec<_>>(),
        [
            (RefKind::Head, "HEAD"),
            (RefKind::Local, "main"),
            (RefKind::Remote, "origin/main"),
            (RefKind::Local, "feature,review"),
            (RefKind::Local, "feature/2"),
            (RefKind::Local, "feature/10"),
            (RefKind::Local, "origin/main"),
            (RefKind::Tag, "main"),
            (RefKind::Tag, "nested"),
            (RefKind::Tag, "release"),
            (RefKind::Tag, "v1,stable"),
        ]
    );
    assert_eq!(repo.containing_branches(&snapshot.commits[0].id, &token).unwrap(),
        ["HEAD", "main", "origin/main", "feature,review", "feature/2", "feature/10", "origin/main"]);
    f.git(&["checkout", "--detach"]);
    let detached = repo.snapshot(None, 20, None, &token).unwrap();
    assert_eq!(&detached.commits[0].refs, labels);
    assert_eq!(detached.current_branch, None);
}

#[test]
fn initial_history_window_keeps_snapshot_roots_and_layout() {
    let f = Fixture::new();
    f.write("file.txt", "one\n");
    f.commit("one");
    f.write("file.txt", "two\n");
    f.commit("two");
    let repo = f.repo();
    let token = Cancel::default();
    let initial = plico::history::read(&repo, None, 0, 3, None, None, &token).unwrap();
    let expected = repo.snapshot(None, 3, None, &token).unwrap();
    assert_eq!(initial.snapshot.tips, expected.tips);
    assert_eq!(
        initial
            .snapshot
            .commits
            .iter()
            .map(|c| &c.id)
            .collect::<Vec<_>>(),
        expected.commits.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    assert_eq!(
        format!("{:?}", initial.rows),
        format!("{:?}", plico::graph::layout(&expected.commits))
    );
}

#[test]
fn default_history_interleaves_branch_dates_without_breaking_paged_graph() {
    let f = Fixture::new();
    f.write("file", "same tree\n");
    f.commit("seed");
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let make = |name: &str, day: u8, parents: &[&str]| {
        let date = format!("2026-01-{day:02}T12:00:00Z");
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&f.0)
            .args(["commit-tree", &tree, "-m", name])
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date);
        for parent in parents {
            command.args(["-p", parent]);
        }
        let output = command.output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    let root = make("root", 1, &[]);
    let a1 = make("a1", 2, &[&root]);
    let b1 = make("b1", 3, &[&root]);
    let a2 = make("a2", 4, &[&a1]);
    let b2 = make("b2", 5, &[&b1]);
    let merge = make("merge", 6, &[&a2, &b2]);
    f.git(&["update-ref", "refs/heads/main", &merge]);
    let repo = f.repo();
    let token = Cancel::default();
    let all = repo.snapshot(None, 20, None, &token).unwrap();
    assert_eq!(
        all.commits
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        [&merge, &b2, &a2, &b1, &a1, &root]
    );
    let full = plico::graph::layout(&all.commits);
    for offset in [0, 2, 4] {
        let page =
            plico::history::read(&repo, None, offset, 2, Some(&all.tips), None, &token)
                .unwrap();
        assert_eq!(
            format!("{:?}", page.rows),
            format!("{:?}", &full[offset..offset + 2])
        );
    }
}

#[test]
fn branch_snapshot_paging_merge_and_worktree() {
    let f = Fixture::new();
    let repo = f.repo();
    let t = Cancel::default();
    f.write("root.txt", "root\n");
    f.commit("root");
    f.git(&["checkout", "-b", "feature"]);
    f.write("feature.txt", "feature\n");
    f.commit("feature");
    f.git(&["checkout", "main"]);
    f.write("main.txt", "main\n");
    f.commit("main");
    f.git(&["merge", "--no-ff", "feature", "-m", "merge"]);
    let first = repo.snapshot(None, 2, None, &t).unwrap();
    assert!(first.more);
    assert_eq!(first.commits[0].parents.len(), 2);
    let all = repo.snapshot(None, 20, Some(&first.tips), &t).unwrap();
    assert_eq!(all.commits.len(), 4);
    assert_eq!(first.commits[0].id, all.commits[0].id);
    let second = repo.snapshot_at(None, 2, 2, Some(&first.tips), &t).unwrap();
    assert!(!second.more);
    assert_eq!(
        second.commits.iter().map(|c| &c.id).collect::<Vec<_>>(),
        all.commits[2..].iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    let (mut rows, frontier) =
        plico::graph::layout_window(&first.commits, Default::default(), 500_000).unwrap();
    let (older, _) =
        plico::graph::layout_window(&second.commits, frontier, 500_000).unwrap();
    rows.extend(older);
    assert_eq!(
        format!("{rows:?}"),
        format!("{:?}", plico::graph::layout(&all.commits))
    );
    // New refs must not move the already-open window's fixed history roots.
    f.write("later.txt", "later\n");
    f.commit("later");
    let fixed = repo.snapshot_at(None, 2, 2, Some(&first.tips), &t).unwrap();
    assert_eq!(fixed.commits[0].id, second.commits[0].id);
    // Returning to a newer window keeps fixed roots even after refs move.
    let back = plico::history::read(&repo, None, 0, 2, Some(&first.tips), None, &t).unwrap();
    assert_eq!(back.snapshot.commits[0].id, first.commits[0].id);
    assert_eq!(
        format!("{:?}", back.rows),
        format!("{:?}", &plico::graph::layout(&all.commits)[..2])
    );
    // Reconstructing a nonzero frontier is equivalent to carrying it forward.
    let returned =
        plico::history::read(&repo, None, 2, 2, Some(&first.tips), None, &t).unwrap();
    assert_eq!(returned.snapshot.commits[0].id, second.commits[0].id);
    assert_eq!(
        format!("{:?}", returned.rows),
        format!("{:?}", &plico::graph::layout(&all.commits)[2..])
    );
    // Refresh of an older window reconstructs lanes against fresh roots,
    // rather than joining the new snapshot onto stale boundary state.
    let refreshed = plico::history::read(&repo, None, 2, 2, None, None, &t).unwrap();
    let fresh = repo.snapshot(None, 20, None, &t).unwrap();
    assert_eq!(refreshed.snapshot.commits[0].id, fresh.commits[2].id);
    assert_eq!(
        format!("{:?}", refreshed.rows),
        format!("{:?}", &plico::graph::layout(&fresh.commits)[2..4])
    );
    let d = repo.details(&all.commits[0], &t).unwrap();
    assert_eq!(d.files.len(), 1);
    assert_eq!(d.files[0].label(), "feature.txt");
    assert!(
        repo.patch(Some(&all.commits[0]), &d.files[0], &t)
            .unwrap()
            .contains("+feature")
    );
    let feature = repo
        .snapshot(Some("refs/heads/feature"), 20, None, &t)
        .unwrap();
    assert_eq!(feature.commits.len(), 2);
    let wt = f.0.join("linked");
    f.git(&["worktree", "add", "--detach", wt.to_str().unwrap(), "HEAD"]);
    assert_eq!(
        Repository::new(wt)
            .snapshot(None, 20, None, &t)
            .unwrap()
            .commits
            .len(),
        5
    );
}
#[test]
fn conflicts_are_index_states_not_graph_guesses() {
    let f = Fixture::new();
    let repo = f.repo();
    let t = Cancel::default();
    f.write("a", "root\n");
    f.commit("root");
    f.git(&["checkout", "-b", "other"]);
    f.write("a", "other\n");
    f.commit("other");
    f.git(&["checkout", "main"]);
    f.write("a", "main\n");
    f.commit("main");
    // Divergent branches alone are not an index conflict.
    assert!(repo.local_changes(&t).unwrap().is_empty());
    let output = Command::new("git")
        .arg("-C")
        .arg(&f.0)
        .args(["merge", "other"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let files = repo.local_changes(&t).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].area, Area::Conflict);
    assert!(
        repo.patch(None, &files[0], &t)
            .unwrap_err()
            .contains("Unmerged")
    );
    // The next read reflects external conflict removal, with no stale state.
    f.git(&["merge", "--abort"]);
    assert!(repo.local_changes(&t).unwrap().is_empty());
}

#[test]
fn reads_do_not_change_index_or_refs_and_cancel_early() {
    let f = Fixture::new();
    f.write("a", "root\n");
    f.commit("root");
    let repo = f.repo();
    let t = Cancel::default();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    let s = repo.snapshot(None, 20, None, &t).unwrap();
    let d = repo.details(&s.commits[0], &t).unwrap();
    repo.patch(Some(&s.commits[0]), &d.files[0], &t).unwrap();
    repo.local_changes(&t).unwrap();
    assert_eq!(index, fs::read(f.0.join(".git/index")).unwrap());
    assert_eq!(head, f.git(&["rev-parse", "HEAD"]));
    t.cancel();
    assert_eq!(repo.snapshot(None, 20, None, &t).unwrap_err(), "Cancelled");
}

#[test]
fn untracked_preview_rejects_replaced_parent_links_and_parent_paths() {
    let f = Fixture::new();
    let outside = Fixture::new();
    fs::create_dir(f.0.join("nested")).unwrap();
    f.write("nested/review.txt", "inside\n");
    outside.write("review.txt", "outside\n");
    let repo = f.repo();
    let token = Cancel::default();
    let change = repo
        .local_changes(&token)
        .unwrap()
        .into_iter()
        .find(|c| c.path == PathBuf::from("nested/review.txt"))
        .unwrap();
    assert_eq!(repo.patch(None, &change, &token).unwrap(), "inside\n");
    fs::remove_file(f.0.join("nested/review.txt")).unwrap();
    fs::remove_dir(f.0.join("nested")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside.0, f.0.join("nested")).unwrap();
    #[cfg(windows)]
    {
        let result = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(f.0.join("nested"))
            .arg(&outside.0)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "Could not create test junction: {:?}",
            result
        );
    }
    assert!(
        repo.patch(None, &change, &token)
            .unwrap_err()
            .contains("linked")
    );
    #[cfg(windows)]
    fs::remove_dir(f.0.join("nested")).unwrap();
    #[cfg(unix)]
    fs::remove_file(f.0.join("nested")).unwrap();
    let parent_path = Change {
        path: "../review.txt".into(),
        old_path: None,
        status: "??".into(),
        area: Area::Untracked,
    };
    assert!(
        repo.patch(None, &parent_path, &token)
            .unwrap_err()
            .contains("inside the repository")
    );
}

#[test]
fn mode_only_changes_remain_visible_without_text_hunks() {
    let f = Fixture::new();
    f.write("run.sh", "echo review\n");
    f.commit("script");
    f.git(&["config", "core.filemode", "false"]);
    f.git(&["update-index", "--chmod=+x", "run.sh"]);
    let repo = f.repo();
    let token = Cancel::default();
    let local = repo.local_changes(&token).unwrap();
    let staged = local.iter().find(|c| c.area == Area::Staged).unwrap();
    let staged_patch = repo.patch(None, staged, &token).unwrap();
    assert!(staged_patch.contains("old mode 100644\nnew mode 100755"));
    assert!(!staged_patch.contains("@@"));
    let full = repo.full_patch(None, staged, &token).unwrap();
    assert!(full.contains("@@ -1,1 +1,1 @@\n echo review\n"));
    f.git(&["commit", "-m", "Make script executable"]);
    let snapshot = repo.snapshot(None, 10, None, &token).unwrap();
    let commit = &snapshot.commits[0];
    let details = repo.details(commit, &token).unwrap();
    assert_eq!(details.files.len(), 1);
    assert_eq!(details.files[0].status, "M");
    let patch = repo.patch(Some(commit), &details.files[0], &token).unwrap();
    assert_eq!(patch, staged_patch);
    assert_eq!(repo.full_patch(Some(commit), &details.files[0], &token).unwrap(), full);
}

#[test]
fn binary_and_oversized_changes_have_explicit_results() {
    let f = Fixture::new();
    fs::write(f.0.join("binary.dat"), [0u8, 1, 2, 3]).unwrap();
    f.commit("binary");
    let repo = f.repo();
    let token = Cancel::default();
    let s = repo.snapshot(None, 10, None, &token).unwrap();
    let d = repo.details(&s.commits[0], &token).unwrap();
    assert!(
        repo.patch(Some(&s.commits[0]), &d.files[0], &token)
            .unwrap()
            .contains("Binary files")
    );
    f.write("large.txt", &"a long source line\n".repeat(600_000));
    f.commit("oversized text");
    let s = repo.snapshot(None, 10, None, &token).unwrap();
    let d = repo.details(&s.commits[0], &token).unwrap();
    assert!(
        repo.patch(Some(&s.commits[0]), &d.files[0], &token)
            .unwrap_err()
            .contains("limit")
    );
}

#[test]
fn submodule_patch_stays_explicit_despite_repository_diff_settings() {
    let f = Fixture::new();
    f.write("root.txt", "one\n");
    f.commit("one");
    let first = f.git(&["rev-parse", "HEAD"]);
    f.write("root.txt", "two\n");
    f.commit("two");
    let second = f.git(&["rev-parse", "HEAD"]);
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        "160000",
        &first,
        "dependency",
    ]);
    f.git(&["commit", "-m", "add submodule pointer"]);
    f.git(&[
        "update-index",
        "--cacheinfo",
        "160000",
        &second,
        "dependency",
    ]);
    f.git(&["commit", "-m", "update submodule pointer"]);
    f.git(&["config", "diff.ignoreSubmodules", "all"]);
    let repo = f.repo();
    let token = Cancel::default();
    for setting in ["log", "diff"] {
        f.git(&["config", "diff.submodule", setting]);
        let snapshot = repo.snapshot(None, 10, None, &token).unwrap();
        let commit = &snapshot.commits[0];
        let details = repo.details(commit, &token).unwrap();
        assert_eq!(details.files.len(), 1);
        let patch = repo.patch(Some(commit), &details.files[0], &token).unwrap();
        assert!(patch.contains("160000"));
        assert!(patch.contains(&format!("-Subproject commit {first}")));
        assert!(patch.contains(&format!("+Subproject commit {second}")));
        // No nested source-code diff or repository-configured log format.
        assert!(!patch.contains("-one"));
        assert!(!patch.contains("+two"));
    }
    f.git(&[
        "update-index",
        "--cacheinfo",
        "160000",
        &first,
        "dependency",
    ]);
    let changes = repo.local_changes(&token).unwrap();
    let staged = changes
        .iter()
        .find(|c| c.area == Area::Staged && c.path == std::path::Path::new("dependency"))
        .unwrap();
    let patch = repo.patch(None, staged, &token).unwrap();
    assert!(patch.contains(&format!("-Subproject commit {second}")));
    assert!(patch.contains(&format!("+Subproject commit {first}")));
}

#[test]
fn shallow_boundary_is_not_an_empty_tree_comparison() {
    let source = Fixture::new();
    source.write("a", "root\n");
    source.commit("root");
    source.write("a", "middle\n");
    source.commit("middle");
    source.write("a", "tip\n");
    source.commit("tip");
    let clone = source.0.join("shallow-clone");
    source.git(&[
        "clone",
        "--no-local",
        "--depth=2",
        source.0.to_str().unwrap(),
        clone.to_str().unwrap(),
    ]);
    let repo = Repository::new(clone.clone());
    let token = Cancel::default();
    let snapshot = repo.snapshot(None, 20, None, &token).unwrap();
    assert!(snapshot.shallow);
    assert_eq!(snapshot.commits.len(), 2);
    assert!(!snapshot.commits[0].shallow_boundary);
    let boundary = &snapshot.commits[1];
    assert!(boundary.shallow_boundary);
    assert!(boundary.parents.is_empty());
    let details = repo.details(boundary, &token).unwrap();
    assert!(details.base.contains("Unavailable"));
    assert!(details.files.is_empty());
    assert!(details.message.contains("middle"));
    let change = Change {
        path: "a".into(),
        old_path: None,
        status: "M".into(),
        area: Area::Commit,
    };
    assert!(
        repo.patch(Some(boundary), &change, &token)
            .unwrap_err()
            .contains("shallow boundary")
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args(["fetch", "--unshallow"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let complete = repo.snapshot(None, 20, None, &token).unwrap();
    assert!(!complete.shallow);
    assert!(complete.commits.iter().all(|c| !c.shallow_boundary));
    let root = complete.commits.last().unwrap();
    assert!(root.parents.is_empty());
    let root_details = repo.details(root, &token).unwrap();
    assert_eq!(root_details.base, "Empty tree");
    assert_eq!(root_details.files.len(), 1);
}

#[test]
fn message_filter_queries_history_before_paging_and_keeps_real_parents() {
    let f = Fixture::new();
    for message in [
        "old Needle.*",
        "hidden parent",
        "recent NEEDLE.*",
        "new unrelated",
    ] {
        f.git(&["commit", "--allow-empty", "-m", message]);
    }
    let repo = f.repo();
    let token = Cancel::default();
    let all = repo.snapshot(None, 10, None, &token).unwrap();
    let filter = LogFilter {
        text: "needle.*".into(),
        ..Default::default()
    };
    let first = repo
        .snapshot_filtered_at(None, 0, 1, None, Some(&filter), &token)
        .unwrap();
    assert_eq!(first.commits[0].id, all.commits[1].id);
    assert!(first.more);
    // A matching row still points to the hidden parent, not to the next match.
    assert_eq!(first.commits[0].parents, vec![all.commits[2].id.clone()]);
    let second = repo
        .snapshot_filtered_at(None, 1, 1, Some(&first.tips), Some(&filter), &token)
        .unwrap();
    assert_eq!(second.commits[0].id, all.commits[3].id);
    assert!(!second.more);
    let exact = LogFilter {
        text: "Needle.*".into(),
        match_case: true,
        regex: false,
        ..Default::default()
    };
    assert_eq!(
        repo.snapshot_filtered_at(None, 0, 10, None, Some(&exact), &token)
            .unwrap()
            .commits
            .len(),
        1
    );
    let regex = LogFilter {
        text: "^(old|recent) ".into(),
        regex: true,
        match_case: true,
        ..Default::default()
    };
    assert_eq!(
        repo.snapshot_filtered_at(None, 0, 10, None, Some(&regex), &token)
            .unwrap()
            .commits
            .len(),
        2
    );
    let invalid = LogFilter {
        text: "[".into(),
        regex: true,
        match_case: false,
        ..Default::default()
    };
    assert!(
        repo.snapshot_filtered_at(None, 0, 10, None, Some(&invalid), &token)
            .is_err()
    );
    let whitespace = LogFilter {
        text: "  ".into(),
        ..Default::default()
    };
    assert_eq!(
        repo.snapshot_filtered_at(None, 0, 10, None, Some(&whitespace), &token)
            .unwrap()
            .commits
            .len(),
        4
    );
    // Option-looking input is a grep value, never another Git argument.
    let option = LogFilter {
        text: "--all".into(),
        ..Default::default()
    };
    assert!(
        repo.snapshot_filtered_at(None, 0, 10, None, Some(&option), &token)
            .unwrap()
            .commits
            .is_empty()
    );
}

#[test]
fn filtered_graph_windows_keep_real_diff_base_and_project_hidden_parent() {
    use plico::filtered_graph;
    use std::collections::HashSet;
    let f = Fixture::new();
    f.write("review.txt", "root\n");
    f.commit("match root");
    f.write("review.txt", "hidden\n");
    f.commit("hidden middle");
    f.write("review.txt", "final\n");
    f.commit("match tip");
    let repo = f.repo();
    let token = Cancel::default();
    let all = repo.snapshot(None, 100, None, &token).unwrap();
    let filter = LogFilter {
        text: "match".into(),
        ..Default::default()
    };
    let found = repo
        .snapshot_filtered_at(None, 0, 100, Some(&all.tips), Some(&filter), &token)
        .unwrap();
    let loaded =
        plico::history::read_filtered(&repo, None, 0, 1, Some(&all.tips), &filter, &token)
            .unwrap();
    assert_eq!(loaded.snapshot.commits[0].id, all.commits[0].id);
    assert!(loaded.snapshot.more);
    assert!(loaded.rows[0].segments.iter().any(|edge| edge.dotted()));
    let older =
        plico::history::read_filtered(&repo, None, 1, 1, Some(&all.tips), &filter, &token)
            .unwrap();
    assert_eq!(older.snapshot.commits[0].id, all.commits[2].id);
    assert!(!older.snapshot.more);
    let matching: HashSet<_> = found.commits.iter().map(|c| c.id.clone()).collect();
    let whole = filtered_graph::window(&all.commits, &matching, 0, 100, 100, 100).unwrap();
    assert_eq!(whole.indices, vec![0, 2]);
    assert!(
        whole
            .rows
            .iter()
            .flat_map(|r| r.segments.iter())
            .all(|e| e.dotted())
    );
    let first = filtered_graph::window(&all.commits, &matching, 0, 1, 100, 100).unwrap();
    let second = filtered_graph::window(&all.commits, &matching, 1, 1, 100, 100).unwrap();
    assert!(first.more);
    assert!(!second.more);
    assert_eq!(
        format!("{:?}", whole.rows),
        format!("{:?}", [first.rows, second.rows].concat())
    );
    let selected = &all.commits[whole.indices[0]];
    let details = repo.details(selected, &token).unwrap();
    assert_eq!(details.base, all.commits[1].id);
    let patch = repo
        .patch(Some(selected), &details.files[0], &token)
        .unwrap();
    assert!(patch.contains("-hidden"));
    assert!(patch.contains("+final"));
    assert!(!patch.contains("-root"));
    assert!(
        filtered_graph::window(
            &all.commits[..2],
            &HashSet::from([all.commits[0].id.clone()]),
            0,
            10,
            100,
            100
        )
        .is_err()
    );
    assert!(
        filtered_graph::window(
            &all.commits,
            &HashSet::from(["missing".into()]),
            0,
            10,
            100,
            100
        )
        .is_err()
    );
    assert!(filtered_graph::window(&all.commits, &matching, 0, 10, 100, 0).is_err());
}

#[test]
fn graph_only_pages_match_history_and_keep_fixed_roots() {
    let f = Fixture::new();
    f.write("base.txt", "base");
    f.commit("base");
    f.git(&["checkout", "-b", "side"]);
    f.write("side.txt", "side");
    f.commit("side");
    f.git(&["checkout", "main"]);
    f.write("main.txt", "main");
    f.commit("main");
    f.git(&["merge", "--no-ff", "side", "-m", "merge"]);
    let repo = f.repo();
    let token = Cancel::default();
    let snapshot = repo.snapshot(None, 100, None, &token).unwrap();
    f.git(&[
        "commit",
        "--allow-empty",
        "-m",
        "created after fixed snapshot",
    ]);
    let mut actual = Vec::new();
    loop {
        let page = repo
            .topology_page(&snapshot.tips, actual.len(), 2, &token)
            .unwrap();
        actual.extend(page.commits);
        if !page.more {
            break;
        }
    }
    let expected: Vec<_> = snapshot
        .commits
        .iter()
        .map(|c| TopologyCommit {
            id: c.id.clone(),
            parents: c.parents.clone(),
        })
        .collect();
    assert_eq!(actual, expected);
    let matches = snapshot.commits.iter().map(|c| c.id.clone()).collect();
    let full = plico::filtered_graph::window(&snapshot.commits, &matches, 0, 100, 100, 100)
        .unwrap();
    let compact =
        plico::filtered_graph::window(&actual, &matches, 0, 100, 100, 100).unwrap();
    assert_eq!(full.indices, compact.indices);
    assert_eq!(format!("{:?}", full.rows), format!("{:?}", compact.rows));
    assert_eq!(actual[0].parents.len(), 2);
    assert!(
        repo.topology_page(&snapshot.tips, actual.len(), 2, &token)
            .unwrap()
            .commits
            .is_empty()
    );
    assert!(
        repo.topology_page(&[], 0, 2, &token)
            .unwrap()
            .commits
            .is_empty()
    );
    assert!(repo.topology_page(&["--all".into()], 0, 2, &token).is_err());
    token.cancel();
    assert!(repo.topology_page(&snapshot.tips, 0, 2, &token).is_err());
}

#[test]
fn unified_hash_query_overrides_branch_and_falls_back_to_messages() {
    use plico::{git::LogFilter, history};
    let f = Fixture::new();
    f.write("a", "root\n"); f.commit("root");
    let root = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "-b", "side"]);
    f.write("a", "side\n"); f.commit("side");
    let side = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.write("a", "main\n"); f.commit("mentions deadbee in message");
    let main = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo(); let token = Cancel::default();
    let filter = |text: String| LogFilter { text, ..Default::default() };
    let query = filter(side[..8].to_uppercase());
    let result = history::read_filtered(&repo, Some("refs/heads/main"), 0, 1, None, &query, &token).unwrap();
    assert_eq!(result.snapshot.commits[0].id, side);
    assert_eq!(result.snapshot.commits[0].parents, vec![root.clone()]);
    assert!(!result.snapshot.more);
    assert!(!result.snapshot.commits[0].refs.is_empty());
    let query = filter(format!("{}; {}, {}", &side[..8], &root[..8], &side[..8]));
    let all = history::read_filtered(&repo, None, 0, 10, None, &query, &token).unwrap();
    assert_eq!(all.snapshot.commits.len(), 2);
    let first = history::read_filtered(&repo, None, 0, 1, None, &query, &token).unwrap();
    assert!(first.snapshot.more);
    let second = history::read_filtered(&repo, None, 1, 1, Some(&first.snapshot.tips), &query, &token).unwrap();
    assert_eq!(second.snapshot.commits[0].id, root);
    assert!(!second.snapshot.more);
    let query = filter("deadbee".into());
    let fallback = history::read_filtered(&repo, Some("refs/heads/main"), 0, 10, None, &query, &token).unwrap();
    assert_eq!(fallback.snapshot.commits[0].id, main);
    for text in ["abc123", "--all", "abcdef0 words", "abcdefg", " , ; "] {
        assert!(filter(text.into()).hashes().is_none());
    }
    assert_eq!(filter(" ABCDEF0,1234567; 7654321\n".into()).hashes().unwrap(), vec!["abcdef0", "1234567", "7654321"]);
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(history::read_filtered(&repo, None, 0, 10, None, &query, &cancelled).is_err());
}

#[test]
fn head_filter_tracks_detached_head_and_pins_paging_roots() {
    let f = Fixture::new(); let repo = f.repo(); let token = Cancel::default();
    assert!(repo.snapshot(Some("HEAD"), 10, None, &token).unwrap().commits.is_empty());
    f.write("a", "root"); f.commit("root");
    let root = f.git(&["rev-parse", "HEAD"]);
    f.write("a", "second"); f.commit("second");
    let second = f.git(&["rev-parse", "HEAD"]);
    let initial = repo.snapshot(Some("HEAD"), 1, None, &token).unwrap();
    assert_eq!(initial.head_id.as_deref(), Some(second.as_str()));
    assert_eq!(initial.tips, vec![second.clone()]);
    f.write("a", "third"); f.commit("third");
    let page = repo.snapshot_at(Some("HEAD"), 1, 1, Some(&initial.tips), &token).unwrap();
    assert_eq!(page.commits[0].id, root);
    f.git(&["checkout", "--detach", &second]);
    let detached = repo.snapshot(Some("HEAD"), 10, None, &token).unwrap();
    assert!(detached.current_branch.is_none());
    assert_eq!(detached.head_id.as_deref(), Some(second.as_str()));
    assert_eq!(detached.commits.len(), 2);
    assert_eq!(repo.snapshot(None, 10, None, &token).unwrap().commits.len(), 3);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), second);
}

#[test]
fn head_membership_includes_merged_sides_but_not_unmerged_branches() {
    let f = Fixture::new(); let repo = f.repo(); let t = Cancel::default();
    f.write("base", "root"); f.commit("root");
    let root = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "-b", "merged"]);
    f.write("side", "side"); f.commit("merged side");
    let side = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.write("main", "main"); f.commit("main");
    f.git(&["merge", "--no-ff", "merged", "-m", "merge"]);
    let head = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "-b", "unmerged", &root]);
    f.write("other", "other"); f.commit("unmerged side");
    let other = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    let all = repo.snapshot(None, 100, None, &t).unwrap();
    for id in [&root, &side, &head] { assert!(all.included_in_head.contains(id)); }
    assert!(!all.included_in_head.contains(&other));
    let filtered = repo.snapshot(Some("refs/heads/unmerged"), 100, None, &t).unwrap();
    assert!(filtered.included_in_head.contains(&root));
    assert!(!filtered.included_in_head.contains(&other));
    f.git(&["checkout", "--detach", &side]);
    let detached = repo.snapshot(None, 100, None, &t).unwrap();
    assert!(detached.included_in_head.contains(&side));
    assert!(detached.included_in_head.contains(&root));
    assert!(!detached.included_in_head.contains(&head));
    assert!(!detached.included_in_head.contains(&other));
    let one = repo.snapshot(None, 1, None, &t).unwrap();
    assert!(one.included_in_head.len() <= 1);
}

#[test]
fn captured_diff_target_keeps_revision_after_head_moves() {
    use plico::diff_target::Target;
    let f = Fixture::new(); let repo = f.repo(); let token = Cancel::default();
    f.write("a", "before\n"); f.commit("before");
    f.write("a", "reviewed\n"); f.commit("reviewed");
    let selected = repo.snapshot(None, 10, None, &token).unwrap().commits[0].clone();
    let change = repo.details(&selected, &token).unwrap().files[0].clone();
    let tab = Target::new(repo.clone(), Some(selected.clone()), change.clone()).unwrap();
    let same = Target::new(repo.clone(), Some(selected), change).unwrap();
    assert_eq!(tab.identity(), same.identity());
    f.write("a", "later\n"); f.commit("later");
    let later = repo.snapshot(None, 10, None, &token).unwrap().commits[0].clone();
    let later_change = repo.details(&later, &token).unwrap().files[0].clone();
    let next = Target::new(repo.clone(), Some(later), later_change.clone()).unwrap();
    assert_ne!(tab.identity(), next.identity());
    let patch = tab.read(&token).unwrap();
    assert!(patch.contains("-before"));
    assert!(patch.contains("+reviewed"));
    assert!(!patch.contains("+later"));
    assert!(next.read(&token).unwrap().contains("+later"));
    assert!(Target::new(repo.clone(), None, later_change).is_err());
    let local = plico::git::Change { path: "a".into(), old_path: None, status: "M".into(), area: Area::Unstaged };
    let unstaged = Target::new(repo.clone(), None, local.clone()).unwrap();
    let staged = Target::new(repo, None, plico::git::Change { area: Area::Staged, ..local }).unwrap();
    assert_ne!(unstaged.identity(), staged.identity());
}

#[test]
fn diff_target_includes_complete_file_in_preview_and_tab() {
    use plico::diff_target::Target;
    let f = Fixture::new(); let repo = f.repo(); let t = Cancel::default();
    let before = (0..300).map(|i| format!("line-{i:03}\n")).collect::<String>();
    f.write("long.txt", &before); f.commit("base");
    f.write("long.txt", &before.replace("line-150", "changed-150")); f.commit("middle only");
    let commit = repo.snapshot(None, 1, None, &t).unwrap().commits.remove(0);
    let change = repo.details(&commit, &t).unwrap().files.remove(0);
    let target = Target::new(repo, Some(commit), change).unwrap();
    let preview = target.read(&t).unwrap();
    assert!(preview.contains(" line-000"), "embedded preview dropped the beginning of the file");
    assert!(preview.contains(" line-299"), "embedded preview dropped the end of the file");
    let full = target.read_full(&t).unwrap();
    assert!(full.contains(" line-000")); assert!(full.contains(" line-299"));
    assert!(full.contains("-line-150")); assert!(full.contains("+changed-150"));
    assert!(full.contains("@@ -1,300 +1,300 @@"));
    assert_eq!(preview, full);
}

#[test]
fn full_diff_local_versions_keep_index_and_worktree_separate() {
    use plico::diff_target::Target;
    let f = Fixture::new();
    let repo = f.repo();
    let token = Cancel::default();
    let base = (1..=300).map(|i| format!("line-{i:03}\n")).collect::<String>();
    f.write("review.txt", &base);
    f.commit("base");
    let staged = base.replace("line-150", "staged-150");
    f.write("review.txt", &staged);
    f.git(&["add", "review.txt"]);
    f.write("review.txt", &staged.replace("line-250", "working-250"));
    let changes = repo.local_changes(&token).unwrap();
    for area in [Area::Staged, Area::Unstaged] {
        let change = changes.iter().find(|c| c.area == area).unwrap().clone();
        let full = Target::new(repo.clone(), None, change).unwrap().read_full(&token).unwrap();
        assert!(full.contains("@@ -1,300 +1,300 @@"));
        assert!(full.contains(" line-001\n"));
        assert!(full.contains(" line-300\n"));
        if area == Area::Staged {
            assert!(full.contains("-line-150\n+staged-150\n"));
            assert!(full.contains(" line-250\n"));
            assert!(!full.contains("working-250"));
        } else {
            assert!(full.contains(" staged-150\n"));
            assert!(full.contains("-line-250\n+working-250\n"));
            assert!(!full.contains("-line-150"));
        }
    }
}

#[test]
fn full_diff_metadata_rename_retains_text_and_missing_newline() {
    let f = Fixture::new();
    f.write("old name.txt", "first\nlast"); f.commit("base");
    f.git(&["mv", "old name.txt", "新 name.txt"]); f.git(&["commit", "-m", "rename"]);
    let repo = f.repo(); let token = Cancel::default();
    let commit = repo.snapshot(None, 1, None, &token).unwrap().commits.remove(0);
    let change = repo.details(&commit, &token).unwrap().files.remove(0);
    let patch = repo.full_patch(Some(&commit), &change, &token).unwrap();
    assert!(patch.contains("similarity index 100%"));
    assert!(patch.contains("@@ -1,2 +1,2 @@\n first\n last\n\\ No newline at end of file\n"));
    assert!(patch.contains("--- \"a/old name.txt\""));
    // A later working-tree edit must not leak into the committed rename review.
    f.write("新 name.txt", "unrelated later edit");
    assert_eq!(repo.full_patch(Some(&commit), &change, &token).unwrap(), patch);
}

#[test]
fn diff_chain_keeps_revision_order_and_stops_at_boundaries() {
    use plico::diff_target::{Chain, Target};
    let f = Fixture::new();
    f.write("a.txt", "before a\n"); f.write("b.txt", "before b\n"); f.commit("base");
    f.write("a.txt", "review a\n"); f.write("b.txt", "review b\n"); f.commit("review");
    let repo = f.repo(); let token = Cancel::default();
    let commit = repo.snapshot(None, 1, None, &token).unwrap().commits.remove(0);
    let files = repo.details(&commit, &token).unwrap().files;
    let target = Target::new(repo, Some(commit), files[0].clone()).unwrap();
    let mut chain = Chain::new(&target, &files).unwrap();
    assert_eq!(chain.len(), 2); assert_eq!(chain.index(), 0);
    assert!(chain.step(false).is_none());
    f.write("b.txt", "later b\n"); f.commit("later");
    let next = chain.step(true).unwrap();
    assert_eq!(next.identity().revision, target.identity().revision);
    assert_eq!(next.change().path, files[1].path);
    assert!(next.read_full(&token).unwrap().contains("+review b"));
    assert!(chain.step(true).is_none()); assert_eq!(chain.index(), 1);
    assert_eq!(chain.step(false).unwrap().identity(), target.identity());
    assert!(Chain::new(&target, &[]).is_err());
}

#[test]
fn date_filter_uses_commit_time_combines_text_and_keeps_paging() {
    let f = Fixture::new();
    for (year, message) in [(2020, "old needle"), (2021, "middle needle"), (2022, "new needle")] {
        let output = Command::new("git").arg("-C").arg(&f.0)
            .args(["commit", "--allow-empty", "-m", message])
            .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00+00:00")
            .env("GIT_COMMITTER_DATE", format!("{year}-01-01T00:00:00+00:00"))
            .output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }
    let repo = f.repo(); let token = Cancel::default();
    let original_head = f.git(&["rev-parse", "HEAD"]);
    let filter = LogFilter { after: Some(1609459199), before: Some(1609459201), ..Default::default() };
    let found = plico::history::read_filtered(&repo, None, 0, 10, None, &filter, &token).unwrap();
    assert_eq!(found.snapshot.commits.len(), 1);
    assert_eq!(found.snapshot.commits[0].subject, "middle needle");
    let excluded_text = LogFilter { text: "new".into(), ..filter.clone() };
    assert!(repo.snapshot_filtered_at(None, 0, 10, None, Some(&excluded_text), &token).unwrap().commits.is_empty());
    let since = LogFilter { after: Some(1577836801), ..Default::default() };
    let first = plico::history::read_filtered(&repo, None, 0, 1, None, &since, &token).unwrap();
    assert_eq!(first.snapshot.commits[0].subject, "new needle"); assert!(first.snapshot.more);
    let second = plico::history::read_filtered(&repo, None, 1, 1, Some(&first.snapshot.tips), &since, &token).unwrap();
    assert_eq!(second.snapshot.commits[0].subject, "middle needle"); assert!(!second.snapshot.more);
    let all = repo.snapshot(None, 10, None, &token).unwrap();
    let hash = LogFilter { text: all.commits[2].id[..8].into(), ..filter };
    let by_hash = plico::history::read_filtered(&repo, None, 0, 10, None, &hash, &token).unwrap();
    assert_eq!(by_hash.snapshot.commits[0].subject, "old needle");
    assert_eq!(f.git(&["rev-parse", "HEAD"]), original_head);
}

#[test]
fn author_filter_uses_raw_identities_and_composes_before_paging() {
    use plico::history;
    let f = Fixture::new();
    let repo = f.repo();
    let token = Cancel::default();
    let alice = Author { name: r"Review.+[x](y){z}|^$\-".into(), email: "review+one@example.invalid".into() };
    let bob = Author { name: "Other Developer".into(), email: "other@example.invalid".into() };
    let impostor = Author { name: format!("Not {}", alice.name), email: alice.email.clone() };
    let commit = |author: &Author, message: &str| {
        f.git(&["commit", "--allow-empty", "--author", &author.exact_name(), "-m", message]);
        f.git(&["rev-parse", "HEAD"])
    };
    let old = commit(&alice, "Needle old");
    let other = commit(&bob, "Needle other");
    let new = commit(&alice, "needle new");
    let prefixed = commit(&impostor, "Needle impostor");
    f.write(".mailmap", &format!("Mapped Person <mapped@example.invalid> {}\n", alice.exact_name()));
    f.commit("mailmap");
    f.git(&["config", "log.mailmap", "true"]);
    let head = f.git(&["rev-parse", "HEAD"]);
    let index_before = fs::read(f.0.join(".git/index")).unwrap();
    let filter = LogFilter { authors: Some(vec![alice.clone()]), ..Default::default() };
    let first = repo.snapshot_filtered_at(None, 0, 1, None, Some(&filter), &token).unwrap();
    assert_eq!(first.commits[0].id, new);
    assert_eq!(first.commits[0].author, alice.name);
    assert!(first.more);
    let second = repo.snapshot_filtered_at(None, 1, 1, Some(&first.tips), Some(&filter), &token).unwrap();
    assert_eq!(second.commits[0].id, old);
    assert!(!second.more);
    let graph = repo.topology_filtered_page(&first.tips, 0, 10, Some(&filter), &token).unwrap();
    assert_eq!(graph.commits.iter().map(|c| &c.id).collect::<Vec<_>>(), vec![&new, &old]);
    let union = LogFilter { authors: Some(vec![alice.clone(), bob.clone()]), ..Default::default() };
    let projected = history::read_filtered(&repo, None, 0, 10, None, &union, &token).unwrap();
    assert_eq!(projected.snapshot.commits.iter().map(|c| &c.id).collect::<Vec<_>>(), vec![&new, &other, &old]);
    // Git's shared case/regexp flags apply to text and author, like GitLogProvider.
    for regex in [false, true] {
        let text = LogFilter { text: "Needle".into(), regex, match_case: true,
            authors: Some(vec![alice.clone(), bob.clone()]), ..Default::default() };
        let rows = repo.snapshot_filtered_at(None, 0, 10, None, Some(&text), &token).unwrap();
        // Fixed-string mode in the reference uses substring matching for the
        // full identity; regex mode additionally anchors it at both ends.
        let expected = if regex { vec![&other, &old] } else { vec![&prefixed, &other, &old] };
        assert_eq!(rows.commits.iter().map(|c| &c.id).collect::<Vec<_>>(), expected);
        let insensitive = LogFilter { match_case: false, ..text };
        assert_eq!(repo.snapshot_filtered_at(None, 0, 10, None, Some(&insensitive), &token).unwrap().commits.len(), if regex { 3 } else { 4 });
    }
    let empty = LogFilter { authors: Some(vec![]), ..Default::default() };
    assert!(history::read_filtered(&repo, None, 0, 10, None, &empty, &token).unwrap().rows.is_empty());
    assert!(repo.topology_filtered_page(&first.tips, 0, 10, Some(&empty), &token).unwrap().commits.is_empty());
    let future = LogFilter { after: Some(4102444800), ..filter };
    assert!(repo.snapshot_filtered_at(None, 0, 10, None, Some(&future), &token).unwrap().commits.is_empty());
    let hash = LogFilter { text: old[..8].into(), ..empty };
    assert_eq!(history::read_filtered(&repo, None, 0, 10, None, &hash, &token).unwrap().snapshot.commits[0].id, old);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index_before);
}

#[test]
fn current_author_uses_effective_config_and_keeps_missing_values_distinct_from_errors() {
    let f = Fixture::new();
    f.git(&["commit", "--allow-empty", "-m", "base"]);
    let repo = f.repo();
    let token = Cancel::default();
    assert_eq!(repo.current_author(&token).unwrap(), Some(Author {
        name: "测试 Developer".into(), email: "test@example.invalid".into(),
    }));
    // Included configuration is read by Git, with its normal precedence.
    fs::write(f.0.join(".git/identity-config"), "[user]\nname = Included Developer\nemail = included@example.invalid\n").unwrap();
    f.git(&["config", "include.path", "identity-config"]);
    let head = f.git(&["rev-parse", "HEAD"]);
    let config = fs::read(f.0.join(".git/config")).unwrap();
    assert_eq!(repo.current_author(&token).unwrap(), Some(Author {
        name: "Included Developer".into(), email: "included@example.invalid".into(),
    }));
    assert_eq!(fs::read(f.0.join(".git/config")).unwrap(), config);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    fs::write(f.0.join(".git/identity-config"), "[user]\nname = Included Developer\nemail =\n").unwrap();
    assert_eq!(repo.current_author(&token).unwrap().unwrap().email, "");
    fs::write(f.0.join(".git/identity-config"), "[user]\nname =\n").unwrap();
    assert!(repo.current_author(&token).unwrap().is_none());
    fs::write(f.0.join(".git/identity-config"), "[broken\n").unwrap();
    assert!(repo.current_author(&token).is_err());
    let cancelled = Cancel::default(); cancelled.cancel();
    assert_eq!(repo.current_author(&cancelled).unwrap_err(), "Cancelled");
}

#[test]
fn author_catalog_streams_all_roots_preserves_raw_names_and_pins_history() {
    let f = Fixture::new();
    let repo = f.repo();
    let token = Cancel::default();
    assert!(repo.author_catalog(&[], &token).unwrap().users().is_empty());
    let alice = Author { name: "Alice Review".into(), email: "Alice@example.invalid".into() };
    let bob = Author { name: "另一位开发者".into(), email: "bob@example.invalid".into() };
    f.git(&["commit", "--allow-empty", "--author", &alice.exact_name(), "-m", "base"]);
    f.git(&["checkout", "-b", "other"]);
    f.git(&["commit", "--allow-empty", "--author", &bob.exact_name(), "-m", "other"]);
    f.git(&["checkout", "main"]);
    f.write(".mailmap", "Mapped Author <mapped@example.invalid> Alice Review <Alice@example.invalid>\n");
    f.git(&["add", ".mailmap"]);
    f.git(&["commit", "--author", &alice.exact_name(), "-m", "mailmap"]);
    f.git(&["config", "log.mailmap", "true"]);
    let snapshot = repo.snapshot(None, 1, None, &token).unwrap();
    assert_eq!(snapshot.commits.len(), 1);
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    let directory = plico::user_filter::Directory::read(&repo, &token).unwrap();
    assert_eq!(directory.roots, snapshot.tips);
    assert_eq!(directory.current, repo.current_author(&token).unwrap());
    let selection = plico::user_filter::Selection::parse("Alice Chen|另一位开发者").unwrap();
    assert_eq!(selection.resolve(&directory), Some(vec![bob.clone()]));
    let catalog = directory.catalog;
    assert_eq!(catalog.users().len(), 2);
    assert!(catalog.users().contains(&alice));
    assert!(catalog.users().contains(&bob));
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    f.git(&["commit", "--allow-empty", "--author", "Later User <later@example.invalid>", "-m", "later"]);
    assert_eq!(repo.author_catalog(&snapshot.tips, &token).unwrap().users(), catalog.users());
    let current = repo.snapshot(None, 1, None, &token).unwrap();
    assert_eq!(repo.author_catalog(&current.tips, &token).unwrap().users().len(), 3);
    assert!(repo.author_catalog(&["--all".into()], &token).is_err());
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(matches!(repo.author_catalog(&snapshot.tips, &cancelled), Err(e) if e == "Cancelled"));
}

#[test]
fn path_filter_keeps_literal_names_rename_semantics_and_real_diff_parents() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("src")).unwrap();
    f.write("src/[x].txt", "before\n");
    f.write("src/x.txt", "other\n");
    f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.write("src/x.txt", "unrelated\n"); f.commit("hidden literal sibling");
    let hidden = f.git(&["rev-parse", "HEAD"]);
    f.write("src/[x].txt", "after\n"); f.commit("Needle review");
    let review = f.git(&["rev-parse", "HEAD"]);
    f.git(&["mv", "src/[x].txt", "src/renamed.txt"]); f.commit("rename");
    let rename = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo(); let token = Cancel::default();
    let filter = LogFilter { paths: vec!["src/[x].txt".into()], ..Default::default() };
    let window = plico::history::read_filtered(&repo, None, 0, 10, None, &filter, &token).unwrap();
    assert_eq!(window.snapshot.commits.iter().map(|c| &c.id).collect::<Vec<_>>(), vec![&rename, &review, &base]);
    let commit = &window.snapshot.commits[1];
    assert_eq!(commit.parents, vec![hidden.clone()]);
    let change = repo.details(commit, &token).unwrap().files.remove(0);
    let patch = repo.patch(Some(commit), &change, &token).unwrap();
    assert!(patch.contains("-before\n+after\n"));
    let direct = repo.snapshot_filtered_at(None, 0, 10, None, Some(&filter), &token).unwrap();
    assert_eq!(direct.commits[1].parents, vec![hidden]);
    let new_name = LogFilter { paths: vec!["src/renamed.txt".into()], ..Default::default() };
    let renamed = plico::history::read_filtered(&repo, None, 0, 10, None, &new_name, &token).unwrap();
    // Log structure filtering does not implicitly follow a new name backwards.
    assert_eq!(renamed.snapshot.commits.len(), 1);
    assert_eq!(renamed.snapshot.commits[0].id, rename);
    let directory = LogFilter { paths: vec!["src".into()], ..Default::default() };
    assert_eq!(plico::history::read_filtered(&repo, None, 0, 10, None, &directory, &token).unwrap().snapshot.commits.len(), 4);
    let union = LogFilter { paths: vec!["src/[x].txt".into(), "src/x.txt".into()], ..Default::default() };
    assert_eq!(plico::history::read_filtered(&repo, None, 0, 10, None, &union, &token).unwrap().snapshot.commits.len(), 4);
    let combined = LogFilter { text: "Needle".into(), ..filter.clone() };
    let matches = plico::history::read_filtered(&repo, None, 0, 10, None, &combined, &token).unwrap();
    assert_eq!(matches.snapshot.commits.len(), 1); assert_eq!(matches.snapshot.commits[0].id, review);
    let first = plico::history::read_filtered(&repo, None, 0, 1, None, &filter, &token).unwrap();
    assert!(first.snapshot.more);
    let next = plico::history::read_filtered(&repo, None, 1, 1, Some(&first.snapshot.tips), &filter, &token).unwrap();
    assert_eq!(next.snapshot.commits[0].id, review);
    let missing = LogFilter { paths: vec!["absent".into()], ..Default::default() };
    assert!(plico::history::read_filtered(&repo, None, 0, 10, None, &missing, &token).unwrap().snapshot.commits.is_empty());
    let hash = LogFilter { text: base[..8].into(), ..missing };
    assert_eq!(plico::history::read_filtered(&repo, None, 0, 10, None, &hash, &token).unwrap().snapshot.commits[0].id, base);
}

#[test]
fn path_filter_merge_preserves_both_parents_and_first_parent_diff() {
    let f = Fixture::new();
    let body = (0..30).map(|i| format!("line-{i:02}\n")).collect::<String>();
    f.write("review.txt", &body); f.commit("base");
    f.git(&["checkout", "-b", "side"]);
    f.write("review.txt", &body.replace("line-01", "side-01")); f.commit("side");
    let side = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.write("review.txt", &body.replace("line-28", "main-28")); f.commit("main");
    let main = f.git(&["rev-parse", "HEAD"]);
    f.git(&["merge", "--no-ff", "side", "-m", "merge both edits"]);
    let merge = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo(); let token = Cancel::default();
    let filter = LogFilter { paths: vec!["review.txt".into()], ..Default::default() };
    let window = plico::history::read_filtered(&repo, None, 0, 10, None, &filter, &token).unwrap();
    let commit = &window.snapshot.commits[0];
    assert_eq!(commit.id, merge);
    assert_eq!(commit.parents, vec![main, side]);
    assert_eq!(window.snapshot.commits.len(), 4);
    for expected in &window.snapshot.commits {
        let plico::history::FilteredLocation::Found(located) = plico::history::locate_filtered(
            &repo, &window.snapshot.tips, &expected.id, 1, &filter, &token).unwrap() else { panic!("Visible path commit not found"); };
        assert_eq!(located.window.snapshot.commits[located.selected].id, expected.id);
        assert_eq!(located.window.snapshot.commits[located.selected].parents, expected.parents);
    }
    let change = repo.details(commit, &token).unwrap().files.remove(0);
    let patch = repo.patch(Some(commit), &change, &token).unwrap();
    assert!(patch.contains("-line-01\n+side-01\n"));
    assert!(!patch.contains("+main-28"));
}

#[test]
fn path_filter_rejects_outside_paths_and_excessive_command_arguments() {
    let f = Fixture::new(); f.write("file", "base"); f.commit("base");
    let repo = f.repo(); let token = Cancel::default();
    for paths in [vec!["../outside".into()], vec!["/absolute".into()], vec!["C:/outside".into()],
        vec!["file".into(); 129], vec!["x".repeat(8193)]] {
        let filter = LogFilter { paths, ..Default::default() };
        assert!(repo.snapshot_filtered_at(None, 0, 10, None, Some(&filter), &token).is_err());
    }
}

#[test]
fn path_tree_lists_live_directories_and_tracked_files_without_untracked_files() {
    let f = Fixture::new();
    fs::create_dir_all(f.0.join("src/deep")).unwrap();
    fs::create_dir_all(f.0.join("empty")).unwrap();
    f.write("tracked.txt", "base"); f.write("deleted.txt", "gone");
    f.write(".hidden", "tracked hidden"); f.write(".gitignore", "ignored.txt\n");
    f.write("src/cache.rs", "code"); f.write("src/deep/other.rs", "nested");
    f.commit("base");
    f.write("untracked.txt", "not versioned"); f.write("ignored.txt", "ignored");
    f.write("src/untracked.rs", "not versioned");
    fs::remove_file(f.0.join("deleted.txt")).unwrap();
    let repo = f.repo(); let token = Cancel::default();
    let root = plico::path_tree::children(&repo, ".", &token).unwrap();
    assert_eq!(root.iter().map(|n| (n.path.as_str(), n.directory)).collect::<Vec<_>>(),
        vec![("empty", true), ("src", true), (".gitignore", false), (".hidden", false), ("tracked.txt", false)]);
    let src = plico::path_tree::children(&repo, "src", &token).unwrap();
    assert_eq!(src.iter().map(|n| (n.path.as_str(), n.directory)).collect::<Vec<_>>(),
        vec![("src/deep", true), ("src/cache.rs", false)]);
    assert!(plico::path_tree::children(&repo, "../outside", &token).is_err());
    assert!(plico::path_tree::children(&repo, "missing", &token).is_err());
    token.cancel(); assert!(plico::path_tree::children(&repo, ".", &token).is_err());
    #[cfg(unix)] {
        std::os::unix::fs::symlink(f.0.join("src"), f.0.join("linked-dir")).unwrap();
        assert!(plico::path_tree::children(&repo, "linked-dir", &Cancel::default()).unwrap_err().contains("Linked directories"));
    }
}

#[test]
fn containing_branches_use_ancestry_not_loaded_history_or_tags() {
    let f = Fixture::new();
    f.write("file", "base\n");
    f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["branch", "feature/review"]);
    f.git(&["branch", "功能/审阅"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &base]);
    f.git(&["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
    f.git(&["tag", "release-base"]);
    f.write("file", "main only\n");
    f.commit("main tip");
    let tip = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo();
    let token = Cancel::default();
    // The selected ancestor is deliberately outside the loaded one-row window.
    let window = repo.snapshot(None, 1, None, &token).unwrap();
    assert_eq!(window.commits.len(), 1);
    assert_eq!(window.commits[0].id, tip);
    assert_eq!(repo.containing_branches(&base, &token).unwrap(),
        ["HEAD", "main", "origin/main", "feature/review", "功能/审阅"]);
    assert_eq!(repo.containing_branches(&tip, &token).unwrap(), ["HEAD", "main"]);
    // A side commit is absent from HEAD until a merge includes its parent chain.
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let side = f.git(&["commit-tree", &tree, "-p", &base, "-m", "side work"]);
    f.git(&["update-ref", "refs/heads/feature/review", &side]);
    assert_eq!(repo.containing_branches(&side, &token).unwrap(), ["feature/review"]);
    let merge = f.git(&["commit-tree", &tree, "-p", &tip, "-p", &side, "-m", "merge side"]);
    f.git(&["update-ref", "refs/heads/main", &merge]);
    assert_eq!(repo.containing_branches(&side, &token).unwrap(), ["HEAD", "main", "feature/review"]);
    // Detaching at an older commit must not claim HEAD contains the newer tip.
    f.git(&["checkout", "--detach", &base]);
    assert_eq!(repo.containing_branches(&tip, &token).unwrap(), ["main"]);
    assert_eq!(repo.containing_branches(&base, &token).unwrap(),
        ["HEAD", "main", "origin/main", "feature/review", "功能/审阅"]);
    // A valid, unreferenced commit is distinct from a failed query.
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let orphan = f.git(&["commit-tree", &tree, "-m", "unreferenced"]);
    assert!(repo.containing_branches(&orphan, &token).unwrap().is_empty());
    assert!(repo.containing_branches("--all", &token).is_err());
    token.cancel();
    assert_eq!(repo.containing_branches(&base, &token).unwrap_err(), "Cancelled");
}

#[test]
fn tag_tree_preserves_peeled_targets_outside_loaded_history() {
    let f = Fixture::new();
    f.write("review.txt", "old\n"); f.commit("old");
    let old = f.git(&["rev-parse", "HEAD"]);
    f.git(&["tag", "release/v2"]);
    f.git(&["tag", "-a", "release/v10", "-m", "annotated"]);
    f.git(&["tag", "-a", "nested", "-m", "nested", "release/v10"]);
    f.git(&["tag", "blob-tag", "HEAD:review.txt"]);
    f.write("review.txt", "new\n"); f.commit("new");
    let repo = f.repo(); let token = Cancel::default();
    let snapshot = repo.snapshot(None, 1, None, &token).unwrap();
    assert_eq!(snapshot.commits.len(), 1);
    assert_ne!(snapshot.commits[0].id, old.trim());
    assert_eq!(snapshot.tags.len(), 3);
    assert!(snapshot.tags.iter().all(|tag| tag.id == old.trim()));
    assert!(snapshot.branches.iter().all(|branch| !branch.name.starts_with("refs/tags/")));
    let names: Vec<_> = snapshot.tags.iter().map(|tag| tag.name.as_str()).collect();
    assert_eq!(names, ["refs/tags/nested", "refs/tags/release/v10", "refs/tags/release/v2"]);
    assert!(repo.snapshot(Some("refs/tags/blob-tag"), 10, None, &token).is_err());
    assert!(repo.snapshot(Some("refs/tags/missing"), 10, None, &token).is_err());
}

#[test]
fn commit_navigation_keeps_pinned_history_order_and_graph_window() {
    use plico::{graph, history};
    let f = Fixture::new();
    f.write("review.txt", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["tag", "old", &base]);
    f.write("review.txt", "tip\n"); f.commit("tip");
    let tip = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let side = f.git(&["commit-tree", &tree, "-p", &base, "-m", "side"]);
    let merge = f.git(&["commit-tree", &tree, "-p", &tip, "-p", &side, "-m", "merge"]);
    f.git(&["update-ref", "refs/heads/main", &merge]);
    let repo = f.repo(); let token = Cancel::default();
    let snapshot = repo.snapshot(None, 10, None, &token).unwrap();
    let roots = snapshot.tips.clone();
    let original_graph = graph::layout(&snapshot.commits);
    // Move a live branch after taking roots. Navigation must keep the old order.
    f.write("review.txt", "later\n"); f.commit("later");
    let later = f.git(&["rev-parse", "HEAD"]);
    for (index, commit) in snapshot.commits.iter().enumerate() {
        assert_eq!(repo.commit_position(&roots, &commit.id, &token).unwrap(), Some(index));
        let located = history::locate(&repo, &roots, &commit.id, 2, &token).unwrap().unwrap();
        assert_eq!(located.offset + located.selected, index);
        assert_eq!(located.window.snapshot.commits[located.selected].id, commit.id);
        assert_eq!(located.window.snapshot.commits[located.selected].parents, commit.parents);
        let geometry = |row: &graph::Row| (row.node, row.color, row.width,
            row.segments.iter().map(|edge| (edge.from(), edge.to(), edge.upper(), edge.full(), edge.dotted(), edge.color)).collect::<Vec<_>>());
        assert_eq!(geometry(&located.window.rows[located.selected]), geometry(&original_graph[index]));
    }
    assert!(history::locate(&repo, &roots, &later, 2, &token).unwrap().is_none());
    assert!(repo.commit_position(&[], &base, &token).unwrap().is_none());
    assert!(repo.commit_position(&roots, "--all", &token).is_err());
    assert!(repo.commit_position(&["HEAD".into()], &base, &token).is_err());
    token.cancel();
    assert_eq!(repo.commit_position(&roots, &base, &token).unwrap_err(), "Cancelled");
}

#[test]
fn filtered_navigation_preserves_display_order_filters_and_real_parents() {
    use plico::{graph, history::{self, FilteredLocation}};
    let f = Fixture::new();
    f.write("review.txt", "base\n"); f.commit("needle base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.write("other.txt", "unrelated\n"); f.commit("hidden");
    let hidden = f.git(&["rev-parse", "HEAD"]);
    f.write("review.txt", "tip\n"); f.commit("needle tip");
    let tip = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo(); let token = Cancel::default();
    let roots = repo.snapshot(None, 10, None, &token).unwrap().tips;
    let filters = [
        LogFilter { text: "needle".into(), ..Default::default() },
        LogFilter { paths: vec!["review.txt".into()], ..Default::default() },
        LogFilter { text: "needle (base|tip)".into(), regex: true, ..Default::default() },
        LogFilter { text: "needle".into(), authors: Some(vec![]), ..Default::default() },
        // Git falls back to the current time for out-of-range dates.
        // Use a valid future timestamp so separate queries cannot disagree at a second boundary.
        LogFilter { after: Some(chrono::Utc::now().timestamp() + 86_400), ..Default::default() },
        // Matching hashes ignore even an invalid path and empty author set.
        LogFilter { text: format!("{},{}", &base[..8], &tip[..8]), paths: vec!["../outside".into()], authors: Some(vec![]), ..Default::default() },
    ];
    f.write("review.txt", "later\n"); f.commit("needle later");
    let later = f.git(&["rev-parse", "HEAD"]);
    let geometry = |row: &graph::Row| (row.node, row.color, row.width,
        row.segments.iter().map(|edge| (edge.from(), edge.to(), edge.upper(), edge.full(), edge.dotted(), edge.color)).collect::<Vec<_>>());
    for filter in filters {
        let all = history::read_filtered(&repo, None, 0, 10, Some(&roots), &filter, &token).unwrap();
        if filter.after.is_some() {
            assert!(all.snapshot.commits.is_empty(), "future date must exclude fixture commits");
        }
        for target in [&base, &hidden, &tip] {
            match history::locate_filtered(&repo, &roots, target, 1, &filter, &token)
                .unwrap_or_else(|error| panic!("filter {filter:?}, target {target}: {error}")) {
                FilteredLocation::Found(found) => {
                    let index = all.snapshot.commits.iter().position(|c| &c.id == target).unwrap();
                    assert_eq!(found.offset + found.selected, index);
                    assert_eq!(found.window.snapshot.commits.len(), 1);
                    let actual = &found.window.snapshot.commits[found.selected];
                    assert_eq!(&actual.id, target);
                    assert_eq!(actual.parents, all.snapshot.commits[index].parents);
                    assert_eq!(geometry(&found.window.rows[found.selected]), geometry(&all.rows[index]));
                    assert_eq!(found.window.snapshot.more, index + 1 < all.snapshot.commits.len());
                }
                FilteredLocation::Hidden => assert!(!all.snapshot.commits.iter().any(|c| &c.id == target)),
                FilteredLocation::OutsideRoots => panic!("reachable target misclassified"),
            }
        }
        assert!(matches!(history::locate_filtered(&repo, &roots, &later, 1, &filter, &token).unwrap(), FilteredLocation::OutsideRoots));
    }
    let fallback = LogFilter { text: "deadbeef".into(), ..Default::default() };
    assert!(matches!(history::locate_filtered(&repo, &roots, &base, 1, &fallback, &token).unwrap(), FilteredLocation::Hidden));
    let invalid = LogFilter { text: "[".into(), regex: true, ..Default::default() };
    assert!(history::locate_filtered(&repo, &roots, &base, 1, &invalid, &token).is_err());
    assert!(history::locate_filtered(&repo, &roots, "HEAD", 1, &fallback, &token).is_err());
    assert!(history::locate_filtered(&repo, &["--all".into()], &base, 1, &fallback, &token).is_err());
    token.cancel();
    assert!(history::locate_filtered(&repo, &roots, &base, 1, &fallback, &token).is_err());
}

#[test]
fn rebased_fetch_specs_fetch_all_prune_and_preserve_worktree() {
    use plico::fetch::{Spec, Tags};
    let source = Fixture::new();
    source.write("review.txt", "one\n"); source.commit("one");
    let first = source.git(&["rev-parse", "HEAD"]);
    source.git(&["branch", "obsolete"]);
    source.git(&["tag", "v1"]);
    let consumer = Fixture::new();
    let token = Cancel::default();
    assert!(consumer.repo().fetch_specs(&token).unwrap().is_empty());
    // Local repositories as transport endpoints keep the test network-free.
    consumer.git(&["remote", "add", "origin", source.0.to_str().unwrap()]);
    consumer.git(&["remote", "add", "second", source.0.to_str().unwrap()]);
    consumer.write("local.txt", "keep staged\n"); consumer.commit("local");
    let local_head = consumer.git(&["rev-parse", "HEAD"]);
    consumer.write("local.txt", "staged\n"); consumer.git(&["add", "local.txt"]);
    consumer.write("local.txt", "unstaged\n"); consumer.write("untracked.txt", "keep\n");
    let index = consumer.git(&["write-tree"]);
    let status = consumer.git(&["status", "--porcelain"]);
    let execute = |spec: &Spec| {
        let output = consumer.repo().fetch(spec, &token, &plico::operation_process::Progress::default()).unwrap();
        assert_eq!(output.stopped, None);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr.bytes()));
    };
    let specs = consumer.repo().fetch_specs(&token).unwrap();
    assert_eq!(specs.iter().map(Spec::remote).collect::<Vec<_>>(), ["origin", "second"]);
    for spec in &specs { execute(spec); }
    assert_eq!(consumer.git(&["rev-parse", "origin/main"]), first);
    assert_eq!(consumer.git(&["rev-parse", "second/main"]), first);
    assert_eq!(consumer.git(&["rev-parse", "v1"]), first);
    source.git(&["branch", "-D", "obsolete"]);
    source.write("review.txt", "two\n"); source.commit("two");
    source.git(&["tag", "v2"]);
    let second = source.git(&["rev-parse", "HEAD"]);
    let mut specific = Spec::new("second".into()).unwrap(); specific.tags = Tags::None;
    execute(&specific);
    assert_eq!(consumer.git(&["rev-parse", "second/main"]), second);
    assert_eq!(consumer.git(&["rev-parse", "origin/main"]), first);
    assert!(consumer.git(&["for-each-ref", "--format=%(refname)", "refs/remotes/second/obsolete"]).is_empty());
    assert!(!consumer.git(&["for-each-ref", "--format=%(refname)", "refs/remotes/origin/obsolete"]).is_empty());
    assert!(consumer.git(&["tag", "--list", "v2"]).is_empty());
    assert_eq!(consumer.git(&["rev-parse", "HEAD"]), local_head);
    assert_eq!(consumer.git(&["write-tree"]), index);
    assert_eq!(consumer.git(&["status", "--porcelain"]), status);
    assert_eq!(fs::read_to_string(consumer.0.join("local.txt")).unwrap(), "unstaged\n");
    assert_eq!(fs::read_to_string(consumer.0.join("untracked.txt")).unwrap(), "keep\n");
    let cancelled = Cancel::default(); cancelled.cancel();
    assert_eq!(consumer.repo().fetch_specs(&cancelled).unwrap_err(), "Cancelled");
    assert!(consumer.repo().fetch(&specific, &cancelled, &Default::default()).is_err());
    assert!(consumer.repo().fetch(&Spec::new("missing".into()).unwrap(), &token, &Default::default()).is_err());
    consumer.git(&["remote", "add", "broken", consumer.0.join("absent.git").to_str().unwrap()]);
    let failure = consumer.repo().fetch(&Spec::new("broken".into()).unwrap(), &token, &Default::default()).unwrap();
    assert!(!failure.status.success());
    assert_eq!(failure.stopped, None);
    execute(&specific); // failed transport must release operation ownership
    assert_eq!(consumer.git(&["rev-parse", "HEAD"]), local_head);
    assert_eq!(consumer.git(&["write-tree"]), index);
    assert_eq!(consumer.git(&["status", "--porcelain"]), status);
}

#[test]
fn fetch_batch_retains_partial_success_and_continues_after_failure() {
    use plico::fetch::{self, RemoteState};
    let source = Fixture::new();
    source.write("review.txt", "remote content\n"); source.commit("remote");
    let head = source.git(&["rev-parse", "HEAD"]);
    let consumer = Fixture::new();
    consumer.git(&["remote", "add", "a-broken", consumer.0.join("missing.git").to_str().unwrap()]);
    consumer.git(&["remote", "add", "b-working", source.0.to_str().unwrap()]);
    consumer.git(&["remote", "add", "c-broken", consumer.0.join("missing-again.git").to_str().unwrap()]);
    let token = Cancel::default();
    let specs = consumer.repo().fetch_specs(&token).unwrap();
    let result = fetch::execute(&consumer.repo(), &specs, &token, &Default::default()).unwrap();
    assert!(!result.successful());
    assert_eq!(result.remotes.iter().map(|r| &r.state).collect::<Vec<_>>(),
        [&RemoteState::Failed, &RemoteState::Succeeded, &RemoteState::Failed]);
    assert_eq!(consumer.git(&["rev-parse", "b-working/main"]), head);
    assert!(!result.remotes[0].diagnostic.is_empty());
    assert!(result.remotes[1].diagnostic.is_empty());
    assert!(!result.remotes[2].diagnostic.is_empty());
    assert!(result.remotes.iter().map(|r| r.diagnostic.len()).sum::<usize>() <= 65536);
    // A pre-cancelled batch must not fetch the newly added remote commit.
    source.write("review.txt", "new remote content\n"); source.commit("new remote");
    token.cancel();
    let cancelled = fetch::execute(&consumer.repo(), &specs, &token, &Default::default()).unwrap();
    assert!(!cancelled.successful());
    assert!(cancelled.remotes.iter().all(|r| r.state == RemoteState::NotStarted));
    assert_eq!(consumer.git(&["rev-parse", "b-working/main"]), head);
}

#[test]
fn updater_default_uses_actual_current_branch_config_before_pull_config() {
    use plico::update_method::Method;
    let fixture = Fixture::new();
    let token = Cancel::default();
    fixture.git(&["config", "pull.rebase", "false"]);
    assert_eq!(fixture.repo().default_update_method(&token).unwrap(), Method::Merge);
    fixture.git(&["config", "pull.rebase", "true"]);
    assert_eq!(fixture.repo().default_update_method(&token).unwrap(), Method::Rebase);
    fixture.git(&["config", "branch.main.rebase", "false"]);
    assert_eq!(fixture.repo().default_update_method(&token).unwrap(), Method::Merge);
    fixture.git(&["config", "branch.main.rebase", "interactive"]);
    assert_eq!(fixture.repo().default_update_method(&token).unwrap(), Method::Rebase);
    fixture.write("file.txt", "content\n"); fixture.commit("initial");
    fixture.git(&["checkout", "-b", "feature/review"]);
    fixture.git(&["config", "branch.feature/review.rebase", "off"]);
    assert_eq!(fixture.repo().default_update_method(&token).unwrap(), Method::Merge);
    fixture.git(&["checkout", "--detach"]);
    assert_eq!(fixture.repo().default_update_method(&token).unwrap(), Method::Rebase);
    token.cancel();
    assert_eq!(fixture.repo().default_update_method(&token).unwrap_err(), "Cancelled");
}

#[test]
fn operation_state_tracks_merge_rebase_and_linked_worktree_conflicts() {
    use plico::operation_state::Operation;
    let fixture = Fixture::new();
    let token = Cancel::default();
    fixture.write("conflict file.txt", "base\n"); fixture.commit("base");
    fixture.git(&["checkout", "-b", "feature"]);
    fixture.write("conflict file.txt", "feature\n"); fixture.commit("feature");
    fixture.git(&["checkout", "main"]);
    fixture.write("conflict file.txt", "main\n"); fixture.commit("main");
    let linked = fixture.0.join("linked-worktree");
    fixture.git(&["worktree", "add", linked.to_str().unwrap(), "feature"]);
    let repo = Repository::new(linked.clone());
    let merge = Command::new("git").arg("-C").arg(&linked).args(["merge", "main"]).output().unwrap();
    assert!(!merge.status.success());
    let state = repo.operation_state(&token).unwrap();
    assert_eq!(state.operation, Operation::Merge);
    assert_eq!(state.conflicts, [PathBuf::from("conflict file.txt")]);
    assert!(!state.detached);
    assert!(!fixture.repo().operation_state(&token).unwrap().in_progress());
    // Resolved index entries do not mean that the merge itself has completed.
    fs::write(linked.join("conflict file.txt"), "resolved\n").unwrap();
    assert!(Command::new("git").arg("-C").arg(&linked).args(["add", "conflict file.txt"]).output().unwrap().status.success());
    let resolved = repo.operation_state(&token).unwrap();
    assert_eq!(resolved.operation, Operation::Merge);
    assert!(resolved.conflicts.is_empty());
    assert!(Command::new("git").arg("-C").arg(&linked).args(["merge", "--abort"]).output().unwrap().status.success());
    assert!(!repo.operation_state(&token).unwrap().in_progress());
    let rebase = Command::new("git").arg("-C").arg(&linked).args(["rebase", "--merge", "main"]).output().unwrap();
    assert!(!rebase.status.success());
    let state = repo.operation_state(&token).unwrap();
    assert_eq!(state.operation, Operation::RebaseMerge);
    assert!(state.detached);
    assert_eq!(state.rebase_branch.as_deref(), Some("feature"));
    assert_eq!(state.conflicts, [PathBuf::from("conflict file.txt")]);
    assert!(!fixture.repo().operation_state(&token).unwrap().in_progress());
    assert!(Command::new("git").arg("-C").arg(&linked).args(["rebase", "--abort"]).output().unwrap().status.success());
    assert!(!repo.operation_state(&token).unwrap().in_progress());
    token.cancel();
    assert_eq!(repo.operation_state(&token).unwrap_err(), "Cancelled");
}

#[test]
fn pull_targets_follow_tracking_and_do_not_guess_remote_branch() {
    let fixture = Fixture::new();
    let token = Cancel::default();
    assert!(fixture.repo().pull_targets(&token).unwrap().default_remote().is_none());
    fixture.write("review.txt", "base\n"); fixture.commit("base");
    for remote in ["company", "origin"] {
        fixture.git(&["remote", "add", remote, "https://example.invalid/repo.git"]);
    }
    // Modern `remote add` rejects overlapping names, but existing/manual config
    // can contain them; Rebased explicitly defines how these refs are grouped.
    fixture.git(&["config", "remote.company/team.url", "https://example.invalid/repo.git"]);
    fixture.git(&["config", "remote.company/team.fetch", "+refs/heads/*:refs/remotes/company/team/*"]);
    for reference in ["refs/remotes/origin/main", "refs/remotes/company/topic", "refs/remotes/company/team/nested"] {
        fixture.git(&["update-ref", reference, "HEAD"]);
    }
    fixture.git(&["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
    let initial = fixture.repo().pull_targets(&token).unwrap();
    assert_eq!(initial.default_remote(), Some("origin"));
    assert_eq!(initial.default_branch("origin", None), Some("main"));
    assert_eq!(initial.default_branch("company", None), None);
    assert!(!initial.contains("origin", "HEAD"));
    assert!(initial.contains("company", "team/nested")); // upstream first-prefix rule
    fixture.git(&["branch", "--set-upstream-to=company/topic", "main"]);
    let tracked = fixture.repo().pull_targets(&token).unwrap();
    assert_eq!(tracked.default_remote(), Some("company"));
    assert_eq!(tracked.default_branch("company", None), Some("topic"));
    assert_eq!(tracked.default_branch("origin", Some("main")), Some("main"));
    assert_eq!(tracked.default_branch("origin", None), Some("topic"));
    assert!(!tracked.contains("origin", "topic")); // invalid draft must require selection
    fixture.git(&["checkout", "--detach"]);
    let detached = fixture.repo().pull_targets(&token).unwrap();
    assert_eq!(detached.default_remote(), Some("origin"));
    assert_eq!(detached.default_branch("origin", None), None);
    fixture.git(&["remote", "remove", "origin"]);
    assert_eq!(fixture.repo().pull_targets(&token).unwrap().default_remote(), Some("company"));
    token.cancel();
    assert_eq!(fixture.repo().pull_targets(&token).unwrap_err(), "Cancelled");
}

#[test]
fn operation_events_observe_real_merge_and_stash_conflicts() {
    use plico::{operation_events::Event, operation_process::{self, Progress}};
    let f = Fixture::new();
    f.write("conflict.txt", "base\n");
    f.commit("base");
    f.git(&["checkout", "-b", "topic"]);
    f.write("conflict.txt", "topic\n");
    f.commit("topic");
    f.git(&["checkout", "main"]);
    f.write("conflict.txt", "main\n");
    f.commit("main");
    let run = |args: &[&str]| {
        let mut command = Command::new("git");
        command.arg("-C").arg(&f.0).args(args).env("LC_ALL", "C").env("LANG", "C");
        operation_process::run(&mut command, &Cancel::default(), &Progress::default(), None).unwrap()
    };
    let result = run(&["merge", "topic"]);
    assert!(!result.status.success());
    assert!(result.events.contains(Event::MergeConflict));
    assert_eq!(f.repo().operation_state(&Cancel::default()).unwrap().conflicts.len(), 1);
    f.git(&["merge", "--abort"]);
    assert_eq!(fs::read_to_string(f.0.join("conflict.txt")).unwrap(), "main\n");
    assert!(run(&["merge", "main"]).events.contains(Event::AlreadyUpToDate));
    f.write("conflict.txt", "saved local work\n");
    f.git(&["stash", "push", "-m", "fixture"]);
    let stash = f.git(&["rev-parse", "refs/stash"]);
    f.write("conflict.txt", "new main\n");
    f.commit("new main");
    let result = run(&["stash", "pop"]);
    assert!(!result.status.success());
    assert!(result.events.contains(Event::UnstashConflict));
    assert_eq!(f.git(&["rev-parse", "refs/stash"]), stash);
    assert_eq!(f.repo().operation_state(&Cancel::default()).unwrap().conflicts.len(), 1);
    assert_eq!(f.git(&["show", "refs/stash:conflict.txt"]), "saved local work");
}

#[test]
fn preserving_session_restores_index_once_and_keeps_unfinished_changes_saved() {
    use plico::{operation_process::Progress, update_session::{Session, UpdateResult}};
    for result in [UpdateResult::Success, UpdateResult::ResolvedConflicts, UpdateResult::NothingToUpdate,
        UpdateResult::Incomplete, UpdateResult::Cancelled, UpdateResult::Error, UpdateResult::NotReady] {
        let f = Fixture::new();
        f.write("tracked.txt", "base\n");
        f.commit("base");
        f.write("tracked.txt", "staged\n");
        f.git(&["add", "tracked.txt"]);
        f.write("tracked.txt", "working\n");
        f.write("untracked.txt", "keep me\n");
        let repo = f.repo();
        let mut session = Session::begin(&repo, &Cancel::default()).unwrap();
        assert!(session.save("update fixture", &Cancel::default(), &Progress::default()).unwrap().status.success());
        assert!(session.saved_oid().is_some());
        assert_eq!(fs::read_to_string(f.0.join("tracked.txt")).unwrap(), "base\n");
        assert_eq!(fs::read_to_string(f.0.join("untracked.txt")).unwrap(), "keep me\n");
        let restored = session.finish(result, true, &Progress::default()).unwrap();
        let should_restore = matches!(result, UpdateResult::Success | UpdateResult::ResolvedConflicts | UpdateResult::NothingToUpdate);
        assert_eq!(restored.is_some(), should_restore);
        if should_restore {
            assert!(restored.unwrap().status.success());
            assert_eq!(f.git(&["show", ":tracked.txt"]), "staged");
            assert_eq!(fs::read_to_string(f.0.join("tracked.txt")).unwrap(), "working\n");
            assert!(session.saved_oid().is_none());
            assert!(session.finish(result, true, &Progress::default()).unwrap().is_none());
        } else {
            assert_eq!(f.git(&["show", "refs/stash:tracked.txt"]), "working");
            assert_eq!(fs::read_to_string(f.0.join("tracked.txt")).unwrap(), "base\n");
        }
    }
}

#[test]
fn preserving_session_retains_stash_on_conflict_and_refuses_unrelated_stash() {
    use plico::{operation_process::Progress, update_session::{Session, UpdateResult}};
    for external_stash in [false, true] {
        let f = Fixture::new();
        f.write("tracked.txt", "base\n");
        f.commit("base");
        f.write("tracked.txt", "saved changes\n");
        let repo = f.repo();
        let mut session = Session::begin(&repo, &Cancel::default()).unwrap();
        assert!(session.save("fixture", &Cancel::default(), &Progress::default()).unwrap().status.success());
        let saved = session.saved_oid().unwrap().to_owned();
        // Simulate the update (or another process changing the shared stash).
        f.write("tracked.txt", "incoming\n");
        if external_stash { f.git(&["stash", "push", "-m", "external"]); }
        else { f.commit("incoming"); }
        let before = f.git(&["rev-parse", "refs/stash"]);
        let result = session.finish(UpdateResult::Success, false, &Progress::default());
        if external_stash {
            assert!(result.is_err());
            assert!(repo.operation_state(&Cancel::default()).unwrap().conflicts.is_empty());
        } else {
            assert!(!result.unwrap().unwrap().status.success());
            assert_eq!(repo.operation_state(&Cancel::default()).unwrap().conflicts.len(), 1);
        }
        assert_eq!(session.saved_oid(), Some(saved.as_str()));
        assert_eq!(f.git(&["rev-parse", "refs/stash"]), before);
        assert_eq!(f.git(&["show", &format!("{saved}:tracked.txt")]), "saved changes");
        assert!(session.finish(UpdateResult::Success, false, &Progress::default()).unwrap().is_none());
    }
}

#[test]
fn preserving_session_handles_clean_tree_save_failure_and_precancel() {
    use plico::{operation_process::Progress, update_session::{Session, UpdateResult}};
    let f = Fixture::new();
    f.write("tracked.txt", "base\n");
    f.commit("base");
    let repo = f.repo();
    {
        let mut session = Session::begin(&repo, &Cancel::default()).unwrap();
        assert!(session.save("clean", &Cancel::default(), &Progress::default()).unwrap().status.success());
        assert!(session.saved_oid().is_none());
        assert!(session.finish(UpdateResult::Success, true, &Progress::default()).unwrap().is_none());
    }
    f.write("tracked.txt", "local\n");
    {
        let mut session = Session::begin(&repo, &Cancel::default()).unwrap();
        let cancel = Cancel::default();
        cancel.cancel();
        assert!(session.save("cancelled", &cancel, &Progress::default()).is_err());
        f.write(".git/index.lock", "fixture lock");
        assert!(!session.save("locked", &Cancel::default(), &Progress::default()).unwrap().status.success());
        assert!(session.finish(UpdateResult::Success, true, &Progress::default()).unwrap().is_none());
        assert!(session.saved_oid().is_none());
        assert_eq!(fs::read_to_string(f.0.join("tracked.txt")).unwrap(), "local\n");
        fs::remove_file(f.0.join(".git/index.lock")).unwrap();
    }
    assert_eq!(f.git(&["stash", "list"]), "");
}

#[test]
fn update_fast_forward_preserves_dirty_files_and_falls_back_on_divergence_or_overlap() {
    use plico::{operation_process::Progress, pull::Target, update_session::Session};
    for mode in ["clean-forward", "diverged", "overlap"] {
        let f = Fixture::new();
        f.write("local.txt", "base local\n");
        f.write("remote.txt", "base remote\n");
        f.commit("base");
        let base = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "-b", "incoming"]);
        f.write("remote.txt", "incoming\n");
        f.commit("incoming");
        let incoming = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "main"]);
        f.git(&["remote", "add", "origin", "https://example.invalid/repo.git"]);
        f.git(&["update-ref", "refs/remotes/origin/main", &incoming]);
        if mode == "diverged" {
            f.write("separate.txt", "local commit\n");
            f.commit("local commit");
        }
        let before = f.git(&["rev-parse", "HEAD"]);
        let dirty = if mode == "overlap" { "remote.txt" } else { "local.txt" };
        f.write(dirty, "staged\n");
        f.git(&["add", dirty]);
        f.write(dirty, "unstaged\n");
        let repo = f.repo();
        let mut session = Session::begin(&repo, &Cancel::default()).unwrap();
        let result = session.try_fast_forward(&Target { remote: "origin".into(), branch: "main".into() },
            &Cancel::default(), &Progress::default()).unwrap();
        let expected = if mode == "clean-forward" { &incoming } else { &before };
        assert_eq!(result.before.trim(), before);
        assert_eq!(result.after.unwrap().trim(), expected);
        assert_eq!(result.outcome.status.success(), mode == "clean-forward");
        assert_eq!(result.outcome.stopped, None);
        assert_eq!(f.git(&["rev-parse", "HEAD"]), *expected);
        assert_eq!(fs::read_to_string(f.0.join(dirty)).unwrap(), "unstaged\n");
        assert_eq!(f.git(&["show", &format!(":{dirty}")]), "staged");
        assert_eq!(f.git(&["stash", "list"]), "");
        if mode != "clean-forward" {
            assert!(session.save("fallback", &Cancel::default(), &Progress::default()).unwrap().status.success());
            assert!(session.saved_oid().is_some());
        } else { assert_ne!(base, incoming); }
    }
}

#[test]
fn published_rebase_check_uses_protected_ancestry_not_all_remote_commits() {
    use plico::{pull::Target, published_commits::default_protected};
    let f = Fixture::new();
    f.write("base.txt", "base\n"); f.commit("base");
    let base_oid = f.git(&["rev-parse", "HEAD"]);
    f.write("local.txt", "local\n"); f.commit("local");
    let local_oid = f.git(&["rev-parse", "HEAD"]);
    f.git(&["remote", "add", "origin", "https://example.invalid/repo.git"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &base_oid]);
    f.git(&["update-ref", "refs/remotes/origin/topic", &local_oid]);
    let target = |branch: &str| Target { remote: "origin".into(), branch: branch.into() };
    let repo = f.repo();
    let check = |protected: &[Target]| repo.published_rebase_target(&target("main"), protected, &Cancel::default()).unwrap();
    let defaults: Vec<_> = repo.pull_targets(&Cancel::default()).unwrap().branches.into_iter().filter(default_protected).collect();
    assert_eq!(check(&defaults), None); // Published to unprotected topic is not enough.
    assert_eq!(check(&[target("topic")]), Some(target("topic"))); // Explicit protection changes the decision.
    f.git(&["update-ref", "refs/remotes/origin/master", &local_oid]);
    assert_eq!(check(&[target("master")]), Some(target("master")));
    f.git(&["update-ref", "refs/remotes/origin/main", &local_oid]);
    assert_eq!(check(&[target("master")]), None); // Already in the update base, not rewritten.
    f.git(&["update-ref", "refs/remotes/origin/main", &base_oid]);
    f.git(&["checkout", "--orphan", "unrelated"]);
    f.git(&["rm", "-rf", "."]);
    f.write("other.txt", "other\n"); f.commit("other root");
    let other = f.git(&["rev-parse", "HEAD"]);
    f.git(&["update-ref", "refs/remotes/origin/master", &other]);
    f.git(&["checkout", "main"]);
    assert_eq!(check(&[target("master")]), None); // Disjoint history is not a query error.
    assert!(repo.published_rebase_target(&target("missing"), &[], &Cancel::default()).is_err());
    let cancel = Cancel::default(); cancel.cancel();
    assert!(repo.published_rebase_target(&target("main"), &defaults, &cancel).is_err());
}

#[test]
fn published_rebase_check_keeps_all_criss_cross_merge_bases() {
    use plico::pull::Target;
    let f = Fixture::new();
    f.write("file.txt", "base\n"); f.commit("root");
    let root = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let a = f.git(&["commit-tree", &tree, "-p", &root, "-m", "a"]);
    let b = f.git(&["commit-tree", &tree, "-p", &root, "-m", "b"]);
    let left = f.git(&["commit-tree", &tree, "-p", &a, "-p", &b, "-m", "left"]);
    let right = f.git(&["commit-tree", &tree, "-p", &b, "-p", &a, "-m", "right"]);
    f.git(&["update-ref", "refs/heads/main", &left]);
    f.git(&["remote", "add", "origin", "https://example.invalid/repo.git"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &a]);
    f.git(&["update-ref", "refs/remotes/origin/master", &right]);
    assert_eq!(f.git(&["merge-base", "--all", &left, &right]).lines().count(), 2);
    let base = Target { remote: "origin".into(), branch: "main".into() };
    let protected = Target { remote: "origin".into(), branch: "master".into() };
    assert_eq!(f.repo().published_rebase_target(&base, &[protected.clone()], &Cancel::default()).unwrap(), Some(protected.clone()));
    f.git(&["update-ref", "refs/remotes/origin/main", &left]);
    assert_eq!(f.repo().published_rebase_target(&base, &[protected], &Cancel::default()).unwrap(), None);
}

#[test]
fn update_session_rebases_and_restores_or_aborts_with_saved_changes_retained() {
    use plico::{operation_process::Progress, pull::Target, update_session::{Session, UpdateResult}};
    for mode in ["clean", "abort", "continue"] {
        let conflict = mode != "clean";
        let f = Fixture::new();
        f.write("shared.txt", "base\n");
        f.write("dirty.txt", "base dirty\n"); f.commit("base");
        f.git(&["checkout", "-b", "incoming"]);
        f.write("shared.txt", "remote\n"); f.commit("remote");
        let remote = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "main"]);
        f.write(if conflict { "shared.txt" } else { "local.txt" }, "local commit\n"); f.commit("local");
        let original = f.git(&["rev-parse", "HEAD"]);
        f.git(&["remote", "add", "origin", "https://example.invalid/repo.git"]);
        f.git(&["update-ref", "refs/remotes/origin/main", &remote]);
        f.write("dirty.txt", "uncommitted\n");
        let repo = f.repo();
        let mut session = Session::begin(&repo, &Cancel::default()).unwrap();
        assert!(session.save("fixture update", &Cancel::default(), &Progress::default()).unwrap().status.success());
        // Fixture-only editor; production must use its app-owned editor interaction.
        let step = session.rebase(&Target { remote: "origin".into(), branch: "main".into() },
            std::ffi::OsStr::new("true"), &Cancel::default(), &Progress::default()).unwrap();
        assert_eq!(step.outcome.status.success(), !conflict);
        assert!(step.cancellation_abort.is_none());
        if conflict {
            assert_eq!(step.state.unwrap().conflicts.len(), 1);
            assert!(session.finish(UpdateResult::Incomplete, false, &Progress::default()).unwrap().is_none());
            if mode == "continue" {
                assert!(session.continue_rebase(std::ffi::OsStr::new("true"), &Cancel::default(), &Progress::default()).is_err());
                f.write("shared.txt", "resolved\n");
                f.git(&["add", "shared.txt"]);
                let continued = session.continue_rebase(std::ffi::OsStr::new("true"), &Cancel::default(), &Progress::default()).unwrap();
                assert!(continued.outcome.status.success());
                assert!(!continued.state.unwrap().in_progress());
                assert_eq!(f.git(&["rev-parse", "HEAD^"]), remote);
                assert!(session.finish(UpdateResult::ResolvedConflicts, false, &Progress::default()).unwrap().unwrap().status.success());
                assert_eq!(fs::read_to_string(f.0.join("dirty.txt")).unwrap(), "uncommitted\n");
                assert_eq!(fs::read_to_string(f.0.join("shared.txt")).unwrap(), "resolved\n");
                continue;
            }
            assert!(session.abort_rebase(&Progress::default()).unwrap().status.success());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), original);
            assert!(!repo.operation_state(&Cancel::default()).unwrap().in_progress());
            assert!(session.finish(UpdateResult::Cancelled, false, &Progress::default()).unwrap().is_none());
            assert_eq!(f.git(&["show", "refs/stash:dirty.txt"]), "uncommitted");
            assert_eq!(fs::read_to_string(f.0.join("dirty.txt")).unwrap(), "base dirty\n");
        } else {
            assert!(!step.state.unwrap().in_progress());
            assert_eq!(f.git(&["rev-parse", "HEAD^"]), remote);
            assert_ne!(f.git(&["rev-parse", "HEAD"]), original);
            assert!(session.finish(UpdateResult::Success, false, &Progress::default()).unwrap().unwrap().status.success());
            assert_eq!(fs::read_to_string(f.0.join("dirty.txt")).unwrap(), "uncommitted\n");
            assert_eq!(f.git(&["stash", "list"]), "");
        }
    }
}

#[test]
fn cancelling_rebase_aborts_owned_operation_and_keeps_stash() {
    use plico::{operation_process::{Progress, StopReason}, pull::Target, update_session::Session};
    let f = Fixture::new();
    f.write("dirty.txt", "base\n"); f.commit("base");
    f.git(&["checkout", "-b", "incoming"]);
    f.write("remote.txt", "remote\n"); f.commit("remote");
    let remote = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.write("local.txt", "local\n"); f.commit("local");
    let original = f.git(&["rev-parse", "HEAD"]);
    f.git(&["remote", "add", "origin", "https://example.invalid/repo.git"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &remote]);
    f.write("dirty.txt", "saved\n");
    // Pause after commit replay, while rebase state still exists.
    f.write(".git/hooks/post-rewrite", "#!/bin/sh\necho fixture-rewrite-ready >&2\nsleep 20\n");
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(f.0.join(".git/hooks/post-rewrite"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let repo = f.repo();
    let mut session = Session::begin(&repo, &Cancel::default()).unwrap();
    assert!(session.save("fixture", &Cancel::default(), &Progress::default()).unwrap().status.success());
    let cancel = Cancel::default();
    let worker_cancel = cancel.clone();
    let progress = Progress::default();
    let worker_progress = progress.clone();
    let watcher = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        loop {
            let ready = worker_progress.snapshot().bytes().windows(21).any(|w| w == b"fixture-rewrite-ready");
            if ready || started.elapsed() > std::time::Duration::from_secs(10) {
                worker_cancel.cancel();
                return ready;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    let result = session.rebase(&Target { remote: "origin".into(), branch: "main".into() },
        std::ffi::OsStr::new("true"), &cancel, &progress).unwrap();
    assert!(watcher.join().unwrap(), "rewrite hook was not reached");
    assert_eq!(result.outcome.stopped, Some(StopReason::Cancelled));
    assert!(result.cancellation_abort.unwrap().unwrap().status.success());
    assert!(!result.state.unwrap().in_progress());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), original);
    assert_eq!(f.git(&["show", "refs/stash:dirty.txt"]), "saved");
}

#[test]
fn ordinary_pull_preserves_options_git_strategy_and_real_conflict_state() {
    use plico::{pull::{self, CommandSpec, Options, OptionFlag, Target}, operation_process::Progress, operation_state::Operation};
    for mode in ["fast-forward", "merge", "no-commit", "conflict", "untracked", "ff-only", "configured-rebase", "squash", "interactive-editor"] {
        let remote = Fixture::new();
        remote.write("shared.txt", "base\n"); remote.commit("base");
        let original_remote = remote.git(&["rev-parse", "HEAD"]);
        let f = Fixture::new();
        f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
        f.git(&["fetch", "origin"]);
        f.git(&["checkout", "-B", "main", "origin/main"]);
        f.git(&["config", "pull.rebase", if mode == "configured-rebase" { "true" } else if mode == "interactive-editor" { "interactive" } else { "false" }]);
        remote.write("shared.txt", "remote\n");
        remote.write("remote-new.txt", "remote new\n"); remote.commit("remote advance");
        let remote_tip = remote.git(&["rev-parse", "HEAD"]);
        if !matches!(mode, "fast-forward" | "untracked" | "interactive-editor") {
            f.write(if mode == "conflict" { "shared.txt" } else { "local.txt" }, "local commit\n");
            f.commit("local advance");
        }
        if mode == "untracked" { f.write("remote-new.txt", "valuable untracked\n"); }
        let before = f.git(&["rev-parse", "HEAD"]);
        let options = match mode {
            "no-commit" => Options::new([OptionFlag::NoFf, OptionFlag::NoCommit]).unwrap(),
            "ff-only" => Options::new([OptionFlag::FfOnly]).unwrap(),
            "squash" => Options::new([OptionFlag::Squash]).unwrap(),
            _ => Options::default(),
        };
        let spec = CommandSpec { target: Target { remote: "origin".into(), branch: "main".into() }, options,
            commit_editor: "true".into(), sequence_editor: None }; // fixture editor only
        let result = pull::execute_command(&f.repo(), &spec, &Cancel::default(), &Progress::default(), None);
        if mode == "interactive-editor" {
            assert!(result.is_err());
            assert_eq!(f.git(&["rev-parse", "origin/main"]), original_remote);
            continue;
        }
        let result = result.unwrap();
        let outcome = result.command.unwrap();
        let state = result.state.unwrap();
        assert_eq!(f.git(&["rev-parse", "origin/main"]), remote_tip); // Fetch may succeed even if integration fails.
        assert_eq!(outcome.status.success(), !matches!(mode, "conflict" | "untracked" | "ff-only"), "{mode}: {:?}", outcome.stderr.bytes());
        match mode {
            "fast-forward" => assert_eq!(f.git(&["rev-parse", "HEAD"]), remote_tip),
            "merge" => { assert_eq!(f.git(&["rev-parse", "HEAD^1"]), before); assert_eq!(f.git(&["rev-parse", "HEAD^2"]), remote_tip); }
            "configured-rebase" => assert_eq!(f.git(&["rev-parse", "HEAD^"]), remote_tip),
            "no-commit" => { assert_eq!(state.operation, Operation::Merge); assert!(state.conflicts.is_empty()); assert!(!result.commit_after_merge); }
            "conflict" => { assert_eq!(state.operation, Operation::Merge); assert_eq!(state.conflicts.len(), 1); }
            "untracked" => assert_eq!(fs::read_to_string(f.0.join("remote-new.txt")).unwrap(), "valuable untracked\n"),
            "squash" => { assert_eq!(state.operation, Operation::None); assert_eq!(f.git(&["show", ":remote-new.txt"]), "remote new"); }
            _ => {}
        }
        if matches!(mode, "no-commit" | "conflict" | "untracked" | "ff-only" | "squash") {
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
        }
    }
}

#[test]
fn push_parser_reads_real_local_remote_outcomes() {
    use plico::push_result::{parse, Kind};
    let remote = Fixture::new();
    remote.git(&["config", "core.bare", "true"]);
    let f = Fixture::new(); f.write("base.txt", "base\n"); f.commit("base");
    f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
    let push = |extra: &[&str]| {
        let output = Command::new("git").arg("-C").arg(&f.0).args(["push", "--porcelain"])
            .args(extra).args(["origin", "refs/heads/main:refs/heads/main"]).output().unwrap();
        (output.status.success(), String::from_utf8(output.stdout).unwrap())
    };
    let (ok, output) = push(&[]); assert!(ok);
    assert_eq!(parse(&output).next().unwrap().kind, Kind::NewRef);
    let (ok, output) = push(&[]); assert!(ok);
    assert_eq!(parse(&output).next().unwrap().kind, Kind::UpToDate);
    let first = f.git(&["rev-parse", "HEAD"]);
    f.write("base.txt", "advance\n"); f.commit("advance");
    let (ok, output) = push(&[]); assert!(ok);
    let result = parse(&output).next().unwrap();
    assert_eq!(result.kind, Kind::Success); assert!(result.range.is_some());
    let remote_tip = remote.git(&["rev-parse", "main"]);
    f.git(&["reset", "--hard", &first]);
    f.write("other.txt", "diverged\n"); f.commit("diverged");
    let (ok, output) = push(&[]); assert!(!ok);
    assert!(parse(&output).next().unwrap().is_non_ff_update());
    assert_eq!(remote.git(&["rev-parse", "main"]), remote_tip);
    remote.git(&["config", "receive.denyNonFastForwards", "true"]);
    // Force is exercised only against a disposable fixture, not a product action.
    let (ok, output) = push(&["--force"]); assert!(!ok);
    let result = parse(&output).next().unwrap();
    assert_eq!(result.kind, Kind::Rejected); assert_eq!(result.summary, "[remote rejected]");
    remote.git(&["config", "receive.denyNonFastForwards", "false"]);
    let (ok, output) = push(&["--force"]); assert!(ok);
    assert_eq!(parse(&output).next().unwrap().kind, Kind::ForcedUpdate);
}

#[test]
fn push_snapshot_reads_refspecs_tracking_and_new_branch_without_network() {
    use plico::push_target::Origin;
    let f = Fixture::new(); let token = Cancel::default();
    assert!(f.repo().push_snapshot(&token).unwrap().is_none());
    f.write("base.txt", "base\n"); f.commit("base");
    let head = f.git(&["rev-parse", "HEAD"]);
    assert!(f.repo().push_snapshot(&token).unwrap().unwrap().default.is_none());
    for remote in ["origin", "my/remote"] {
        // No network requests should be needed to select a push target.
        f.git(&["remote", "add", remote, "https://example.invalid/repo"]);
        f.git(&["update-ref", &format!("refs/remotes/{remote}/main"), "HEAD"]);
    }
    let snapshot = f.repo().push_snapshot(&token).unwrap().unwrap();
    assert_eq!(snapshot.source_oid, head); assert_eq!(snapshot.source_branch, "main");
    let target = snapshot.default.unwrap();
    assert_eq!(target.remote, "origin"); assert!(!target.new_branch);
    f.git(&["config", "--add", "remote.origin.push", "refs/heads/*:refs/heads/review/*"]);
    f.git(&["config", "--add", "remote.origin.push", "HEAD:refs/for/main"]);
    let target = f.repo().push_snapshot(&token).unwrap().unwrap().default.unwrap();
    assert_eq!(target.origin, Origin::PushSpec); assert!(target.special); assert_eq!(target.destination(), "refs/for/main");
    f.git(&["config", "branch.main.remote", "my/remote"]);
    f.git(&["config", "branch.main.merge", "refs/heads/main"]);
    let target = f.repo().push_snapshot(&token).unwrap().unwrap().default.unwrap();
    assert_eq!(target.remote, "my/remote"); assert_eq!(target.origin, Origin::TrackingBranch);
    f.git(&["config", "remote.my/remote.push", "refs/heads/*:refs/remotes/my/remote/*"]);
    let target = f.repo().push_snapshot(&token).unwrap().unwrap().default.unwrap();
    assert_eq!(target.remote, "my/remote"); assert!(!target.special); assert!(!target.new_branch);
    f.git(&["config", "remote.my/remote.push", "HEAD:refs/for/main"]);
    let target = f.repo().push_snapshot(&token).unwrap().unwrap().default.unwrap();
    assert!(target.special); assert_eq!(target.destination(), "refs/for/main");
    f.git(&["checkout", "-b", "new-topic"]);
    f.git(&["config", "--unset-all", "remote.origin.push"]);
    let snapshot = f.repo().push_snapshot(&token).unwrap().unwrap();
    let target = snapshot.default.unwrap();
    assert_eq!(target.branch, "new-topic"); assert!(target.new_branch);
    assert!(target.should_set_upstream(true, snapshot.targets.tracked.is_some(), false));
    f.git(&["checkout", "--detach", "HEAD"]);
    assert!(f.repo().push_snapshot(&token).unwrap().is_none());
    f.git(&["checkout", "main"]);
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(f.repo().push_snapshot(&cancelled).is_err());
    // Corrupt configuration must not become an empty configuration/default target.
    fs::write(f.0.join(".git/config"), "[unterminated\n").unwrap();
    assert!(f.repo().push_snapshot(&token).is_err());
}

#[test]
fn outgoing_push_history_uses_target_range_and_selected_remote_exclusions() {
    use plico::push_target::{Target, Origin};
    let f = Fixture::new();
    f.write("a", "base"); f.commit("base"); let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["remote", "add", "origin", "https://example.invalid/repo"]);
    f.git(&["remote", "add", "other", "https://example.invalid/other"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &base]);
    f.write("a", "one"); f.commit("one"); let one = f.git(&["rev-parse", "HEAD"]);
    f.git(&["update-ref", "refs/remotes/other/main", &one]);
    f.write("a", "two"); f.commit("two"); let source = f.git(&["rev-parse", "HEAD"]);
    let mut target = Target { remote: "origin".into(), branch: "main".into(), special: false, new_branch: false, origin: Origin::TrackingBranch };
    let repo = f.repo(); let token = Cancel::default();
    let page = repo.outgoing_commits(&source, &target, 0, 1, &token).unwrap();
    assert_eq!(page.commits[0].id, source); assert!(page.more); assert!(!page.capped);
    let page = repo.outgoing_commits(&source, &target, 1, 1, &token).unwrap();
    assert_eq!(page.commits[0].id, one); assert!(!page.more);
    assert!(repo.outgoing_commits(&base, &target, 0, 20, &token).unwrap().commits.is_empty());
    target.new_branch = true; target.branch = "new".into();
    let page = repo.outgoing_commits(&source, &target, 0, 20, &token).unwrap();
    assert_eq!(page.commits.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec![source.as_str(), one.as_str()]);
    // Another branch on this remote also excludes its reachable history.
    f.git(&["update-ref", "refs/remotes/origin/topic", &one]);
    assert_eq!(repo.outgoing_commits(&source, &target, 0, 20, &token).unwrap().commits.len(), 1);
    target.special = true; target.branch = "refs/for/main".into();
    assert_eq!(repo.outgoing_commits(&source, &target, 0, 20, &token).unwrap().commits.len(), 1);
    target.new_branch = false;
    assert!(repo.outgoing_commits(&source, &target, 0, 20, &token).is_err());
    target.remote = "missing".into();
    assert!(repo.outgoing_commits(&source, &target, 0, 20, &token).is_err());
    assert!(repo.outgoing_commits("--all", &target, 0, 20, &token).is_err());
    token.cancel();
    assert!(repo.outgoing_commits(&source, &target, 0, 20, &token).is_err());
}

#[test]
fn outgoing_push_new_branch_ceiling_is_distinct_from_existing_target_pagination() {
    use plico::push_target::{Target, Origin};
    use std::io::Write;
    use std::process::Stdio;
    let f = Fixture::new();
    let mut child = Command::new("git").arg("-C").arg(&f.0).args(["fast-import", "--quiet"])
        .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    for i in 0..1002 {
        writeln!(input, "commit refs/heads/main\ncommitter Fixture <fixture@example.invalid> {} +0000\ndata 1\nx\n", 1700000000+i).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    f.git(&["remote", "add", "origin", "https://example.invalid/repo"]);
    let source = f.git(&["rev-parse", "HEAD"]);
    let mut target = Target { remote: "origin".into(), branch: "main".into(), special: false,
        new_branch: true, origin: Origin::TrackingBranch };
    let repo = f.repo(); let token = Cancel::default();
    let page = repo.outgoing_commits(&source, &target, 990, 20, &token).unwrap();
    assert_eq!(page.commits.len(), 10); assert!(page.capped); assert!(!page.more);
    let page = repo.outgoing_commits(&source, &target, 1000, 20, &token).unwrap();
    assert!(page.commits.is_empty()); assert!(page.capped);
    let base = f.git(&["rev-list", "--max-parents=0", "HEAD"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &base]);
    target.new_branch = false;
    let page = repo.outgoing_commits(&source, &target, 990, 10, &token).unwrap();
    assert_eq!(page.commits.len(), 10); assert!(page.more); assert!(!page.capped);
    let page = repo.outgoing_commits(&source, &target, 1000, 10, &token).unwrap();
    assert_eq!(page.commits.len(), 1); assert!(!page.more); assert!(!page.capped);
}

#[test]
fn ordinary_push_attempt_sets_tracking_preserves_worktree_and_reports_rejection() {
    use plico::{push::{self, Spec, Status}, push_target::{Target, Origin}, operation_process::Progress};
    let remote = Fixture::new(); remote.git(&["config", "core.bare", "true"]);
    let f = Fixture::new(); f.write("a", "base"); f.commit("base");
    f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
    let mut spec = Spec { tags: None, force: false, source_branch: "main".into(), source_oid: f.git(&["rev-parse", "HEAD"]),
        target: Target { remote: "origin".into(), branch: "main".into(), special: false, new_branch: true, origin: Origin::TrackingBranch }, set_upstream: true };
    let token = Cancel::default(); let progress = Progress::default();
    f.write("a", "unstaged local work"); f.write("untracked", "keep");
    let result = push::execute(&f.repo(), &spec, &token, &progress, None).unwrap();
    assert_eq!(result.status, Status::NewBranch);
    assert_eq!(remote.git(&["rev-parse", "main"]), spec.source_oid);
    assert_eq!(f.git(&["rev-parse", "--symbolic-full-name", "@{upstream}"]), "refs/remotes/origin/main");
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "unstaged local work");
    assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep");
    spec.set_upstream = false; spec.target.new_branch = false;
    assert_eq!(push::execute(&f.repo(), &spec, &token, &progress, None).unwrap().status, Status::UpToDate);
    f.commit("advance");
    assert!(push::execute(&f.repo(), &spec, &token, &progress, None).is_err());
    let base = spec.source_oid.clone(); spec.source_oid = f.git(&["rev-parse", "HEAD"]);
    assert_eq!(push::execute(&f.repo(), &spec, &token, &progress, None).unwrap().status, Status::Success);
    let remote_tip = remote.git(&["rev-parse", "main"]);
    f.git(&["reset", "--hard", &base]); f.write("b", "diverged"); f.commit("diverged"); spec.source_oid = f.git(&["rev-parse", "HEAD"]);
    assert_eq!(push::execute(&f.repo(), &spec, &token, &progress, None).unwrap().status, Status::RejectedNonFf);
    assert_eq!(remote.git(&["rev-parse", "main"]), remote_tip);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), spec.source_oid);
    spec.target.branch = "feature".into(); spec.target.new_branch = true;
    assert_eq!(push::execute(&f.repo(), &spec, &token, &progress, None).unwrap().status, Status::NewBranch);
    assert_eq!(f.git(&["rev-parse", "--symbolic-full-name", "@{upstream}"]), "refs/remotes/origin/main");
    spec.target.branch = "invalid..name".into();
    assert!(push::execute(&f.repo(), &spec, &token, &progress, None).is_err());
    token.cancel(); assert!(push::execute(&f.repo(), &spec, &token, &progress, None).is_err());
}

#[test]
fn ordinary_push_multiple_urls_does_not_hide_partial_remote_failure() {
    use plico::{push::{self, Spec, Status}, push_target::{Target, Origin}};
    let first = Fixture::new(); first.git(&["config", "core.bare", "true"]);
    let second = Fixture::new(); second.git(&["config", "core.bare", "true"]);
    let f = Fixture::new(); f.write("a", "base"); f.commit("base");
    f.git(&["remote", "add", "origin", first.0.to_str().unwrap()]);
    for remote in [&first, &second] { f.git(&["config", "--add", "remote.origin.pushurl", remote.0.to_str().unwrap()]); }
    let mut spec = Spec { tags: None, force: false, source_branch: "main".into(), source_oid: f.git(&["rev-parse", "HEAD"]), set_upstream: false,
        target: Target { remote: "origin".into(), branch: "main".into(), special: false, new_branch: true, origin: Origin::TrackingBranch } };
    let token = Cancel::default();
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::NewBranch);
    let old = spec.source_oid.clone();
    f.write("a", "first advance"); f.commit("first advance"); let ahead = f.git(&["rev-parse", "HEAD"]);
    f.git(&["push", first.0.to_str().unwrap(), "main"]);
    f.git(&["reset", "--hard", &old]); f.write("b", "second advance"); f.commit("second advance");
    spec.source_oid = f.git(&["rev-parse", "HEAD"]); spec.target.new_branch = false;
    let result = push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap();
    assert_eq!(result.status, Status::RejectedNonFf);
    assert_eq!(first.git(&["rev-parse", "main"]), ahead);
    assert_eq!(second.git(&["rev-parse", "main"]), spec.source_oid);
    assert!(!result.command.unwrap().status.success());
    // First URL still rejects non-FF; the second now rejects updating its
    // checked-out branch. That custom rejection must suppress recovery.
    let second_before = spec.source_oid.clone();
    second.git(&["config", "core.bare", "false"]);
    second.git(&["config", "receive.denyCurrentBranch", "refuse"]);
    f.write("c", "next"); f.commit("next"); spec.source_oid = f.git(&["rev-parse", "HEAD"]);
    let result = push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap();
    assert_eq!(result.status, Status::RejectedOther);
    assert_eq!(first.git(&["rev-parse", "main"]), ahead);
    assert_eq!(second.git(&["rev-parse", "main"]), second_before);

}

#[test]
fn custom_push_target_validates_remote_and_branch_before_preview() {
    let f = Fixture::new(); f.write("a", "base"); f.commit("base");
    f.git(&["remote", "add", "origin", "https://example.invalid/repo"]);
    f.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    let repo = f.repo(); let token = Cancel::default();
    assert!(!repo.custom_push_target("origin", "main", &token).unwrap().new_branch);
    let new = repo.custom_push_target("origin", "feature/审阅", &token).unwrap();
    assert!(new.new_branch); assert_eq!(new.destination(), "refs/heads/feature/审阅");
    for branch in ["", "--force", "a..b", "a b", "a:b", "a\nb", "a.lock", "a/", "a\"b", "a./b"] {
        assert!(repo.custom_push_target("origin", branch, &token).is_err(), "{branch:?}");
    }
    assert!(repo.custom_push_target("missing", "main", &token).is_err());
    token.cancel(); assert!(repo.custom_push_target("origin", "main", &token).is_err());
}

#[test]
fn outgoing_pages_reject_changed_remote_references() {
    use plico::push_target::{Target, Origin};
    let f = Fixture::new();
    f.write("a", "base"); f.commit("base"); let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["remote", "add", "origin", "https://example.invalid/repo"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &base]);
    f.write("a", "one"); f.commit("one"); let one = f.git(&["rev-parse", "HEAD"]);
    f.write("a", "two"); f.commit("two"); let source = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo(); let token = Cancel::default();
    for new_branch in [false, true] {
        f.git(&["update-ref", "refs/remotes/origin/main", &base]);
        let target = Target { remote: "origin".into(), branch: if new_branch { "new" } else { "main" }.into(),
            special: false, new_branch, origin: Origin::TrackingBranch };
        let first = repo.outgoing_commits_checked(&source, &target, 0, 1, None, &token).unwrap();
        assert_eq!(first.commits[0].id, source);
        let second = repo.outgoing_commits_checked(&source, &target, 1, 1, Some(&first.references), &token).unwrap();
        assert_eq!(second.commits[0].id, one);
        f.git(&["update-ref", "refs/remotes/origin/main", &one]);
        assert_eq!(repo.outgoing_commits_checked(&source, &target, 1, 1, Some(&first.references), &token).err().unwrap(),
            "Push references changed; reopen the push dialog");
        let refreshed = repo.outgoing_commits_checked(&source, &target, 0, 1, None, &token).unwrap();
        assert_eq!(refreshed.commits[0].id, source); assert!(!refreshed.more);
        f.git(&["update-ref", "refs/remotes/origin/new-topic", &source]);
        assert!(repo.outgoing_commits_checked(&source, &target, 1, 1, Some(&refreshed.references), &token).is_err());
        f.git(&["update-ref", "-d", "refs/remotes/origin/new-topic"]);
    }
    assert_eq!(f.git(&["rev-parse", "HEAD"]), source);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn commit_checkout_preserves_changes_and_reports_actual_head() {
    use plico::{checkout, operation_process::Progress};
    for mode in ["clean", "unrelated", "staged", "overwritten", "untracked", "stale", "missing", "blob", "cancel"] {
        let f = Fixture::new();
        f.write("a", "base"); f.write("keep", "original"); f.write("restored", "tracked"); f.commit("base");
        let base = f.git(&["rev-parse", "HEAD"]);
        f.write("a", "tip"); f.git(&["rm", "restored"]); f.commit("tip");
        let head = f.git(&["rev-parse", "HEAD"]);
        match mode {
            "unrelated" | "staged" => { f.write("keep", "local"); if mode == "staged" { f.git(&["add", "keep"]); } }
            "overwritten" => f.write("a", "local"),
            "untracked" => f.write("restored", "untracked"),
            _ => {}
        }
        let before = f.git(&["status", "--porcelain"]);
        let token = Cancel::default(); if mode == "cancel" { token.cancel(); }
        let target = match mode {
            "missing" => "1111111111111111111111111111111111111111".into(),
            "blob" => f.git(&["rev-parse", "HEAD:a"]),
            _ => base.clone(),
        };
        let result = checkout::execute(&f.repo(), &target, if mode == "stale" { &base } else { &head }, &token, &Progress::default());
        match mode {
            "stale" | "missing" | "blob" | "cancel" => { assert!(result.is_err(), "{mode}"); assert_eq!(f.git(&["rev-parse", "HEAD"]), head); }
            "overwritten" | "untracked" => {
                let result = result.unwrap(); assert!(!result.checked_out(&base));
                assert_eq!(result.status(&base), if mode == "overwritten" { checkout::Status::LocalChanges } else { checkout::Status::UntrackedFiles });
                let files = result.affected_files(&base);
                assert!(files.complete); assert_eq!(files.paths, [if mode == "overwritten" { "a" } else { "restored" }]);
                assert!(!result.command.unwrap().status.success()); assert!(!result.state.unwrap().detached);
                assert_eq!(result.head.unwrap(), head);
            }
            _ => {
                let result = result.unwrap(); assert!(result.checked_out(&base), "{mode}");
                assert_eq!(result.status(&base), checkout::Status::Success);
                assert_eq!(f.git(&["rev-parse", "refs/heads/main"]), head);
                assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "base");
                // Repeated checkout of the current commit remains detached.
                assert!(checkout::execute(&f.repo(), &base, &base, &token, &Progress::default()).unwrap().checked_out(&base));
            }
        }
        assert_eq!(f.git(&["status", "--porcelain"]), before, "{mode}");
        if matches!(mode, "unrelated" | "staged") { assert_eq!(fs::read_to_string(f.0.join("keep")).unwrap(), "local"); }
        if mode == "overwritten" { assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "local"); }
        if mode == "untracked" { assert_eq!(fs::read_to_string(f.0.join("restored")).unwrap(), "untracked"); }
    }
}

#[test]
fn commit_checkout_does_not_discard_unmerged_index() {
    use plico::{checkout, operation_process::Progress, operation_state::Operation};
    let f = Fixture::new();
    f.write("shared", "base\n"); f.commit("base"); let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "-b", "topic"]); f.write("shared", "topic\n"); f.commit("topic");
    f.git(&["checkout", "main"]); f.write("shared", "main\n"); f.commit("main");
    let head = f.git(&["rev-parse", "HEAD"]);
    assert!(!Command::new("git").arg("-C").arg(&f.0).args(["merge", "topic"]).output().unwrap().status.success());
    let index = f.git(&["ls-files", "--unmerged"]);
    let worktree = fs::read(f.0.join("shared")).unwrap();
    let result = checkout::execute(&f.repo(), &base, &head, &Cancel::default(), &Progress::default()).unwrap();
    assert!(!result.checked_out(&base));
    assert_eq!(result.status(&base), checkout::Status::Unmerged);
    assert!(result.command.unwrap().events.contains(plico::operation_events::Event::UnmergedPreventingCheckout));
    assert_eq!(result.state.unwrap().operation, Operation::Merge);
    assert_eq!(result.head.unwrap(), head);
    assert_eq!(f.git(&["ls-files", "--unmerged"]), index);
    assert_eq!(fs::read(f.0.join("shared")).unwrap(), worktree);
    for target in ["--force", "HEAD", "abc", "../file"] {
        assert!(checkout::execute(&f.repo(), target, &head, &Cancel::default(), &Progress::default()).is_err());
    }
}

#[test]
fn smart_checkout_restores_after_success_failure_and_retains_conflicted_stash() {
    use plico::{checkout, operation_process::Progress};
    for mode in ["success", "checkout-failed", "restore-conflict", "save-failed"] {
        let f = Fixture::new();
        let base_text = (1..=20).map(|i| format!("line {i}\n")).collect::<String>();
        f.write("a", &base_text); f.write("old", "initial"); f.commit("base");
        let base = f.git(&["rev-parse", "HEAD"]);
        f.write("a", &base_text.replace("line 1\n", "target line\n"));
        f.write("incoming", "target file"); f.commit("target");
        let target = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "--detach", &base]);
        // A pre-existing stash must survive the entire session.
        f.write("old", "previous stash"); f.git(&["stash", "push", "-m", "earlier"]);
        let earlier = f.git(&["rev-parse", "refs/stash"]);
        let local = base_text.replace(if mode == "restore-conflict" { "line 1\n" } else { "line 20\n" }, "local line\n");
        f.write("a", &local);
        if mode == "success" { f.git(&["add", "a"]); }
        if mode == "checkout-failed" { f.write("incoming", "untracked local"); }
        if mode == "save-failed" { fs::write(f.0.join(".git/index.lock"), "fixture lock").unwrap(); }
        let result = checkout::smart_execute(&f.repo(), &target, &base, mode == "success", &Cancel::default(), &Progress::default()).unwrap();
        if mode == "save-failed" {
            assert!(!result.save.unwrap().status.success()); assert!(result.checkout.is_none());
            assert!(result.restore.unwrap().is_none()); assert_eq!(result.head.unwrap(), base);
            assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), local);
        } else {
            assert!(result.save.unwrap().status.success());
            let checked = result.checkout.unwrap().unwrap();
            if mode == "checkout-failed" {
                assert_eq!(checked.status(&target), checkout::Status::UntrackedFiles);
                assert_eq!(result.head.unwrap(), base);
                assert_eq!(fs::read_to_string(f.0.join("incoming")).unwrap(), "untracked local");
                assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), local);
            } else { assert!(checked.checked_out(&target)); assert_eq!(result.head.unwrap(), target); }
            let restore = result.restore.unwrap().unwrap();
            if mode == "restore-conflict" {
                assert!(!restore.status.success()); assert!(!result.state.unwrap().conflicts.is_empty());
                assert_eq!(result.saved_oid.unwrap(), f.git(&["rev-parse", "refs/stash"]));
                assert_eq!(f.git(&["rev-parse", "stash@{1}"]), earlier);
                continue;
            }
            assert!(restore.status.success()); assert!(result.saved_oid.is_none());
            if mode == "success" {
                assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), local.replace("line 1\n", "target line\n"));
                assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "a");
            }
        }
        assert_eq!(f.git(&["rev-parse", "refs/stash"]), earlier);
    }
}

#[test]
fn checkout_affected_files_decode_git_quoted_unicode_names() {
    use plico::{checkout, operation_process::Progress};
    let f = Fixture::new();
    let names = ["中文 文件.txt", "ordinary space.txt"];
    for path in names { f.write(path, "base"); } f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    for path in names { f.write(path, "tip"); } f.commit("tip");
    let head = f.git(&["rev-parse", "HEAD"]);
    for path in names { f.write(path, "local"); }
    f.git(&["config", "core.quotePath", "true"]);
    let result = checkout::execute(&f.repo(), &base, &head, &Cancel::default(), &Progress::default()).unwrap();
    let files = result.affected_files(&base);
    assert!(files.complete); assert_eq!(files.paths.len(), names.len());
    for path in names {
        assert!(files.paths.contains(&path.to_owned()), "{:?}", files.paths);
        assert_eq!(fs::read_to_string(f.0.join(path)).unwrap(), "local");
    }
    assert_eq!(result.head.unwrap(), head);
}

#[test]
fn explicit_force_checkout_discards_tracked_changes_but_guards_stale_head() {
    use plico::{checkout, operation_process::Progress};
    let f = Fixture::new(); f.write("a", "base"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.write("a", "tip"); f.commit("tip"); let head = f.git(&["rev-parse", "HEAD"]);
    f.write("a", "local"); f.write("unrelated", "keep");
    assert!(checkout::force_execute(&f.repo(), &base, &base, &Cancel::default(), &Progress::default()).is_err());
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "local");
    let result = checkout::force_execute(&f.repo(), &base, &head, &Cancel::default(), &Progress::default()).unwrap();
    assert!(result.checked_out(&base));
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "base");
    assert_eq!(fs::read_to_string(f.0.join("unrelated")).unwrap(), "keep");
    assert_eq!(f.git(&["rev-parse", "main"]), head);
}

#[test]
fn merge_updater_uses_remote_tracking_ref_and_stops_for_conflicts() {
    use plico::{update_session::{Session, UpdateResult}, pull::Target, operation_process::Progress};
    for mode in ["fast-forward", "merge", "conflict", "local-overwrite", "cancelled"] {
        let f = Fixture::new(); f.write("shared", "base\n"); f.write("local", "base\n"); f.commit("base");
        f.git(&["checkout", "-b", "topic"]);
        f.write("shared", "incoming\n"); f.commit("incoming"); let incoming = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "main"]);
        if mode == "merge" { f.write("ours", "ours\n"); f.commit("ours"); }
        if mode == "conflict" { f.write("shared", "ours\n"); f.commit("ours"); }
        if mode == "local-overwrite" { f.write("shared", "valuable\n"); }
        f.write("local", "unrelated local\n");
        f.git(&["remote", "add", "origin", "https://example.invalid/not-contacted"]);
        f.git(&["update-ref", "refs/remotes/origin/main", &incoming]);
        let before = f.git(&["rev-parse", "HEAD"]);
        let repo = f.repo(); let token = Cancel::default(); let mut session = Session::begin(&repo, &token).unwrap();
        if mode == "cancelled" { token.cancel(); }
        let target = Target { remote: "origin".into(), branch: "main".into() };
        let result = session.merge(&target, std::ffi::OsStr::new("true"), &token, &Progress::default());
        if mode == "cancelled" { assert!(result.is_err()); assert_eq!(f.git(&["rev-parse", "HEAD"]), before); continue; }
        let step = result.unwrap();
        assert_eq!(step.result(), match mode { "conflict" => UpdateResult::Incomplete, "local-overwrite" => UpdateResult::Error, _ => UpdateResult::Success });
        assert!(step.cancellation_reset.is_none());
        assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "unrelated local\n");
        match mode {
            "merge" => { assert_eq!(f.git(&["rev-parse", "HEAD^1"]), before); assert_eq!(f.git(&["rev-parse", "HEAD^2"]), incoming); }
            "fast-forward" => assert_eq!(f.git(&["rev-parse", "HEAD"]), incoming),
            _ => assert_eq!(f.git(&["rev-parse", "HEAD"]), before),
        }
        if mode == "local-overwrite" { assert_eq!(fs::read_to_string(f.0.join("shared")).unwrap(), "valuable\n"); }
        assert!(session.merge(&target, std::ffi::OsStr::new("true"), &token, &Progress::default()).is_err());
    }
}

#[test]
fn merge_updater_running_cancel_records_reset_outcome_and_preserves_local_work() {
    use plico::{update_session::{Session, UpdateResult}, pull::Target, operation_process::{Progress, StopReason}};
    for block_reset in [false, true] {
        let f = Fixture::new(); f.write("dirty", "base\n"); f.commit("base");
        f.git(&["checkout", "-b", "incoming"]); f.write("incoming", "remote\n"); f.commit("incoming");
        let incoming = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "main"]); f.write("ours", "ours\n"); f.commit("ours");
        let before = f.git(&["rev-parse", "HEAD"]);
        f.git(&["remote", "add", "origin", "https://example.invalid/not-contacted"]);
        f.git(&["update-ref", "refs/remotes/origin/main", &incoming]);
        f.write("dirty", "valuable unstaged\n"); f.write("untracked", "keep\n");
        let hook = format!("#!/bin/sh\n{}echo fixture-merge-ready >&2\nsleep 20\n",
            if block_reset { "echo fixture-lock > .git/index.lock\n" } else { "" });
        f.write(".git/hooks/pre-merge-commit", &hook);
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(f.0.join(".git/hooks/pre-merge-commit"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let repo = f.repo(); let token = Cancel::default(); let progress = Progress::default();
        let mut session = Session::begin(&repo, &token).unwrap();
        let watched_token = token.clone(); let watched_progress = progress.clone();
        let watcher = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            loop {
                let ready = watched_progress.snapshot().bytes().windows(b"fixture-merge-ready".len()).any(|w| w == b"fixture-merge-ready");
                if ready || started.elapsed() > std::time::Duration::from_secs(10) { watched_token.cancel(); return ready; }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        let result = session.merge(&Target { remote: "origin".into(), branch: "main".into() },
            std::ffi::OsStr::new("true"), &token, &progress);
        assert!(watcher.join().unwrap(), "merge hook was not reached");
        let result = result.unwrap();
        assert_eq!(result.result(), UpdateResult::Cancelled);
        assert_eq!(result.outcome.stopped, Some(StopReason::Cancelled));
        let reset = result.cancellation_reset.unwrap().unwrap();
        assert_eq!(reset.status.success(), !block_reset, "{:?}", reset.stderr.bytes());
        assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
        assert_eq!(fs::read_to_string(f.0.join("dirty")).unwrap(), "valuable unstaged\n");
        assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep\n");
        if !block_reset {
            assert!(!f.0.join("incoming").exists());
            assert!(f.git(&["diff", "--cached", "--name-only"]).is_empty());
            assert!(!result.state.unwrap().in_progress());
        } else {
            assert!(f.0.join(".git/index.lock").exists());
            assert!(f.0.join("incoming").exists());
        }
    }
}

#[test]
fn merge_update_save_decision_matches_staged_or_overlapping_paths() {
    use plico::pull::Target;
    let f = Fixture::new(); f.write("shared", "base"); f.write("other", "base"); f.write("old", "base"); f.commit("base");
    f.git(&["checkout", "-b", "topic"]); f.write("shared", "remote"); f.commit("remote");
    let incoming = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.git(&["remote", "add", "origin", "https://example.invalid/not-contacted"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &incoming]);
    let repo = f.repo(); let token = Cancel::default(); let target = Target { remote: "origin".into(), branch: "main".into() };
    assert!(!repo.merge_update_save_needed(&target, &token).unwrap());
    f.write("untracked", "local");
    assert!(!repo.merge_update_save_needed(&target, &token).unwrap());
    f.write("other", "local");
    assert!(!repo.merge_update_save_needed(&target, &token).unwrap());
    f.git(&["add", "other"]);
    assert!(repo.merge_update_save_needed(&target, &token).unwrap());
    f.git(&["reset", "HEAD", "--", "other"]);
    f.write("shared", "local");
    assert!(repo.merge_update_save_needed(&target, &token).unwrap());
    f.git(&["checkout", "--", "shared"]);
    f.git(&["mv", "old", "renamed"]);
    assert!(repo.merge_update_save_needed(&target, &token).unwrap());
    f.git(&["reset", "--hard", "HEAD"]);
    f.write("other", "preserve");
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"]);
    assert!(repo.merge_update_save_needed(&target, &token).unwrap());
    assert_eq!(fs::read_to_string(f.0.join("other")).unwrap(), "preserve");
    assert_eq!(f.git(&["stash", "list"]), "");
    token.cancel(); assert!(repo.merge_update_save_needed(&target, &token).is_err());
}

#[test]
fn merge_update_flow_fetches_preserves_and_blocks_push_on_incomplete_recovery() {
    use plico::{merge_update::{self, Request}, pull::Target, update_session::UpdateResult};
    for mode in ["nothing", "fast-forward", "saved-merge", "merge-conflict", "restore-conflict", "fetch-failed"] {
        let remote = Fixture::new(); remote.write("shared", "base\n"); remote.write("local", "base\n"); remote.commit("base");
        let f = Fixture::new(); f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
        f.git(&["fetch", "origin"]); f.git(&["reset", "--hard", "origin/main"]);
        f.git(&["branch", "--set-upstream-to=origin/main", "main"]);
        if mode != "nothing" { remote.write("shared", "incoming\n"); remote.commit("incoming"); }
        let incoming = remote.git(&["rev-parse", "HEAD"]);
        if mode == "saved-merge" { f.write("ours", "ours\n"); f.commit("ours"); }
        if mode == "merge-conflict" { f.write("shared", "ours\n"); f.commit("ours"); }
        if mode == "restore-conflict" { f.write("shared", "valuable local\n"); }
        else { f.write("local", "valuable local\n"); }
        if matches!(mode, "saved-merge" | "merge-conflict") { f.git(&["add", "local"]); }
        f.write("untracked", "keep\n");
        if mode == "fetch-failed" { f.git(&["remote", "set-url", "origin", f.0.join("missing-remote").to_str().unwrap()]); }
        let before = f.git(&["rev-parse", "HEAD"]);
        let request = Request { branch: "main".into(), head: before.clone(), target: Target { remote: "origin".into(), branch: "main".into() }, reinstate_index: true };
        let report = merge_update::execute(&f.repo(), &request, std::ffi::OsStr::new("true"), &Cancel::default(), &Default::default(), None).unwrap();
        assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep\n");
        match mode {
            "nothing" => { assert_eq!(report.result, UpdateResult::NothingToUpdate); assert!(report.ready_to_push()); assert!(report.save.is_none()); assert!(report.merge.is_none()); }
            "fast-forward" => { assert!(report.ready_to_push()); assert!(report.save.is_none()); assert_eq!(report.head.unwrap(), incoming); }
            "saved-merge" => { assert!(report.ready_to_push()); assert!(report.save.unwrap().unwrap().status.success());
                assert_eq!(f.git(&["rev-parse", "HEAD^1"]), before); assert_eq!(f.git(&["rev-parse", "HEAD^2"]), incoming);
                assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "local"); }
            "merge-conflict" => { assert_eq!(report.result, UpdateResult::Incomplete); assert!(!report.ready_to_push());
                assert!(report.restore.unwrap().unwrap().is_none()); assert!(report.saved_oid.is_some());
                assert_eq!(f.git(&["show", "refs/stash:local"]), "valuable local"); }
            "restore-conflict" => { assert_eq!(report.result, UpdateResult::Success); assert!(!report.ready_to_push());
                assert!(!report.restore.unwrap().unwrap().unwrap().status.success()); assert!(report.saved_oid.is_some());
                assert_eq!(f.git(&["show", "refs/stash:shared"]), "valuable local"); }
            "fetch-failed" => { assert_eq!(report.result, UpdateResult::NotReady); assert!(!report.ready_to_push());
                assert!(report.save.is_none()); assert!(report.merge.is_none()); assert_eq!(report.head.unwrap(), before); }
            _ => unreachable!(),
        }
        if !matches!(mode, "merge-conflict" | "restore-conflict") {
            assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "valuable local\n");
            assert!(f.git(&["stash", "list"]).is_empty());
        }
    }
}

#[test]
fn rebase_update_flow_preserves_changes_and_obeys_decision_order() {
    use plico::{merge_update::Request, rebase_update, pull::Target, update_session::UpdateResult};
    for mode in ["nothing", "fast-forward", "clean", "saved", "conflict", "published-cancel", "published-accept", "unknown", "gate-cancel", "fetch-failed"] {
        let remote = Fixture::new(); remote.write("shared", "base\n"); remote.write("local", "base\n"); remote.commit("base");
        let f = Fixture::new(); f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
        f.git(&["fetch", "origin"]); f.git(&["reset", "--hard", "origin/main"]);
        f.git(&["branch", "--set-upstream-to=origin/main", "main"]);
        if mode != "nothing" { remote.write("shared", "incoming\n"); remote.commit("incoming"); }
        let incoming = remote.git(&["rev-parse", "HEAD"]);
        if !matches!(mode, "nothing" | "fast-forward") {
            f.write(if mode == "conflict" { "shared" } else { "ours" }, "ours\n"); f.commit("ours");
        }
        let before = f.git(&["rev-parse", "HEAD"]);
        if mode != "clean" { f.write("local", "valuable staged\n"); f.git(&["add", "local"]); }
        f.write("untracked", "keep\n");
        let mut protected = vec![];
        if mode.starts_with("published") {
            remote.git(&["fetch", f.0.to_str().unwrap(), "main:refs/heads/protected"]);
            protected.push(Target { remote: "origin".into(), branch: "protected".into() });
        }
        if mode == "unknown" { protected.push(Target { remote: "origin".into(), branch: "missing".into() }); }
        if mode == "fetch-failed" { f.git(&["remote", "set-url", "origin", f.0.join("missing-remote").to_str().unwrap()]); }
        let request = Request { branch: "main".into(), head: before.clone(),
            target: Target { remote: "origin".into(), branch: "main".into() }, reinstate_index: true };
        let repo = f.repo(); let mut gated = false; let mut confirmed = false;
        let report = rebase_update::execute(&repo, &request, std::ffi::OsStr::new("true"),
            &Cancel::default(), &Default::default(), None, plico::published_commits::Protection::Explicit(&protected),
            |over_merge| { gated = true; assert_eq!(over_merge.as_ref().unwrap(), &false); assert!(f.git(&["stash", "list"]).is_empty()); Ok((mode != "gate-cancel").then_some(plico::update_method::Method::Rebase)) },
            |check| { confirmed = true;
                assert!(!f.git(&["stash", "list"]).is_empty());
                if mode == "unknown" { assert!(check.is_err()); } else { assert!(check.as_ref().unwrap().is_some()); }
                mode == "published-accept"
            }).unwrap();
        assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep\n");
        assert_eq!(confirmed, mode.starts_with("published") || mode == "unknown");
        assert_eq!(gated, !matches!(mode, "nothing" | "fast-forward" | "fetch-failed"));
        match mode {
            "nothing" => { assert_eq!(report.result, UpdateResult::NothingToUpdate); assert!(report.ready_to_push()); assert!(report.fast_forward.is_none()); }
            "fast-forward" => { assert!(report.ready_to_push()); assert!(report.fast_forward.unwrap().outcome.status.success());
                assert!(report.save.is_none()); assert_eq!(report.head.unwrap(), incoming); }
            "clean" | "saved" | "published-accept" => {
                assert_eq!(report.result, UpdateResult::Success); assert!(report.ready_to_push());
                assert_eq!(f.git(&["rev-parse", "HEAD^"]), incoming);
                assert_eq!(fs::read_to_string(f.0.join("ours")).unwrap(), "ours\n");
                assert_eq!(f.git(&["diff", "--cached", "--name-only"]), if mode == "clean" { "" } else { "local" });
                assert!(report.saved_oid.is_none()); assert_eq!(report.save.is_some(), mode != "clean");
            }
            "conflict" => { assert_eq!(report.result, UpdateResult::Incomplete); assert!(!report.ready_to_push());
                assert!(report.state.unwrap().in_progress()); assert_eq!(f.git(&["show", "refs/stash:local"]), "valuable staged"); }
            "published-cancel" | "unknown" => { assert_eq!(report.result, UpdateResult::Cancelled); assert!(!report.ready_to_push());
                assert!(report.rebase.is_none()); assert!(report.saved_oid.is_some()); assert_eq!(report.head.unwrap(), before);
                assert_eq!(f.git(&["show", "refs/stash:local"]), "valuable staged"); }
            "gate-cancel" => { assert_eq!(report.result, UpdateResult::Cancelled); assert!(!report.ready_to_push());
                assert!(report.save.is_none()); assert!(report.rebase.is_none()); assert_eq!(report.head.unwrap(), before); }
            "fetch-failed" => { assert_eq!(report.result, UpdateResult::NotReady); assert!(!report.ready_to_push()); assert!(report.save.is_none()); }
            _ => unreachable!(),
        }
    }
}

#[test]
fn rebase_over_merge_checks_combined_changes_not_ordinary_merge_nodes() {
    use plico::pull::Target;
    for mode in ["ordinary", "extra", "resolution", "included"] {
        let f = Fixture::new(); f.write("shared", "base\n"); f.commit("base");
        let base = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "-b", "side"]);
        f.write(if mode == "resolution" { "shared" } else { "side" }, "side\n"); f.commit("side");
        f.git(&["checkout", "main"]);
        f.write(if mode == "resolution" { "shared" } else { "main" }, "main\n"); f.commit("main");
        let merge = std::process::Command::new("git").arg("-C").arg(&f.0)
            .args(["merge", "--no-commit", "--no-ff", "side"]).output().unwrap();
        assert_eq!(merge.status.success(), mode != "resolution");
        if mode == "resolution" { f.write("shared", "resolved differently from both parents\n"); }
        if mode == "extra" || mode == "included" { f.write("额外 file", "extra merge edit\n"); }
        f.commit("merge");
        let head = f.git(&["rev-parse", "HEAD"]);
        f.git(&["remote", "add", "origin", "https://example.invalid/repo"]);
        f.git(&["update-ref", "refs/remotes/origin/main", if mode == "included" { &head } else { &base }]);
        let repo = f.repo(); let target = Target { remote: "origin".into(), branch: "main".into() };
        // A first-parent diff exists even for an ordinary merge; it must not warn.
        assert!(!f.git(&["diff", "HEAD^", "HEAD", "--name-only"]).is_empty());
        assert_eq!(repo.rebase_over_merge_problem(&target, &Cancel::default()).unwrap(), matches!(mode, "extra" | "resolution"));
        assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
        assert!(f.git(&["status", "--porcelain"]).is_empty());
        let token = Cancel::default(); token.cancel();
        assert!(repo.rebase_over_merge_problem(&target, &token).is_err());
        assert!(repo.rebase_over_merge_problem(&Target { remote: "origin".into(), branch: "missing".into() }, &Cancel::default()).is_err());
    }
}

#[test]
fn rebase_update_rejects_changed_refs_after_decisions_without_losing_saved_work() {
    use plico::{merge_update::Request, rebase_update, pull::Target, update_session::UpdateResult};
    for stage in ["before-save", "published"] {
        for changed in ["target", "protected", "new-ref"] {
            let remote = Fixture::new(); remote.write("shared", "base\n"); remote.write("local", "base\n"); remote.commit("base");
            let f = Fixture::new(); f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
            f.git(&["fetch", "origin"]); f.git(&["reset", "--hard", "origin/main"]);
            f.git(&["branch", "--set-upstream-to=origin/main", "main"]);
            remote.write("shared", "incoming\n"); remote.commit("incoming");
            let incoming = remote.git(&["rev-parse", "HEAD"]);
            f.write("ours", "ours\n"); f.commit("ours");
            let before = f.git(&["rev-parse", "HEAD"]);
            remote.git(&["fetch", f.0.to_str().unwrap(), "main:refs/heads/protected"]);
            f.write("local", "valuable staged\n"); f.git(&["add", "local"]);
            let change = || {
                let reference = match changed {
                    "target" => "refs/remotes/origin/main",
                    "protected" => "refs/remotes/origin/protected",
                    _ => "refs/remotes/origin/new-ref",
                };
                let oid = if changed == "target" { &before } else { &incoming };
                f.git(&["update-ref", reference, oid]);
            };
            let request = Request { branch: "main".into(), head: before.clone(),
                target: Target { remote: "origin".into(), branch: "main".into() }, reinstate_index: true };
            let mut prompted = false;
            let report = rebase_update::execute(&f.repo(), &request, std::ffi::OsStr::new("true"),
                &Cancel::default(), &Default::default(), None,
                plico::published_commits::Protection::Explicit(&[Target { remote: "origin".into(), branch: "protected".into() }]),
                |_| { if stage == "before-save" { change(); } Ok(Some(plico::update_method::Method::Rebase)) },
                |_| { prompted = true; assert_eq!(stage, "published"); change(); true }).unwrap();
            assert_eq!(prompted, stage == "published");
            assert_eq!(report.result, UpdateResult::Error);
            assert!(!report.ready_to_push());
            assert!(report.error.unwrap().contains("Remote references changed"));
            assert!(report.rebase.is_none()); assert_eq!(report.head.unwrap(), before);
            assert!(!report.state.unwrap().in_progress());
            if stage == "before-save" {
                assert!(report.save.is_none()); assert!(report.saved_oid.is_none());
                assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "local");
                assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "valuable staged\n");
            } else {
                assert!(report.saved_oid.is_some());
                assert_eq!(f.git(&["show", "refs/stash:local"]), "valuable staged");
                assert!(f.git(&["diff", "--cached", "--name-only"]).is_empty());
            }
        }
    }
}

#[test]
fn rejected_push_flow_updates_retries_or_preserves_conflict_and_cancel() {
    use plico::{push::{Spec, Status}, push_flow::{self, Decisions}, push_target::{Target, Origin}, update_method::Method};
    struct Choice { mode: &'static str, asked: usize }
    impl Decisions for Choice {
        fn rejected(&mut self, default: Method, problem: &Result<bool>) -> Option<Method> {
            self.asked += 1; assert_eq!(default, Method::Merge); assert_eq!(problem.as_ref().unwrap(), &false);
            match self.mode { "cancel" => None, "rebase" => Some(Method::Rebase), _ => Some(Method::Merge) }
        }
        fn rebase_over_merge(&mut self, _: &Result<bool>) -> Result<Option<Method>> { panic!("Unexpected second warning") }
        fn published(&mut self, _: &Result<Option<plico::pull::Target>>) -> bool { panic!("Unexpected published warning") }
    }
    for mode in ["merge", "rebase", "cancel", "conflict"] {
        let remote = Fixture::new(); remote.git(&["config", "core.bare", "true"]);
        let f = Fixture::new(); f.write("shared", "base\n"); f.write("dirty", "base\n"); f.commit("base");
        let base = f.git(&["rev-parse", "HEAD"]);
        f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]); f.git(&["push", "-u", "origin", "main"]);
        f.write("shared", "incoming\n"); f.commit("incoming"); f.git(&["push", "origin", "main"]);
        let incoming = f.git(&["rev-parse", "HEAD"]);
        f.git(&["reset", "--hard", &base]);
        f.write(if mode == "conflict" { "shared" } else { "ours" }, "ours\n"); f.commit("ours");
        let before = f.git(&["rev-parse", "HEAD"]);
        f.write("dirty", "valuable staged\n"); f.git(&["add", "dirty"]); f.write("untracked", "keep\n");
        let spec = Spec { tags: None, force: false, source_branch: "main".into(), source_oid: before.clone(), set_upstream: false,
            target: Target { remote: "origin".into(), branch: "main".into(), special: false, new_branch: false, origin: Origin::TrackingBranch } };
        let mut choices = Choice { mode, asked: 0 };
        f.git(&["config", "pull.rebase", "true"]);
        let report = push_flow::execute(&f.repo(), spec, std::ffi::OsStr::new("true"), &Cancel::default(),
            &Default::default(), None, Default::default(), plico::published_commits::Protection::Defaults, &mut choices);
        assert!(report.error.is_none(), "{:?}", report.error); assert_eq!(choices.asked, 1);
        assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep\n");
        if mode == "cancel" || mode == "conflict" {
            assert_eq!(report.attempts, 1); assert_eq!(report.push.unwrap().status, Status::RejectedNonFf);
            assert_eq!(remote.git(&["rev-parse", "main"]), incoming);
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
            assert_eq!(report.cancelled_decision, mode == "cancel");
            if mode == "conflict" {
                assert!(f.repo().operation_state(&Cancel::default()).unwrap().in_progress());
                assert_eq!(f.git(&["show", "refs/stash:dirty"]), "valuable staged");
            } else { assert!(report.update.is_none()); assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "dirty"); }
        } else {
            assert_eq!(report.attempts, 2); assert_eq!(report.push.unwrap().status, Status::Success);
            assert_eq!(remote.git(&["rev-parse", "main"]), f.git(&["rev-parse", "HEAD"]));
            assert_eq!(f.git(&["rev-parse", if mode == "merge" { "HEAD^2" } else { "HEAD^" }]), incoming);
            assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "dirty");
            assert_eq!(fs::read_to_string(f.0.join("dirty")).unwrap(), "valuable staged\n");
        }
    }
}

#[test]
fn rebase_update_merge_instead_uses_merge_save_rules_and_preserves_history() {
    use plico::{merge_update::Request, rebase_update, pull::Target, update_method::Method, update_session::UpdateResult};
    for mode in ["unstaged", "staged", "conflict"] {
        let remote = Fixture::new(); remote.write("shared", "base\n"); remote.write("local", "base\n"); remote.commit("base");
        let f = Fixture::new(); f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
        f.git(&["fetch", "origin"]); f.git(&["reset", "--hard", "origin/main"]);
        f.git(&["branch", "--set-upstream-to=origin/main", "main"]);
        f.git(&["checkout", "-b", "side"]); f.write("side", "side\n"); f.commit("side");
        f.git(&["checkout", "main"]);
        f.write(if mode == "conflict" { "shared" } else { "ours" }, "ours\n"); f.commit("ours");
        f.git(&["merge", "--no-commit", "--no-ff", "side"]);
        f.write("merge-only", "extra change\n"); f.commit("merge with extra change");
        let before = f.git(&["rev-parse", "HEAD"]);
        remote.write("shared", "incoming\n"); remote.commit("incoming");
        let incoming = remote.git(&["rev-parse", "HEAD"]);
        f.write("local", "valuable local\n");
        if mode != "unstaged" { f.git(&["add", "local"]); }
        let request = Request { branch: "main".into(), head: before.clone(),
            target: Target { remote: "origin".into(), branch: "main".into() }, reinstate_index: true };
        let mut asked = 0;
        let report = rebase_update::execute(&f.repo(), &request, std::ffi::OsStr::new("true"),
            &Cancel::default(), &Default::default(), None, plico::published_commits::Protection::Explicit(&[]),
            |problem| { assert_eq!(problem.as_ref().unwrap(), &true); asked += 1; Ok(Some(Method::Merge)) },
            |_| panic!("Merge must not ask about rewriting published commits")).unwrap();
        assert_eq!(asked, 1); assert!(report.error.is_none(), "{:?}", report.error);
        assert!(report.rebase.is_none()); assert!(report.merge.is_some()); assert!(report.published.is_none());
        assert_eq!(report.save.is_some(), mode != "unstaged");
        if mode == "conflict" {
            assert_eq!(report.result, UpdateResult::Incomplete); assert!(!report.ready_to_push());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
            assert_eq!(f.git(&["show", "refs/stash:local"]), "valuable local");
            assert!(report.saved_oid.is_some());
        } else {
            assert!(report.ready_to_push()); assert!(report.saved_oid.is_none());
            assert_eq!(f.git(&["rev-parse", "HEAD^1"]), before);
            assert_eq!(f.git(&["rev-parse", "HEAD^2"]), incoming);
            assert_eq!(fs::read_to_string(f.0.join("merge-only")).unwrap(), "extra change\n");
            assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "valuable local\n");
            assert_eq!(f.git(&["diff", "--cached", "--name-only"]), if mode == "staged" { "local" } else { "" });
        }
    }
}

#[test]
fn push_update_preferences_are_private_per_worktree_and_not_inherited_by_clone() {
    use plico::update_preferences::{self, Preferences, Strategy};
    let f = Fixture::new(); f.write("tracked", "base\n"); f.commit("base");
    let original_head = f.git(&["rev-parse", "HEAD"]);
    let config = fs::read(f.0.join(".git/config")).unwrap();
    let (path, defaults) = update_preferences::for_repository(&f.repo(), &Cancel::default()).unwrap();
    assert_eq!(defaults, Preferences::default()); assert!(!path.exists()); assert!(!path.parent().unwrap().exists());
    let remembered = Preferences { strategy: Strategy::Rebase, auto_update: true };
    update_preferences::save(&path, remembered).unwrap();
    assert_eq!(update_preferences::for_repository(&f.repo(), &Cancel::default()).unwrap().1, remembered);
    assert_eq!(fs::read(f.0.join(".git/config")).unwrap(), config);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), original_head);
    assert!(f.git(&["status", "--porcelain", "--untracked-files=all"]).is_empty());
    let linked = f.0.join("linked-worktree");
    f.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
    let linked_repo = Repository::new(&linked);
    let (linked_path, linked_defaults) = update_preferences::for_repository(&linked_repo, &Cancel::default()).unwrap();
    assert_ne!(linked_path, path); assert_eq!(linked_defaults, Preferences::default());
    update_preferences::save(&linked_path, Preferences { strategy: Strategy::BranchDefault, auto_update: false }).unwrap();
    assert_eq!(update_preferences::load(&path), remembered);
    let clone_path = f.0.join("clone-check");
    f.git(&["clone", "--no-local", f.0.to_str().unwrap(), clone_path.to_str().unwrap()]);
    let (clone_settings, clone_defaults) = update_preferences::for_repository(&Repository::new(&clone_path), &Cancel::default()).unwrap();
    assert_eq!(clone_defaults, Preferences::default()); assert!(!clone_settings.exists());
    let token = Cancel::default(); token.cancel();
    assert!(update_preferences::for_repository(&f.repo(), &token).is_err());
    assert_eq!(update_preferences::load(&path), remembered);
}

#[test]
fn auto_push_rebase_discovers_protected_branches_created_before_fetch() {
    use plico::{push::{Spec, Status}, push_flow::{self, Decisions}, push_target::{Target, Origin},
        update_method::Method, update_preferences::{Preferences, Strategy}, published_commits::Protection};
    struct Choice { protected: bool, accept: bool, asked: usize }
    impl Decisions for Choice {
        fn rejected(&mut self, _: Method, _: &Result<bool>) -> Option<Method> { panic!("Auto-update must skip initial choice") }
        fn rebase_over_merge(&mut self, _: &Result<bool>) -> Result<Option<Method>> { panic!("No merge commits") }
        fn published(&mut self, result: &Result<Option<plico::pull::Target>>) -> bool {
            self.asked += 1; assert!(self.protected);
            let found = result.as_ref().unwrap().as_ref().unwrap();
            assert_eq!(found.remote, "origin"); assert_eq!(found.branch, "master"); self.accept
        }
    }
    for (branch, accept) in [("master", false), ("master", true), ("review", false)] {
        let remote = Fixture::new(); remote.git(&["config", "core.bare", "true"]);
        let f = Fixture::new(); f.write("base", "base\n"); f.write("dirty", "original\n"); f.commit("base");
        let base = f.git(&["rev-parse", "HEAD"]);
        f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]); f.git(&["push", "-u", "origin", "main"]);
        f.write("incoming", "incoming\n"); f.commit("incoming"); f.git(&["push", "origin", "main"]);
        let incoming = f.git(&["rev-parse", "HEAD"]);
        f.git(&["reset", "--hard", &base]); f.write("ours", "ours\n"); f.commit("ours");
        let ours = f.git(&["rev-parse", "HEAD"]);
        // The remote has a new branch publishing ours, but the client's refs
        // cannot see it until the update's Fetch (no client-side push/fetch here).
        remote.git(&["fetch", f.0.to_str().unwrap(), &format!("main:refs/heads/{branch}")]);
        let repo = f.repo(); let token = Cancel::default();
        assert!(!repo.pull_targets(&token).unwrap().branches.iter().any(|t| t.branch == branch));
        let update_target = plico::pull::Target { remote: "origin".into(), branch: "main".into() };
        assert_eq!(Protection::Defaults.check(&repo, &update_target, &token).unwrap(), None);
        f.write("dirty", "valuable staged\n"); f.git(&["add", "dirty"]);
        let spec = Spec { tags: None, force: false, source_branch: "main".into(), source_oid: ours.clone(), set_upstream: false,
            target: Target { remote: "origin".into(), branch: "main".into(), special: false, new_branch: false, origin: Origin::TrackingBranch } };
        let mut choices = Choice { protected: branch == "master", accept, asked: 0 };
        let report = push_flow::execute(&repo, spec, std::ffi::OsStr::new("true"), &token, &Default::default(), None,
            Preferences { strategy: Strategy::Rebase, auto_update: true }, Protection::Defaults, &mut choices);
        assert!(report.error.is_none(), "{:?}", report.error);
        assert_eq!(choices.asked, usize::from(branch == "master"));
        assert!(repo.pull_targets(&token).unwrap().branches.iter().any(|t| t.branch == branch));
        assert_eq!(remote.git(&["rev-parse", branch]), ours);
        if branch == "master" && !accept {
            assert_eq!(report.attempts, 1); assert_eq!(report.push.unwrap().status, Status::RejectedNonFf);
            assert_eq!(f.git(&["rev-parse", "HEAD"]), ours); assert_eq!(remote.git(&["rev-parse", "main"]), incoming);
            assert_eq!(f.git(&["show", "refs/stash:dirty"]), "valuable staged");
            assert!(report.update.as_ref().unwrap().cancelled());
            assert!(report.update.as_ref().unwrap().has_saved_changes());
            let Some(push_flow::Update::Rebase(update)) = report.update else { panic!("Expected Rebase report") };
            assert!(update.rebase.is_none()); assert!(!update.ready_to_push());
        } else {
            assert_eq!(report.attempts, 2); assert_eq!(report.push.unwrap().status, Status::Success);
            assert_eq!(f.git(&["rev-parse", "HEAD^"]), incoming);
            assert_eq!(f.git(&["rev-parse", "HEAD"]), remote.git(&["rev-parse", "main"]));
            assert_eq!(f.git(&["diff", "--cached", "--name-only"]), "dirty");
            assert_eq!(fs::read_to_string(f.0.join("dirty")).unwrap(), "valuable staged\n");
            assert!(f.git(&["stash", "list"]).is_empty());
        }
    }
}

#[test]
fn stash_view_keeps_identity_index_and_untracked_content() {
    use plico::stash;
    let f = Fixture::new();
    let repo = f.repo(); let token = Cancel::default();
    assert!(stash::list(&repo, 10, &token).unwrap().entries.is_empty());
    assert!(stash::list(&repo, 0, &token).is_err());
    f.write("tracked.txt", "base\n"); f.commit("base");
    f.write("tracked.txt", "index version\n"); f.git(&["add", "tracked.txt"]);
    f.write("tracked.txt", "working version\n");
    f.write("untracked.txt", "saved untracked\n");
    f.git(&["stash", "push", "-u", "-m", "中文: keep colon"]);
    let stack = stash::list(&repo, 10, &token).unwrap();
    assert_eq!(stack.entries.len(), 1); assert!(!stack.more);
    let saved = &stack.entries[0];
    assert_eq!(saved.selector, "stash@{0}");
    assert_eq!(saved.message, "中文: keep colon");
    assert_eq!(saved.parents.len(), 3);
    f.write("tracked.txt", "later stash\n"); f.git(&["stash", "push", "-m", "later"]);
    let limited = stash::list(&repo, 1, &token).unwrap();
    assert!(limited.more); assert_eq!(limited.entries.len(), 1);
    assert_ne!(limited.entries[0].oid, saved.oid);
    let before = f.git(&["stash", "list"]);
    let data = stash::contents(&repo, saved, &token).unwrap();
    assert_eq!(data.commit.id, saved.oid);
    assert_eq!(data.working.base, saved.parents[0]);
    assert_eq!(data.working.files.len(), 1);
    assert!(repo.full_patch(Some(&data.commit), &data.working.files[0], &token).unwrap().contains("+working version"));
    assert_eq!(data.parents.len(), 2);
    let (index, details) = &data.parents[0];
    assert!(repo.full_patch(Some(index), &details.files[0], &token).unwrap().contains("+index version"));
    let (untracked, details) = &data.parents[1];
    assert!(repo.full_patch(Some(untracked), &details.files[0], &token).unwrap().contains("+saved untracked"));
    assert_eq!(f.git(&["stash", "list"]), before);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
    token.cancel();
    assert!(stash::list(&repo, 10, &token).is_err());
    assert!(stash::contents(&repo, saved, &token).is_err());
}

#[test]
fn explicit_stash_apply_pop_preserve_index_and_failures() {
    use plico::{stash, stash_operation::{self, Action, Status}, operation_process::Progress};
    for mode in ["apply", "pop", "index", "index-conflict", "conflict", "dirty", "untracked", "stale", "cancel"] {
        let f = Fixture::new();
        f.write("tracked.txt", "base\n"); f.commit("base");
        f.write("tracked.txt", "staged\n"); f.git(&["add", "tracked.txt"]);
        f.write("tracked.txt", "working\n");
        f.write("new.txt", "saved new\n");
        f.git(&["stash", "push", "-u", "-m", "saved"]);
        let repo = f.repo(); let token = Cancel::default();
        let entry = stash::list(&repo, 10, &token).unwrap().entries.remove(0);
        match mode {
            "conflict" | "index-conflict" => { f.write("tracked.txt", "incoming\n"); f.commit("incoming"); }
            "dirty" => f.write("tracked.txt", "valuable dirty\n"),
            "untracked" => f.write("new.txt", "valuable untracked\n"),
            "stale" => { f.write("tracked.txt", "newer saved\n"); f.git(&["stash", "push", "-m", "newer"]); }
            "cancel" => token.cancel(), _ => {}
        }
        let head = f.git(&["rev-parse", "HEAD"]);
        let status_before = f.git(&["status", "--porcelain"]);
        let stashes_before = f.git(&["stash", "list"]);
        let result = stash_operation::execute(&repo, &entry, if mode == "apply" { Action::Apply } else { Action::Pop },
            matches!(mode, "index" | "index-conflict"), &token, &Progress::default());
        if matches!(mode, "stale" | "cancel") {
            assert!(result.is_err());
            assert_eq!(f.git(&["status", "--porcelain"]), status_before);
            assert_eq!(f.git(&["stash", "list"]), stashes_before);
            continue;
        }
        let result = result.unwrap();
        assert_eq!(f.git(&["rev-parse", "HEAD"]), head, "{mode}");
        match mode {
            "apply" | "pop" | "index" => {
                assert_eq!(result.status, Status::Success, "{mode}: {:?}", result.command);
                assert_eq!(fs::read_to_string(f.0.join("tracked.txt")).unwrap(), "working\n");
                assert_eq!(fs::read_to_string(f.0.join("new.txt")).unwrap(), "saved new\n");
                assert_eq!(f.git(&["show", ":tracked.txt"]), if mode == "index" { "staged" } else { "base" });
                assert_eq!(stash::list(&repo, 10, &Cancel::default()).unwrap().entries.len(), if mode == "apply" { 1 } else { 0 });
            }
            "index-conflict" => { assert_eq!(result.status, Status::IndexConflict); }
            "conflict" => { assert_eq!(result.status, Status::Conflicts); assert!(!result.state.unwrap().conflicts.is_empty()); }
            "dirty" => { assert_eq!(result.status, Status::LocalChanges); assert_eq!(fs::read_to_string(f.0.join("tracked.txt")).unwrap(), "valuable dirty\n"); }
            "untracked" => { assert_ne!(result.status, Status::Success); assert_eq!(fs::read_to_string(f.0.join("new.txt")).unwrap(), "valuable untracked\n"); }
            _ => unreachable!(),
        }
        if matches!(mode, "conflict" | "index-conflict" | "dirty" | "untracked") { assert_eq!(f.git(&["stash", "list"]), stashes_before); }
    }
}

#[test]
fn explicit_pull_rebase_uses_selected_target_without_rewriting_tracking() {
    use plico::{merge_update::Request, rebase_update, pull::Target,
        published_commits::Protection, update_method::Method, update_session::UpdateResult};
    for mode in ["no-upstream", "other-upstream", "head-changed", "target-removed", "conflict"] {
        let remote = Fixture::new(); remote.git(&["config", "core.bare", "true"]);
        let author = Fixture::new(); author.write("shared", "base\n"); author.write("local", "base\n"); author.commit("base");
        author.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
        author.git(&["push", "origin", "main", "main:topic"]);
        let f = Fixture::new(); f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
        f.git(&["fetch", "origin"]); f.git(&["reset", "--hard", "origin/main"]);
        if mode != "no-upstream" { f.git(&["branch", "--set-upstream-to=origin/main", "main"]); }
        author.write("shared", "incoming\n"); author.commit("incoming"); author.git(&["push", "origin", "main:topic"]);
        let incoming = author.git(&["rev-parse", "HEAD"]);
        f.write(if mode == "conflict" { "shared" } else { "ours" }, "ours\n"); f.commit("ours");
        let original = f.git(&["rev-parse", "HEAD"]);
        f.write("local", "valuable staged\n"); f.git(&["add", "local"]);
        f.write("local", "valuable unstaged\n"); f.write("untracked", "keep\n");
        let request = Request { branch: "main".into(), head: original.clone(),
            target: Target { remote: "origin".into(), branch: "topic".into() }, reinstate_index: true };
        // Push recovery must still reject a non-tracking target, before Fetch/save.
        assert!(rebase_update::execute(&f.repo(), &request, std::ffi::OsStr::new("true"),
            &Cancel::default(), &Default::default(), None, Protection::Explicit(&[]),
            |_| panic!("tracking guard bypassed"), |_| panic!("tracking guard bypassed")).is_err());
        assert_eq!(f.git(&["rev-parse", "HEAD"]), original);
        assert!(f.git(&["stash", "list"]).is_empty());
        let report = rebase_update::execute_selected(&f.repo(), &request, std::ffi::OsStr::new("true"),
            &Cancel::default(), &Default::default(), None, Protection::Explicit(&[]),
            |problem| {
                assert_eq!(problem.as_ref().unwrap(), &false);
                if mode == "head-changed" { f.git(&["reset", "--soft", "HEAD^"]); }
                if mode == "target-removed" { f.git(&["update-ref", "-d", "refs/remotes/origin/topic"]); }
                Ok(Some(Method::Rebase))
            }, |_| panic!("no protected branches")).unwrap();
        if mode != "conflict" { assert_eq!(f.git(&["symbolic-ref", "--short", "HEAD"]), "main"); }
        // During a rebase conflict HEAD is detached; inspect the original branch's
        // tracking configuration, not the currently attached branch (there is none).
        assert_eq!(f.git(&["for-each-ref", "--format=%(upstream:short)", "refs/heads/main"]),
            if mode == "no-upstream" { "" } else { "origin/main" });
        assert_eq!(remote.git(&["rev-parse", "topic"]), incoming);
        assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep\n");
        match mode {
            "head-changed" | "target-removed" => {
                assert_eq!(report.result, UpdateResult::Error);
                assert!(report.error.is_some()); assert!(report.save.is_none()); assert!(report.rebase.is_none());
                assert!(f.git(&["stash", "list"]).is_empty());
                assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "valuable unstaged\n");
            }
            "conflict" => {
                assert_eq!(report.result, UpdateResult::Incomplete);
                assert!(report.saved_oid.is_some()); assert!(!report.state.unwrap().conflicts.is_empty());
                assert_eq!(f.git(&["show", "refs/stash:local"]), "valuable unstaged");
                assert_eq!(f.git(&["show", "refs/stash^2:local"]), "valuable staged");
            }
            _ => {
                assert_eq!(report.result, UpdateResult::Success); assert!(report.ready_to_push());
                assert_eq!(f.git(&["rev-parse", "HEAD^"]), incoming);
                assert_eq!(f.git(&["show", ":local"]), "valuable staged");
                assert_eq!(fs::read_to_string(f.0.join("local")).unwrap(), "valuable unstaged\n");
                assert!(f.git(&["stash", "list"]).is_empty());
            }
        }
    }
}

#[test]
fn pull_source_requires_an_existing_attached_head_and_obeys_cancellation() {
    let f = Fixture::new(); let token = Cancel::default();
    assert!(plico::pull::source(&f.repo(), &token).unwrap().is_none());
    f.write("file", "base\n"); f.commit("base");
    let head = f.git(&["rev-parse", "HEAD"]);
    let source = plico::pull::source(&f.repo(), &token).unwrap().unwrap();
    assert_eq!(source.branch, "main"); assert_eq!(source.head, head);
    f.git(&["checkout", "--detach", &head]);
    assert!(plico::pull::source(&f.repo(), &token).unwrap().is_none());
    token.cancel(); assert!(plico::pull::source(&f.repo(), &token).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn existing_rebase_can_continue_or_abort_without_original_session() {
    use plico::{rebase_operation::{self, Action}, operation_process::Progress};
    use std::ffi::OsStr;
    for mode in ["continue", "abort", "editor-failure", "cancel"] {
        let f = Fixture::new();
        f.write("shared.txt", "base\n");
        f.write("dirty.txt", "base\n"); f.commit("base");
        f.git(&["checkout", "-b", "incoming"]);
        f.write("shared.txt", "incoming\n"); f.commit("incoming");
        let incoming = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "main"]);
        f.write("shared.txt", "local\n"); f.commit("local");
        let before = f.git(&["rev-parse", "HEAD"]);
        f.write("dirty.txt", "valuable saved work\n"); f.git(&["stash", "push"]);
        let stash = f.git(&["rev-parse", "refs/stash"]);
        let token = Cancel::default(); let progress = Progress::default();
        assert!(rebase_operation::execute(&f.repo(), Action::Abort, &token, &progress).is_err());
        let output = Command::new("git").arg("-C").arg(&f.0)
            .args(["rebase", "incoming"]).output().unwrap();
        assert!(!output.status.success());
        // New Repository value: no live update Session or remembered rebase owner.
        let repo = f.repo();
        assert!(rebase_operation::execute(&repo, Action::Continue {
            editor: OsStr::new("true"), requests: None }, &token, &progress).is_err());
        if mode == "cancel" {
            token.cancel();
            assert!(rebase_operation::execute(&repo, Action::Abort, &token, &progress).is_err());
            assert!(repo.operation_state(&Cancel::default()).unwrap().in_progress());
        } else {
            if mode != "abort" {
                f.write("shared.txt", "resolved\n"); f.git(&["add", "shared.txt"]);
            }
            let action = if mode == "abort" { Action::Abort } else { Action::Continue {
                editor: OsStr::new(if mode == "editor-failure" { "false" } else { "true" }), requests: None } };
            let result = rebase_operation::execute(&repo, action, &token, &progress).unwrap();
            let result = Ok(result);
            assert_eq!(rebase_operation::diagnostic(&result).is_some(), mode == "editor-failure");
            let result = result.unwrap();
            assert_eq!(result.command.unwrap().status.success(), mode != "editor-failure");
            assert_eq!(result.state.unwrap().in_progress(), mode == "editor-failure");
            if mode == "continue" {
                assert_eq!(f.git(&["rev-parse", "HEAD^"]), incoming);
                assert_eq!(f.git(&["symbolic-ref", "--short", "HEAD"]), "main");
            } else if mode == "abort" {
                assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
            }
        }
        assert_eq!(f.git(&["rev-parse", "refs/stash"]), stash);
        assert_eq!(f.git(&["show", "refs/stash:dirty.txt"]), "valuable saved work");
        assert_eq!(fs::read_to_string(f.0.join("dirty.txt")).unwrap(), "base\n");
    }
}

#[test]
fn rebase_no_change_recovery_stage_failure_and_repeated_error_stop() {
    use plico::{rebase_operation::{self, Action}, operation_process::Progress};
    use std::ffi::OsStr;
    for mode in ["stage", "stage-failure", "repeat", "retry"] {
        let f = Fixture::new();
        f.write("shared.txt", "base\n"); f.commit("base");
        f.git(&["checkout", "-b", "incoming"]);
        f.write("shared.txt", "incoming\n"); f.commit("incoming");
        let incoming = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "main"]);
        f.write("shared.txt", "local\n"); f.commit("local");
        let output = Command::new("git").arg("-C").arg(&f.0).args(["rebase", "incoming"]).output().unwrap();
        assert!(!output.status.success());
        f.write("shared.txt", "resolved\n"); f.git(&["add", "shared.txt"]);
        f.write("untracked.txt", "keep untracked\n");
        // Fault injection through Git's real editor. Modern Git does not
        // naturally reach every legacy GitRebaser no-change recovery branch.
        let reset = if mode.starts_with("stage") { "git reset -q HEAD -- shared.txt || exit 90\n" } else { "" };
        let lock = if mode == "stage-failure" { "echo fixture > .git/index.lock\n" } else { "" };
        // A broken retry guard must fail the test instead of hanging forever.
        let retry = if mode == "repeat" { "if [ \"$calls\" -gt 4 ]; then exit 0; fi\n" }
            else { "if [ \"$calls\" -gt 1 ]; then exit 0; fi\n" };
        f.write(".git/editor-test.sh", &format!(r#"calls=0
if [ -f .git/editor-calls ]; then calls=$(cat .git/editor-calls); fi
calls=$((calls + 1))
echo "$calls" > .git/editor-calls
{retry}{reset}{lock}echo "No changes - did you forget to use 'git add'?" >&2
exit 1
"#));
        let command = "sh .git/editor-test.sh";
        let result = rebase_operation::execute(&f.repo(), Action::Continue {
            editor: OsStr::new(command), requests: None }, &Cancel::default(), &Progress::default()).unwrap();
        let outcome = result.command.unwrap();
        let success = matches!(mode, "stage" | "retry");
        assert_eq!(outcome.status.success(), success, "{mode}: {}\n{}", String::from_utf8_lossy(&outcome.stderr.bytes()), String::from_utf8_lossy(&outcome.stdout.bytes()));
        assert_eq!(result.state.unwrap().in_progress(), !success, "{mode}");
        assert_eq!(fs::read_to_string(f.0.join(".git/editor-calls")).unwrap().trim(), if mode == "stage-failure" { "1" } else { "2" }, "{mode}");
        if mode == "stage-failure" {
            assert!(f.0.join(".git/index.lock").is_file());
            fs::remove_file(f.0.join(".git/index.lock")).unwrap();
            assert!(f.git(&["diff", "--cached", "--name-only"]).is_empty());
            assert_eq!(f.git(&["diff", "--name-only"]), "shared.txt");
        }
        if success {
            assert_eq!(f.git(&["rev-parse", "HEAD^"]), incoming);
            assert_eq!(f.git(&["show", "HEAD:shared.txt"]), "resolved");
            assert_eq!(f.git(&["status", "--porcelain"]), "?? untracked.txt");
        } else {
            assert_eq!(f.git(&["rev-parse", "HEAD"]), incoming);
            if mode == "repeat" { assert_eq!(f.git(&["show", ":shared.txt"]), "resolved"); }
        }
        assert_eq!(fs::read_to_string(f.0.join("shared.txt")).unwrap(), "resolved\n");
        assert_eq!(fs::read_to_string(f.0.join("untracked.txt")).unwrap(), "keep untracked\n");
        assert_eq!(f.git(&["ls-files", "untracked.txt"]), "");
    }
}

#[test]
fn existing_rebase_apply_no_changes_recovers_without_adding_untracked_files() {
    use plico::{rebase_operation::{self, Action}, operation_process::Progress};
    use std::ffi::OsStr;
    for mode in ["skip", "tracked", "staged"] {
        let f = Fixture::new();
        f.write("shared.txt", "base\n"); f.write("tracked.txt", "base\n"); f.commit("base");
        f.git(&["checkout", "-b", "incoming"]);
        f.write("shared.txt", "incoming\n"); f.commit("incoming");
        let incoming = f.git(&["rev-parse", "HEAD"]);
        f.git(&["checkout", "main"]);
        f.write("shared.txt", "local\n"); f.commit("local");
        let output = Command::new("git").arg("-C").arg(&f.0).args(["rebase", "--apply", "incoming"]).output().unwrap();
        assert!(!output.status.success());
        // Resolve to the upstream content: the patch has become empty.
        f.write("shared.txt", "incoming\n"); f.git(&["add", "shared.txt"]);
        f.write("untracked.txt", "never stage this\n");
        if mode != "skip" {
            f.write("tracked.txt", "reviewed resolution\n");
            if mode == "staged" { f.git(&["add", "tracked.txt"]); }
        }
        let result = rebase_operation::execute(&f.repo(), Action::Continue {
            editor: OsStr::new("true"), requests: None }, &Cancel::default(), &Progress::default()).unwrap();
        let outcome = result.command.unwrap();
        if mode == "tracked" {
            // Modern Git rejects unstaged edits before emitting the no-change
            // indicator. Do not infer permission to stage from this other error.
            assert!(!outcome.status.success());
            assert!(result.state.unwrap().in_progress());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), incoming);
            assert!(f.git(&["diff", "--cached", "--name-only"]).is_empty());
            assert_eq!(fs::read_to_string(f.0.join("tracked.txt")).unwrap(), "reviewed resolution\n");
            assert_eq!(fs::read_to_string(f.0.join("untracked.txt")).unwrap(), "never stage this\n");
            continue;
        }
        assert!(outcome.status.success(), "{mode}: {}\n{}", String::from_utf8_lossy(&outcome.stderr.bytes()), String::from_utf8_lossy(&outcome.stdout.bytes()));
        assert!(!result.state.unwrap().in_progress(), "{mode}");
        assert_eq!(f.git(&["symbolic-ref", "--short", "HEAD"]), "main");
        if mode == "skip" { assert_eq!(f.git(&["rev-parse", "HEAD"]), incoming); }
        else {
            assert_eq!(f.git(&["rev-parse", "HEAD^"]), incoming);
            assert_eq!(f.git(&["show", "HEAD:tracked.txt"]), "reviewed resolution");
        }
        assert_eq!(f.git(&["status", "--porcelain"]), "?? untracked.txt");
        assert_eq!(fs::read_to_string(f.0.join("untracked.txt")).unwrap(), "never stage this\n");
    }
}

#[test]
fn existing_rebase_multiple_conflicts_continue_or_abort_both_backends() {
    use plico::{rebase_operation::{self, Action}, operation_process::Progress,
        operation_state::Operation};
    use std::ffi::OsStr;
    // Rebased GitSingleRepoRebaseTest: multiple conflicts and continue after resolution.
    // Separate Repository values intentionally discard any original update-session owner.
    for backend in ["--merge", "--apply"] {
        for abort in [false, true] {
            let f = Fixture::new();
            f.write("shared.txt", "base\n");
            f.write("saved.txt", "base\n"); f.commit("base");
            f.git(&["checkout", "-b", "incoming"]);
            f.write("shared.txt", "incoming\n"); f.commit("incoming");
            let incoming = f.git(&["rev-parse", "HEAD"]);
            f.git(&["checkout", "main"]);
            f.write("shared.txt", "local one\n"); f.commit("local one");
            f.write("shared.txt", "local two\n"); f.commit("local two");
            let before = f.git(&["rev-parse", "HEAD"]);
            f.write("saved.txt", "valuable staged work\n"); f.git(&["add", "saved.txt"]);
            f.write("saved.txt", "valuable working work\n"); f.git(&["stash", "push"]);
            let stash = f.git(&["rev-parse", "refs/stash"]);
            let output = Command::new("git").arg("-C").arg(&f.0)
                .args(["rebase", backend, "incoming"]).output().unwrap();
            assert!(!output.status.success(), "{backend}");
            let token = Cancel::default(); let progress = Progress::default();
            let expected = if backend == "--merge" { Operation::RebaseMerge } else { Operation::RebaseApply };
            let state = f.repo().operation_state(&token).unwrap();
            assert_eq!(state.operation, expected);
            assert!(!state.conflicts.is_empty());
            f.write("shared.txt", "first resolved\n"); f.git(&["add", "shared.txt"]);
            let result = rebase_operation::execute(&f.repo(), Action::Continue {
                editor: OsStr::new("true"), requests: None }, &token, &progress).unwrap();
            // First patch was committed, but the second patch now conflicts.
            assert!(!result.command.unwrap().status.success());
            let state = result.state.unwrap();
            assert_eq!(state.operation, expected);
            assert!(!state.conflicts.is_empty());
            assert_eq!(f.git(&["rev-parse", "HEAD^"]), incoming);
            let intermediate = f.git(&["rev-parse", "HEAD"]);
            let unmerged = f.git(&["ls-files", "--unmerged"]);
            assert!(rebase_operation::execute(&f.repo(), Action::Continue {
                editor: OsStr::new("true"), requests: None }, &token, &progress).is_err());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), intermediate);
            assert_eq!(f.git(&["ls-files", "--unmerged"]), unmerged);
            let action = if abort { Action::Abort } else {
                f.write("shared.txt", "second resolved\n"); f.git(&["add", "shared.txt"]);
                Action::Continue { editor: OsStr::new("true"), requests: None }
            };
            let result = rebase_operation::execute(&f.repo(), action, &token, &progress).unwrap();
            assert!(result.command.unwrap().status.success());
            assert!(!result.state.unwrap().in_progress());
            assert_eq!(f.git(&["symbolic-ref", "--short", "HEAD"]), "main");
            if abort {
                assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
                assert_eq!(fs::read_to_string(f.0.join("shared.txt")).unwrap(), "local two\n");
            } else {
                assert_eq!(f.git(&["rev-parse", "HEAD^"]), intermediate);
                assert_eq!(f.git(&["rev-parse", "HEAD~2"]), incoming);
                assert_eq!(fs::read_to_string(f.0.join("shared.txt")).unwrap(), "second resolved\n");
            }
            assert!(f.git(&["status", "--porcelain"]).is_empty());
            assert_eq!(f.git(&["rev-parse", "refs/stash"]), stash);
            assert_eq!(f.git(&["show", "refs/stash^2:saved.txt"]), "valuable staged work");
            assert_eq!(f.git(&["show", "refs/stash:saved.txt"]), "valuable working work");
            assert_eq!(fs::read_to_string(f.0.join("saved.txt")).unwrap(), "base\n");
        }
    }
}

#[test]
fn branch_popup_recents_follow_checkout_order_and_exclude_deleted_or_detached() {
    use plico::branch_popup::recent_branches;
    let f = Fixture::new();
    f.write("base.txt", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    for i in 0..7 { f.git(&["checkout", "-b", &format!("topic-{i}")]); }
    f.git(&["checkout", "main"]);
    f.git(&["branch", "-D", "topic-6"]);
    f.git(&["checkout", "topic-3"]);
    f.git(&["checkout", "--detach", &base]);
    let branches: Vec<_> = std::iter::once("main".to_string()).chain((0..6).map(|i| format!("topic-{i}")))
        .map(|name| Branch { name: format!("refs/heads/{name}"), id: base.clone() }).collect();
    let repo = f.repo();
    assert_eq!(recent_branches(&repo, &branches, Some("refs/heads/topic-0"), &Cancel::default()).unwrap(),
        ["refs/heads/topic-3", "refs/heads/main", "refs/heads/topic-5", "refs/heads/topic-4", "refs/heads/topic-2"]);
    let token = Cancel::default(); token.cancel();
    assert!(recent_branches(&repo, &branches, None, &token).is_err());
    // Clone-style initial state has no checkout reflog entry. App history is fallback.
    let initial = Fixture::new(); initial.write("a", "a"); initial.commit("initial");
    let main = [Branch { name: "refs/heads/main".into(), id: String::new() }];
    assert!(recent_branches(&initial.repo(), &main, None, &Cancel::default()).unwrap().is_empty());
    assert_eq!(recent_branches(&initial.repo(), &main, Some("refs/heads/main"), &Cancel::default()).unwrap(), ["refs/heads/main"]);
    assert!(recent_branches(&initial.repo(), &main, Some("refs/heads/deleted"), &Cancel::default()).unwrap().is_empty());
}

#[test]
fn checkout_input_resolves_and_pins_refs_without_switching_or_accepting_noncommits() {
    use plico::checkout;
    let f = Fixture::new(); f.write("f", "first"); f.commit("first");
    let first = f.git(&["rev-parse", "HEAD"]);
    f.git(&["tag", "-a", "release", "-m", "release"]);
    f.write("f", "second"); f.commit("second");
    let second = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo();
    for (text, expected) in [(" release ", &first), ("HEAD~1", &first), (&first[..8], &first), ("main", &second)] {
        let resolved = checkout::resolve_input(&repo, text, &Cancel::default()).unwrap();
        assert_eq!(&resolved.target, expected);
        assert_eq!(resolved.expected_head, second);
        assert_eq!(f.git(&["symbolic-ref", "--short", "HEAD"]), "main");
    }
    let blob = f.git(&["rev-parse", "HEAD:f"]);
    for text in ["", " ", "--help", "missing-ref", "HEAD^{tree}", &blob, "HEAD\nmain"] {
        assert!(checkout::resolve_input(&repo, text, &Cancel::default()).is_err(), "{text}");
    }
    let pinned = checkout::resolve_input(&repo, "release", &Cancel::default()).unwrap();
    f.git(&["tag", "-f", "release", &second]);
    assert_eq!(pinned.target, first);
    assert_eq!(f.git(&["status", "--porcelain"]), "");
    // The target stays the reviewed commit even if its symbolic name moves.
    let result = checkout::execute(&repo, &pinned.target, &pinned.expected_head, &Cancel::default(), &Default::default()).unwrap();
    assert!(result.checked_out(&first));
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(checkout::resolve_input(&repo, "HEAD", &cancelled).is_err());
}

#[test]
fn branch_menu_checkout_stays_attached_and_preserves_overwritten_changes() {
    use plico::checkout;
    let f = Fixture::new(); f.write("f", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["branch", "feature/topic"]);
    f.write("f", "main\n"); f.commit("main");
    let main = f.git(&["rev-parse", "HEAD"]);
    // A same-name tag must not turn a branch-menu action into detached checkout.
    f.git(&["tag", "feature/topic"]);
    let repo = f.repo();
    let run = |reference: &str, target: &str, head: &str| checkout::execute_local(
        &repo, reference, target, head, &Cancel::default(), &Default::default());
    for invalid in ["feature/topic", "refs/tags/feature/topic", "refs/remotes/origin/main", "refs/heads/--help", "refs/heads/a\nb"] {
        assert!(run(invalid, &base, &main).is_err(), "{invalid}");
    }
    assert!(run("refs/heads/feature/topic", &main, &main).is_err());
    assert!(run("refs/heads/feature/topic", &base, &base).is_err());
    f.write("f", "unsaved\n");
    let refused = run("refs/heads/feature/topic", &base, &main).unwrap();
    assert!(!refused.checked_out("refs/heads/feature/topic", &base));
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert_eq!(std::fs::read_to_string(f.0.join("f")).unwrap(), "unsaved\n");
    f.git(&["restore", "f"]);
    let result = run("refs/heads/feature/topic", &base, &main).unwrap();
    assert!(result.checked_out("refs/heads/feature/topic", &base));
    assert_eq!(f.git(&["rev-parse", "refs/heads/main"]), main);
    assert_eq!(f.git(&["rev-parse", "refs/tags/feature/topic"]), main);
    assert_eq!(f.git(&["status", "--porcelain"]), "");
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(checkout::execute_local(&repo, "refs/heads/main", &main, &base, &cancelled, &Default::default()).is_err());
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/feature/topic");
    // Returning from detached state must reattach even when HEAD has the same OID.
    f.git(&["checkout", "--detach", &base]);
    assert!(run("refs/heads/feature/topic", &base, &base).unwrap().checked_out("refs/heads/feature/topic", &base));
}

#[test]
fn local_smart_checkout_restores_index_and_retains_conflicted_stash() {
    use plico::checkout::{self, Status};
    let f = Fixture::new(); f.write("f", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["branch", "feature"]);
    f.write("other", "main\n"); f.commit("main");
    let main = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo();
    f.write("f", "staged\n"); f.git(&["add", "f"]);
    let smart = checkout::smart_execute_local(&repo, "refs/heads/feature", &base, &main,
        true, &Cancel::default(), &Default::default()).unwrap();
    assert_eq!(smart.checkout.unwrap().unwrap().status("refs/heads/feature", &base), Status::Success);
    assert!(smart.restore.unwrap().unwrap().status.success());
    assert!(smart.saved_oid.is_none());
    assert_eq!(smart.branch.unwrap(), "refs/heads/feature");
    assert_eq!(f.git(&["show", ":f"]), "staged");
    assert_eq!(f.git(&["stash", "list"]), "");
    f.git(&["reset", "--hard"]);
    f.write("f", "feature\n"); f.commit("feature");
    let feature = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.write("f", "local\n");
    let refused = checkout::execute_local(&repo, "refs/heads/feature", &feature, &main,
        &Cancel::default(), &Default::default()).unwrap();
    assert_eq!(refused.status("refs/heads/feature", &feature), Status::LocalChanges);
    assert!(refused.affected_files("refs/heads/feature", &feature).paths.iter().any(|p| p == "f"));
    let smart = checkout::smart_execute_local(&repo, "refs/heads/feature", &feature, &main,
        true, &Cancel::default(), &Default::default()).unwrap();
    assert!(smart.checkout.unwrap().unwrap().checked_out("refs/heads/feature", &feature));
    assert!(!smart.restore.unwrap().unwrap().status.success());
    assert!(!smart.state.unwrap().conflicts.is_empty());
    let saved = smart.saved_oid.unwrap();
    assert_eq!(f.git(&["show", &format!("{saved}:f")]), "local");
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/feature");
}

#[test]
fn local_force_checkout_requires_fresh_target_and_stays_attached() {
    use plico::checkout;
    let f = Fixture::new(); f.write("f", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["branch", "feature"]);
    f.write("f", "main\n"); f.commit("main");
    let main = f.git(&["rev-parse", "HEAD"]);
    f.write("f", "local\n");
    let repo = f.repo();
    assert!(checkout::force_execute_local(&repo, "refs/heads/feature", &main, &main,
        &Cancel::default(), &Default::default()).is_err());
    assert_eq!(std::fs::read_to_string(f.0.join("f")).unwrap(), "local\n");
    let result = checkout::force_execute_local(&repo, "refs/heads/feature", &base, &main,
        &Cancel::default(), &Default::default()).unwrap();
    assert!(result.checked_out("refs/heads/feature", &base));
    assert_eq!(f.git(&["status", "--porcelain"]), "");
    assert_eq!(f.git(&["rev-parse", "main"]), main);
}

#[test]
fn favorites_are_private_per_worktree_and_reads_leave_git_state_unchanged() {
    use plico::{favorite_preferences as prefs, branch_popup::Favorites};
    let f = Fixture::new(); f.write("tracked", "base\n"); f.commit("base");
    let head = f.git(&["rev-parse", "HEAD"]);
    let config = fs::read(f.0.join(".git/config")).unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let (path, defaults) = prefs::for_repository(&f.repo(), &Cancel::default()).unwrap();
    assert_eq!(defaults, Favorites::default());
    assert!(!path.exists()); assert!(!path.parent().unwrap().exists());
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    let mut main = defaults;
    main.set("refs/heads/main", false); main.set("refs/tags/发布", true);
    prefs::save(&path, &main).unwrap();
    assert_eq!(prefs::for_repository(&f.repo(), &Cancel::default()).unwrap().1, main);

    // Outside the main worktree so creating the fixtures doesn't affect its status.
    let container = Fixture::new();
    let linked = container.0.join("linked");
    f.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
    let linked_repo = Repository::new(&linked);
    let (linked_path, mut linked_prefs) = prefs::for_repository(&linked_repo, &Cancel::default()).unwrap();
    assert_ne!(linked_path, path); assert_eq!(linked_prefs, Favorites::default());
    assert!(!linked_path.parent().unwrap().exists());
    linked_prefs.set("refs/heads/linked", true);
    prefs::save(&linked_path, &linked_prefs).unwrap();
    assert_eq!(prefs::for_repository(&f.repo(), &Cancel::default()).unwrap().1, main);
    assert_eq!(prefs::for_repository(&linked_repo, &Cancel::default()).unwrap().1, linked_prefs);
    let clone = container.0.join("clone");
    f.git(&["clone", "--no-local", f.0.to_str().unwrap(), clone.to_str().unwrap()]);
    let (clone_path, clone_prefs) = prefs::for_repository(&Repository::new(&clone), &Cancel::default()).unwrap();
    assert_eq!(clone_prefs, Favorites::default()); assert!(!clone_path.parent().unwrap().exists());

    let before = fs::read(&path).unwrap();
    let cancel = Cancel::default(); cancel.cancel();
    assert!(prefs::for_repository(&f.repo(), &cancel).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::write(&linked_path, b"git-review-favorites-v1\n+refs/heads/partial\n").unwrap();
    assert!(prefs::for_repository(&linked_repo, &Cancel::default()).is_err());
    assert_eq!(prefs::load(&path).unwrap(), main);
    assert_eq!(fs::read(&linked_path).unwrap(), b"git-review-favorites-v1\n+refs/heads/partial\n");
    assert_eq!(fs::read(f.0.join(".git/config")).unwrap(), config);
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert!(f.git(&["status", "--porcelain", "--untracked-files=all"]).is_empty());
}

#[test]
fn project_update_preflight_uses_actual_upstream_and_preserves_local_changes() {
    use plico::project_update;
    let f = Fixture::new();
    f.write("file", "base\n"); f.commit("base");
    let head = f.git(&["rev-parse", "HEAD"]);
    let remote = f.0.join("upstream.git");
    f.git(&["init", "--bare", remote.to_str().unwrap()]);
    f.git(&["remote", "add", "upstream", remote.to_str().unwrap()]);
    f.git(&["update-ref", "refs/remotes/upstream/review", &head]);
    let repo = f.repo(); let token = Cancel::default();
    assert!(project_update::prepare(&repo, &token).is_err(), "do not invent tracking");
    f.git(&["branch", "--set-upstream-to=upstream/review", "main"]);
    f.write("file", "staged\n"); f.git(&["add", "file"]);
    f.write("file", "unstaged\n");
    f.write("untracked", "keep\n");
    let status = f.git(&["status", "--porcelain"]);
    let request = project_update::prepare(&repo, &token).unwrap();
    assert_eq!(request.branch, "main");
    assert_eq!(request.head, head);
    assert_eq!(request.target.remote, "upstream");
    assert_eq!(request.target.branch, "review");
    assert_eq!(f.git(&["status", "--porcelain"]), status);
    assert_eq!(f.git(&["show", ":file"]), "staged");
    assert_eq!(fs::read_to_string(f.0.join("file")).unwrap(), "unstaged\n");
    assert!(!f.0.join(".git/refs/stash").exists());
    f.git(&["update-ref", "-d", "refs/remotes/upstream/review"]);
    assert!(project_update::prepare(&repo, &token).is_err(), "missing upstream must not fall back");
    f.git(&["checkout", "--detach", "HEAD"]);
    assert!(project_update::prepare(&repo, &token).is_err());
    token.cancel();
    assert!(project_update::prepare(&repo, &token).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
}

#[test]
fn project_update_preflight_rejects_unresolved_merge_before_any_update() {
    let f = Fixture::new();
    f.write("file", "base\n"); f.commit("base");
    f.git(&["checkout", "-b", "topic"]);
    f.write("file", "topic\n"); f.commit("topic");
    f.git(&["checkout", "main"]);
    f.write("file", "main\n"); f.commit("main");
    let head = f.git(&["rev-parse", "HEAD"]);
    let result = Command::new("git").arg("-C").arg(&f.0).args(["merge", "topic"]).output().unwrap();
    assert!(!result.status.success());
    let before = f.git(&["ls-files", "--unmerged"]);
    assert!(!before.is_empty());
    assert!(plico::project_update::prepare(&f.repo(), &Cancel::default()).err().unwrap()
        .contains("unfinished operation"));
    assert_eq!(f.git(&["ls-files", "--unmerged"]), before);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
}

#[test]
fn project_update_runs_selected_method_without_pushing_and_rejects_stale_confirmation() {
    use plico::{project_update, update_method::Method, published_commits::Protection, update_session::UpdateResult};
    for mode in ["merge", "rebase", "nothing", "stale", "cancel-risk"] {
        let upstream = Fixture::new();
        upstream.write("base", "base\n"); upstream.commit("base");
        let bare = upstream.0.join("remote.git");
        upstream.git(&["init", "--bare", bare.to_str().unwrap()]);
        upstream.git(&["remote", "add", "server", bare.to_str().unwrap()]);
        upstream.git(&["push", "server", "main"]);
        let f = Fixture::new();
        f.git(&["remote", "add", "upstream", bare.to_str().unwrap()]);
        f.git(&["fetch", "upstream"]); f.git(&["reset", "--hard", "upstream/main"]);
        f.git(&["branch", "--set-upstream-to=upstream/main", "main"]);
        let initial_remote = f.git(&["rev-parse", "upstream/main"]);
        if mode != "nothing" {
            // Do not add the nested bare repository to the fixture's worktree.
            upstream.write("incoming", "incoming\n"); upstream.git(&["add", "incoming"]);
            upstream.git(&["commit", "-m", "incoming"]); upstream.git(&["push", "server", "main"]);
            f.write("ours", "ours\n"); f.commit("ours");
        }
        if mode == "cancel-risk" {
            f.git(&["checkout", "-b", "side"]);
            f.write("side", "side\n"); f.commit("side");
            f.git(&["checkout", "main"]);
            f.git(&["merge", "--no-ff", "--no-commit", "side"]);
            f.write("merge-only", "change introduced only by merge\n"); f.commit("merge side with additional edit");
        }
        let remote_head = upstream.git(&["rev-parse", "HEAD"]);
        let repo = f.repo(); let token = Cancel::default();
        let request = project_update::prepare(&repo, &token).unwrap();
        f.write("base", "valuable staged\n"); f.git(&["add", "base"]);
        f.write("base", "valuable unstaged\n"); f.write("untracked", "keep\n");
        if mode == "stale" { f.git(&["checkout", "-b", "different"]); }
        let result = project_update::execute(&repo, &request,
            if matches!(mode, "rebase" | "cancel-risk") { Method::Rebase } else { Method::Merge },
            std::ffi::OsStr::new("true"), &token, &Default::default(), None, Protection::Defaults,
            |problem| {
                assert_eq!(mode, "cancel-risk", "unexpected risk gate");
                assert!(matches!(problem, Ok(true)));
                assert!(f.git(&["stash", "list"]).is_empty(), "risk gate must run before save");
                Ok(None)
            },
            |_| panic!("private commits must not require published confirmation"));
        if mode == "stale" {
            assert!(result.is_err());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), request.head);
            assert_eq!(f.git(&["rev-parse", "upstream/main"]), initial_remote, "reject before Fetch");
        } else if mode == "cancel-risk" {
            let report = result.unwrap();
            assert_eq!(report.result(), UpdateResult::Cancelled);
            assert!(!report.complete());
            assert!(report.saved_oid().is_none());
            assert_eq!(f.git(&["rev-parse", "HEAD"]), request.head);
        } else {
            let report = result.unwrap();
            assert!(report.complete(), "{mode}: {:?}", report.result());
            assert_eq!(report.result(), if mode == "nothing" { UpdateResult::NothingToUpdate } else { UpdateResult::Success });
            assert!(report.saved_oid().is_none());
            if mode == "merge" {
                assert_eq!(f.git(&["rev-parse", "HEAD^1"]), request.head);
                assert_eq!(f.git(&["rev-parse", "HEAD^2"]), remote_head);
            } else if mode == "rebase" {
                assert_eq!(f.git(&["rev-parse", "HEAD^"]), remote_head);
                assert_ne!(f.git(&["rev-parse", "HEAD"]), request.head);
            }
        }
        assert_eq!(f.git(&["show", ":base"]), "valuable staged");
        assert_eq!(fs::read_to_string(f.0.join("base")).unwrap(), "valuable unstaged\n");
        assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep\n");
        assert!(f.git(&["stash", "list"]).is_empty());
        let remote_actual = Command::new("git").arg("--git-dir").arg(&bare).args(["rev-parse", "refs/heads/main"]).output().unwrap();
        assert!(remote_actual.status.success());
        assert_eq!(String::from_utf8(remote_actual.stdout).unwrap().trim(), remote_head, "update must not push");
    }
}

#[test]
fn project_update_options_load_shared_method_without_writing_or_reusing_push_consent() {
    use plico::{project_update_preferences::Preferences, update_preferences::{self, Strategy}, update_method::Method};
    let f = Fixture::new(); f.write("file", "base\n"); f.commit("base");
    let repo = f.repo(); let token = Cancel::default();
    let before = f.git(&["rev-parse", "HEAD"]);
    let (path, _) = update_preferences::for_repository(&repo, &token).unwrap();
    let draft = Preferences::for_repository(&repo, &token).unwrap();
    assert_eq!(draft.method, Method::Merge); assert!(draft.show_options);
    assert!(!path.parent().unwrap().exists());
    update_preferences::save(&path, update_preferences::Preferences { strategy: Strategy::BranchDefault, auto_update: true }).unwrap();
    f.git(&["config", "branch.main.rebase", "true"]);
    let draft = Preferences::for_repository(&repo, &token).unwrap();
    assert_eq!(draft.method, Method::Rebase); assert!(draft.show_options);
    // Dismissing a loaded draft has no persistence side effects.
    drop(draft);
    assert_eq!(update_preferences::load(&path).strategy, Strategy::BranchDefault);
    assert!(!path.with_file_name("project-update-options").exists());
    let draft = Preferences::for_repository(&repo, &token).unwrap();
    draft.accept(Method::Merge, false).unwrap();
    let next = Preferences::for_repository(&repo, &token).unwrap();
    assert_eq!(next.method, Method::Merge); assert!(!next.show_options);
    assert!(update_preferences::load(&path).auto_update);
    // A later Push/settings method change is visible at the next Update invocation.
    update_preferences::save(&path, update_preferences::Preferences { strategy: Strategy::Rebase, auto_update: false }).unwrap();
    let next = Preferences::for_repository(&repo, &token).unwrap();
    assert_eq!(next.method, Method::Rebase); assert!(!next.show_options);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), before);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
    token.cancel(); assert!(Preferences::for_repository(&repo, &token).is_err());
}

#[test]
fn outgoing_preview_collects_all_pages_and_rejects_partial_or_stale_ranges() {
    use plico::push_target::{Origin, Target};
    let f = Fixture::new();
    f.write("a", "base"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let mut source = base.clone();
    let mut ids = Vec::new();
    for n in 0..105 {
        source = f.git(&["commit-tree", &tree, "-p", &source, "-m", &format!("outgoing {n}")]);
        ids.push(source.clone());
    }
    f.git(&["remote", "add", "origin", "https://example.invalid/preview"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &base]);
    let target = Target { remote: "origin".into(), branch: "main".into(), special: false,
        new_branch: false, origin: Origin::TrackingBranch };
    let repo = f.repo(); let token = Cancel::default();
    let all = repo.outgoing_preview_commits(&source, &target, None, 105, &token).unwrap();
    ids.reverse();
    assert_eq!(all.commits.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), ids);
    assert!(!all.more && !all.capped);
    assert!(repo.outgoing_preview_commits(&source, &target, None, 100, &token).err().unwrap().contains("limit"));
    assert!(repo.outgoing_preview_commits(&base, &target, None, 0, &token).unwrap().commits.is_empty());
    assert!(repo.outgoing_preview_commits(&source, &target, None, 0, &token).is_err());
    f.git(&["update-ref", "refs/remotes/origin/main", &source]);
    assert!(repo.outgoing_preview_commits(&source, &target, Some(&all.references), 105, &token)
        .err().unwrap().contains("references changed"));
    token.cancel();
    assert!(repo.outgoing_preview_commits(&source, &target, None, 105, &token).is_err());
}

#[test]
fn push_commit_changes_follow_combined_merge_and_root_rules() {
    let f = Fixture::new();
    let commit = |id: String, parents: Vec<String>| Commit { id, parents, shallow_boundary: false,
        author: String::new(), date: String::new(), subject: String::new(), refs: Vec::new() };
    let token = Cancel::default(); let repo = f.repo();
    f.write("shared", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    let root = repo.push_commit_changes(&commit(base.clone(), vec![]), &token).unwrap();
    assert_eq!(root.len(), 1); assert!(root[0].before.is_none());
    f.git(&["checkout", "-b", "side"]);
    f.write("side-only", "side\n"); f.write("shared", "side\n"); f.commit("side");
    let side = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.write("main-only", "main\n"); f.write("shared", "main\n"); f.commit("main");
    let main = f.git(&["rev-parse", "HEAD"]);
    // Create a merge result containing each side's files and a resolution
    // differing from both parents, without invoking a working-tree merge.
    f.write("side-only", "side\n"); f.write("shared", "resolved\n");
    f.git(&["add", "."]); let tree = f.git(&["write-tree"]);
    let merge = f.git(&["commit-tree", &tree, "-p", &main, "-p", &side, "-m", "resolved merge"]);
    let result = repo.push_commit_changes(&commit(merge.clone(), vec![main.clone(), side.clone()]), &token).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].before.as_ref().unwrap().path, PathBuf::from("shared"));
    assert_eq!(result[0].before.as_ref().unwrap().revision, main);
    assert_eq!(result[0].after.as_ref().unwrap().revision, merge);
    // Taking one parent's shared content is not a combined change.
    f.write("shared", "side\n"); f.git(&["add", "."]);
    let tree = f.git(&["write-tree"]);
    let clean = f.git(&["commit-tree", &tree, "-p", &main, "-p", &side, "-m", "clean merge"]);
    assert!(repo.push_commit_changes(&commit(clean, vec![main.clone(), side]), &token).unwrap().is_empty());
    f.git(&["reset", "--hard", &main]);
    f.git(&["mv", "shared", "renamed"]); f.commit("rename");
    let renamed = f.git(&["rev-parse", "HEAD"]);
    let result = repo.push_commit_changes(&commit(renamed, vec![main.clone()]), &token).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].before.as_ref().unwrap().path, PathBuf::from("shared"));
    assert_eq!(result[0].after.as_ref().unwrap().path, PathBuf::from("renamed"));
    let mut shallow = commit(main, vec![base]); shallow.shallow_boundary = true;
    assert!(repo.push_commit_changes(&shallow, &token).is_err());
    token.cancel(); assert!(repo.push_commit_changes(&shallow, &token).is_err());
}

#[test]
fn push_preview_snapshot_folds_range_and_rejects_shallow_boundary() {
    use plico::{push_preview, push_target::{Origin, Target}};
    let f = Fixture::new();
    f.write("old", "original\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["remote", "add", "origin", "https://example.invalid/preview"]);
    f.git(&["update-ref", "refs/remotes/origin/main", &base]);
    f.git(&["mv", "old", "new"]); f.write("temporary", "added\n"); f.commit("rename and add");
    f.write("new", "edited after rename\n"); f.git(&["rm", "temporary"]); f.commit("edit and delete");
    let source = f.git(&["rev-parse", "HEAD"]);
    let mut target = Target { remote: "origin".into(), branch: "main".into(), special: false,
        new_branch: false, origin: Origin::TrackingBranch };
    let repo = f.repo(); let token = Cancel::default();
    let result = push_preview::load(&repo, &source, &target, None, 1000, &token).unwrap();
    assert_eq!(result.commit_count, 2); assert!(!result.capped);
    assert_eq!(result.files.len(), 1);
    let file = &result.files[0];
    assert_eq!(file.before.as_ref().unwrap().path, PathBuf::from("old"));
    assert_eq!(file.before.as_ref().unwrap().revision, base);
    assert_eq!(file.after.as_ref().unwrap().path, PathBuf::from("new"));
    assert_eq!(file.after.as_ref().unwrap().revision, source);
    assert!(push_preview::load(&repo, &source, &target, Some(&result.references), 1, &token).is_err());
    // Marking the source shallow makes Git log hide its real parents. The
    // aggregate must reject it, not show every file as a root addition.
    fs::write(f.0.join(".git/shallow"), format!("{source}\n")).unwrap();
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"]);
    target.new_branch = true;
    assert!(push_preview::load(&repo, &source, &target, None, 1000, &token)
        .err().unwrap().contains("shallow history boundary"));
    token.cancel(); assert!(push_preview::load(&repo, &source, &target, None, 1000, &token).is_err());
}

#[test]
fn combined_revision_contents_are_full_literal_and_distinguish_absence() {
    use plico::combined_changes::{Change as Combined, Revision, read_contents};
    let f = Fixture::new();
    let original = (0..300).map(|i| format!("line {i}\n")).collect::<String>();
    f.write("old[1].txt", &original); f.write("empty", "");
    fs::write(f.0.join("binary"), b"a\0b").unwrap();
    fs::write(f.0.join("non-utf8"), [0xff]).unwrap();
    f.commit("base"); let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["mv", "old[1].txt", "new[2].txt"]);
    let updated = original.replace("line 150\n", "reviewed\n").trim_end_matches('\n').to_owned();
    f.write("new[2].txt", &updated); f.commit("rename and edit");
    let tip = f.git(&["rev-parse", "HEAD"]);
    let revision = |id: &str, path: &str| Revision { revision: id.into(), path: path.into() };
    let change = Combined { before: Some(revision(&base, "old[1].txt")), after: Some(revision(&tip, "new[2].txt")) };
    let token = Cancel::default(); let repo = f.repo();
    // Worktree modifications must not affect either immutable revision.
    f.write("new[2].txt", "uncommitted");
    let target = plico::diff_target::Target::combined(repo.clone(), change.clone()).unwrap();
    let contents = target.read_combined(&token).unwrap().unwrap();
    assert_eq!(contents.before.as_deref(), Some(original.as_str()));
    assert_eq!(contents.after.as_deref(), Some(updated.as_str()));
    let empty = read_contents(&repo, &Combined { before: None, after: Some(revision(&base, "empty")) }, &token).unwrap();
    assert!(empty.before.is_none()); assert_eq!(empty.after.as_deref(), Some(""));
    let deleted = read_contents(&repo, &Combined { before: change.before.clone(), after: None }, &token).unwrap();
    assert_eq!(deleted.before.as_deref(), Some(original.as_str())); assert!(deleted.after.is_none());
    for path in ["binary", "non-utf8", "missing", "../outside"] {
        assert!(read_contents(&repo, &Combined { before: None, after: Some(revision(&base, path)) }, &token).is_err(), "{path}");
    }
    token.cancel(); assert!(read_contents(&repo, &change, &token).is_err());
}

#[test]
fn log_commit_changes_follow_parent_merge_and_root_rules() {
    let f = Fixture::new();
    let commit = |id: String, parents: Vec<String>| Commit { id, parents, shallow_boundary: false,
        author: String::new(), date: String::new(), subject: String::new(), refs: Vec::new() };
    let token = Cancel::default(); let repo = f.repo();
    f.write("shared", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    let root = repo.log_commit_changes(&commit(base.clone(), vec![]), &token).unwrap();
    assert_eq!(root.len(), 1); assert!(root[0].before.is_none());
    f.git(&["checkout", "-b", "side"]);
    f.write("side-only", "side\n"); f.write("shared", "side\n"); f.commit("side");
    let side = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.write("main-only", "main\n"); f.write("shared", "main\n"); f.commit("main");
    let main = f.git(&["rev-parse", "HEAD"]);
    // Create a merge result containing each side's files and a resolution
    // differing from both parents, without invoking a working-tree merge.
    f.write("side-only", "side\n"); f.write("shared", "resolved\n");
    f.git(&["add", "."]); let tree = f.git(&["write-tree"]);
    let merge = f.git(&["commit-tree", &tree, "-p", &main, "-p", &side, "-m", "resolved merge"]);
    let result = repo.log_commit_changes(&commit(merge.clone(), vec![main.clone(), side.clone()]), &token).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].before.as_ref().unwrap().path, PathBuf::from("shared"));
    assert_eq!(result[0].before.as_ref().unwrap().revision, main);
    assert_eq!(result[0].after.as_ref().unwrap().revision, merge);
    // Taking one parent's shared content is not a combined change.
    f.write("shared", "side\n"); f.git(&["add", "."]);
    let tree = f.git(&["write-tree"]);
    let clean = f.git(&["commit-tree", &tree, "-p", &main, "-p", &side, "-m", "clean merge"]);
    assert!(repo.log_commit_changes(&commit(clean, vec![main.clone(), side]), &token).unwrap().is_empty());
    f.git(&["reset", "--hard", &main]);
    f.git(&["mv", "shared", "renamed"]); f.commit("rename");
    let renamed = f.git(&["rev-parse", "HEAD"]);
    let result = repo.log_commit_changes(&commit(renamed, vec![main.clone()]), &token).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].before.as_ref().unwrap().path, PathBuf::from("shared"));
    assert_eq!(result[0].after.as_ref().unwrap().path, PathBuf::from("renamed"));
    let mut shallow = commit(main, vec![base]); shallow.shallow_boundary = true;
    assert!(repo.log_commit_changes(&shallow, &token).is_err());
    token.cancel(); assert!(repo.log_commit_changes(&shallow, &token).is_err());
}


#[test]
fn log_selection_aggregates_only_selected_commits_and_checks_boundaries() {
    use plico::{log_changes, combined_changes};
    let f = Fixture::new(); let repo = f.repo(); let token = Cancel::default();
    f.write("old", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["mv", "old", "renamed"]); f.write("temporary", "added\n"); f.commit("rename and add");
    let first = f.git(&["rev-parse", "HEAD"]);
    f.write("unselected", "gap only\n"); f.commit("not selected");
    let gap = f.git(&["rev-parse", "HEAD"]);
    f.write("renamed", "base\nfinal\n"); f.git(&["rm", "temporary"]); f.commit("edit and delete");
    let last = f.git(&["rev-parse", "HEAD"]);
    let result = log_changes::load(&repo, &[last.clone(), first.clone()], &token).unwrap();
    assert_eq!(result.commits.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), [last.as_str(), first.as_str()]);
    assert_eq!(result.files.len(), 1);
    let file = &result.files[0];
    assert_eq!(file.before.as_ref().unwrap().revision, base);
    assert_eq!(file.before.as_ref().unwrap().path, PathBuf::from("old"));
    assert_eq!(file.after.as_ref().unwrap().revision, last);
    let body = combined_changes::read_contents(&repo, file, &token).unwrap();
    assert_eq!(body.before.as_deref(), Some("base\n"));
    assert_eq!(body.after.as_deref(), Some("base\nfinal\n"));
    assert!(log_changes::load(&repo, &[], &token).unwrap().files.is_empty());
    assert!(log_changes::load(&repo, &[first.clone(), first], &token).is_err());
    assert!(log_changes::load(&repo, &["invalid".into()], &token).is_err());
    fs::write(f.0.join(".git/shallow"), format!("{gap}\n")).unwrap();
    assert!(log_changes::load(&repo, &[gap], &token).is_err());
    fs::remove_file(f.0.join(".git/shallow")).unwrap();
    token.cancel(); assert!(log_changes::load(&repo, &[last.clone()], &token).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), last);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn commit_metadata_reads_message_without_parent_or_file_diff() {
    let f = Fixture::new();
    f.write("file", "base\n"); f.commit("标题\n\nBody <literal> & text");
    let id = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo(); let token = Cancel::default();
    let mut commit = repo.snapshot_commits(std::slice::from_ref(&id), std::slice::from_ref(&id), &token).unwrap().commits.remove(0);
    let complete = repo.details(&commit, &token).unwrap();
    let metadata = repo.commit_metadata(&commit, &token).unwrap();
    assert_eq!(metadata.message, complete.message);
    assert_eq!(metadata.author, complete.author);
    assert_eq!(metadata.authored, complete.authored);
    assert_eq!(metadata.committer, complete.committer);
    assert_eq!(metadata.committed, complete.committed);
    assert_eq!(metadata.base, complete.base);
    assert!(metadata.files.is_empty()); assert_eq!(complete.files.len(), 1);
    // A missing comparison parent must not prevent reading available commit text.
    commit.parents = vec!["f".repeat(40)];
    assert!(repo.details(&commit, &token).is_err());
    assert_eq!(repo.commit_metadata(&commit, &token).unwrap().message, complete.message);
    commit.shallow_boundary = true;
    let shallow = repo.details(&commit, &token).unwrap();
    assert_eq!(shallow.message, complete.message); assert!(shallow.files.is_empty());
    token.cancel(); assert!(repo.commit_metadata(&commit, &token).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), id);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn squash_preparation_checks_head_path_protected_ancestry_and_preserves_state() {
    use plico::{squash_check::prepare, published_commits::Protection, pull::Target};
    let f = Fixture::new();
    f.write("base", "base\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    let mut ids = Vec::new();
    for i in 0..5 { f.write(&format!("f{i}"), "value\n"); f.commit(&format!("commit{i}")); ids.push(f.git(&["rev-parse", "HEAD"])); }
    let head = ids[4].clone(); let selected = vec![ids[4].clone(), ids[0].clone(), ids[2].clone()];
    f.write("f0", "unstaged edit\n"); f.write("f1", "staged edit\n"); f.git(&["add", "f1"]);
    let before = f.git(&["status", "--porcelain"]); let index = f.git(&["ls-files", "--stage"]);
    let token = Cancel::default(); let repo = f.repo();
    let check = prepare(&repo, &head, &selected, Protection::Defaults, &token).unwrap();
    assert_eq!(check.base, base); assert_eq!(check.branch.as_deref(), Some("refs/heads/main"));
    assert_eq!(check.selected_oldest_first, [ids[0].clone(),ids[2].clone(),ids[4].clone()]);
    assert_eq!(check.plan.entries().iter().map(|e| e.oid.clone()).collect::<Vec<_>>(),
        [ids[0].clone(),ids[2].clone(),ids[4].clone(),ids[1].clone(),ids[3].clone()]);
    assert!(prepare(&repo, &ids[3], &selected, Protection::Defaults, &token).unwrap_err().contains("HEAD changed"));
    assert!(prepare(&repo, &head, &[base.clone(),ids[0].clone()], Protection::Defaults, &token).unwrap_err().contains("root or merge"));
    f.git(&["remote", "add", "origin", "."]);
    f.git(&["update-ref", "refs/remotes/origin/main", &ids[1]]);
    assert!(prepare(&repo, &head, &selected, Protection::Defaults, &token).unwrap_err().contains("protected branch origin/main"));
    // Published to an ordinary branch is allowed by upstream; protection is policy.
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"]);
    f.git(&["update-ref", "refs/remotes/origin/review", &head]);
    assert!(prepare(&repo, &head, &selected, Protection::Defaults, &token).is_ok());
    let custom = [Target { remote: "origin".into(), branch: "review".into() }];
    assert!(prepare(&repo, &head, &selected, Protection::Explicit(&custom), &token).unwrap_err().contains("origin/review"));
    // A protected ancestor strictly before the oldest selection is not rewritten.
    f.git(&["update-ref", "refs/remotes/origin/main", &base]);
    assert!(prepare(&repo, &head, &selected, Protection::Defaults, &token).is_ok());
    assert_eq!(f.git(&["status", "--porcelain"]), before); assert_eq!(f.git(&["ls-files", "--stage"]), index);
    f.git(&["checkout", "--detach"]);
    assert_eq!(prepare(&repo, &head, &selected, Protection::Defaults, &token).unwrap().branch, None);
    fs::write(f.0.join(".git/shallow"), format!("{}\n", ids[0])).unwrap();
    assert!(prepare(&repo, &head, &selected, Protection::Defaults, &token).unwrap_err().contains("shallow boundary"));
    fs::remove_file(f.0.join(".git/shallow")).unwrap();
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(prepare(&repo, &head, &selected, Protection::Defaults, &cancelled).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head); assert_eq!(f.git(&["status", "--porcelain"]), before);
}

#[test]
fn squash_preparation_rejects_merge_in_path_and_selection_outside_head() {
    use plico::{squash_check::prepare, published_commits::Protection};
    let f = Fixture::new(); f.write("base", "base"); f.commit("base");
    f.write("a", "a"); f.commit("a"); let a = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "-b", "topic"]); f.write("topic", "topic"); f.commit("topic");
    let other = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]); f.write("b", "b"); f.commit("b"); let b = f.git(&["rev-parse", "HEAD"]);
    let token = Cancel::default();
    assert!(prepare(&f.repo(), &b, &[b.clone(),other], Protection::Defaults, &token).is_err());
    f.git(&["merge", "--no-ff", "topic", "-m", "merge"]);
    f.write("c", "c"); f.commit("c"); let c = f.git(&["rev-parse", "HEAD"]);
    assert!(prepare(&f.repo(), &c, &[c.clone(),a], Protection::Defaults, &token).unwrap_err().contains("root or merge"));
    // A merge before the oldest selected commit need not be rewritten.
    f.write("d", "d"); f.commit("d"); let d = f.git(&["rev-parse", "HEAD"]);
    assert!(prepare(&f.repo(), &d, &[d.clone(),c], Protection::Defaults, &token).is_ok());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), d); assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn squash_message_draft_loads_full_messages_in_selection_order() {
    let f = Fixture::new(); f.write("a", "base"); f.commit("base");
    f.git(&["commit", "--allow-empty", "-m", "first subject", "-m", "first body"]);
    let first = f.git(&["rev-parse", "HEAD"]);
    f.git(&["commit", "--allow-empty", "-m", "second subject", "-m", "second body"]);
    let second = f.git(&["rev-parse", "HEAD"]);
    let token = Cancel::default();
    let draft = plico::squash_check::message(&f.repo(), &[second.clone(), first.clone()], &token).unwrap();
    assert!(draft.starts_with("second subject\n\nsecond body"));
    assert!(draft.ends_with("first subject\n\nfirst body\n"));
    assert!(plico::squash_check::message(&f.repo(), &[second.clone(), "--all".into()], &token).is_err());
    token.cancel();
    assert!(plico::squash_check::message(&f.repo(), &[second, first], &token).is_err());
}

#[test]
fn squash_confirmation_revalidates_snapshot_without_changing_index_or_history() {
    use plico::{squash_check::{prepare, revalidate}, published_commits::Protection};
    let f = Fixture::new();
    f.write("base", "base"); f.commit("base");
    f.write("one", "one"); f.commit("one");
    let first = f.git(&["rev-parse", "HEAD"]);
    f.write("two", "two"); f.commit("two");
    let head = f.git(&["rev-parse", "HEAD"]);
    let token = Cancel::default(); let repo = f.repo();
    let snapshot = prepare(&repo, &head, &[head.clone(), first], Protection::Defaults, &token).unwrap();
    f.write("one", "staged"); f.git(&["add", "one"]); f.write("one", "unstaged");
    let index = f.git(&["ls-files", "--stage"]);
    let status = f.git(&["status", "--porcelain"]);
    assert!(revalidate(&repo, &snapshot, Protection::Defaults, &token).is_ok());
    f.git(&["symbolic-ref", "HEAD", "refs/heads/other"]);
    f.git(&["update-ref", "HEAD", &head]);
    assert!(revalidate(&repo, &snapshot, Protection::Defaults, &token).unwrap_err().contains("selection changed"));
    f.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
    f.git(&["remote", "add", "origin", "."]);
    f.git(&["update-ref", "refs/remotes/origin/main", &head]);
    assert!(revalidate(&repo, &snapshot, Protection::Defaults, &token).err().unwrap().contains("protected branch"));
    f.git(&["update-ref", "-d", "refs/remotes/origin/main"]);
    fs::write(f.0.join(".git/MERGE_HEAD"), format!("{head}\n")).unwrap();
    assert!(revalidate(&repo, &snapshot, Protection::Defaults, &token).unwrap_err().contains("current Git operation"));
    fs::remove_file(f.0.join(".git/MERGE_HEAD")).unwrap();
    let cancelled = Cancel::default(); cancelled.cancel();
    assert!(revalidate(&repo, &snapshot, Protection::Defaults, &cancelled).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(f.git(&["ls-files", "--stage"]), index);
    assert_eq!(f.git(&["status", "--porcelain"]), status);
    f.git(&["update-ref", "HEAD", &f.git(&["rev-parse", "HEAD^"])]);
    assert!(revalidate(&repo, &snapshot, Protection::Defaults, &token).unwrap_err().contains("HEAD changed"));
}

#[test]
fn graph_navigation_uses_filtered_adjacency_and_fixed_roots() {
    use plico::{history, git::LogFilter};
    let f = Fixture::new();
    let mut ids = Vec::new();
    for subject in ["visible base", "hidden middle", "visible tip"] {
        f.write("file", subject); f.commit(subject); ids.push(f.git(&["rev-parse", "HEAD"]));
    }
    let repo = f.repo(); let token = Cancel::default(); let roots = vec![ids[2].clone()];
    let filter = LogFilter { text: "visible".into(), ..Default::default() };
    assert_eq!(history::adjacent(&repo, &roots, &ids[2], true, None, &token).unwrap(), vec![ids[1].clone()]);
    assert_eq!(history::adjacent(&repo, &roots, &ids[2], true, Some(&filter), &token).unwrap(), vec![ids[0].clone()]);
    assert_eq!(history::adjacent(&repo, &roots, &ids[0], false, Some(&filter), &token).unwrap(), vec![ids[2].clone()]);
    assert!(history::adjacent(&repo, &roots, &ids[1], true, Some(&filter), &token).unwrap().is_empty());
    assert!(history::adjacent(&repo, &roots, &ids[0], true, None, &token).unwrap().is_empty());
    f.write("file", "later"); f.commit("visible later");
    let head = f.git(&["rev-parse", "HEAD"]);
    assert!(history::adjacent(&repo, &roots, &ids[2], false, None, &token).unwrap().is_empty());
    let new_roots = vec![head.clone()];
    assert_eq!(history::adjacent(&repo, &new_roots, &ids[2], false, None, &token).unwrap(), vec![head.clone()]);
    let hash = LogFilter { text: format!("{},{}", &ids[0][..8], &ids[2][..8]), paths: vec!["missing".into()], ..Default::default() };
    assert_eq!(history::adjacent(&repo, &roots, &ids[2], true, Some(&hash), &token).unwrap(), vec![ids[0].clone()]);
    token.cancel();
    assert!(history::adjacent(&repo, &roots, &ids[2], true, None, &token).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert!(f.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn revision_comparison_uses_two_trees_literal_paths_and_immutable_endpoints() {
    use plico::combined_changes::read_contents;
    let f = Fixture::new();
    f.write("shared", "base\n"); f.write("old name", "rename content\n");
    f.write("[literal] 中文.txt", "old\n"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "-b", "side"]);
    f.write("shared", "side\n"); f.write("side-only", "side\n"); f.commit("side");
    let side = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    f.write("shared", "main\n"); f.write("[literal] 中文.txt", "new\n");
    f.git(&["mv", "old name", "new name"]); f.write("main-only", "main\n"); f.commit("main");
    let main = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo(); let token = Cancel::default();
    let changes = repo.revision_changes(&side, &main, &[], &token).unwrap();
    let shared = changes.iter().find(|c| c.after.as_ref().is_some_and(|r| r.path == PathBuf::from("shared"))).unwrap();
    let text = read_contents(&repo, shared, &token).unwrap();
    assert_eq!(text.before.as_deref(), Some("side\n")); assert_eq!(text.after.as_deref(), Some("main\n"));
    assert!(changes.iter().any(|c| c.after.is_none() && c.before.as_ref().is_some_and(|r| r.path == PathBuf::from("side-only"))));
    assert!(changes.iter().any(|c| c.before.is_none() && c.after.as_ref().is_some_and(|r| r.path == PathBuf::from("main-only"))));
    assert!(changes.iter().any(|c| c.before.as_ref().is_some_and(|r| r.path == PathBuf::from("old name"))
        && c.after.as_ref().is_some_and(|r| r.path == PathBuf::from("new name"))));
    let literal = repo.revision_changes(&base, &main, &[PathBuf::from("[literal] 中文.txt")], &token).unwrap();
    assert_eq!(literal.len(), 1);
    assert_eq!(read_contents(&repo, &literal[0], &token).unwrap().after.as_deref(), Some("new\n"));
    assert!(repo.revision_changes(&main, &main, &[], &token).unwrap().is_empty());
    f.write("shared", "dirty\n");
    assert_eq!(read_contents(&repo, shared, &token).unwrap().after.as_deref(), Some("main\n"));
    let inverse = repo.revision_changes(&main, &side, &[PathBuf::from("shared")], &token).unwrap();
    assert_eq!(read_contents(&repo, &inverse[0], &token).unwrap().after.as_deref(), Some("side\n"));
    assert!(repo.revision_changes("HEAD", &main, &[], &token).is_err());
    assert!(repo.revision_changes(&base, &main, &[PathBuf::from("../outside")], &token).is_err());
    token.cancel(); assert!(repo.revision_changes(&side, &main, &[], &token).is_err());
    assert_eq!(f.git(&["rev-parse", "HEAD"]), main);
    assert_eq!(fs::read_to_string(f.0.join("shared")).unwrap(), "dirty\n");
}

#[test]
fn squash_external_recovery_acknowledges_only_the_confirmed_record() {
    use plico::squash_saved;
    for mode in ["valid", "corrupt", "changed", "operation", "cancel", "other-repo"] {
        let f = Fixture::new(); f.write("file", "base\n"); f.commit("base");
        let head = f.git(&["rev-parse", "HEAD"]);
        f.write("file", "saved\n"); f.git(&["stash", "push"]);
        let stash = f.git(&["rev-parse", "refs/stash"]);
        f.write("file", "staged\n"); f.git(&["add", "file"]); f.write("file", "working\n");
        let index = f.git(&["ls-files", "--stage"]);
        let status = f.git(&["status", "--porcelain"]);
        let path = squash_saved::path(&f.0.join(".git"));
        let record = if mode == "corrupt" { b"damaged record\0\xff".to_vec() }
            else { format!("GitReview squash saved v1\n{head}\n{head}\n{stash}\nrefs/heads/main\n").into_bytes() };
        fs::write(&path, &record).unwrap();
        let repo = f.repo(); let token = Cancel::default();
        let expected = squash_saved::pending(&repo, &token).unwrap().unwrap();
        match mode {
            "changed" => fs::write(&path, b"new owner").unwrap(),
            "operation" => fs::write(f.0.join(".git/MERGE_HEAD"), format!("{head}\n")).unwrap(),
            "cancel" => token.cancel(),
            _ => {}
        }
        let other = Fixture::new(); let other_repo = other.repo();
        let result = squash_saved::acknowledge_external_recovery(
            if mode == "other-repo" { &other_repo } else { &repo }, &expected, &token);
        assert_eq!(result.is_ok(), matches!(mode, "valid" | "corrupt"), "{mode}: {result:?}");
        if result.is_err() { assert!(path.exists()); } else { assert!(!path.exists()); }
        assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
        assert_eq!(f.git(&["rev-parse", "refs/stash"]), stash);
        assert_eq!(f.git(&["ls-files", "--stage"]), index);
        assert_eq!(fs::read_to_string(f.0.join("file")).unwrap(), "working\n");
        if mode == "operation" { fs::remove_file(f.0.join(".git/MERGE_HEAD")).unwrap(); }
        assert_eq!(f.git(&["status", "--porcelain"]), status);
    }
}

#[test]
fn squash_external_recovery_is_worktree_scoped_even_with_identical_records() {
    use plico::squash_saved;
    let f = Fixture::new(); f.write("file", "base\n"); f.commit("base");
    let container = Fixture::new(); let linked = container.0.join("linked");
    f.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
    let linked_repo = Repository::new(&linked); let repo = f.repo(); let token = Cancel::default();
    f.write("file", "saved\n"); f.git(&["stash", "push"]);
    let stash = f.git(&["rev-parse", "refs/stash"]);
    let main_path = squash_saved::path(&repo.editor_directory(&token).unwrap());
    let linked_path = squash_saved::path(&linked_repo.editor_directory(&token).unwrap());
    assert_ne!(main_path, linked_path);
    for path in [&main_path, &linked_path] { fs::write(path, b"identical recovery record").unwrap(); }
    let main_pending = squash_saved::pending(&repo, &token).unwrap().unwrap();
    let linked_pending = squash_saved::pending(&linked_repo, &token).unwrap().unwrap();
    let main_index = fs::read(f.0.join(".git/index")).unwrap();
    let linked_index_path = linked_repo.editor_directory(&token).unwrap().join("index");
    let linked_index = fs::read(&linked_index_path).unwrap();
    let refs = f.git(&["show-ref"]);
    assert!(squash_saved::acknowledge_external_recovery(&linked_repo, &main_pending, &token).is_err());
    assert!(squash_saved::acknowledge_external_recovery(&repo, &linked_pending, &token).is_err());
    assert_eq!(fs::read(&main_path).unwrap(), b"identical recovery record");
    assert_eq!(fs::read(&linked_path).unwrap(), b"identical recovery record");
    squash_saved::acknowledge_external_recovery(&linked_repo, &linked_pending, &token).unwrap();
    assert!(!linked_path.exists()); assert!(main_path.exists());
    assert_eq!(f.git(&["rev-parse", "refs/stash"]), stash);
    assert_eq!(f.git(&["show-ref"]), refs);
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), main_index);
    assert_eq!(fs::read(linked_index_path).unwrap(), linked_index);
    assert_eq!(fs::read(linked.join("file")).unwrap(), b"base\n");
}

#[test]
fn squash_external_recovery_refuses_stash_conflicts_without_merge_metadata() {
    use plico::squash_saved;
    let f = Fixture::new(); f.write("file", "base\n"); f.commit("base");
    f.write("file", "saved\n"); f.git(&["stash", "push"]);
    f.write("file", "new head\n"); f.commit("advance");
    let output = Command::new("git").arg("-C").arg(&f.0).args(["stash", "apply"]).output().unwrap();
    assert!(!output.status.success());
    let repo = f.repo(); let token = Cancel::default();
    let state = repo.operation_state(&token).unwrap();
    assert!(!state.in_progress()); assert!(!state.conflicts.is_empty());
    let path = squash_saved::path(&repo.editor_directory(&token).unwrap());
    fs::write(&path, b"saved changes recovery").unwrap();
    let pending = squash_saved::pending(&repo, &token).unwrap().unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let file = fs::read(f.0.join("file")).unwrap(); let refs = f.git(&["show-ref"]);
    assert!(squash_saved::acknowledge_external_recovery(&repo, &pending, &token).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"saved changes recovery");
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(f.0.join("file")).unwrap(), file);
    assert_eq!(f.git(&["show-ref"]), refs);
}

#[test]
fn squash_external_recovery_rejects_oversized_and_non_file_replacements() {
    use plico::squash_saved;
    let f = Fixture::new(); f.write("file", "base\n"); f.commit("base");
    let repo = f.repo(); let token = Cancel::default();
    let path = squash_saved::path(&repo.editor_directory(&token).unwrap());
    assert!(squash_saved::pending(&repo, &token).unwrap().is_none());
    fs::write(&path, vec![b'x'; 8192]).unwrap();
    let pending = squash_saved::pending(&repo, &token).unwrap().unwrap();
    fs::write(&path, vec![b'x'; 8193]).unwrap();
    assert!(squash_saved::pending(&repo, &token).is_err());
    assert!(squash_saved::acknowledge_external_recovery(&repo, &pending, &token).is_err());
    assert_eq!(fs::metadata(&path).unwrap().len(), 8193);
    fs::remove_file(&path).unwrap(); fs::create_dir(&path).unwrap();
    fs::write(path.join("keep"), b"must stay").unwrap();
    assert!(squash_saved::pending(&repo, &token).is_err());
    assert!(squash_saved::acknowledge_external_recovery(&repo, &pending, &token).is_err());
    assert_eq!(fs::read(path.join("keep")).unwrap(), b"must stay");
    fs::remove_dir_all(&path).unwrap();
    assert!(squash_saved::acknowledge_external_recovery(&repo, &pending, &token).is_err());
    #[cfg(unix)] {
        let target = f.0.join("outside-record"); fs::write(&target, b"do not remove").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(squash_saved::pending(&repo, &token).is_err());
        assert!(squash_saved::acknowledge_external_recovery(&repo, &pending, &token).is_err());
        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
        assert_eq!(fs::read(target).unwrap(), b"do not remove");
    }
}

#[test]
fn force_push_uses_explicit_lease_and_preserves_unknown_remote_commits() {
    use plico::{push::{self, Spec, Status}, push_target::{Target, Origin}};
    // Rebased GitPushOperationSingleRepoTest: known tip, stale tip, new target,
    // existing but unseen target. All writes are to temporary local repositories.
    let remote = Fixture::new(); remote.git(&["config", "core.bare", "true"]);
    let f = Fixture::new(); f.write("a", "base"); f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
    let mut spec = Spec { tags: None, force: false, source_branch: "main".into(), source_oid: base.clone(),
        target: Target { remote: "origin".into(), branch: "topic".into(), special: false, new_branch: true, origin: Origin::Custom }, set_upstream: false };
    let token = Cancel::default();
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::NewBranch);
    f.write("a", "first published"); f.commit("first"); spec.source_oid = f.git(&["rev-parse", "HEAD"]);
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::Success);
    f.git(&["reset", "--hard", &base]); f.write("a", "rewritten"); f.commit("rewritten");
    spec.source_oid = f.git(&["rev-parse", "HEAD"]);
    f.write("a", "uncommitted work"); f.write("untracked", "keep");
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::RejectedNonFf);
    spec.force = true;
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::Forced);
    assert_eq!(remote.git(&["rev-parse", "topic"]), spec.source_oid);
    let bro = Fixture::new(); bro.git(&["remote", "add", "origin", remote.0.to_str().unwrap()]);
    bro.git(&["fetch", "origin"]); bro.git(&["checkout", "-B", "topic", "origin/topic"]);
    bro.write("b", "other developer"); bro.commit("other developer"); bro.git(&["push", "origin", "topic"]);
    let other = bro.git(&["rev-parse", "HEAD"]);
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::RejectedStale);
    assert_eq!(remote.git(&["rev-parse", "topic"]), other);
    f.git(&["fetch", "origin"]);
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::Forced);
    assert_eq!(remote.git(&["rev-parse", "topic"]), spec.source_oid);
    spec.target.branch = "new-topic".into();
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::NewBranch);
    remote.git(&["update-ref", "refs/heads/unseen", &other]); spec.target.branch = "unseen".into();
    assert_eq!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).unwrap().status, Status::RejectedStale);
    assert_eq!(remote.git(&["rev-parse", "unseen"]), other);
    for branch in ["main", "master"] {
        spec.target.branch = branch.into(); assert!(!push::force_allowed(&spec.target));
        assert!(push::execute(&f.repo(), &spec, &token, &Default::default(), None).err().unwrap().contains("protected branch"));
    }
    assert_eq!(f.git(&["rev-parse", "HEAD"]), spec.source_oid);
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "uncommitted work");
    assert_eq!(fs::read_to_string(f.0.join("untracked")).unwrap(), "keep");
}

#[test]
fn push_tag_modes_match_rebased_all_and_current_branch() {
    use plico::{push::{self, Spec, Status, TagMode}, push_target::{Target, Origin}};
    let remote=Fixture::new();remote.git(&["config","core.bare","true"]);
    let f=Fixture::new();f.write("a","base");f.commit("base");
    f.git(&["remote","add","origin",remote.0.to_str().unwrap()]);
    f.git(&["tag","lightweight"]);f.git(&["tag","-a","annotated","-m","tag message"]);
    let mut spec=Spec{tags:None,force:false,source_branch:"main".into(),source_oid:f.git(&["rev-parse","HEAD"]),set_upstream:false,
        target:Target{remote:"origin".into(),branch:"topic".into(),special:false,new_branch:true,origin:Origin::Custom}};
    let token=Cancel::default();
    assert_eq!(push::execute(&f.repo(),&spec,&token,&Default::default(),None).unwrap().status,Status::NewBranch);
    assert!(remote.git(&["tag","--list"]).is_empty());
    spec.tags=Some(TagMode::CurrentBranch);
    assert_eq!(push::execute(&f.repo(),&spec,&token,&Default::default(),None).unwrap().status,Status::Success);
    assert_eq!(remote.git(&["tag","--list"]),"annotated");
    spec.tags=Some(TagMode::All);
    assert_eq!(push::execute(&f.repo(),&spec,&token,&Default::default(),None).unwrap().status,Status::Success);
    assert_eq!(remote.git(&["tag","--list"]),"annotated\nlightweight");
}
