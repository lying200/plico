//! Ref checkout preparation. Rebased GitCheckoutAction / GitRemoteBranchesUtil /
//! GitBranchCheckoutOperation (4aec35256d765c217aa0d662b82ac4e860c396fd).
use crate::{git::{Repository, Cancel}, operation_process::{self, Progress, Outcome}};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub reference: String,
    pub target: String,
    pub head: String,
    symbolic_head: String,
    raw_ref: String,
    pub local: Option<String>,
    local_tip: Option<String>,
    upstream: Option<String>,
    pub naming_conflict: bool,
    pub local_ahead: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice { Checkout, Rebase, Reset }

fn optional_ref(repo: &Repository, reference: &str, token: &Cancel) -> Result<Option<String>, String> {
    let refs=repo.text(&["for-each-ref", "--format=%(refname)%09%(objectname)", reference],token)?;
    Ok(refs.lines().filter_map(|line|line.split_once('\t')).find(|(name,_)|*name==reference).map(|(_,oid)|oid.to_owned()))
}
fn upstream(repo: &Repository, reference: &str, token: &Cancel) -> Result<Option<String>, String> {
    let refs=repo.text(&["for-each-ref", "--format=%(refname)%09%(upstream)", reference],token)?;
    Ok(refs.lines().filter_map(|line|line.split_once('\t')).find(|(name,_)|*name==reference).and_then(|(_,value)|(!value.is_empty()).then(||value.to_owned())))
}
impl Prepared {
    pub fn read(repo: &Repository, reference: &str, target: &str, head: &str, local_name: Option<&str>, token: &Cancel) -> Result<Self,String> {
        if !["refs/heads/","refs/remotes/","refs/tags/"].iter().any(|p|reference.starts_with(p)) {return Err("Unsupported checkout reference".into());}
        repo.text(&["check-ref-format",reference],token)?;
        let actual=repo.text(&["rev-parse","--verify","--end-of-options",&format!("{reference}^{{commit}}")],token)?;
        if actual.trim()!=target {return Err("Reference changed; refresh before checking out".into());}
        let current=repo.text(&["rev-parse","--verify","HEAD"],token)?;
        if current.trim()!=head {return Err("HEAD changed; refresh before checking out".into());}
        let symbolic_head=repo.text(&["rev-parse","--symbolic-full-name","HEAD"],token)?.trim().to_owned();
        let raw_ref=optional_ref(repo,reference,token)?.ok_or("Reference no longer exists")?;
        let mut result=Self {reference:reference.into(),target:target.into(),head:head.into(),symbolic_head,raw_ref,local:None,local_tip:None,upstream:None,naming_conflict:false,local_ahead:false};
        if let Some(remote_ref)=reference.strip_prefix("refs/remotes/") {
            // Remote names may contain slashes; use configured names, longest first.
            let remotes=repo.text(&["remote"],token)?;
            let remote=remotes.lines().filter(|r|remote_ref.starts_with(&format!("{r}/"))).max_by_key(|r|r.len()).ok_or("Remote no longer exists")?;
            let suggested=&remote_ref[remote.len()+1..];
            let name=local_name.unwrap_or(suggested);
            if name.is_empty() || name.starts_with('-') {return Err("Invalid branch name".into());}
            let local_ref=format!("refs/heads/{name}");
            repo.text(&["check-ref-format",&local_ref],token)?;
            let tip=optional_ref(repo,&local_ref,token)?;
            let tracking=upstream(repo,&local_ref,token)?;
            result.naming_conflict=if local_name.is_some() {tip.is_some()} else {tracking.as_deref().is_some_and(|v|v!=reference)};
            if let Some(tip)=&tip {
                result.local_ahead=!repo.text(&["rev-list","--max-count=1",&format!("{target}..{tip}"),"--"],token)?.trim().is_empty();
            }
            result.local=Some(name.into()); result.local_tip=tip; result.upstream=tracking;
        }
        Ok(result)
    }
    fn validate(&self,repo:&Repository,token:&Cancel)->Result<(),String> {
        let current=repo.text(&["rev-parse","--verify","HEAD"],token)?;
        let symbolic=repo.text(&["rev-parse","--symbolic-full-name","HEAD"],token)?;
        if current.trim()!=self.head || symbolic.trim()!=self.symbolic_head {return Err("HEAD changed; refresh before checking out".into());}
        if optional_ref(repo,&self.reference,token)?.as_ref()!=Some(&self.raw_ref) {return Err("Reference changed; refresh before checking out".into());}
        if let Some(name)=&self.local {
            let reference=format!("refs/heads/{name}");
            if optional_ref(repo,&reference,token)?!=self.local_tip || upstream(repo,&reference,token)?!=self.upstream {return Err("Local branch changed; retry checkout".into());}
        }
        let state=repo.operation_state(token)?;
        if state.in_progress() || !state.conflicts.is_empty() {return Err("Resolve the current Git operation before checkout".into());}
        if self.naming_conflict {return Err("Choose a new local branch name".into());}
        Ok(())
    }
}

pub struct Report {pub verified: bool, pub command: Outcome, pub state: crate::operation_state::State, pub head: String, pub symbolic_head: String}
/// Preparation never writes. All choices revalidate the displayed refs under the
/// repository operation lock; resetting local commits is an explicit choice.
pub fn execute(repo:&Repository,prepared:&Prepared,choice:Choice,token:&Cancel,progress:&Progress)->Result<Report,String> {
    let _guard=repo.operation_guard(token)?;
    prepared.validate(repo,token)?;
    if prepared.local_ahead && choice==Choice::Checkout {return Err("Choose how to handle local commits".into());}
    if choice==Choice::Rebase && (!prepared.local_ahead || prepared.local.is_none()) {return Err("Invalid checkout rebase choice".into());}
    let mut command=repo.operation_command();
    command.env("LC_ALL","C").env("LANG","C");
    match (&prepared.local,choice) {
        (Some(name),Choice::Rebase)=>{command.env("GIT_EDITOR","true").args(["rebase","--no-autostash",&prepared.target,name]);}
        (Some(name),_)=>{command.args(["checkout","--track",if prepared.local_tip.is_some(){"-B"}else{"-b"},name,&prepared.reference,"--"]);}
        (None,_)=>{let reference=prepared.reference.strip_prefix("refs/heads/").unwrap_or(&prepared.reference);command.args(["checkout",reference,"--"]);}
    }
    let command=operation_process::run(&mut command,token,progress,None).map_err(|e|e.to_string())?;
    let refresh=Cancel::default();
    let state=repo.operation_state(&refresh)?;
    let head=repo.text(&["rev-parse","--verify","HEAD"],&refresh)?.trim().to_owned();
    let symbolic_head=repo.text(&["rev-parse","--symbolic-full-name","HEAD"],&refresh)?.trim().to_owned();
    let expected_branch=prepared.local.as_ref().map(|name|format!("refs/heads/{name}"))
        .or_else(||prepared.reference.starts_with("refs/heads/").then(||prepared.reference.clone()));
    let attached=expected_branch.as_ref().map_or(state.detached,|name|!state.detached && &symbolic_head==name);
    let target_matches=if choice==Choice::Rebase {
        repo.text(&["rev-list","--max-count=1",&format!("{}..{}",head,prepared.target),"--"],&refresh)?.trim().is_empty()
    } else {head==prepared.target};
    let verified=command.status.success() && command.stopped.is_none() && !state.in_progress()
        && state.conflicts.is_empty() && attached && target_matches;
    Ok(Report {verified,command,state,head,symbolic_head})
}
