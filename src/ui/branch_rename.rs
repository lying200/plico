//! Rebased GitRenameBranchAction and GitNewBranchDialog (RENAME).
use super::*;
use gpui_kit::component::checkbox::Checkbox;
use plico::branch_rename::{self, Prepared};
struct RenameBranch {
    name: Entity<InputState>,
    unset: bool,
    has_upstream: bool,
    existing: Vec<String>,
    busy: bool,
    closed: bool,
    error: Option<String>,
    token: Cancel,
    task: Task<()>,
    _subscription: Subscription,
}
impl Drop for RenameBranch {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl RenameBranch {
    fn invalid(&self, cx: &App) -> bool {
        let value = self.name.read(cx).value();
        let name = value.trim();
        name.is_empty()
            || name.eq_ignore_ascii_case("HEAD")
            || self.existing.iter().any(|n| n == name)
    }
}
impl Render for RenameBranch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .child(
                        div()
                            .pt_1()
                            .w(px(100.))
                            .flex_shrink_0()
                            .child(t("Branch name:")),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_3()
                            .child(Input::new(&self.name).disabled(self.busy).w_full())
                            .when(self.has_upstream, |v| {
                                v.child(
                                    Checkbox::new("rename-unset-upstream")
                                        .label(t("Unset upstream branch"))
                                        .checked(self.unset)
                                        .disabled(self.busy)
                                        .on_click(cx.listener(|s, checked, _, cx| {
                                            s.unset = *checked;
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    div()
                        .whitespace_normal()
                        .text_color(rgb(0xf2a6a6))
                        .child(error),
                )
            })
    }
}
impl Review {
    pub(super) fn branch_rename_item(
        owner: &WeakEntity<Self>,
        reference: &str,
        cx: &App,
    ) -> PopupMenuItem {
        let enabled = owner.upgrade().is_some_and(|s| {
            let s = s.read(cx);
            reference.starts_with("refs/heads/")
                && !s.operation.busy()
                && !s.operation.saving_documents()
                && s.operation_state
                    .as_ref()
                    .is_some_and(|v| !v.in_progress() && v.conflicts.is_empty())
        });
        let owner = owner.clone();
        let reference = reference.to_owned();
        PopupMenuItem::new(t("Rename…"))
            .disabled(!enabled)
            .on_click(move |_, w, cx| {
                let _ = owner.update(cx, |s, cx| {
                    s.prepare_rename_branch(reference.clone(), w, cx)
                });
            })
    }
    pub(super) fn prepare_rename_branch(
        &mut self,
        reference: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tip) = self
            .snapshot
            .as_ref()
            .and_then(|s| s.branches.iter().find(|b| b.name == reference))
            .map(|b| b.id.clone())
        else {
            return;
        };
        self.dismiss_repository_menu(cx);
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::RenameBranch,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    Prepared::read(&repo, &reference, &token).and_then(|p| {
                        if p.tip == tip {
                            Ok(p)
                        } else {
                            Err("Repository changed; review branch rename again".into())
                        }
                    })
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if cancelled {
                    return;
                }
                match result {
                    Ok(p) => s.rename_branch_dialog(p, w, cx),
                    Err(e) => crate::error_ui::show_details(e, "Could not rename branch", w, cx),
                }
            });
        });
    }
    fn rename_branch_dialog(
        &mut self,
        prepared: Prepared,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = self
            .snapshot
            .as_ref()
            .map(|s| {
                s.branches
                    .iter()
                    .filter_map(|b| b.name.strip_prefix("refs/heads/").map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let options = cx.new(|cx| {
            let name =
                cx.new(|cx| InputState::new(window, cx).default_value(prepared.name.clone()));
            name.update(cx, |s, cx| s.select_all(window, cx));
            let subscription = cx.subscribe_in(
                &name,
                window,
                |s: &mut RenameBranch, input, event: &InputEvent, w, cx| {
                    if !matches!(event, InputEvent::Change | InputEvent::Blur) {
                        return;
                    }
                    s.error = None;
                    input.update(cx, |i, cx| {
                        if gpui::EntityInputHandler::marked_text_range(i, w, cx).is_some() {
                            return;
                        }
                        let original = i.value();
                        let clean = plico::branch_create::clean_name_on_typing(&original);
                        if clean != original.as_ref() {
                            let caret =
                                plico::branch_create::clean_name_on_typing(&original[..i.cursor()])
                                    .len()
                                    .min(clean.len());
                            i.set_value(clean, w, cx);
                            i.set_selected_range(caret..caret, cx);
                        }
                    });
                    cx.notify();
                },
            );
            RenameBranch {
                name,
                unset: false,
                has_upstream: prepared.has_upstream,
                existing,
                busy: false,
                closed: false,
                error: None,
                token: Cancel::default(),
                task: Task::ready(()),
                _subscription: subscription,
            }
        });
        let focus = options.read(cx).name.read(cx).focus_handle(cx);
        let owner = cx.entity().downgrade();
        let repo = self.repo.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let confirm = options.clone();
            let close = options.clone();
            let owner = owner.clone();
            let repo = repo.clone();
            let prepared = prepared.clone();
            dialog
                .title(format!("{} {}", t("Rename Branch"), prepared.name))
                .w(px(540.))
                .centered(true)
                .overlay_closable(false)
                .child(options.clone())
                .footer(crate::dialog_style::confirmation(
                    Button::new("rename-branch-confirm")
                        .primary()
                        .label(t("Rename"))
                        .disabled(options.read(cx).busy || options.read(cx).invalid(cx)),
                ))
                .on_ok(move |_, w, cx| {
                    let owner = owner.clone();
                    let repo = repo.clone();
                    let prepared = prepared.clone();
                    confirm.update(cx, |s, cx| {
                        if s.busy || s.closed || s.invalid(cx) {
                            return;
                        }
                        s.busy = true;
                        s.error = None;
                        let name = plico::branch_create::clean_name(&s.name.read(cx).value());
                        let unset = s.unset;
                        let token = s.token.clone();
                        let path = repo.path.clone();
                        s.task = cx.spawn_in(w, async move |this, wx| {
                            let checked = name.clone();
                            let result = wx
                                .background_spawn(async move {
                                    branch_rename::validate_name(&repo, &checked, &token)
                                })
                                .await;
                            let _ = wx.update(|w, cx| {
                                let ready = this
                                    .update(cx, |s, cx| {
                                        if s.closed || s.token.cancelled() {
                                            return false;
                                        }
                                        s.busy = false;
                                        if let Err(e) = result {
                                            s.error = Some(e);
                                            cx.notify();
                                            false
                                        } else {
                                            true
                                        }
                                    })
                                    .unwrap_or(false);
                                if ready {
                                    w.close_dialog(cx);
                                    let _ = owner.update(cx, |s, cx| {
                                        if s.repo.path == path {
                                            s.execute_branch_rename(prepared, name, unset, w, cx);
                                        }
                                    });
                                }
                            });
                        });
                        cx.notify();
                    });
                    false
                })
                .on_close(move |_, _, cx| {
                    close.update(cx, |s, _| {
                        s.closed = true;
                        s.token.cancel();
                    })
                })
        });
        focus.focus(window, cx);
    }
    fn execute_branch_rename(
        &mut self,
        prepared: Prepared,
        name: String,
        unset: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::RenameBranch,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Renaming branch…").into());
        let repo = self.repo.clone();
        let old_ref = format!("refs/heads/{}", prepared.name);
        let new_ref = format!("refs/heads/{name}");
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    branch_rename::execute(
                        &repo,
                        &prepared,
                        &name,
                        unset,
                        &token,
                        &Default::default(),
                    )
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                let (success, renamed, error) = match result {
                    Ok(r) => {
                        let success = r.succeeded();
                        let error = (!success).then(|| r.diagnostic());
                        (success, r.renamed, error)
                    }
                    Err(e) => (false, false, Some(e)),
                };
                if renamed {
                    // Keep an explicitly selected branch attached to its renamed ref.
                    // Favorites remain name-scoped, matching DvcsBranchManager storage.
                    for reference in &mut s.branch_filter {
                        if reference == &old_ref {
                            *reference = new_ref.clone();
                        }
                    }
                    if s.selected_tree_ref.as_deref() == Some(old_ref.as_str()) {
                        s.selected_tree_ref = Some(new_ref.clone());
                    }
                }
                let message = if success {
                    "Branch renamed"
                } else if renamed {
                    "Branch renamed; upstream settings need attention"
                } else if cancelled {
                    "Branch operation cancelled; repository refreshed"
                } else {
                    "Could not rename branch"
                };
                s.status_message = Some(t(message).into());
                if let Some(error) = error {
                    crate::error_ui::show_details(error, message, w, cx);
                }
                s.refresh_impl(false, true, None, cx);
                cx.notify();
            });
        });
        cx.notify();
    }
}
