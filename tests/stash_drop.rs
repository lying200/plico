use plico::{git::{Repository,Cancel},stash,stash_operation};
use std::{fs,path::PathBuf,process::Command,sync::atomic::{AtomicUsize,Ordering}};
struct Fixture(PathBuf);
impl Fixture {
 fn new()->Self {
  static NEXT:AtomicUsize=AtomicUsize::new(0);
  let f=Self(std::env::temp_dir().join(format!("plico-stash-drop-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed))));
  fs::create_dir(&f.0).unwrap();f.git(&["init","-b","main"]);f.git(&["config","user.name","Test"]);f.git(&["config","user.email","test@example.invalid"]);f.git(&["config","commit.gpgsign","false"]);f.git(&["config","core.hooksPath","disabled-hooks"]);
  fs::write(f.0.join("file"),"base").unwrap();f.git(&["add","file"]);f.git(&["commit","-m","base"]);f
 }
 fn git(&self,args:&[&str])->String {let out=Command::new("git").arg("-C").arg(&self.0).args(args).output().unwrap();assert!(out.status.success(),"{args:?}: {}",String::from_utf8_lossy(&out.stderr));String::from_utf8(out.stdout).unwrap().trim().into()}
 fn save(&self,message:&str)->stash::Entry {fs::write(self.0.join("file"),message).unwrap();self.git(&["stash","push","-m",message]);self.entries().remove(0)}
 fn entries(&self)->Vec<stash::Entry> {stash::list(&Repository::new(&self.0),stash::MAX_ENTRIES,&Cancel::default()).unwrap().entries}
 fn drop_entry(&self,entry:&stash::Entry,token:&Cancel)->Result<plico::operation_process::Outcome,String> {stash_operation::drop_stash(&Repository::new(&self.0),entry,token,&Default::default())}
}
impl Drop for Fixture {fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
#[test] fn drops_only_confirmed_entry_without_changing_worktree_or_index() {
 let f=Fixture::new();let first=f.save("first");let second=f.save("second");let target=f.entries().remove(1);
 assert_eq!(target.oid,first.oid);fs::write(f.0.join("file"),"staged").unwrap();f.git(&["add","file"]);fs::write(f.0.join("file"),"draft").unwrap();
 let head=f.git(&["rev-parse","HEAD"]);let index=f.git(&["write-tree"]);
 let out=f.drop_entry(&target,&Cancel::default()).unwrap();assert!(out.status.success());
 assert_eq!(f.entries().len(),1);assert_eq!(f.entries()[0].oid,second.oid);
 assert_eq!(f.git(&["rev-parse","HEAD"]),head);assert_eq!(f.git(&["write-tree"]),index);assert_eq!(fs::read_to_string(f.0.join("file")).unwrap(),"draft");
}
#[test] fn stale_selector_cannot_delete_a_new_stash() {
 let f=Fixture::new();let old=f.save("first");let new=f.save("second");
 assert!(f.drop_entry(&old,&Cancel::default()).is_err());assert_eq!(f.entries().len(),2);assert_eq!(f.entries()[0].oid,new.oid);
}
#[test] fn cancellation_and_invalid_identity_never_delete() {
 let f=Fixture::new();let mut entry=f.save("first");let token=Cancel::default();token.cancel();
 assert!(f.drop_entry(&entry,&token).is_err());entry.selector="--all".into();assert!(f.drop_entry(&entry,&Cancel::default()).is_err());assert_eq!(f.entries().len(),1);
}
