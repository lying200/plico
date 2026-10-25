//! Main-window branches coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    fn branch_selection_rows(&self, _cx: &App) -> Vec<String> {
        let mut rows = Vec::new();
        if self.head_in_tree() {
            rows.push("HEAD".into());
        }
        rows.extend(self.branch_rows.iter().map(|row| row.key.clone()));
        rows
    }
    fn select_branch_row(&mut self, name: &str, toggle: bool, extend: bool, cx: &App) {
        let rows = self.branch_selection_rows(cx);
        if let Some(index) = rows.iter().position(|row| row == name) {
            if self.branch_selection.ordered(&rows).is_empty() {
                if let Some(anchor) = self
                    .selected_tree_ref
                    .as_ref()
                    .and_then(|id| rows.iter().position(|row| row == id))
                {
                    self.branch_selection.select(&rows, anchor, false, false);
                }
            }
            self.branch_selection.select(&rows, index, toggle, extend);
            let selected = self.branch_selection.ordered(&rows);
            self.selected_tree_ref = if selected.contains(&name) {
                Some(name.to_owned())
            } else {
                selected.last().map(|name| (*name).to_owned())
            };
        }
    }
    fn first_navigatable_tree_ref(&self) -> Option<String> {
        self.branch_selection
            .first_matching(|name| {
                (name == "HEAD" && self.head_in_tree())
                    || self
                        .branch_rows
                        .iter()
                        .any(|row| row.key == name && row.branch.is_some())
            })
            .map(str::to_owned)
    }
    fn selected_branch_pair(&self, cx: &App) -> Option<(String, String)> {
        let rows = self.branch_selection_rows(cx);
        let selected = self.branch_selection.ordered(&rows);
        let [first, second] = selected.as_slice() else {
            return None;
        };
        let resolve = |reference: &str| {
            if reference == "HEAD" {
                self.snapshot
                    .as_ref()?
                    .current_branch
                    .as_ref()
                    .map(|name| format!("refs/heads/{name}"))
            } else if reference.starts_with("refs/heads/") || reference.starts_with("refs/remotes/")
            {
                Some(reference.to_owned())
            } else {
                None
            }
        };
        // Rebased places the selected branch before the current branch for HEAD pairs.
        let (first, second) = if *first == "HEAD" {
            (second, first)
        } else {
            (first, second)
        };
        Some((resolve(first)?, resolve(second)?))
    }
    fn branch_pair_items(
        owner: &WeakEntity<Self>,
        first: String,
        second: String,
    ) -> [PopupMenuItem; 2] {
        let disabled = first == second;
        let files_owner = owner.clone();
        let compare_owner = owner.clone();
        let files_first = first.clone();
        let files_second = second.clone();
        [
            PopupMenuItem::new(t("Compare Branches"))
                .disabled(disabled)
                .on_click(move |_, window, cx| {
                    let _ = compare_owner.update(cx, |s, cx| {
                        s.prepare_branch_pair(first.clone(), second.clone(), window, cx);
                    });
                }),
            PopupMenuItem::new(t("Show Files Diff"))
                .disabled(disabled)
                .on_click(move |_, window, cx| {
                    let _ = files_owner.update(cx, |s, cx| {
                        let view = cx.new(|cx| {
                            crate::compare_view::CompareView::branches(
                                s.repo.clone(),
                                files_first.clone(),
                                files_second.clone(),
                                window,
                                cx,
                            )
                        });
                        s.show_comparison(view, window, cx);
                    });
                }),
        ]
    }
    pub(super) fn rebuild_branches(&mut self, cx: &mut Context<Self>) {
        if let Some(snapshot) = &self.snapshot {
            self.branch_rows = Arc::new(branches::filtered_ref_rows(
                &snapshot.branches,
                &snapshot.tags,
                &self.collapsed_branches,
                &self.branch_search.read(cx).value(),
                snapshot
                    .current_branch
                    .as_ref()
                    .map(|name| format!("refs/heads/{name}"))
                    .as_deref(),
            ));
        }
        self.branch_selection
            .reconcile(&self.branch_selection_rows(cx));
        cx.notify();
    }
    pub(super) fn select_branch(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        self.branch_selection = Default::default();
        self.selected_tree_ref = branch.clone();
        self.branch_filter = branch.into_iter().collect();
        self.local = false;
        self.limit = git::HISTORY_PAGE;
        self.refresh(false, cx);
    }
    pub(super) fn navigate_tree_ref(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.snapshot.as_ref().and_then(|snapshot| {
            (if name == "HEAD" {
                snapshot.head_id.clone()
            } else {
                None
            })
            .or_else(|| {
                snapshot
                    .tags
                    .iter()
                    .find(|r| r.name == name)
                    .map(|r| r.id.clone())
            })
            .or_else(|| {
                snapshot
                    .branches
                    .iter()
                    .find(|r| r.name == name)
                    .map(|r| r.id.clone())
            })
        });
        if let Some(target) = target {
            self.selected_tree_ref = Some(name.to_owned());
            let label = name
                .strip_prefix("refs/tags/")
                .or_else(|| name.strip_prefix("refs/heads/"))
                .or_else(|| name.strip_prefix("refs/remotes/"))
                .unwrap_or(name);
            self.navigate_commit(label.to_owned(), target, false, window, cx);
        }
    }
    pub(super) fn navigate_adjacent(
        &mut self,
        parent: bool,
        keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.log_query.history_loading() || self.settings_open || self.welcome {
            return;
        }
        let ids = self.commit_ids();
        let selected = self.log_selection.ordered(&ids);
        if selected.len() != 1 {
            return;
        }
        let selected = selected[0].to_owned();
        let Some(roots) = self.snapshot.as_ref().map(|s| s.tips.clone()) else {
            return;
        };
        self.adjacent_token.cancel();
        self.adjacent_token = Cancel::default();
        let token = self.adjacent_token.clone();
        let repo = self.repo.clone();
        let generation = self.log_query.stamp();
        let filter = self.text_filter.clone();
        self.adjacent_position = if keyboard {
            let scroll = self.history_scroll.0.borrow();
            let bounds = scroll.base_handle.bounds();
            let index = self.selected.unwrap_or(0);
            let y = bounds.origin.y
                + scroll.base_handle.offset().y
                + px((index + 1) as f32 * log_row_height(cx));
            point(
                bounds.origin.x + px(80.),
                y.max(bounds.origin.y).min(bounds.bottom()),
            )
        } else {
            window.mouse_position()
        };
        self.adjacent_task = cx.spawn_in(window, async move |this, wx| {
            let target = selected.clone();
            let worker_token = token.clone();
            let result = wx
                .background_spawn(async move {
                    history::adjacent(
                        &repo,
                        &roots,
                        &target,
                        parent,
                        filter.as_ref(),
                        &worker_token,
                    )
                })
                .await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    if token.cancelled()
                        || s.log_query.stamp() != generation
                        || s.log_selection.ordered(&s.commit_ids()) != [selected.as_str()]
                    {
                        return;
                    }
                    let targets = match result {
                        Ok(targets) => targets,
                        Err(error) => {
                            s.error = Some(error);
                            cx.notify();
                            return;
                        }
                    };
                    let title = t(if parent {
                        "Select Parent Commit"
                    } else {
                        "Select Child Commit"
                    });
                    if targets.len() == 1 {
                        s.navigate_commit(title.into(), targets[0].clone(), false, window, cx);
                        return;
                    }
                    if targets.is_empty() {
                        return;
                    }
                    let owner = cx.entity().downgrade();
                    let focus = s.history_focus.clone();
                    let labels = targets
                        .into_iter()
                        .map(|target| {
                            let label = s
                                .commits
                                .iter()
                                .find(|c| c.id == target)
                                .map(|c| {
                                    let subject = c.subject.chars().take(40).collect::<String>();
                                    format!(
                                        "{}  {}  {}  {}",
                                        &target[..8],
                                        subject,
                                        c.author,
                                        c.date
                                    )
                                })
                                .unwrap_or_else(|| target[..8].to_owned());
                            (target, label)
                        })
                        .collect::<Vec<_>>();
                    let menu = gpui_kit::component::menu::PopupMenu::build(
                        window,
                        cx,
                        move |mut menu, _, _| {
                            menu = menu
                                .action_context(focus)
                                .max_w(px(620.))
                                .max_h(px(320.))
                                .scrollable(true)
                                .item(PopupMenuItem::label(title));
                            for (index, (target, label)) in labels.into_iter().enumerate() {
                                let label = if index < 10 {
                                    format!("{}.  {}", (index + 1) % 10, label)
                                } else {
                                    label
                                };
                                let owner = owner.clone();
                                let selected = selected.clone();
                                menu = menu.item(PopupMenuItem::new(label).on_click(
                                    move |_, window, cx| {
                                        let _ = owner.update(cx, |s, cx| {
                                            if s.log_query.stamp() == generation
                                                && s.log_selection.ordered(&s.commit_ids())
                                                    == [selected.as_str()]
                                            {
                                                s.navigate_commit(
                                                    title.into(),
                                                    target.clone(),
                                                    false,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        });
                                    },
                                ));
                            }
                            menu.numbered_navigation()
                        },
                    );
                    s.adjacent_subscription =
                        Some(cx.subscribe(&menu, |s, _, _: &DismissEvent, cx| {
                            s.adjacent_menu = None;
                            cx.notify();
                        }));
                    menu.read(cx).focus_handle(cx).focus(window, cx);
                    s.adjacent_menu = Some(menu);
                    cx.notify();
                });
            });
        });
    }
    pub(super) fn navigate_commit(
        &mut self,
        name: String,
        target: String,
        clear_filters: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate_commit_impl(name, target, clear_filters, false, window, cx);
    }
    pub(super) fn navigate_commit_impl(
        &mut self,
        name: String,
        target: String,
        clear_filters: bool,
        fresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !fresh && self.log_query.history_loading() {
            return;
        }
        if !clear_filters && !fresh {
            if let Some(index) = self.commits.iter().position(|commit| commit.id == target) {
                self.local = false;
                self.select_commit(index, cx);
                self.history_scroll
                    .scroll_to_item(index, ScrollStrategy::Top);
                self.history_focus.focus(window, cx);
                return;
            }
        }
        // Cancel old work without clearing the visible comparison. A hidden
        // target must leave the user's current view intact until explicit reset.
        let (generation, token) = self.log_query.begin(log_query::Kind::Navigation);
        self.error = None;
        self.navigation_notice = None;
        self.navigation_loading = Some(name.clone());
        let repo = self.repo.clone();
        let branches = if clear_filters {
            Vec::new()
        } else {
            self.branch_filter.clone()
        };
        let fixed = if clear_filters || fresh {
            None
        } else {
            self.snapshot.as_ref().map(|s| s.tips.clone())
        };
        let filter = if clear_filters {
            None
        } else {
            self.text_filter.clone()
        };
        let limit = self.limit;
        let query_target = target.clone();
        self.log_query.track(
            generation,
            cx.spawn_in(window, async move |this, wx| {
                let result = wx
                    .background_spawn(async move {
                        let roots = match fixed {
                            Some(roots) => roots,
                            None => {
                                repo.snapshot_branches_filtered_at(
                                    &branches, 0, 1, None, None, &token,
                                )?
                                .tips
                            }
                        };
                        let located = match filter {
                            Some(filter) => history::locate_filtered(
                                &repo,
                                &roots,
                                &query_target,
                                limit,
                                &filter,
                                &token,
                            )?,
                            None => {
                                match history::locate(&repo, &roots, &query_target, limit, &token)?
                                {
                                    Some(found) => history::FilteredLocation::Found(found),
                                    None => history::FilteredLocation::OutsideRoots,
                                }
                            }
                        };
                        if matches!(located, history::FilteredLocation::OutsideRoots)
                            && !clear_filters
                        {
                            let all_roots = repo.snapshot(None, 1, None, &token)?.tips;
                            if repo
                                .commit_position(&all_roots, &query_target, &token)?
                                .is_some()
                            {
                                return Ok(history::FilteredLocation::Hidden);
                            }
                        }
                        Ok::<_, String>(located)
                    })
                    .await;
                let _ = wx.update(|window, cx| {
                    let _ = this.update(cx, |s, cx| {
                        if !s.log_query.finish(generation) {
                            return;
                        }
                        s.navigation_loading = None;
                        match result {
                            Ok(history::FilteredLocation::Found(found)) => {
                                s.local = false;
                                if clear_filters {
                                    s.branch_filter.clear();
                                    s.text_filter = None;
                                    s.date_filter = Default::default();
                                    s.filter_search
                                        .update(cx, |input, cx| input.set_value("", window, cx));
                                    s.user_menu.update(cx, |menu, cx| menu.clear_filter(cx));
                                    s.paths_menu.update(cx, |menu, cx| menu.clear_filter(cx));
                                }
                                let history::Window {
                                    mut snapshot,
                                    rows,
                                    start,
                                    end,
                                } = found.window;
                                s.history_offset = found.offset;
                                s.window_start = start;
                                s.window_end = end;
                                s.rows = Arc::new(rows);
                                s.commits = Arc::new(std::mem::take(&mut snapshot.commits));
                                s.snapshot = Some(snapshot);
                                s.update_history_count(cx);
                                s.rebuild_branches(cx);
                                s.select_commit(found.selected, cx);
                                s.history_scroll
                                    .scroll_to_item(found.selected, ScrollStrategy::Top);
                                s.history_focus.focus(window, cx);
                            }
                            Ok(outcome) => {
                                s.navigation_notice = Some(NavigationNotice {
                                    name,
                                    target,
                                    can_clear: matches!(outcome, history::FilteredLocation::Hidden),
                                })
                            }
                            Err(error) => s.error = Some(error),
                        }
                        cx.notify();
                    });
                });
            }),
        );
        cx.notify();
    }
    pub(super) fn set_date_filter(&mut self, after: Option<i64>, cx: &mut Context<Self>) {
        self.date_filter = plico::date_filter::Bounds {
            after,
            before: None,
        };
        self.apply_text_filter(cx);
    }
    pub(super) fn select_date_period(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = cx.new(|cx| crate::date_dialog::DateDialog::new(self.date_filter, window, cx));
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let draft_confirm = draft.clone();
            let owner = owner.clone();
            dialog
                .title(
                    div()
                        .font_weight(FontWeight::NORMAL)
                        .child(t("Select Period")),
                )
                .w(px(514.))
                .min_h(px(132.))
                .margin_top(((window.viewport_size().height - px(132.)) / 2.).max(px(16.)))
                .overlay_closable(false)
                .child(draft.clone())
                .footer(DialogFooter::new().child(div().w(px(72.)).child(
                    DialogAction::new().child(Button::new("date-ok").primary().label(t("OK"))),
                )))
                .on_ok(move |_, _, cx| {
                    let Ok(bounds) = draft_confirm.update(cx, |s, cx| s.confirm(cx)) else {
                        return false;
                    };
                    if !bounds.is_empty() {
                        let _ = owner.update(cx, |s, cx| {
                            s.date_filter = bounds;
                            s.apply_text_filter(cx);
                        });
                    }
                    true
                })
        });
    }
    pub(super) fn date_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let caption = self.date_filter.caption();
        Button::new("date-filter")
            .ghost()
            .small()
            .selected(!self.date_filter.is_empty())
            .child(
                h_flex()
                    .gap_1()
                    .child(caption)
                    .child(jb_icon("chevronDown_dark.svg")),
            )
            .dropdown_menu(move |mut menu, _, _| {
                let select = owner.clone();
                menu = menu.item(PopupMenuItem::new(t("Select…")).on_click(
                    move |_, window, cx| {
                        let _ = select.update(cx, |s, cx| s.select_date_period(window, cx));
                    },
                ));
                let bounds = plico::date_filter::presets(chrono::Local::now());
                for (label, index) in [
                    ("All dates", None),
                    ("Last 24 hours", Some(0)),
                    ("Last 7 days", Some(1)),
                ] {
                    let owner = owner.clone();
                    let bounds = bounds.clone();
                    menu = menu.item(PopupMenuItem::new(t(label)).on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |s, cx| match index {
                            None => s.set_date_filter(None, cx),
                            Some(index) => match &bounds {
                                Ok(bounds) => s.set_date_filter(Some(bounds[index]), cx),
                                Err(error) => {
                                    s.error = Some(error.clone());
                                    cx.notify();
                                }
                            },
                        });
                    }));
                }
                menu
            })
    }
    pub(super) fn branch_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut groups = std::collections::BTreeMap::<String, Vec<(String, String)>>::new();
        if let Some(snapshot) = &self.snapshot {
            for branch in &snapshot.branches {
                if let Some(name) = branch.name.strip_prefix("refs/heads/") {
                    groups
                        .entry(t("Local").into())
                        .or_default()
                        .push((name.into(), branch.name.clone()));
                } else if let Some(name) = branch.name.strip_prefix("refs/remotes/") {
                    if let Some((remote, _)) = name.split_once('/') {
                        groups
                            .entry(format!("{remote}/…"))
                            .or_default()
                            .push((name.into(), branch.name.clone()));
                    }
                }
            }
        }
        let has_head = self.snapshot.as_ref().is_some_and(|s| s.head_id.is_some());
        let owner = cx.entity().downgrade();
        Button::new("branch-filter")
            .ghost()
            .small()
            .child(
                h_flex()
                    .gap_1()
                    .child(if self.branch_filter.is_empty() {
                        t("Branch").to_owned()
                    } else {
                        let names = self
                            .branch_filter
                            .iter()
                            .map(|b| {
                                b.strip_prefix("refs/heads/")
                                    .or_else(|| b.strip_prefix("refs/remotes/"))
                                    .unwrap_or(b)
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("{}: {}", t("Branch"), names)
                    })
                    .child(jb_icon("chevronDown_dark.svg")),
            )
            .selected(!self.branch_filter.is_empty())
            .tooltip(if self.branch_filter.is_empty() {
                t("All branches").to_owned()
            } else {
                self.branch_filter.join(", ")
            })
            .dropdown_menu(move |mut menu, window, cx| {
                let clear = owner.clone();
                menu = menu
                    .min_w(px(250.))
                    .item(
                        PopupMenuItem::new(t("All branches")).on_click(move |_, _, cx| {
                            let _ = clear.update(cx, |s, cx| s.select_branch(None, cx));
                        }),
                    )
                    .separator();
                if has_head {
                    let owner = owner.clone();
                    menu = menu
                        .item(PopupMenuItem::new("HEAD").on_click(move |_, _, cx| {
                            let _ =
                                owner.update(cx, |s, cx| s.select_branch(Some("HEAD".into()), cx));
                        }))
                        .separator();
                }
                for (group, entries) in &groups {
                    let entries = entries.clone();
                    let owner = owner.clone();
                    menu = menu.submenu(group.clone(), window, cx, move |mut submenu, _, _| {
                        submenu = submenu.min_w(px(210.));
                        for (label, name) in &entries {
                            let owner = owner.clone();
                            let name = name.clone();
                            submenu = submenu.item(PopupMenuItem::new(label.clone()).on_click(
                                move |_, _, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        s.select_branch(Some(name.clone()), cx)
                                    });
                                },
                            ));
                        }
                        submenu
                    });
                }
                menu
            })
    }
    pub(super) fn branches_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (label_asset, label_height) = if i18n::is_chinese() {
            ("ui/branches-stripe-zh.svg", 24.)
        } else {
            ("ui/branches-stripe.svg", 60.)
        };
        v_flex()
            .id("branches-rail")
            .w(px(28.))
            .h_full()
            .flex_shrink_0()
            .bg(rgb(PANEL))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                Button::new("toggle-branches")
                    .ghost()
                    .small()
                    .w(px(28.))
                    .h(px(LOG_TOOLBAR_HEIGHT))
                    .icon(jb_icon(if self.branches_visible {
                        "left_dark.svg"
                    } else {
                        "chevronRight_dark.svg"
                    }))
                    .tooltip(t(if self.branches_visible {
                        "Hide branches"
                    } else {
                        "Show branches"
                    }))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.branches_visible = !s.branches_visible;
                        cx.notify();
                    })),
            )
            .when(!self.branches_visible, |rail| {
                rail.child(
                    div()
                        .id("expand-branches-label")
                        .w_full()
                        .h(px(65.))
                        .flex()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(0x292b2e)))
                        .child(
                            svg()
                                .path(label_asset)
                                .w(px(14.))
                                .h(px(label_height))
                                .text_color(rgb(FG)),
                        )
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.branches_visible = true;
                            cx.notify();
                        })),
                )
            })
    }
    pub(super) fn head_in_tree(&self) -> bool {
        // Rebased treats HEAD as a structural node, outside the ref-name filter.
        self.snapshot.as_ref().is_some_and(|s| s.head_id.is_some())
    }
    pub(super) fn filter_selected_tree_ref(&mut self, cx: &mut Context<Self>) {
        let rows = self.branch_selection_rows(cx);
        let mut selected = self.branch_selection.ordered(&rows);
        if selected.is_empty() {
            selected.extend(self.selected_tree_ref.as_deref());
        }
        let branches = selected
            .into_iter()
            .filter(|name| {
                *name == "HEAD"
                    || self
                        .snapshot
                        .as_ref()
                        .is_some_and(|s| s.branches.iter().any(|r| r.name == *name))
            })
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if branches.is_empty() {
            return;
        }
        self.branch_filter = branches;
        self.local = false;
        self.limit = git::HISTORY_PAGE;
        self.refresh(false, cx);
    }
    pub(super) fn tree_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.tree_focus.is_focused(window) {
            return;
        }
        let modifiers = &event.keystroke.modifiers;
        if modifiers.alt
            && !modifiers.control
            && !modifiers.shift
            && !modifiers.platform
            && event.keystroke.key == "f1"
        {
            if let Some(name) = self.first_navigatable_tree_ref() {
                self.navigate_tree_ref(&name, window, cx);
            }
            cx.stop_propagation();
            return;
        }
        let key = event.keystroke.key.as_str();
        if modifiers.alt
            || modifiers.control
            || modifiers.platform
            || (modifiers.shift && !matches!(key, "up" | "down" | "home" | "end"))
        {
            return;
        }
        if key == "f2" {
            // Rebased exposes single-ref actions only for a single tree node.
            if self
                .branch_selection
                .ordered(&self.branch_selection_rows(cx))
                .len()
                > 1
            {
                cx.stop_propagation();
                return;
            }
            if let Some(reference) = self
                .selected_tree_ref
                .clone()
                .filter(|r| r.starts_with("refs/heads/"))
            {
                self.prepare_rename_branch(reference, window, cx);
            }
            cx.stop_propagation();
            return;
        }
        if key == "enter" {
            self.filter_selected_tree_ref(cx);
        } else if matches!(key, "up" | "down" | "home" | "end") {
            let head = self.head_in_tree() as usize;
            let count = self.branch_rows.len() + head;
            if count == 0 {
                return;
            }
            let current = self.selected_tree_ref.as_ref().and_then(|selected| {
                if head == 1 && selected == "HEAD" {
                    Some(0)
                } else {
                    self.branch_rows
                        .iter()
                        .position(|row| &row.key == selected)
                        .map(|i| i + head)
                }
            });
            let next = match key {
                "home" => 0,
                "end" => count - 1,
                "up" => current.map(|i| i.saturating_sub(1)).unwrap_or(count - 1),
                _ => current.map(|i| (i + 1).min(count - 1)).unwrap_or(0),
            };
            self.selected_tree_ref = Some(if head == 1 && next == 0 {
                "HEAD".into()
            } else {
                self.tree_scroll
                    .scroll_to_item(next - head, ScrollStrategy::Top);
                self.branch_rows[next - head].key.clone()
            });
        } else if matches!(key, "left" | "right") {
            let Some(index) = self
                .branch_rows
                .iter()
                .position(|row| Some(&row.key) == self.selected_tree_ref.as_ref())
            else {
                return;
            };
            let row = self.branch_rows[index].clone();
            if row.branch.is_none()
                && ((key == "right" && !row.expanded) || (key == "left" && row.expanded))
            {
                if self.branch_search.read(cx).value().trim().is_empty() {
                    if key == "right" {
                        self.collapsed_branches.remove(&row.key);
                    } else {
                        self.collapsed_branches.insert(row.key);
                    }
                    self.rebuild_branches(cx);
                }
            } else if key == "right" && row.branch.is_none() && row.expanded {
                if let Some(child) = self
                    .branch_rows
                    .get(index + 1)
                    .filter(|child| child.depth > row.depth)
                {
                    self.selected_tree_ref = Some(child.key.clone());
                    self.tree_scroll
                        .scroll_to_item(index + 1, ScrollStrategy::Top);
                }
            } else if key == "left" {
                if let Some(parent) = self.branch_rows[..index]
                    .iter()
                    .rposition(|parent| parent.depth < row.depth)
                {
                    self.selected_tree_ref = Some(self.branch_rows[parent].key.clone());
                    self.tree_scroll.scroll_to_item(parent, ScrollStrategy::Top);
                }
            }
        } else {
            return;
        }
        if matches!(key, "up" | "down" | "home" | "end" | "left" | "right") {
            if let Some(name) = self.selected_tree_ref.clone() {
                self.select_branch_row(&name, false, modifiers.shift, cx);
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn branches(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection_color = row_selection(&self.tree_focus, window);
        let owner = cx.entity().downgrade();
        let head_owner = owner.clone();
        let branches = self.branch_rows.clone();
        let selection = self.branch_selection.clone();
        let selected = self.selected_tree_ref.clone();
        let multiple_selection = !selection
            .ordered(&self.branch_selection_rows(cx))
            .is_empty();
        let current = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.current_branch.as_ref())
            .map(|name| format!("refs/heads/{name}"));
        v_flex()
            .id("branches-tree")
            .track_focus(&self.tree_focus)
            .on_key_down(cx.listener(Self::tree_key))
            .size_full()
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .h(px(LOG_TOOLBAR_HEIGHT))
                    .flex_shrink_0()
                    .px_2()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        Input::new(&self.branch_search)
                            .small()
                            .cleanable(true)
                            .prefix(jb_icon("search_dark.svg")),
                    ),
            )
            .when(self.head_in_tree(), |panel| {
                panel.child(
                    h_flex()
                        .id("head-branch")
                        .relative()
                        .h(px(28.))
                        .w_full()
                        .pl(px(12.))
                        .pr_2()
                        .gap(px(6.))
                        .text_size(cx.theme().font_size)
                        .overflow_hidden()
                        .cursor_pointer()
                        .when(
                            if multiple_selection {
                                selection.contains("HEAD")
                            } else {
                                selected.as_deref() == Some("HEAD")
                            },
                            |row| row.child(tree_selection(selection_color)),
                        )
                        .child(jb_icon("branchLabel_dark.svg"))
                        .child(t("HEAD (Current Branch)"))
                        .on_click(cx.listener(|s, event: &ClickEvent, window, cx| {
                            s.tree_focus.focus(window, cx);
                            s.select_branch_row(
                                "HEAD",
                                event.modifiers().control || event.modifiers().platform,
                                event.modifiers().shift,
                                cx,
                            );
                            if event.click_count() == 2 {
                                s.filter_selected_tree_ref(cx);
                            }
                            cx.notify();
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|s, _, window, cx| {
                                s.tree_focus.focus(window, cx);
                                let rows = s.branch_selection_rows(cx);
                                if let Some(index) = rows.iter().position(|row| row == "HEAD") {
                                    s.branch_selection.context(&rows, index);
                                    s.selected_tree_ref = Some("HEAD".into());
                                }
                                cx.notify();
                            }),
                        )
                        .context_menu(move |mut menu, _, cx| {
                            if let Some(pair) = head_owner
                                .upgrade()
                                .and_then(|s| s.read(cx).selected_branch_pair(cx))
                            {
                                for item in Self::branch_pair_items(&head_owner, pair.0, pair.1) {
                                    menu = menu.item(item);
                                }
                            }
                            menu
                        }),
                )
            })
            .when(
                branches.is_empty()
                    && !self.branch_search.read(cx).value().trim().is_empty()
                    && !(self
                        .snapshot
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.head_id.is_some())
                        && "head (current branch)"
                            .contains(&self.branch_search.read(cx).value().trim().to_lowercase())),
                |d| {
                    d.child(
                        div()
                            .px_3()
                            .py_2()
                            .text_color(rgb(MUTED))
                            .child(t("No matching branches")),
                    )
                },
            )
            .child(
                uniform_list("branches", branches.len(), move |range, _, cx| {
                    range
                        .map(|i| {
                            let branch = branches[i].clone();
                            let owner = owner.clone();
                            let menu_owner = owner.clone();
                            let context_owner = owner.clone();
                            let context_key = branch.key.clone();
                            let menu_ref = branch.branch.clone();
                            let is_current = branch.branch.is_some() && branch.branch == current;
                            let icon = if branch.branch.is_some() {
                                "branchLabel_dark.svg"
                            } else if branch.expanded {
                                "chevronDown_dark.svg"
                            } else {
                                "chevronRight_dark.svg"
                            };
                            div()
                                .relative()
                                .id(i)
                                .w_full()
                                .h(px(28.))
                                .pl(px(12. + branch.depth as f32 * 12.))
                                .pr_2()
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .text_size(cx.theme().font_size)
                                .when(
                                    if multiple_selection {
                                        selection.contains(&branch.key)
                                    } else {
                                        selected.as_ref() == Some(&branch.key)
                                    },
                                    |d| d.child(tree_selection(selection_color)),
                                )
                                .gap(px(6.))
                                .child(
                                    jb_icon(icon).text_color(rgb(if is_current {
                                        // Rebased new/currentBranchLabel_dark.svg uses
                                        // the same label geometry with this gold color.
                                        0xf2c55c
                                    } else if branch
                                        .branch
                                        .as_deref()
                                        .is_some_and(|name| name.starts_with("refs/tags/"))
                                    {
                                        0x868a91
                                    } else {
                                        FG
                                    })),
                                )
                                .when(branch.branch.is_none() && branch.depth > 0, |d| {
                                    d.child(
                                        img("jb/folder_dark_4x.svg").size(px(16.)).flex_shrink_0(),
                                    )
                                })
                                .child(div().truncate().child(match branch.key.as_str() {
                                    "refs/heads" => t("Local").to_owned(),
                                    "refs/remotes" => t("Remote").to_owned(),
                                    "refs/tags" => t("Tags").to_owned(),
                                    _ => branch.label.clone(),
                                }))
                                .on_click(move |event, window, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        s.tree_focus.focus(window, cx);
                                        s.select_branch_row(
                                            &branch.key,
                                            event.modifiers().control || event.modifiers().platform,
                                            event.modifiers().shift,
                                            cx,
                                        );
                                        cx.notify();
                                        if branch.branch.is_some() {
                                            if event.click_count() == 2 {
                                                s.filter_selected_tree_ref(cx);
                                            }
                                        } else if s.branch_search.read(cx).value().trim().is_empty()
                                        {
                                            if !s.collapsed_branches.remove(&branch.key) {
                                                s.collapsed_branches.insert(branch.key.clone());
                                            }
                                            s.rebuild_branches(cx);
                                            cx.notify();
                                        }
                                    });
                                })
                                .on_mouse_down(MouseButton::Right, move |_, window, cx| {
                                    let _ = context_owner.update(cx, |s, cx| {
                                        s.tree_focus.focus(window, cx);
                                        let rows = s.branch_selection_rows(cx);
                                        if let Some(index) =
                                            rows.iter().position(|row| row == &context_key)
                                        {
                                            s.branch_selection.context(&rows, index);
                                            s.selected_tree_ref = Some(context_key.clone());
                                        }
                                        cx.notify();
                                    });
                                })
                                .context_menu(move |mut menu, _, cx| {
                                    let Some(name) = &menu_ref else {
                                        return menu;
                                    };
                                    let name = name.clone();
                                    let owner = menu_owner.clone();
                                    if let Some(view) = owner.upgrade() {
                                        let state = view.read(cx);
                                        let selected = state
                                            .branch_selection
                                            .ordered(&state.branch_selection_rows(cx))
                                            .len();
                                        if selected > 1 && state.branch_selection.contains(&name) {
                                            if let Some((first, second)) =
                                                state.selected_branch_pair(cx)
                                            {
                                                for item in
                                                    Self::branch_pair_items(&owner, first, second)
                                                {
                                                    menu = menu.item(item);
                                                }
                                                return menu;
                                            }
                                            return menu;
                                        }
                                    }
                                    if owner.upgrade().is_some_and(|s| {
                                        s.read(cx).reference_checkout_target(&name).is_some()
                                    }) {
                                        let disabled = owner
                                            .upgrade()
                                            .is_none_or(|s| s.read(cx).operation.busy());
                                        let checkout_owner = owner.clone();
                                        let reference = name.clone();
                                        menu = menu
                                            .item(
                                                PopupMenuItem::new(t("Checkout"))
                                                    .disabled(disabled)
                                                    .on_click(move |_, w, cx| {
                                                        let _ =
                                                            checkout_owner.update(cx, |s, cx| {
                                                                s.checkout_reference(
                                                                    reference.clone(),
                                                                    w,
                                                                    cx,
                                                                )
                                                            });
                                                    }),
                                            )
                                            .separator();
                                    }
                                    if name.starts_with("refs/heads/") {
                                        menu = menu
                                            .item(Review::branch_rename_item(&owner, &name, cx));
                                    }
                                    if name.starts_with("refs/heads/")
                                        || name.starts_with("refs/remotes/")
                                    {
                                        menu = menu
                                            .item(Review::branch_compare_item(&owner, &name, cx))
                                            .item(Review::branch_working_comparison_item(
                                                &owner, &name, cx,
                                            ))
                                            .item(Review::branch_rebase_item(&owner, &name, cx));
                                        let merge_owner = owner.clone();
                                        let merge_ref = name.clone();
                                        let enabled = owner
                                            .upgrade()
                                            .is_some_and(|s| s.read(cx).can_merge_branch(&name));
                                        menu = menu.item(
                                            PopupMenuItem::new(t("Merge into Current"))
                                                .disabled(!enabled)
                                                .on_click(move |_, w, cx| {
                                                    let _ = merge_owner.update(cx, |s, cx| {
                                                        s.merge_branch(merge_ref.clone(), w, cx)
                                                    });
                                                }),
                                        );
                                    }
                                    let filter_owner = owner.clone();
                                    let filter_name = name.clone();
                                    menu.when(!name.starts_with("refs/tags/"), |menu| {
                                        menu.item(
                                            PopupMenuItem::new(t("Update Branch Filter in Log"))
                                                .on_click(move |_, _, cx| {
                                                    let _ = filter_owner.update(cx, |s, cx| {
                                                        s.select_branch(
                                                            Some(filter_name.clone()),
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        )
                                    })
                                    .item(
                                        PopupMenuItem::new(t(
                                            "Navigate Log to Selected Branch Head",
                                        ))
                                        .on_click(
                                            move |_, window, cx| {
                                                let _ = owner.update(cx, |s, cx| {
                                                    s.navigate_tree_ref(&name, window, cx)
                                                });
                                            },
                                        ),
                                    )
                                })
                                .into_any_element()
                        })
                        .collect()
                })
                .track_scroll(&self.tree_scroll)
                .flex_1()
                .min_h_0(),
            )
    }
    // Compact refs follow Rebased GitRefManager / SimpleRefGroup: prefer the
    // current branch and represent each ref type with at most two stacked icons.
    // GraphCommitCellRenderer caps compact labels at one third of the text area.
}

impl Review {
    pub(super) fn apply_text_filter(&mut self, cx: &mut Context<Self>) {
        let text = self.filter_search.read(cx).value().to_string();
        let authors = self.user_menu.read(cx).authors();
        let paths = self.paths_menu.read(cx).paths();
        self.text_filter = if text.trim().is_empty()
            && self.date_filter.is_empty()
            && authors.is_none()
            && paths.is_empty()
        {
            None
        } else {
            Some(git::LogFilter {
                text,
                regex: self.filter_regex,
                match_case: self.filter_match_case,
                after: self.date_filter.after,
                before: self.date_filter.before,
                authors,
                paths,
            })
        };
        self.limit = git::HISTORY_PAGE;
        self.refresh(false, cx);
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod selection_tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;

    #[gpui::test]
    fn branch_search_keeps_head_available_for_comparison(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            view.update(cx, |s, cx| {
                s.branch_search.update(cx, |input, cx| {
                    input.set_value("unmatched-topic", window, cx)
                });
                s.snapshot = Some(Snapshot {
                    head_id: Some("a".repeat(40)),
                    current_branch: Some("main".into()),
                    included_in_head: Default::default(),
                    ref_titles: Default::default(),
                    commits: vec![],
                    branches: vec![],
                    tags: vec![],
                    tips: vec![],
                    more: false,
                    shallow: false,
                });
                assert!(s.head_in_tree());
                assert_eq!(
                    s.branch_selection_rows(cx).first().map(String::as_str),
                    Some("HEAD")
                );
                s.snapshot.as_mut().unwrap().current_branch = None;
                assert!(s.head_in_tree(), "detached HEAD remains navigable");
                s.snapshot.as_mut().unwrap().head_id = None;
                assert!(!s.head_in_tree(), "unborn repository has no HEAD commit");
            });
            Root::new(view, window, cx)
        });
    }

    #[gpui::test]
    fn branch_pair_selection_tracks_identity_and_rejects_non_branches(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        review.unwrap().update(cx, |s, cx| {
            let rows = [
                "refs/heads/a",
                "refs/heads/b",
                "refs/remotes/origin/c",
                "refs/tags/v1",
            ];
            s.branch_rows = Arc::new(
                rows.iter()
                    .map(|name| branches::Row {
                        key: (*name).into(),
                        label: (*name).into(),
                        depth: 1,
                        branch: Some((*name).into()),
                        expanded: false,
                    })
                    .collect(),
            );
            s.select_branch_row(rows[0], false, false, cx);
            s.branch_filter = vec!["refs/heads/existing-filter".into()];
            s.select_branch_row(rows[0], true, false, cx);
            assert!(s.selected_tree_ref.is_none());
            assert!(
                s.branch_selection
                    .ordered(&s.branch_selection_rows(cx))
                    .is_empty()
            );
            s.filter_selected_tree_ref(cx);
            assert_eq!(s.branch_filter, vec!["refs/heads/existing-filter"]);
            s.select_branch_row(rows[2], true, false, cx);
            assert_eq!(
                s.branch_selection.ordered(&s.branch_selection_rows(cx)),
                vec![rows[2]],
                "Ctrl selecting after an empty selection must not resurrect the old branch"
            );
            s.select_branch_row(rows[0], false, false, cx);
            s.select_branch_row(rows[2], true, false, cx);
            assert_eq!(
                s.selected_branch_pair(cx),
                Some((rows[0].into(), rows[2].into()))
            );
            s.select_branch_row(rows[1], true, false, cx);
            assert!(s.selected_branch_pair(cx).is_none());
            s.select_branch_row(rows[0], false, false, cx);
            s.select_branch_row(rows[1], false, true, cx);
            assert_eq!(
                s.selected_branch_pair(cx),
                Some((rows[0].into(), rows[1].into()))
            );
            s.select_branch_row(rows[1], true, false, cx);
            assert!(s.selected_branch_pair(cx).is_none());
            s.select_branch_row(rows[3], true, false, cx);
            assert!(
                s.selected_branch_pair(cx).is_none(),
                "tag is not a branch pair"
            );
            s.snapshot = Some(Snapshot {
                head_id: Some("a".repeat(40)),
                current_branch: Some("a".into()),
                included_in_head: Default::default(),
                ref_titles: Default::default(),
                commits: vec![],
                branches: vec![],
                tags: vec![],
                tips: vec![],
                more: false,
                shallow: false,
            });
            s.select_branch_row(rows[1], false, false, cx);
            s.select_branch_row("HEAD", true, false, cx);
            assert_eq!(s.first_navigatable_tree_ref().as_deref(), Some(rows[1]));
            s.select_branch_row("HEAD", false, false, cx);
            s.select_branch_row(rows[1], true, false, cx);
            assert_eq!(s.first_navigatable_tree_ref().as_deref(), Some("HEAD"));
            assert_eq!(
                s.selected_branch_pair(cx),
                Some((rows[1].into(), rows[0].into())),
                "HEAD pair places the selected branch first"
            );
            s.select_branch_row("HEAD", false, false, cx);
            s.select_branch_row(rows[0], true, false, cx);
            assert_eq!(
                s.selected_branch_pair(cx),
                Some((rows[0].into(), rows[0].into())),
                "same branch remains visible as disabled actions"
            );
            s.snapshot.as_mut().unwrap().current_branch = None;
            assert!(
                s.selected_branch_pair(cx).is_none(),
                "detached HEAD cannot form a branch pair"
            );
            s.snapshot.as_mut().unwrap().current_branch = Some("a".into());
            s.snapshot.as_mut().unwrap().branches = rows[..3]
                .iter()
                .map(|name| git::Branch {
                    name: (*name).into(),
                    id: "a".repeat(40),
                })
                .collect();
            s.select_branch_row(rows[0], false, false, cx);
            s.select_branch_row(rows[2], true, false, cx);
            s.select_branch_row(rows[3], true, false, cx);
            s.filter_selected_tree_ref(cx);
            assert_eq!(
                s.branch_filter,
                vec![rows[0].to_owned(), rows[2].to_owned()]
            );
            assert!(
                s.branch_selection.contains(rows[0]) && s.branch_selection.contains(rows[2]),
                "applying a filter must preserve tree selection"
            );
            s.select_branch(None, cx);
            assert!(s.branch_filter.is_empty());
            s.select_branch(Some(rows[1].into()), cx);
            assert_eq!(s.branch_filter, vec![rows[1].to_owned()]);
            s.snapshot = None;
            s.select_branch_row(rows[1], false, false, cx);
            s.select_branch_row(rows[2], true, false, cx);
            s.branch_rows = Arc::new(
                s.branch_rows
                    .iter()
                    .filter(|row| row.key != rows[1])
                    .cloned()
                    .collect(),
            );
            s.rebuild_branches(cx);
            assert!(!s.branch_selection.contains(rows[1]));
            assert!(s.selected_branch_pair(cx).is_none());
        });
    }
}
