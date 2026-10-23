use plico::{
    file_history as history,
    git::{Cancel, Repository},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "gitui-file-history-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        let f = Self(p);
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Reviewer"]);
        f.git(&["config", "user.email", "review@example.invalid"]);
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().into()
    }
    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn file_history_follows_renames_stops_at_recreation_and_pages() {
    let f = Fixture::new();
    f.write("old.txt", "one\ntwo\nthree\n");
    let a = f.commit("created\n\nbody");
    f.write("old.txt", "one\nTWO\nthree\n");
    let b = f.commit("edited");
    f.git(&["mv", "old.txt", "中文 [x].txt"]);
    let c = f.commit("renamed");
    f.write("中文 [x].txt", "zero\none\nTWO\nthree\n");
    let d = f.commit("shifted");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let h = history::load(&repo, Path::new("中文 [x].txt"), 100, &token).unwrap();
    assert_eq!(
        h.revisions
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        vec![d.as_str(), c.as_str(), b.as_str(), a.as_str()]
    );
    assert_eq!(h.revisions[2].path, Path::new("old.txt"));
    assert_eq!(h.revisions[3].message, "created\n\nbody");
    let p = history::load(&repo, Path::new("中文 [x].txt"), 2, &token).unwrap();
    assert_eq!(p.revisions.len(), 2);
    assert!(p.more);
    fs::remove_file(f.0.join("中文 [x].txt")).unwrap();
    f.commit("deleted");
    f.write("中文 [x].txt", "unrelated\n");
    let new = f.commit("recreated");
    let h = history::load(&repo, Path::new("中文 [x].txt"), 100, &token).unwrap();
    assert_eq!(h.revisions.len(), 1);
    assert_eq!(h.revisions[0].id, new);
    let token = Cancel::default();
    token.cancel();
    assert!(history::load(&repo, Path::new("中文 [x].txt"), 100, &token).is_err());
}
#[test]
fn selection_tracks_local_changes_and_ignores_unrelated_edits() {
    let f = Fixture::new();
    f.write("f.txt", "one\ntwo\nthree\n");
    let a = f.commit("created");
    f.write("f.txt", "one\nTWO\nthree\n");
    let b = f.commit("selected block edit");
    f.write("f.txt", "zero\none\nTWO\nthree\n");
    f.commit("unrelated insertion");
    f.write("f.txt", "zero\none\nlocal\nthree\n");
    let repo = Repository::new(&f.0);
    let t = Cancel::default();
    let h = history::load(&repo, Path::new("f.txt"), 100, &t).unwrap();
    let rows = history::trace(
        &repo,
        &h,
        &history::Selection {
            text: fs::read_to_string(f.0.join("f.txt")).unwrap(),
            lines: 2..3,
        },
        &t,
    )
    .unwrap();
    let changed: Vec<_> = rows.iter().filter(|r| r.changed).collect();
    assert_eq!(changed.len(), 3);
    assert!(changed[0].revision.is_none());
    assert_eq!(changed[1].revision.as_ref().unwrap().id, b);
    assert_eq!(changed[2].revision.as_ref().unwrap().id, a);
    assert_eq!(changed[1].block, "TWO");
    assert_eq!(changed[2].lines, 1..2);
    assert_eq!(
        fs::read_to_string(f.0.join("f.txt")).unwrap(),
        "zero\none\nlocal\nthree\n"
    );
}
#[test]
fn merged_rename_follows_the_parent_that_contains_the_original_file() {
    let f = Fixture::new();
    f.write("initial", "base");
    let base = f.commit("initial");
    f.write("unrelated", "main");
    f.commit("main unrelated");
    f.git(&["checkout", "-b", "feature", &base]);
    f.write("a.txt", "one\ntwo\nthree\n");
    let a = f.commit("created on feature");
    f.write("a.txt", "one\nTWO\nthree\n");
    let b = f.commit("edited on feature");
    f.git(&["checkout", "main"]);
    f.git(&["merge", "--no-ff", "--no-commit", "feature"]);
    f.git(&["mv", "a.txt", "b.txt"]);
    let merge = f.commit("merged rename");
    let h = history::load(
        &Repository::new(&f.0),
        Path::new("b.txt"),
        100,
        &Cancel::default(),
    )
    .unwrap();
    assert_eq!(
        h.revisions
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        vec![merge.as_str(), b.as_str(), a.as_str()]
    );
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let scoped =
        history::load_scoped(&repo, Path::new("b.txt"), Some("HEAD"), 100, &token).unwrap();
    let ids: Vec<_> = scoped
        .history
        .revisions
        .iter()
        .map(|r| r.id.clone())
        .collect();
    let mut selection = plico::log_selection::Selection::default();
    selection.select(&ids, 0, false, false);
    let c = history::selected_comparison(&scoped.history, &scoped.links, &selection)
        .unwrap()
        .unwrap();
    let before = c.before.unwrap();
    assert_eq!(before.id, b);
    assert_eq!(before.path, Path::new("a.txt"));
    assert_eq!(
        history::contents(&repo, &before, &token).unwrap(),
        "one\nTWO\nthree\n"
    );
    assert!(
        history::previous_revision(&repo, &scoped.history.revisions[0], &token)
            .unwrap()
            .is_none(),
        "fixture must distinguish visible file parent from an addition against the actual first parent"
    );
}

#[test]
fn cyclic_renames_and_untracked_files_do_not_mix_histories() {
    let f = Fixture::new();
    f.write("a.txt", "one\ntwo\n");
    let first = f.commit("create a");
    f.git(&["mv", "a.txt", "b.txt"]);
    let second = f.commit("a to b");
    f.git(&["mv", "b.txt", "a.txt"]);
    let third = f.commit("b back to a");
    let repo = Repository::new(&f.0);
    let t = Cancel::default();
    let h = history::load(&repo, Path::new("a.txt"), 100, &t).unwrap();
    assert_eq!(
        h.revisions
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        vec![third.as_str(), second.as_str(), first.as_str()]
    );
    assert!(
        history::load(&repo, Path::new("not-tracked"), 100, &t)
            .unwrap()
            .revisions
            .is_empty()
    );
    assert!(history::load(&repo, Path::new("../outside"), 100, &t).is_err());
}

#[test]
fn independent_branch_history_projects_hidden_commits_and_compares_real_parent() {
    let f = Fixture::new();
    f.write("f.txt", "base\n");
    let base = f.commit("base");
    f.git(&["checkout", "-b", "side"]);
    f.write("f.txt", "side\n");
    let side = f.commit("side edit");
    f.git(&["tag", "-a", "side-tag", "-m", "annotated"]);
    f.git(&["checkout", "main"]);
    f.write("other.txt", "hidden\n");
    let hidden = f.commit("unrelated");
    f.write("f.txt", "main\n");
    let main = f.commit("main edit");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let head = history::load_scoped(&repo, Path::new("f.txt"), Some("HEAD"), 100, &token).unwrap();
    assert_eq!(
        head.history
            .revisions
            .iter()
            .map(|r| &r.id)
            .collect::<Vec<_>>(),
        vec![&main, &base]
    );
    assert_eq!(head.links[0], vec![(base.clone(), true)]);
    assert_eq!(head.graph.len(), 2);
    assert_eq!(head.history.revisions[0].parents, vec![hidden.clone()]);
    let all = history::load_scoped(&repo, Path::new("f.txt"), None, 100, &token).unwrap();
    assert_eq!(all.history.revisions.len(), 3);
    for (r, links) in all.history.revisions.iter().zip(&all.links) {
        if r.id == main {
            assert_eq!(links, &vec![(base.clone(), true)]);
        }
        if r.id == side {
            assert_eq!(links, &vec![(base.clone(), false)]);
        }
    }
    let before = history::previous_revision(
        &repo,
        &all.history.revisions.iter().find(|r| r.id == main).unwrap(),
        &token,
    )
    .unwrap()
    .unwrap();
    assert_eq!(before.id, hidden);
    assert_eq!(history::contents(&repo, &before, &token).unwrap(), "base\n");
    for branch in ["refs/heads/side", "refs/tags/side-tag"] {
        let h = history::load_scoped(&repo, Path::new("f.txt"), Some(branch), 100, &token).unwrap();
        assert_eq!(
            h.history
                .revisions
                .iter()
                .map(|r| &r.id)
                .collect::<Vec<_>>(),
            vec![&side, &base]
        );
    }
    let page = history::load_scoped(&repo, Path::new("f.txt"), None, 1, &token).unwrap();
    assert_eq!(page.history.revisions.len(), 1);
    assert!(page.history.more);
    assert!(page.history.older.is_some());
    assert!(
        history::load_scoped(
            &repo,
            Path::new("f.txt"),
            Some("refs/heads/missing"),
            100,
            &token
        )
        .is_err()
    );
}

#[test]
fn scoped_history_crosses_rename_and_parent_preview_uses_original_path() {
    let f = Fixture::new();
    f.write("old.txt", "one\ntwo\nthree\n");
    let base = f.commit("created");
    f.git(&["mv", "old.txt", "new.txt"]);
    let renamed = f.commit("renamed");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let h = history::load_scoped(&repo, Path::new("new.txt"), Some("HEAD"), 100, &token).unwrap();
    assert_eq!(
        h.history
            .revisions
            .iter()
            .map(|r| &r.id)
            .collect::<Vec<_>>(),
        vec![&renamed, &base]
    );
    assert_eq!(h.links[0], vec![(base, false)]);
    let before = history::previous_revision(&repo, &h.history.revisions[0], &token)
        .unwrap()
        .unwrap();
    assert_eq!(before.path, Path::new("old.txt"));
    assert_eq!(
        history::contents(&repo, &before, &token).unwrap(),
        "one\ntwo\nthree\n"
    );
    assert!(
        history::previous_revision(&repo, &h.history.revisions[1], &token)
            .unwrap()
            .is_none()
    );
    f.write("new.txt", "different\n");
    f.commit("edited after rename");
    f.git(&[
        "clone",
        "--depth",
        "1",
        &format!("file://{}", f.0.display()),
        f.0.join("shallow").to_str().unwrap(),
    ]);
    let h = history::load_scoped(
        &Repository::new(f.0.join("shallow")),
        Path::new("new.txt"),
        Some("HEAD"),
        100,
        &token,
    )
    .unwrap();
    assert_eq!(h.history.revisions.len(), 1);
}

#[test]
fn multiselect_comparison_uses_extreme_selected_versions_across_rename() {
    let f = Fixture::new();
    f.write("old.txt", "base\n");
    let base = f.commit("created");
    f.write("old.txt", "middle\n");
    f.commit("middle version");
    f.git(&["mv", "old.txt", "new.txt"]);
    f.commit("rename");
    f.write("new.txt", "latest\n");
    let latest = f.commit("latest version");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let h = history::load_scoped(&repo, Path::new("new.txt"), Some("HEAD"), 100, &token)
        .unwrap()
        .history;
    let ids: Vec<_> = h.revisions.iter().map(|r| r.id.clone()).collect();
    let mut selection = plico::log_selection::Selection::default();
    assert_eq!(history::comparison_rows(&h, &selection), None);
    selection.select(&ids, 3, false, false);
    assert_eq!(history::comparison_rows(&h, &selection), Some((3, None)));
    // Gesture lead is latest, anchor oldest; endpoint order follows the table.
    selection.select(&ids, 0, true, false);
    assert_eq!(history::comparison_rows(&h, &selection), Some((0, Some(3))));
    let (after, before) = history::comparison_rows(&h, &selection).unwrap();
    let before = &h.revisions[before.unwrap()];
    assert_eq!((&h.revisions[after].id, &before.id), (&latest, &base));
    assert_eq!(before.path, Path::new("old.txt"));
    assert_eq!(history::contents(&repo, before, &token).unwrap(), "base\n");
    assert_eq!(
        history::contents(&repo, &h.revisions[after], &token).unwrap(),
        "latest\n"
    );
    // Adding an interior row leaves the two endpoints unchanged.
    selection.select(&ids, 1, true, false);
    assert_eq!(history::comparison_rows(&h, &selection), Some((0, Some(3))));
    selection.select(&ids, 0, true, false);
    assert_eq!(history::comparison_rows(&h, &selection), Some((1, Some(3))));
    selection.select(&ids, 3, true, false);
    selection.select(&ids, 1, true, false);
    assert_eq!(history::comparison_rows(&h, &selection), None);
}

#[test]
fn multiselect_history_restore_reconciles_ids_not_old_row_numbers() {
    let f = Fixture::new();
    for text in ["first", "second", "third"] {
        f.write("f.txt", text);
        f.commit(text);
    }
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let h = history::load_scoped(&repo, Path::new("f.txt"), Some("HEAD"), 100, &token)
        .unwrap()
        .history;
    let ids: Vec<_> = h.revisions.iter().map(|r| r.id.clone()).collect();
    let mut selected = plico::log_selection::Selection::default();
    selected.select(&ids, 0, false, false);
    selected.select(&ids, 2, false, true);
    let mut restored = selected.clone();
    f.write("f.txt", "fourth");
    f.commit("fourth");
    let refreshed = history::load_scoped(&repo, Path::new("f.txt"), Some("HEAD"), 100, &token)
        .unwrap()
        .history;
    let ids: Vec<_> = refreshed.revisions.iter().map(|r| r.id.clone()).collect();
    restored.reconcile(&ids);
    assert_eq!(
        history::comparison_rows(&refreshed, &restored),
        Some((1, Some(3)))
    );
    assert_eq!(restored.lead(&ids), Some(3));
    let short = history::History {
        revisions: refreshed.revisions[..3].to_vec(),
        more: true,
        older: None,
    };
    let short_ids: Vec<_> = short.revisions.iter().map(|r| r.id.clone()).collect();
    restored.reconcile(&short_ids);
    assert_eq!(
        history::comparison_rows(&short, &restored),
        Some((1, Some(2)))
    );
}

fn revision(id: u8, path: &str, deleted: bool) -> history::Revision {
    history::Revision {
        id: format!("{id:040x}"),
        parents: vec![],
        path: path.into(),
        deleted,
        timestamp: 0,
        author: String::new(),
        message: String::new(),
    }
}
#[test]
fn file_history_merges_parent_statuses_in_projected_row_order() {
    // FileHistoryUtil calls VcsChangesMerger.merge(path, changes), rather
    // than intersecting arbitrary change lists as the main Log does.
    let after = revision(1, "new.txt", false);
    let parents = [
        revision(2, "old-a.txt", false),
        revision(3, "old-b.txt", false),
    ];
    let h = history::History {
        revisions: vec![after, parents[0].clone(), parents[1].clone()],
        more: false,
        older: None,
    };
    let links = vec![
        vec![
            (parents[0].id.clone(), false),
            (parents[1].id.clone(), false),
        ],
        vec![],
        vec![],
    ];
    let mut selected = plico::log_selection::Selection::default();
    let ids = h.revisions.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    selected.select(&ids, 0, false, false);
    let c = history::selected_comparison(&h, &links, &selected)
        .unwrap()
        .unwrap();
    let before = c.before.unwrap();
    assert_eq!(before.id, parents[0].id);
    assert_eq!(
        before.path,
        Path::new("new.txt"),
        "different rename sources must merge into a modification at the selected path"
    );
}

#[test]
fn file_history_parent_merger_preserves_missing_empty_and_rename_states() {
    fn compare(path: &str, deleted: bool, parents: &[(&str, bool)]) -> Option<history::Comparison> {
        let h = history::History {
            revisions: std::iter::once(revision(1, path, deleted))
                .chain(
                    parents
                        .iter()
                        .enumerate()
                        .map(|(i, (p, d))| revision(i as u8 + 2, p, *d)),
                )
                .collect(),
            more: false,
            older: None,
        };
        let ids = h.revisions.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
        let mut selection = plico::log_selection::Selection::default();
        selection.select(&ids, 0, false, false);
        let mut links = vec![vec![]; ids.len()];
        links[0] = ids[1..].iter().map(|id| (id.clone(), true)).collect();
        history::selected_comparison(&h, &links, &selection).unwrap()
    }
    let same = compare("new", false, &[("old", false), ("old", false)]).unwrap();
    assert_eq!(same.before.unwrap().path, Path::new("old"));
    let mixed = compare("new", false, &[("new", false), ("old", false)]).unwrap();
    assert_eq!(mixed.before.unwrap().path, Path::new("new"));
    let add_modify = compare("new", false, &[("new", true), ("new", false)]).unwrap();
    assert_eq!(add_modify.before.unwrap().id, format!("{:040x}", 2));
    assert!(add_modify.after.is_some());
    let added = compare("new", false, &[("new", true), ("new", true)]).unwrap();
    assert!(added.before.is_none());
    let deleted = compare("gone", true, &[("gone", true), ("old", false)]).unwrap();
    assert_eq!(deleted.before.unwrap().id, format!("{:040x}", 3));
    assert!(deleted.after.is_none());
    assert!(compare("gone", true, &[("gone", true), ("gone", true)]).is_none());
    assert!(compare("root", false, &[]).unwrap().before.is_none());
    assert!(compare("root", true, &[]).is_none());
}

#[test]
fn real_merge_preview_uses_file_graph_parents_and_keeps_bounded_window_honest() {
    let f = Fixture::new();
    f.write("f.txt", "one\ntwo\nthree\n");
    let base = f.commit("base");
    f.git(&["checkout", "-b", "feature"]);
    f.write("f.txt", "one\ntwo\nTHREE\n");
    let feature = f.commit("feature edit");
    f.git(&["checkout", "main"]);
    f.write("f.txt", "ONE\ntwo\nthree\n");
    let main = f.commit("main edit");
    f.git(&["merge", "--no-ff", "-m", "merge both edits", "feature"]);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let h = history::load_scoped(&repo, Path::new("f.txt"), Some("HEAD"), 100, &token).unwrap();
    let ids = h
        .history
        .revisions
        .iter()
        .map(|r| r.id.clone())
        .collect::<Vec<_>>();
    let mut selection = plico::log_selection::Selection::default();
    selection.select(&ids, 0, false, false);
    assert_eq!(h.links[0].len(), 2);
    let comparison = history::selected_comparison(&h.history, &h.links, &selection)
        .unwrap()
        .unwrap();
    let request = history::preview_revisions(comparison.clone());
    assert_eq!(request.len(), 3);
    assert_eq!(request[0].as_ref().unwrap().id, h.links[0][0].0);
    assert_eq!(request[1].as_ref().unwrap().id, ids[0]);
    assert_eq!(request[2].as_ref().unwrap().id, h.links[0][1].0);
    let before = comparison.before.unwrap();
    assert_eq!(before.id, h.links[0][0].0);
    assert!([main, feature].contains(&before.id));
    assert_eq!(
        history::contents(&repo, &before, &token).unwrap().trim(),
        f.git(&["show", &format!("{}:f.txt", before.id)])
    );
    assert_eq!(
        history::contents(&repo, &comparison.after.unwrap(), &token).unwrap(),
        "ONE\ntwo\nTHREE\n"
    );
    // A range compares the two selected versions, not their merged parents.
    selection.select(
        &ids,
        ids.iter().position(|id| id == &base).unwrap(),
        true,
        false,
    );
    assert_eq!(
        history::selected_comparison(&h.history, &h.links, &selection)
            .unwrap()
            .unwrap()
            .before
            .unwrap()
            .id,
        base
    );
    let page = history::load_scoped(&repo, Path::new("f.txt"), Some("HEAD"), 1, &token).unwrap();
    selection.select(&[ids[0].clone()], 0, false, false);
    assert!(
        history::selected_comparison(&page.history, &page.links, &selection)
            .unwrap()
            .unwrap()
            .before
            .is_some()
    );
    // If projection still names a parent beyond retained metadata, do not turn
    // its absent cache entry into an absent file (a fabricated file creation).
    let clipped = history::History {
        revisions: vec![h.history.revisions[0].clone()],
        more: true,
        older: None,
    };
    assert!(
        history::selected_comparison(&clipped, &[h.links[0].clone()], &selection)
            .unwrap_err()
            .contains("Load more")
    );
}

#[test]
fn merge_request_keeps_both_parents_and_reduces_only_missing_sides() {
    fn request(left: bool, center: bool, right: bool) -> Vec<Option<history::Revision>> {
        let left = left.then(|| revision(2, "left", false));
        let center = center.then(|| revision(1, "merged", false));
        let right = right.then(|| revision(3, "right", false));
        history::preview_revisions(history::Comparison {
            before: left.clone(),
            after: center,
            source_before: vec![left, right],
        })
    }
    let ids = |s: Vec<Option<history::Revision>>| {
        s.into_iter().map(|r| r.map(|r| r.id)).collect::<Vec<_>>()
    };
    assert_eq!(
        ids(request(true, true, true)),
        [
            Some(format!("{:040x}", 2)),
            Some(format!("{:040x}", 1)),
            Some(format!("{:040x}", 3))
        ]
    );
    assert_eq!(
        ids(request(false, true, true)),
        [Some(format!("{:040x}", 1)), Some(format!("{:040x}", 3))]
    );
    assert_eq!(
        ids(request(true, true, false)),
        [Some(format!("{:040x}", 2)), Some(format!("{:040x}", 1))]
    );
    assert_eq!(
        ids(request(true, false, true)),
        [Some(format!("{:040x}", 2)), Some(format!("{:040x}", 3))]
    );
    assert_eq!(
        ids(request(false, true, false)),
        [Some(format!("{:040x}", 1)), None]
    );
}

#[test]
fn file_history_uses_default_long_edge_projection_without_changing_comparison_parents() {
    use std::io::Write as _;
    use std::process::Stdio;
    let fixture = Fixture::new();
    let mut import = Command::new("git")
        .arg("-C")
        .arg(&fixture.0)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = import.stdin.take().unwrap();
    for i in 0..100 {
        let message = format!("Revision {i:03}");
        let content = format!("revision {i}\n");
        write!(input, "commit refs/heads/main\nmark :{}\ncommitter Reviewer <review@example.invalid> {} +0000\ndata {}\n{}\n", i + 1, 1700000000 + i, message.len(), message).unwrap();
        if i > 0 {
            writeln!(input, "from :{i}").unwrap();
        }
        if i == 99 {
            writeln!(input, "merge :1").unwrap();
        }
        write!(
            input,
            "M 100644 inline history.txt\ndata {}\n{}\n",
            content.len(),
            content
        )
        .unwrap();
    }
    writeln!(input, "done").unwrap();
    drop(input);
    assert!(import.wait().unwrap().success());
    fixture.git(&["reset", "--hard", "HEAD"]);
    let repo = Repository::new(&fixture.0);
    let token = Cancel::default();
    let full = history::load_scoped(&repo, Path::new("history.txt"), None, 100, &token).unwrap();
    assert_eq!(full.history.revisions.len(), 100);
    let root = full.history.revisions[99].id.clone();
    assert_eq!(
        full.history.revisions[0].parents,
        [full.history.revisions[1].id.clone(), root.clone()]
    );
    assert_eq!(
        full.links[0],
        [(full.history.revisions[1].id.clone(), false), (root, false)]
    );
    assert_eq!(
        full.graph[2].width, 1,
        "file history must hide long-edge interiors like the main Log"
    );
    let down = full.graph[1]
        .segments
        .iter()
        .position(|s| s.arrow() && !s.upper())
        .unwrap();
    assert_eq!(full.graph[1].arrow_target(down), Some(99));
    let up = full.graph[98]
        .segments
        .iter()
        .position(|s| s.arrow() && s.upper())
        .unwrap();
    assert_eq!(full.graph[98].arrow_target(up), Some(0));
    let mut selected = plico::log_selection::Selection::default();
    selected.select(
        &full
            .history
            .revisions
            .iter()
            .map(|r| r.id.clone())
            .collect::<Vec<_>>(),
        0,
        false,
        false,
    );
    let comparison = history::selected_comparison(&full.history, &full.links, &selected)
        .unwrap()
        .unwrap();
    assert_eq!(comparison.before.unwrap().id, full.history.revisions[1].id);
    let partial = history::load_scoped(&repo, Path::new("history.txt"), None, 40, &token).unwrap();
    assert!(partial.history.more);
    assert_eq!(partial.graph.len(), 40);
    assert!(
        partial.graph.iter().all(|row| row
            .segments
            .iter()
            .enumerate()
            .all(|(index, _)| row.arrow_target(index).is_none_or(|target| target < 40))),
        "partial history must never expose an index from the untruncated candidates"
    );
}

#[test]
fn file_history_head_marker_tracks_checkout_not_first_file_revision() {
    let f = Fixture::new();
    f.write("review.txt", "base\n");
    let base = f.commit("file base");
    f.write("review.txt", "changed\n");
    let changed = f.commit("file change");
    f.write("unrelated.txt", "unrelated\n");
    let tip = f.commit("unrelated change");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let path = Path::new("review.txt");
    let h = history::load_scoped(&repo, path, None, 100, &token).unwrap();
    assert_eq!(h.head_id, tip);
    assert_eq!(h.history.revisions[0].id, changed);
    assert!(
        h.history.revisions.iter().all(|r| r.id != h.head_id),
        "a newer unrelated commit must not give the first file revision a HEAD marker"
    );
    f.git(&["checkout", "--detach", &base]);
    let h = history::load_scoped(&repo, path, Some("refs/heads/main"), 100, &token).unwrap();
    assert_eq!(h.head_id, base);
    assert_eq!(h.history.revisions[0].id, changed);
    assert_eq!(
        h.history
            .revisions
            .iter()
            .filter(|r| r.id == h.head_id)
            .count(),
        1
    );
    let h = history::load_scoped(&repo, path, Some("HEAD"), 100, &token).unwrap();
    assert_eq!(h.history.revisions.len(), 1);
    assert_eq!(h.history.revisions[0].id, h.head_id);
    assert_eq!(f.git(&["status", "--porcelain"]), "");
}

#[test]
fn background_scope_reaches_past_one_thousand_and_keeps_fixed_roots() {
    use std::io::Write;
    use std::process::Stdio;
    let f = Fixture::new();
    let mut input = Vec::new();
    for i in 0..1105 {
        let message = format!("revision {i}");
        let body = format!("{}revision {i}\n", "unchanged context\n".repeat(20));
        let path = if i < 600 { "old.txt" } else { "new.txt" };
        writeln!(input, "commit refs/heads/main\ncommitter Reviewer <review@example.invalid> {} +0000\ndata {}\n{}", 1700000000 + i, message.len(), message).unwrap();
        if i == 600 {
            writeln!(input, "D old.txt").unwrap();
        }
        writeln!(
            input,
            "M 100644 inline {path}\ndata {}\n{}",
            body.len(),
            body
        )
        .unwrap();
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(&f.0)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&input).unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    f.git(&["reset", "--hard", "HEAD"]);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let scope = history::Scope::open(&repo, Path::new("new.txt"), Some("HEAD"), &token).unwrap();
    let first = scope
        .load(&repo, Path::new("new.txt"), 100, &token)
        .unwrap();
    assert_eq!(first.history.revisions.len(), 100);
    assert!(first.history.more);
    f.write("new.txt", "later change\n");
    f.commit("later HEAD must not leak into fixed scope");
    let all = scope
        .load(&repo, Path::new("new.txt"), 2000, &token)
        .unwrap();
    assert_eq!(all.history.revisions.len(), 1105);
    assert!(!all.history.more);
    assert_eq!(all.history.revisions[0].message, "revision 1104");
    assert_eq!(all.history.revisions[1104].message, "revision 0");
    assert_eq!(all.history.revisions[1104].path, Path::new("old.txt"));
    assert_eq!(all.history.revisions[99].id, first.history.revisions[99].id);
    assert_eq!(all.graph.len(), 1105);
    assert_eq!(all.links.len(), 1105);
    token.cancel();
    assert!(
        scope
            .load(&repo, Path::new("new.txt"), 2000, &token)
            .is_err()
    );
}
