//! Main-window repository popup coordination.
//! Rendering adapter; session owns popup state and request lifetimes.
pub(super) mod session;
use super::*;
impl Review {
    pub(super) fn dismiss_repository_child(&mut self, cx: &mut Context<Self>) {
        self.branch_popup.child_selection = None;
        if let Some(menu) = self.branch_popup.child_menu.take() {
            // An action can close its parent while PopupMenu is itself being
            // updated. Defer the event to avoid re-borrowing that entity.
            cx.defer(move |cx| {
                if let Some(menu) = menu.upgrade() {
                    menu.update(cx, |_, cx| cx.emit(DismissEvent));
                }
            });
        }
    }
    pub(super) fn dismiss_repository_menu(&mut self, cx: &mut Context<Self>) {
        self.dismiss_repository_child(cx);
        self.branch_popup.dismiss();
    }
    pub(super) fn load_repository_recents(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = &self.snapshot else {
            self.branch_popup.invalidate();
            return;
        };
        let project_roots = self
            .project_roots
            .read(cx)
            .snapshot()
            .and_then(Result::ok)
            .filter(|roots| roots.roots.len() > 1);
        let branches = snapshot.branches.clone();
        let repo = self.repo.clone();
        let load_favorites = self.branch_popup.load_favorites();
        let load = self.branch_popup.begin_load();
        let token = load.token.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let favorites = load_favorites
                        .then(|| plico::favorite_preferences::for_repository(&repo, &token));
                    (
                        plico::branch_popup::recent_branches(&repo, &branches, None, &token),
                        favorites,
                        repo.fetch_specs(&token).map(|specs| {
                            specs
                                .into_iter()
                                .map(|s| s.remote().to_owned())
                                .collect::<Vec<_>>()
                        }),
                        project_roots.map(|roots| {
                            plico::branch_popup::project::Project::read(&roots.roots, &token)
                        }),
                    )
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if !s.branch_popup.accept_load(&load) {
                    return;
                }
                s.branch_popup.project = result.3.and_then(Result::ok).map(Arc::new);
                s.branch_popup.push_remotes = result.2.unwrap_or_default();
                if let Some(favorites) = result.1 {
                    s.branch_popup.install_favorites(favorites);
                }
                match result.0 {
                    Ok(recent) => {
                        for name in &recent {
                            if let Some(name) = name.strip_prefix("refs/heads/") {
                                s.branch_popup.expansion.reveal(
                                    plico::branch_popup::Section::Recent,
                                    name,
                                    true,
                                );
                            }
                        }
                        s.branch_popup.recent = recent;
                    }
                    Err(_) => {
                        s.branch_popup.recent.clear();
                    }
                }
                s.apply_repository_search(cx);
            });
        });
        self.branch_popup.track_load(task);
    }
    pub(super) fn toggle_repository_favorite(&mut self, reference: String, cx: &mut Context<Self>) {
        let Some(save) = self.branch_popup.begin_favorite(&reference) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (save, result) = cx
                .background_spawn(async move {
                    let result = save.write();
                    (save, result)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.branch_popup.finish_favorite(save, result) {
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn apply_repository_search(&mut self, cx: &mut Context<Self>) {
        use plico::{branch_popup, branch_search};
        self.branch_popup.prepare_search(cx);
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let current = snapshot
            .current_branch
            .as_ref()
            .map(|b| format!("refs/heads/{b}"));
        let mut groups = branch_popup::groups(
            &snapshot.branches,
            &snapshot.tags,
            current.as_deref(),
            &self
                .branch_popup
                .favorites()
                .present(&snapshot.branches, &snapshot.tags),
            &self.branch_popup.recent,
            true,
            None,
        );
        let result = branch_search::apply_to_popup(
            self.branch_popup.search.as_ref(),
            &mut groups,
            &[
                ("commit", t("Commit…")),
                ("new-branch", t("New Branch…")),
                ("revision", t("Checkout Tag or Revision…")),
            ],
            true,
            current.as_deref(),
            &mut self.branch_popup.expansion,
            true,
        );
        self.branch_popup.actions = result.actions;
        self.branch_popup.selection = result.selected;
        if let Some(branch_search::Selection::Reference(section, name)) =
            &self.branch_popup.selection
        {
            let rows = branch_popup::visible_rows(&groups, &self.branch_popup.expansion, true);
            if let Some(index)=rows.iter().position(|row| row.section==*section && matches!(&row.node,branch_popup::VisibleNode::Entry(branch_popup::Node::Ref {reference,..}) if reference.full_name==name)) {
                self.branch_popup.scroll.scroll_to_item(index,ScrollStrategy::Top);
            }
        }
        cx.notify();
    }
    pub(super) fn repository_groups(&self) -> Vec<plico::branch_popup::Group<'_>> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        let current = snapshot
            .current_branch
            .as_ref()
            .map(|b| format!("refs/heads/{b}"));
        let mut groups = plico::branch_popup::groups(
            &snapshot.branches,
            &snapshot.tags,
            current.as_deref(),
            &self
                .branch_popup
                .favorites()
                .present(&snapshot.branches, &snapshot.tags),
            &self.branch_popup.recent,
            true,
            None,
        );
        if let Some(search) = &self.branch_popup.search {
            search.filter_groups(&mut groups);
        }
        groups
    }
    pub(super) fn repository_branch_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let (incoming, outgoing, unfetched) = self.branch_update_counts();
        let head = self.snapshot.as_ref().and_then(|s| s.head_id.as_ref());
        let label = self
            .snapshot
            .as_ref()
            .and_then(|s| s.current_branch.clone())
            .or_else(|| {
                self.snapshot.as_ref().and_then(|s| {
                    s.tags
                        .iter()
                        .find(|tag| Some(&tag.id) == head)
                        .map(|tag| tag.name.trim_start_matches("refs/tags/").to_owned())
                })
            })
            .unwrap_or_else(|| {
                head.map(|id| format!("{} ({})", &id[..8], t("detached HEAD")))
                    .unwrap_or_else(|| t("Branch").into())
            });
        // Rebase temporarily detaches HEAD, but the widget describes the
        // operation's branch (GitBranchUtil.getDisplayableBranchText).
        let label = self
            .operation_state
            .as_ref()
            .map_or(label.clone(), |state| {
                use plico::operation_state::Operation;
                if matches!(
                    state.operation,
                    Operation::RebaseApply | Operation::RebaseMerge
                ) {
                    format!(
                        "{} {}",
                        t("Rebasing"),
                        state.rebase_branch.as_deref().unwrap_or(&label)
                    )
                } else if state.operation == Operation::Merge {
                    format!("{} {}", t("Merging"), label)
                } else {
                    label
                }
            });
        gpui_kit::component::popover::Popover::new("repository-branch-popup")
            .appearance(false)
            .overlay_closable(false)
            .open(self.branch_popup.open)
            .trigger(
                titlebar_menu_button("repository-branch-menu")
                    .tooltip(self.branch_update_tooltip())
                    .ghost()
                    .disabled(
                        (self.operation.busy()
                            && !self
                                .operation
                                .is_running(plico::active_operation::Kind::Fetch))
                            || self.settings_open
                            || head.is_none(),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                jb_icon(
                                    if self
                                        .operation_state
                                        .as_ref()
                                        .is_some_and(|state| state.in_progress())
                                    {
                                        "warning_dark.svg"
                                    } else if self.snapshot.as_ref().is_some_and(|snapshot| {
                                        snapshot.current_branch.is_none()
                                            && snapshot.head_id.is_some()
                                    }) {
                                        if self.snapshot.as_ref().is_some_and(|snapshot| {
                                            snapshot.tags.iter().any(|tag| {
                                                Some(&tag.id) == snapshot.head_id.as_ref()
                                            })
                                        }) {
                                            "branchLabel_dark.svg"
                                        } else {
                                            "warning_dark.svg"
                                        }
                                    } else {
                                        "vcs_dark.svg"
                                    },
                                )
                                .size(px(16.)),
                            )
                            .child(
                                div()
                                    .max_w(px(220.))
                                    .truncate()
                                    .text_size(cx.theme().font_size)
                                    .font_weight(FontWeight::NORMAL)
                                    .child(label),
                            )
                            .when(incoming > 0 || unfetched, |v| {
                                v.child(jb_icon("incoming_dark.svg").size(px(12.)).text_color(rgb(
                                    crate::ui_theme::color(cx, 0x548af7, 0x3574f0),
                                )))
                            })
                            .when(outgoing > 0, |v| {
                                v.child(jb_icon("outgoing_dark.svg").size(px(12.)).text_color(rgb(
                                    crate::ui_theme::color(cx, 0x57965c, 0x369650),
                                )))
                            })
                            .child(jb_icon("chevronDown_dark.svg")),
                    ),
            )
            .on_open_change(cx.listener(|s, open, window, cx| {
                s.branch_popup.open = *open;
                if *open {
                    s.branch_popup.search_editing = false;
                    let placeholder = t(if s.project_roots.read(cx).has_multiple_roots() {
                        "Search for branches, actions, repositories"
                    } else {
                        "Search for branches and actions"
                    });
                    s.branch_popup.input.update(cx, |input, cx| {
                        input.set_placeholder(placeholder, window, cx);
                        input.set_value("", window, cx);
                        input.focus(window, cx);
                    });
                    s.apply_repository_search(cx);
                    s.load_repository_recents(cx);
                } else {
                    s.dismiss_repository_menu(cx);
                }
                if *open && s.branch_popup.expansion.expanded.is_empty() {
                    if let Some(name) = s.snapshot.as_ref().and_then(|s| s.current_branch.as_ref())
                    {
                        s.branch_popup.expansion.reveal(
                            plico::branch_popup::Section::Local,
                            name,
                            true,
                        );
                    }
                }
                cx.notify();
            }))
            .content(move |_, _, cx| {
                owner
                    .update(cx, |s, cx| s.repository_menu_body(cx))
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .map(titlebar_menu)
    }
    pub(super) fn repository_pointer_selection(
        &mut self,
        event: &MouseMoveEvent,
        selection: plico::branch_search::Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self.branch_popup.pointer.replace(event.position);
        if previous.is_none() || previous == Some(event.position) || window.is_a11y_active() {
            return;
        }
        if self.branch_popup.selection.as_ref() != Some(&selection) {
            self.branch_popup.selection = Some(selection);
            cx.notify();
        }
    }
    pub(super) fn repository_visible_move(
        &mut self,
        direction: plico::branch_navigation::Move,
        cx: &mut Context<Self>,
    ) {
        use plico::branch_navigation;
        let groups = self.repository_groups();
        let rows = plico::branch_popup::visible_rows(&groups, &self.branch_popup.expansion, true);
        let actions = self
            .branch_popup
            .actions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let next = if self.branch_popup.project.is_some() {
            self.project_repository_move(direction)
        } else {
            branch_navigation::navigate_visible(
                &rows,
                &actions,
                self.branch_popup.selection.as_ref(),
                direction,
                false,
            )
        };
        if let Some(next) = next {
            if let Some(index) = rows
                .iter()
                .position(|row| branch_navigation::row_matches(row, &next))
            {
                self.branch_popup
                    .scroll
                    .scroll_to_item(index, ScrollStrategy::Top);
            }
            self.branch_popup.selection = Some(next);
            cx.notify();
        }
    }
    pub(super) fn activate_repository_action(
        &mut self,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || self.settings_open || self.welcome {
            return;
        }
        self.dismiss_repository_menu(cx);
        match action {
            "commit" => {
                self.open_local(window, cx);
                if let Some(view) = &self.local_view {
                    view.update(cx, |view, cx| view.focus_message(window, cx));
                }
            }
            "new-branch" => self.new_branch_dialog("HEAD".into(), window, cx),
            "revision" => self.open_checkout_input(window, cx),
            _ => {}
        }
        cx.notify();
    }
    pub(super) fn repository_activate_selection(
        &mut self,
        final_choice: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.branch_popup.input.focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        self.branch_popup.search_editing = false;
        if self.branch_popup.flush_search(cx) {
            self.apply_repository_search(cx);
        }
        match self.branch_popup.selection.clone() {
            Some(plico::branch_search::Selection::Repository(_))
                if self.repository_roots_updating(cx) => {}
            Some(plico::branch_search::Selection::Folder(key)) => {
                self.dismiss_repository_child(cx);
                if final_choice {
                    self.branch_popup.expansion.toggle(key);
                } else if !self.branch_popup.expansion.expanded.contains(&key) {
                    self.branch_popup.expansion.expanded.insert(key);
                } else {
                    self.repository_visible_move(plico::branch_navigation::Move::Next, cx);
                }
            }
            Some(plico::branch_search::Selection::Action(id)) if final_choice => {
                self.activate_repository_action(&id, window, cx);
            }
            Some(
                selection @ (plico::branch_search::Selection::Reference(..)
                | plico::branch_search::Selection::Repository(..)),
            ) => {
                if self.branch_popup.child_selection.as_ref() != Some(&selection) {
                    self.dismiss_repository_child(cx);
                    self.branch_popup.child_selection = Some(selection);
                }
            }
            None if !final_choice => {
                self.repository_visible_move(plico::branch_navigation::Move::First, cx)
            }
            _ => {}
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn repository_tree_left(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use plico::{branch_navigation, branch_search::Selection};
        if !self.branch_popup.input.focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        self.branch_popup.search_editing = false;
        if self.branch_popup.flush_search(cx) {
            self.apply_repository_search(cx);
        }
        self.dismiss_repository_child(cx);
        let groups = self.repository_groups();
        let rows = plico::branch_popup::visible_rows(&groups, &self.branch_popup.expansion, true);
        let index = self.branch_popup.selection.as_ref().and_then(|selected| {
            rows.iter()
                .position(|row| branch_navigation::row_matches(row, selected))
        });
        if let Some(index) = index {
            let row = &rows[index];
            if row.expanded {
                if let Selection::Folder(key) = branch_navigation::row_selection(row) {
                    self.branch_popup.expansion.expanded.remove(&key);
                }
            } else if let Some(parent) = rows[..index].iter().rposition(|r| r.depth < row.depth) {
                self.branch_popup.selection = Some(branch_navigation::row_selection(&rows[parent]));
                self.branch_popup
                    .scroll
                    .scroll_to_item(parent, ScrollStrategy::Top);
            } else if let Some(previous) = rows[..index].iter().rposition(|r| r.expanded) {
                self.branch_popup.selection =
                    Some(branch_navigation::row_selection(&rows[previous]));
                self.branch_popup
                    .scroll
                    .scroll_to_item(previous, ScrollStrategy::Top);
            } else if let Some(action) = self.branch_popup.actions.last() {
                self.branch_popup.selection = Some(Selection::Action(action.clone()));
            }
            cx.notify();
        } else {
            self.repository_visible_move(branch_navigation::Move::First, cx);
        }
        window.prevent_default();
        cx.stop_propagation();
    }
    pub(super) fn repository_search_move(
        &mut self,
        direction: plico::branch_navigation::Move,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use plico::branch_navigation;
        // Input bindings consume navigation before raw key-down dispatch.
        // Capture navigation while this input owns the branch tree.
        if !self.branch_popup.input.focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        // A pending query must be applied before navigating, not overwrite this
        // selection when its debounce fires afterwards.
        self.branch_popup.search_editing = false;
        if self.branch_popup.flush_search(cx) {
            self.apply_repository_search(cx);
        }
        let actions = self
            .branch_popup
            .actions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let groups = self.repository_groups();
        let next = if self.branch_popup.project.is_some() {
            self.project_repository_move(direction)
        } else if self.branch_popup.search.is_some() {
            branch_navigation::navigate(
                &groups,
                &actions,
                self.branch_popup.selection.as_ref(),
                direction,
                true,
                false,
            )
        } else {
            let rows =
                plico::branch_popup::visible_rows(&groups, &self.branch_popup.expansion, true);
            branch_navigation::navigate_visible(
                &rows,
                &actions,
                self.branch_popup.selection.as_ref(),
                direction,
                false,
            )
        };
        if let Some(next) = next {
            self.dismiss_repository_child(cx);
            if let plico::branch_search::Selection::Reference(section, name) = &next {
                if let Some(reference) = self
                    .repository_groups()
                    .into_iter()
                    .filter(|g| g.section == *section)
                    .flat_map(|g| g.references)
                    .find(|r| r.full_name == name)
                {
                    let short = reference.name.to_owned();
                    self.branch_popup.expansion.reveal(*section, &short, true);
                }
            }
            self.branch_popup.selection = Some(next);
            let groups = self.repository_groups();
            let rows =
                plico::branch_popup::visible_rows(&groups, &self.branch_popup.expansion, true);
            if let Some(selected) = &self.branch_popup.selection {
                if let Some(index) = rows
                    .iter()
                    .position(|row| branch_navigation::row_matches(row, selected))
                {
                    self.branch_popup
                        .scroll
                        .scroll_to_item(index, ScrollStrategy::Top);
                }
            }
            cx.notify();
        }
        window.prevent_default();
        cx.stop_propagation();
    }
    pub(super) fn repository_search_label(&self, label: &str) -> AnyElement {
        let ranges = self
            .branch_popup
            .search
            .as_ref()
            .and_then(|search| search.find(label))
            .map(|found| plico::branch_search::byte_ranges(label, &found.fragments))
            .unwrap_or_default();
        StyledText::new(label.to_owned())
            .with_highlights(ranges.into_iter().map(|range| {
                (
                    range,
                    HighlightStyle {
                        color: Some(rgb(0x000000).into()),
                        background_color: Some(rgba(0xffd042b3).into()),
                        ..Default::default()
                    },
                )
            }))
            .into_any_element()
    }
    pub(super) fn repository_menu_body(&self, cx: &mut Context<Self>) -> AnyElement {
        use plico::branch_popup::{self, FolderKey, Node, Section, VisibleNode};
        let count = branch_popup::visible_rows(
            &self.repository_groups(),
            &self.branch_popup.expansion,
            true,
        )
        .len();
        v_flex().id("repository-menu-body")
            .key_context("BranchPopup")
            .capture_action(cx.listener(|s, _: &RenameBranch, w, cx| {
                if let Some(plico::branch_search::Selection::Reference(_, reference)) = s.branch_popup.selection.clone() {
                    if reference.starts_with("refs/heads/") { s.prepare_rename_branch(reference,w,cx); }
                }
            }))
            .capture_action(cx.listener(|s, action: &gpui_kit::component::input::Enter, w, cx| {
                if action.secondary || action.shift { cx.propagate(); } else { s.repository_activate_selection(true,w,cx); }
            }))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveLeft, w, cx| {
                if s.branch_popup.search_editing { cx.propagate(); } else { s.repository_tree_left(w,cx); }
            }))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveRight, w, cx| {
                if s.branch_popup.search_editing { cx.propagate(); } else { s.repository_activate_selection(false,w,cx); }
            }))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveUp, w, cx| s.repository_search_move(plico::branch_navigation::Move::Previous,w,cx)))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveDown, w, cx| s.repository_search_move(plico::branch_navigation::Move::Next,w,cx)))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveHome, w, cx| {
                if s.branch_popup.search_editing { cx.propagate(); } else { s.repository_search_move(plico::branch_navigation::Move::First,w,cx); }
            }))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveEnd, w, cx| {
                if s.branch_popup.search_editing { cx.propagate(); } else { s.repository_search_move(plico::branch_navigation::Move::Last,w,cx); }
            }))
            .w(px(360.)).p_2().gap_1().bg(rgb(crate::ui_theme::color(cx, 0x25262a, 0xf7f8f9))).border_1().border_color(rgb(crate::ui_theme::palette(cx).popup_border)).rounded(px(8.))
            .on_mouse_down_out(cx.listener(|s,event: &MouseDownEvent,_,cx| {
                let in_child=s.branch_popup.child_menu.as_ref().and_then(|menu|menu.upgrade())
                    .is_some_and(|menu|menu.read(cx).contains_menu_position(&event.position, cx));
                if !in_child { s.dismiss_repository_menu(cx); cx.notify(); }
            }))
            .child(h_flex().items_center().gap_1()
                .child(div().flex_1().min_w_0().on_mouse_down(MouseButton::Left, cx.listener(|s, _, _, cx| {
                    s.branch_popup.search_editing = true; cx.notify();
                })).child(Input::new(&self.branch_popup.input).prefix(jb_icon("search_dark.svg"))))
                .child(Button::new("repository-fetch").ghost().small().w(px(28.)).h(px(28.))
                    .icon(jb_icon("fetch_dark.svg"))
                    .loading(self.operation.is_running(plico::active_operation::Kind::Fetch))
                    .disabled(self.operation.busy() && !self.operation.is_running(plico::active_operation::Kind::Fetch))
                    .tooltip(t(if self.operation.is_running(plico::active_operation::Kind::Fetch) { "Fetching…" } else { "Fetch" }))
                    .on_click(cx.listener(|s, _, window, cx| s.start_fetch(window, cx)))))
            .children(self.branch_popup.actions.iter().map(|id| {
                let action = id.clone();
                let hovered_action = id.clone();
                let label = t(if id == "commit" { "Commit…" } else if id == "new-branch" { "New Branch…" } else { "Checkout Tag or Revision…" });
                v_flex().w_full()
                    .child(Button::new(SharedString::from(format!("repository-action:{id}")))
                        .on_mouse_move(cx.listener(move |s,e,w,cx| s.repository_pointer_selection(e,plico::branch_search::Selection::Action(hovered_action.clone()),w,cx)))
                        .custom(ButtonCustomVariant::new(cx).active(rgb(crate::ui_theme::color(cx, 0x2e4773, 0xd0dffe)).into()))
                        .h(px(28.)).w_full()
                        .selected(matches!(&self.branch_popup.selection,Some(plico::branch_search::Selection::Action(selected)) if selected == id))
                        .child(h_flex().w_full().gap_2()
                            .child(div().w(px(16.)).when(id == "commit", |v| v.child(jb_icon("tool-commit_dark.svg"))))
                            .child(self.repository_search_label(label)))
                        .on_click(cx.listener(move |s,_,w,cx| s.activate_repository_action(&action,w,cx))))
                    .when(id == "commit", |v| v.child(div().h(px(1.)).my_1().w_full().bg(rgb(crate::ui_theme::palette(cx).popup_border))))
            }))
            .children(self.repository_project_rows(cx))
            .when(self.branch_popup.favorites_failed(), |v| v.child(div().text_color(rgb(crate::ui_theme::color(cx, 0xe08888, 0xc54e58))).child(t("Could not load or save favorites"))))
            .when(count>0 && !self.branch_popup.actions.is_empty(), |v|v.child(div().h(px(1.)).w_full().bg(rgb(crate::ui_theme::palette(cx).popup_border))))
            .when(count==0 && self.branch_popup.actions.is_empty(), |v| v.child(div().p_2().text_color(rgb(crate::ui_theme::palette(cx).muted)).child(t("No matching branches or actions"))))
            .when(count>0, |v|v.child(uniform_list("repository-reference-tree", count, cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                if s.snapshot.is_none() { return Vec::new(); }
                let groups=s.repository_groups();
                let rows = branch_popup::visible_rows(&groups, &s.branch_popup.expansion, true);
                range.filter_map(|index| rows.get(index).map(|row| {
                    let indent = px(8. + row.depth as f32 * 16.);
                    let folder = match &row.node {
                        VisibleNode::Section(section) => Some((FolderKey { section: *section, prefix: String::new() }, t(match section {
                            Section::Recent => "Recent", Section::Local => "Local", Section::Remote => "Remote", Section::Tags => "Tags" }).to_owned())),
                        VisibleNode::Entry(Node::Folder { key, label }) => Some((key.clone(), (*label).to_owned())),
                        _ => None,
                    };
                    if let Some((key,label)) = folder {
                        let pointer_key=key.clone();
                        let selected=matches!(&s.branch_popup.selection,Some(plico::branch_search::Selection::Folder(other)) if other==&key);
                        return div().id(SharedString::from(format!("repository-folder:{:?}:{}", key.section, key.prefix))).h(px(28.)).w_full().pl(indent).flex().items_center().gap_1().cursor_pointer()
                            .on_mouse_move(cx.listener(move |s,e,w,cx|s.repository_pointer_selection(e,plico::branch_search::Selection::Folder(pointer_key.clone()),w,cx)))
                            .when(selected,|d|d.bg(rgb(crate::ui_theme::color(cx, 0x2e4773, 0xd0dffe))))
                            .hover(|d| d.bg(rgb(if selected {crate::ui_theme::color(cx, 0x2e4773, 0xd0dffe)} else {crate::ui_theme::color(cx, 0x34363b, 0xf7f8f9)})))
                            .child(jb_icon(if row.expanded { "chevronDown_dark.svg" } else { "chevronRight_dark.svg" })).child(if key.prefix.is_empty() { div().child(label.clone()).into_any_element() } else { s.repository_search_label(&label) })
                            .on_click(cx.listener(move |s,_,_,cx| { s.dismiss_repository_child(cx); s.branch_popup.expansion.toggle(key.clone()); s.branch_popup.selection=Some(plico::branch_search::Selection::Folder(key.clone())); cx.notify(); })).into_any_element();
                    }
                    let VisibleNode::Entry(Node::Ref { reference, label }) = &row.node else { unreachable!() };
                    let full_ref = reference.full_name.to_owned();
                    let current = reference.current;
                    let tag = full_ref.starts_with("refs/tags/");
                    let owner = cx.entity().downgrade();
                    let section = row.section;
                    let identity=plico::branch_search::Selection::Reference(section,full_ref.clone());
                    let hover_identity=identity.clone();
                    let child_open=s.branch_popup.child_selection.as_ref()==Some(&identity);
                    let context_owner=owner.clone(); let context_ref=full_ref.clone();
                    let change_owner=owner.clone();
                    let favorite = reference.favorite;
                    let favorite_ref = full_ref.clone();
                    let icon = if current && favorite { "currentBranchFavoriteLabel_dark.svg" } else if current { "currentBranchLabel_dark.svg" }
                        else if favorite { "favorite.svg" } else if tag { "branchLabel_dark.svg" } else { "vcs_dark.svg" };
                    let star = Button::new(SharedString::from(format!("favorite:{:?}:{}",row.section,full_ref))).custom(ButtonCustomVariant::new(cx)).h(px(28.)).w(px(24.)).p_0().flex_shrink_0()
                        .disabled(!s.branch_popup.can_favorite())
                        .tooltip(t(if favorite { "Remove from Favorites" } else { "Add to Favorites" }))
                        .child(jb_icon(icon).text_color(rgb(if favorite { 0xf4af3d } else { crate::ui_theme::palette(cx).foreground })))
                        .on_click(cx.listener(move |s,_,_,cx| s.toggle_repository_favorite(favorite_ref.clone(),cx)));
                    let selected=matches!(&s.branch_popup.selection,Some(plico::branch_search::Selection::Reference(section,name)) if *section==row.section && name==&full_ref);
                    h_flex().id(SharedString::from(format!("repository-row:{:?}:{}",row.section,full_ref)))
                        .on_mouse_move(cx.listener(move |s,e,w,cx|s.repository_pointer_selection(e,hover_identity.clone(),w,cx)))
                        .h(px(28.)).w_full().pl(indent).when(selected,|v|v.bg(rgb(crate::ui_theme::color(cx, 0x2e4773, 0xd0dffe)))).child(star).child(div().flex_1().child(Button::new(SharedString::from(format!("repository-reference:{:?}:{}", row.section, full_ref))).custom(ButtonCustomVariant::new(cx)).h(px(28.)).w_full()
                        .child(h_flex().w_full().gap_1().child(s.repository_search_label(label))
                            .child(div().flex_1()).child(jb_icon("chevronRight_dark.svg")))
                        .dropdown_menu_with_anchor(Anchor::LeftCenter, move |mut menu,w,menu_cx| {
                            menu=menu.close_on_left(true);
                            let child=menu_cx.entity().downgrade();
                            let _=owner.update(menu_cx,|s,cx| {
                                s.dismiss_repository_child(cx);
                                s.branch_popup.child_selection=Some(plico::branch_search::Selection::Reference(section,full_ref.clone()));
                                s.branch_popup.child_menu=Some(child);
                                s.branch_popup.selection=Some(plico::branch_search::Selection::Reference(section,full_ref.clone()));
                                cx.notify();
                            });
                            let disabled = owner.upgrade().is_none_or(|s| s.read(menu_cx).operation.busy() || s.read(menu_cx).operation.saving_documents());
                            let checkout = owner.upgrade().is_some_and(|s|s.read(menu_cx).reference_checkout_target(&full_ref).is_some());
                            if checkout {
                                let owner=owner.clone();let reference=full_ref.clone();
                                menu=menu.item(PopupMenuItem::new(t("Checkout")).disabled(disabled).on_click(move |_,w,cx| {
                                    let _=owner.update(cx,|s,cx|s.checkout_reference(reference.clone(),w,cx));
                                }));
                            }
                            if full_ref.starts_with("refs/heads/") {
                                menu=menu.item(Review::branch_rename_item(&owner,&full_ref,menu_cx));
                                let delete_owner=owner.clone();let delete_ref=full_ref.clone();
                                menu=menu.item(PopupMenuItem::new(t("Delete…")).disabled(disabled || current).on_click(move |_,w,cx| {
                                    let _=delete_owner.update(cx,|s,cx|s.prepare_delete_branch(delete_ref.clone(),w,cx));
                                }));
                            }
                            if !tag && !current {
                                menu=menu.item(Review::branch_compare_item(&owner, &full_ref, menu_cx)).item(Review::branch_working_comparison_item(&owner, &full_ref, menu_cx));
                                menu=menu.item(Review::branch_rebase_item(&owner, &full_ref, menu_cx));
                                let merge_owner=owner.clone();let merge_ref=full_ref.clone();
                                let enabled=owner.upgrade().is_some_and(|s|s.read(menu_cx).can_merge_branch(&full_ref));
                                menu=menu.item(PopupMenuItem::new(t("Merge into Current")).disabled(!enabled).on_click(move |_,w,cx| {
                                    let _=merge_owner.update(cx,|s,cx|s.merge_branch(merge_ref.clone(),w,cx));
                                }));
                            }
                            if tag { menu = Review::tag_push_menu(menu, &owner, &full_ref, w, menu_cx); }
                            let create_owner = owner.clone(); let create_ref = full_ref.clone();
                            menu = menu.item(PopupMenuItem::new(t("New Branch…")).disabled(disabled).on_click(move |_, w, cx| {
                                let _ = create_owner.update(cx, |s, cx| s.new_branch_dialog(create_ref.clone(), w, cx));
                            }));
                            let full_ref = full_ref.clone();
                            menu.item(PopupMenuItem::new(t("Copy Reference Name")).on_click(move |_,_,cx| cx.write_to_clipboard(ClipboardItem::new_string(full_ref.clone()))))
                        }).side_start(true).open(child_open).on_open_change(move |open,_,cx| {
                            let _=change_owner.update(cx,|s,cx| {
                                if *open { s.branch_popup.child_selection=Some(identity.clone()); }
                                else if s.branch_popup.child_selection.as_ref()==Some(&identity) {
                                    s.branch_popup.child_selection=None;
                                    s.branch_popup.child_menu=None;
                                }
                                cx.notify();
                            });
                        })))
                        .context_menu(move |menu,w,cx| {
                            let available=context_owner.upgrade().is_some_and(|s|s.read(cx).reference_checkout_target(&context_ref).is_some());
                            let disabled=context_owner.upgrade().is_none_or(|s|s.read(cx).operation.busy() || s.read(cx).operation.saving_documents());
                            let owner=context_owner.clone();let reference=context_ref.clone();
                            let mut menu=menu;
                            if context_ref.starts_with("refs/heads/") {
                                menu=menu.item(Review::branch_rename_item(&context_owner,&context_ref,cx));
                                let delete_owner=context_owner.clone();let delete_ref=context_ref.clone();
                                menu=menu.item(PopupMenuItem::new(t("Delete…")).disabled(disabled || current).on_click(move |_,w,cx| {
                                    let _=delete_owner.update(cx,|s,cx|s.prepare_delete_branch(delete_ref.clone(),w,cx));
                                }));
                            }
                            if !tag && !current {
                                menu=menu.item(Review::branch_compare_item(&context_owner, &context_ref, cx)).item(Review::branch_working_comparison_item(&context_owner, &context_ref, cx));
                                menu=menu.item(Review::branch_rebase_item(&context_owner, &context_ref, cx));
                                let merge_owner=context_owner.clone();let merge_ref=context_ref.clone();
                                let enabled=context_owner.upgrade().is_some_and(|s|s.read(cx).can_merge_branch(&context_ref));
                                menu=menu.item(PopupMenuItem::new(t("Merge into Current")).disabled(!enabled).on_click(move |_,w,cx| {
                                    let _=merge_owner.update(cx,|s,cx|s.merge_branch(merge_ref.clone(),w,cx));
                                }));
                            }
                            if tag { menu = Review::tag_push_menu(menu, &context_owner, &context_ref, w, cx); }
                            let create_owner=context_owner.clone(); let create_ref=context_ref.clone();
                            menu.item(PopupMenuItem::new(t("New Branch…")).disabled(disabled).on_click(move |_,w,cx| {
                                let _=create_owner.update(cx,|s,cx|s.new_branch_dialog(create_ref.clone(),w,cx));
                            })).when(available,|menu|menu.item(PopupMenuItem::new(t("Checkout")).disabled(disabled).on_click(move |_,w,cx| {
                                let _=owner.update(cx,|s,cx|s.checkout_reference(reference.clone(),w,cx));
                            }))).item(PopupMenuItem::new(t("Copy Reference Name")).on_click({
                                let reference=context_ref.clone();move |_,_,cx|cx.write_to_clipboard(ClipboardItem::new_string(reference.clone()))
                            }))
                        })
                        .into_any_element()
                })).collect::<Vec<_>>()
            })).track_scroll(&self.branch_popup.scroll).h(px(count.min(12) as f32 * 28.))))
            .into_any_element()
    }
}
