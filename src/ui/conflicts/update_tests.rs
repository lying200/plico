use super::*;
use core::prelude::v1::test;
extern crate gpui_kit as gpui;

#[gpui::test]
fn update_conflict_actions_keep_main_task_and_continue_owned_session(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::component::Root;
    use plico::{
        project_update::{self, conflicts::channel},
        published_commits::Protection,
        update_method::Method,
    };
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).unwrap();
    let fixture = Fixture(std::env::temp_dir().join(format!("plico-update-ui-{nonce:x?}")));
    let root = fixture.0.join("working");
    std::fs::create_dir_all(&root).unwrap();
    let git = |path: &PathBuf, args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}: {out:?}");
    };
    let setup = |path: &PathBuf| {
        for args in [
            ["config", "user.name", "Test"],
            ["config", "user.email", "test@example.invalid"],
            ["config", "commit.gpgsign", "false"],
            ["config", "core.hooksPath", "disabled-hooks"],
        ] {
            git(path, &args);
        }
    };
    git(&root, &["init", "-b", "main"]);
    setup(&root);
    std::fs::write(root.join("file"), "base\n").unwrap();
    std::fs::write(root.join("draft"), "clean\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "base"]);
    let remote = fixture.0.join("remote.git");
    git(&root, &["init", "--bare", remote.to_str().unwrap()]);
    git(
        &root,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&root, &["push", "-u", "origin", "main"]);
    let writer = fixture.0.join("writer");
    git(
        &root,
        &[
            "clone",
            "-b",
            "main",
            remote.to_str().unwrap(),
            writer.to_str().unwrap(),
        ],
    );
    setup(&writer);
    std::fs::write(writer.join("file"), "remote\n").unwrap();
    git(&writer, &["commit", "-qam", "remote"]);
    git(&writer, &["push", "origin", "main"]);
    std::fs::write(root.join("file"), "local\n").unwrap();
    git(&root, &["commit", "-qam", "local"]);
    std::fs::write(root.join("draft"), "unsaved draft\n").unwrap();
    let repo = Repository::new(&root);
    let prepared = project_update::Prepared::read(&repo, &Cancel::default()).unwrap();
    cx.update(gpui::component::init);
    let mut review = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Review::new(root.clone(), 100, None, w, cx));
        review = Some(view.clone());
        Root::new(view, w, cx)
    });
    let review = review.unwrap();
    cx.run_until_parked();
    struct MainTask(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for MainTask {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let main_dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let marker = main_dropped.clone();
    let (id, token) = cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            let operation = s
                .begin_operation(
                    plico::active_operation::Kind::Update,
                    plico::active_operation::Phase::Executing,
                    cx,
                )
                .unwrap();
            s.operation_task = cx.spawn_in(w, async move |_, _| {
                let _marker = MainTask(marker);
                std::future::pending::<()>().await;
            });
            operation
        })
    });
    cx.run_until_parked();
    let (worker, ui) = channel::channel();
    let cancel = token.clone();
    let (done, stopped) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if stopped
            .recv_timeout(std::time::Duration::from_secs(45))
            .is_err()
        {
            cancel.cancel();
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
            |i| worker.resolve(i),
        )
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let pending = loop {
        if let Some(pending) = ui.take_pending() {
            break pending;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    cx.update(|w, cx| review.update(cx, |s, cx| s.open_update_conflicts(pending, w, cx)));
    cx.run_until_parked();
    let view = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
    let entry = view.read_with(cx, |s, _| s.entries[0].clone());
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            let view = view.clone();
            let entry = entry.clone();
            s.with_conflict_documents(w, cx, move |s, w, cx| {
                s.accept_conflict_in_session_confirmed(view, entry, Side::Yours, w, cx);
            });
        })
    });
    cx.run_until_parked();
    view.read_with(cx, |s, _| {
        assert!(s.all_resolved(), "{:?}", s.error);
        assert!(!s.busy);
    });
    assert_eq!(std::fs::read(root.join("file")).unwrap(), b"local\n");
    assert!(!main_dropped.load(std::sync::atomic::Ordering::SeqCst));
    assert!(review.read_with(cx, |s, _| {
        s.operation
            .is_running(plico::active_operation::Kind::Update)
    }));
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.revert_conflict_resolution_confirmed(view.clone(), entry.key().clone(), w, cx)
        })
    });
    cx.run_until_parked();
    assert_eq!(std::fs::read(root.join("file")).unwrap(), b"base\n");
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.resolve_conflicts_automatically(view.clone(), w, cx)
        })
    });
    cx.run_until_parked();
    view.read_with(cx, |s, _| assert!(s.error.is_none(), "{:?}", s.error));
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.accept_conflict_in_session_confirmed(view.clone(), entry.clone(), Side::Yours, w, cx)
        })
    });
    cx.run_until_parked();
    // Enter the real three-way editor while the outer Update still owns Git.
    cx.update(|w, cx| view.update(cx, |s, cx| s.merge(w, cx)));
    cx.run_until_parked();
    let draft = view.read_with(cx, |s, _| s.drafts.get(entry.key()).unwrap().clone());
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.apply_merge_result(
                draft.prepared.clone(),
                "edited result\n".into(),
                draft.decisions.clone(),
                Box::new(|ok, w, cx| {
                    assert!(ok);
                    w.close_dialog(cx);
                    w.close_dialog(cx);
                }),
                w,
                cx,
            );
        })
    });
    cx.run_until_parked();
    let view = review.read_with(cx, |s, _| s.conflict_session.clone().unwrap());
    assert!(!main_dropped.load(std::sync::atomic::Ordering::SeqCst));
    assert!(review.read_with(cx, |s, _| {
        s.operation
            .is_running(plico::active_operation::Kind::Update)
    }));
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.close_conflict_session(view.clone(), true, w, cx)
        })
    });
    cx.run_until_parked();
    let report = task.join().unwrap().unwrap();
    assert!(report.complete(), "{:?}", report.diagnostic());
    assert_eq!(
        std::fs::read(root.join("draft")).unwrap(),
        b"unsaved draft\n"
    );
    assert_eq!(
        std::fs::read(root.join("file")).unwrap(),
        b"edited result\n"
    );
    assert!(!main_dropped.load(std::sync::atomic::Ordering::SeqCst));
    cx.update(|_, cx| {
        review.update(cx, |s, cx| {
            assert_eq!(s.finish_operation(id, cx), Some(false));
            s.operation_task = Task::ready(());
        })
    });
    done.send(()).unwrap();
    watchdog.join().unwrap();
}
