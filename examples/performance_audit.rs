//! Read-only backend diagnostics; run against a disposable repository.
//! JSONL timings include process overhead, not UI latency. No timing assertions.
use plico::{
    git::{Cancel, Repository},
    process_tree::ProcessTree,
};
use std::{
    process::{Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

fn sample<T>(name: &str, iteration: usize, work: impl FnOnce() -> T) -> T {
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let clock = Instant::now();
    let value = work();
    println!(
        "{}",
        serde_json::json!({"name":name,"iteration":iteration,"start":started,"ms":clock.elapsed().as_secs_f64()*1000.0})
    );
    value
}
fn command(repo: &Repository) -> Command {
    let mut c = Command::new("git");
    c.args([
        "--no-pager",
        "--literal-pathspecs",
        "-c",
        "core.fsmonitor=false",
        "-C",
    ])
    .arg(&repo.path)
    .args(["rev-parse", "--verify", "HEAD"])
    .env("GIT_OPTIONAL_LOCKS", "0")
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c
}
fn main() {
    let repo = Repository::new(std::env::args_os().nth(1).expect("disposable repository"));
    let token = Cancel::default();
    if std::env::args().nth(2).as_deref() == Some("--parent-changes") {
        let snapshot = repo.snapshot(None, 50, None, &token).unwrap();
        for count in [1, 20, 50] {
            let commits: Vec<_> = snapshot.commits.iter().take(count).collect();
            assert_eq!(commits.len(), count, "parent benchmark requires 50 commits");
            for iteration in 0..5 {
                let separate = sample(&format!("parents-separate-{count}"), iteration, || {
                    commits
                        .iter()
                        .map(|c| repo.log_commit_changes(c, &token).unwrap())
                        .collect::<Vec<_>>()
                });
                let batch = sample(&format!("parents-batch-{count}"), iteration, || {
                    repo.log_commit_changes_batch(&commits, &token).unwrap()
                });
                assert_eq!(separate, batch);
            }
        }
        return;
    }
    if std::env::args().nth(2).as_deref() == Some("--metadata") {
        let snapshot = repo.snapshot(None, 50, None, &token).unwrap();
        for count in [1, 20, 50] {
            let commits: Vec<_> = snapshot.commits.iter().take(count).collect();
            assert_eq!(
                commits.len(),
                count,
                "metadata benchmark requires 50 commits"
            );
            for iteration in 0..5 {
                let separate = sample(&format!("metadata-separate-{count}"), iteration, || {
                    commits
                        .iter()
                        .map(|c| repo.commit_metadata(c, &token).unwrap())
                        .collect::<Vec<_>>()
                });
                let batch = sample(&format!("metadata-batch-{count}"), iteration, || {
                    repo.commit_metadata_batch(&commits, &token).unwrap()
                });
                for (before, after) in separate.iter().zip(batch) {
                    assert_eq!(
                        (&before.message, &before.author, &before.base),
                        (&after.message, &after.author, &after.base)
                    );
                }
            }
        }
        return;
    }
    if std::env::args().nth(2).as_deref() == Some("--revision-content") {
        use plico::combined_changes::{Change, from_file, read_contents};
        let commit = repo
            .snapshot(None, 1, None, &token)
            .unwrap()
            .commits
            .remove(0);
        let file = repo.details(&commit, &token).unwrap().files.remove(0);
        let pair = from_file(
            &file,
            &commit.id,
            commit.parents.first().map(String::as_str),
        )
        .unwrap();
        assert!(
            pair.before.is_some() && pair.after.is_some(),
            "requires a two-sided file change"
        );
        let before = Change {
            before: pair.before.clone(),
            after: None,
        };
        let after = Change {
            before: None,
            after: pair.after.clone(),
        };
        for i in 0..20 {
            let separate = sample("revision-content-separate", i, || {
                (
                    read_contents(&repo, &before, &token).unwrap().before,
                    read_contents(&repo, &after, &token).unwrap().after,
                )
            });
            let batched = sample("revision-content-batched", i, || {
                read_contents(&repo, &pair, &token).unwrap()
            });
            assert_eq!(separate, (batched.before, batched.after));
        }
        return;
    }
    if std::env::args().nth(2).as_deref() == Some("--configuration") {
        let mut roots = vec![repo.clone()];
        roots.extend(std::env::args_os().skip(3).map(Repository::new));
        let mut events = plico::worktree_events::ProjectEvents::default();
        events.reconcile(&roots, &token).unwrap();
        let paths: Vec<_> = roots.iter().map(|r| r.path.clone()).collect();
        let start = Instant::now();
        for i in 0..10 {
            let now = start + std::time::Duration::from_secs(i as u64 * 31);
            sample("configuration-poll", i, || {
                let mut batches = 0;
                while events.configuration_due(&[], now) {
                    sample("configuration-batch", i, || {
                        events
                            .poll_configuration(&mut Vec::new(), now, &token)
                            .unwrap();
                    });
                    batches += 1;
                    assert!(batches <= roots.len());
                }
            });
            events.acknowledge_configuration(&paths);
        }
        return;
    }
    if std::env::args().nth(2).as_deref() == Some("--operation-inputs") {
        for i in 0..10 {
            for shared in if i % 2 == 0 {
                [true, false]
            } else {
                [false, true]
            } {
                let name = if shared {
                    "operation-shared-inputs"
                } else {
                    "operation-separate-inputs"
                };
                sample(name, i, || {
                    let request = plico::performance_trace::Request::new(name, Some(&repo.path));
                    request.context().scope(|| {
                        if shared {
                            plico::operation_inspection::Inspection::read(&repo, &token)
                                .state
                                .unwrap();
                        } else {
                            repo.operation_state(&token).unwrap();
                            plico::merge_saved::read(&repo.editor_directory(&token).unwrap())
                                .unwrap();
                            plico::revert_operation::Recovery::record_exists(&repo, &token)
                                .unwrap();
                            plico::revert_operation::SavedChanges::record_exists(&repo, &token)
                                .unwrap();
                        }
                    });
                    request.finish("ok");
                });
                let name = if shared {
                    "observer-shared-inputs"
                } else {
                    "observer-separate-inputs"
                };
                sample(name, i, || {
                    let request = plico::performance_trace::Request::new(name, Some(&repo.path));
                    request.context().scope(|| {
                        if shared {
                            plico::operation_state::observe(&repo, None, &token)
                                .unwrap()
                                .unwrap();
                        } else {
                            let before =
                                plico::operation_state::Observation::read(&repo, &token).unwrap();
                            repo.operation_state(&token).unwrap();
                            assert_eq!(
                                before,
                                plico::operation_state::Observation::read(&repo, &token).unwrap()
                            );
                        }
                    });
                    request.finish("ok");
                });
            }
        }
        return;
    }
    if std::env::args().nth(2).as_deref() == Some("--history-inputs") {
        // Controlled comparison of separate versus shared observation in the
        // CURRENT reader, not a claim to benchmark a historical executable.
        // Alternate order to reduce systematic cache/order bias.
        for i in 0..10 {
            for separate in if i % 2 == 0 {
                [true, false]
            } else {
                [false, true]
            } {
                let name = if separate {
                    "history-separate-inputs"
                } else {
                    "history-shared-inputs"
                };
                sample(name, i, || {
                    let request = plico::performance_trace::Request::new(name, Some(&repo.path));
                    request.context().scope(|| {
                        let observation = separate.then(|| {
                            plico::branch_updates::HistoryObservation::read(&repo, &token).unwrap()
                        });
                        let window =
                            plico::history::read(&repo, None, 0, 1000, None, None, &token).unwrap();
                        let observation = observation.or(window.observation).unwrap();
                        observation.references.counts(&repo, &token).unwrap();
                        assert!(!window.snapshot.commits.is_empty());
                    });
                    request.finish("ok");
                });
            }
        }
        return;
    }
    for i in 0..20 {
        // Alternate the control and production process ownership paths.
        sample("plain-total", i, || {
            let mut child = sample("plain-spawn", i, || command(&repo).spawn().unwrap());
            sample("plain-wait", i, || assert!(child.wait().unwrap().success()));
        });
        sample("tree-total", i, || {
            let mut tree = sample("tree-spawn", i, || {
                ProcessTree::spawn(&mut command(&repo)).unwrap()
            });
            sample("tree-wait", i, || {
                assert!(tree.child.wait().unwrap().success())
            });
            sample("tree-drop", i, || drop(tree));
        });
        sample("production-head", i, || {
            plico::commit_operation::head(&repo, &token).unwrap()
        });
    }
    for i in 0..5 {
        sample("operation-observation", i, || {
            plico::operation_state::Observation::read(&repo, &token).unwrap()
        });
        sample("operation-state", i, || {
            repo.operation_state(&token).unwrap()
        });
        sample("reference-observation", i, || {
            plico::branch_updates::HistoryObservation::read(&repo, &token).unwrap()
        });
        sample("changes", i, || {
            plico::commit_changes::Snapshot::load(&repo, &token).unwrap()
        });
        let history = sample("history-1000", i, || {
            plico::history::read(&repo, None, 0, 1000, None, None, &token).unwrap()
        });
        if let Some(commit) = history.snapshot.commits.first() {
            let details = sample("commit-details", i, || {
                repo.details(commit, &token).unwrap()
            });
            if let Some(file) = details.files.first() {
                sample("committed-full-patch", i, || {
                    repo.full_patch(Some(commit), file, &token).unwrap()
                });
            }
        }
        for count in [1, 20, 50] {
            let ids = history
                .snapshot
                .commits
                .iter()
                .take(count)
                .map(|c| c.id.clone())
                .collect::<Vec<_>>();
            sample(&format!("log-selection-{count}"), i, || {
                let selection = plico::log_changes::load(&repo, &ids, &token).unwrap();
                repo.commit_metadata_batch(
                    &selection.commits.iter().take(50).collect::<Vec<_>>(),
                    &token,
                )
                .unwrap();
            });
        }
        sample("push-targets", i, || repo.push_snapshot(&token).unwrap());
        sample("update-preflight", i, || {
            plico::project_update::Prepared::read(&repo, &token).unwrap()
        });
        sample("update-registered-preflight", i, || {
            plico::project_update::Prepared::read_registered(std::slice::from_ref(&repo), &token)
                .unwrap()
        });
    }
}
