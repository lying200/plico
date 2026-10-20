use plico::{
    git::{Cancel, Repository},
    line_changes,
    partial_commit::{self, Chunk},
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
        let path = std::env::temp_dir().join(format!(
            "plico-chunk-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.autocrlf", "false"],
            vec!["config", "core.hooksPath", "disabled-hooks"],
        ] {
            f.git(&args);
        }
        f
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
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
        out.stdout
    }
    fn write(&self, path: &str, text: &str) {
        fs::write(self.0.join(path), text).unwrap();
    }
    fn repo(&self) -> Repository {
        Repository::new(self.0.clone())
    }
    fn chunk(&self, path: &str, current: &str, index: usize) -> Chunk {
        let oid =
            line_changes::base_oid(&self.repo(), std::path::Path::new(path), &Cancel::default())
                .unwrap()
                .unwrap();
        let base = line_changes::base_text(&self.repo(), &oid, &Cancel::default()).unwrap();
        Chunk {
            path: path.into(),
            base_oid: oid,
            block: line_changes::blocks(&base, current)[index].clone(),
            current: current.into(),
            message: "Selected block".into(),
        }
    }
    fn commit(&self, c: &Chunk) -> plico::commit_operation::Report {
        partial_commit::execute(&self.repo(), c, &Cancel::default(), &Default::default()).unwrap()
    }
    fn base(&self, path: &str, text: &str) {
        self.write(path, text);
        self.write("other", "base\n");
        self.git(&["add", "."]);
        self.git(&["commit", "-m", "base"]);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn chunk_only_preserves_worktree_and_other_staged_content() {
    let f = Fixture::new();
    f.base("file [1] 雪", "A\nB\nC\nD\nE\n");
    f.write("other", "staged\n");
    f.git(&["add", "other"]);
    let other = f.git(&["ls-files", "--stage", "--", "other"]);
    f.write("other", "unstaged\n");
    f.write("file [1] 雪", "X\nB\nC\nD\nE\n");
    f.git(&["add", "--", "file [1] 雪"]);
    let current = "X\nB\nC\nD\nZ\n";
    f.write("file [1] 雪", current);
    let chunk = f.chunk("file [1] 雪", current, 1);
    assert!(f.commit(&chunk).succeeded());
    assert_eq!(f.git(&["show", "HEAD:file [1] 雪"]), b"A\nB\nC\nD\nZ\n");
    assert_eq!(
        fs::read(f.0.join("file [1] 雪")).unwrap(),
        current.as_bytes()
    );
    assert_eq!(f.git(&["ls-files", "--stage", "--", "other"]), other);
    assert_eq!(f.git(&["show", "HEAD:other"]), b"base\n");
    assert_eq!(fs::read(f.0.join("other")).unwrap(), b"unstaged\n");
}
#[test]
fn additions_deletions_eof_bom_and_line_endings() {
    for (base, current, wanted) in [
        ("A\nB\nC", "A\nnew\nB\nZ", "A\nnew\nB\nC"),
        ("A\nB\nC\nD", "A\nC\nZ", "A\nC\nD"),
        ("A\nB\nC", "X\nB\nC\n", "X\nB\nC"),
        ("\u{feff}A\r\nB\r\nC", "X\nB\nZ", "\u{feff}X\r\nB\r\nC"),
    ] {
        let f = Fixture::new();
        f.base("file", base);
        let disk = if base.contains('\r') {
            format!("\u{feff}{}", current.replace('\n', "\r\n"))
        } else {
            current.into()
        };
        f.write("file", &disk);
        assert!(f.commit(&f.chunk("file", current, 0)).succeeded());
        assert_eq!(f.git(&["show", "HEAD:file"]), wanted.as_bytes());
        assert_eq!(fs::read(f.0.join("file")).unwrap(), disk.as_bytes());
    }
}
#[test]
fn autocrlf_and_clean_filter_are_applied() {
    let f = Fixture::new();
    f.git(&["config", "core.autocrlf", "true"]);
    f.base("file", "A\r\nB\r\nC");
    f.write("file", "X\r\nB\r\nZ");
    assert!(f.commit(&f.chunk("file", "X\nB\nZ", 1)).succeeded());
    assert_eq!(f.git(&["show", "HEAD:file"]), b"A\nB\nZ");
    assert_eq!(fs::read(f.0.join("file")).unwrap(), b"X\r\nB\r\nZ");
}
#[test]
fn stale_source_base_range_and_cancel_do_not_stage() {
    for case in ["source", "base", "range", "cancel"] {
        let f = Fixture::new();
        f.base("file", "A\nB\nC");
        f.write("file", "X\nB\nZ");
        let mut c = f.chunk("file", "X\nB\nZ", 0);
        match case {
            "source" => f.write("file", "newer\nB\nZ"),
            "base" => {
                f.git(&["add", "file"]);
                f.git(&["commit", "-m", "external"]);
            }
            "range" => c.block.original = 90..91,
            _ => {}
        }
        let index = f.git(&["ls-files", "--stage", "-z"]);
        let head = f.git(&["rev-parse", "HEAD"]);
        let token = Cancel::default();
        if case == "cancel" {
            token.cancel();
        }
        assert!(partial_commit::execute(&f.repo(), &c, &token, &Default::default()).is_err());
        assert_eq!(f.git(&["ls-files", "--stage", "-z"]), index);
        assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    }
}
#[test]
fn rejected_commit_retains_worktree_and_restores_excluded_index() {
    let f = Fixture::new();
    f.base("file", "A\nB\nC");
    f.write("file", "X\nB\nZ");
    f.write("other", "staged\n");
    f.git(&["add", "other"]);
    let other = f.git(&["show", ":other"]);
    let head = f.git(&["rev-parse", "HEAD"]);
    let c = f.chunk("file", "X\nB\nZ", 0);
    f.git(&["config", "user.name", ""]);
    let report = f.commit(&c);
    assert!(!report.succeeded());
    assert!(report.restore.is_ok());
    assert_eq!(f.git(&["show", ":other"]), other);
    assert_eq!(f.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.0.join("file")).unwrap(), b"X\nB\nZ");
    // Like upstream, the explicitly selected file retains its prepared index.
    assert_eq!(f.git(&["show", ":file"]), b"X\nB\nC");
}

#[test]
fn clean_filter_and_executable_mode_use_git_index_rules() {
    let f = Fixture::new();
    f.git(&["config", "filter.upper.clean", "tr a-z A-Z"]);
    f.git(&["config", "filter.upper.smudge", "cat"]);
    f.write(".gitattributes", "file filter=upper\n");
    f.base("file", "A\nB\nC");
    f.git(&["update-index", "--chmod=+x", "file"]);
    f.git(&["commit", "-m", "executable"]);
    f.write("file", "x\nB\nz");
    assert!(f.commit(&f.chunk("file", "x\nB\nz", 1)).succeeded());
    assert_eq!(f.git(&["show", "HEAD:file"]), b"A\nB\nZ");
    assert!(f.git(&["ls-tree", "HEAD", "file"]).starts_with(b"100755"));
    assert_eq!(fs::read(f.0.join("file")).unwrap(), b"x\nB\nz");
}

#[test]
fn mixed_partial_lines_blocks_and_whole_files_commit_together() {
    use plico::{
        commit_operation,
        partial_selection::{File, Selection},
    };
    let f = Fixture::new();
    let old = "a\nb\nkeep\nc\nd\nend\ne\n";
    let current = "A\nB\nkeep\nC\nD\nend\nE\n";
    f.base("review", old);
    f.write("review", current);
    f.write("whole", "whole\n");
    f.write("other", "staged\n");
    f.git(&["add", "other"]);
    f.write("other", "unstaged\n");
    let mut selection = Selection::new(old.into(), current.into(), false);
    assert_eq!(selection.blocks.len(), 3);
    selection.toggle_block(2);
    selection.set_lines(true, 0..1, true);
    selection.set_lines(false, 0..1, true);
    assert_eq!(selection.compose().unwrap(), "b\nA\nkeep\nc\nd\nend\nE\n");
    let request = commit_operation::Request {
        add: vec!["review".into(), "whole".into()],
        remove: vec![],
        message: "partial and whole".into(),
        amend: false,
        expected_head: commit_operation::head(&f.repo(), &Cancel::default()).unwrap(),
    };
    let files = vec![File {
        path: "review".into(),
        selection,
    }];
    assert!(
        commit_operation::execute_partial(
            &f.repo(),
            &request,
            &files,
            &Cancel::default(),
            &Default::default()
        )
        .unwrap()
        .succeeded()
    );
    assert_eq!(
        f.git(&["show", "HEAD:review"]),
        b"b\nA\nkeep\nc\nd\nend\nE\n"
    );
    assert_eq!(f.git(&["show", "HEAD:whole"]), b"whole\n");
    assert_eq!(fs::read_to_string(f.0.join("review")).unwrap(), current);
    assert_eq!(f.git(&["show", ":other"]), b"staged\n");
    assert_eq!(fs::read_to_string(f.0.join("other")).unwrap(), "unstaged\n");
}
#[test]
fn partial_selection_rejects_stale_or_unselected_file_before_staging() {
    use plico::{
        commit_operation,
        partial_selection::{File, Selection},
    };
    let f = Fixture::new();
    f.base("review", "a\nkeep\nb\n");
    f.write("review", "A\nkeep\nB\n");
    let mut selection = Selection::new("a\nkeep\nb\n".into(), "A\nkeep\nB\n".into(), false);
    selection.toggle_block(0);
    let request = commit_operation::Request {
        add: vec!["review".into()],
        remove: vec![],
        message: "partial".into(),
        amend: false,
        expected_head: commit_operation::head(&f.repo(), &Cancel::default()).unwrap(),
    };
    let files = vec![File {
        path: "review".into(),
        selection,
    }];
    let index = f.git(&["ls-files", "--stage"]);
    f.write("review", "different\n");
    assert!(
        commit_operation::execute_partial(
            &f.repo(),
            &request,
            &files,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(index, f.git(&["ls-files", "--stage"]));
    let mut invalid = request;
    invalid.add = vec!["other".into()];
    assert!(
        commit_operation::execute_partial(
            &f.repo(),
            &invalid,
            &files,
            &Cancel::default(),
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(index, f.git(&["ls-files", "--stage"]));
}
