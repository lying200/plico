//! Inclusion for the Rebased-style non-staging Commit tree. Before/after paths
//! drive staging; excluded index paths are preserved by commit_operation.
use crate::{commit_changes::{File,Snapshot},commit_operation::Request};
use std::{collections::{BTreeSet,HashSet},path::PathBuf};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum Kind { Tracked, Unversioned, Conflict }
#[derive(Clone,Debug,PartialEq,Eq,Hash)]
pub struct Identity {
    pub kind: Kind,
    pub path: PathBuf,
    pub before: Option<PathBuf>,
    pub after: Option<PathBuf>,
}
impl Identity {
    pub fn of(file:&File)->Self {
        match file {
            File::Tracked(c)=>Self{kind:Kind::Tracked,path:c.path.clone(),
                before:if c.status.starts_with('A'){None}else{Some(c.old_path.clone().unwrap_or_else(||c.path.clone()))},
                after:if c.status.starts_with('D'){None}else{Some(c.path.clone())}},
            File::Unversioned(c)=>Self{kind:Kind::Unversioned,path:c.path.clone(),before:None,after:Some(c.path.clone())},
            File::Conflict(c)=>Self{kind:Kind::Conflict,path:c.path.clone(),before:None,after:None},
        }
    }
}
#[derive(Default)]
pub struct Inclusion { available: HashSet<Identity>, included: HashSet<Identity>, head:Option<String> }
impl Inclusion {
    pub fn replace(&mut self,snapshot:&Snapshot) {
        let head_changed=self.head!=snapshot.head;
        self.head=snapshot.head.clone();
        self.available=snapshot.files.iter().map(Identity::of).collect();
        if head_changed {self.included.clear();}
        else {self.included.retain(|id|self.available.contains(id) && id.kind!=Kind::Conflict);}
    }
    pub fn clear(&mut self) {self.included.clear();}
    pub fn count(&self)->usize {self.included.len()}
    pub fn included(&self,id:&Identity)->bool {self.included.contains(id)}
    pub fn set(&mut self,id:&Identity,checked:bool) {
        if id.kind==Kind::Conflict || !self.available.contains(id) {return;}
        if checked {self.included.insert(id.clone());} else {self.included.remove(id);}
    }
    pub fn set_scope(&mut self,ids:&HashSet<Identity>,checked:bool) {
        for id in ids {self.set(id,checked);}
    }
    pub fn scope_counts(&self,ids:&HashSet<Identity>)->(usize,usize) {
        ids.iter().filter(|id|id.kind!=Kind::Conflict && self.available.contains(*id))
            .fold((0,0),|(included,total),id|(included+usize::from(self.included.contains(id)),total+1))
    }
    pub fn request(&self,message:String,amend:bool)->Result<Request,String> {
        if self.available.iter().any(|id|id.kind==Kind::Conflict) {return Err("Resolve conflicts before committing".into());}
        let mut add=BTreeSet::new();let mut remove=BTreeSet::new();
        for id in &self.included {
            add.extend(id.after.iter().cloned());remove.extend(id.before.iter().cloned());
        }
        // Same rule as GitCheckinEnvironment.stageAndCommit: added paths win.
        remove.retain(|path|!add.contains(path));
        if add.is_empty() && remove.is_empty() && !amend {return Err("Select changes to commit".into());}
        Ok(Request{add:add.into_iter().collect(),remove:remove.into_iter().collect(),message,amend,expected_head:self.head.clone()})
    }
}
