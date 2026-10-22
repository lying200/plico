//! Amend message lifecycle from Rebased AmendCommitHandlerImpl and
//! NonModalAmendCommitHandler, 4aec352 (JetBrains, Apache-2.0).
use crate::git::{Cancel, Repository};
const MESSAGE_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Amend {
    Off,
    Loading(String),
    Ready(String),
    Failed(String),
}
pub struct Draft {
    message: String,
    amend: Amend,
    generation: u64,
    edit_revision: u64,
    loading_revision: u64,
    automatic: Option<(String, String)>,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            message: String::new(),
            amend: Amend::Off,
            generation: 0,
            edit_revision: 0,
            loading_revision: 0,
            automatic: None,
        }
    }
}
impl Draft {
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn amend(&self) -> &Amend {
        &self.amend
    }
    pub fn edit(&mut self, message: String) -> Result<(), String> {
        validate(&message)?;
        if self.message != message {
            self.message = message;
            self.edit_revision += 1;
        }
        Ok(())
    }
    /// Caller starts a cancellable background load using this ticket and OID.
    pub fn enable_amend(&mut self, oid: &str) -> Result<u64, String> {
        if !crate::git::is_oid(oid) {
            return Err("Invalid amend commit identity".into());
        }
        self.disable_amend();
        self.loading_revision = self.edit_revision;
        self.amend = Amend::Loading(oid.into());
        Ok(self.generation)
    }
    /// Late completions after toggling off/reloading must never change the draft.
    /// Editing while loading also wins over the asynchronous automatic message.
    pub fn loaded(&mut self, ticket: u64, oid: &str, result: Result<String, String>) -> bool {
        if ticket != self.generation
            || !matches!(&self.amend, Amend::Loading(expected) if expected == oid)
        {
            return false;
        }
        let message = match result.and_then(|s| validate(&s).map(|()| s)) {
            Ok(message) => message,
            Err(error) => {
                self.amend = Amend::Failed(error);
                return true;
            }
        };
        if self.loading_revision == self.edit_revision && !equivalent(&self.message, &message) {
            let previous = std::mem::replace(&mut self.message, message.clone());
            self.automatic = Some((previous, message));
        }
        self.amend = Amend::Ready(oid.into());
        true
    }
    pub fn disable_amend(&mut self) {
        self.generation += 1;
        if let Some((before, automatic)) = self.automatic.take() {
            if self.message == automatic {
                self.message = before;
            }
        }
        self.amend = Amend::Off;
    }
    /// A failed/cancelled submission must not call this method.
    pub fn committed(&mut self) {
        self.generation += 1;
        self.edit_revision += 1;
        self.message.clear();
        self.amend = Amend::Off;
        self.automatic = None;
    }
    pub fn can_submit(&self, selected_files: usize) -> bool {
        !self.message.trim().is_empty()
            && match self.amend {
                Amend::Ready(_) => true,
                Amend::Off => selected_files > 0,
                _ => false,
            }
    }
}
fn validate(message: &str) -> Result<(), String> {
    if message.len() > MESSAGE_LIMIT || message.contains('\0') {
        Err("Invalid or oversized commit message".into())
    } else {
        Ok(())
    }
}
fn equivalent(a: &str, b: &str) -> bool {
    // StringsKmp.equalsIgnoreWhitespaces ignores exactly SP, TAB and LF, not CR
    // or all Unicode whitespace. Keep the original text if already equivalent.
    a.chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\n'))
        .eq(b.chars().filter(|c| !matches!(c, ' ' | '\t' | '\n')))
}
pub fn load_amend_message(repo: &Repository, oid: &str, token: &Cancel) -> Result<String, String> {
    if !crate::git::is_oid(oid) {
        return Err("Invalid amend commit identity".into());
    }
    if crate::commit_operation::head(repo, token)?.as_deref() != Some(oid) {
        return Err("HEAD changed; reload Amend".into());
    }
    let message = repo.text(
        &[
            "log",
            "--max-count=1",
            "--encoding=UTF-8",
            "--pretty=format:%B",
            oid,
            "--",
        ],
        token,
    )?;
    validate(&message)?;
    if crate::commit_operation::head(repo, token)?.as_deref() != Some(oid) {
        return Err("HEAD changed; reload Amend".into());
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEAD: &str = "1111111111111111111111111111111111111111";
    #[test]
    fn untouched_automatic_message_restores_original_but_manual_edit_survives() {
        for edit in [false, true] {
            let mut d = Draft::default();
            d.edit("new work draft".into()).unwrap();
            let ticket = d.enable_amend(HEAD).unwrap();
            assert!(!d.can_submit(1));
            assert!(d.loaded(ticket, HEAD, Ok("previous commit\n".into())));
            assert!(d.can_submit(0));
            if edit {
                d.edit("revised previous commit".into()).unwrap();
            }
            d.disable_amend();
            assert_eq!(
                d.message(),
                if edit {
                    "revised previous commit"
                } else {
                    "new work draft"
                }
            );
            assert!(!d.can_submit(0));
        }
    }
    #[test]
    fn stale_results_and_typing_during_load_do_not_overwrite_user_text() {
        let mut d = Draft::default();
        let stale = d.enable_amend(HEAD).unwrap();
        d.disable_amend();
        assert!(!d.loaded(stale, HEAD, Ok("stale".into())));
        let ticket = d.enable_amend(HEAD).unwrap();
        d.edit("typed during load".into()).unwrap();
        assert!(d.loaded(ticket, HEAD, Ok("old message".into())));
        assert_eq!(d.message(), "typed during load");
        d.disable_amend();
        assert_eq!(d.message(), "typed during load");
    }
    #[test]
    fn whitespace_rules_failure_and_success_lifecycle() {
        let mut d = Draft::default();
        d.edit("a b\n".into()).unwrap();
        let ticket = d.enable_amend(HEAD).unwrap();
        d.loaded(ticket, HEAD, Ok("ab".into()));
        assert_eq!(d.message(), "a b\n");
        d.disable_amend();
        assert!(!equivalent("a\rb", "ab"));
        assert!(!equivalent("a\u{a0}b", "ab"));
        let ticket = d.enable_amend(HEAD).unwrap();
        d.loaded(ticket, HEAD, Err("failed".into()));
        assert!(!d.can_submit(1));
        assert_eq!(d.message(), "a b\n");
        d.disable_amend();
        assert!(d.can_submit(1));
        d.committed();
        assert_eq!(d.message(), "");
        assert_eq!(d.amend(), &Amend::Off);
        assert!(!d.can_submit(1));
    }
}
