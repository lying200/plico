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

#[cfg(feature="desktop")]
#[test]
fn eol_only_working_diff_has_highlight_ranges() {
    use gpui_kit::component::diff::{DiffFile,DiffSide};
    let f=Fixture::new(); f.git(&["config","core.autocrlf","false"]);
    f.write("file.txt","one\ntwo\n"); f.commit("LF base");
    f.write("file.txt","one\r\ntwo\r\n");
    let contents=working_copy::read(&target(&f,Area::WorkingTree,"M","file.txt",None),&Cancel::default()).unwrap();
    let file=DiffFile::from_editor_sources(Some(("file.txt",&contents.original)),Some(("file.txt",&contents.working.comparison_text(contents.working.text())))).unwrap();
    assert!(file.has_changes(),"CRLF-only change disappeared before diff construction");
    assert!(!file.changed_ranges(DiffSide::Modified).is_empty(),"No highlight for changed line endings");
}

#[cfg(feature="desktop")]
#[test]
fn editor_eol_comparison_uses_normalized_offsets_and_preserves_saves() {
    use gpui_kit::component::diff::{DiffFile,DiffSide,UnifiedEditProjection};
    use plico::file_document::Document;
    for (old,new,changed) in [
        ("中文\nlast\n","中文\r\nlast\r\n",true),
        ("中文\r\nlast\r\n","中文\nlast\n",true),
        ("中文\r\nlast\n","中文\nlast\n",true),
        ("中文\rlast\r","中文\nlast\n",true),
        ("中文\r\nlast","中文\r\nlast",false),
        ("中文\nlast\n","中文\nlast",true),
    ] {
        let mut document=Document::new(new.as_bytes().to_vec()).unwrap();
        let raw=document.comparison_text(document.text());
        assert_eq!(raw,new);
        let file=DiffFile::from_editor_sources(Some(("a",old)),Some(("a",&raw))).unwrap();
        assert_eq!(file.has_changes(),changed,"{old:?} -> {new:?}");
        assert_eq!(file.source(DiffSide::Modified),document.text());
        for side in [DiffSide::Original,DiffSide::Modified] {
            for range in file.changed_ranges(side) { assert!(file.source(side).get(range).is_some()); }
        }
        let projection=UnifiedEditProjection::new(&file);
        assert!(!projection.text().contains('\r'));
        assert!(document.begin_save().unwrap().is_none(),"Viewing EOL diff must not rewrite disk bytes");
        document.edit(format!("{}edited\n",document.text())).unwrap();
        let comparison=document.comparison_text(document.text());
        let save=document.begin_save().unwrap().unwrap();
        assert_eq!(comparison.as_bytes(),save.bytes.as_ref());
    }
}

#[cfg(feature="desktop")]
#[test]
fn large_history_preview_keeps_complete_sources_and_changed_ranges() {
    use gpui_kit::component::diff::{DiffFile, DiffSide};
    let f=Fixture::new();
    let before=(0..30_000).map(|n|format!("let value_{n} = {n};\n")).collect::<String>();
    f.write("large.rs",&before);f.commit("base");
    let after=before.replace("let value_15000 = 15000;","let value_15000 = 42;");
    f.write("large.rs",&after);f.commit("middle change");
    let repo=Repository::new(&f.0);let cancel=Cancel::default();
    let commit=repo.snapshot(None,1,None,&cancel).unwrap().commits.remove(0);
    let change=repo.details(&commit,&cancel).unwrap().files.remove(0);
    let target=Target::new(repo,Some(commit),change).unwrap();
    let file=DiffFile::parse(&target.read(&cancel).unwrap()).unwrap().remove(0);
    assert_eq!(file.source(DiffSide::Original),before);
    assert_eq!(file.source(DiffSide::Modified),after);
    assert!(!file.change_line_ranges().is_empty());
}

#[test]
fn refreshing_index_base_does_not_read_or_write_the_working_file() {
    let f=Fixture::new();f.write("file.txt","base\n");f.commit("base");
    let target=target(&f,Area::Unstaged,"M","file.txt",None);
    f.write("file.txt","new staged\n");f.git(&["add","file.txt"]);
    fs::remove_file(f.0.join("file.txt")).unwrap();
    assert_eq!(working_copy::read_base(&target,&Cancel::default()).unwrap(),"new staged\n");
    assert!(!f.0.join("file.txt").exists());
}

#[test]
fn formerly_untracked_tab_follows_external_commit_baseline() {
    let f=Fixture::new(); f.write("new.txt","first\n");
    let target=target(&f,Area::Untracked,"?","new.txt",None);
    assert_eq!(working_copy::read_base(&target,&Cancel::default()).unwrap(),"");
    f.commit("external commit");
    assert_eq!(working_copy::read_base(&target,&Cancel::default()).unwrap(),"first\n");
}
