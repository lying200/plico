//! Rebased GitWorkingTreeDialog: source, optional new branch, name and location.
use super::*;
struct Form {
    reference: Option<String>,
    refs: Vec<String>,
    occupied: Vec<Tree>,
    branch: Entity<InputState>,
    name: Entity<InputState>,
    location: Entity<InputState>,
    new_branch: bool,
    base_name: String,
    busy: bool,
    closed: bool,
    error: Option<String>,
    token: Cancel,
    task: Task<()>,
    _subscription: Subscription,
}
impl Drop for Form {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
fn short(reference: &str) -> &str {
    reference
        .strip_prefix("refs/heads/")
        .or_else(|| reference.strip_prefix("refs/remotes/"))
        .or_else(|| reference.strip_prefix("refs/tags/"))
        .unwrap_or(reference)
}
fn source_icon(reference: &str, trees: &[Tree]) -> &'static str {
    if trees
        .iter()
        .any(|t| t.current && t.branch.as_deref() == Some(reference))
    {
        "currentBranchLabel_dark.svg"
    } else if reference.starts_with("refs/tags/") {
        "branchLabel_dark.svg"
    } else {
        "vcs_dark.svg"
    }
}
impl Form {
    fn browse(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let result = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t("Location:").into()),
        });
        cx.spawn_in(w, async move |this, wx| {
            if let Some(path) = result
                .await
                .ok()
                .and_then(|v| v.ok())
                .flatten()
                .and_then(|v| v.into_iter().next())
            {
                let _ = this.update_in(wx, |s, w, cx| {
                    if !s.closed && !s.busy {
                        s.location
                            .update(cx, |i, cx| i.set_value(project_path_text(&path), w, cx));
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }
    fn suggest(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let branch = self.branch.read(cx).value();
        let reference = if self.new_branch && !branch.is_empty() {
            branch.as_ref()
        } else {
            self.reference.as_deref().unwrap_or_default()
        };
        let value = format!(
            "{}-{}",
            self.base_name,
            reference.rsplit('/').next().unwrap_or_default()
        );
        self.name.update(cx, |s, cx| s.set_value(value, w, cx));
    }
    fn occupied(&self) -> bool {
        !self.new_branch
            && self
                .reference
                .as_ref()
                .is_some_and(|r| self.occupied.iter().any(|t| t.branch.as_ref() == Some(r)))
    }
    fn invalid(&self, cx: &App) -> bool {
        let name = self.name.read(cx).value();
        self.busy
            || self.reference.is_none()
            || self.occupied()
            || name.trim().is_empty()
            || name.contains(['/', '\\'])
            || matches!(name.trim(), "." | "..")
            || self.location.read(cx).value().trim().is_empty()
            || (self.new_branch && self.branch.read(cx).value().trim().is_empty())
    }
}
impl Render for Form {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let refs = self.refs.clone();
        let occupied = self.occupied.clone();
        let busy = self.busy;
        let field = |label: &'static str, input: &Entity<InputState>| {
            h_flex()
                .gap_3()
                .child(div().w(px(100.)).flex_shrink_0().child(t(label)))
                .child(
                    Input::new(input)
                        .external_focus_ring(false)
                        .bg(rgb(BG))
                        .border_color(rgb(0x393b40))
                        .rounded(px(4.))
                        .w_full()
                        .disabled(busy),
                )
        };
        v_flex()
            .pt(px(8.))
            .pb(px(8.))
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .child(div().w(px(100.)).flex_shrink_0().child(t("From branch:")))
                    .child(
                        Button::new("worktree-source")
                            .accessibility_label(t("From branch:"))
                            .w(px(356.))
                            .h(px(24.))
                            .rounded(px(4.))
                            .dropdown_caret(true)
                            .disabled(busy)
                            .child(
                                h_flex()
                                    .gap_2()
                                    .min_w_0()
                                    .when_some(self.reference.as_deref(), |row, reference| {
                                        row.child(jb_icon(source_icon(reference, &self.occupied)))
                                    })
                                    .child(
                                        div().truncate().child(
                                            self.reference
                                                .as_deref()
                                                .map(short)
                                                .unwrap_or(t("Select branch"))
                                                .to_owned(),
                                        ),
                                    ),
                            )
                            .dropdown_menu(move |mut menu, _, _| {
                                for reference in &refs {
                                    let owner = owner.clone();
                                    let r = reference.clone();
                                    let tree =
                                        occupied.iter().find(|t| t.branch.as_ref() == Some(&r));
                                    let name = short(&r).to_owned();
                                    let path = tree
                                        .and_then(|t| t.path.file_name())
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_default();
                                    let icon = source_icon(&r, &occupied);
                                    menu =
                                        menu.item(
                                            PopupMenuItem::element(move |_, _| {
                                                h_flex()
                                                    .gap_3()
                                                    .child(jb_icon(icon))
                                                    .child(name.clone())
                                                    .child(
                                                        div()
                                                            .text_color(rgb(MUTED))
                                                            .child(path.clone()),
                                                    )
                                            })
                                            .on_click(move |_, w, cx| {
                                                let _ = owner.update(cx, |s, cx| {
                                                    s.reference = Some(r.clone());
                                                    s.suggest(w, cx);
                                                    cx.notify();
                                                });
                                            }),
                                        );
                                }
                                menu
                            }),
                    ),
            )
            .when(self.occupied(), |v| {
                v.child(div().pl(px(112.)).text_color(rgb(0xd5a454)).child(t(
                    "Selected branch is checked out; create a new branch or open its worktree.",
                )))
            })
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        div().w(px(100.)).flex_shrink_0().child(
                            crate::ui_style::checkbox("worktree-new-branch")
                                .label(t("New branch:"))
                                .checked(self.new_branch)
                                .disabled(busy)
                                .on_click(cx.listener(|s, v, w, cx| {
                                    s.new_branch = *v;
                                    s.suggest(w, cx);
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        Input::new(&self.branch)
                            .external_focus_ring(false)
                            .bg(rgb(BG))
                            .border_color(rgb(0x393b40))
                            .rounded(px(4.))
                            .w_full()
                            .disabled(busy),
                    ),
            )
            .child(div().h(px(16.)))
            .child(field("Project name:", &self.name))
            .child(
                h_flex()
                    .gap_3()
                    .child(div().w(px(100.)).flex_shrink_0().child(t("Location:")))
                    .child(
                        Input::new(&self.location)
                            .external_focus_ring(false)
                            .bg(rgb(BG))
                            .border_color(rgb(0x393b40))
                            .rounded(px(4.))
                            .flex_1()
                            .disabled(busy)
                            .suffix(
                                Button::new("worktree-browse")
                                    .ghost()
                                    .small()
                                    .icon(jb_icon("open_dark.svg"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|s, _, w, cx| s.browse(w, cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .pl(px(112.))
                    .text_color(rgb(MUTED))
                    .whitespace_normal()
                    .child(format!(
                        "{}\n{}",
                        t("The worktree will be created in:"),
                        std::path::Path::new(self.location.read(cx).value().as_ref())
                            .join(self.name.read(cx).value().as_ref())
                            .display()
                    )),
            )
            .when_some(self.error.clone(), |v, e| {
                v.child(div().text_color(rgb(0xf2a6a6)).whitespace_normal().child(e))
            })
    }
}
impl Review {
    pub(in super::super) fn new_worktree(
        &mut self,
        reference: Option<String>,
        occupied: Vec<Tree>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
            return;
        }
        let refs = self
            .snapshot
            .as_ref()
            .map(|s| {
                // Keep full refs as identities: a branch and tag can share a name.
                // Lexical full-ref order matches Rebased's local/remote/tag groups.
                let mut refs: Vec<_> = s
                    .branches
                    .iter()
                    .map(|b| b.name.clone())
                    .chain(s.tags.iter().map(|t| t.name.clone()))
                    .filter(|r| !(r.starts_with("refs/remotes/") && r.ends_with("/HEAD")))
                    .collect();
                refs.sort();
                refs
            })
            .unwrap_or_default();
        let repo = self.repo.clone();
        let owner = cx.entity().downgrade();
        let form = cx.new(|cx| {
            let branch = cx.new(|cx| InputState::new(w, cx));
            let sub = cx.subscribe_in(&branch, w, |s: &mut Form, _, event: &InputEvent, w, cx| {
                if matches!(event, InputEvent::Change) {
                    s.new_branch = !s.branch.read(cx).value().is_empty();
                    s.suggest(w, cx);
                    cx.notify();
                }
            });
            Form {
                reference,
                refs,
                occupied,
                branch,
                name: cx.new(|cx| InputState::new(w, cx)),
                location: cx.new(|cx| {
                    InputState::new(w, cx)
                        .default_value(project_path_text(repo.path.parent().unwrap_or(&repo.path)))
                }),
                new_branch: false,
                base_name: repo
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                busy: false,
                closed: false,
                error: None,
                token: Cancel::default(),
                task: Task::ready(()),
                _subscription: sub,
            }
        });
        form.update(cx, |s, cx| {
            if s.reference.is_some() {
                s.suggest(w, cx);
            }
        });
        w.open_dialog(cx, move |dialog, _, cx| {
            let confirm = form.clone();
            let close = form.clone();
            let repo = repo.clone();
            let owner = owner.clone();
            dialog
                .title(
                    h_flex()
                        .gap_2()
                        .child(gpui_kit::img("branding/plico.svg").size(px(16.)))
                        .child(
                            div()
                                .font_weight(FontWeight::NORMAL)
                                .child(t("New Worktree")),
                        ),
                )
                .w(px(500.))
                .centered(true)
                .overlay_closable(false)
                .child(form.clone())
                .footer(crate::dialog_style::confirmation(
                    Button::new("create-worktree")
                        .primary()
                        .label(t("Create & Open Worktree"))
                        .disabled(form.read(cx).invalid(cx)),
                ))
                .on_ok(move |_, w, cx| {
                    let repo = repo.clone();
                    let owner = owner.clone();
                    confirm.update(cx, |s, cx| {
                        if s.closed || s.invalid(cx) {
                            return;
                        }
                        let Some(reference) = s.reference.clone() else {
                            return;
                        };
                        let path = PathBuf::from(s.location.read(cx).value().trim())
                            .join(s.name.read(cx).value().trim());
                        let name = s
                            .new_branch
                            .then(|| s.branch.read(cx).value().trim().to_owned());
                        let token = s.token.clone();
                        let repo_path = repo.path.clone();
                        s.busy = true;
                        s.error = None;
                        s.task = cx.spawn_in(w, async move |this, wx| {
                            let result = wx
                                .background_spawn(async move {
                                    worktrees::Create::prepare(&repo, path, reference, name, &token)
                                })
                                .await;
                            let _ = wx.update(|w, cx| {
                                let ready = this
                                    .update(cx, |s, cx| {
                                        if s.closed || s.token.cancelled() {
                                            return None;
                                        }
                                        s.busy = false;
                                        match result {
                                            Ok(p) => Some(p),
                                            Err(e) => {
                                                s.error = Some(e);
                                                cx.notify();
                                                None
                                            }
                                        }
                                    })
                                    .ok()
                                    .flatten();
                                if let Some(prepared) = ready {
                                    w.close_dialog(cx);
                                    let _ = owner.update(cx, |s, cx| {
                                        if s.repo.path == repo_path {
                                            s.create_worktree(prepared, w, cx);
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
    }
    fn create_worktree(
        &mut self,
        prepared: worktrees::Create,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Worktree,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        let path = prepared.path.clone();
        self.status_message = Some(t("Creating worktree…").into());
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    worktrees::create(&repo, &prepared, &token, &Default::default())
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_operation(id, cx).is_none() {
                    return;
                }
                s.finish_worktree(result, Some(path), w, cx);
            });
        });
        cx.notify();
    }
}
