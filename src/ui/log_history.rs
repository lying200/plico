//! Main-window history coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn set_long_edges(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.history_loading || self.graph_print_loading || enabled == self.show_long_edges {
            return;
        }
        let Some(model) = self.graph_model.clone() else {
            return;
        };
        if self.rows.is_empty() {
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
                        .graph_model
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
                                s.rows = Arc::new(rows);
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
        let disabled = self.history_loading
            || self.graph_print_loading
            || self.rows.is_empty()
            || self.graph_model.is_none();
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
                            && !s.history_loading
                            && !s.graph_print_loading
                            && !s.rows.is_empty()
                            && s.graph_model.is_some();
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
                || self.history_loading
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
                commits: self.commits.len(),
                graph_rows: self.rows.len(),
                max_columns: self.rows.iter().map(|row| row.width).max().unwrap_or(0),
                offset: self.history_offset,
                more: self.snapshot.as_ref().is_some_and(|s| s.more),
                filtered: self.branch.is_some() || self.text_filter.is_some(),
            },
        ))
    }
    pub(super) fn history(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection_color = row_selection(&self.history_focus, window);
        let row_height = log_row_height(cx);
        let checkout_head = self.snapshot.as_ref().and_then(|s| s.head_id.clone());
        let checkout_enabled = !self.operation.busy() && checkout_head.is_some();
        let checkout_path = self.repo.path.clone();
        let ref_titles = self
            .snapshot
            .as_ref()
            .map(|s| s.ref_titles.clone())
            .unwrap_or_default();
        let commits = self.commits.clone();
        let rows = self.rows.clone();
        let graph_hover = self.graph_hover;
        let selected = self.log_selection.clone();
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
                                                        range
                                                            .map(|i| {
                                                                let c = &commits[i];
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
                                        .min_w(px(736.))
                                        .when(included.contains(&c.id), |d| d.bg(rgb(0x1d2336)))
                                        // VcsLogNewUiCellWrapper: single-repository selection
                                        // has 4px side insets and Popup.Selection.arc=8 (diameter).
                                        .when(selected.contains(&c.id), |d| d.child(
                                            div().absolute().left(px(4.)).right(px(4.)).top_0().h_full()
                                                .rounded(px(4.)).bg(rgb(selection_color))
                                        ))
                                        .child(h_flex().flex_1().min_w_0().overflow_hidden().gap_2()
                                            .child(interactive_graph_cell(rows[i].clone(), graph::text_offset_columns(&rows[i], recommended_width) * 16. + 12., row_height, i,
                                                GraphNodeStyle { head: checkout_head.as_deref() == Some(c.id.as_str()),
                                                    background: if selected.contains(&c.id) { selection_color } else if included.contains(&c.id) { 0x1d2336 } else { BG } },
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
                                                                if selected.contains(&c.id) { selection_color } else if included.contains(&c.id) { 0x1d2336 } else { BG },
                                                            )
                                                        )
                                                        .child(div().min_w_0().truncate().text_color(rgb(MUTED)).child({
                                                            ref_titles.get(&c.id).cloned().unwrap_or_default()
                                                        }))
                                                )),
                                        ))
                                        .child(
                                            div()
                                                .w(px(110.)).flex_shrink_0()
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
                                            let head = checkout_head.clone();
                                            let path = checkout_path.clone();
                                            let hashes = checkout_owner.upgrade().map(|s| {
                                                let s = s.read(cx); let ids = s.commit_ids();
                                                let mut hashes = s.log_selection.ordered(&ids); hashes.reverse(); hashes.join(" ")
                                            }).unwrap_or_default();
                                            let reword_owner = checkout_owner.clone();
                                            let reword_enabled = checkout_owner.upgrade().is_some_and(|e| e.read(cx).can_reword(&target, cx));
                                            let create_owner=checkout_owner.clone(); let create_target=target.clone(); let create_path=path.clone();
                                            let reset_owner=checkout_owner.clone(); let reset_target=target.clone(); let reset_path=path.clone();
                                            let reset_enabled=checkout_owner.upgrade().is_some_and(|e|{let s=e.read(cx);!s.operation.busy() && !s.operation.saving_documents() && s.log_selection.ordered(&s.commit_ids()).len()==1});
                                            let parent_owner = checkout_owner.clone(); let child_owner = checkout_owner.clone();
                                            let adjacent_enabled = checkout_owner.upgrade().is_some_and(|e| {
                                                let s = e.read(cx); !s.history_loading && s.log_selection.ordered(&s.commit_ids()).len() == 1
                                            });
                                            let compare_owner = checkout_owner.clone();
                                            let compare_enabled = checkout_owner.upgrade().is_some_and(|e| e.read(cx).log_selection.ordered(&e.read(cx).commit_ids()).len() == 2);
                                            let squash_owner = checkout_owner.clone();
                                            let squash_enabled = checkout_owner.upgrade().is_some_and(|entity| {
                                                let s = entity.read(cx); !s.operation.busy() && s.log_selection.ordered(&s.commit_ids()).len() >= 2
                                            });
                                            menu.item(PopupMenuItem::new(t("Copy Revision Number")).disabled(hashes.is_empty()).on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(hashes.clone()));
                                            })).when(compare_enabled, |menu| menu.item(PopupMenuItem::new(t("Compare Versions")).on_click(move |_, w, cx| {
                                                let _ = compare_owner.update(cx, |s, cx| s.compare_versions(w, cx));
                                            }))).separator().item(PopupMenuItem::new(t("Squash Commits…")).disabled(!squash_enabled)
                                                .on_click(move |_, window, cx| { let _ = squash_owner.update(cx, |s, cx| s.prepare_squash(window, cx)); }))
                                                .separator().when(true, |menu| {
                                                    let branches = checkout_owner.upgrade().and_then(|e| {
                                                        let s = e.read(cx);
                                                        s.snapshot.as_ref().map(|snapshot| snapshot.branches.iter().map(|b|(&b.name,&b.id))
                                                            .chain(snapshot.tags.iter().map(|t|(&t.name,&t.id)))
                                                            .filter(|(name,id)| **id==target && s.reference_checkout_target(name).is_some())
                                                            .map(|(name,_)|name.clone()).collect::<Vec<_>>())
                                                    }).unwrap_or_default();
                                                    if branches.is_empty() {
                                                        menu.item(PopupMenuItem::new(format!("{} {}", t("Checkout Revision"), &target[..8])).disabled(!checkout_enabled)
                                                            .on_click(move |_, window, cx| { let _ = owner.update(cx, |s, cx| {
                                                                if s.repo.path == path { if let Some(head) = &head { s.start_checkout(target.clone(), head.clone(), window, cx); } }
                                                            }); }))
                                                    } else {
                                                        menu.submenu(t("Checkout"), window, cx, move |mut sub, _, _| {
                                                            for reference in &branches {
                                                                let reference = reference.clone(); let owner = owner.clone(); let path = path.clone();
                                                                sub = sub.item(PopupMenuItem::new(reference.strip_prefix("refs/heads/").unwrap_or(&reference).to_owned()).disabled(!checkout_enabled)
                                                                    .on_click(move |_, w, cx| { let _ = owner.update(cx, |s, cx| {
                                                                        if s.repo.path == path { s.checkout_reference(reference.clone(), w, cx); }
                                                                    }); }));
                                                            }
                                                            let owner = owner.clone(); let target = target.clone(); let head = head.clone(); let path = path.clone();
                                                            sub.item(PopupMenuItem::new(t("Checkout Revision")).disabled(!checkout_enabled)
                                                                .on_click(move |_, w, cx| { let _ = owner.update(cx, |s, cx| {
                                                                    if s.repo.path == path { if let Some(head) = &head { s.start_checkout(target.clone(), head.clone(), w, cx); } }
                                                                }); }))
                                                        })
                                                    }
                                                })
                                                .separator()
                                                .item(PopupMenuItem::new(t("New Branch…")).disabled(!reset_enabled).on_click(move |_,w,cx| {
                                                    let _=create_owner.update(cx,|s,cx|if s.repo.path==create_path {s.new_branch_dialog(create_target.clone(),w,cx);});
                                                }))
                                                .item(PopupMenuItem::new(t("Reset Current Branch to Here…")).disabled(!reset_enabled).on_click(move|_,w,cx| {
                                                    let _=reset_owner.update(cx,|s,cx|{if s.repo.path==reset_path{s.prepare_reset(reset_target.clone(),w,cx);}});
                                                })).separator()
                                                .item(PopupMenuItem::new(t("Edit Commit Message…")).action(Box::new(EditCommitMessage)).disabled(!reword_enabled).on_click(move |_, w, cx| {
                                                    let _ = reword_owner.update(cx, |s, cx| s.edit_commit_message(w, cx));
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
                    .when(self.commits.is_empty() && self.error.is_some(), |v| v.child(
                        v_flex().absolute().inset_0().items_center().justify_center().gap_2().p_4().bg(rgb(BG))
                            .child(t(plico::git_error::summary(self.error.as_deref().unwrap_or_default())))
                            .child(h_flex().gap_3()
                                .child(Button::new("retry-repository-load").link().small().label(t("Refresh"))
                                    .disabled(self.busy || self.history_loading).on_click(cx.listener(|s, _, w, cx| s.refresh_current(w, cx))))
                                .child(Button::new("repository-load-details").link().small().label(t("Details"))
                                    .on_click(cx.listener(|s, _, w, cx| s.show_error_details(w, cx)))))
                    )),
            )
            .when(self.history_offset > 0 || self.snapshot.as_ref().is_some_and(|s| s.more), |v| v.child(
                h_flex()
                    .h(px(24.)).min_h(px(24.)).max_h(px(24.)).flex_shrink_0().items_center()
                    .px_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))

                    .when(self.history_offset > 0, |d| {
                        d.child(
                            Button::new("previous-window")
                                .ghost()
                                .small()
                                .label(t("Newer window"))
                                .tooltip(t("Return to the preceding history window in this snapshot"))
                                .on_click(cx.listener(|s, _, _, cx| s.previous_window(cx))),
                        )
                        .child(
                            Button::new("latest")
                                .ghost()
                                .small()
                                .label(t("Back to latest"))
                                .on_click(cx.listener(|s, _, _, cx| s.refresh(false, cx))),
                        )
                    })
                    .when(self.snapshot.as_ref().is_some_and(|s| s.more), |d| {
                        d.child(
                            Button::new("more")
                                .ghost()
                                .small()
                                .label(t(if self.limit >= git::HISTORY_MAX {
                                    "Older window"
                                } else {
                                    "Load more"
                                }))
                                .on_click(cx.listener(|s, _, _, cx| {
                                    if !s.history_loading {
                                        s.refresh(true, cx);
                                    }
                                })),
                        )
                    })

,
            ))
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
                if !Arc::ptr_eq(&hover_rows, &s.rows) || s.history_loading || s.graph_print_loading
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
                if !Arc::ptr_eq(&rows, &s.rows)
                    || s.history_loading
                    || s.graph_print_loading
                    || target >= s.commits.len()
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
