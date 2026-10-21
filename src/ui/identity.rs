//! Identity preflight shared by file/partial commits, before any index mutation.
use super::*;
impl Review {
    pub(super) fn with_commit_identity(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        action: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Commit,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        let path = repo.path.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let read_repo = repo.clone();
            let result = wx
                .background_spawn(async move {
                    plico::identity_config::Identity::read(&read_repo, &token)
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_operation(id, cx) != Some(false) || s.repo.path != path {
                    return;
                }
                match result {
                    Ok(identity) if identity.missing() => {
                        let owner = cx.entity().downgrade();
                        crate::identity_dialog::show(repo, identity, w, cx, move |w, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                if s.repo.path == path && !s.operation.busy() {
                                    action(s, w, cx);
                                }
                            });
                        });
                    }
                    Ok(_) => action(s, w, cx),
                    Err(error) => {
                        crate::error_ui::show_details(error, "Could not read Git identity", w, cx)
                    }
                }
            });
        });
    }
}
