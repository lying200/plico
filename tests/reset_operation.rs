use plico::{git::{Repository,Cancel}, reset_operation::{Mode,Prepared}};
use std::{fs, path::PathBuf, process::Command, sync::atomic::{AtomicUsize, Ordering}};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("plico-reset-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-b", "main"]);
        f.git(&["config", "user.name", "Test"]);
        f.git(&["config", "user.email", "test@example.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "core.autocrlf", "false"]);
        f.git(&["config", "core.hooksPath", "disabled-hooks"]);
        f
    }
    fn command(&self) -> Command { let mut c = Command::new("git"); c.arg("-C").arg(&self.0); c }
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let out = self.command().args(args).output().unwrap();
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        out.stdout
    }
    fn write(&self, name: &str, content: &str) { fs::write(self.0.join(name), content).unwrap(); }

}
impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }


fn history()->(Fixture,String,String) {
    let f=Fixture::new(); f.write("file","base\n");f.write("other","other\n");f.git(&["add","."]);f.git(&["commit","-m","base"]);
    let base=String::from_utf8(f.git(&["rev-parse","HEAD"])).unwrap().trim().to_owned();
    f.write("file","second\n");f.git(&["add","."]);f.git(&["commit","-m","second"]);
    let head=String::from_utf8(f.git(&["rev-parse","HEAD"])).unwrap().trim().to_owned();(f,base,head)
}
#[test]
fn modes_preserve_or_reset_index_and_worktree() {
    for mode in [Mode::Soft,Mode::Mixed,Mode::Hard] {
        let (f,base,_)=history();let repo=Repository::new(&f.0);
        f.write("file","staged\n");f.git(&["add","file"]);f.write("file","unstaged\n");f.write("untracked","keep me");
        let prepared=Prepared::prepare(&repo,base.clone(),&Cancel::default()).unwrap();
        let report=prepared.execute(&repo,mode,&Cancel::default(),&Default::default()).unwrap();
        assert!(report.command.unwrap().status.success());assert!(report.recovery_error.is_none());
        assert_eq!(String::from_utf8(f.git(&["rev-parse","HEAD"])).unwrap().trim(),base);
        assert_eq!(f.git(&["show",":file"]),if mode==Mode::Soft{b"staged\n".as_slice()}else{b"base\n"});
        assert_eq!(fs::read(f.0.join("file")).unwrap(),if mode==Mode::Hard{b"base\n".as_slice()}else{b"unstaged\n"});
        assert_eq!(fs::read(f.0.join("untracked")).unwrap(),b"keep me");
    }
}
#[test]
fn keep_preserves_unrelated_edits_and_rejects_overwrites() {
    for overlap in [false,true] {
        let(f,base,head)=history();let repo=Repository::new(&f.0);
        f.write(if overlap{"file"}else{"other"},"valuable\n");
        let report=Prepared::prepare(&repo,base.clone(),&Cancel::default()).unwrap().execute(&repo,Mode::Keep,&Cancel::default(),&Default::default()).unwrap();
        assert_eq!(report.command.unwrap().status.success(),!overlap);
        assert_eq!(String::from_utf8(f.git(&["rev-parse","HEAD"])).unwrap().trim(),if overlap{head}else{base});
        assert_eq!(fs::read(f.0.join(if overlap{"file"}else{"other"})).unwrap(),b"valuable\n");
    }
}
#[test]
fn revalidates_head_branch_and_cancellation() {
    for change in ["head","branch","cancel"] {
        let(f,base,head)=history();let repo=Repository::new(&f.0);
        let prepared=Prepared::prepare(&repo,base.clone(),&Cancel::default()).unwrap();let token=Cancel::default();
        match change {"head"=>{f.git(&["reset","--soft",&base]);},"branch"=>{f.git(&["checkout","-b","another"]);},_=>token.cancel()}
        f.write("file","valuable\n");
        assert!(prepared.execute(&repo,Mode::Hard,&token,&Default::default()).is_err());
        assert_eq!(fs::read(f.0.join("file")).unwrap(),b"valuable\n");
        assert_eq!(String::from_utf8(f.git(&["rev-parse","HEAD"])).unwrap().trim(),if change=="head"{base}else{head});
    }
}
#[test]
fn detached_and_same_head_reset_and_invalid_target() {
    let(f,base,head)=history();let repo=Repository::new(&f.0);
    assert!(Prepared::prepare(&repo,"--hard".into(),&Cancel::default()).is_err());
    f.write("file","dirty");
    let r=Prepared::prepare(&repo,head.clone(),&Cancel::default()).unwrap().execute(&repo,Mode::Hard,&Cancel::default(),&Default::default()).unwrap();
    assert!(r.command.unwrap().status.success());assert_eq!(fs::read(f.0.join("file")).unwrap(),b"second\n");
    f.git(&["checkout","--detach",&head]);
    let r=Prepared::prepare(&repo,base,&Cancel::default()).unwrap().execute(&repo,Mode::Hard,&Cancel::default(),&Default::default()).unwrap();
    assert!(r.command.unwrap().status.success());assert!(!f.command().args(["symbolic-ref","-q","HEAD"]).status().unwrap().success());
}

#[test]
fn lock_failure_and_in_progress_operation_do_not_reset() {
    let(f,base,head)=history();let repo=Repository::new(&f.0);
    let prepared=Prepared::prepare(&repo,base.clone(),&Cancel::default()).unwrap();
    fs::write(f.0.join(".git/index.lock"),"held").unwrap();
    let report=prepared.execute(&repo,Mode::Hard,&Cancel::default(),&Default::default()).unwrap();
    assert!(!report.command.unwrap().status.success());assert!(report.recovery_error.is_some());
    assert_eq!(String::from_utf8(f.git(&["rev-parse","HEAD"])).unwrap().trim(),head);
    fs::remove_file(f.0.join(".git/index.lock")).unwrap();
    fs::write(f.0.join(".git/MERGE_HEAD"),format!("{base}\n")).unwrap();
    assert!(Prepared::prepare(&repo,base,&Cancel::default()).is_err());
}

#[cfg(unix)]
#[test]
fn cancellation_during_hard_reset_leaves_a_complete_repository() {
    use std::{thread,time::{Duration,Instant}};
    let f=Fixture::new();
    for (value,message) in [("base\n","base"),("second\n","second")] {
        f.write("a.txt",value);f.write("z.txt",value);f.git(&["add","."]);f.git(&["commit","-m",message]);
    }
    let base=String::from_utf8(f.git(&["rev-parse","HEAD~"])).unwrap().trim().to_owned();
    f.write(".gitattributes","z.txt filter=delay\n");
    let marker=f.0.join("started");
    f.git(&["config","filter.delay.smudge",&format!("touch '{}'; sleep 1; cat",marker.display())]);
    let repo=Repository::new(&f.0);let token=Cancel::default();
    let prepared=Prepared::prepare(&repo,base.clone(),&token).unwrap();
    let worker_token=token.clone();
    let worker=thread::spawn(move||prepared.execute(&repo,Mode::Hard,&worker_token,&Default::default()));
    let deadline=Instant::now()+Duration::from_secs(5);
    while !marker.exists() && Instant::now()<deadline {thread::sleep(Duration::from_millis(5));}
    assert!(marker.exists(),"smudge fixture did not start");
    token.cancel();let _=worker.join().unwrap();
    // Once the non-transactional checkout has begun, finishing is safe;
    // killing it can leave HEAD/index/files describing different revisions.
    assert_eq!(String::from_utf8(f.git(&["rev-parse","HEAD"])).unwrap().trim(),base,"Reset was killed midway");
    assert_eq!(fs::read(f.0.join("a.txt")).unwrap(),b"base\n");
    assert_eq!(fs::read(f.0.join("z.txt")).unwrap(),b"base\n");
    assert!(!f.0.join(".git/index.lock").exists(),"Abandoned Git lock");
}
