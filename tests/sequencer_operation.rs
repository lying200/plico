use plico::{
    git::{Cancel, Repository},
    operation_state::Operation,
    sequencer_operation::{self, Action},
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let f = Self(std::env::temp_dir().join(format!(
            "plico-sequencer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&f.0).unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.hooksPath", "disabled-hooks"],
        ] {
            f.git(&args, true);
        }
        f
    }
    fn git(&self, args: &[&str], ok: bool) -> String {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            o.status.success(),
            ok,
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8(o.stdout).unwrap().trim().to_owned()
    }
    fn commit(&self, value: &str) {
        fs::write(self.0.join("a"), value).unwrap();
        self.git(&["add", "a"], true);
        self.git(&["commit", "-m", value], true);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn existing_cherry_pick_and_revert_continue_or_abort() {
    for revert in [false, true] {
        for abort in [false, true] {
            let f = Fixture::new();
            f.commit("base");
            f.git(&["checkout", "-b", "task"], true);
            f.commit("task");
            let task = f.git(&["rev-parse", "HEAD"], true);
            if !revert {
                f.git(&["checkout", "main"], true);
            }
            f.commit("other");
            let before = f.git(&["rev-parse", "HEAD"], true);
            f.git(
                &[if revert { "revert" } else { "cherry-pick" }, &task],
                false,
            );
            let repo = Repository::new(&f.0);
            let token = Cancel::default();
            let kind = if revert {
                Operation::Revert
            } else {
                Operation::CherryPick
            };
            assert_eq!(repo.operation_state(&token).unwrap().operation, kind);
            let wrong = if revert {
                Operation::CherryPick
            } else {
                Operation::Revert
            };
            assert!(
                sequencer_operation::execute(
                    &repo,
                    wrong,
                    Action::Abort,
                    &token,
                    &Default::default()
                )
                .is_err()
            );
            assert!(
                sequencer_operation::execute(
                    &repo,
                    kind,
                    Action::Continue {
                        editor: "true".as_ref(),
                        requests: None
                    },
                    &token,
                    &Default::default()
                )
                .is_err()
            );
            if !abort {
                fs::write(f.0.join("a"), "resolved").unwrap();
                f.git(&["add", "a"], true);
            }
            let result = sequencer_operation::execute(
                &repo,
                kind,
                if abort {
                    Action::Abort
                } else {
                    Action::Continue {
                        editor: "true".as_ref(),
                        requests: None,
                    }
                },
                &token,
                &Default::default(),
            )
            .unwrap();
            let o = result.command.unwrap();
            assert!(o.status.success(), "{:?}", o.stderr);
            assert!(o.stopped.is_none());
            assert!(!result.state.unwrap().in_progress());
            assert_eq!(f.git(&["status", "--porcelain"], true), "");
            if abort {
                assert_eq!(before, f.git(&["rev-parse", "HEAD"], true));
                assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "other");
            } else {
                assert_ne!(before, f.git(&["rev-parse", "HEAD"], true));
                assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "resolved");
            }
        }
    }
}

#[test]
fn manual_commit_keeps_multicommit_sequence_discoverable() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["checkout", "-b", "task"], true);
    f.commit("task");
    let first = f.git(&["rev-parse", "HEAD"], true);
    fs::write(f.0.join("b"), "second").unwrap();
    f.git(&["add", "b"], true);
    f.git(&["commit", "-m", "second"], true);
    let second = f.git(&["rev-parse", "HEAD"], true);
    f.git(&["checkout", "main"], true);
    f.commit("other");
    f.git(&["cherry-pick", &first, &second], false);
    f.commit("resolved manually");
    assert!(!f.0.join(".git/CHERRY_PICK_HEAD").exists());
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    assert_eq!(
        repo.operation_state(&token).unwrap().operation,
        Operation::CherryPick
    );
    let result = sequencer_operation::execute(
        &repo,
        Operation::CherryPick,
        Action::Continue {
            editor: "true".as_ref(),
            requests: None,
        },
        &token,
        &Default::default(),
    )
    .unwrap();
    assert!(result.command.unwrap().status.success());
    assert!(!result.state.unwrap().in_progress());
    assert_eq!(fs::read_to_string(f.0.join("b")).unwrap(), "second");
}

#[test]
fn empty_cherry_pick_requires_explicit_choice_and_revalidates_index() {
    use plico::sequencer_operation::{EmptyChoice, EmptyCommit};
    for choice in [EmptyChoice::Skip, EmptyChoice::Keep] {
        let f = Fixture::new();
        f.commit("base");
        f.git(&["checkout", "-b", "task"], true);
        f.commit("task");
        let task = f.git(&["rev-parse", "HEAD"], true);
        f.git(&["checkout", "main"], true);
        f.commit("other");
        let before = f.git(&["rev-parse", "HEAD"], true);
        f.git(&["cherry-pick", &task], false);
        f.git(&["checkout", "--ours", "--", "a"], true);
        f.git(&["add", "a"], true);
        let repo = Repository::new(&f.0);
        let token = Cancel::default();
        let empty = EmptyCommit::read(&repo, &token).unwrap().unwrap();
        // A new staged edit after the prompt must never be discarded or
        // smuggled into the supposedly empty commit.
        fs::write(f.0.join("a"), "new edit").unwrap();
        f.git(&["add", "a"], true);
        assert!(
            sequencer_operation::execute(
                &repo,
                Operation::CherryPick,
                Action::Empty {
                    expected: &empty,
                    choice
                },
                &token,
                &Default::default()
            )
            .is_err()
        );
        f.git(
            &[
                "restore",
                "--source=HEAD",
                "--staged",
                "--worktree",
                "--",
                "a",
            ],
            true,
        );
        let result = sequencer_operation::execute(
            &repo,
            Operation::CherryPick,
            Action::Empty {
                expected: &empty,
                choice,
            },
            &token,
            &Default::default(),
        )
        .unwrap();
        let command = result.command.unwrap();
        assert!(command.status.success(), "{:?}", command.stderr);
        assert!(!result.state.unwrap().in_progress());
        assert!(result.empty.is_none());
        let head = f.git(&["rev-parse", "HEAD"], true);
        match choice {
            EmptyChoice::Skip => assert_eq!(head, before),
            EmptyChoice::Keep => {
                assert_ne!(head, before);
                assert_eq!(
                    f.git(&["rev-parse", "HEAD^{tree}"], true),
                    f.git(&["rev-parse", &format!("{before}^{{tree}}")], true)
                );
            }
        }
        assert_eq!(f.git(&["status", "--porcelain"], true), "");
    }
}
