use plico::{
    clone_repository::{self as clone, Request},
    git::Cancel,
};
use std::{fs, path::PathBuf, process::Command};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut id = [0u8; 16];
        getrandom::fill(&mut id).unwrap();
        // This fixture tests clone semantics, not file-URL metacharacters.
        let id: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir().join(format!("plico-clone-{id}"));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn git(&self, args: &[&str]) -> String {
        let o = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().into()
    }
    fn source(&self) {
        self.git(&["init", "-b", "main"]);
        for (k, v) in [
            ("user.name", "Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "disabled-hooks"),
        ] {
            self.git(&["config", k, v]);
        }
        for n in 0..3 {
            fs::write(self.0.join("file"), format!("version {n}\n")).unwrap();
            self.git(&["add", "file"]);
            self.git(&["commit", "-m", "source"]);
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn request(source: &Fixture, target: &Fixture) -> Request {
    Request {
        source: source.0.to_str().unwrap().into(),
        destination: target.0.join("new parent/clone folder"),
        depth: None,
        recurse_submodules: false,
    }
}
#[test]
fn clones_and_sets_tracking_without_changing_source() {
    let source = Fixture::new();
    source.source();
    let target = Fixture::new();
    let request = request(&source, &target);
    let head = source.git(&["rev-parse", "HEAD"]);
    let report = clone::execute(&request, &Cancel::default(), &Default::default(), None).unwrap();
    assert!(report.succeeded());
    assert!(report.destination_exists);
    assert_eq!(
        fs::read_to_string(report.destination.join("file")).unwrap(),
        "version 2\n"
    );
    let cloned = Fixture(report.destination);
    assert_eq!(cloned.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(cloned.git(&["config", "branch.main.remote"]), "origin");
    assert_eq!(source.git(&["rev-parse", "HEAD"]), head);
}
#[test]
fn accepts_empty_directory_but_never_overwrites_existing_files() {
    let source = Fixture::new();
    source.source();
    let target = Fixture::new();
    let request = request(&source, &target);
    fs::create_dir_all(&request.destination).unwrap();
    fs::write(request.destination.join("keep"), "untouched").unwrap();
    assert!(clone::execute(&request, &Cancel::default(), &Default::default(), None).is_err());
    assert_eq!(
        fs::read_to_string(request.destination.join("keep")).unwrap(),
        "untouched"
    );
    fs::remove_file(request.destination.join("keep")).unwrap();
    assert!(
        clone::execute(&request, &Cancel::default(), &Default::default(), None)
            .unwrap()
            .succeeded()
    );
}
#[test]
fn cancelled_before_start_does_not_create_parent_directories() {
    let source = Fixture::new();
    source.source();
    let target = Fixture::new();
    let request = request(&source, &target);
    let cancel = Cancel::default();
    cancel.cancel();
    assert!(clone::execute(&request, &cancel, &Default::default(), None).is_err());
    assert!(!request.destination.parent().unwrap().exists());
}
#[test]
fn failure_is_not_success_and_does_not_delete_sibling_data() {
    let source = Fixture::new();
    let target = Fixture::new();
    fs::write(target.0.join("keep"), "keep").unwrap();
    let report = clone::execute(
        &request(&source, &target),
        &Cancel::default(),
        &Default::default(),
        None,
    )
    .unwrap();
    assert!(!report.succeeded());
    assert!(!report.diagnostic().is_empty());
    assert_eq!(fs::read_to_string(target.0.join("keep")).unwrap(), "keep");
}
#[test]
fn shallow_clone_uses_requested_depth() {
    let source = Fixture::new();
    source.source();
    let target = Fixture::new();
    let mut request = request(&source, &target);
    request.source = url::Url::from_directory_path(&source.0).unwrap().into();
    request.depth = Some(1);
    assert!(
        clone::execute(&request, &Cancel::default(), &Default::default(), None)
            .unwrap()
            .succeeded()
    );
    let cloned = Fixture(request.destination);
    assert_eq!(cloned.git(&["rev-list", "--count", "HEAD"]), "1");
}
#[test]
fn validates_url_forms_without_treating_input_as_a_command() {
    for s in [
        "http://example.com/git/repo.git",
        "git@example.com:dir/repo.git",
        "user@example.com:/home/user@example.com/project",
    ] {
        assert!(clone::source_url(s).is_ok());
    }
    assert_eq!(
        clone::source_url(" git clone https://example.com/repo.git ").unwrap(),
        "https://example.com/repo.git"
    );
    for s in [
        "",
        "--upload-pack=other",
        "https://example.com/repo\n.git",
        "something://not a url",
    ] {
        assert!(clone::source_url(s).is_err());
    }
}
