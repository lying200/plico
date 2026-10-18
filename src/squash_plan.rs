//! Squash ordering and message draft adapted from Rebased 4aec352:
//! GitRebaseTodoModel.unite, GitSquashOperation and GitSquashedCommitsMessage.
//! Copyright JetBrains s.r.o. and contributors, Apache-2.0.
//! This is a plan, not authorization to rewrite: the caller must independently
//! verify topology, HEAD, operation state, protected refs and recovery ownership.
use std::collections::{HashMap, HashSet};
use crate::git::{is_oid, HISTORY_MAX, OUTPUT_LIMIT};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action { Pick, Reword, Fixup }
#[derive(Debug, Eq, PartialEq)]
pub struct Entry { pub oid: String, pub action: Action }
#[derive(Debug)]
pub struct Plan { entries: Vec<Entry>, original_order: Vec<usize> }
impl Plan {
    pub fn entries(&self) -> &[Entry] { &self.entries }
    /// Complete linear replay range, oldest first. Selection order is immaterial:
    /// upstream matches selection against todo order before calling unite.
    /// Full OIDs deliberately avoid ambiguous abbreviation matching.
    pub fn new(replay: &[String], selected: &[String]) -> Result<Self, String> {
        if replay.len() > HISTORY_MAX || selected.len() > HISTORY_MAX {
            return Err("Squash plan exceeds history limit".into());
        }
        let chosen: HashSet<_> = selected.iter().map(String::as_str).collect();
        let all: HashSet<_> = replay.iter().map(String::as_str).collect();
        if chosen.len() < 2 || chosen.len() != selected.len() || all.len() != replay.len()
            || replay.iter().any(|id| !is_oid(id)) || !chosen.is_subset(&all) {
            return Err("Squash requires distinct commits present in the replay range".into());
        }
        let root = replay.iter().position(|id| chosen.contains(id.as_str())).unwrap();
        let mut entries = Vec::with_capacity(replay.len());
        let mut append = |id: &String, action| entries.push(Entry { oid: id.clone(), action });
        for id in &replay[..root] { append(id, Action::Pick); }
        append(&replay[root], Action::Reword);
        for id in &replay[root + 1..] {
            if chosen.contains(id.as_str()) { append(id, Action::Fixup); }
        }
        for id in &replay[root + 1..] {
            if !chosen.contains(id.as_str()) { append(id, Action::Pick); }
        }
        let positions = entries.iter().enumerate().map(|(i, e)| (e.oid.as_str(), i)).collect::<HashMap<_, _>>();
        let original_order = replay.iter().map(|id| positions[id.as_str()]).collect();
        Ok(Self { entries, original_order })
    }
    /// Reconcile the actual Git-generated pick list before replacing it. Never
    /// silently drop unknown commands or accept a missing/reordered commit.
    /// The caller supplies Git's concrete comment string, not the config value `auto`.
    /// Existing autosquash/update-ref commands need the full upstream model;
    /// until then they fail explicitly rather than being discarded.
    pub fn rewrite_todo(&self, text: &str, comment: &str) -> Result<String, String> {
        if text.len() > 1024 * 1024 || text.contains('\0') || comment.is_empty()
            || comment.chars().any(|c| c.is_whitespace() || c == '\0') {
            return Err("Invalid or oversized Squash todo".into());
        }
        let mut sorted = self.entries.iter().map(|e| e.oid.as_str()).collect::<Vec<_>>();
        sorted.sort_unstable();
        let mut count = 0;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(comment) { continue; }
            let mut fields = line.split_whitespace();
            if !matches!(fields.next(), Some("pick" | "p")) {
                return Err("Squash todo contains an unsupported action; no changes applied".into());
            }
            let prefix = fields.next().ok_or("Missing commit ID in Squash todo")?;
            if !(4..=64).contains(&prefix.len()) || !prefix.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("Invalid commit ID in Squash todo".into());
            }
            let first = sorted.partition_point(|id| *id < prefix);
            let matching = sorted.get(first).filter(|id| id.starts_with(prefix))
                .ok_or("Squash todo contains a commit outside the prepared range")?;
            if sorted.get(first + 1).is_some_and(|id| id.starts_with(prefix)) {
                return Err("Ambiguous commit ID in Squash todo".into());
            }
            let expected = self.original_order.get(count).map(|&i| self.entries[i].oid.as_str());
            if expected != Some(*matching) { return Err("Squash todo order differs from the prepared range".into()); }
            count += 1;
        }
        if count != self.entries.len() { return Err("Squash todo omits prepared commits".into()); }
        Ok(self.todo())
    }
    /// No subjects or user message enter todo syntax. Message is delivered by
    /// the commit editor, separately from the sequence editor.
    fn todo(&self) -> String {
        self.entries.iter().map(|entry| format!("{} {}\n", match entry.action {
            Action::Pick => "pick", Action::Reword => "reword", Action::Fixup => "fixup",
        }, entry.oid)).collect()
    }
}

fn subject(message: &str) -> &str { message.split(['\r', '\n']).next().unwrap_or("") }
fn autosquash_target(subject: &str) -> Option<&str> {
    ["fixup! ", "squash! ", "amend! "].iter().find_map(|prefix|
        subject.strip_prefix(prefix).filter(|target| !target.is_empty()))
}
/// Preserve caller order (Log display order), deduplicate complete messages,
/// and strip autosquash subjects only when their ordinary target is selected.
/// Bounded input/output; no commit content is loaded by this function.
pub fn draft(messages: &[String]) -> Result<String, String> {
    if messages.len() > HISTORY_MAX || messages.iter().try_fold(0usize, |n, s| n.checked_add(s.len()))
        .is_none_or(|n| n > OUTPUT_LIMIT) || messages.iter().any(|s| s.contains('\0')) {
        return Err("Squash messages exceed preview limits or contain NUL".into());
    }
    let subjects: HashSet<_> = messages.iter().map(|s| subject(s))
        .filter(|s| autosquash_target(s).is_none()).collect();
    let mut seen = HashSet::new();
    let mut result = String::new();
    for message in messages {
        let message = if autosquash_target(subject(message)).is_some_and(|target| subjects.contains(target)) {
            let body = message.split_once('\n').map_or("", |(_, body)| body).trim();
            if body.is_empty() { continue; }
            body
        } else { message.as_str() };
        if seen.insert(message) {
            let separator = if seen.len() > 1 { "\n\n\n" } else { "" };
            if result.len() + separator.len() + message.len() > OUTPUT_LIMIT {
                return Err("Squash draft exceeds preview limit".into());
            }
            result.push_str(separator); result.push_str(message);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ids(indices: &[usize]) -> Vec<String> { indices.iter().map(|i| format!("{i:040x}")).collect() }
    #[test]
    fn unite_preserves_unselected_order_and_uses_todo_order() {
        let plan = Plan::new(&ids(&[1,2,3,4,5,6,7]), &ids(&[6,2,4])).unwrap();
        assert_eq!(plan.entries.iter().map(|e| e.oid.clone()).collect::<Vec<_>>(), ids(&[1,2,4,6,3,5,7]));
        assert_eq!(plan.entries.iter().map(|e| e.action).collect::<Vec<_>>(),
            [Action::Pick, Action::Reword, Action::Fixup, Action::Fixup, Action::Pick, Action::Pick, Action::Pick]);
        for selected in [&[1][..], &[1,1], &[1,8]] { assert!(Plan::new(&ids(&[1,2,3]), &ids(selected)).is_err()); }
        assert!(Plan::new(&ids(&[1,1,2]), &ids(&[1,2])).is_err());
        assert!(Plan::new(&["bad".into(), "other".into()], &["bad".into(), "other".into()]).is_err());
    }
    #[test]
    fn actual_todo_must_match_the_complete_ordered_range() {
        let replay = vec![format!("aaaa{}", "0".repeat(36)), format!("bbbb{}", "0".repeat(36)), format!("cccc{}", "0".repeat(36))];
        let plan = Plan::new(&replay, &[replay[2].clone(), replay[0].clone()]).unwrap();
        let text = "# generated\r\np aaaa 中文 subject\r\npick bbbb ; not a command\r\npick cccc # empty\r\n";
        assert_eq!(plan.rewrite_todo(text, "#").unwrap(), plan.todo());
        assert_eq!(plan.rewrite_todo(&text.replace("#", ";;"), ";;").unwrap(), plan.todo());
        for bad in ["pick aaaa\npick cccc", "pick bbbb\npick aaaa\npick cccc", "pick aaaa\npick bbbb\npick bbbb",
            "pick aaaa\npick bbbb\npick dddd", "pick aaaa\npick bbbb\npick cccc\nexec dangerous",
            "noop", "fixup aaaa", "update-ref refs/heads/topic", "pick --bad", "pick aa"] {
            assert!(plan.rewrite_todo(bad, "#").is_err(), "{bad}");
        }
        let ambiguous = Plan::new(&ids(&[1,2]), &ids(&[1,2])).unwrap();
        assert!(ambiguous.rewrite_todo("pick 0000\npick 0000", "#").unwrap_err().contains("Ambiguous"));
    }
    #[test]
    fn message_draft_matches_autosquash_and_deduplication_rules() {
        let check = |input: &[&str], expected| assert_eq!(draft(&input.iter().map(|s| (*s).into()).collect::<Vec<_>>()).unwrap(), expected);
        check(&["fix", "fix", "fix!!!", "fix"], "fix\n\n\nfix!!!");
        check(&["amend! topic\n\n替换", "squash! topic\n\nbody", "fixup! topic", "topic"], "替换\n\n\nbody\n\n\ntopic");
        check(&["fixup! topic", "fixup! topic"], "fixup! topic");
        check(&["fixup! topic", "topic"], "topic");
        check(&["other", "fixup! topic"], "other\n\n\nfixup! topic");
        check(&["fixup! topic\r\n\r\nbody", "topic\r\n"], "body\n\n\ntopic\r\n");
        check(&["fixup! ", ""], "fixup! \n\n\n");
        assert!(draft(&["bad\0message".into()]).is_err());
    }
}
