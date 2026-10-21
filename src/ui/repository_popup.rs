//! Main-window repository popup coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn dismiss_repository_child(&mut self, cx: &mut Context<Self>) {
        self.repository_child_selection = None;
        if let Some(menu) = self.repository_child_menu.take() {
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
        self.repository_menu_open = false;
        self.repository_pointer = None;
        self.repository_search_task = Task::ready(());
        self.repository_search = None;
        self.repository_selection = None;
        if let Some(token) = self.repository_recent_token.take() {
            token.cancel();
        }
        self.repository_recent_generation += 1;
        self.repository_recent_task = Task::ready(());
    }
    pub(super) fn load_repository_recents(&mut self, cx: &mut Context<Self>) {
        if let Some(token) = self.repository_recent_token.take() {
            token.cancel();
        }
        self.repository_recent_generation += 1;
        let generation = self.repository_recent_generation;
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let branches = snapshot.branches.clone();
        let repo = self.repo.clone();
        let load_favorites = !self.repository_favorites_saving;
        let path = repo.path.clone();
        let token = Cancel::default();
        self.repository_recent_token = Some(token.clone());
        self.repository_recent_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let favorites = load_favorites
                        .then(|| plico::favorite_preferences::for_repository(&repo, &token));
                    (
                        plico::branch_popup::recent_branches(&repo, &branches, None, &token),
                        favorites,
                    )
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.repository_recent_generation != generation
                    || s.repo.path != path
                    || !s.repository_menu_open
                {
                    return;
                }
                s.repository_recent_token = None;
                if let Some(favorites) = result.1 {
                    match favorites {
                        Ok((path, prefs)) => {
                            s.repository_favorites_path = Some(path);
                            s.repository_favorites = prefs;
                            s.repository_favorites_error = None;
                        }
                        Err(error) => {
                            s.repository_favorites_path = None;
                            s.repository_favorites_error = Some(error);
                        }
                    }
                }
                match result.0 {
                    Ok(recent) => {
                        for name in &recent {
                            if let Some(name) = name.strip_prefix("refs/heads/") {
                                s.repository_expansion.reveal(
                                    plico::branch_popup::Section::Recent,
                                    name,
                                    true,
                                );
                            }
                        }
                        s.repository_recent = recent;
                    }
                    Err(_) => {
                        s.repository_recent.clear();
                    }
                }
                s.apply_repository_search(cx);
            });
        });
    }
    pub(super) fn toggle_repository_favorite(&mut self, reference: String, cx: &mut Context<Self>) {
        if self.repository_favorites_saving || self.repository_recent_token.is_some() {
            return;
        }
        let Some(file) = self.repository_favorites_path.clone() else {
            return;
        };
        let previous = self.repository_favorites.clone();
        if !self.repository_favorites.toggle(&reference) {
            return;
        }
        let next = self.repository_favorites.clone();
        let epoch = self.repository_favorites_epoch;
        self.repository_favorites_saving = true;
        self.repository_favorites_error = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { plico::favorite_preferences::save(&file, &next) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.repository_favorites_epoch != epoch {
                    return;
                }
                s.repository_favorites_saving = false;
                if let Err(error) = result {
                    s.repository_favorites = previous;
                    s.repository_favorites_error = Some(error.to_string());
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn apply_repository_search(&mut self, cx: &mut Context<Self>) {
        use plico::{branch_popup, branch_search};
        let value = self.repository_search_input.read(cx).value();
        self.repository_applied_query = value.to_string();
        self.repository_search = branch_search::Search::new(
            branch_search::popup_pattern(value.as_str()),
            &Default::default(),
            cfg!(target_os = "linux"),
        );
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
                .repository_favorites
                .present(&snapshot.branches, &snapshot.tags),
            &self.repository_recent,
            true,
            None,
        );
        let result = branch_search::apply_to_popup(
            self.repository_search.as_ref(),
            &mut groups,
            &[
                ("commit", t("Commit…")),
                ("new-branch", t("New Branch…")),
                ("revision", t("Checkout Tag or Revision…")),
            ],
            true,
            current.as_deref(),
            &mut self.repository_expansion,
            true,
        );
        self.repository_actions = result.actions;
        self.repository_selection = result.selected;
        if let Some(branch_search::Selection::Reference(section, name)) = &self.repository_selection
        {
            let rows = branch_popup::visible_rows(&groups, &self.repository_expansion, true);
            if let Some(index)=rows.iter().position(|row| row.section==*section && matches!(&row.node,branch_popup::VisibleNode::Entry(branch_popup::Node::Ref {reference,..}) if reference.full_name==name)) {
                self.repository_scroll.scroll_to_item(index,ScrollStrategy::Top);
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
                .repository_favorites
                .present(&snapshot.branches, &snapshot.tags),
            &self.repository_recent,
            true,
            None,
        );
        if let Some(search) = &self.repository_search {
            search.filter_groups(&mut groups);
        }
        groups
    }
    pub(super) fn repository_branch_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let (incoming,outgoing,unfetched)=self.branch_update_counts();
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
        gpui_kit::component::popover::Popover::new("repository-branch-popup")
            .appearance(false)
            .overlay_closable(false)
            .open(self.repository_menu_open)
            .trigger(
                titlebar_menu_button("repository-branch-menu")
                    .tooltip(self.branch_update_tooltip())
                    .ghost()
                    .disabled((self.operation.busy() && !self.operation.is_running(plico::active_operation::Kind::Fetch)) || self.settings_open || head.is_none())
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
                            .when(incoming>0 || unfetched,|v|v.child(jb_icon("incoming_dark.svg").size(px(12.)).text_color(rgb(0x548af7))))
                            .when(outgoing>0,|v|v.child(jb_icon("outgoing_dark.svg").size(px(12.)).text_color(rgb(0x57965c))))
                            .child(jb_icon("chevronDown_dark.svg")),
                    ),
            )
            .on_open_change(cx.listener(|s, open, window, cx| {
                s.repository_menu_open = *open;
                if *open {
                    s.repository_search_editing = false;
                    s.repository_search_input.update(cx, |input, cx| {
                        input.set_value("", window, cx);
                        input.focus(window, cx);
                    });
                    s.apply_repository_search(cx);
                    s.load_repository_recents(cx);
                } else {
                    s.dismiss_repository_menu(cx);
                }
                if *open && s.repository_expansion.expanded.is_empty() {
                    if let Some(name) = s.snapshot.as_ref().and_then(|s| s.current_branch.as_ref())
                    {
                        s.repository_expansion.reveal(
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
        let previous = self.repository_pointer.replace(event.position);
        if previous.is_none() || previous == Some(event.position) || window.is_a11y_active() {
            return;
        }
        if self.repository_selection.as_ref() != Some(&selection) {
            self.repository_selection = Some(selection);
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
        let rows = plico::branch_popup::visible_rows(&groups, &self.repository_expansion, true);
        let actions = self
            .repository_actions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        if let Some(next) = branch_navigation::navigate_visible(
            &rows,
            &actions,
            self.repository_selection.as_ref(),
            direction,
            false,
        ) {
            if let Some(index) = rows
                .iter()
                .position(|row| branch_navigation::row_matches(row, &next))
            {
                self.repository_scroll
                    .scroll_to_item(index, ScrollStrategy::Top);
            }
            self.repository_selection = Some(next);
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
        if !self
            .repository_search_input
            .focus_handle(cx)
            .is_focused(window)
        {
            cx.propagate();
            return;
        }
        self.repository_search_editing = false;
        self.repository_search_task = Task::ready(());
        if self.repository_search_input.read(cx).value().as_str() != self.repository_applied_query {
            self.apply_repository_search(cx);
        }
        match self.repository_selection.clone() {
            Some(plico::branch_search::Selection::Folder(key)) => {
                self.dismiss_repository_child(cx);
                if final_choice {
                    self.repository_expansion.toggle(key);
                } else if !self.repository_expansion.expanded.contains(&key) {
                    self.repository_expansion.expanded.insert(key);
                } else {
                    self.repository_visible_move(plico::branch_navigation::Move::Next, cx);
                }
            }
            Some(plico::branch_search::Selection::Action(id)) if final_choice => {
                self.activate_repository_action(&id, window, cx);
            }
            Some(selection @ plico::branch_search::Selection::Reference(..)) => {
                if self.repository_child_selection.as_ref() != Some(&selection) {
                    self.dismiss_repository_child(cx);
                    self.repository_child_selection = Some(selection);
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
        if !self
            .repository_search_input
            .focus_handle(cx)
            .is_focused(window)
        {
            cx.propagate();
            return;
        }
        self.repository_search_editing = false;
        self.repository_search_task = Task::ready(());
        if self.repository_search_input.read(cx).value().as_str() != self.repository_applied_query {
            self.apply_repository_search(cx);
        }
        self.dismiss_repository_child(cx);
        let groups = self.repository_groups();
        let rows = plico::branch_popup::visible_rows(&groups, &self.repository_expansion, true);
        let index = self.repository_selection.as_ref().and_then(|selected| {
            rows.iter()
                .position(|row| branch_navigation::row_matches(row, selected))
        });
        if let Some(index) = index {
            let row = &rows[index];
            if row.expanded {
                if let Selection::Folder(key) = branch_navigation::row_selection(row) {
                    self.repository_expansion.expanded.remove(&key);
                }
            } else if let Some(parent) = rows[..index].iter().rposition(|r| r.depth < row.depth) {
                self.repository_selection = Some(branch_navigation::row_selection(&rows[parent]));
                self.repository_scroll
                    .scroll_to_item(parent, ScrollStrategy::Top);
            } else if let Some(previous) = rows[..index].iter().rposition(|r| r.expanded) {
                self.repository_selection = Some(branch_navigation::row_selection(&rows[previous]));
                self.repository_scroll
                    .scroll_to_item(previous, ScrollStrategy::Top);
            } else if let Some(action) = self.repository_actions.last() {
                self.repository_selection = Some(Selection::Action(action.clone()));
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
        if !self
            .repository_search_input
            .focus_handle(cx)
            .is_focused(window)
        {
            cx.propagate();
            return;
        }
        // A pending query must be applied before navigating, not overwrite this
        // selection when its debounce fires afterwards.
        self.repository_search_editing = false;
        self.repository_search_task = Task::ready(());
        if self.repository_search_input.read(cx).value().as_str() != self.repository_applied_query {
            self.apply_repository_search(cx);
        }
        let actions = self
            .repository_actions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let groups = self.repository_groups();
        let next = if self.repository_search.is_some() {
            branch_navigation::navigate(
                &groups,
                &actions,
                self.repository_selection.as_ref(),
                direction,
                true,
                false,
            )
        } else {
            let rows = plico::branch_popup::visible_rows(&groups, &self.repository_expansion, true);
            branch_navigation::navigate_visible(
                &rows,
                &actions,
                self.repository_selection.as_ref(),
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
                    self.repository_expansion.reveal(*section, &short, true);
                }
            }
            self.repository_selection = Some(next);
            let groups = self.repository_groups();
            let rows = plico::branch_popup::visible_rows(&groups, &self.repository_expansion, true);
            if let Some(selected) = &self.repository_selection {
                if let Some(index) = rows
                    .iter()
                    .position(|row| branch_navigation::row_matches(row, selected))
                {
                    self.repository_scroll
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
            .repository_search
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
        let count =
            branch_popup::visible_rows(&self.repository_groups(), &self.repository_expansion, true)
                .len();
        v_flex().id("repository-menu-body")
            .capture_action(cx.listener(|s, action: &gpui_kit::component::input::Enter, w, cx| {
                if action.secondary || action.shift { cx.propagate(); } else { s.repository_activate_selection(true,w,cx); }
            }))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveLeft, w, cx| {
                if s.repository_search_editing { cx.propagate(); } else { s.repository_tree_left(w,cx); }
            }))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveRight, w, cx| {
                if s.repository_search_editing { cx.propagate(); } else { s.repository_activate_selection(false,w,cx); }
            }))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveUp, w, cx| s.repository_search_move(plico::branch_navigation::Move::Previous,w,cx)))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveDown, w, cx| s.repository_search_move(plico::branch_navigation::Move::Next,w,cx)))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveHome, w, cx| {
                if s.repository_search_editing { cx.propagate(); } else { s.repository_search_move(plico::branch_navigation::Move::First,w,cx); }
            }))
            .capture_action(cx.listener(|s, _: &gpui_kit::component::input::MoveEnd, w, cx| {
                if s.repository_search_editing { cx.propagate(); } else { s.repository_search_move(plico::branch_navigation::Move::Last,w,cx); }
            }))
            .w(px(360.)).p_2().gap_1().bg(rgb(0x25262a)).border_1().border_color(rgb(POPUP_BORDER)).rounded(px(8.))
            .on_mouse_down_out(cx.listener(|s,event: &MouseDownEvent,_,cx| {
                let in_child=s.repository_child_menu.as_ref().and_then(|menu|menu.upgrade())
                    .is_some_and(|menu|menu.read(cx).contains_position(&event.position));
                if !in_child { s.dismiss_repository_menu(cx); cx.notify(); }
            }))
            .child(h_flex().items_center().gap_1()
                .child(div().flex_1().min_w_0().on_mouse_down(MouseButton::Left, cx.listener(|s, _, _, cx| {
                    s.repository_search_editing = true; cx.notify();
                })).child(Input::new(&self.repository_search_input).prefix(jb_icon("search_dark.svg"))))
                .child(Button::new("repository-fetch").ghost().small().w(px(28.)).h(px(28.))
                    .icon(jb_icon("fetch_dark.svg"))
                    .loading(self.operation.is_running(plico::active_operation::Kind::Fetch))
                    .disabled(self.operation.busy() && !self.operation.is_running(plico::active_operation::Kind::Fetch))
                    .tooltip(t(if self.operation.is_running(plico::active_operation::Kind::Fetch) { "Fetching…" } else { "Fetch" }))
                    .on_click(cx.listener(|s, _, window, cx| s.start_fetch(window, cx)))))
            .children(self.repository_actions.iter().map(|id| {
                let action = id.clone();
                let hovered_action = id.clone();
                let label = t(if id == "commit" { "Commit…" } else if id == "new-branch" { "New Branch…" } else { "Checkout Tag or Revision…" });
                v_flex().w_full()
                    .child(Button::new(SharedString::from(format!("repository-action:{id}")))
                        .on_mouse_move(cx.listener(move |s,e,w,cx| s.repository_pointer_selection(e,plico::branch_search::Selection::Action(hovered_action.clone()),w,cx)))
                        .custom(ButtonCustomVariant::new(cx).active(rgb(0x2e4773).into()))
                        .h(px(28.)).w_full()
                        .selected(matches!(&self.repository_selection,Some(plico::branch_search::Selection::Action(selected)) if selected == id))
                        .child(h_flex().w_full().gap_2()
                            .child(div().w(px(16.)).when(id == "commit", |v| v.child(jb_icon("tool-commit_dark.svg"))))
                            .child(self.repository_search_label(label)))
                        .on_click(cx.listener(move |s,_,w,cx| s.activate_repository_action(&action,w,cx))))
                    .when(id == "commit", |v| v.child(div().h(px(1.)).my_1().w_full().bg(rgb(POPUP_BORDER))))
            }))
            .when(self.repository_favorites_error.is_some(), |v| v.child(div().text_color(rgb(0xe08888)).child(t("Could not load or save favorites"))))
            .when(count>0 && !self.repository_actions.is_empty(), |v|v.child(div().h(px(1.)).w_full().bg(rgb(POPUP_BORDER))))
            .when(count==0 && self.repository_actions.is_empty(), |v| v.child(div().p_2().text_color(rgb(MUTED)).child(t("No matching branches or actions"))))
            .when(count>0, |v|v.child(uniform_list("repository-reference-tree", count, cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                if s.snapshot.is_none() { return Vec::new(); }
                let groups=s.repository_groups();
                let rows = branch_popup::visible_rows(&groups, &s.repository_expansion, true);
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
                        let selected=matches!(&s.repository_selection,Some(plico::branch_search::Selection::Folder(other)) if other==&key);
                        return div().id(SharedString::from(format!("repository-folder:{:?}:{}", key.section, key.prefix))).h(px(28.)).w_full().pl(indent).flex().items_center().gap_1().cursor_pointer()
                            .on_mouse_move(cx.listener(move |s,e,w,cx|s.repository_pointer_selection(e,plico::branch_search::Selection::Folder(pointer_key.clone()),w,cx)))
                            .when(selected,|d|d.bg(rgb(0x2e4773)))
                            .hover(move |d| d.bg(rgb(if selected {0x2e4773} else {0x34363b})))
                            .child(jb_icon(if row.expanded { "chevronDown_dark.svg" } else { "chevronRight_dark.svg" })).child(if key.prefix.is_empty() { div().child(label.clone()).into_any_element() } else { s.repository_search_label(&label) })
                            .on_click(cx.listener(move |s,_,_,cx| { s.dismiss_repository_child(cx); s.repository_expansion.toggle(key.clone()); s.repository_selection=Some(plico::branch_search::Selection::Folder(key.clone())); cx.notify(); })).into_any_element();
                    }
                    let VisibleNode::Entry(Node::Ref { reference, label }) = &row.node else { unreachable!() };
                    let full_ref = reference.full_name.to_owned();
                    let current = reference.current;
                    let tag = full_ref.starts_with("refs/tags/");
                    let owner = cx.entity().downgrade();
                    let section = row.section;
                    let identity=plico::branch_search::Selection::Reference(section,full_ref.clone());
                    let hover_identity=identity.clone();
                    let child_open=s.repository_child_selection.as_ref()==Some(&identity);
                    let context_owner=owner.clone(); let context_ref=full_ref.clone();
                    let change_owner=owner.clone();
                    let favorite = reference.favorite;
                    let favorite_ref = full_ref.clone();
                    let icon = if current && favorite { "currentBranchFavoriteLabel_dark.svg" } else if current { "currentBranchLabel_dark.svg" }
                        else if favorite { "favorite.svg" } else if tag { "branchLabel_dark.svg" } else { "vcs_dark.svg" };
                    let star = Button::new(SharedString::from(format!("favorite:{:?}:{}",row.section,full_ref))).custom(ButtonCustomVariant::new(cx)).h(px(28.)).w(px(24.)).p_0().flex_shrink_0()
                        .disabled(s.repository_favorites_path.is_none() || s.repository_favorites_saving || s.repository_recent_token.is_some())
                        .tooltip(t(if favorite { "Remove from Favorites" } else { "Add to Favorites" }))
                        .child(jb_icon(icon).text_color(rgb(if favorite { 0xf4af3d } else { FG })))
                        .on_click(cx.listener(move |s,_,_,cx| s.toggle_repository_favorite(favorite_ref.clone(),cx)));
                    let selected=matches!(&s.repository_selection,Some(plico::branch_search::Selection::Reference(section,name)) if *section==row.section && name==&full_ref);
                    h_flex().id(SharedString::from(format!("repository-row:{:?}:{}",row.section,full_ref)))
                        .on_mouse_move(cx.listener(move |s,e,w,cx|s.repository_pointer_selection(e,hover_identity.clone(),w,cx)))
                        .h(px(28.)).w_full().pl(indent).when(selected,|v|v.bg(rgb(0x2e4773))).child(star).child(div().flex_1().child(Button::new(SharedString::from(format!("repository-reference:{:?}:{}", row.section, full_ref))).custom(ButtonCustomVariant::new(cx)).h(px(28.)).w_full()
                        .child(h_flex().w_full().gap_1().child(s.repository_search_label(label))
                            .child(div().flex_1()).child(jb_icon("chevronRight_dark.svg")))
                        .dropdown_menu_with_anchor(Anchor::LeftCenter, move |mut menu,_,menu_cx| {
                            menu=menu.close_on_left(true);
                            let child=menu_cx.entity().downgrade();
                            let _=owner.update(menu_cx,|s,cx| {
                                s.dismiss_repository_child(cx);
                                s.repository_child_selection=Some(plico::branch_search::Selection::Reference(section,full_ref.clone()));
                                s.repository_child_menu=Some(child);
                                s.repository_selection=Some(plico::branch_search::Selection::Reference(section,full_ref.clone()));
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
                                let delete_owner=owner.clone();let delete_ref=full_ref.clone();
                                menu=menu.item(PopupMenuItem::new(t("Delete…")).disabled(disabled || current).on_click(move |_,w,cx| {
                                    let _=delete_owner.update(cx,|s,cx|s.prepare_delete_branch(delete_ref.clone(),w,cx));
                                }));
                            }
                            if !tag && !current {
                                let merge_owner=owner.clone();let merge_ref=full_ref.clone();
                                let enabled=owner.upgrade().is_some_and(|s|s.read(menu_cx).can_merge_branch(&full_ref));
                                menu=menu.item(PopupMenuItem::new(t("Merge into Current")).disabled(!enabled).on_click(move |_,w,cx| {
                                    let _=merge_owner.update(cx,|s,cx|s.merge_branch(merge_ref.clone(),w,cx));
                                }));
                            }
                            let create_owner = owner.clone(); let create_ref = full_ref.clone();
                            menu = menu.item(PopupMenuItem::new(t("New Branch…")).disabled(disabled).on_click(move |_, w, cx| {
                                let _ = create_owner.update(cx, |s, cx| s.new_branch_dialog(create_ref.clone(), w, cx));
                            }));
                            let full_ref = full_ref.clone();
                            menu.item(PopupMenuItem::new(t("Copy Reference Name")).on_click(move |_,_,cx| cx.write_to_clipboard(ClipboardItem::new_string(full_ref.clone()))))
                        }).side_start(true).open(child_open).on_open_change(move |open,_,cx| {
                            let _=change_owner.update(cx,|s,cx| {
                                if *open { s.repository_child_selection=Some(identity.clone()); }
                                else if s.repository_child_selection.as_ref()==Some(&identity) {
                                    s.repository_child_selection=None;
                                    s.repository_child_menu=None;
                                }
                                cx.notify();
                            });
                        })))
                        .context_menu(move |menu,_,cx| {
                            let available=context_owner.upgrade().is_some_and(|s|s.read(cx).reference_checkout_target(&context_ref).is_some());
                            let disabled=context_owner.upgrade().is_none_or(|s|s.read(cx).operation.busy() || s.read(cx).operation.saving_documents());
                            let owner=context_owner.clone();let reference=context_ref.clone();
                            let mut menu=menu;
                            if context_ref.starts_with("refs/heads/") {
                                let delete_owner=context_owner.clone();let delete_ref=context_ref.clone();
                                menu=menu.item(PopupMenuItem::new(t("Delete…")).disabled(disabled || current).on_click(move |_,w,cx| {
                                    let _=delete_owner.update(cx,|s,cx|s.prepare_delete_branch(delete_ref.clone(),w,cx));
                                }));
                            }
                            if !tag && !current {
                                let merge_owner=context_owner.clone();let merge_ref=context_ref.clone();
                                let enabled=context_owner.upgrade().is_some_and(|s|s.read(cx).can_merge_branch(&context_ref));
                                menu=menu.item(PopupMenuItem::new(t("Merge into Current")).disabled(!enabled).on_click(move |_,w,cx| {
                                    let _=merge_owner.update(cx,|s,cx|s.merge_branch(merge_ref.clone(),w,cx));
                                }));
                            }
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
            })).track_scroll(&self.repository_scroll).h(px(count.min(12) as f32 * 28.))))
            .into_any_element()
    }
}
