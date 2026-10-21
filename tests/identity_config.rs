use plico::{
    git::{Cancel, Repository},
    identity_config::{self, Identity, Scope},
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

#[test]
fn local_identity_preserves_index_worktree_and_head() {
    let f = Fixture::new();
    let head = f.commit("base\n", "base");
    fs::write(f.0.join("a"), "draft\n").unwrap();
    let identity = Identity {
        name: "Plico Test User".into(),
        email: "new@example.invalid".into(),
    };
    let report = identity_config::set(
        &f.repo(),
        &identity,
        Scope::Repository,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(report.succeeded());
    assert!(report.effective.unwrap() == identity);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(f.git(&["show", ":a"]), "base");
    assert_eq!(fs::read(f.0.join("a")).unwrap(), b"draft\n");
}
#[test]
fn invalid_identity_and_cancel_leave_configuration_unchanged() {
    let f = Fixture::new();
    let before = Identity::read(&f.repo(), &Cancel::default()).unwrap();
    for (name, email) in [
        ("", "a@b"),
        ("Name", ""),
        ("Name", "missing-at"),
        ("Name\nInjected", "a@b"),
    ] {
        assert!(
            identity_config::set(
                &f.repo(),
                &Identity {
                    name: name.into(),
                    email: email.into()
                },
                Scope::Repository,
                &Cancel::default(),
                &Default::default()
            )
            .is_err()
        );
    }
    let token = Cancel::default();
    token.cancel();
    assert!(
        identity_config::set(
            &f.repo(),
            &Identity {
                name: "Other".into(),
                email: "other@example.invalid".into()
            },
            Scope::Repository,
            &token,
            &Default::default()
        )
        .is_err()
    );
    assert!(Identity::read(&f.repo(), &Cancel::default()).unwrap() == before);
}
#[test]
fn empty_email_requires_guidance_even_with_a_name() {
    let f = Fixture::new();
    f.git(&["config", "user.email", ""]);
    assert!(
        Identity::read(&f.repo(), &Cancel::default())
            .unwrap()
            .missing()
    );
    f.git(&["config", "user.email", "user@example.invalid"]);
    assert!(
        !Identity::read(&f.repo(), &Cancel::default())
            .unwrap()
            .missing()
    );
}

#[test]
fn global_configuration_is_tested_in_an_isolated_child() {
    let f = Fixture::new();
    let global = f.0.join("isolated-global-config");
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "global_configuration_child", "--nocapture"])
        .env("PLICO_IDENTITY_GLOBAL_TEST", &global)
        .env("GIT_CONFIG_GLOBAL", &global)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let text = fs::read_to_string(global).unwrap();
    assert!(text.contains("Global Test"));
    assert!(text.contains("global@example.invalid"));
}

#[test]
fn global_configuration_child() {
    let Some(path) = std::env::var_os("PLICO_IDENTITY_GLOBAL_TEST") else {
        return;
    };
    assert_eq!(std::env::var_os("GIT_CONFIG_GLOBAL"), Some(path));
    let f = Fixture::new();
    f.git(&["config", "--local", "--unset-all", "user.name"]);
    f.git(&["config", "--local", "--unset-all", "user.email"]);
    let identity = Identity {
        name: "Global Test".into(),
        email: "global@example.invalid".into(),
    };
    let report = identity_config::set(
        &f.repo(),
        &identity,
        Scope::Global,
        &Cancel::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(report.succeeded());
    assert!(report.effective.unwrap() == identity);
    assert!(
        !fs::read_to_string(f.0.join(".git/config"))
            .unwrap()
            .contains("Global Test")
    );
}
