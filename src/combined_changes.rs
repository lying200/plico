// Copyright 2000-2024 JetBrains s.r.o. and contributors.
// Rust adaptation licensed under Apache-2.0; original sources listed below and in THIRD-PARTY.txt.
//! Rebased CommittedChangesTreeBrowser.zipChanges, commit 4aec352.
//! Algorithm translated from JetBrains (Apache-2.0); see docs/REBASED-REFERENCE.md.
//! Input must be oldest first. This preserves the upstream path-based collision
//! rules; it is not a topology-aware diff between two arbitrary branch tips.
use std::{collections::{HashMap,VecDeque},path::PathBuf};

#[derive(Clone,Debug,PartialEq,Eq,Hash)]
pub struct Revision { pub revision:String, pub path:PathBuf }
#[derive(Clone,Debug,PartialEq,Eq,Hash)]
pub struct Change { pub before:Option<Revision>,pub after:Option<Revision> }
pub const MAX_INPUT_CHANGES:usize=100_000;

/// Absent revisions stay distinct from existing empty files.
pub struct Contents {
    pub before: Option<String>,
    pub after: Option<String>,
}

pub fn read_contents(repo: &crate::git::Repository, change: &Change, token: &crate::git::Cancel) -> Result<Contents, String> {
    use std::{ffi::{OsStr, OsString}, path::Component};
    fn read(repo: &crate::git::Repository, revision: &Revision, token: &crate::git::Cancel) -> Result<String, String> {
        if !crate::git::is_oid(&revision.revision) || revision.path.as_os_str().is_empty()
            || revision.path.components().any(|part| !matches!(part, Component::Normal(_))) {
            return Err("Invalid preview revision or file path".into());
        }
        let mut spec = OsString::from(&revision.revision);
        spec.push(":");
        spec.push(&revision.path);
        // Raw immutable blob content: never run repository filters/textconv or
        // interpret a tree/gitlink as text. Paths are literal, not pathspecs.
        let bytes = repo.read(&[OsStr::new("cat-file"), OsStr::new("blob"), &spec], token)?;
        if bytes.contains(&0) { return Err("Binary file — text preview unavailable".into()); }
        String::from_utf8(bytes).map_err(|_| "Non-UTF-8 text cannot be displayed safely".into())
    }
    if change.before.is_none() && change.after.is_none() { return Err("Change has no revisions".into()); }
    if token.cancelled() { return Err("Cancelled".into()); }
    let before = change.before.as_ref().map(|r| read(repo, r, token)).transpose()?;
    let after = change.after.as_ref().map(|r| read(repo, r, token)).transpose()?;
    if before.as_ref().map_or(0, String::len).saturating_add(after.as_ref().map_or(0, String::len)) > crate::git::OUTPUT_LIMIT {
        return Err("Combined file exceeds the preview output limit".into());
    }
    if token.cancelled() { return Err("Cancelled".into()); }
    Ok(Contents { before, after })
}

/// Convert a committed file status to the revision endpoints used by zipChanges.
/// The caller must supply the parent against which this status was computed;
/// selecting a merge parent is deliberately not inferred here.
pub fn from_file(file: &crate::git::Change, revision: &str, parent: Option<&str>) -> Result<Change, String> {
    use crate::git::Area;
    fn valid_oid(value: &str) -> bool {
        matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
    }
    if file.area != Area::Commit || !valid_oid(revision) || parent.is_some_and(|p| !valid_oid(p)) {
        return Err("Combined preview requires committed revision IDs".into());
    }
    let status = file.status.as_bytes().first().copied().ok_or("Missing committed file status")?;
    let before_path = match status {
        b'A' => None,
        b'M' | b'T' | b'D' => Some(&file.path),
        b'R' | b'C' => Some(file.old_path.as_ref().ok_or("Moved change has no original path")?),
        _ => return Err("Unsupported committed file status".into()),
    };
    let before = match before_path {
        Some(path) => Some(Revision {
            revision: parent.ok_or("Committed change requires a parent revision")?.into(),
            path: path.clone(),
        }),
        None => None,
    };
    let after = (status != b'D').then(|| Revision { revision: revision.into(), path: file.path.clone() });
    Ok(Change { before, after })
}

pub fn zip_changes(changes:impl IntoIterator<Item=Change>)->Result<Vec<Change>,String> {
    // Index buckets by insertion order, as the upstream linked multimap does.
    let mut keys=HashMap::<Option<PathBuf>,usize>::new();
    let mut buckets=Vec::<VecDeque<Change>>::new();
    fn insert(keys:&mut HashMap<Option<PathBuf>,usize>,buckets:&mut Vec<VecDeque<Change>>,key:Option<PathBuf>,change:Change) {
        let index=*keys.entry(key).or_insert_with(||{buckets.push(VecDeque::new());buckets.len()-1});
        buckets[index].push_back(change);
    }
    for (n,change) in changes.into_iter().enumerate() {
        if n>=MAX_INPUT_CHANGES {return Err("Combined preview exceeds the change limit".into());}
        if change.before.is_none() && change.after.is_none() {return Err("Change has no revisions".into());}
        let key=change.after.as_ref().map(|r|r.path.clone());
        let old=change.before.as_ref().and_then(|r|keys.get(&Some(r.path.clone())))
            .and_then(|index|buckets[*index].pop_front());
        let combined=match old {
            Some(old)=>Change{before:old.before,after:change.after},
            None=>change,
        };
        if combined.before.is_some() || combined.after.is_some() {insert(&mut keys,&mut buckets,key,combined);}
    }
    // Deletions are reinserted by their before path after folding, matching upstream.
    if let Some(index)=keys.remove(&None) {
        let deleted=std::mem::take(&mut buckets[index]);
        for change in deleted {
            let key=Some(change.before.as_ref().ok_or("Deletion has no before revision")?.path.clone());
            insert(&mut keys,&mut buckets,key,change);
        }
    }
    Ok(buckets.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn r(path:&str,n:u8)->Revision {Revision{path:path.into(),revision:n.to_string()}}
    fn edit(a:&str,b:&str,from:u8,to:u8)->Change {Change{before:Some(r(a,from)),after:Some(r(b,to))}}
    fn add(path:&str,n:u8)->Change {Change{before:None,after:Some(r(path,n))}}
    fn delete(path:&str,n:u8)->Change {Change{before:Some(r(path,n)),after:None}}
    #[test]
    fn committed_endpoints_fold_root_addition_and_rename_then_delete() {
        let first = "1".repeat(40);
        let second = "2".repeat(40);
        let third = "3".repeat(40);
        let file = |status: &str, path: &str, old: Option<&str>| crate::git::Change {
            area: crate::git::Area::Commit, status: status.into(), path: path.into(), old_path: old.map(Into::into),
        };
        let add = from_file(&file("A", "old", None), &first, None).unwrap();
        let moved = from_file(&file("R100", "new", Some("old")), &second, Some(&first)).unwrap();
        let deleted = from_file(&file("D", "new", None), &third, Some(&second)).unwrap();
        assert_eq!(zip_changes([add.clone(), moved.clone()]).unwrap(), vec![Change {
            before: None, after: Some(Revision { revision: second.clone(), path: "new".into() }),
        }]);
        assert!(zip_changes([add, moved, deleted]).unwrap().is_empty());
        assert!(from_file(&file("M", "a", None), &first, None).is_err());
        assert!(from_file(&file("R100", "a", None), &second, Some(&first)).is_err());
        assert!(from_file(&file("U", "a", None), &second, Some(&first)).is_err());
        assert!(from_file(&file("A", "a", None), "HEAD", None).is_err());
    }
    #[test]
    fn edits_and_rename_chain_keep_earliest_before_and_latest_after() {
        assert_eq!(zip_changes([edit("a","a",0,1),edit("a","b",1,2),edit("b","c",2,3)]).unwrap(),vec![edit("a","c",0,3)]);
        // Upstream intentionally joins by path even when revision numbers are discontinuous.
        assert_eq!(zip_changes([edit("a","a",0,1),edit("a","a",3,4)]).unwrap(),vec![edit("a","a",0,4)]);
    }
    #[test]
    fn added_then_deleted_disappears_but_deleted_then_added_stays_distinct() {
        assert!(zip_changes([add("a",1),delete("a",2)]).unwrap().is_empty());
        assert_eq!(zip_changes([delete("a",1),add("a",2)]).unwrap(),vec![add("a",2),delete("a",1)]);
        assert_eq!(zip_changes([edit("a","b",1,2),delete("b",3)]).unwrap(),vec![delete("a",1)]);
    }
    #[test]
    fn collisions_consume_only_first_candidate_and_preserve_other_files() {
        assert_eq!(zip_changes([add("a",1),add("a",2),edit("a","b",2,3)]).unwrap(),vec![add("a",2),add("b",3)]);
        assert_eq!(zip_changes([edit("a","b",0,1),edit("a","a",2,3)]).unwrap(),vec![edit("a","b",0,1),edit("a","a",2,3)]);
    }
    #[test]
    fn input_budget_is_enforced_even_when_every_pair_cancels_out() {
        let changes=(0..MAX_INPUT_CHANGES+1).map(|i|if i%2==0{add("a",1)}else{delete("a",2)});
        assert!(zip_changes(changes).is_err());
        assert!(zip_changes([Change{before:None,after:None}]).is_err());
    }
}
