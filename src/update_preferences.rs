//! GitVcsOptions defaults and GitPushOperation.savePushUpdateSettings.
//! Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd, Apache-2.0.
use crate::{update_method::Method, update_decision::{Action, Answer, Problem}};
use std::{io::Read, path::Path};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Strategy { #[default] Merge, Rebase, BranchDefault }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences { pub strategy: Strategy, pub auto_update: bool }
impl Preferences {
    pub fn resolve(self, branch_default: Method) -> Method {
        match self.strategy { Strategy::Merge => Method::Merge, Strategy::Rebase => Method::Rebase, Strategy::BranchDefault => branch_default }
    }
    /// Only a confirmed dialog result changes preferences. Unknown merge-check
    /// results retain strategy like the warning case, a local conservative adaptation.
    pub fn accepted(self, answer: Answer, problem: Problem) -> Self {
        let strategy = match answer.action {
            Action::Cancel => return self,
            Action::Merge => Strategy::Merge,
            Action::Rebase => Strategy::Rebase,
        };
        Self { auto_update: answer.remember,
            strategy: if problem == Problem::No && self.strategy != Strategy::BranchDefault { strategy } else { self.strategy } }
    }
    fn encode(self) -> String {
        format!("version=1\nstrategy={}\nauto-update={}\n", match self.strategy {
            Strategy::Merge => "merge", Strategy::Rebase => "rebase", Strategy::BranchDefault => "branch-default",
        }, self.auto_update)
    }
}
fn parse(text: &str) -> Option<Preferences> {
    let mut lines = text.lines();
    if lines.next()? != "version=1" { return None; }
    let strategy = match lines.next()? {
        "strategy=merge" => Strategy::Merge, "strategy=rebase" => Strategy::Rebase,
        "strategy=branch-default" => Strategy::BranchDefault, _ => return None,
    };
    let auto_update = match lines.next()? { "auto-update=true" => true, "auto-update=false" => false, _ => return None };
    if lines.next().is_some() { return None; }
    Some(Preferences { strategy, auto_update })
}
/// Private per-worktree storage: ordinary repos use .git, linked worktrees use
/// their own administrative directory. Cloning does not copy this consent.
/// Resolving/loading creates no directories and does not modify Git config.
pub fn for_repository(repo: &crate::git::Repository, token: &crate::git::Cancel)
    -> Result<(std::path::PathBuf, Preferences), String> {
    let directory = repo.editor_directory(token)?;
    let path = directory.join("git-review").join("push-update-preferences");
    let preferences = load(&path);
    Ok((path, preferences))
}

/// Caller supplies a workspace-scoped file; never store automatic-update consent
/// in a global preference or in version-controlled repository content.
pub fn load(path: &Path) -> Preferences {
    let mut text = String::new();
    if std::fs::File::open(path).and_then(|f| f.take(129).read_to_string(&mut text)).is_err() || text.len() > 128 {
        return Preferences::default();
    }
    parse(&text).unwrap_or_default()
}
pub fn save(path: &Path, preferences: Preferences) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) { std::fs::create_dir_all(parent)?; }
    // Incomplete writes cannot enable automatic updates: the strict reader
    // requires the entire versioned record and otherwise falls back to false.
    std::fs::write(path, preferences.encode())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_defaults_and_branch_default_are_distinct() {
        let p = Preferences::default();
        assert!(!p.auto_update); assert_eq!(p.resolve(Method::Rebase), Method::Merge);
        let p = Preferences { strategy: Strategy::BranchDefault, auto_update: false };
        assert_eq!(p.resolve(Method::Rebase), Method::Rebase);
        assert_eq!(p.resolve(Method::Merge), Method::Merge);
    }
    #[test]
    fn confirmed_choices_follow_upstream_persistence_rules() {
        for strategy in [Strategy::Merge, Strategy::Rebase, Strategy::BranchDefault] {
            for auto_update in [true, false] {
                let previous = Preferences { strategy, auto_update };
                for problem in [Problem::No, Problem::Yes, Problem::Unknown] {
                    for remember in [true, false] {
                        assert_eq!(previous.accepted(Answer { action: Action::Cancel, remember }, problem), previous);
                        for action in [Action::Merge, Action::Rebase] {
                            let next = previous.accepted(Answer { action, remember }, problem);
                            assert_eq!(next.auto_update, remember);
                            let expected = if strategy == Strategy::BranchDefault || problem != Problem::No { strategy }
                                else if action == Action::Merge { Strategy::Merge } else { Strategy::Rebase };
                            assert_eq!(next.strategy, expected);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn bounded_records_roundtrip_and_corruption_never_enables_auto_update() {
        let path = std::env::temp_dir().join(format!("gitui-update-preferences-{}", std::process::id())).join("settings");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load(&path), Preferences::default());
        for strategy in [Strategy::Merge, Strategy::Rebase, Strategy::BranchDefault] {
            for auto_update in [false, true] {
                let p = Preferences { strategy, auto_update }; save(&path, p).unwrap(); assert_eq!(load(&path), p);
            }
        }
        for text in ["version=2\nstrategy=rebase\nauto-update=true\n".to_owned(),
            "version=1\nstrategy=rebase\n".into(), "auto-update=true\n".into(),
            "version=1\nstrategy=other\nauto-update=true\n".into(),
            "version=1\nstrategy=rebase\nauto-update=true\nauto-update=false\n".into(), "x".repeat(129)] {
            std::fs::write(&path, text).unwrap(); assert_eq!(load(&path), Preferences::default());
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
