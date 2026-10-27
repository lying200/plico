use super::*;
use core::prelude::v1::test;
extern crate gpui_kit as gpui;
use gpui::component::Root;
struct Fixture(PathBuf);
impl Fixture {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "plico-interactive-ui-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-q", "-b", "main"]);
        f.git(&["config", "user.name", "Test"]);
        f.git(&["config", "user.email", "test@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["commit", "--allow-empty", "-qm", "initial"]);
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().to_owned()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[gpui::test]
fn initial_commit_warning_precedes_writes_and_captured_repository_is_used(
    cx: &mut gpui::TestAppContext,
) {
    let primary = Fixture::new("primary");
    let child = Fixture::new("child");
    child.git(&["commit", "--allow-empty", "-qm", "child second"]);
    let initial = child.git(&["rev-parse", "HEAD^"]);
    let child_head = child.git(&["rev-parse", "HEAD"]);
    let primary_head = primary.git(&["rev-parse", "HEAD"]);
    cx.update(gpui::component::init);
    let mut review = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Review::new(primary.0.clone(), 100, None, w, cx));
        review = Some(view.clone());
        Root::new(view, w, cx)
    });
    let review = review.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        review.update(cx, |s, cx| {
            s.prepare_interactive_rebase(Repository::new(&child.0), initial.clone(), w, cx)
        })
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        assert!(w.has_active_dialog(cx));
        review.read_with(cx, |s, _| {
            assert!(!s.operation.busy());
            assert!(!s.documents.locked());
            assert_eq!(s.repo.path, primary.0);
            assert_eq!(s.status_message, None);
        });
        let _ = w.draw(cx);
        w.close_dialog(cx);
    });
    assert_eq!(primary.git(&["rev-parse", "HEAD"]), primary_head);
    assert_eq!(child.git(&["rev-parse", "HEAD"]), child_head);
    assert!(!child.0.join(".git/rebase-merge").exists());
    cx.update(|_, cx| {
        review.update(cx, |s, cx| {
            s.select_log_row(0, false, false, false, cx);
            assert!(s.can_interactive_rebase(0, cx));
            let root = s.log.repository(0).unwrap().path;
            let mut commits = s.log.commits().as_ref().clone();
            commits[0].shallow_boundary = true;
            s.log = crate::ui::log_display::Display::fixture_single(&root, commits.clone());
            assert!(!s.can_interactive_rebase(0, cx));
            commits[0].shallow_boundary = false;
            commits[0].parents = vec!["a".repeat(40), "b".repeat(40)];
            s.log = crate::ui::log_display::Display::fixture_single(&root, commits);
            assert!(!s.can_interactive_rebase(0, cx));
        })
    });
}

#[gpui::test]
fn non_head_selection_uses_matching_cached_ancestry(cx: &mut gpui::TestAppContext) {
    let fixture = Fixture::new("ancestry");
    fixture.git(&["checkout", "-qb", "side"]);
    fixture.git(&["commit", "--allow-empty", "-qm", "side only"]);
    let side = fixture.git(&["rev-parse", "HEAD"]);
    fixture.git(&["checkout", "-q", "main"]);
    cx.update(gpui::component::init);
    let mut review = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Review::new(fixture.0.clone(), 100, None, w, cx));
        review = Some(view.clone());
        Root::new(view, w, cx)
    });
    let review = review.unwrap();
    cx.run_until_parked();
    let index = cx.update(|_, cx| {
        review.update(cx, |s, cx| {
            let index = s.log.commits().iter().position(|c| c.id == side).unwrap();
            s.select_log_row(index, false, false, false, cx);
            index
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        review.read_with(cx, |s, cx| {
            assert!(!s.can_interactive_rebase(index, cx));
            let cache = s.containing_branches.as_ref().unwrap().read(cx);
            assert_eq!(cache.contains_head(&fixture.0, &side), Some(false));
            assert_eq!(cache.contains_head(&fixture.0.join("other"), &side), None);
            assert_eq!(cache.contains_head(&fixture.0, &"a".repeat(40)), None);
        });
    });
    cx.update(|_, cx| {
        review.update(cx, |s, cx| {
            let index = s.log.commits().iter().position(|c| c.id != side).unwrap();
            s.select_log_row(index, false, false, false, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        review.read_with(cx, |s, cx| {
            assert!(s.can_interactive_rebase(s.selected.unwrap(), cx));
        });
    });
}

#[gpui::test]
fn protected_remote_commit_disables_interactive_entry(cx: &mut gpui::TestAppContext) {
    let fixture = Fixture::new("protected");
    let head = fixture.git(&["rev-parse", "HEAD"]);
    fixture.git(&["remote", "add", "test", "."]);
    fixture.git(&["update-ref", "refs/remotes/test/main", &head]);
    cx.update(gpui::component::init);
    let mut review = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Review::new(fixture.0.clone(), 100, None, w, cx));
        review = Some(view.clone());
        Root::new(view, w, cx)
    });
    let review = review.unwrap();
    cx.run_until_parked();
    cx.update(|_, cx| review.update(cx, |s, cx| s.select_log_row(0, false, false, false, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        review.read_with(cx, |s, cx| {
            let query = s.containing_branches.as_ref().unwrap().read(cx);
            assert_eq!(query.contains_head(&fixture.0, &head), Some(true));
            assert_eq!(query.is_protected(&fixture.0, &head), Some(true));
            assert_eq!(query.is_protected(&fixture.0.join("other"), &head), None);
            assert!(!s.can_interactive_rebase(0, cx));
        })
    });
}

#[gpui::test]
fn operation_gate_is_scoped_to_selected_repository(cx: &mut gpui::TestAppContext) {
    use plico::operation_state::{Operation, State};
    let fixture = Fixture::new("operation");
    let other = Fixture::new("other-operation");
    cx.update(gpui::component::init);
    let mut review = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| Review::new(fixture.0.clone(), 100, None, w, cx));
        review = Some(view.clone());
        Root::new(view, w, cx)
    });
    let review = review.unwrap();
    cx.run_until_parked();
    cx.update(|_, cx| review.update(cx, |s, cx| s.select_log_row(0, false, false, false, cx)));
    cx.run_until_parked();
    for operation in [
        Operation::Merge,
        Operation::RebaseApply,
        Operation::RebaseMerge,
        Operation::CherryPick,
        Operation::Revert,
        Operation::Sequencer,
    ] {
        for root in [&other.0, &fixture.0] {
            cx.update(|_, cx| {
                let roots = review.read(cx).project_roots.clone();
                let observed = roots.read(cx).observation_roots();
                roots.update(cx, |s, cx| {
                    assert!(s.publish_operations(
                        &observed,
                        Arc::new(vec![plico::project_operations::Entry {
                            repository: Repository::new(root),
                            state: Ok(Arc::new(State {
                                operation,
                                detached: false,
                                rebase_branch: None,
                                conflicts: vec![],
                            })),
                        }]),
                        cx
                    ));
                });
                assert_eq!(
                    review.read(cx).can_interactive_rebase(0, cx),
                    root != &fixture.0
                );
            });
        }
    }
    cx.update(|_, cx| {
        let roots = review.read(cx).project_roots.clone();
        let observed = roots.read(cx).observation_roots();
        roots.update(cx, |s, cx| {
            assert!(s.publish_operations(
                &observed,
                Arc::new(vec![plico::project_operations::Entry {
                    repository: Repository::new(&fixture.0),
                    state: Ok(Arc::new(State {
                        operation: Operation::None,
                        detached: true,
                        rebase_branch: None,
                        conflicts: vec![],
                    })),
                }]),
                cx
            ));
        });
        assert!(review.read(cx).can_interactive_rebase(0, cx));
    });
}
