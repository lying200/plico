//! General Git Tag dialog, separate from the Log's single-name prompt.
//! Rebased 4aec352 GitTagDialog (JetBrains contributors, Apache-2.0).
use super::*;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Textarea, TextareaState};
struct Draft {
    repo: Repository,
    branch: String,
    name: Entity<InputState>,
    source: Entity<InputState>,
    message: Entity<TextareaState>,
    existing: Vec<String>,
    force: bool,
    busy: bool,
    closed: bool,
    invalid_commit: bool,
    error: Option<String>,
    validation_token: Cancel,
    validation_task: Task<()>,
    operation_token: Option<Cancel>,
    _subscriptions: Vec<Subscription>,
}
impl Drop for Draft {
    fn drop(&mut self) {
        self.validation_token.cancel();
    }
}
impl Draft {
    fn duplicate(&self, cx: &App) -> bool {
        self.existing
            .iter()
            .any(|n| n == self.name.read(cx).value().as_ref())
    }
    fn valid(&self, cx: &App) -> bool {
        !self.busy
            && !self.closed
            && !self.invalid_commit
            && !self.name.read(cx).value().is_empty()
            && (!self.duplicate(cx) || self.force)
    }
    fn validate(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.validation_token.cancel();
        let token = Cancel::default();
        self.validation_token = token.clone();
        let repo = self.repo.clone();
        let source = self.source.read(cx).value().to_string();
        self.error = None;
        self.validation_task = cx.spawn_in(w, async move |this, cx| {
            let worker_token = token.clone();
            let result = cx
                .background_spawn(async move {
                    plico::tag_create::preview(&repo, &source, &worker_token)
                })
                .await;
            let _ = this.update_in(cx, |s, w, cx| {
                if token.cancelled() || s.closed {
                    return;
                }
                s.invalid_commit = result.is_err();
                match result {
                    Ok(preview) => super::tag_details::show(s.repo.clone(), preview, w, cx),
                    Err(_) => s.error = Some(t("Invalid commit").into()),
                }
                cx.notify();
            });
        });
    }
}
fn field(label: &'static str, element: impl IntoElement) -> impl IntoElement {
    h_flex()
        .gap_3()
        .items_start()
        .child(div().w(px(116.)).pt_1().flex_shrink_0().child(t(label)))
        .child(div().flex_1().min_w_0().child(element))
}
impl Render for Draft {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let duplicate = self.duplicate(cx);
        let error = if duplicate && !self.force {
            Some(t("A tag with this name already exists").to_owned())
        } else {
            self.error.clone()
        };
        v_flex()
            .gap_3()
            .min_w_0()
            .child(field(
                "Git Root:",
                div().truncate().child(self.repo.path.display().to_string()),
            ))
            .child(field("Current Branch:", self.branch.clone()))
            .child(field(
                "Tag Name:",
                Input::new(&self.name)
                    .external_focus_ring(false)
                    .disabled(self.busy)
                    .w_full(),
            ))
            .child(field(
                "",
                Checkbox::new("tag-force")
                    .label(t("Force"))
                    .checked(self.force && duplicate)
                    .disabled(self.busy || !duplicate)
                    .on_click(cx.listener(|s, checked, _, cx| {
                        s.force = *checked;
                        cx.notify();
                    })),
            ))
            .child(field(
                "Commit:",
                h_flex()
                    .gap_3()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.source)
                                .external_focus_ring(false)
                                .disabled(self.busy)
                                .w_full(),
                        ),
                    )
                    .child(
                        Button::new("validate-tag-commit")
                            .label(t("Validate"))
                            .disabled(self.busy || self.source.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(|s, _, w, cx| s.validate(w, cx))),
                    ),
            ))
            .child(field(
                "Message:",
                Textarea::new(&self.message)
                    .disabled(self.busy)
                    .external_focus_ring(false)
                    .h(px(110.)),
            ))
            .when_some(error, |v, e| {
                v.child(
                    div()
                        .whitespace_normal()
                        .text_color(rgb(crate::ui_theme::color(cx, 0xf2a6a6, 0xc54e58)))
                        .child(e),
                )
            })
    }
}
impl Review {
    pub(super) fn open_tag(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.operation.saving_documents() || self.welcome {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let branch = snapshot
            .current_branch
            .clone()
            .unwrap_or_else(|| t("Detached HEAD").into());
        let existing = snapshot
            .tags
            .iter()
            .map(|r| r.name.trim_start_matches("refs/tags/").to_owned())
            .collect();
        let repo = self.repo.clone();
        self.dismiss_repository_menu(cx);
        let draft = cx.new(|cx| {
            let name = cx.new(|cx| InputState::new(w, cx));
            let source = cx.new(|cx| InputState::new(w, cx));
            let message = cx.new(|cx| TextareaState::new(w, cx));
            let a = cx.subscribe(&name, |s: &mut Draft, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    if !s.duplicate(cx) {
                        s.force = false;
                    }
                    s.error = None;
                    cx.notify();
                }
            });
            let b = cx.subscribe(&source, |s: &mut Draft, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    s.validation_token.cancel();
                    s.invalid_commit = false;
                    s.error = None;
                    cx.notify();
                }
            });
            let c = cx.observe(&message, |_, _, cx| cx.notify());
            Draft {
                repo,
                branch,
                name,
                source,
                message,
                existing,
                force: false,
                busy: false,
                closed: false,
                invalid_commit: false,
                error: None,
                validation_token: Cancel::default(),
                validation_task: Task::ready(()),
                operation_token: None,
                _subscriptions: vec![a, b, c],
            }
        });
        let focus = draft.read(cx).name.read(cx).focus_handle(cx);
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        w.open_dialog(cx, move |dialog, _, cx| {
            let close = draft.clone();
            let confirm = draft.clone();
            let owner = owner.clone();
            let path = path.clone();
            dialog
                .title(t("Tag"))
                .w(px(720.))
                .centered(true)
                .overlay_closable(false)
                .child(draft.clone())
                .footer(crate::dialog_style::confirmation(
                    Button::new("create-general-tag")
                        .primary()
                        .label(t("Create Tag"))
                        .disabled(!draft.read(cx).valid(cx)),
                ))
                .on_ok(move |_, w, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path && confirm.read(cx).valid(cx) {
                            s.create_general_tag(confirm.clone(), w, cx);
                        }
                    });
                    false
                })
                .on_close(move |_, _, cx| {
                    close.update(cx, |s, _| {
                        s.closed = true;
                        s.validation_token.cancel();
                        if let Some(token) = &s.operation_token {
                            token.cancel();
                        }
                    })
                })
        });
        focus.focus(w, cx);
    }
    fn create_general_tag(&mut self, draft: Entity<Draft>, w: &mut Window, cx: &mut Context<Self>) {
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Tag,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let (name, source, message, force) = draft.update(cx, |s, cx| {
            s.busy = true;
            s.error = None;
            s.validation_token.cancel();
            s.operation_token = Some(token.clone());
            cx.notify();
            (
                s.name.read(cx).value().to_string(),
                s.source.read(cx).value().to_string(),
                s.message.read(cx).value().to_string(),
                s.force && s.duplicate(cx),
            )
        });
        let repo = self.repo.clone();
        self.status_message = Some(t("Creating tag…").into());
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    let prepared = plico::tag_create::Prepared::read_options(
                        &repo, &name, &source, &message, force, &token,
                    )?;
                    let out =
                        plico::tag_create::execute(&repo, &prepared, &token, &Default::default())?;
                    if out.status.success() && out.stopped.is_none() && out.input_error.is_none() {
                        Ok(())
                    } else {
                        Err(String::from_utf8_lossy(&out.stderr.bytes()).into_owned())
                    }
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(operation, cx) else {
                    return;
                };
                let error = if cancelled {
                    Some(t("Operation cancelled; repository refreshed").to_owned())
                } else {
                    result.err()
                };
                draft.update(cx, |d, cx| {
                    d.busy = false;
                    d.error = error.clone();
                    cx.notify();
                });
                s.status_message = Some(
                    t(if error.is_none() {
                        "Tag created"
                    } else {
                        "Could not create tag"
                    })
                    .into(),
                );
                if error.is_none() && !draft.read(cx).closed {
                    w.close_dialog(cx);
                }
                s.refresh_impl(false, true, None, cx);
                cx.notify();
            });
        });
    }
}
