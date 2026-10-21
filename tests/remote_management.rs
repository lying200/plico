use plico::{
    git::{Cancel, Repository},
    remote_management::{self as remotes, Edit},
};
use std::{fs, path::PathBuf, process::Command};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let f = Self(std::env::temp_dir().join(format!("plico-remotes-{:x?}", random)));
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
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    fn commit(&self, text: &str, message: &str) -> String {
        fs::write(self.0.join("a"), text).unwrap();
        self.git(&["add", "a"]);
        self.git(&["commit", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
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

fn run(f: &Fixture, edit: Edit) {
    let report =
        remotes::execute(&f.repo(), &edit, &Cancel::default(), &Default::default()).unwrap();
    assert!(report.succeeded());
}
#[test]
fn add_edit_remove_preserve_other_configuration_and_worktree() {
    let f = Fixture::new();
    let head = f.commit("base\n", "base");
    fs::write(f.0.join("a"), "draft\n").unwrap();
    run(
        &f,
        Edit::Add {
            name: "zebra".into(),
            url: "../remote with spaces.git".into(),
        },
    );
    run(
        &f,
        Edit::Add {
            name: "origin".into(),
            url: "../origin.git".into(),
        },
    );
    f.git(&["config", "--add", "remote.origin.pushurl", "../push.git"]);
    f.git(&["config", "branch.main.remote", "origin"]);
    let list = remotes::read(&f.repo(), &Cancel::default()).unwrap();
    assert_eq!(
        list.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["origin", "zebra"]
    );
    run(
        &f,
        Edit::Change {
            previous: list[0].clone(),
            name: "upstream".into(),
            url: "../new.git".into(),
        },
    );
    assert_eq!(f.git(&["config", "remote.upstream.url"]), "../new.git");
    assert_eq!(f.git(&["config", "remote.upstream.pushurl"]), "../push.git");
    assert_eq!(f.git(&["config", "branch.main.remote"]), "upstream");
    let list = remotes::read(&f.repo(), &Cancel::default()).unwrap();
    run(
        &f,
        Edit::Remove {
            previous: list.iter().find(|r| r.name == "upstream").unwrap().clone(),
        },
    );
    assert_eq!(f.git(&["remote"]), "zebra");
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"draft\n");
}
#[test]
fn stale_duplicate_and_foreign_remote_requests_are_rejected() {
    let f = Fixture::new();
    run(
        &f,
        Edit::Add {
            name: "origin".into(),
            url: "../one.git".into(),
        },
    );
    let previous = remotes::read(&f.repo(), &Cancel::default())
        .unwrap()
        .remove(0);
    f.git(&["remote", "set-url", "origin", "../external.git"]);
    assert!(
        remotes::execute(
            &f.repo(),
            &Edit::Remove {
                previous: previous.clone()
            },
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    assert!(
        remotes::execute(
            &f.repo(),
            &Edit::Add {
                name: "origin".into(),
                url: "../two.git".into()
            },
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    let other = Fixture::new();
    run(
        &other,
        Edit::Add {
            name: "origin".into(),
            url: "../one.git".into(),
        },
    );
    assert!(
        remotes::execute(
            &other.repo(),
            &Edit::Remove { previous },
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(f.git(&["config", "remote.origin.url"]), "../external.git");
    assert_eq!(other.git(&["remote"]), "origin");
}
#[test]
fn invalid_input_and_cancel_never_create_a_remote() {
    let f = Fixture::new();
    for (name, url) in [
        ("--upload-pack=bad", "x"),
        ("bad..name", "x"),
        ("origin", "--bad"),
        ("origin", ""),
        ("origin", "x\ny"),
    ] {
        assert!(
            remotes::execute(
                &f.repo(),
                &Edit::Add {
                    name: name.into(),
                    url: url.into()
                },
                &Cancel::default(),
                &Default::default()
            )
            .is_err()
        );
    }
    let token = Cancel::default();
    token.cancel();
    assert!(
        remotes::execute(
            &f.repo(),
            &Edit::Add {
                name: "origin".into(),
                url: "../local.git".into()
            },
            &token,
            &Default::default()
        )
        .is_err()
    );
    assert!(f.git(&["remote"]).is_empty());
}
#[test]
fn multiple_urls_failure_reports_partial_rename_without_losing_urls() {
    let f = Fixture::new();
    run(
        &f,
        Edit::Add {
            name: "origin".into(),
            url: "../one.git".into(),
        },
    );
    f.git(&["config", "--add", "remote.origin.url", "../two.git"]);
    let previous = remotes::read(&f.repo(), &Cancel::default())
        .unwrap()
        .remove(0);
    let report = remotes::execute(
        &f.repo(),
        &Edit::Change {
            previous: previous.clone(),
            name: "origin".into(),
            url: "../one.git".into(),
        },
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(report.commands.is_empty());
    let report = remotes::execute(
        &f.repo(),
        &Edit::Change {
            previous,
            name: "renamed".into(),
            url: "../replacement.git".into(),
        },
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!report.succeeded());
    assert_eq!(report.commands.len(), 2);
    assert!(report.commands[0].status.success());
    assert!(!report.commands[1].status.success());
    let list = remotes::read(&f.repo(), &Cancel::default()).unwrap();
    assert_eq!(list[0].name, "renamed");
    assert_eq!(list[0].urls, ["../one.git", "../two.git"]);
}

#[test]
fn access_check_reads_local_remote_without_modifying_configuration() {
    let f = Fixture::new();
    let source = Fixture::new();
    source.commit("remote\n", "remote commit");
    let token = Cancel::default();
    let requests = plico::auth_request::Requests::new(token.clone());
    let output = remotes::check_access(
        &f.repo(),
        source.0.to_str().unwrap(),
        &requests,
        std::path::Path::new(env!("CARGO_BIN_EXE_plico-askpass")),
        &token,
        &Default::default(),
    )
    .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout.bytes()).contains("refs/heads/main"));
    assert!(f.git(&["remote"]).is_empty());
    let missing = source.0.join("missing.git");
    let failure = remotes::check_access(
        &f.repo(),
        missing.to_str().unwrap(),
        &requests,
        std::path::Path::new(env!("CARGO_BIN_EXE_plico-askpass")),
        &token,
        &Default::default(),
    )
    .unwrap();
    assert!(!failure.status.success());
    assert!(f.git(&["remote"]).is_empty());
}

#[test]
fn dotted_remote_names_do_not_pollute_other_snapshots() {
    let f = Fixture::new();
    run(
        &f,
        Edit::Add {
            name: "team".into(),
            url: "../team.git".into(),
        },
    );
    run(
        &f,
        Edit::Add {
            name: "team.member".into(),
            url: "../member.git".into(),
        },
    );
    let previous = remotes::read(&f.repo(), &Cancel::default())
        .unwrap()
        .into_iter()
        .find(|r| r.name == "team")
        .unwrap();
    f.git(&["remote", "set-url", "team.member", "../updated.git"]);
    run(
        &f,
        Edit::Change {
            previous,
            name: "team".into(),
            url: "../updated-team.git".into(),
        },
    );
    assert_eq!(
        f.git(&["config", "remote.team.member.url"]),
        "../updated.git"
    );
}

#[test]
fn form_preflight_is_read_only_and_does_not_replace_execution_validation() {
    let f = Fixture::new();
    let before = fs::read(f.0.join(".git/config")).unwrap();
    let edit = Edit::Add {
        name: "origin".into(),
        url: "../source.git".into(),
    };
    remotes::validate_edit(&f.repo(), &edit, &Cancel::default()).unwrap();
    assert_eq!(fs::read(f.0.join(".git/config")).unwrap(), before);
    // A different tool can add the same name after the form checked it.
    f.git(&["remote", "add", "origin", "../other.git"]);
    assert!(remotes::execute(&f.repo(), &edit, &Cancel::default(), &Default::default()).is_err());
    assert_eq!(f.git(&["remote", "get-url", "origin"]), "../other.git");
}

#[test]
fn table_url_hides_credentials_and_tokens() {
    assert_eq!(
        remotes::display_url(
            "https://user:password@example.invalid/repo.git?token=secret#fragment"
        ),
        "https://example.invalid/repo.git"
    );
    assert_eq!(
        remotes::display_url("git@example.invalid:team/repo.git"),
        "git@example.invalid:team/repo.git"
    );
    assert_eq!(
        remotes::display_url("C:/repositories/repo.git"),
        "C:/repositories/repo.git"
    );
}
