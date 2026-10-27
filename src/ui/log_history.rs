//! Main-window history coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;
use gpui_kit::component::Icon;

impl Review {
    pub(super) fn set_long_edges(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.log_query.history_loading()
            || self.graph_print_loading
            || enabled == self.show_long_edges
        {
            return;
        }
        let Some(model) = self.log.model().clone() else {
            return;
        };
        if self.log.rows().is_empty() {
            return;
        }
        self.graph_print_token.cancel();
        self.graph_print_token = Cancel::default();
        self.graph_print_generation += 1;
        let generation = self.graph_print_generation;
        let token = self.graph_print_token.clone();
        let current = model.clone();
        self.graph_print_loading = true;
        self.graph_print_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { model.project(enabled, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.graph_print_generation
                    || !s
                        .log
                        .model()
                        .as_ref()
                        .is_some_and(|m| Arc::ptr_eq(m, &current))
                {
                    return;
                }
                s.graph_print_loading = false;
                match result {
                    Ok(rows) => {
                        match layout_preferences::save_log_long_edges(&s.repo.path, enabled) {
                            Ok(()) => {
                                s.log.reproject(&current, rows);
                                s.show_long_edges = enabled;
                                s.graph_hover = None;
                            }
                            Err(error) => {
                                s.error =
                                    Some(format!("{}: {error}", t("Settings could not be saved")))
                            }
                        }
                    }
                    Err(error) => s.error = Some(error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    pub(super) fn log_view_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        let checked = self.show_long_edges;
        let disabled = self.log_query.history_loading()
            || self.graph_print_loading
            || self.log.rows().is_empty()
            || self.log.model().is_none();
        Button::new("log-view-options")
            .ghost()
            .small()
            .w(px(28.))
            .flex_shrink_0()
            .icon(jb_icon("history-show_dark.svg"))
            .tooltip(t("View Options"))
            .accessibility_label(t("View Options"))
            .dropdown_menu(move |mut menu, _, cx| {
                if let Some(view) = owner.upgrade() {
                    let current_path = path.clone();
                    let subscription = cx.observe(&view, move |menu, view, cx| {
                        let s = view.read(cx);
                        let checked = s.show_long_edges;
                        let enabled = s.repo.path == current_path
                            && !s.log_query.history_loading()
                            && !s.graph_print_loading
                            && !s.log.rows().is_empty()
                            && s.log.model().is_some();
                        menu.set_item_state(0, checked, enabled, cx);
                    });
                    menu = menu.with_subscription(subscription);
                }
                menu.min_w(px(200.)).close_on_confirm(false).item(
                    PopupMenuItem::new(t("Long Edges"))
                        .checked(checked)
                        .disabled(disabled)
                        .on_click({
                            let owner = owner.clone();
                            let path = path.clone();
                            move |_, _, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    if s.repo.path == path {
                                        s.set_long_edges(!s.show_long_edges, cx);
                                    }
                                });
                            }
                        }),
                )
            })
    }
    pub(super) fn startup_frame(
        &self,
    ) -> Option<(
        Arc<crate::startup_report::Probe>,
        crate::startup_report::Frame,
    )> {
        let probe = self.startup_report.as_ref().filter(|probe| !probe.done())?;
        if !self.welcome
            && (self.snapshot.is_none()
                || self.log_query.history_loading()
                || self.graph_print_loading
                || self.error.is_some())
        {
            return None;
        }
        Some((
            probe.clone(),
            crate::startup_report::Frame {
                welcome: self.welcome,
                requested: self.limit,
                commits: self.log.commits().len(),
                graph_rows: self.log.rows().len(),
                max_columns: self
                    .log
                    .rows()
                    .iter()
                    .map(|row| row.width)
                    .max()
                    .unwrap_or(0),
                offset: self.log.offset(),
                more: self.history_has_more(),
                filtered: !self.branch_filter.is_empty() || self.text_filter.is_some(),
            },
        ))
    }
    pub(super) fn history(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection_color = row_selection(&self.history_focus, window);
        let row_height = log_row_height(cx);
        let root_column = log_root_column::Column::new(self, window, cx);
        let root_scroll = self.history_scroll.clone();
        let root_ids = self.log.ids().clone();
        let checkout_head = self.snapshot.as_ref().and_then(|s| s.head_id.clone());
        let checkout_enabled = !self.operation.busy() && !self.operation.saving_documents();
        let checkout_path = self.repo.path.clone();
        let ref_titles = self
            .snapshot
            .as_ref()
            .map(|s| s.ref_titles.clone())
            .unwrap_or_default();
        let project_ref_titles = self.log.scope().as_ref().map(|scope| {
            scope
                .roots
                .iter()
                .map(|root| (root.query.repository.path.clone(), root.ref_titles.clone()))
                .collect::<std::collections::HashMap<_, _>>()
        });
        let commits = self.log.commits().clone();
        let rows = self.log.rows().clone();
        let graph_hover = self.graph_hover;
        let selected = self.log_selection.clone();
        let sources = self.log.sources().cloned();
        let root = Arc::new(self.repo.path.clone());
        let owner = cx.entity().downgrade();
        let recommended_width = rows.iter().map(|r| r.width).max().unwrap_or(1);
        let included = self
            .snapshot
            .as_ref()
            .map(|s| s.included_in_head.clone())
            .unwrap_or_default();
        v_flex()
            .size_full()
            .min_w_0()
            .bg(rgb(BG))
            .when_some(self.startup_frame(), |v, (probe, frame)| v.on_prepaint(move |_, _, _| probe.record(frame)))
            .child(
                h_flex()
                    .min_w_0().w_full()
                    .on_prepaint(observe_review_width(cx.entity().downgrade(), false))
                    .h(px(LOG_TOOLBAR_HEIGHT))
                    .flex_shrink_0()
                    .px_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(div().flex_1().min_w_0().max_w(px(260.)).child(
                        Input::new(&self.filter_search).small()
                            .prefix(jb_icon("search_dark.svg"))
                            .suffix(h_flex()
                                .child(Button::new("filter-regex").ghost().small().label(".*")
                                    .selected(self.filter_regex).tooltip(t("Regular expression"))
                                    .on_click(cx.listener(|s, _, _, cx| { s.filter_regex = !s.filter_regex; s.apply_text_filter(cx); })))
                                .child(Button::new("filter-match-case").ghost().small().label("Cc")
                                    .selected(self.filter_match_case).tooltip(t("Match case"))
                                    .on_click(cx.listener(|s, _, _, cx| { s.filter_match_case = !s.filter_match_case; s.apply_text_filter(cx); }))))))
                    .when(self.history_toolbar_width >= px(360.), |v| v.child(self.branch_filter_control(cx)))
                    .when(self.history_toolbar_width >= px(650.), |v| v.child(self.user_menu.clone())
                        .child(self.date_filter_control(cx)).child(self.paths_menu.clone()))
                    .when(self.history_toolbar_width < px(650.), |v| v.child(self.filter_overflow(self.history_toolbar_width < px(360.),cx)))
                    .child(self.log_view_options(cx)),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .pb(px(12.))
                    .child(
                        div()
                            .id("history-horizontal")
                            .size_full()
                            .overflow_x_scroll()
                            .restrict_scroll_to_axis()
                            .overflow_y_hidden()
                            .track_scroll(&self.history_horizontal)
                            .child(
                                v_flex()
                                    .h_full()
                                    .w_full()
                                    .min_w(px(736.))
                                    .child(
                                        div()
                                            .relative()
                                            .flex_1()
                                            .min_h_0()
                                            .track_focus(&self.history_focus)
                                            .key_context("History")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|s, _, w, cx| {
                                                    s.history_focus.focus(w, cx)
                                                }),
                                            )
                                            .child(
                                                uniform_list(
                                                    "history",
                                                    commits.len(),
                                                    move |range, _, _| {
                                                        let offset = root_scroll.0.borrow().base_handle.offset().y;
                                                        let first_readable = ((-f32::from(offset) / row_height - 0.5).ceil().max(0.)) as usize;
                                                        range
                                                            .map(|i| {
                                                                let c = &commits[i];
                                                                let is_selected = match &sources {
                                                                    Some(ids) => ids.get(i).is_some_and(|id| id.hash == c.id && selected.contains(id)),
                                                                    None => selected.contains(&plico::project_log::CommitId { root: root.clone(), hash: c.id.clone() }),
                                                                };
                                                                let owner = owner.clone();
                                                                let checkout_owner = owner.clone();
                                                                let context_owner = owner.clone();
                                                                let checkout_target = c.id.clone();
                                                                let checkout_head = checkout_head.clone();
                                                                let checkout_path = checkout_path.clone();
                                                                h_flex()
                                        .id(i)
                                        .relative()
                                        .w_full()
                                        .h(px(row_height))
                                        .px_2()
                                        .gap_2()
                                        .when(root_column.is_some(), |d| d.pl_0().gap_0())
                                        .min_w(px(736.))
                                        .when(included.contains(&c.id), |d| d.bg(rgb(0x1d2336)))
                                        // VcsLogNewUiCellWrapper: single-repository selection
                                        // has 4px side insets and Popup.Selection.arc=8 (diameter).
                                        .when(is_selected, |d| d.child(
                                            div().absolute().left(root_column.as_ref().map_or(px(4.), |c| c.width())).right(px(4.)).top_0().h_full()
                                                .rounded(px(4.)).bg(rgb(selection_color))
                                        ))
                                        .when_some(root_column.as_ref(), |d, column| d.child(
                                            column.cell(i, &root_ids, first_readable, owner.clone(), if included.contains(&c.id) { 0x1d2336 } else { BG })
                                        ))
                                        .child(h_flex().flex_1().min_w_0().overflow_hidden().gap_2()
                                            .when(root_column.is_some(), |d| d.mr_2())
                                            .child(interactive_graph_cell(rows[i].clone(), graph::text_offset_columns(&rows[i], recommended_width) * 16. + 12., row_height, i,
                                                GraphNodeStyle { head: checkout_head.as_deref() == Some(c.id.as_str()),
                                                    background: if is_selected { selection_color } else if included.contains(&c.id) { 0x1d2336 } else { BG } },
                                                owner.clone(), rows.clone(), commits.clone(), graph_hover))
                                        .child(
                                            h_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .overflow_hidden()
                                                .gap_2()
                                                .child(div().flex_1().min_w_0().truncate().child(c.subject.clone()))
                                                .when(c.shallow_boundary, |d| d.child(
                                                    div().px_1().flex_shrink_0().text_size(px(12.))
                                                        .text_color(rgb(0xe2b765)).bg(rgb(0x443b27))
                                                        .rounded_sm().child(t("Shallow boundary"))
                                                ))
                                                .when(!c.refs.is_empty(), |d| d.child(
                                                    h_flex()
                                                        .id(("commit-refs", i))
                                                        .max_w(relative(1. / 3.))
                                                        .min_w_0()
                                                        .gap_1()
                                                        .overflow_hidden()
                                                        .tooltip({
                                                            let refs = c.refs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join("\n");
                                                            move |window, cx| Tooltip::new(refs.clone()).build(window, cx)
                                                        })
                                                        .child(
                                                            reference_icon(
                                                                [git::RefKind::Head, git::RefKind::Local, git::RefKind::Remote, git::RefKind::Tag]
                                                                    .into_iter().flat_map(|kind| std::iter::repeat_n(ref_color(kind), c.refs.iter().filter(|r| r.kind == kind).count().min(2))).collect(),
                                                                if is_selected { selection_color } else if included.contains(&c.id) { 0x1d2336 } else { BG },
                                                            )
                                                        )
                                                        .child(div().min_w_0().truncate().text_color(rgb(MUTED)).child({
                                                            match &sources {
                                                                Some(ids) => ids.get(i).filter(|id| id.hash == c.id)
                                                                    .and_then(|id| project_ref_titles.as_ref()?.get(id.root.as_ref()))
                                                                    .and_then(|titles| titles.get(&c.id)).cloned().unwrap_or_default(),
                                                                None => ref_titles.get(&c.id).cloned().unwrap_or_default(),
                                                            }
                                                        }))
                                                )),
                                        ))
                                        .child(
                                            div()
                                                .w(px(110.)).flex_shrink_0()
                                                .when(root_column.is_some(), |d| d.mr_2())
                                                .text_color(rgb(MUTED))
                                                .truncate()
                                                .child(c.author.clone()),
                                        )
                                        .child(
                                            div()
                                                .w(px(145.)).flex_shrink_0()
                                                .truncate()
                                                .text_color(rgb(MUTED))
                                                .child(plico::commit_date::format(&c.date)),
                                        )
                                        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                            let _ = owner.update(cx, |s, cx| s.select_log_row(i, event.modifiers.control, event.modifiers.shift, false, cx));
                                        })
                                        .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                                            let _ = context_owner.update(cx, |s, cx| s.select_log_row(i, false, false, true, cx));
                                        })
                                        .context_menu(move |menu, window, cx| {
                                            let checkout_enabled = checkout_enabled && checkout_owner.upgrade().is_some_and(|s| s.read(cx).log_selection.ordered(&s.read(cx).commit_ids()).len() == 1);
                                            let owner = checkout_owner.clone();
                                            let target = checkout_target.clone();
                                            let checkout_repository = checkout_owner.upgrade().and_then(|e| e.read(cx).log_repository(i));
                                            let checkout_enabled = checkout_enabled && checkout_repository.is_some();
                                            let path = checkout_path.clone();
                                            let hashes = checkout_owner.upgrade().map(|s| {
                                                let s = s.read(cx); let ids = s.commit_ids();
                                                let mut hashes = s.log_selection.ordered(&ids); hashes.reverse(); hashes.iter().map(|id| id.hash.as_str()).collect::<Vec<_>>().join(" ")
                                            }).unwrap_or_default();
                                            let interactive_owner=checkout_owner.clone();let interactive_target=target.clone();let interactive_path=path.clone();
                                            let interactive_repository=checkout_repository.clone();
                                            let interactive_enabled=checkout_owner.upgrade().is_some_and(|e|e.read(cx).can_interactive_rebase(i, cx));
                                            let reword_owner = checkout_owner.clone();
                                            let reword_enabled = checkout_owner.upgrade().is_some_and(|e| e.read(cx).can_reword(&target, cx));
                                            let create_owner=checkout_owner.clone(); let create_target=target.clone(); let create_path=path.clone();
                                            let create_repository=checkout_owner.upgrade().and_then(|e| e.read(cx).log_repository(i));
                                            let tag_owner=checkout_owner.clone(); let tag_target=target.clone(); let tag_path=path.clone();
                                            let tag_repository=checkout_owner.upgrade().and_then(|e| e.read(cx).log_repository(i));
                                            let reset_owner=checkout_owner.clone(); let reset_target=target.clone(); let reset_path=path.clone();
                                            let reset_repository=checkout_owner.upgrade().and_then(|e| e.read(cx).log_repository(i));
                                            let reset_enabled=checkout_owner.upgrade().is_some_and(|e|{let s=e.read(cx);!s.operation.busy() && !s.operation.saving_documents() && s.log_selection.ordered(&s.commit_ids()).len()==1});
                                            let revert_owner=checkout_owner.clone(); let revert_path=path.clone();
                                            let revert_targets = checkout_owner.upgrade().map(|e| {
                                                let s=e.read(cx); s.log_selection.ordered(&s.commit_ids()).into_iter().cloned().collect::<Vec<_>>()
                                            }).unwrap_or_default();
                                            let revert_enabled=checkout_owner.upgrade().is_some_and(|e| {
                                                let s=e.read(cx); !s.operation.busy() && !s.operation.saving_documents()
                                                    && s.can_sequence_selection(&revert_targets)
                                            });
                                            let cherry_owner = checkout_owner.clone();
                                            let cherry_path = path.clone();
                                            let cherry_targets = revert_targets.clone();
                                            let parent_owner = checkout_owner.clone(); let child_owner = checkout_owner.clone();
                                            let adjacent_enabled = checkout_owner.upgrade().is_some_and(|e| {
                                                let s = e.read(cx); !s.log_query.history_loading() && s.log_selection.ordered(&s.commit_ids()).len() == 1
                                            });
                                            let compare_owner = checkout_owner.clone();
                                            let compare_enabled = checkout_owner.upgrade().is_some_and(|e| e.read(cx).selected_comparison().is_some());
                                            let squash_owner = checkout_owner.clone();
                                            let squash_enabled = checkout_owner.upgrade().is_some_and(|entity| {
                                                let s = entity.read(cx); !s.operation.busy() && s.squash_selection_is_single_root()
                                            });
                                            menu.item(PopupMenuItem::new(t("Copy Revision Number")).disabled(hashes.is_empty()).on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(hashes.clone()));
                                            })).item(PopupMenuItem::new(t("Cherry-Pick")).icon(Icon::default().path("jb/cherryPick_dark.svg")).disabled(!revert_enabled).on_click(move |_, w, cx| {
                                                let _ = cherry_owner.update(cx, |s, cx| if s.repo.path == cherry_path {
                                                    s.start_log_sequence(plico::operation_state::Operation::CherryPick, cherry_targets.clone(), w, cx);
                                                });
                                            })).when(compare_enabled, |menu| menu.item(PopupMenuItem::new(t("Compare Versions")).on_click(move |_, w, cx| {
                                                let _ = compare_owner.update(cx, |s, cx| s.compare_versions(w, cx));
                                            }))).separator().item(PopupMenuItem::new(t("Squash Commits…")).disabled(!squash_enabled)
                                                .on_click(move |_, window, cx| { let _ = squash_owner.update(cx, |s, cx| s.prepare_squash(window, cx)); }))
                                                .separator().when(true, |menu| {
                                                    let branches = checkout_owner.upgrade().map(|e| e.read(cx).log_checkout_branches(i)).unwrap_or_default();
                                                    if branches.is_empty() {
                                                        menu.item(PopupMenuItem::new(format!("{} {}", t("Checkout Revision"), &target[..8])).disabled(!checkout_enabled)
                                                            .on_click(move |_, window, cx| { let _ = owner.update(cx, |s, cx| {
                                                                if s.repo.path == path { if let Some(repo) = &checkout_repository { s.start_log_checkout(repo.clone(), target.clone(), None, window, cx); } }
                                                            }); }))
                                                    } else {
                                                        menu.submenu(t("Checkout"), window, cx, move |mut sub, _, _| {
                                                            for reference in &branches {
                                                                let reference = reference.clone(); let owner = owner.clone(); let path = path.clone();
                                                                let repository = checkout_repository.clone(); let target = target.clone();
                                                                sub = sub.item(PopupMenuItem::new(reference.strip_prefix("refs/heads/").unwrap_or(&reference).to_owned()).disabled(!checkout_enabled)
                                                                    .on_click(move |_, w, cx| { let _ = owner.update(cx, |s, cx| {
                                                                        if s.repo.path == path { if let Some(repo) = &repository { s.start_log_checkout(repo.clone(), target.clone(), Some(reference.clone()), w, cx); } }
                                                                    }); }));
                                                            }
                                                            let owner = owner.clone(); let target = target.clone(); let path = path.clone();
                                                            let checkout_repository = checkout_repository.clone();
                                                            sub.item(PopupMenuItem::new(t("Checkout Revision")).disabled(!checkout_enabled)
                                                                .on_click(move |_, w, cx| { let _ = owner.update(cx, |s, cx| {
                                                                    if s.repo.path == path { if let Some(repo) = &checkout_repository { s.start_log_checkout(repo.clone(), target.clone(), None, w, cx); } }
                                                                }); }))
                                                        })
                                                    }
                                                })
                                                .separator()
                                                .item(PopupMenuItem::new(t("New Branch…")).disabled(!reset_enabled || create_repository.is_none()).on_click(move |_,w,cx| {
                                                    let _=create_owner.update(cx,|s,cx|if s.repo.path==create_path {if let Some(repo)=&create_repository{s.new_log_branch_dialog(repo.clone(),create_target.clone(),w,cx);}});
                                                }))
                                                .item(PopupMenuItem::new(t("New Tag…")).disabled(!reset_enabled || tag_repository.is_none()).on_click(move |_,w,cx| {
                                                    let _=tag_owner.update(cx,|s,cx|if s.repo.path==tag_path {if let Some(repo)=&tag_repository {s.new_tag_dialog(repo.clone(),tag_target.clone(),w,cx);}});
                                                }))
                                                .item(PopupMenuItem::new(t("Reset Current Branch to Here…")).disabled(!reset_enabled).on_click(move|_,w,cx| {
                                                    let _=reset_owner.update(cx,|s,cx|{if s.repo.path==reset_path{if let Some(repo)=&reset_repository { s.prepare_reset(repo.clone(),reset_target.clone(),w,cx); }}});
                                                }))
                                                .item(PopupMenuItem::new(t(if revert_targets.len() == 1 { "Revert Commit" } else { "Revert Commits" })).disabled(!revert_enabled).on_click(move |_,w,cx| {
                                                    let _=revert_owner.update(cx,|s,cx|if s.repo.path==revert_path {s.start_log_sequence(plico::operation_state::Operation::Revert,revert_targets.clone(),w,cx);});
                                                })).separator()
                                                .item(PopupMenuItem::new(t("Edit Commit Message…")).action(Box::new(EditCommitMessage)).disabled(!reword_enabled).on_click(move |_, w, cx| {
                                                    let _ = reword_owner.update(cx, |s, cx| s.edit_commit_message(w, cx));
                                                }))
                                                .item(PopupMenuItem::new(t("Interactively Rebase from Here…")).disabled(!interactive_enabled).on_click(move |_,w,cx|{
                                                    let _=interactive_owner.update(cx,|s,cx|if s.repo.path==interactive_path {if let Some(repo)=&interactive_repository{s.prepare_interactive_rebase(repo.clone(),interactive_target.clone(),w,cx);}});
                                                }))
                                                .item(PopupMenuItem::new(t("Go to Child Commit")).disabled(!adjacent_enabled).on_click(move |_, w, cx| {
                                                    let _ = child_owner.update(cx, |s, cx| s.navigate_adjacent(false, false, w, cx));
                                                }))
                                                .item(PopupMenuItem::new(t("Go to Parent Commit")).disabled(!adjacent_enabled).on_click(move |_, w, cx| {
                                                    let _ = parent_owner.update(cx, |s, cx| s.navigate_adjacent(true, false, w, cx));
                                                }))
                                        })
                                        .into_any_element()
                                                            })
                                                            .collect()
                                                    },
                                                )
                                                .size_full()
                                                .map(|mut list| {
                                                    list.style().restrict_scroll_to_axis =
                                                        Some(true);
                                                    list
                                                })
                                                .track_scroll(&self.history_scroll),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom(px(12.))
                            .right_0()
                            .w(px(12.))
                            .child(
                                Scrollbar::vertical(&self.history_scroll).viewport_from_layout(),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .h(px(12.))
                            .child(
                                Scrollbar::horizontal(&self.history_horizontal)
                                    .viewport_from_layout(),
                            ),
                    )
                    .when(self.log.commits().is_empty() && self.error.is_none() && !self.log_query.history_loading()
                        && self.paths_menu.read(cx).selected_roots().is_some(), |v| v.child(
                        v_flex().absolute().inset_0().items_center().justify_center().gap_2().p_4().bg(rgb(BG))
                            .child(t("No commits matching filters"))
                            .child(Button::new("reset-project-log-filters").link().small().label(t("Reset filters"))
                                .on_click(cx.listener(|s, _, window, cx| {
                                    s.branch_filter.clear();
                                    s.date_filter = Default::default();
                                    s.filter_search.update(cx, |input, cx| input.set_value("", window, cx));
                                    s.user_menu.update(cx, |menu, cx| menu.clear_filter(cx));
                                    s.paths_menu.update(cx, |menu, cx| menu.clear_filter(cx));
                                    s.apply_text_filter(cx);
                                })))
                    ))
                    .when(self.log.commits().is_empty() && self.error.is_some(), |v| v.child(
                        v_flex().absolute().inset_0().items_center().justify_center().gap_2().p_4().bg(rgb(BG))
                            .child(t(plico::git_error::summary(self.error.as_deref().unwrap_or_default())))
                            .child(h_flex().gap_3()
                                .child(Button::new("retry-repository-load").link().small().label(t("Refresh"))
                                    .disabled(self.log_query.busy() || self.log_query.history_loading()).on_click(cx.listener(|s, _, w, cx| s.refresh_current(w, cx))))
                                .child(Button::new("repository-load-details").link().small().label(t("Details"))
                                    .on_click(cx.listener(|s, _, w, cx| s.show_error_details(w, cx)))))
                    )),
            )
            .child(self.history_footer(cx))
    }
}

fn interactive_graph_cell(
    row: Row,
    width: f32,
    row_height: f32,
    index: usize,
    node_style: GraphNodeStyle,
    owner: WeakEntity<Review>,
    rows: Arc<Vec<Row>>,
    commits: Arc<Vec<Commit>>,
    hovered: Option<(usize, usize)>,
) -> AnyElement {
    let hovered = hovered
        .filter(|(r, _)| *r == index)
        .map(|(_, segment)| segment);
    let tooltip = hovered.and_then(|segment| {
        row.segments.get(segment).filter(|s| s.arrow()).map(|_| {
            row.arrow_target(segment)
                .and_then(|target| commits.get(target))
                .map(|commit| {
                    graph_arrow_tooltip(
                        &commit.id,
                        &commit.subject,
                        &commit.author,
                        &plico::commit_date::format(&commit.date),
                    )
                })
                .unwrap_or_else(|| t("Commit is outside the loaded history window").into())
        })
    });
    let hover_owner = owner.clone();
    let hover_rows = rows.clone();
    graph_arrow_cell(
        row,
        width,
        row_height,
        index,
        node_style,
        hovered,
        tooltip,
        move |segment, cx| {
            let _ = hover_owner.update(cx, |s, cx| {
                if !Arc::ptr_eq(&hover_rows, &s.log.rows())
                    || s.log_query.history_loading()
                    || s.graph_print_loading
                {
                    return;
                }
                if segment.is_none() && s.graph_hover.is_some_and(|(row, _)| row != index) {
                    return;
                }
                let hovered = segment.map(|segment| (index, segment));
                if s.graph_hover != hovered {
                    s.graph_hover = hovered;
                    cx.notify();
                }
            });
        },
        move |target, window, cx| {
            let _ = owner.update(cx, |s, cx| {
                if !Arc::ptr_eq(&rows, &s.log.rows())
                    || s.log_query.history_loading()
                    || s.graph_print_loading
                    || target >= s.log.commits().len()
                {
                    return;
                }
                s.local = false;
                s.graph_hover = None;
                s.select_commit(target, cx);
                s.history_scroll
                    .scroll_to_item(target, ScrollStrategy::Nearest);
                s.history_focus.focus(window, cx);
            });
        },
    )
}
