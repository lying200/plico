use plico::{
    git::{Cancel, Repository},
    tag_create,
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
            "plico-tag-create-{}-{}",
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
fn creates_at_selected_commit_and_preserves_dirty_state() {
    let f = Fixture::new();
    f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"], true);
    f.commit("later");
    let head = f.git(&["rev-parse", "HEAD"], true);
    fs::write(f.0.join("a"), "staged").unwrap();
    f.git(&["add", "a"], true);
    fs::write(f.0.join("a"), "unstaged").unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let p = tag_create::Prepared::read(&repo, "release/v1", &base, &token).unwrap();
    assert!(
        tag_create::execute(&repo, &p, &token, &Default::default())
            .unwrap()
            .status
            .success()
    );
    assert_eq!(f.git(&["rev-parse", "refs/tags/release/v1"], true), base);
    assert_eq!(
        f.git(&["cat-file", "-t", "refs/tags/release/v1"], true),
        "commit"
    );
    assert_eq!(f.git(&["rev-parse", "HEAD"], true), head);
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read_to_string(f.0.join("a")).unwrap(), "unstaged");
}
#[test]
fn duplicate_race_invalid_name_and_cancel_do_not_replace_tags() {
    let f = Fixture::new();
    f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"], true);
    f.commit("later");
    let head = f.git(&["rev-parse", "HEAD"], true);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let p = tag_create::Prepared::read(&repo, "v1", &base, &token).unwrap();
    f.git(&["tag", "v1", &head], true);
    assert!(tag_create::execute(&repo, &p, &token, &Default::default()).is_err());
    assert_eq!(f.git(&["rev-parse", "v1"], true), head);
    for name in ["", "-f", "a b", "bad..name", "a.lock", "a\0b"] {
        assert!(tag_create::Prepared::read(&repo, name, &base, &token).is_err());
    }
    let p = tag_create::Prepared::read(&repo, "cancelled", &base, &token).unwrap();
    token.cancel();
    assert!(tag_create::execute(&repo, &p, &token, &Default::default()).is_err());
    assert_eq!(f.git(&["tag", "--list"], true), "v1");
}

#[test]
fn annotated_and_force_options_preserve_target_and_revalidate() {
    let f = Fixture::new();
    f.commit("base");
    let base = f.git(&["rev-parse", "HEAD"], true);
    f.commit("later");
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let p = tag_create::Prepared::read_options(
        &repo,
        "v1",
        " HEAD~1 ",
        "发布说明\n\nMore details",
        false,
        &token,
    )
    .unwrap();
    let out = tag_create::execute(&repo, &p, &token, &Default::default()).unwrap();
    assert!(out.status.success());
    assert!(out.input_error.is_none());
    assert_eq!(f.git(&["cat-file", "-t", "v1"], true), "tag");
    assert_eq!(f.git(&["rev-parse", "v1^{}"], true), base);
    assert_eq!(
        f.git(
            &["for-each-ref", "--format=%(contents)", "refs/tags/v1"],
            true
        ),
        "发布说明\n\nMore details"
    );
    assert!(tag_create::Prepared::read_options(&repo, "v1", "", "", false, &token).is_err());
    let p = tag_create::Prepared::read_options(&repo, "v1", "", "  \n", true, &token).unwrap();
    assert!(
        tag_create::execute(&repo, &p, &token, &Default::default())
            .unwrap()
            .status
            .success()
    );
    assert_eq!(f.git(&["cat-file", "-t", "v1"], true), "commit");
    assert_eq!(
        f.git(&["rev-parse", "v1"], true),
        f.git(&["rev-parse", "HEAD"], true)
    );
    let p = tag_create::Prepared::read_options(&repo, "v1", &base, "replacement", true, &token)
        .unwrap();
    f.git(&["tag", "-f", "v1", &base], true);
    assert!(tag_create::execute(&repo, &p, &token, &Default::default()).is_err());
    let p = tag_create::Prepared::read_options(&repo, "head-tag", "", "", false, &token).unwrap();
    f.commit("moved");
    assert!(tag_create::execute(&repo, &p, &token, &Default::default()).is_err());
    assert!(tag_create::resolve(&repo, "--help", &token).is_err());
    assert!(tag_create::resolve(&repo, "missing", &token).is_err());
}

#[test]
fn validation_preview_uses_resolved_commit_and_historical_paths() {
    let f = Fixture::new();
    f.commit("base");
    f.git(&["mv", "a", "renamed"], true);
    f.git(&["commit", "-m", "rename message"], true);
    let repo = Repository::new(&f.0);
    let token = Cancel::default();
    let preview = tag_create::preview(&repo, "HEAD", &token).unwrap();
    assert_eq!(preview.id, f.git(&["rev-parse", "HEAD"], true));
    assert_eq!(preview.message.trim(), "rename message");
    assert_eq!(preview.files.len(), 1);
    assert_eq!(
        preview.files[0].before.as_ref().unwrap().path,
        PathBuf::from("a")
    );
    assert_eq!(
        preview.files[0].after.as_ref().unwrap().path,
        PathBuf::from("renamed")
    );
    fs::remove_file(f.0.join("renamed")).unwrap();
    let contents =
        plico::combined_changes::read_contents(&repo, &preview.files[0], &token).unwrap();
    assert_eq!(contents.before.as_deref(), Some("base"));
    assert_eq!(contents.after.as_deref(), Some("base"));
    let root = tag_create::preview(&repo, "HEAD~1", &token).unwrap();
    assert!(root.files[0].before.is_none());
    token.cancel();
    assert!(tag_create::preview(&repo, "HEAD", &token).is_err());
}
