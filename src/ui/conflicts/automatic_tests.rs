use super::*;
use core::prelude::v1::test;
extern crate gpui_kit as gpui;

#[gpui::test]
fn cancellation_and_batch_results_keep_iterative_review_state(cx: &mut gpui::TestAppContext) {
    use gpui::component::Root;
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("plico-auto-{}-{nonce}", std::process::id())));
    let root = fixture.0.clone();
    std::fs::create_dir(&root).unwrap();
    let git = |args: &[&str], success| {
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(o.status.success(), success, "{args:?}: {o:?}");
        String::from_utf8(o.stdout).unwrap()
    };
    git(&["init", "-b", "main"], true);
    git(&["config", "user.name", "Test"], true);
    git(&["config", "user.email", "test@example.invalid"], true);
    git(&["config", "commit.gpgsign", "false"], true);
    let content = |first: &str, last: &str| {
        format!(
            "{first}\n{}\n{last}\n",
            (0..8)
                .map(|i| format!("same {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    for path in ["a", "b", "c"] {
        std::fs::write(
            root.join(path),
            content("alpha beta gamma", "one two three"),
        )
        .unwrap();
    }
    git(&["add", "."], true);
    git(&["commit", "-qm", "base"], true);
    git(&["checkout", "-qb", "topic"], true);
    for path in ["a", "b", "c"] {
        std::fs::write(
            root.join(path),
            content(
                if path == "c" {
                    "theirs"
                } else {
                    "alpha beta GAMMA"
                },
                if path == "a" {
                    "one two THREE"
                } else {
                    "theirs end"
                },
            ),
        )
        .unwrap();
    }
    git(&["commit", "-qam", "topic"], true);
    git(&["checkout", "main"], true);
    for path in ["a", "b", "c"] {
        std::fs::write(
            root.join(path),
            content(
                if path == "c" {
                    "ours"
                } else {
                    "ALPHA beta gamma"
                },
                if path == "a" {
                    "ONE two three"
                } else {
                    "ours end"
                },
            ),
        )
        .unwrap();
    }
    git(&["commit", "-qam", "main"], true);
    git(&["merge", "topic"], false);
    let originals = ["a", "b", "c"].map(|p| std::fs::read(root.join(p)).unwrap());
    let repo = Repository::new(&root);
    cx.update(gpui::component::init);
    let mut review = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Review::new(root.clone(), 100, None, w, cx));
        review = Some(view.clone());
        Root::new(view, w, cx)
    });
    let review = review.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| review.update(cx, |s, cx| s.open_conflicts_in(repo.clone(), w, cx)));
    cx.run_until_parked();
    let session = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.resolve_conflicts_automatically(session.clone(), w, cx);
            s.operation.request_cancel();
        })
    });
    cx.run_until_parked();
    session.read_with(cx, |s, _| {
        assert!(!s.busy);
        assert!(!s.automatic.attempted);
        assert!(s.drafts.is_empty());
    });
    for (p, bytes) in ["a", "b", "c"].iter().zip(&originals) {
        assert_eq!(&std::fs::read(root.join(p)).unwrap(), bytes);
    }
    // Cancel from a real per-file adoption notification, after a successful
    // write. This exercises the asynchronous batch rather than pre-cancellation.
    let subscription = cx.update(|_, cx| {
        review.update(cx, |_, cx| {
            cx.observe(&session, |s, session, cx| {
                let batch = session.read(cx);
                if batch.busy
                    && batch
                        .drafts
                        .contains_key(&batch.key(std::path::Path::new("a")))
                {
                    s.operation.request_cancel();
                }
            })
        })
    });
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.resolve_conflicts_automatically(session.clone(), w, cx)
        })
    });
    cx.run_until_parked();
    drop(subscription);
    session.read_with(cx, |s, _| {
        assert!(!s.busy);
        assert!(s.error.is_none(), "{:?}", s.error);
        assert!(!s.automatic.attempted);
        assert_eq!(s.drafts.len(), 1);
        let saved = &s.drafts[&s.key(std::path::Path::new("a"))];
        assert_eq!(saved.decisions.progress(), (2, 2));
        assert!(!saved.reviewed);
    });
    assert_eq!(
        std::fs::read_to_string(root.join("a")).unwrap(),
        content("ALPHA beta GAMMA", "ONE two THREE")
    );
    for (p, bytes) in ["b", "c"].iter().zip(&originals[1..]) {
        assert_eq!(&std::fs::read(root.join(p)).unwrap(), bytes);
    }
    assert_eq!(git(&["ls-files", "-u"], true).lines().count(), 9);
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.resolve_conflicts_automatically(session.clone(), w, cx)
        })
    });
    cx.run_until_parked();
    session.read_with(cx, |s, _| {
        assert!(!s.busy);
        assert!(s.error.is_none(), "{:?}", s.error);
        assert!(s.automatic.attempted);
        assert_eq!(
            s.drafts[&s.key(std::path::Path::new("a"))]
                .decisions
                .progress(),
            (2, 2)
        );
        assert_eq!(
            s.drafts[&s.key(std::path::Path::new("b"))]
                .decisions
                .progress(),
            (1, 2)
        );
        assert!(!s.drafts.contains_key(&s.key(std::path::Path::new("c"))));
        assert_eq!(
            s.automatic.counts[&s.key(std::path::Path::new("c"))],
            (0, 2)
        );
        assert!(s.drafts.values().all(|d| !d.reviewed));
        assert!(!s.all_reviewed());
    });
    assert_eq!(
        std::fs::read_to_string(root.join("a")).unwrap(),
        content("ALPHA beta GAMMA", "ONE two THREE")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("b")).unwrap(),
        content("ALPHA beta GAMMA", "one two three")
    );
    assert_eq!(std::fs::read(root.join("c")).unwrap(), originals[2]);
    assert_eq!(git(&["ls-files", "-u"], true).lines().count(), 9);
    // Reverting an automatic result restores eligibility to run the action.
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.revert_conflict_resolution_confirmed(
                session.clone(),
                session.read(cx).key(std::path::Path::new("a")),
                w,
                cx,
            )
        })
    });
    cx.run_until_parked();
    session.read_with(cx, |s, _| {
        assert!(!s.automatic.attempted);
        assert!(s.automatic.message.is_none());
    });
}
