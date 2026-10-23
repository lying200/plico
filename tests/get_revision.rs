use plico::{
    file_history::Revision,
    get_revision::Prepared,
    git::{Cancel, Repository},
};
use std::{fs, path::PathBuf, process::Command};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let f =
            Self(std::env::temp_dir().join(format!("plico-get-{}-{nonce}", std::process::id())));
        fs::create_dir(&f.0).unwrap();
        f.git(&["init", "-q"]);
        f.git(&["config", "user.name", "Get Test"]);
        f.git(&["config", "user.email", "get@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.autocrlf", "false"]);
        f
    }
    fn git(&self, args: &[&str]) -> String {
        let o = Command::new("git")
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().into()
    }
    fn commit(&self, content: &[u8]) -> Revision {
        fs::write(self.0.join("file.txt"), content).unwrap();
        self.git(&["add", "file.txt"]);
        self.git(&["commit", "-qm", "revision"]);
        Revision {
            id: self.git(&["rev-parse", "HEAD"]),
            parents: vec![],
            path: "file.txt".into(),
            timestamp: 0,
            author: String::new(),
            message: String::new(),
            deleted: false,
        }
    }
    fn repo(&self) -> Repository {
        Repository::new(self.0.clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn restores_bytes_preserving_head_and_partial_index_and_other_files() {
    let f = Fixture::new();
    let old = f.commit(b"old\r\n\0binary\xff");
    let current = f.commit(b"current\n");
    fs::write(f.0.join("file.txt"), "staged\n").unwrap();
    f.git(&["add", "file.txt"]);
    fs::write(f.0.join("file.txt"), "working\n").unwrap();
    fs::write(f.0.join("unrelated.txt"), "keep").unwrap();
    let index = fs::read(f.0.join(".git/index")).unwrap();
    let token = Cancel::default();
    let p = Prepared::read(&f.repo(), &old.path, &old, &token).unwrap();
    assert!(p.modified);
    // Preparing/closing the confirmation does not write anything.
    assert_eq!(fs::read(f.0.join("file.txt")).unwrap(), b"working\n");
    p.execute(&f.repo(), &token).unwrap();
    assert_eq!(
        fs::read(f.0.join("file.txt")).unwrap(),
        b"old\r\n\0binary\xff"
    );
    assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), current.id);
    assert_eq!(
        fs::read_to_string(f.0.join("unrelated.txt")).unwrap(),
        "keep"
    );
}
#[test]
fn old_path_content_is_written_to_current_path_and_missing_file_is_created() {
    let f = Fixture::new();
    let old = f.commit(b"old");
    f.git(&["mv", "file.txt", "renamed.txt"]);
    f.git(&["commit", "-qm", "rename"]);
    let p = Prepared::read(&f.repo(), "renamed.txt".as_ref(), &old, &Cancel::default()).unwrap();
    assert!(!p.modified);
    p.execute(&f.repo(), &Cancel::default()).unwrap();
    assert!(!f.0.join("file.txt").exists());
    fs::remove_file(f.0.join("renamed.txt")).unwrap();
    let p = Prepared::read(&f.repo(), "renamed.txt".as_ref(), &old, &Cancel::default()).unwrap();
    assert!(!p.modified);
    p.execute(&f.repo(), &Cancel::default()).unwrap();
    assert_eq!(fs::read(f.0.join("renamed.txt")).unwrap(), b"old");
    let p = Prepared::read(
        &f.repo(),
        "nested/new/file.txt".as_ref(),
        &old,
        &Cancel::default(),
    )
    .unwrap();
    p.execute(&f.repo(), &Cancel::default()).unwrap();
    assert_eq!(fs::read(f.0.join("nested/new/file.txt")).unwrap(), b"old");
}
#[test]
fn stale_disk_index_head_and_cancellation_do_not_overwrite() {
    let f = Fixture::new();
    let old = f.commit(b"old");
    f.commit(b"current");
    for mutation in ["disk", "index", "head", "cancel"] {
        let token = Cancel::default();
        let p = Prepared::read(&f.repo(), &old.path, &old, &token).unwrap();
        match mutation {
            "disk" => fs::write(f.0.join("file.txt"), "external").unwrap(),
            "index" => {
                f.git(&["add", "file.txt"]);
            }
            "head" => {
                f.git(&["commit", "--allow-empty", "-qm", "external commit"]);
            }
            _ => token.cancel(),
        }
        let disk = fs::read(f.0.join("file.txt")).unwrap();
        let index = fs::read(f.0.join(".git/index")).unwrap();
        assert!(p.execute(&f.repo(), &token).is_err(), "{mutation}");
        assert_eq!(fs::read(f.0.join("file.txt")).unwrap(), disk);
        assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
    }
}
#[test]
fn absent_destination_race_and_deleted_revision_are_rejected() {
    let f = Fixture::new();
    let mut old = f.commit(b"old");
    fs::remove_file(f.0.join("file.txt")).unwrap();
    let p = Prepared::read(&f.repo(), &old.path, &old, &Cancel::default()).unwrap();
    fs::write(f.0.join("file.txt"), "new external").unwrap();
    assert!(p.execute(&f.repo(), &Cancel::default()).is_err());
    old.deleted = true;
    assert!(Prepared::read(&f.repo(), &old.path, &old, &Cancel::default()).is_err());
    old.deleted = false;
    for path in ["../escape", ".git/config", ""] {
        assert!(Prepared::read(&f.repo(), path.as_ref(), &old, &Cancel::default()).is_err());
    }
}

#[test]
fn ignored_existing_destination_still_requires_confirmation() {
    let f = Fixture::new();
    let revision = f.commit(b"version");
    fs::write(f.0.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(f.0.join("ignored.txt"), "local data").unwrap();
    let p = Prepared::read(
        &f.repo(),
        "ignored.txt".as_ref(),
        &revision,
        &Cancel::default(),
    )
    .unwrap();
    assert!(p.modified);
    assert_eq!(fs::read(f.0.join("ignored.txt")).unwrap(), b"local data");
}

#[cfg(unix)]
#[test]
fn linked_parent_cannot_redirect_a_missing_file_write() {
    let f = Fixture::new();
    let revision = f.commit(b"version");
    fs::create_dir(f.0.join("outside")).unwrap();
    std::os::unix::fs::symlink(f.0.join("outside"), f.0.join("linked")).unwrap();
    assert!(
        Prepared::read(
            &f.repo(),
            "linked/new.txt".as_ref(),
            &revision,
            &Cancel::default()
        )
        .is_err()
    );
    let p = Prepared::read(
        &f.repo(),
        "missing/new.txt".as_ref(),
        &revision,
        &Cancel::default(),
    )
    .unwrap();
    std::os::unix::fs::symlink(f.0.join("outside"), f.0.join("missing")).unwrap();
    assert!(p.execute(&f.repo(), &Cancel::default()).is_err());
    assert!(!f.0.join("outside/new.txt").exists());
}
