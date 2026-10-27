use super::*;
use core::prelude::v1::test;
extern crate gpui_kit as gpui;

#[gpui::test]
fn project_list_keeps_same_name_drafts_and_stages_each_root(cx: &mut gpui::TestAppContext) {
    use gpui::component::Root;
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).unwrap();
    let fixture = Fixture(std::env::temp_dir().join(format!("plico-project-conflicts-{nonce:x?}")));
    std::fs::create_dir_all(&fixture.0).unwrap();
    let git = |repo: &Repository, args: &[&str], success: bool| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.success(), success, "{args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    let roots = ["parent", "child"].map(|name| Repository::new(fixture.0.join(name)));
    for repo in &roots {
        std::fs::create_dir(&repo.path).unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            git(repo, &args, true);
        }
        std::fs::write(repo.path.join("same.txt"), "alpha beta gamma\n").unwrap();
        git(repo, &["add", "."], true);
        git(repo, &["commit", "-qm", "base"], true);
        git(repo, &["checkout", "-qb", "topic"], true);
        std::fs::write(repo.path.join("same.txt"), "alpha beta GAMMA\n").unwrap();
        git(repo, &["commit", "-qam", "theirs"], true);
        git(repo, &["checkout", "main"], true);
        std::fs::write(repo.path.join("same.txt"), "ALPHA beta gamma\n").unwrap();
        git(repo, &["commit", "-qam", "ours"], true);
        git(repo, &["merge", "topic"], false);
    }
    let heads = roots
        .each_ref()
        .map(|r| git(r, &["rev-parse", "HEAD"], true));
    cx.update(gpui::component::init);
    let mut review = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Review::new(roots[0].path.clone(), 100, None, w, cx));
        review = Some(view.clone());
        Root::new(view, w, cx)
    });
    let review = review.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.project_roots =
                cx.new(|cx| super::super::project_roots::ProjectRoots::new(roots.to_vec(), cx));
            s.open_conflicts(w, cx);
        })
    });
    cx.run_until_parked();
    let session = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
    session.read_with(cx, |s, _| {
        assert_eq!(s.entries.len(), 2);
        assert_ne!(s.entries[0].key(), s.entries[1].key());
        assert!(s.continuation.is_none());
    });
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.resolve_conflicts_automatically(session.clone(), w, cx)
        })
    });
    cx.run_until_parked();
    session.read_with(cx, |s, _| {
        assert_eq!(s.drafts.len(), 2);
        assert!(s.all_resolved());
        assert!(!s.all_reviewed());
        assert!(s.error.is_none(), "{:?}", s.error);
    });
    for repo in &roots {
        assert_eq!(
            std::fs::read_to_string(repo.path.join("same.txt")).unwrap(),
            "ALPHA beta GAMMA\n"
        );
        assert_eq!(git(repo, &["ls-files", "-u"], true).lines().count(), 3);
    }
    // Overwrite only one same-named file, then close the aggregate dialog.
    let entry = session.read_with(cx, |s, _| s.entries[0].clone());
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.accept_conflict_in_session_confirmed(
                session.clone(),
                entry.clone(),
                Side::Theirs,
                w,
                cx,
            )
        })
    });
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(entry.full_path()).unwrap(),
        "alpha beta GAMMA\n"
    );
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.close_conflict_session_confirmed(session.clone(), true, w, cx)
        })
    });
    cx.run_until_parked();
    assert!(review.read_with(cx, |s, _| s.conflict_session.is_none()));
    for (repo, head) in roots.iter().zip(&heads) {
        assert!(git(repo, &["ls-files", "-u"], true).trim().is_empty());
        assert_eq!(&git(repo, &["rev-parse", "HEAD"], true), head);
        assert!(
            repo.path.join(".git/MERGE_HEAD").exists(),
            "manual project dialog must not commit"
        );
    }
}

#[gpui::test]
fn closing_rebase_conflicts_keeps_rebase_paused(cx: &mut gpui::TestAppContext) {
    use gpui::component::Root;
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).unwrap();
    let fixture = Fixture(std::env::temp_dir().join(format!("plico-rebase-intent-{nonce:x?}")));
    std::fs::create_dir_all(&fixture.0).unwrap();
    let git = |args: &[&str], success: bool| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&fixture.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.success(), success, "{args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.name", "Test"],
        vec!["config", "user.email", "test@example.invalid"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        git(&args, true);
    }
    std::fs::write(fixture.0.join("file"), "base\n").unwrap();
    git(&["add", "."], true);
    git(&["commit", "-qm", "base"], true);
    git(&["checkout", "-qb", "topic"], true);
    std::fs::write(fixture.0.join("file"), "topic\n").unwrap();
    git(&["commit", "-qam", "topic"], true);
    git(&["checkout", "main"], true);
    std::fs::write(fixture.0.join("file"), "main\n").unwrap();
    git(&["commit", "-qam", "main"], true);
    git(&["checkout", "topic"], true);
    git(&["rebase", "main"], false);
    let head = git(&["rev-parse", "HEAD"], true);
    let index = git(&["ls-files", "-u"], true);
    let repo = Repository::new(&fixture.0);
    cx.update(gpui::component::init);
    let mut review = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Review::new(repo.path.clone(), 100, None, w, cx));
        review = Some(view.clone());
        Root::new(view, w, cx)
    });
    let review = review.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| review.update(cx, |s, cx| s.choose_rebase_root(false, w, cx)));
    cx.run_until_parked();
    let session = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
    session.read_with(cx, |s, _| {
        assert!(matches!(s.continuation, Some(Continuation::Rebase(_))));
        assert_eq!(s.entries.len(), 1);
    });
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.close_conflict_session(session.clone(), false, w, cx)
        })
    });
    cx.run_until_parked();
    assert_eq!(git(&["rev-parse", "HEAD"], true), head);
    assert_eq!(git(&["ls-files", "-u"], true), index);
    assert!(fixture.0.join(".git/rebase-merge").exists());
    cx.update(|w, cx| review.update(cx, |s, cx| s.open_conflicts(w, cx)));
    cx.run_until_parked();
    review.read_with(cx, |s, cx| {
        assert!(
            s.conflict_session
                .as_ref()
                .unwrap()
                .read(cx)
                .continuation
                .is_none()
        );
    });
}
