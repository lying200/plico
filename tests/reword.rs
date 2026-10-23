use plico::{
    git::{Cancel, Repository},
    reword::Prepared,
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
        let path = std::env::temp_dir().join(format!(
            "plico-reword-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Test"]);
        f.git(&["config", "user.email", "test@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.autocrlf", "false"]);
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f
    }
    fn command(&self) -> Command {
        let mut c = Command::new("git");
        c.arg("-C").arg(&self.0);
        c
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let out = self.command().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }
    fn write(&self, name: &str, content: &str) {
        fs::write(self.0.join(name), content).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Fixture {
    fn text(&self, args: &[&str]) -> String {
        String::from_utf8(self.git(args))
            .unwrap()
            .trim_end()
            .to_owned()
    }
    fn commit(&self, name: &str) -> String {
        self.git(&["add", "."]);
        self.git(&["commit", "--allow-empty", "-m", name]);
        self.text(&["rev-parse", "HEAD"])
    }
    fn prepare(&self, id: &str) -> Prepared {
        Prepared::prepare(&Repository::new(&self.0), id.into(), &Cancel::default()).unwrap()
    }
    fn rewrite(&self, p: &Prepared, msg: &str) -> plico::reword::Completed {
        p.execute(
            &Repository::new(&self.0),
            msg,
            &Cancel::default(),
            &Default::default(),
        )
        .unwrap()
    }
}
#[test]
fn head_and_older_messages_preserve_index_worktree_and_author() {
    for older in [false, true] {
        let f = Fixture::new();
        f.write("a", "original");
        let target = f.commit("old message");
        if older {
            f.write("b", "child");
            f.commit("child");
        }
        let tree = f.text(&["rev-parse", "HEAD^{tree}"]);
        let author = f.text(&["show", "-s", "--format=%an %ae %aI", &target]);
        f.write("a", "staged");
        f.git(&["add", "a"]);
        f.write("a", "unstaged");
        f.write("untracked", "keep");
        let status = f.git(&["status", "--porcelain"]);
        let index = f.git(&["show", ":a"]);
        let p = f.prepare(&target);
        assert!(p.initial);
        let r = f.rewrite(&p, "Correct message\n\n# retained body\n中文");
        assert!(r.warning.is_none(), "{:?}", r.warning);
        assert_eq!(
            f.text(&["show", "-s", "--format=%B", &r.target]),
            "Correct message\n\n# retained body\n中文"
        );
        assert_eq!(
            f.text(&["show", "-s", "--format=%an %ae %aI", &r.target]),
            author
        );
        assert_eq!(f.text(&["rev-parse", "HEAD^{tree}"]), tree);
        assert_eq!(f.git(&["status", "--porcelain"]), status);
        assert_eq!(f.git(&["show", ":a"]), index);
        assert_eq!(fs::read(f.0.join("a")).unwrap(), b"unstaged");
        assert_eq!(fs::read(f.0.join("untracked")).unwrap(), b"keep");
        if older {
            assert_eq!(f.text(&["log", "-1", "--format=%s"]), "child");
            assert_eq!(f.text(&["rev-parse", "HEAD^"]), r.target);
        }
    }
}
#[test]
fn selection_rules_and_stale_confirmation() {
    let f = Fixture::new();
    let root = f.commit("root");
    f.git(&["checkout", "-b", "side"]);
    let side = f.commit("side");
    f.git(&["checkout", "main"]);
    f.commit("main");
    let repo = Repository::new(&f.0);
    assert!(
        Prepared::prepare(&repo, side, &Cancel::default())
            .unwrap_err()
            .contains("HEAD history")
    );
    let p = f.prepare(&root);
    f.commit("new");
    assert!(
        p.execute(&repo, "new", &Cancel::default(), &Default::default())
            .is_err()
    );
    f.git(&["remote", "add", "origin", "https://example.invalid/repo"]);
    f.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    assert!(
        Prepared::prepare(&repo, root, &Cancel::default())
            .unwrap_err()
            .contains("protected")
    );
}

#[test]
fn protected_remote_identity_is_not_confused_with_local_branch_names() {
    let f = Fixture::new();
    let base = f.commit("base");
    let selected = f.commit("selected");
    f.git(&["remote", "add", "team/nested", "."]);
    // The protected remote does not contain selected. A similarly named local
    // branch must not make it look as though this commit has been published.
    f.git(&["update-ref", "refs/remotes/team/nested/main", &base]);
    f.git(&["branch", "team/nested/main", &selected]);
    let repo = Repository::new(&f.0);
    assert!(Prepared::prepare(&repo, selected.clone(), &Cancel::default()).is_ok());
    // A symbolic remote alias named master is not an independent remote branch.
    f.git(&["update-ref", "refs/remotes/team/nested/topic", &selected]);
    f.git(&[
        "symbolic-ref",
        "refs/remotes/team/nested/master",
        "refs/remotes/team/nested/topic",
    ]);
    let prepared = Prepared::prepare(&repo, selected.clone(), &Cancel::default()).unwrap();
    // A real remote tip containing selected must still disable editing.
    f.git(&["update-ref", "refs/remotes/team/nested/main", &selected]);
    assert!(
        Prepared::prepare(&repo, selected.clone(), &Cancel::default())
            .unwrap_err()
            .contains("protected branch: team/nested/main")
    );
    assert!(
        prepared
            .execute(
                &repo,
                "must not rewrite",
                &Cancel::default(),
                &Default::default()
            )
            .err()
            .unwrap()
            .contains("protected branch: team/nested/main")
    );
    assert_eq!(f.text(&["rev-parse", "HEAD"]), selected);
    assert_eq!(f.text(&["show", "-s", "--format=%s", "HEAD"]), "selected");
}
#[test]
fn merge_head_allowed_older_merge_disabled_and_topology_preserved() {
    let f = Fixture::new();
    let root = f.commit("root");
    f.git(&["checkout", "-b", "side"]);
    f.write("side", "side");
    f.commit("side");
    f.git(&["checkout", "main"]);
    f.write("main", "main");
    let target = f.commit("main");
    f.git(&["merge", "--no-ff", "side", "-m", "merge"]);
    let merge = f.text(&["rev-parse", "HEAD"]);
    let parents = f.text(&["show", "-s", "--format=%P", "HEAD"]);
    let r = f.rewrite(&f.prepare(&merge), "merge renamed");
    assert_eq!(f.text(&["show", "-s", "--format=%P", "HEAD"]), parents);
    f.commit("child");
    assert!(
        Prepared::prepare(&Repository::new(&f.0), r.target, &Cancel::default())
            .unwrap_err()
            .contains("Merge")
    );
    let tree = f.text(&["rev-parse", "HEAD^{tree}"]);
    let r = f.rewrite(&f.prepare(&target), "main renamed");
    assert_eq!(f.text(&["rev-parse", "HEAD^^1"]), r.target);
    assert_eq!(f.text(&["rev-parse", "HEAD^{tree}"]), tree);
    f.rewrite(&f.prepare(&root), "root renamed");
    assert_eq!(
        f.text(&["show", "-s", "--format=%P", "HEAD^"])
            .split_whitespace()
            .count(),
        2
    );
}
#[test]
fn cancellation_and_branch_switch_do_not_publish() {
    let f = Fixture::new();
    let head = f.commit("root");
    let p = f.prepare(&head);
    let token = Cancel::default();
    token.cancel();
    assert!(
        p.execute(&Repository::new(&f.0), "new", &token, &Default::default())
            .is_err()
    );
    assert_eq!(f.text(&["rev-parse", "HEAD"]), head);
    f.git(&["checkout", "-b", "other"]);
    assert!(
        p.execute(
            &Repository::new(&f.0),
            "new",
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    let p = f.prepare(&head);
    f.git(&["checkout", "--detach"]);
    assert!(
        p.execute(
            &Repository::new(&f.0),
            "new",
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    let r = f.rewrite(&f.prepare(&head), "detached rename");
    assert_eq!(f.text(&["rev-parse", "HEAD"]), r.target);
}
#[test]
fn notes_are_copied_and_whitespace_cleanup_is_applied() {
    let f = Fixture::new();
    let head = f.commit("root");
    f.git(&["config", "notes.rewriteRef", "refs/notes/commits"]);
    f.git(&["notes", "add", "-m", "a note"]);
    let r = f.rewrite(&f.prepare(&head), "  message  \n\nbody  \n\n");
    assert_eq!(f.text(&["notes", "show", &r.target]), "a note");
    assert_eq!(
        f.text(&["show", "-s", "--format=%B", "HEAD"]),
        "  message\n\nbody"
    );
}

#[test]
fn operation_state_allows_only_resolved_rebase_head() {
    let f = Fixture::new();
    let root = f.commit("root");
    let head = f.commit("head");
    let repo = Repository::new(&f.0);
    fs::create_dir(f.0.join(".git/rebase-merge")).unwrap();
    assert!(Prepared::prepare(&repo, head.clone(), &Cancel::default()).is_ok());
    assert!(Prepared::prepare(&repo, root, &Cancel::default()).is_err());
    fs::remove_dir(f.0.join(".git/rebase-merge")).unwrap();
    fs::write(f.0.join(".git/MERGE_HEAD"), &head).unwrap();
    assert!(Prepared::prepare(&repo, head, &Cancel::default()).is_err());
}

#[cfg(unix)]
#[test]
fn post_rewrite_receives_mapping_and_failure_does_not_repeat_commit() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let root = f.commit("root");
    let old_head = f.commit("head");
    f.git(&["config", "core.hooksPath", ".git/hooks"]);
    let hook = f.0.join(".git/hooks/post-rewrite");
    fs::write(
        &hook,
        "#!/bin/sh\ncat > .git/reword-mappings\necho hook-failed >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
    let done = f.rewrite(&f.prepare(&root), "new root");
    assert!(done.warning.unwrap().contains("hook-failed"));
    let map = fs::read_to_string(f.0.join(".git/reword-mappings")).unwrap();
    assert!(map.contains(&format!("{root} {}", done.target)));
    assert!(map.contains(&format!("{old_head} {}", f.text(&["rev-parse", "HEAD"]))));
    assert_eq!(f.text(&["show", "-s", "--format=%s", "HEAD^"]), "new root");
    assert!(!fs::read_dir(f.0.join(".git")).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("plico-post-rewrite-")
    }));
}

#[test]
fn prepared_message_cannot_be_published_in_another_worktree() {
    let f = Fixture::new();
    let head = f.commit("original");
    // Both worktrees have identical detached HEADs, so only repository
    // identity can reject applying this prepared operation to the other one.
    f.git(&["checkout", "--detach", &head]);
    let prepared = f.prepare(&head);
    let other = f.0.join("linked");
    f.git(&[
        "worktree",
        "add",
        "--detach",
        other.to_str().unwrap(),
        &head,
    ]);
    let repo = Repository::new(&other);
    assert!(
        prepared
            .execute(
                &repo,
                "wrong repository",
                &Cancel::default(),
                &Default::default()
            )
            .is_err()
    );
    assert_eq!(f.text(&["rev-parse", "HEAD"]), head);
    let actual = Command::new("git")
        .arg("-C")
        .arg(&other)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(actual.status.success());
    assert_eq!(String::from_utf8(actual.stdout).unwrap().trim(), head);
    f.git(&["worktree", "remove", other.to_str().unwrap()]);
}

#[test]
fn reword_keeps_other_branch_and_tag_references_at_original_commits() {
    let f = Fixture::new();
    let target = f.commit("original");
    f.git(&["branch", "shared", &target]);
    f.git(&["tag", "saved", &target]);
    f.commit("descendant");
    let old_head = f.text(&["rev-parse", "HEAD"]);
    f.git(&["branch", "old-tip", &old_head]);
    let tree = f.text(&["rev-parse", "HEAD^{tree}"]);
    let result = f.rewrite(&f.prepare(&target), "edited");
    assert_ne!(result.target, target);
    assert_eq!(f.text(&["rev-parse", "HEAD^"]), result.target);
    assert_eq!(f.text(&["rev-parse", "HEAD^{tree}"]), tree);
    assert_eq!(f.text(&["rev-parse", "refs/heads/shared"]), target);
    assert_eq!(f.text(&["rev-parse", "refs/tags/saved"]), target);
    assert_eq!(f.text(&["rev-parse", "refs/heads/old-tip"]), old_head);
}

#[test]
fn locked_reference_failure_preserves_state_and_allows_explicit_retry() {
    for detached in [false, true] {
        let f = Fixture::new();
        f.write("file", "base\n");
        let target = f.commit("original");
        f.commit("descendant");
        if detached {
            f.git(&["checkout", "--detach"]);
        }
        f.git(&["config", "notes.rewriteRef", "refs/notes/commits"]);
        f.git(&["notes", "add", "-m", "keep note", &target]);
        f.write("file", "staged\n");
        f.git(&["add", "file"]);
        f.write("file", "unstaged\n");
        let prepared = f.prepare(&target);
        let head = f.text(&["rev-parse", "HEAD"]);
        let refs = f.git(&["show-ref"]);
        let index = fs::read(f.0.join(".git/index")).unwrap();
        let head_log = fs::read(f.0.join(".git/logs/HEAD")).unwrap();
        let lock = f.0.join(if detached {
            ".git/HEAD.lock"
        } else {
            ".git/refs/heads/main.lock"
        });
        fs::write(&lock, b"another writer owns this lock\n").unwrap();
        let error = prepared
            .execute(
                &Repository::new(&f.0),
                "edited",
                &Cancel::default(),
                &Default::default(),
            )
            .err()
            .expect("publication must fail while the reference is locked");
        assert!(error.contains("cannot lock ref"), "{error}");
        assert_eq!(fs::read(&lock).unwrap(), b"another writer owns this lock\n");
        assert_eq!(f.text(&["rev-parse", "HEAD"]), head);
        assert_eq!(f.git(&["show-ref"]), refs);
        assert_eq!(fs::read(f.0.join(".git/logs/HEAD")).unwrap(), head_log);
        assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
        assert_eq!(fs::read(f.0.join("file")).unwrap(), b"unstaged\n");
        fs::remove_file(&lock).unwrap(); // Only the fixture's simulated writer releases it.
        let result = f.rewrite(&prepared, "edited");
        assert!(result.warning.is_none());
        assert_eq!(
            f.text(&["show", "-s", "--format=%s", &result.target]),
            "edited"
        );
        assert_eq!(f.text(&["notes", "show", &result.target]), "keep note");
        assert_eq!(f.text(&["show", ":file"]), "staged");
        assert_eq!(fs::read(f.0.join("file")).unwrap(), b"unstaged\n");
        assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    }
}

#[test]
fn prepared_message_rechecks_external_merge_without_head_change() {
    let f = Fixture::new();
    f.write("base", "base\n");
    let head = f.commit("base");
    f.git(&["checkout", "-b", "topic"]);
    f.write("added", "topic\n");
    f.commit("topic");
    f.git(&["checkout", "main"]);
    let repo = Repository::new(&f.0);
    let prepared = f.prepare(&head);
    let refs = f.git(&["show-ref"]);
    f.git(&["merge", "--no-ff", "--no-commit", "topic"]);
    assert_eq!(f.text(&["rev-parse", "HEAD"]), head);
    assert_eq!(f.git(&["show-ref"]), refs);
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let merge_head = fs::read(f.0.join(".git/MERGE_HEAD")).unwrap();
    let reflog = f.git(&["reflog", "show", "HEAD"]);
    let error = prepared
        .execute(
            &repo,
            "must not publish",
            &Cancel::default(),
            &Default::default(),
        )
        .err()
        .expect("external merge must reject publication");
    assert!(error.contains("another Git operation"), "{error}");
    assert!(Prepared::prepare(&repo, head.clone(), &Cancel::default()).is_err());
    assert_eq!(f.git(&["show-ref"]), refs);
    assert_eq!(f.git(&["reflog", "show", "HEAD"]), reflog);
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(f.0.join(".git/MERGE_HEAD")).unwrap(), merge_head);
    assert_eq!(fs::read(f.0.join("added")).unwrap(), b"topic\n");
    f.git(&["merge", "--quit"]);
    assert_eq!(f.git(&["show-ref"]), refs);
    assert!(Prepared::prepare(&repo, head, &Cancel::default()).is_ok());
    let done = f.rewrite(&prepared, "explicit retry after quit");
    assert!(done.warning.is_none());
    assert_eq!(
        f.text(&["show", "-s", "--format=%s", "HEAD"]),
        "explicit retry after quit"
    );
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(f.0.join("added")).unwrap(), b"topic\n");
}
