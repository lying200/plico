//! Conflict operations either own an independent recovery or borrow the active
//! Update conversation. This keeps the primary update/credential task alive.
use super::*;
use plico::{
    active_operation::{Id, Kind, Phase},
    merge_result::Prepared,
    project_update::conflicts::channel::Client,
};

#[derive(Clone, Default)]
pub(in crate::ui) struct Access(Option<Client>);
impl Access {
    pub fn prepare(
        &self,
        repo: &Repository,
        snapshot: &Arc<Snapshot>,
        file: &plico::conflicts::File,
        token: &Cancel,
    ) -> Result<Arc<Prepared>, String> {
        match &self.0 {
            Some(client) => client.prepare(snapshot.clone(), file.clone()),
            None => Prepared::read(repo, snapshot, file, token).map(Arc::new),
        }
    }
    pub fn resolve(
        &self,
        repo: &Repository,
        snapshot: &Arc<Snapshot>,
        file: &plico::conflicts::File,
        resolution: Resolution,
        token: &Cancel,
    ) -> Result<Snapshot, String> {
        match &self.0 {
            Some(client) => client.resolve(snapshot.clone(), file.clone(), resolution),
            None => plico::conflicts::resolve(
                repo,
                snapshot,
                file,
                resolution,
                token,
                &Default::default(),
            )
            .and_then(|r| plico::merge_result::checked_report(r, &file.path)),
        }
    }
    pub fn apply(
        &self,
        prepared: &Arc<Prepared>,
        repo: &Repository,
        token: &Cancel,
    ) -> Result<(), String> {
        match &self.0 {
            Some(client) => client.apply(prepared.clone()),
            None => prepared.apply(repo, token),
        }
    }
    pub fn saved_text(
        &self,
        prepared: &Arc<Prepared>,
        repo: &Repository,
        token: &Cancel,
    ) -> Result<String, String> {
        match &self.0 {
            Some(client) => client.saved_text(prepared.clone()),
            None => prepared.saved_text(repo, token),
        }
    }
    pub fn stage(
        &self,
        prepared: &Arc<Prepared>,
        repo: &Repository,
        token: &Cancel,
    ) -> Result<Snapshot, String> {
        match &self.0 {
            Some(client) => client.stage(prepared.clone()),
            None => prepared
                .stage(repo, token, &Default::default())
                .and_then(|r| plico::merge_result::checked_report(r, &prepared.file().path)),
        }
    }
    pub fn stage_resolved(
        &self,
        drafts: &[Arc<Prepared>],
        token: &Cancel,
    ) -> plico::merge_result::StageResult {
        match &self.0 {
            Some(client) => client.stage_resolved(drafts.to_vec()),
            None => plico::merge_result::stage_resolved(drafts, token),
        }
    }
    pub fn automatic(
        &self,
        prepared: &Arc<Prepared>,
        repo: &Repository,
        saved: Option<&plico::text_merge::State>,
        token: &Cancel,
    ) -> Result<Option<plico::merge_result::AutomaticResult>, String> {
        match &self.0 {
            Some(client) => client.resolve_automatically(prepared.clone(), saved.cloned()),
            None => prepared.resolve_automatically(repo, saved, token),
        }
    }
}
pub(in crate::ui) struct Action {
    id: Option<Id>,
    view: Option<Entity<ConflictList>>,
    pub access: Access,
}
impl Review {
    fn update_conflict_client(&self, cx: &App) -> Option<Client> {
        let view = self.conflict_session.as_ref()?.read(cx);
        if view.repository != self.repo.path {
            return None;
        }
        match &view.continuation {
            Some(Continuation::Update(client)) => Some(client.clone()),
            _ => None,
        }
    }
    pub(in crate::ui) fn with_conflict_documents(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        action: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if let Some(client) = self.update_conflict_client(cx) {
            // Update saved documents before starting and keeps them locked.
            // A newly pending document must not be silently overwritten.
            if client.active()
                && self.operation.is_running(Kind::Update)
                && !self.documents.pending(cx)
            {
                action(self, window, cx);
            }
        } else {
            self.with_saved_documents(window, cx, action);
        }
    }
    pub(in crate::ui) fn begin_conflict_operation(
        &mut self,
        phase: Phase,
        cx: &mut Context<Self>,
    ) -> Option<(Action, Cancel)> {
        if let Some(client) = self.update_conflict_client(cx) {
            let view = self.conflict_session.clone()?;
            if !client.active() || !self.operation.is_running(Kind::Update) || view.read(cx).busy {
                return None;
            }
            view.update(cx, |s, cx| {
                s.busy = true;
                cx.notify();
            });
            let token = client.cancel_token();
            Some((
                Action {
                    id: None,
                    view: Some(view),
                    access: Access(Some(client)),
                },
                token,
            ))
        } else {
            let (id, token) = self.begin_operation(Kind::Recovery, phase, cx)?;
            Some((
                Action {
                    id: Some(id),
                    view: None,
                    access: Access::default(),
                },
                token,
            ))
        }
    }
    pub(in crate::ui) fn finish_conflict_operation(
        &mut self,
        action: Action,
        cx: &mut Context<Self>,
    ) -> Option<bool> {
        if let Some(id) = action.id {
            return self.finish_operation(id, cx);
        }
        let view = action.view?;
        if !self.owns_conflict_session(&view, cx) {
            return None;
        }
        let old = action.access.0?;
        if !self.update_conflict_client(cx)?.same_interaction(&old) {
            return None;
        }
        view.update(cx, |s, cx| {
            s.busy = false;
            cx.notify();
        });
        Some(!old.active())
    }
}
