//! Rebased GitPushTargetPanel: define a remote without leaving Push.
use super::*;
impl Review {
    pub(super) fn define_push_remote(
        &mut self,
        draft: Entity<crate::push_dialog::PushDialog>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || draft.read(cx).closed {
            return;
        }
        let repo = self.repo.clone();
        let path = repo.path.clone();
        let owner = cx.entity().downgrade();
        let draft = draft.downgrade();
        crate::define_remote_dialog::show(
            repo,
            None,
            "origin".into(),
            w,
            cx,
            move |edit, fetch, w, cx| {
                let Some(draft) = draft.upgrade() else {
                    return;
                };
                if draft.read(cx).closed {
                    return;
                }
                let _ = owner.update(cx, |s, cx| {
                    if s.repo.path == path {
                        s.add_push_remote(edit, fetch, draft, w, cx);
                    }
                });
            },
        );
    }
    fn add_push_remote(
        &mut self,
        edit: plico::remote_management::Edit,
        fetch: bool,
        draft: Entity<crate::push_dialog::PushDialog>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let plico::remote_management::Edit::Add { name, .. } = &edit else {
            return;
        };
        let name = name.clone();
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::ConfigureRemote,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        draft.update(cx, |s, cx| s.configuring_remote(cx));
        self.status_message = Some(t("Define Remote").into());
        let repo = self.repo.clone();
        let path = repo.path.clone();
        let requests = plico::auth_request::Requests::persistent(token.clone());
        requests.set_project(&path);
        let worker_requests = requests.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = (|| -> Result<(), String> {
                let report =
                    plico::remote_management::execute(&repo, &edit, &token, &Default::default())?;
                if !report.succeeded() {
                    return Err(report.diagnostic());
                }
                if fetch && !token.cancelled() {
                    let helper = std::env::current_exe()
                        .map_err(|e| e.to_string())?
                        .with_file_name(if cfg!(windows) {
                            "plico-askpass.exe"
                        } else {
                            "plico-askpass"
                        });
                    let spec = plico::fetch::Spec::new(name)?;
                    let outcome = repo.fetch_authenticated(
                        &spec,
                        &token,
                        &Default::default(),
                        &worker_requests,
                        &helper,
                    )?;
                    if !outcome.status.success() || outcome.stopped.is_some() {
                        return Err(String::from_utf8_lossy(&outcome.stderr.bytes()).into_owned());
                    }
                }
                Ok(())
            })();
            // Remote addition can succeed even when fetch is cancelled or fails.
            let snapshot = repo.push_snapshot(&Cancel::default());
            let _ = tx.send((result, snapshot));
        })
        .detach();
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            loop {
                if let Some(pending) = requests.take_pending() {
                    if let Ok(dialog) = wx.update(|w, cx| crate::auth_dialog::show(pending, w, cx)) { login = Some(dialog); }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some((Err("Remote configuration worker ended".into()), Err("Remote configuration worker ended".into()))),
                };
                if let Some((result, snapshot)) = result {
                    let _ = this.update_in(wx, |s, w, cx| {
                        if login.as_ref().is_some_and(|d| d.read(cx).active()) { w.close_dialog(cx); }
                        let Some(cancelled) = s.finish_operation(id, cx) else { return; };
                        if s.repo.path != path { return; }
                        s.status_message = None;
                        draft.update(cx, |d, cx| d.remote_configured(snapshot, w, cx));
                        if cancelled {
                            s.error = Some(t("Remote configuration cancelled; check the current configuration").into());
                        } else if let Err(error) = result {
                            crate::error_ui::show_details(plico::remote_management::display_error(&error), "Git Remotes", w, cx);
                        }
                        s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                        s.refresh_impl(false, true, None, cx);
                        cx.notify();
                    });
                    break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
        });
    }
}
