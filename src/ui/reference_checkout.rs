//! Shared reference actions for the branch popup and Log reference tree.
use super::*;
use plico::reference_checkout::{Choice, Prepared};
impl Review {
    pub(super) fn reference_checkout_target(&self, reference: &str) -> Option<(String, String)> {
        let snapshot = self.snapshot.as_ref()?;
        let head = snapshot.head_id.clone()?;
        if snapshot
            .current_branch
            .as_ref()
            .is_some_and(|name| reference == format!("refs/heads/{name}"))
        {
            return None;
        }
        if snapshot.current_branch.is_none()
            && snapshot
                .tags
                .iter()
                .find(|tag| tag.id == head)
                .is_some_and(|tag| tag.name == reference)
        {
            return None;
        }
        let target = snapshot
            .branches
            .iter()
            .find(|b| b.name == reference)
            .map(|b| b.id.clone())
            .or_else(|| {
                snapshot
                    .tags
                    .iter()
                    .find(|t| t.name == reference)
                    .map(|t| t.id.clone())
            })?;
        Some((target, head))
    }
    pub(super) fn checkout_reference(
        &mut self,
        reference: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((target, head)) = self.reference_checkout_target(&reference) else {
            return;
        };
        self.dismiss_repository_menu(cx);
        if reference.starts_with("refs/heads/") {
            self.start_local_checkout(reference, target, head, window, cx);
        } else {
            self.prepare_reference_checkout(reference, target, head, None, window, cx);
        }
    }
    fn prepare_reference_checkout(
        &mut self,
        reference: String,
        target: String,
        head: String,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Checking checkout target…").into());
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    Prepared::read(&repo, &reference, &target, &head, name.as_deref(), &token)
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(operation, cx) else {
                    return;
                };
                if cancelled {
                    s.status_message = Some(t("Checkout cancelled; repository refreshed").into());
                    s.refresh_impl(false, true, None, cx);
                    return;
                }
                s.status_message = None;
                match result {
                    Ok(prepared) if prepared.naming_conflict => {
                        s.checkout_name_dialog(prepared, w, cx)
                    }
                    Ok(prepared) if prepared.local_ahead => {
                        s.checkout_commits_dialog(prepared, w, cx)
                    }
                    Ok(prepared) => s.execute_reference_checkout(prepared, Choice::Checkout, w, cx),
                    Err(error) => {
                        s.status_message = Some(format!(
                            "{}: {error}",
                            t("Could not check out branch; repository refreshed")
                        ));
                        s.refresh_impl(false, true, None, cx);
                        cx.notify();
                    }
                }
            });
        });
    }
    fn checkout_name_dialog(
        &mut self,
        prepared: Prepared,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            InputState::new(window, cx).default_value(prepared.local.clone().unwrap_or_default())
        });
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx,move |dialog,_,_| {
            let confirm=input.clone();let owner=owner.clone();let prepared=prepared.clone();let path=path.clone();
            dialog.title(t("Checkout Remote Branch")).w(px(480.)).centered(true).overlay_closable(false)
                .child(div().child(t("The local branch name is already in use or tracks another remote branch. Choose a new name.")))
                .child(Input::new(&input))
                .footer(crate::dialog_style::confirmation(Button::new("checkout-name-ok").primary().label(t("Checkout"))))
                .on_ok(move |_,w,cx| {
                    let name=confirm.read(cx).value().trim().to_owned();if name.is_empty(){return false;}
                    let _=owner.update(cx,|s,cx|if s.repo.path==path {s.prepare_reference_checkout(prepared.reference.clone(),prepared.target.clone(),prepared.head.clone(),Some(name),w,cx);});true
                })
        });
    }
    fn checkout_commits_dialog(
        &mut self,
        prepared: Prepared,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx,move |dialog,_,_| {
            let mut footer=h_flex().gap_2().w_full();
            for (choice,label) in [(Choice::Rebase,"Checkout and Rebase"),(Choice::Reset,"Drop Local Commits")] {
                let owner=owner.clone();let prepared=prepared.clone();let path=path.clone();
                footer=footer.child(Button::new(label).label(t(label)).when(choice==Choice::Rebase,|b|b.primary())
                    .on_click(move |_,w,cx| {w.close_dialog(cx);let _=owner.update(cx,|s,cx|if s.repo.path==path {s.execute_reference_checkout(prepared.clone(),choice,w,cx);});}));
            }
            footer=footer.child(Button::new("checkout-cancel").label(t("Cancel")).on_click(|_,w,cx|w.close_dialog(cx)));
            dialog.title(t("Checkout Remote Branch")).w(px(580.)).centered(true).overlay_closable(false)
                .child(div().child(format!("{} → {}",prepared.local.as_deref().unwrap_or_default(),prepared.reference)))
                .child(div().child(t("The local branch has commits not present in the remote branch. Rebase them onto the remote branch, or drop them and reset the local branch to the remote tip.")))
                .footer(footer)
        });
    }
    fn execute_reference_checkout(
        &mut self,
        prepared: Prepared,
        choice: Choice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.execute_reference_checkout(prepared, choice, w, cx)
            });
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Checkout,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.status_message = Some(t("Checking out branch…").into());
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::reference_checkout::execute(
                        &repo,
                        &prepared,
                        choice,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = this.update_in(wx, |s, _, cx| {
                let Some(cancelled) = s.finish_operation(operation, cx) else {
                    return;
                };
                s.status_message = Some(if cancelled {
                    t("Checkout cancelled; repository refreshed").into()
                } else {
                    match result {
                        Ok(report) => {
                            let success = report.verified;
                            let details = String::from_utf8_lossy(&report.command.stderr.bytes())
                                .into_owned();
                            s.operation_state = Some(report.state);
                            if success {
                                t("Branch checked out").into()
                            } else {
                                format!(
                                    "{}: {details}",
                                    t("Could not check out branch; repository refreshed")
                                )
                            }
                        }
                        Err(error) => format!(
                            "{}: {error}",
                            t("Could not check out branch; repository refreshed")
                        ),
                    }
                });
                s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                s.refresh_impl(false, true, None, cx);
                cx.notify();
            });
        });
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;
    #[gpui::test]
    fn availability_uses_reference_identity_and_not_loaded_history(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            view.update(cx, |s, _| {
                s.snapshot = Some(Snapshot {
                    head_id: Some("a".into()),
                    current_branch: Some("main".into()),
                    included_in_head: Default::default(),
                    ref_titles: Default::default(),
                    commits: vec![],
                    branches: vec![
                        git::Branch {
                            name: "refs/heads/main".into(),
                            id: "a".into(),
                        },
                        git::Branch {
                            name: "refs/heads/other".into(),
                            id: "a".into(),
                        },
                        git::Branch {
                            name: "refs/remotes/origin/topic".into(),
                            id: "outside-history".into(),
                        },
                    ],
                    tags: vec![git::Tag {
                        name: "refs/tags/release".into(),
                        id: "a".into(),
                    }],
                    tips: vec![],
                    more: false,
                    shallow: false,
                });
                assert!(s.reference_checkout_target("refs/heads/main").is_none());
                assert!(s.reference_checkout_target("refs/heads/other").is_some());
                assert!(
                    s.reference_checkout_target("refs/remotes/origin/topic")
                        .is_some()
                );
                assert!(s.reference_checkout_target("refs/tags/release").is_some());
                s.snapshot.as_mut().unwrap().current_branch = None;
                assert!(s.reference_checkout_target("refs/tags/release").is_none());
                assert!(s.reference_checkout_target("refs/heads/main").is_some());
            });
            Root::new(view, window, cx)
        });
        cx.run_until_parked();
    }
}
