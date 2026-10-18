use plico::{
    diff_target::Target, working_copy,
    git::{Area, Change, Cancel, Repository},
};
use std::{
    fs,
    path::{PathBuf},
    process::Command,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "gitui-working-copy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        let f = Self(p);
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Reviewer"]);
        f.git(&["config", "user.email", "review@example.invalid"]);
        f
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
    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn target(f: &Fixture, area: Area, status: &str, path: &str, old: Option<&str>) -> Target {
    Target::new(Repository::new(&f.0), None, Change { area, status: status.into(), path: path.into(), old_path: old.map(Into::into) }).unwrap()
}
#[test]
fn index_and_head_are_distinct_and_working_bytes_keep_their_format() {
    let f = Fixture::new(); f.git(&["config", "core.autocrlf", "false"]);
    f.write("file.txt", "head\n"); f.commit("base");
    f.write("file.txt", "index\n"); f.git(&["add", "file.txt"]);
    fs::write(f.0.join("file.txt"), b"\xef\xbb\xbfworking\r\n").unwrap();
    for (area, expected) in [(Area::WorkingTree,"head\n"),(Area::Unstaged,"index\n")] {
        let mut contents=working_copy::read(&target(&f,area,"M","file.txt",None),&Cancel::default()).unwrap();
        assert_eq!(contents.original,expected); assert_eq!(contents.working.text(),"working\n");
        contents.working.edit("edited\n".into()).unwrap();
        let request=contents.working.begin_save().unwrap().unwrap();
        assert_eq!(&*request.bytes,b"\xef\xbb\xbfedited\r\n");
    }
    assert_eq!(f.git(&["show", ":file.txt"]),"index");
}
#[test]
fn renamed_base_and_literal_path_are_read_without_patch_reconstruction() {
    let f=Fixture::new(); f.write("old[1].txt","original without newline");f.commit("base");
    fs::rename(f.0.join("old[1].txt"),f.0.join("new.txt")).unwrap();
    f.write("new.txt","changed");
    let c=working_copy::read(&target(&f,Area::WorkingTree,"R","new.txt",Some("old[1].txt")),&Cancel::default()).unwrap();
    assert_eq!(c.original,"original without newline");assert_eq!(c.working.text(),"changed");
}
#[test]
fn new_empty_and_unchanged_files_do_not_depend_on_patch_hunks() {
    let f=Fixture::new(); f.write("new.txt","");
    let c=working_copy::read(&target(&f,Area::Untracked,"?","new.txt",None),&Cancel::default()).unwrap();
    assert_eq!(c.original,"");assert_eq!(c.working.text(),"");
    f.write("same.txt","same\n"); f.commit("base");
    let c=working_copy::read(&target(&f,Area::Unstaged,"M","same.txt",None),&Cancel::default()).unwrap();
    assert_eq!(c.original,c.working.text());
}
#[test]
fn immutable_binary_and_missing_endpoints_are_rejected() {
    let f=Fixture::new();f.write("file.txt","base");f.commit("base");
    for area in [Area::Staged,Area::Conflict] {
        assert!(working_copy::read(&target(&f,area,"M","file.txt",None),&Cancel::default()).is_err());
    }
    fs::write(f.0.join("file.txt"),b"binary\0data").unwrap();
    assert!(working_copy::read(&target(&f,Area::Unstaged,"M","file.txt",None),&Cancel::default()).is_err());
    assert!(working_copy::read(&target(&f,Area::Untracked,"?","missing.txt",None),&Cancel::default()).is_err());
}
