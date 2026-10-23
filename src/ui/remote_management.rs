//! Remote configuration participates in the same operation ownership and
//! post-write refresh as other explicit Git operations.
use super::*;
impl Review {
    pub(super) fn open_remotes(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.welcome {
            return;
        }
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::ConfigureRemote,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        let path = repo.path.clone();
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let read_repo = repo.clone();
            let result = wx
                .background_spawn(async move { plico::remote_management::read(&read_repo, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_operation(id, cx) != Some(false) || s.repo.path != path {
                    return;
                }
                match result {
                    Ok(rows) => {
                        let owner = cx.entity().downgrade();
                        crate::remotes_dialog::show(
                            repo,
                            rows,
                            w,
                            cx,
                            move |edit, fetch, manager, w, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    if s.repo.path == path {
                                        s.edit_remote(edit, fetch, manager, w, cx);
                                    }
                                });
                            },
                        );
                    }
                    Err(_) => crate::error_ui::show_details(
                        t("Could not reload remote configuration").into(),
                        "Git Remotes",
                        w,
                        cx,
                    ),
                }
            });
        });
    }
    fn edit_remote(
        &mut self,
        edit: plico::remote_management::Edit,
        fetch: bool,
        manager: Entity<crate::remotes_dialog::Manager>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::ConfigureRemote,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let fetch_name = if fetch {
            match &edit {
                plico::remote_management::Edit::Add { name, url }
                | plico::remote_management::Edit::Change { name, url, .. } => {
                    Some((name.clone(), url.clone()))
                }
                _ => None,
            }
        } else {
            None
        };
        manager.update(cx, |s, cx| s.begin(cx));
        let repo = self.repo.clone();
        let path = repo.path.clone();
        self.operation_task=cx.spawn_in(w,async move|this,wx|{
            let (result,rows)=wx.background_spawn(async move{
                let result=plico::remote_management::execute(&repo,&edit,&token,&Default::default());
                // Cancellation may follow a successful rename: always read the
                // actual configuration using a fresh bounded read token.
                let rows=plico::remote_management::read(&repo,&Default::default());
                (result,rows)
            }).await;
            let _=this.update_in(wx,|s,w,cx|{
                let Some(cancelled)=s.finish_operation(id,cx) else{return;};
                let success=!cancelled&&result.as_ref().is_ok_and(|r|r.succeeded());
                let error=if cancelled{Some(t("Remote configuration cancelled; check the current configuration").into())}
                    else if !success{Some(t("Could not update remote configuration; some changes may already have been applied").into())}else{None};
                let mut details = match &result {
                    Ok(report) => report.diagnostic(),
                    Err(error) => plico::remote_management::display_error(error),
                };
                if let Err(error) = &rows {
                    details.push('\n');
                    details.push_str(&plico::remote_management::display_error(error));
                }
                let details = plico::remote_management::display_error(&details);
                let fetch_name=fetch_name.filter(|(name,url)|
                    result.as_ref().is_ok_and(|r|!r.commands.is_empty()) &&
                    rows.as_ref().is_ok_and(|rows|rows.iter().any(|r|r.name==*name&&r.urls.contains(url))))
                    .map(|(name,_)|name);
                manager.update(cx,|s,cx|s.complete(rows,error,cx));
                if s.repo.path!=path{return;}
                if !cancelled && !details.trim().is_empty() {
                    crate::error_ui::show_details(details, "Git Remotes", w, cx);
                }
                s.user_menu.update(cx,|menu,cx|menu.refresh(cx));
                s.refresh_impl(false,true,None,cx);
                if success{if let Some(name)=fetch_name{s.start_fetch_named(Some(name),w,cx);}}
                cx.notify();
            });
        });
    }
}
