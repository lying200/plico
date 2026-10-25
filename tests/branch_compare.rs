use plico::git::Repository;
use plico::{
    branch_compare::{Pair, Side},
    git::Cancel,
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
            "plico-branch-compare-{}-{}",
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
fn comparison_is_symmetric_paged_and_pinned_until_refresh() {
    let f = Fixture::new();
    let t = Cancel::default();
    f.git(&["checkout", "-b", "topic"]);
    for i in 0..3 {
        fs::write(f.0.join("a"), format!("topic {i}\n")).unwrap();
        f.git(&["commit", "-am", &format!("topic {i}")]);
    }
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("main-only"), "main\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "main only"]);
    fs::write(f.0.join("a"), "uncommitted\n").unwrap();
    let before = f.git(&["status", "--porcelain"]);
    let pair = Pair::resolve(&f.repo(), "refs/heads/topic", "refs/heads/main", &t).unwrap();
    assert_eq!(pair.count(&f.repo(), Side::First, &t).unwrap(), 3);
    assert_eq!(pair.count(&f.repo(), Side::Second, &t).unwrap(), 1);
    let page = pair.page(&f.repo(), Side::First, 0, 2, &t).unwrap();
    assert!(page.more);
    assert_eq!(page.commits.len(), 2);
    let end = pair.page(&f.repo(), Side::First, 2, 2, &t).unwrap();
    assert!(!end.more);
    assert_eq!(end.commits.len(), 1);
    assert_eq!(
        pair.reversed().count(&f.repo(), Side::First, &t).unwrap(),
        1
    );
    assert_eq!(pair.changes(&f.repo(), &t).unwrap().len(), 2);
    f.git(&["update-ref", "refs/heads/topic", &pair.second]);
    assert_eq!(pair.count(&f.repo(), Side::First, &t).unwrap(), 3);
    let refreshed = Pair::resolve(&f.repo(), "refs/heads/topic", "refs/heads/main", &t).unwrap();
    assert_eq!(refreshed.count(&f.repo(), Side::First, &t).unwrap(), 0);
    assert!(refreshed.changes(&f.repo(), &t).unwrap().is_empty());
    assert_eq!(f.git(&["status", "--porcelain"]), before);
    assert!(Pair::resolve(&f.repo(), "--all", "HEAD", &t).is_err());
    assert!(Pair::resolve(&f.repo(), "refs/heads/missing", "HEAD", &t).is_err());
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(
        pair.page(&f.repo(), Side::First, 0, 10, &cancelled)
            .is_err()
    );
}

#[test]
fn detail_filters_share_range_count_and_preserve_real_diff_parents() {
    use plico::git::{Author, LogFilter};
    let f = Fixture::new();
    let token = Cancel::default();
    f.git(&["checkout", "-b", "topic"]);
    fs::write(f.0.join("a"), "first\n").unwrap();
    f.git(&["commit", "-am", "Match first"]);
    let first = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("unrelated"), "middle\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "unrelated"]);
    let middle = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("a"), "last\n").unwrap();
    f.git(&["commit", "-am", "Match last"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("main-only"), "other\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "Match outside"]);
    let repo = f.repo();
    let pair = Pair::resolve(&repo, "refs/heads/topic", "refs/heads/main", &token).unwrap();
    let mut filter = LogFilter {
        text: "Match".into(),
        match_case: true,
        paths: vec!["a".into()],
        authors: Some(vec![Author {
            name: "Test".into(),
            email: "test@example.invalid".into(),
        }]),
        ..Default::default()
    };
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &filter, &token)
            .unwrap(),
        2
    );
    let page = pair
        .page_filtered(&repo, Side::First, 0, 1, &filter, &token)
        .unwrap();
    assert!(page.more);
    assert_eq!(page.commits[0].parents, vec![middle]);
    let last = pair
        .page_filtered(&repo, Side::First, 1, 1, &filter, &token)
        .unwrap();
    assert!(!last.more);
    assert_eq!(last.commits[0].id, first);
    assert_eq!(
        pair.count_filtered(&repo, Side::Second, &filter, &token)
            .unwrap(),
        0
    );
    filter.paths.clear();
    assert_eq!(
        pair.count_filtered(&repo, Side::Second, &filter, &token)
            .unwrap(),
        1
    );
    filter.text = "match".into();
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &filter, &token)
            .unwrap(),
        0
    );
    filter.match_case = false;
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &filter, &token)
            .unwrap(),
        2
    );
    filter.regex = true;
    filter.text = "^Match (first|last)$".into();
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &filter, &token)
            .unwrap(),
        2
    );
    filter.before = Some(1);
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &filter, &token)
            .unwrap(),
        0
    );
    filter.before = None;
    filter.authors = Some(vec![]);
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &filter, &token)
            .unwrap(),
        0
    );
    assert!(
        pair.page_filtered(&repo, Side::First, 0, 10, &filter, &token)
            .unwrap()
            .commits
            .is_empty()
    );
    filter = LogFilter {
        paths: vec!["../outside".into()],
        ..Default::default()
    };
    assert!(
        pair.count_filtered(&repo, Side::First, &filter, &token)
            .is_err()
    );
    assert!(
        pair.page_filtered(&repo, Side::First, 0, 10, &filter, &token)
            .is_err()
    );
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &LogFilter::default(), &token)
            .unwrap(),
        3
    );
}

#[test]
fn path_filters_treat_metacharacters_literally_and_keep_merge_parents() {
    use plico::git::LogFilter;
    let f = Fixture::new();
    let token = Cancel::default();
    f.git(&["checkout", "-b", "side"]);
    fs::write(f.0.join("literal[1].txt"), "side\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "side file"]);
    let side = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "-b", "topic", "main"]);
    fs::write(f.0.join("a"), "topic\n").unwrap();
    f.git(&["commit", "-am", "topic"]);
    let topic = f.git(&["rev-parse", "HEAD"]);
    f.git(&["merge", "--no-ff", "side", "-m", "merge"]);
    fs::write(f.0.join("literal[1].txt"), "combined\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "--amend", "--no-edit"]);
    let merge = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo();
    let pair = Pair::resolve(&repo, "refs/heads/topic", "refs/heads/main", &token).unwrap();
    let filter = LogFilter {
        paths: vec!["literal[1].txt".into()],
        ..Default::default()
    };
    let page = pair
        .page_filtered(&repo, Side::First, 0, 20, &filter, &token)
        .unwrap();
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &filter, &token)
            .unwrap(),
        page.commits.len()
    );
    let query = plico::branch_compare::Query::prepare(
        &repo,
        pair.clone(),
        Side::First,
        filter.clone(),
        &token,
    )
    .unwrap();
    let (projected, rows) = query.graph_page(&repo, 0, 20, &token).unwrap();
    assert_eq!(
        projected.commits.iter().map(|c| &c.id).collect::<Vec<_>>(),
        page.commits.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    assert_eq!(rows.len(), projected.commits.len());
    assert_eq!(
        projected
            .commits
            .iter()
            .find(|c| c.id == merge)
            .unwrap()
            .parents,
        [topic.clone(), side.clone()]
    );
    assert!(page.commits.iter().any(|c| c.id == side));
    assert_eq!(
        page.commits.iter().find(|c| c.id == merge).unwrap().parents,
        vec![topic, side]
    );
    let missing = LogFilter {
        paths: vec!["literal1.txt".into()],
        ..Default::default()
    };
    assert_eq!(
        pair.count_filtered(&repo, Side::First, &missing, &token)
            .unwrap(),
        0
    );
}

#[test]
fn hash_query_overrides_range_and_filters_but_clear_restores_range() {
    use plico::{branch_compare::Query, git::LogFilter};
    let f = Fixture::new();
    let token = Cancel::default();
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "-b", "topic"]);
    fs::write(f.0.join("a"), "topic\n").unwrap();
    f.git(&["commit", "-am", "topic"]);
    f.git(&["checkout", "-b", "other", "main"]);
    fs::write(f.0.join("other"), "other\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", &format!("references {}", &base[..9])]);
    let repo = f.repo();
    let pair = Pair::resolve(&repo, "refs/heads/topic", "refs/heads/main", &token).unwrap();
    let filter = LogFilter {
        text: base[..9].to_uppercase(),
        authors: Some(vec![]),
        before: Some(1),
        paths: vec!["missing".into()],
        ..Default::default()
    };
    let query = Query::prepare(&repo, pair.clone(), Side::First, filter, &token).unwrap();
    assert_eq!(query.count(&repo, &token).unwrap(), 2);
    let page = query.page(&repo, 0, 1, &token).unwrap();
    assert!(page.more);
    assert!(page.commits[0].subject.starts_with("references "));
    assert_eq!(query.page(&repo, 1, 1, &token).unwrap().commits[0].id, base);
    assert!(
        query
            .page(&repo, usize::MAX, 1, &token)
            .unwrap()
            .commits
            .is_empty()
    );
    f.git(&[
        "commit",
        "--allow-empty",
        "-m",
        &format!("new reference {}", &base[..9]),
    ]);
    assert_eq!(
        query.count(&repo, &token).unwrap(),
        2,
        "query stays pinned after refs move"
    );
    let (projected, rows) = query.graph_page(&repo, 0, 20, &token).unwrap();
    assert_eq!(projected.commits.len(), rows.len());
    assert_eq!(projected.commits.len(), query.count(&repo, &token).unwrap());

    let cleared = Query::prepare(
        &repo,
        pair.clone(),
        Side::First,
        LogFilter::default(),
        &token,
    )
    .unwrap();
    assert_eq!(cleared.count(&repo, &token).unwrap(), 1);
    assert_eq!(
        cleared.page(&repo, 0, 10, &token).unwrap().commits[0].subject,
        "topic"
    );
    let unmatched = Query::prepare(
        &repo,
        pair,
        Side::First,
        LogFilter {
            text: "0000000".into(),
            ..Default::default()
        },
        &token,
    )
    .unwrap();
    assert_eq!(unmatched.count(&repo, &token).unwrap(), 0);
    token.cancel();
    assert!(query.count(&repo, &token).is_err());
    assert!(query.page(&repo, 0, 10, &token).is_err());
}

#[test]
fn working_tree_comparison_has_explicit_endpoints_and_never_writes_index() {
    use plico::working_comparison::{self, Endpoint};
    let f = Fixture::new();
    fs::write(f.0.join("delete.txt"), "removed\n").unwrap();
    fs::write(f.0.join("edit.txt"), "base\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "comparison base"]);
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["mv", "a", "renamed.txt"]);
    fs::write(f.0.join("edit.txt"), "staged\n").unwrap();
    fs::write(f.0.join("added.txt"), "new\n").unwrap();
    f.git(&["add", "edit.txt", "added.txt"]);
    fs::write(f.0.join("edit.txt"), "working\n").unwrap();
    fs::remove_file(f.0.join("delete.txt")).unwrap();
    fs::write(f.0.join("untracked.txt"), "excluded\n").unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let token = Cancel::default();
    let changes = working_comparison::load(&f.repo(), &base, &token).unwrap();
    assert_eq!(changes.len(), 4);
    let (target, mut chain) =
        plico::diff_target::Chain::working_comparison(f.repo(), changes.clone(), 0).unwrap();
    assert_eq!(chain.len(), 4);
    assert!(target.read_contents(&token).unwrap().is_ok());
    let next = chain.step(true).unwrap();
    assert_eq!(chain.fork_for(&next).unwrap().index(), 1);
    let reversed_target = plico::diff_target::Target::working_comparison(
        f.repo(),
        next.identity()
            .working_comparison
            .as_ref()
            .unwrap()
            .reversed(),
    )
    .unwrap();
    assert_ne!(next.identity(), reversed_target.identity());
    assert!(chain.fork_for(&reversed_target).is_none());
    let mut reversed_chain = chain.reversed_working_comparison().unwrap();
    assert_eq!(reversed_chain.index(), chain.index());
    assert_eq!(
        reversed_chain.select(1).unwrap().identity(),
        reversed_target.identity()
    );
    assert_eq!(
        reversed_chain
            .reversed_working_comparison()
            .unwrap()
            .select(1)
            .unwrap()
            .identity(),
        next.identity()
    );

    assert_eq!(
        next.working_tree_document(),
        next.identity()
            .working_comparison
            .as_ref()
            .unwrap()
            .working_path()
    );
    let find = |path: &str| {
        changes
            .iter()
            .find(|c| {
                c.before
                    .iter()
                    .chain(c.after.iter())
                    .any(|e| e.path() == std::path::Path::new(path))
            })
            .unwrap()
    };
    let edit = find("edit.txt");
    assert!(matches!(edit.before, Some(Endpoint::WorkingTree { .. })));
    let contents = edit.read(&f.repo(), &token).unwrap();
    assert_eq!(contents.before.as_deref(), Some("working\n"));
    assert_eq!(contents.after.as_deref(), Some("base\n"));
    let reversed = edit.reversed();
    assert_eq!(reversed.reversed(), *edit);
    let contents = reversed.read(&f.repo(), &token).unwrap();
    assert_eq!(contents.after.as_deref(), Some("working\n"));
    assert_eq!(find("added.txt").status(), "D");
    assert_eq!(find("delete.txt").status(), "A");
    assert_eq!(find("renamed.txt").status(), "R");
    let rename_target =
        plico::diff_target::Target::working_comparison(f.repo(), find("renamed.txt").clone())
            .unwrap();
    assert_eq!(
        rename_target.source_path(),
        std::path::Path::new("renamed.txt")
    );
    assert_eq!(
        find("renamed.txt").after.as_ref().unwrap().path(),
        std::path::Path::new("a")
    );
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), base);
    fs::write(f.0.join("edit.txt"), "later disk edit\n").unwrap();
    assert_eq!(
        edit.read(&f.repo(), &token).unwrap().before.as_deref(),
        Some("later disk edit\n")
    );
    fs::write(f.0.join("edit.txt"), b"binary\0bytes").unwrap();
    assert!(edit.read(&f.repo(), &token).is_err());
    fs::remove_file(f.0.join("edit.txt")).unwrap();
    assert!(
        edit.read(&f.repo(), &token).is_err(),
        "deleted disk endpoint must not become empty content"
    );
    assert!(working_comparison::load(&f.repo(), "--stat", &token).is_err());
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(working_comparison::load(&f.repo(), &base, &cancelled).is_err());
    assert!(edit.read(&f.repo(), &cancelled).is_err());
}

#[test]
fn editable_comparison_preserves_disk_baseline_and_fixed_revision_in_both_directions() {
    use plico::working_comparison::{Change, Endpoint, WorkingSide};
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["mv", "a", "renamed.txt"]);
    fs::write(f.0.join("renamed.txt"), "new HEAD\n").unwrap();
    f.git(&["add", "renamed.txt"]);
    f.git(&["commit", "-m", "advance HEAD and rename"]);
    fs::write(f.0.join("renamed.txt"), "index content\n").unwrap();
    f.git(&["add", "renamed.txt"]);
    let disk = b"\xef\xbb\xbfworking\r\nsecond\r\n";
    fs::write(f.0.join("renamed.txt"), disk).unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    let repo = f.repo();
    let token = Cancel::default();
    let change = Change {
        before: Some(Endpoint::WorkingTree {
            path: "renamed.txt".into(),
        }),
        after: Some(Endpoint::Revision {
            commit: base,
            path: "a".into(),
        }),
    };
    for (change, side) in [
        (&change, WorkingSide::Before),
        (&change.reversed(), WorkingSide::After),
    ] {
        let target =
            plico::diff_target::Target::working_comparison(repo.clone(), change.clone()).unwrap();
        assert_eq!(
            target.working_tree_document(),
            Some(std::path::Path::new("renamed.txt"))
        );
        assert_eq!(
            plico::working_copy::read_base(&target, &token).unwrap(),
            "base\n"
        );
        let loaded = plico::working_copy::read(&target, &token).unwrap();
        assert_eq!(loaded.original, "base\n");
        assert_eq!(loaded.working.disk().as_ref(), disk);
        let mut editable = change.read_editable(&repo, &token).unwrap();
        assert_eq!(editable.path, std::path::Path::new("renamed.txt"));
        assert_eq!(editable.side, side);
        assert_eq!(
            editable.contents.original, "base\n",
            "neither current HEAD nor index is the comparison baseline"
        );
        assert_eq!(editable.contents.working.disk().as_ref(), disk);
        assert_eq!(editable.contents.working.text(), "working\nsecond\n");
        assert!(editable.contents.working.begin_save().unwrap().is_none());
        editable.contents.working.edit("accepted\n".into()).unwrap();
        let save = editable.contents.working.begin_save().unwrap().unwrap();
        assert_eq!(save.expected.as_ref(), disk);
        assert_eq!(save.bytes.as_ref(), b"\xef\xbb\xbfaccepted\r\n");
    }
    let added = Change {
        before: change.before.clone(),
        after: None,
    };
    assert!(
        added
            .read_editable(&repo, &token)
            .unwrap()
            .contents
            .original
            .is_empty()
    );
    assert!(
        Change {
            before: None,
            after: change.after.clone()
        }
        .read_editable(&repo, &token)
        .is_err()
    );
    assert!(
        Change {
            before: change.before.clone(),
            after: change.before.clone()
        }
        .read_editable(&repo, &token)
        .is_err()
    );
    assert!(
        Change {
            before: Some(Endpoint::WorkingTree {
                path: "../outside".into()
            }),
            after: None
        }
        .read_editable(&repo, &token)
        .is_err()
    );
    assert_eq!(fs::read(f.0.join("renamed.txt")).unwrap(), disk);
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    fs::write(f.0.join("renamed.txt"), b"binary\0data").unwrap();
    assert!(change.read_editable(&repo, &token).is_err());
    fs::remove_file(f.0.join("renamed.txt")).unwrap();
    assert!(
        change.read_editable(&repo, &token).is_err(),
        "missing document must not be recreated"
    );
    let target =
        plico::diff_target::Target::working_comparison(repo.clone(), change.clone()).unwrap();
    assert_eq!(
        plico::working_copy::read_base(&target, &token).unwrap(),
        "base\n",
        "baseline refresh does not depend on the deleted work file"
    );
    assert!(plico::working_copy::read(&target, &token).is_err());
    token.cancel();
    assert!(added.read_editable(&repo, &token).is_err());
}

#[test]
fn working_comparison_during_merge_preserves_conflict_stages_and_other_files() {
    use plico::working_comparison::{Endpoint, load};
    let f = Fixture::new();
    fs::write(f.0.join("other.txt"), "base\n").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "other"]);
    f.git(&["checkout", "-b", "topic"]);
    fs::write(f.0.join("a"), "topic\n").unwrap();
    f.git(&["commit", "-am", "topic"]);
    let revision = f.git(&["rev-parse", "HEAD"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("a"), "main\n").unwrap();
    f.git(&["commit", "-am", "main"]);
    let merge = Command::new("git")
        .arg("-C")
        .arg(&f.0)
        .args(["merge", "topic"])
        .output()
        .unwrap();
    assert!(!merge.status.success());
    fs::write(f.0.join("other.txt"), "working\n").unwrap();
    let stages = f.git(&["ls-files", "--stage"]);
    let head = f.git(&["rev-parse", "HEAD"]);
    let merge_head = f.git(&["rev-parse", "MERGE_HEAD"]);
    let conflict_bytes = fs::read(f.0.join("a")).unwrap();
    let token = Cancel::default();
    let changes = load(&f.repo(), &revision, &token).unwrap();
    assert_eq!(changes.len(), 2);
    let conflict = changes
        .iter()
        .find(|c| c.working_path() == Some(std::path::Path::new("a")))
        .unwrap();
    for change in [conflict.clone(), conflict.reversed()] {
        let contents = change.read_editable(&f.repo(), &token).unwrap().contents;
        assert_eq!(contents.original, "topic\n");
        assert_eq!(contents.working.disk().as_ref(), conflict_bytes);
        assert!(contents.working.text().contains("<<<<<<<"));
        assert!(contents.working.text().contains(">>>>>>>"));
        assert!(
            change
                .before
                .iter()
                .chain(change.after.iter())
                .any(|e| matches!(e, Endpoint::Revision { commit, .. } if commit == &revision))
        );
    }
    let other = changes
        .iter()
        .find(|c| c.working_path() == Some(std::path::Path::new("other.txt")))
        .unwrap();
    assert_eq!(
        other
            .read_editable(&f.repo(), &token)
            .unwrap()
            .contents
            .working
            .text(),
        "working\n"
    );
    assert_eq!(f.git(&["ls-files", "--stage"]), stages);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(f.git(&["rev-parse", "MERGE_HEAD"]), merge_head);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), conflict_bytes);
}

#[test]
fn comparison_graph_connects_hidden_commits_without_rewriting_diff_parents() {
    use plico::{branch_compare::Query, git::LogFilter};
    let f = Fixture::new();
    let token = Cancel::default();
    f.git(&["checkout", "-b", "topic"]);
    let mut ids = Vec::new();
    for (i, message) in ["visible first", "hidden middle", "visible last"]
        .iter()
        .enumerate()
    {
        fs::write(f.0.join("a"), format!("{i}\n")).unwrap();
        f.git(&["commit", "-am", message]);
        ids.push(f.git(&["rev-parse", "HEAD"]));
    }
    let pair = Pair::resolve(&f.repo(), "refs/heads/topic", "refs/heads/main", &token).unwrap();
    let query = Query::prepare(
        &f.repo(),
        pair.clone(),
        Side::First,
        LogFilter {
            text: "visible".into(),
            ..Default::default()
        },
        &token,
    )
    .unwrap();
    let (page, rows) = query.graph_page(&f.repo(), 0, 20, &token).unwrap();
    assert_eq!(
        page.commits.iter().map(|c| &c.id).collect::<Vec<_>>(),
        [&ids[2], &ids[0]]
    );
    assert_eq!(page.commits[0].parents, [ids[1].clone()]);
    assert!(
        rows.iter()
            .flat_map(|r| r.segments.iter())
            .any(|s| s.dotted())
    );
    assert!(!page.more);
    for offset in 0..2 {
        let (single, geometry) = query.graph_page(&f.repo(), offset, 1, &token).unwrap();
        assert_eq!(single.commits[0].id, page.commits[offset].id);
        assert_eq!(single.more, offset == 0);
        assert_eq!(geometry[0].segments, rows[offset].segments);
        assert_eq!(geometry[0].node, rows[offset].node);
    }
    assert_eq!(query.count(&f.repo(), &token).unwrap(), page.commits.len());
    let opposite =
        Query::prepare(&f.repo(), pair, Side::Second, LogFilter::default(), &token).unwrap();
    let (empty, rows) = opposite.graph_page(&f.repo(), 0, 20, &token).unwrap();
    assert!(empty.commits.is_empty() && rows.is_empty() && !empty.more);
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(query.graph_page(&f.repo(), 0, 20, &cancelled).is_err());
}

#[test]
fn remote_comparison_reads_renames_and_binary_without_touching_dirty_state() {
    use plico::combined_changes::read_contents;
    let f = Fixture::new();
    let token = Cancel::default();
    f.git(&["checkout", "-b", "topic"]);
    f.git(&["mv", "a", "renamed.txt"]);
    fs::write(f.0.join("image.bin"), [0, 1, 2, 255]).unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "rename and binary"]);
    f.git(&["update-ref", "refs/remotes/origin/topic", "HEAD"]);
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("a"), "staged\n").unwrap();
    f.git(&["add", "a"]);
    fs::write(f.0.join("a"), "unstaged\n").unwrap();
    fs::write(f.0.join("untracked.txt"), "untracked\n").unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let refs = f.git(&["show-ref"]);
    let head = f.git(&["symbolic-ref", "HEAD"]);
    let repo = f.repo();
    let pair = Pair::resolve(
        &repo,
        "refs/remotes/origin/topic",
        "refs/heads/main",
        &token,
    )
    .unwrap();
    assert_eq!(pair.count(&repo, Side::First, &token).unwrap(), 1);
    assert_eq!(pair.count(&repo, Side::Second, &token).unwrap(), 0);
    assert_eq!(
        pair.page(&repo, Side::First, 0, 10, &token)
            .unwrap()
            .commits
            .len(),
        1
    );
    let changes = pair.changes(&repo, &token).unwrap();
    assert_eq!(changes.len(), 2);
    let rename = changes
        .iter()
        .find(|c| {
            c.after
                .as_ref()
                .is_some_and(|r| r.path == PathBuf::from("renamed.txt"))
        })
        .unwrap();
    assert_eq!(rename.before.as_ref().unwrap().path, PathBuf::from("a"));
    let contents = read_contents(&repo, rename, &token).unwrap();
    assert_eq!(contents.before.as_deref(), Some("base\n"));
    assert_eq!(contents.after.as_deref(), Some("base\n"));
    let binary = changes
        .iter()
        .find(|c| {
            c.after
                .as_ref()
                .is_some_and(|r| r.path == PathBuf::from("image.bin"))
        })
        .unwrap();
    assert!(matches!(read_contents(&repo, binary, &token), Err(error) if error.contains("Binary")));
    let reversed = pair.reversed().changes(&repo, &token).unwrap();
    assert!(
        reversed
            .iter()
            .any(|c| c.before == rename.after && c.after == rename.before)
    );
    assert!(
        reversed
            .iter()
            .any(|c| c.before == binary.after && c.after.is_none())
    );
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"unstaged\n");
    assert_eq!(fs::read(f.0.join("untracked.txt")).unwrap(), b"untracked\n");
    assert_eq!(f.git(&["show-ref"]), refs);
    assert_eq!(f.git(&["symbolic-ref", "HEAD"]), head);
}
