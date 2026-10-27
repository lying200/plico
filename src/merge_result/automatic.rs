//! One iterative automatic-merge step. The caller owns ordering and progress;
//! this module owns input validation, decisions and guarded working-file writes.
use super::*;
use crate::text_merge::{Merge, State};

pub struct AutomaticResult {
    pub decisions: State,
    pub changed: bool,
}
impl Prepared {
    pub fn resolve_automatically(
        &self,
        repo: &Repository,
        saved: Option<&State>,
        token: &Cancel,
    ) -> Result<Option<AutomaticResult>, String> {
        self.resolve_automatically_with(
            repo,
            saved,
            token,
            || self.saved_text(repo, token),
            || self.apply(repo, token),
        )
    }

    pub(crate) fn resolve_automatically_reserved(
        &self,
        repo: &Repository,
        guard: &crate::operation_queue::Guard,
        saved: Option<&State>,
        token: &Cancel,
    ) -> Result<Option<AutomaticResult>, String> {
        if repo.path != self.root || !guard.owns(&repo.operation_key(token)?) {
            return Err("Merge result does not belong to this update interaction".into());
        }
        self.resolve_automatically_with(
            repo,
            saved,
            token,
            || self.saved_text_reserved(repo, guard, token),
            || self.apply_reserved(repo, guard, token),
        )
    }

    fn resolve_automatically_with(
        &self,
        repo: &Repository,
        saved: Option<&State>,
        token: &Cancel,
        saved_text: impl FnOnce() -> Result<String, String>,
        apply: impl FnOnce() -> Result<(), String>,
    ) -> Result<Option<AutomaticResult>, String> {
        self.snapshot.revalidate(repo, &self.file, token)?;
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        let Some([yours, base, theirs]) = self.native_text_inputs()? else {
            return Ok(None);
        };
        let mut model = Merge::new(&base, &yours, &theirs);
        if let Some(saved) = saved {
            model.restore(saved, &saved_text()?);
        }
        let before = model.remaining();
        for i in 0..model.changes.len() {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            model.resolve_automatically(i);
        }
        let changed = model.remaining() < before;
        if changed {
            if token.cancelled() {
                return Err("Cancelled".into());
            }
            let result = self
                .write_text_result(model.text().to_owned())
                .and_then(|_| apply());
            if let Err(error) = result {
                self.retain_output();
                return Err(error);
            }
        }
        // A zero-effect attempt must not save the model's seeded base over the
        // actual conflicted working file (MultipleFileMergeDialog upstream).
        Ok(Some(AutomaticResult {
            decisions: model.state(),
            changed,
        }))
    }
}
