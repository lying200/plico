use plico::{
    git::{Cancel, Repository},
    submodules::{self, IndexState},
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
            "plico-submodules-{}-{}",
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
        fs::write(f.0.join("base"), "base").unwrap();
        f.git(&["add", "base"]);
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
    fn declare(&self, name: &str, path: &str) {
        self.git(&[
            "config",
            "-f",
            ".gitmodules",
            &format!("submodule.{name}.path"),
            path,
        ]);
        self.git(&[
            "config",
            "-f",
            ".gitmodules",
            &format!("submodule.{name}.url"),
            "../child.git",
        ]);
    }
    fn list(&self) -> Vec<submodules::Module> {
        submodules::list(&Repository::new(&self.0), &Cancel::default()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn inventory_preserves_paths_and_index_without_initializing_modules() {
    let f = Fixture::new();
    assert!(f.list().is_empty());
    let path = "modules/with space";
    f.declare("named.child", path);
    f.declare("not-staged", "another");
    let oid = f.git(&["rev-parse", "HEAD"]);
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{oid},{path}"),
    ]);
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    let modules = f.list();
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0].name, "named.child");
    assert_eq!(modules[0].path, PathBuf::from(path));
    assert_eq!(modules[0].index, IndexState::Commit(oid));
    assert_eq!(modules[1].index, IndexState::NotTracked);
    assert!(!f.0.join(path).exists());
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    // Calling from a subdirectory still finds the repository-level declarations.
    fs::create_dir(f.0.join("inside")).unwrap();
    let nested =
        submodules::list(&Repository::new(f.0.join("inside")), &Cancel::default()).unwrap();
    assert_eq!(nested.len(), 2);
    assert_eq!(nested[0].index, modules[0].index);
}
#[test]
fn invalid_declarations_are_not_operation_targets() {
    let f = Fixture::new();
    f.git(&[
        "config",
        "-f",
        ".gitmodules",
        "submodule.partial.path",
        "only-path",
    ]);
    assert!(f.list().is_empty());
    f.declare("outside", "../outside");
    assert!(submodules::list(&Repository::new(&f.0), &Cancel::default()).is_err());
    f.git(&[
        "config",
        "-f",
        ".gitmodules",
        "--remove-section",
        "submodule.outside",
    ]);
    f.declare("a", "same");
    f.declare("b", "same");
    assert!(submodules::list(&Repository::new(&f.0), &Cancel::default()).is_err());
}
#[cfg(unix)]
#[test]
fn inventory_does_not_split_tab_or_newline_in_path() {
    let f = Fixture::new();
    let path = "modules/tab\tand\nnewline";
    f.declare("special", path);
    let oid = f.git(&["rev-parse", "HEAD"]);
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{oid},{path}"),
    ]);
    let modules = f.list();
    assert_eq!(modules[0].path, PathBuf::from(path));
    assert_eq!(modules[0].index, IndexState::Commit(oid));
}

#[test]
fn inventory_reports_unmerged_gitlinks_without_choosing_a_stage() {
    use std::{io::Write, process::Stdio};
    let f = Fixture::new();
    f.declare("child", "modules/child");
    let oid = f.git(&["rev-parse", "HEAD"]);
    let mut child = Command::new("git")
        .arg("-C")
        .arg(&f.0)
        .args(["update-index", "--index-info"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            format!("160000 {oid} 1\tmodules/child\n160000 {oid} 2\tmodules/child\n").as_bytes(),
        )
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(f.list()[0].index, IndexState::Conflict);
}

#[test]
fn probe_distinguishes_missing_occupied_and_broken_children_from_parent() {
    use submodules::Checkout;
    let f = Fixture::new();
    let path = std::path::Path::new("child");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    assert_eq!(
        submodules::inspect(&repo, path, &token).unwrap(),
        Checkout::Uninitialized { occupied: false }
    );
    fs::create_dir(f.0.join(path)).unwrap();
    fs::write(f.0.join(path).join("local"), "keep").unwrap();
    assert_eq!(
        submodules::inspect(&repo, path, &token).unwrap(),
        Checkout::Uninitialized { occupied: true }
    );
    fs::write(f.0.join(path).join(".git"), "gitdir: nonexistent\n").unwrap();
    assert!(submodules::inspect(&repo, path, &token).is_err());
    assert_eq!(fs::read(f.0.join(path).join("local")).unwrap(), b"keep");
    token.cancel();
    assert!(submodules::inspect(&repo, path, &token).is_err());
}

#[test]
fn probe_reads_real_submodule_branch_detached_and_local_changes() {
    use submodules::Checkout;
    let parent = Fixture::new();
    let source = Fixture::new();
    parent.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        source.0.to_str().unwrap(),
        "modules/child",
    ]);
    let path = std::path::Path::new("modules/child");
    let child = parent.0.join(path);
    let repo = Repository::new(&parent.0);
    let token = Cancel::default();
    let oid = source.git(&["rev-parse", "HEAD"]);
    assert_eq!(
        submodules::inspect(&repo, path, &token).unwrap(),
        Checkout::Initialized {
            head: oid.clone(),
            branch: Some("refs/heads/main".into()),
            tracked_changes: false,
            untracked: false,
            conflicts: false
        }
    );
    let status = Command::new("git")
        .arg("-C")
        .arg(&child)
        .args(["checkout", "--detach"])
        .output()
        .unwrap();
    assert!(status.status.success());
    fs::write(child.join("base"), "dirty").unwrap();
    fs::write(child.join("untracked"), "keep").unwrap();
    let index_path = parent.0.join(".git/modules/modules/child/index");
    let index = fs::read(&index_path).unwrap();
    assert_eq!(
        submodules::inspect(&repo, path, &token).unwrap(),
        Checkout::Initialized {
            head: oid,
            branch: None,
            tracked_changes: true,
            untracked: true,
            conflicts: false
        }
    );
    assert_eq!(fs::read(index_path).unwrap(), index);
    assert_eq!(fs::read(child.join("base")).unwrap(), b"dirty");
}

#[cfg(unix)]
#[test]
fn probe_rejects_linked_directory_ancestors() {
    let f = Fixture::new();
    let other = Fixture::new();
    std::os::unix::fs::symlink(&other.0, f.0.join("linked")).unwrap();
    assert!(
        submodules::inspect(
            &Repository::new(&f.0),
            std::path::Path::new("linked"),
            &Cancel::default()
        )
        .is_err()
    );
}

#[test]
fn probe_reports_unmerged_files_inside_child() {
    use std::{io::Write, process::Stdio};
    let f = Fixture::new();
    let source = Fixture::new();
    f.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        source.0.to_str().unwrap(),
        "child",
    ]);
    let blob = source.git(&["rev-parse", "HEAD:base"]);
    let mut process = Command::new("git")
        .arg("-C")
        .arg(f.0.join("child"))
        .args(["update-index", "--index-info"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(
            format!(
                "0 {}\tbase\n100644 {blob} 1\tbase\n100644 {blob} 2\tbase\n",
                "0".repeat(40)
            )
            .as_bytes(),
        )
        .unwrap();
    assert!(process.wait().unwrap().success());
    assert!(matches!(
        submodules::inspect(
            &Repository::new(&f.0),
            std::path::Path::new("child"),
            &Cancel::default()
        )
        .unwrap(),
        submodules::Checkout::Initialized {
            conflicts: true,
            tracked_changes: true,
            ..
        }
    ));
}

fn child_git(parent: &Fixture, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(parent.0.join("child"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn update_fixture() -> (Fixture, Fixture, String, String) {
    let parent = Fixture::new();
    let source = Fixture::new();
    fs::write(source.0.join("stable"), "stable\n").unwrap();
    source.git(&["add", "stable"]);
    source.git(&["commit", "-m", "stable file"]);
    let before = source.git(&["rev-parse", "HEAD"]);
    fs::write(source.0.join("base"), "second\n").unwrap();
    source.git(&["commit", "-am", "second"]);
    let after = source.git(&["rev-parse", "HEAD"]);
    parent.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        source.0.to_str().unwrap(),
        "child",
    ]);
    child_git(&parent, &["checkout", "--detach", &before]);
    (parent, source, before, after)
}

#[test]
fn inventory_handles_more_paths_than_one_windows_command_line() {
    use std::io::Write;
    let parent = Fixture::new();
    let oid = parent.git(&["rev-parse", "HEAD"]);
    let mut config = String::new();
    let paths: Vec<_> = (0..400)
        .map(|i| format!("modules/{i:04}-{}", "long component ".repeat(6).trim_end()))
        .collect();
    assert!(paths.iter().map(|p| p.len() + 3).sum::<usize>() > 32_767);
    for (i, path) in paths.iter().enumerate() {
        config.push_str(&format!(
            "[submodule \"module-{i:04}\"]\npath = \"{path}\"\nurl = ../child.git\n"
        ));
    }
    fs::write(parent.0.join(".gitmodules"), config).unwrap();
    let mut command = Command::new("git")
        .arg("-C")
        .arg(&parent.0)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = command.stdin.take().unwrap();
    for path in &paths {
        writeln!(input, "160000 {oid}\t{path}").unwrap();
    }
    drop(input);
    assert!(command.wait().unwrap().success());
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    let modules = parent.list();
    assert_eq!(modules.len(), paths.len());
    for (module, path) in modules.iter().zip(paths) {
        assert_eq!(module.path, PathBuf::from(path));
        assert_eq!(module.index, IndexState::Commit(oid.clone()));
    }
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);
}

#[test]
fn project_inventory_preserves_independent_roots_and_shared_git_worktrees() {
    use plico::project_repositories::Repositories;
    let first = Fixture::new();
    let second = Fixture::new();
    let linked = first.0.join("linked");
    first.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
    let a = Repository::new(&first.0);
    let b = Repository::new(&second.0);
    let w = Repository::new(&linked);
    let token = Cancel::default();
    assert_eq!(
        first.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
        first.git(&[
            "-C",
            linked.to_str().unwrap(),
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir"
        ])
    );
    let inventory = Repositories::from_registered(&[a.clone(), w, b, a], &token).unwrap();
    assert!(inventory.errors.is_empty(), "{:?}", inventory.errors);
    assert_eq!(inventory.roots.len(), 3);
    assert!(inventory.submodules.is_empty());
    let roots = inventory
        .roots
        .iter()
        .map(|r| r.path.canonicalize().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(roots.contains(&linked.canonicalize().unwrap()));
    assert!(roots.contains(&first.0.canonicalize().unwrap()));
    assert!(roots.contains(&second.0.canonicalize().unwrap()));
    // An unrelated nested repository is not implicitly part of a project.
    assert_eq!(
        Repositories::discover(&Repository::new(&first.0), &token)
            .unwrap()
            .roots
            .len(),
        1
    );
    let missing = Repository::new(first.0.join("absent"));
    let inventory =
        Repositories::from_registered(&[missing, Repository::new(&second.0)], &token).unwrap();
    assert_eq!(inventory.roots.len(), 1);
    assert_eq!(inventory.errors.len(), 1);
    fs::create_dir(first.0.join("plain-directory")).unwrap();
    let inventory =
        Repositories::from_registered(&[Repository::new(first.0.join("plain-directory"))], &token)
            .unwrap();
    assert!(inventory.roots.is_empty());
    assert_eq!(inventory.errors.len(), 1);
    token.cancel();
    assert!(Repositories::from_registered(&[], &token).is_err());
}

#[test]
fn project_mappings_survive_gitlink_removal_but_revalidate_repository_roots() {
    use plico::project_repositories::Mappings;
    let (parent, _source, _, _) = update_fixture();
    parent.git(&["add", "child", ".gitmodules"]);
    parent.git(&["commit", "-m", "register child"]);
    let token = Cancel::default();
    let mut mappings = Mappings::new(vec![Repository::new(&parent.0)]);
    assert_eq!(mappings.refresh(&token).unwrap().roots.len(), 2);
    let child = parent.0.join("child");
    let child_head = child_git(&parent, &["rev-parse", "HEAD"]);
    fs::write(child.join("base"), "keep my draft\n").unwrap();
    parent.git(&["rm", "--cached", "child"]);
    fs::write(parent.0.join(".gitmodules"), "").unwrap();
    let index = fs::read(parent.0.join(".git/index")).unwrap();
    let inventory = mappings.refresh(&token).unwrap();
    assert_eq!(inventory.roots.len(), 2);
    assert!(inventory.submodules.is_empty());
    assert!(inventory.roots.iter().any(|root| root.path == child));
    assert_eq!(child_git(&parent, &["rev-parse", "HEAD"]), child_head);
    assert_eq!(fs::read(child.join("base")).unwrap(), b"keep my draft\n");
    assert_eq!(fs::read(parent.0.join(".git/index")).unwrap(), index);

    // A child whose .git is gone must not silently become its parent repository.
    fs::remove_file(child.join(".git")).unwrap();
    let inventory = mappings.refresh(&token).unwrap();
    assert_eq!(inventory.roots.len(), 1);
    assert!(
        inventory.errors.iter().any(|(path, _)| {
            plico::recent_projects::identity(path) == plico::recent_projects::identity(&child)
        }),
        "expected error for {child:?}, got {:?}",
        inventory.errors
    );
    assert_eq!(mappings.registered().len(), 2);
}

#[test]
fn cancelled_project_scan_does_not_register_partial_discovery() {
    use plico::project_repositories::Mappings;
    let (parent, _source, _, _) = update_fixture();
    let mut mappings = Mappings::new(vec![Repository::new(&parent.0)]);
    let token = Cancel::default();
    token.cancel();
    assert!(mappings.refresh(&token).is_err());
    assert_eq!(mappings.registered().len(), 1);
    assert_eq!(mappings.refresh(&Cancel::default()).unwrap().roots.len(), 2);
    assert_eq!(mappings.registered().len(), 2);
}

#[test]
fn persisted_project_mappings_recover_removed_submodule_after_restart() {
    use plico::project_repositories::{Mappings, Store};
    let (parent, _source, _, _) = update_fixture();
    parent.git(&["add", "child", ".gitmodules"]);
    parent.git(&["commit", "-m", "register child"]);
    let settings = Fixture::new();
    let store = Store::new(settings.0.join("project-roots.json"), &parent.0);
    let token = Cancel::default();
    let mut first = Mappings::new(vec![Repository::new(&parent.0)]);
    assert_eq!(
        first.refresh_persisted(&store, &token).unwrap().roots.len(),
        2
    );
    drop(first);
    parent.git(&["rm", "--cached", "child"]);
    fs::write(parent.0.join(".gitmodules"), "").unwrap();
    fs::write(parent.0.join("child/base"), "keep recovery draft\n").unwrap();
    let mut reopened = Mappings::new(vec![Repository::new(&parent.0)]);
    let inventory = reopened.refresh_persisted(&store, &token).unwrap();
    assert!(inventory.errors.is_empty(), "{:?}", inventory.errors);
    assert_eq!(inventory.roots.len(), 2);
    assert!(inventory.submodules.is_empty());
    assert_eq!(
        fs::read(parent.0.join("child/base")).unwrap(),
        b"keep recovery draft\n"
    );
}

#[test]
fn mapping_store_preserves_other_windows_and_reports_corrupt_records() {
    use plico::project_repositories::{Mappings, Store};
    let parent = Fixture::new();
    let other = Fixture::new();
    let settings = Fixture::new();
    let path = settings.0.join("project-roots.json");
    let store = Store::new(path.clone(), &parent.0);
    let token = Cancel::default();
    let mut window_a = Mappings::new(vec![Repository::new(&parent.0)]);
    let mut window_b = Mappings::new(vec![Repository::new(&parent.0), Repository::new(&other.0)]);
    window_b.refresh_persisted(&store, &token).unwrap();
    assert_eq!(
        window_a
            .refresh_persisted(&store, &token)
            .unwrap()
            .roots
            .len(),
        2
    );
    let bytes = fs::read(&path).unwrap();
    let cancelled = Cancel::default();
    cancelled.cancel();
    assert!(window_a.refresh_persisted(&store, &cancelled).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let wrong_project = Store::new(path.clone(), &other.0);
    assert!(
        !window_a
            .refresh_persisted(&wrong_project, &token)
            .unwrap()
            .errors
            .is_empty()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    fs::write(&path, b"broken").unwrap();
    let inventory = window_a.refresh_persisted(&store, &token).unwrap();
    assert_eq!(inventory.roots.len(), 2);
    assert!(
        inventory
            .errors
            .iter()
            .any(|(_, error)| error.contains("mappings"))
    );
    assert_eq!(fs::read(&path).unwrap(), b"broken");
}

#[test]
fn simultaneous_mapping_writers_merge_instead_of_losing_roots() {
    use plico::project_repositories::{Mappings, Store};
    let parent = Fixture::new();
    let settings = Fixture::new();
    let roots = (0..4).map(|_| Fixture::new()).collect::<Vec<_>>();
    let store = Store::new(settings.0.join("roots.json"), &parent.0);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(roots.len()));
    let threads = roots
        .iter()
        .map(|root| {
            let store = store.clone();
            let barrier = barrier.clone();
            let mut mappings =
                Mappings::new(vec![Repository::new(&parent.0), Repository::new(&root.0)]);
            std::thread::spawn(move || {
                barrier.wait();
                let result = mappings
                    .refresh_persisted(&store, &Cancel::default())
                    .unwrap();
                assert!(result.errors.is_empty(), "{:?}", result.errors);
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    let mut reopened = Mappings::new(vec![Repository::new(&parent.0)]);
    let result = reopened
        .refresh_persisted(&store, &Cancel::default())
        .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.roots.len(), 5);
}
