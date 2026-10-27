//! Main-window remote coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn open_push(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Push,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let worker_repo = repo.clone();
            let result = wx.background_spawn(async move { worker_repo.push_snapshot(&token) }).await;
            let _ = wx.update(|window, cx| { let _ = this.update(cx, |s, cx| {
                let Some(cancelled) = s.finish_operation(operation, cx) else { return; };
                if cancelled { cx.notify(); return; }
                match result {
                    Ok(Some(snapshot)) => {
                        let draft = cx.new(|cx| crate::push_dialog::PushDialog::new(repo.clone(), snapshot, window, cx));
                        cx.subscribe_in(&draft, window, |s, draft, _: &crate::push_dialog::DefineRemote, w, cx| {
                            s.define_push_remote(draft.clone(), w, cx);
                        }).detach();
                        let callback=s.diff_source_callback(cx);draft.update(cx,|s,_|s.source_callback=Some(callback));
                        cx.observe_in(&draft, window, |_, _, w, _| w.refresh()).detach();
                        let owner = cx.entity().downgrade(); let path = repo.path.clone();
                        window.open_dialog(cx, move |dialog, window, cx| {
                            let control_height = text_row_height(28., 10., cx);
                            let dialog_height = px(540.).min((window.viewport_size().height - px(48.)).max(px(0.)));
                            let owner = owner.clone(); let confirm = draft.clone(); let path = path.clone(); let closing = draft.clone();
                            dialog.title(h_flex().gap_2().items_center().child(gpui_kit::img("branding/plico.svg").size(px(16.))).child(div().font_weight(FontWeight::NORMAL).child(format!("{} {}", t("Push Commits to"), repo.path.file_name().unwrap_or_default().to_string_lossy())))).w(px(800.)).h(dialog_height).pb_0().centered(true).overlay_closable(false)
                                .content({ let draft = draft.clone(); move |body, _, _| body.p_0().min_h_0().overflow_hidden().child(draft.clone()) })
                                .footer({
                                    let normal_owner = owner.clone(); let normal_draft = draft.clone(); let normal_path = path.clone();
                                    let force_owner = owner.clone(); let force_draft = draft.clone(); let force_path = path.clone();
                                    let can_force = draft.read(cx).spec().is_some_and(|s| plico::push::force_allowed(&s.target));
                                    let tags_draft = draft.clone(); let mode_draft = draft.clone();
                                    DialogFooter::new().items_center().flex_wrap().gap(px(12.)).mx(px(-16.)).px(px(12.)).py(px(8.)).border_t_1().border_color(rgb(0x26282c))
                                        .child(Button::new("push-help").ghost().label("?").accessibility_label(t("Push help")).size(control_height).with_size(control_height).p_0().rounded(control_height / 2.).border_1().border_color(rgb(0x40434a)).mr(px(8.)).on_click(|_,w,cx| {
                                            w.open_dialog(cx,|d,_,_|d.title(t("Push")).w(px(460.)).child(t("Push selected commits to the target branch. Enable Push tags to include tags. Force Push checks the last known remote revision before replacing history.")).footer(DialogFooter::new().child(gpui_kit::component::dialog::DialogClose::new().trigger(|b|b.label(t("Close"))))));
                                        }))
                                        .child(h_flex().items_center().gap(px(8.))
                                        .child(gpui_kit::component::checkbox::Checkbox::new("push-tags").label(t("Push tags:"))
                                            .checked(draft.read(cx).push_tags).on_click(move|checked,_,cx|tags_draft.update(cx,|d,cx|{d.push_tags=*checked;cx.notify();})))
                                        .child(Button::new("push-tags-mode").w(rems(11.)).h(control_height).with_size(control_height).dropdown_caret(true).rounded(px(4.)).label(t(match draft.read(cx).tag_mode { plico::push::TagMode::All=>"All",plico::push::TagMode::CurrentBranch=>"Current Branch" }))
                                            .border_color(rgb(if draft.read(cx).push_tags {0x40434a}else{0x33353b})).bg(rgb(if draft.read(cx).push_tags {0x26282c}else{0x191a1c})).text_color(rgb(if draft.read(cx).push_tags {FG}else{0x4c4f56})).disabled(!draft.read(cx).push_tags).dropdown_menu(move|mut menu,_,_|{
                                                for (mode,label) in [(plico::push::TagMode::All,"All"),(plico::push::TagMode::CurrentBranch,"Current Branch")] {
                                                    let draft=mode_draft.clone();menu=menu.item(PopupMenuItem::new(t(label)).on_click(move|_,_,cx|draft.update(cx,|d,cx|{d.tag_mode=mode;cx.notify();})));
                                                }menu
                                            })))
                                        .child(div().flex_1())
                                        .child(DropdownButton::new("push-actions").with_size(control_height).separator_color(rgba(0xffffff80).into())
                                            .button(Button::new("push-confirm").primary().h(control_height).min_w(px(72.)).font_weight(FontWeight::SEMIBOLD).label(t("Push"))
                                                .on_click(move |_, w, cx| {
                                                    if let Some(spec) = normal_draft.read(cx).spec() {
                                                        let _ = normal_owner.update(cx, |s, cx| { if s.repo.path == normal_path { w.close_dialog(cx); s.start_push(spec, w, cx); } });
                                                    }
                                                }))
                                            .disabled(draft.read(cx).spec().is_none())
                                            .menu_accessibility_label(t("Push options"))
                                            .dropdown_menu(move |menu, _, _| {
                                                let owner=force_owner.clone(); let draft=force_draft.clone(); let path=force_path.clone();
                                                menu.item(PopupMenuItem::new(t("Force Push…"))
                                                    .disabled(!can_force).on_click(move |_, w, cx| {
                                                        let Some(mut spec) = draft.read(cx).spec() else { return; };
                                                        if !plico::push::force_allowed(&spec.target) { return; }
                                                        spec.force = true;
                                                        let _ = owner.update(cx, |s, cx| { if s.repo.path == path { s.confirm_force_push(spec, w, cx); } });
                                                    }))
                                                    .when(!can_force, |m| m.item(PopupMenuItem::new(t("Force push is prohibited for this protected branch")).disabled(true))).select_first()
                                            }))
                                        .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.label(t("Cancel")).min_w(px(72.)).h(control_height).with_size(control_height).rounded(px(4.))))
                                })
                                .on_close(move |_, _, cx| closing.update(cx, |s, _| s.close()))
                                .on_ok(move |_, window, cx| {
                                    let Some(spec) = confirm.read(cx).spec() else { return false; };
                                    let _ = owner.update(cx, |s, cx| { if s.repo.path == path { s.start_push(spec, window, cx); } }); true
                                })
                        });
                    }
                    Ok(None) => s.status_message = Some(t("Check out a local branch with commits before pushing").into()),
                    Err(error) => {
                        // A repository/configuration failure is not a detached HEAD.
                        // Keep the diagnostic available through the shared Details UI.
                        s.status_message = None;
                        s.error = Some(error);
                        // A refresh can replace the shared status; keep a failed
                        // explicit Push request visible until the user closes it.
                        s.show_error_details(window, cx);
                    }
                }
                cx.notify();
            }); });
        });
        cx.notify();
    }
    pub(super) fn open_project_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.settings_open || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Update,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Preparing project update…").into());
        let repo = self.repo.clone();
        let path = repo.path.clone();
        let project_roots = self.project_roots.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            // Opening Update during initial discovery must wait for the project
            // inventory rather than silently preparing only the primary root.
            let registered = loop {
                if token.cancelled() {
                    break Err("Cancelled".to_owned());
                }
                match wx.update(|_, cx| project_roots.read(cx).snapshot()) {
                    Ok(Some(Ok(inventory))) => {
                        if let Some((path, error)) = inventory.errors.first() {
                            break Err(format!("{}: {error}", path.display()));
                        }
                        break Ok(inventory.roots.clone());
                    }
                    Ok(Some(Err(error))) => break Err(error),
                    Err(error) => break Err(error.to_string()),
                    Ok(None) => {}
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            };
            let result = wx
                .background_spawn(async move {
                    let request =
                        plico::project_update::Prepared::read_registered(&registered?, &token)?;
                    let preferences =
                        plico::project_update_preferences::Preferences::for_repository(
                            &repo, &token,
                        )?;
                    Ok::<_, String>((request, preferences))
                })
                .await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    s.status_message = None;
                    if cancelled || s.repo.path != path {
                        cx.notify();
                        return;
                    }
                    match result {
                        Ok((request, preferences)) => {
                            if !preferences.show_options {
                                s.start_project_update(request, preferences.method, window, cx);
                                return;
                            }
                            let draft = cx.new(|_| {
                                crate::project_update_dialog::Options::new(request, preferences)
                            });
                            cx.observe_in(&draft, window, |_, _, w, _| w.refresh())
                                .detach();
                            let owner = cx.entity().downgrade();
                            window.open_dialog(cx, move |dialog, _, cx| {
                                let owner = owner.clone();
                                let draft = draft.clone();
                                let path = path.clone();
                                let checkbox_draft = draft.clone();
                                dialog
                                    .title(t("Update Project"))
                                    .w(px(560.))
                                    .centered(true)
                                    .overlay_closable(false)
                                    .child(draft.clone())
                                    .footer(
                                        gpui_kit::component::dialog::DialogFooter::new()
                                            .child(
                                                gpui_kit::component::checkbox::Checkbox::new(
                                                    "hide-update-options",
                                                )
                                                .label(t("Don't show again"))
                                                .checked(!draft.read(cx).show_options)
                                                .on_click(move |checked, _, cx| {
                                                    checkbox_draft.update(cx, |d, cx| {
                                                        d.show_options = !*checked;
                                                        cx.notify();
                                                    })
                                                }),
                                            )
                                            .child(div().flex_1())
                                            .child(
                                                gpui_kit::component::dialog::DialogAction::new()
                                                    .child(
                                                        Button::new("update-project-ok")
                                                            .primary()
                                                            .min_w(px(72.))
                                                            .label(t("OK")),
                                                    ),
                                            )
                                            .child(
                                                gpui_kit::component::dialog::DialogClose::new()
                                                    .trigger(|b| {
                                                        b.label(t("Cancel")).min_w(px(72.))
                                                    }),
                                            ),
                                    )
                                    .on_ok(move |_, window, cx| {
                                        owner
                                            .update(cx, |s, cx| {
                                                if s.repo.path != path || s.operation.busy() {
                                                    return false;
                                                }
                                                if !draft.update(cx, |d, cx| d.save(cx)) {
                                                    return false;
                                                }
                                                let method = draft.read(cx).method;
                                                let request =
                                                    draft.update(cx, |d, _| d.request.take());
                                                if let Some(request) = request {
                                                    s.start_project_update(
                                                        request, method, window, cx,
                                                    );
                                                }
                                                true
                                            })
                                            .unwrap_or(true)
                                    })
                            });
                        }
                        Err(error) => {
                            s.error = Some(error);
                            s.status_message = Some(t("Cannot update project").into());
                        }
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn start_project_update(
        &mut self,
        request: plico::project_update::Prepared,
        method: plico::update_method::Method,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_project_update(request, method, w, cx)
            });
            return;
        }
        if self.operation.busy() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let helper = exe.with_file_name(if cfg!(windows) {
            "plico-askpass.exe"
        } else {
            "plico-askpass"
        });
        if !helper.is_file() {
            self.status_message = Some(t("Authentication helper is unavailable").into());
            cx.notify();
            return;
        }
        let editor_helper = exe.with_file_name(if cfg!(windows) {
            "plico-editor.exe"
        } else {
            "plico-editor"
        });
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Update,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Updating project…").into());
        let requests = plico::auth_request::Requests::persistent(token.clone());
        requests.set_project(&self.repo.path);
        let worker_requests = requests.clone();
        let (mut choices, choice_ui) = plico::update_decision::channel(token.clone());
        let (conflict_worker, conflict_ui) = plico::project_update::conflicts::channel::channel();
        let directories = request.editor_directories;
        let group = request.group;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = (|| -> Result<_, String> {
                let editor =
                    plico::editor::command(&editor_helper, false).map_err(|e| e.to_string())?;
                use plico::push_flow::Decisions as _;
                let choices = std::cell::RefCell::new(&mut choices);
                group.execute_with_conflicts(
                    method,
                    &editor,
                    &token,
                    &Default::default(),
                    Some((&worker_requests, &helper)),
                    plico::published_commits::Protection::Defaults,
                    |problem| choices.borrow_mut().rebase_over_merge(problem),
                    |problem| choices.borrow_mut().published(problem),
                    |interaction| conflict_worker.resolve(interaction),
                )
            })();
            let _ = tx.send(result);
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            let mut decision: Option<Entity<crate::update_dialog::UpdateDialog>> = None;
            loop {
                if let Some(pending) = conflict_ui.take_pending() {
                    let _ = this.update_in(wx, |s, w, cx| s.open_update_conflicts(pending, w, cx));
                }
                if let Some(pending) = choice_ui.take_pending() {
                    if let Ok(dialog) =
                        wx.update(|w, cx| crate::update_dialog::show(pending, w, cx))
                    {
                        decision = Some(dialog);
                    }
                }
                if let Some(pending) = requests.take_pending() {
                    use plico::auth_request::Kind;
                    if matches!(pending.kind, Kind::CommitEditor | Kind::SequenceEditor) {
                        {
                            let directories = directories.clone();
                            if let Ok(document) = wx
                                .background_spawn(async move {
                                    plico::editor::Document::open_in_roots(pending, &directories)
                                })
                                .await
                            {
                                if let Ok(dialog) =
                                    wx.update(|w, cx| crate::editor_dialog::show(document, w, cx))
                                {
                                    editor = Some(dialog);
                                }
                            }
                        }
                    } else if let Ok(dialog) =
                        wx.update(|w, cx| crate::auth_dialog::show(pending, w, cx))
                    {
                        login = Some(dialog);
                    }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Update worker ended".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = wx.update(|w, cx| {
                        if login.as_ref().is_some_and(|d| d.read(cx).active())
                            || editor.as_ref().is_some_and(|d| d.read(cx).active())
                            || decision.as_ref().is_some_and(|d| d.read(cx).active())
                        {
                            w.close_dialog(cx);
                        }
                        let _ = this.update(cx, |s, cx| {
                            let Some(cancelled) = s.finish_operation(operation, cx) else {
                                return;
                            };
                            s.clear_update_conflicts(w, cx);
                            s.saved_changes_available |=
                                result.as_ref().ok().is_some_and(|r| r.has_saved_changes());
                            let message =
                                if cancelled || result.as_ref().is_ok_and(|r| r.cancelled()) {
                                    "Update cancelled; review local changes and saved stash"
                                } else if result.as_ref().is_ok_and(|r| r.complete()) {
                                    if result.as_ref().is_ok_and(|r| {
                                        r.result()
                                            == plico::update_session::UpdateResult::NothingToUpdate
                                    }) {
                                        "Already up to date"
                                    } else {
                                        "Project updated"
                                    }
                                } else {
                                    "Update incomplete; review conflicts and saved local changes"
                                };
                            s.status_message = Some(t(message).into());
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            s.refresh_after_project_update(
                                plico::project_update::group::diagnostic(&result),
                                cx,
                            );
                            s.update_diagnostic_warning =
                                result.as_ref().is_ok_and(|report| report.complete());
                        });
                    });
                    break;
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            }
        });
        cx.notify();
    }
    fn refresh_after_project_update(&mut self, diagnostic: Option<String>, cx: &mut Context<Self>) {
        // Operation diagnostics outlive query refreshes and selection changes.
        self.refresh_impl(false, true, None, cx);
        self.update_diagnostic = diagnostic;
        self.update_diagnostic_warning = false;
        cx.notify();
    }
    pub(super) fn confirm_force_push(
        &mut self,
        spec: plico::push::Spec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !plico::push::force_allowed(&spec.target) || self.operation.busy() {
            return;
        }
        if layout_preferences::silent_force_push(
            &self.repo.path,
            &spec.target.remote,
            &spec.target.branch,
        ) {
            window.close_dialog(cx);
            self.start_push(spec, window, cx);
            return;
        }
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        let remember = cx.new(|_| false);
        cx.observe_in(&remember, window, |_, _, w, _| w.refresh())
            .detach();
        window.open_dialog(cx, move |dialog, _, cx| {
            let owner = owner.clone(); let path = path.clone(); let spec = plico::push::Spec {
                tags: spec.tags, force: true, source_branch: spec.source_branch.clone(), source_oid: spec.source_oid.clone(),
                target: spec.target.clone(), set_upstream: spec.set_upstream,
            };
            let checked=remember.clone(); let saved=remember.clone();
            dialog.title(h_flex().gap_3().items_center().child(gpui_kit::img("jb/warning_dark.svg").size(px(32.))).child(t("Force Push")))
                .bg(rgb(0x26282b)).w(px(520.)).centered(true).overlay_closable(false)
                .child(v_flex().pl(px(44.)).gap_4().min_w_0()
                    .child(format!("{} '{}'. {}",t("You're going to force push to"),spec.target.branch,
                        t("It may overwrite commits at the remote. Are you sure you want to proceed?")))
                    .child(gpui_kit::component::checkbox::Checkbox::new("remember-force-target").focus_ring(false)
                        .label(t("Don't warn about this target")).checked(*remember.read(cx))
                        .on_click(move|value,_,cx|checked.update(cx,|s,cx|{*s=*value;cx.notify();}))))
                .footer(crate::dialog_style::confirmation(Button::new("force-push-confirm").primary().label(t("Force Push"))))
                .on_ok(move |_, w, cx| owner.update(cx, |s, cx| {
                    if s.repo.path != path || s.operation.busy() { return false; }
                    if *saved.read(cx) {
                        if let Err(error)=layout_preferences::remember_force_push(&path,&spec.target.remote,&spec.target.branch) {
                            crate::error_ui::show_details(error.to_string(),"Settings could not be saved",w,cx);return false;
                        }
                    }
                    w.close_all_dialogs(cx); s.start_push(plico::push::Spec { tags: spec.tags, force: true, source_branch: spec.source_branch.clone(), source_oid: spec.source_oid.clone(), target: spec.target.clone(), set_upstream: spec.set_upstream }, w, cx); true
                }).unwrap_or(true))
        });
    }
    pub(super) fn start_push(
        &mut self,
        spec: plico::push::Spec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| s.start_push(spec, w, cx));
            return;
        }
        if self.operation.busy() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let helper = exe.with_file_name(if cfg!(windows) {
            "plico-askpass.exe"
        } else {
            "plico-askpass"
        });
        if !helper.is_file() {
            self.status_message = Some(t("Authentication helper is unavailable").into());
            cx.notify();
            return;
        }
        let editor_helper = exe.with_file_name(if cfg!(windows) {
            "plico-editor.exe"
        } else {
            "plico-editor"
        });
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Push,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Pushing…").into());
        let requests = plico::auth_request::Requests::persistent(token.clone());
        requests.set_project(&self.repo.path);
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let (mut choices, choice_ui) = plico::update_decision::channel(token.clone());
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = (|| -> Result<_, String> {
                let editor =
                    plico::editor::command(&editor_helper, false).map_err(|e| e.to_string())?;
                let (path, preferences) = plico::update_preferences::for_repository(&repo, &token)?;
                choices.set_preferences(path, preferences);
                let mut report = plico::push_flow::execute(
                    &repo,
                    spec,
                    &editor,
                    &token,
                    &Default::default(),
                    Some((&worker_requests, &helper)),
                    preferences,
                    plico::published_commits::Protection::Defaults,
                    &mut choices,
                );
                report.preferences_error = choices.persistence_error.clone();
                Ok(report)
            })();
            let _ = tx.send(result);
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            let mut decision: Option<Entity<crate::update_dialog::UpdateDialog>> = None;
            let directory = wx.background_spawn(async move { directory_repo.editor_directory(&Cancel::default()) }).await;
            loop {
                if let Some(pending) = choice_ui.take_pending() {
                    if let Ok(dialog) = wx.update(|w, cx| crate::update_dialog::show(pending, w, cx)) { decision = Some(dialog); }
                }
                if let Some(pending) = requests.take_pending() {
                    use plico::auth_request::Kind;
                    if matches!(pending.kind, Kind::CommitEditor | Kind::SequenceEditor) {
                        if let Ok(dir) = &directory {
                            let dir = dir.clone();
                            if let Ok(document) = wx.background_spawn(async move { plico::editor::Document::open(pending, &dir) }).await {
                                if let Ok(dialog) = wx.update(|w, cx| crate::editor_dialog::show(document, w, cx)) { editor = Some(dialog); }
                            }
                        }
                    } else if let Ok(dialog) = wx.update(|w, cx| crate::auth_dialog::show(pending, w, cx)) { login = Some(dialog); }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("Push worker ended".into())),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = wx.update(|w, cx| {
                        if login.as_ref().is_some_and(|d| d.read(cx).active()) || editor.as_ref().is_some_and(|d| d.read(cx).active()) || decision.as_ref().is_some_and(|d| d.read(cx).active()) { w.close_dialog(cx); }
                        let _ = this.update(cx, |s, cx| {
                            use plico::push::Status;
                            let Some(cancelled) = s.finish_operation(operation, cx) else { return; };
                            let preferences_error = result.as_ref().ok().and_then(|r| r.preferences_error.clone());
                            s.saved_changes_available |= result.as_ref().ok().and_then(|r| r.update.as_ref()).is_some_and(|u| u.has_saved_changes());
                            let message = if cancelled { "Push cancelled; remote changes may have completed" }
                            else if result.as_ref().is_ok_and(|r| r.cancelled_decision) { "Push update cancelled" }
                            else if let Some(update) = result.as_ref().ok().and_then(|r| r.update.as_ref()).filter(|u| u.cancelled()) {
                                if update.has_saved_changes() { "Push update cancelled; local changes remain saved in stash" }
                                else { "Push update cancelled" }
                            }
                            else if result.as_ref().is_ok_and(|r| r.update.as_ref().is_some_and(|u| !u.ready())) { "Update incomplete; review conflicts and saved local changes before pushing" }
                            else {
                                match result.and_then(|r| {
                                    if let Some(error) = r.error { return Err(error); }
                                    if r.cancelled_decision { return Err("Update cancelled".into()); }
                                    r.push.map(|p| (p.status, p.state.is_ok())).ok_or("Push did not start".into())
                                }) {
                                    Ok((Status::Success | Status::Forced | Status::NewBranch, true)) => "Push completed",
                                    Ok((Status::UpToDate, true)) => "Already up to date",
                                    Ok((Status::RejectedNonFf, _)) => "Push rejected: remote history has changed",
                                    Ok((Status::RejectedStale, _)) => "Push rejected: stale remote information",
                                    Ok((Status::RejectedOther, _)) => "Push rejected by remote",
                                    Ok((Status::Cancelled, _)) => "Push cancelled; remote changes may have completed",
                                    _ => "Push failed or incomplete; some remote updates may have completed",
                                }
                            };
                            s.status_message = Some(if preferences_error.is_some() {
                                format!("{} · {}", t(message), t("Settings could not be saved"))
                            } else { t(message).into() });
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            s.refresh_impl(false, true, None, cx); cx.notify();
                        });
                    }); break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
        });
        cx.notify();
    }
    pub(super) fn open_pull(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.welcome {
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Pull,
            plico::active_operation::Phase::Preparing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Loading pull targets…").into());
        let repo = self.repo.clone();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    Ok::<_, String>((
                        repo.pull_targets(&token)?,
                        repo.editor_directory(&token)?,
                        plico::pull::source(&repo, &token)?,
                    ))
                })
                .await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    let Some(cancelled) = s.finish_operation(operation, cx) else {
                        return;
                    };
                    s.status_message = None;
                    if !cancelled {
                        match result {
                            Ok((targets, directory, source)) => {
                                s.show_pull(targets, directory, source, window, cx)
                            }
                            Err(_) => {
                                s.status_message = Some(t("Could not read pull targets").into())
                            }
                        }
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    pub(super) fn show_pull(
        &mut self,
        targets: plico::pull::Targets,
        directory: PathBuf,
        source: Option<plico::pull::Source>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft =
            cx.new(|cx| crate::pull_dialog::PullDialog::new(targets, source.is_some(), window, cx));
        cx.observe_in(&draft, window, |_, _, w, _| w.refresh())
            .detach();
        let owner = cx.entity().downgrade();
        let repo_path = self.repo.path.clone();
        let focus = draft.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let draft_confirm = draft.clone();
            let owner = owner.clone();
            let directory = directory.clone();
            let repo_path = repo_path.clone();
            let source = source.clone();
            dialog
                .title(t("Pull"))
                .w(px(620.))
                .overlay_closable(false)
                .centered(true)
                .child(draft.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(draft.update(cx, |s, cx| s.options_button(cx)))
                        .child(div().flex_1())
                        .child(crate::dialog_style::confirmation(
                            Button::new("pull-confirm")
                                .primary()
                                .label(t("Pull"))
                                .disabled(draft.read(cx).selection(cx).is_none()),
                        )),
                )
                .on_ok(move |_, window, cx| {
                    let Some((target, options)) = draft_confirm.read(cx).selection(cx) else {
                        return false;
                    };
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == repo_path {
                            s.start_pull(
                                target,
                                options,
                                directory.clone(),
                                source.clone(),
                                window,
                                cx,
                            );
                        }
                    });
                    true
                })
        });
        focus.update(cx, |s, cx| s.focus(window, cx));
    }
    pub(super) fn start_pull(
        &mut self,
        target: plico::pull::Target,
        options: plico::pull::Options,
        directory: PathBuf,
        source: Option<plico::pull::Source>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.start_pull(target, options, directory, source, w, cx)
            });
            return;
        }
        if self.operation.busy() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let helper = exe.with_file_name(if cfg!(windows) {
            "plico-askpass.exe"
        } else {
            "plico-askpass"
        });
        let editor = exe.with_file_name(if cfg!(windows) {
            "plico-editor.exe"
        } else {
            "plico-editor"
        });
        let (Ok(commit_editor), Ok(sequence_editor)) = (
            plico::editor::command(&editor, false),
            plico::editor::command(&editor, true),
        ) else {
            self.status_message = Some(t("Git editor helper is unavailable").into());
            cx.notify();
            return;
        };
        if !helper.is_file() {
            self.status_message = Some(t("Authentication helper is unavailable").into());
            cx.notify();
            return;
        }
        let spec = plico::pull::CommandSpec {
            target,
            options,
            commit_editor,
            sequence_editor: Some(sequence_editor),
        };
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Pull,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(t("Pulling…").into());
        let requests = plico::auth_request::Requests::persistent(token.clone());
        requests.set_project(&self.repo.path);
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        enum PullResult {
            Command(plico::pull::CommandResult),
            Rebase(Box<plico::rebase_update::Report>),
        }
        let (choices, choice_ui) = plico::update_decision::channel(token.clone());
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = (|| -> Result<PullResult, String> {
                if !spec.options.contains(plico::pull::OptionFlag::Rebase) {
                    return plico::pull::execute_command(
                        &repo,
                        &spec,
                        &token,
                        &Default::default(),
                        Some((&worker_requests, &helper)),
                    )
                    .map(PullResult::Command);
                }
                let source = source.ok_or("Explicit Rebase requires an attached branch")?;
                let request = plico::merge_update::Request {
                    branch: source.branch,
                    head: source.head,
                    target: spec.target,
                    reinstate_index: true,
                };
                use plico::{push_flow::Decisions, update_method::Method};
                let choices = std::cell::RefCell::new(choices);
                plico::rebase_update::execute_selected(
                    &repo,
                    &request,
                    &spec.commit_editor,
                    &token,
                    &Default::default(),
                    Some((&worker_requests, &helper)),
                    plico::published_commits::Protection::Defaults,
                    |problem| {
                        if matches!(problem, Ok(false)) {
                            Ok(Some(Method::Rebase))
                        } else {
                            choices.borrow_mut().rebase_over_merge(problem)
                        }
                    },
                    |problem| choices.borrow_mut().published(problem),
                )
                .map(|report| PullResult::Rebase(Box::new(report)))
            })();
            let _ = tx.send(result);
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let mut interaction = super::git_interaction::Session::new(operation, this.clone(), requests, vec![directory.clone()]).with_authentication();
            loop {
                if let Some(pending) = choice_ui.take_pending() {
                    interaction.show_decision(pending, wx);
                }
                interaction.poll(wx).await;
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("Pull worker ended".into())),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = wx.update(|w, cx| {
                        let _ = this.update(cx, |s, cx| {
                            let Some(cancelled) = interaction.finish(s, w, cx) else { return; };
                            let saved = matches!(&result, Ok(PullResult::Rebase(report)) if report.saved_oid.is_some());
                            s.saved_changes_available |= saved;
                            let message = if cancelled {
                                if saved { "Pull cancelled; local changes remain saved in stash" } else { "Pull cancelled" }
                            } else {
                                match result {
                                    Ok(PullResult::Rebase(report)) => {
                                        use plico::update_session::UpdateResult;
                                        if report.result == UpdateResult::Cancelled {
                                            if saved { "Pull cancelled; local changes remain saved in stash" } else { "Pull cancelled" }
                                        } else if report.state.as_ref().is_ok_and(|s| !s.conflicts.is_empty()) {
                                            "Pull requires conflict resolution"
                                        } else if report.state.as_ref().is_ok_and(|s| s.in_progress()) {
                                            "Pull left an unfinished operation"
                                        } else if saved { "Pull incomplete; local changes remain saved in stash" }
                                        else if report.ready_to_push() { "Pull completed" }
                                        else { "Pull failed" }
                                    }
                                    Ok(PullResult::Command(result)) => match (result.command, result.state) {
                                        (_, Ok(state)) if !state.conflicts.is_empty() => "Pull requires conflict resolution",
                                        (_, Ok(state)) if state.in_progress() => "Pull left an unfinished operation",
                                        (Ok(outcome), Ok(_)) if outcome.status.success() && outcome.stopped.is_none() => "Pull completed",
                                        _ => "Pull failed",
                                    },
                                    Err(_) => "Pull failed",
                                }
                            };
                            s.status_message = Some(t(message).into());
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            s.refresh_impl(false, true, None, cx); cx.notify();
                        });
                    });
                    break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
        });
        cx.notify();
    }
    pub(super) fn confirm_abort_merge_in(
        &mut self,
        repository: Repository,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() {
            return;
        }
        let owner = cx.entity().downgrade();
        let path = repository.path.clone();
        let project = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let path = path.clone();
            let project = project.clone();
            let repository = repository.clone();
            dialog
                .title(t("Abort Merge"))
                .w(px(760.))
                .centered(true)
                .overlay_closable(false)
                .child(div().child(format!(
                    "{} ({})",
                    t("Abort the merge in this repository?"),
                    path.file_name().unwrap_or_default().to_string_lossy()
                )))
                .child(div().truncate().child(project_path_text(&path)))
                .footer(crate::dialog_style::confirmation(
                    Button::new("abort-merge-confirm")
                        .primary()
                        .label(t("Abort")),
                ))
                .on_ok(move |_, window, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == project
                            && s.merge_roots(cx).iter().any(|r| r.path == path)
                        {
                            s.start_merge_operation_in(
                                repository.clone(),
                                plico::merge_operation::Action::Abort,
                                window,
                                cx,
                            );
                        }
                    });
                    true
                })
        });
    }
    pub(super) fn start_fetch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_fetch_named(None, window, cx);
    }
    pub(super) fn start_fetch_named(
        &mut self,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_fetch_request(self.repo.clone(), name, false, window, cx);
    }
    pub(super) fn unshallow_repository(&self, window: &Window, cx: &App) -> Option<Repository> {
        let catalog = self.project_roots.read(cx);
        let members = catalog.snapshot()?.ok()?;
        let mut current = self
            .project_view
            .as_ref()
            .filter(|_| self.project_visible)
            .and_then(|view| view.read(cx).operation_path(window, cx));
        if current.is_none() && self.history_focus.contains_focused(window, cx) {
            let ids = self.commit_ids();
            let selected = self.log_selection.ordered(&ids);
            // Rebased's Log data context uses the visible query roots when no
            // rows (or more than MAX_SELECTED_COMMITS) are selected. Include
            // failed roots: a partial load must not become a single-root context.
            let roots = if selected.is_empty() || selected.len() > 1000 {
                self.current_log_scope()
                    .map(|scope| {
                        scope
                            .roots
                            .iter()
                            .map(|root| root.query.repository.path.clone())
                            .chain(scope.errors.iter().map(|(root, _)| root.clone()))
                            .collect::<std::collections::BTreeSet<_>>()
                    })
                    .unwrap_or_default()
            } else {
                selected
                    .into_iter()
                    .map(|id| id.root.as_ref().clone())
                    .collect()
            };
            if roots.len() == 1 {
                current = roots.first().cloned();
            }
        }
        let editor = self
            .active_file_index()
            .and_then(|i| self.file_tabs.get(i))
            .map(|tab| {
                let tab = tab.read(cx);
                tab.root().join(&tab.path)
            })
            .or_else(|| {
                self.active_diff_index()
                    .and_then(|i| self.tabs.get(i))
                    .map(|tab| {
                        let id = tab.read(cx).target.identity();
                        id.repository.join(&id.path)
                    })
            });
        let repo =
            members.operation_repository(current.as_deref(), editor.as_deref(), &self.repo.path)?;
        catalog.is_shallow(&repo.path).then(|| repo.clone())
    }
    pub(super) fn start_unshallow(
        &mut self,
        repo: Repository,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The menu captures its context; never re-guess after focus moves into it.
        if !self
            .project_roots
            .read(cx)
            .snapshot()
            .is_some_and(|result| {
                result.is_ok_and(|members| {
                    members.roots.iter().any(|member| member.path == repo.path)
                })
            })
        {
            return;
        }
        self.start_fetch_request(repo, None, true, window, cx);
        if self.operation.busy() {
            window.remove_notification::<super::unshallow_notice::ShallowNotification>(cx);
        }
    }
    fn start_fetch_request(
        &mut self,
        repo: Repository,
        name: Option<String>,
        unshallow: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.welcome {
            return;
        }
        let helper = match std::env::current_exe() {
            Ok(path) => path.with_file_name(if cfg!(windows) {
                "plico-askpass.exe"
            } else {
                "plico-askpass"
            }),
            Err(_) => {
                self.status_message = Some(t("Authentication helper is unavailable").into());
                cx.notify();
                return;
            }
        };
        if !helper.is_file() {
            self.status_message = Some(t("Authentication helper is unavailable").into());
            cx.notify();
            return;
        }
        let Some((operation, token)) = self.begin_operation(
            plico::active_operation::Kind::Fetch,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.status_message = Some(
            t(if unshallow {
                "Unshallowing repository…"
            } else {
                "Fetching…"
            })
            .into(),
        );
        let requests = plico::auth_request::Requests::persistent(token.clone());
        requests.set_project(&self.repo.path);
        let worker_requests = requests.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let specs = if unshallow {
                repo.unshallow_spec(&token).map(|spec| vec![spec])
            } else {
                repo.fetch_specs(&token)
            };
            let result = specs.and_then(|mut specs| {
                if let Some(name) = &name {
                    specs.retain(|spec| spec.remote() == name);
                    if specs.is_empty() {
                        return Err("Fetch remote is no longer configured".into());
                    }
                }
                plico::fetch::execute_authenticated(
                    &repo,
                    &specs,
                    &token,
                    &Default::default(),
                    &worker_requests,
                    &helper,
                )
            });
            let _ = tx.send(result);
        })
        .detach();
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            loop {
                if let Some(pending) = requests.take_pending() {
                    match wx.update(|window, cx| {
                        let _ = this.update(cx, |s, cx| {
                            s.dismiss_repository_menu(cx);
                            cx.notify();
                        });
                        crate::auth_dialog::show(pending, window, cx)
                    }) {
                        Ok(dialog) => login = Some(dialog),
                        Err(_) => break,
                    }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Fetch worker ended".into()))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = wx.update(|window, cx| {
                        if login
                            .as_ref()
                            .is_some_and(|dialog| dialog.read(cx).active())
                        {
                            window.close_dialog(cx);
                        }
                        let _ = this.update(cx, |s, cx| {
                            let Some(cancelled) = s.finish_operation(operation, cx) else {
                                return;
                            };
                            s.update_diagnostic = if cancelled {
                                None
                            } else {
                                match &result {
                                    Ok(batch) => batch.diagnostic(),
                                    Err(error) => {
                                        Some(plico::remote_management::display_error(error))
                                    }
                                }
                            };
                            s.update_diagnostic_warning = false;
                            s.status_message = Some(if cancelled {
                                t("Fetch cancelled").into()
                            } else {
                                match result {
                                    Ok(result) if result.remotes.is_empty() => {
                                        t("No remotes configured").into()
                                    }
                                    Ok(result) if result.successful() => t(if unshallow {
                                        "The repository was converted to a complete one"
                                    } else {
                                        "Fetch completed"
                                    })
                                    .into(),
                                    Ok(result) => {
                                        let names = result
                                            .remotes
                                            .iter()
                                            .filter(|r| {
                                                r.state != plico::fetch::RemoteState::Succeeded
                                            })
                                            .map(|r| r.remote.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        format!(
                                            "{}: {names}",
                                            t(if unshallow {
                                                "Failed to unshallow repository"
                                            } else {
                                                "Fetch failed"
                                            })
                                        )
                                    }
                                    Err(_) => t(if unshallow {
                                        "Failed to unshallow repository"
                                    } else {
                                        "Fetch failed"
                                    })
                                    .into(),
                                }
                            });
                            // Fetch may have changed refs even on failure/cancellation.
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            s.refresh_impl(false, true, None, cx);
                            cx.notify();
                        });
                    });
                    break;
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            }
        });
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod update_tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;
    #[gpui::test]
    fn project_update_diagnostic_survives_its_completion_refresh(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let review = review.unwrap();
        review.update(cx, |s, cx| {
            s.refresh_after_project_update(Some("child: update failed".into()), cx);
            s.refresh_impl(false, true, None, cx);
            s.reset_log_presentation();
            assert_eq!(s.update_diagnostic.as_deref(), Some("child: update failed"));
            s.refresh_after_project_update(None, cx);
            assert!(
                s.update_diagnostic.is_none(),
                "successful update must clear the previous result"
            );
        });
    }
}
