//! File history tab and selection-history window, with on-demand revision previews.
use plico::{
    file_history::{self, History, Revision, Selection, Traced},
    git::{Cancel, Repository},
    i18n::{self, t},
};
use gpui_kit::component::{
    Disableable as _, Icon, Root, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    diff::{Diff, DiffChangeIndicator, DiffFile, DiffMode, DiffState},
    h_flex,
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem},
    resizable::{resizable_panel, v_resizable},
    text::TextView,
    scroll::{Scrollbar, ScrollbarMode},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
actions!(
    file_history_view,
    [
        PreviousRevision,
        NextRevision,
        ExtendPreviousRevision,
        ExtendNextRevision,
        CloseHistory
    ]
);
#[derive(Clone)]
pub struct SourceRequest {
    pub repository: std::path::PathBuf,
    pub path: std::path::PathBuf,
    pub revision: Option<Revision>,
    pub line: usize,
}
pub type SourceCallback = std::rc::Rc<dyn Fn(SourceRequest, &mut App)>;
#[derive(Clone)]
pub struct ReadingState {
    revisions: plico::log_selection::Selection,
    branch: Option<String>,
    revision: Option<String>,
    row: usize,
    mode: DiffMode,
    limit: usize,
}
pub struct HistoryView {
    show_details: bool,
    revisions: plico::log_selection::Selection,
    details: Vec<(String, Vec<plico::git::RefLabel>, plico::commit_presentation::Presentation, Entity<crate::containing_branches::ContainingBranches>)>,
    details_token: Cancel,
    details_generation: u64,
    details_task: Task<()>,
    details_branches: Option<Entity<crate::containing_branches::Batch>>,
    details_loading: bool,
    details_error: Option<String>,
    preview_after: Option<Revision>,
    preview_versions: Vec<Revision>,
    preview_loading: bool,
    branch: Option<String>,
    refs: Vec<file_history::RefChoice>,
    graph: std::sync::Arc<Vec<plico::graph::Row>>,
    graph_hover: Option<(usize, usize)>,
    head_id: Option<String>,
    links: Vec<Vec<(String, bool)>>,
    preview_before: Option<Revision>,
    last_source_position: Option<(bool, usize)>,
    restore: Option<ReadingState>,
    repo: Repository,
    path: std::path::PathBuf,
    selection: Option<Selection>,
    history: History,
    traced: Vec<Traced>,
    visible: Vec<usize>,
    selected: usize,
    changes_only: bool,
    limit: usize,
    loading: bool,
    error: Option<String>,
    viewer: Option<Entity<DiffState>>,
    mode: DiffMode,
    token: Cancel,
    generation: u64,
    task: Task<()>,
    preview_token: Cancel,
    preview_generation: u64,
    preview_task: Task<()>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    message_scroll: ScrollHandle,
}
impl Focusable for HistoryView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Drop for HistoryView {
    fn drop(&mut self) {
        self.token.cancel();
        self.preview_token.cancel();
        self.details_token.cancel();
    }
}
impl HistoryView {
    pub fn new(
        repo: Repository,
        path: std::path::PathBuf,
        selection: Option<Selection>,
        restore: Option<ReadingState>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.bind_keys([
            KeyBinding::new(
                "f4",
                crate::ui_actions::JumpToSource,
                Some("SelectionHistoryWindow"),
            ),
            KeyBinding::new("escape", CloseHistory, Some("SelectionHistoryWindow")),
            KeyBinding::new("up", PreviousRevision, Some("FileHistory")),
            KeyBinding::new("down", NextRevision, Some("FileHistory")),
            KeyBinding::new("shift-up", ExtendPreviousRevision, Some("FileHistory")),
            KeyBinding::new("shift-down", ExtendNextRevision, Some("FileHistory")),
        ]);
        let mode = restore.as_ref().map(|s| s.mode).unwrap_or(DiffMode::Split);
        let limit = restore
            .as_ref()
            .map(|s| s.limit)
            .unwrap_or(file_history::PAGE);
        let branch = restore
            .as_ref()
            .map(|s| s.branch.clone())
            .unwrap_or_else(|| Some("HEAD".into()));
        let revisions = restore
            .as_ref()
            .map(|s| s.revisions.clone())
            .unwrap_or_default();
        let show_details=plico::layout_preferences::load_history_details(&repo.path);
        let mut s = Self {
            show_details,
            revisions,
            details: vec![],
            details_token: Cancel::default(),
            details_generation: 0,
            details_task: Task::ready(()),
            details_branches: None,
            details_loading: false,
            details_error: None,
            preview_after: None,
            preview_versions: vec![],
            preview_loading: false,
            branch,
            refs: vec![],
            graph: Default::default(),
            graph_hover: None,
            head_id: None,
            links: vec![],
            preview_before: None,
            last_source_position: None,
            restore,
            repo,
            path,
            selection,
            history: History {
                revisions: vec![],
                more: false,
                older: None,
            },
            traced: vec![],
            visible: vec![],
            selected: 0,
            changes_only: true,
            limit,
            loading: false,
            error: None,
            viewer: None,
            mode,
            token: Cancel::default(),
            generation: 0,
            task: Task::ready(()),
            preview_token: Cancel::default(),
            preview_generation: 0,
            preview_task: Task::ready(()),
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            message_scroll: ScrollHandle::new(),
        };
        s.load(cx);
        s
    }
    pub fn reading_state(&self) -> ReadingState {
        ReadingState {
            revisions: self.revisions.clone(),
            branch: self.branch.clone(),
            revision: self.revision(self.selected).map(|r| r.id.clone()),
            row: self.selected,
            mode: self.mode,
            limit: self.limit,
        }
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        self.graph_hover = None;
        if self.restore.is_none() && !self.visible.is_empty() {
            self.restore = Some(self.reading_state());
        }
        self.token.cancel();
        self.token = Cancel::default();
        self.generation += 1;
        self.preview_token.cancel();
        self.preview_generation += 1;
        self.preview_task = Task::ready(());
        self.clear_details();
        self.loading = true;
        self.error = None;
        self.viewer = None;
        let repo = self.repo.clone();
        let path = self.path.clone();
        let selection = self.selection.clone();
        let limit = self.limit;
        let branch = self.branch.clone();
        let token = self.token.clone();
        let generation = self.generation;
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let (h, refs, graph, links, head_id) = if selection.is_some() {
                        (
                            file_history::load(&repo, &path, limit, &token)?,
                            vec![],
                            vec![],
                            vec![],
                            None,
                        )
                    } else {
                        let scoped = file_history::load_scoped(
                            &repo,
                            &path,
                            branch.as_deref(),
                            limit,
                            &token,
                        )?;
                        (scoped.history, scoped.refs, scoped.graph, scoped.links, Some(scoped.head_id))
                    };
                    let rows = selection
                        .as_ref()
                        .map(|s| file_history::trace(&repo, &h, s, &token))
                        .transpose()?;
                    Ok::<_, String>((h, rows.unwrap_or_default(), refs, graph, links, head_id))
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.generation {
                    return;
                }
                s.loading = false;
                match result {
                    Ok((h, rows, refs, graph, links, head_id)) => {
                        s.head_id = head_id;
                        s.links = links;
                        s.refs = refs;
                        s.graph = std::sync::Arc::new(graph);
                        s.history = h;
                        s.traced = rows;
                        s.rebuild();
                        let restored = s.restore.take();
                        if let Some(state) = &restored {
                            s.revisions = state.revisions.clone();
                        }
                        let row = restored
                            .as_ref()
                            .map(|state| {
                                (0..s.visible.len())
                                    .find(|i| {
                                        s.revision(*i).map(|r| r.id.as_str())
                                            == state.revision.as_deref()
                                    })
                                    .unwrap_or(state.row.min(s.visible.len().saturating_sub(1)))
                            })
                            .unwrap_or(0);
                        let ids = s.revision_ids();
                        s.revisions.reconcile(&ids);
                        if s.selection.is_none()
                            && restored.as_ref().is_some_and(|st| st.revision.is_some())
                            && s.revisions.lead(&ids).is_some()
                        {
                            s.selected = s.revisions.lead(&ids).unwrap();
                            s.scroll.scroll_to_item(s.selected, ScrollStrategy::Nearest);
                            s.refresh_preview(cx);
                        } else {
                            s.select(row, cx);
                        }
                    }
                    Err(e) => {
                        s.visible.clear();
                        s.graph = Default::default();
                        s.traced.clear();
                        s.history = History {
                            revisions: vec![],
                            more: false,
                            older: None,
                        };
                        s.error = Some(e);
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn rebuild(&mut self) {
        self.visible = if self.selection.is_some() {
            self.traced
                .iter()
                .enumerate()
                .filter(|(i, r)| {
                    *i <= self.history.revisions.len() && (!self.changes_only || r.changed)
                })
                .map(|(i, _)| i)
                .collect()
        } else {
            (0..self.history.revisions.len()).collect()
        };
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }
    fn revision(&self, row: usize) -> Option<&Revision> {
        let i = *self.visible.get(row)?;
        if self.selection.is_some() {
            self.traced.get(i)?.revision.as_ref()
        } else {
            self.history.revisions.get(i)
        }
    }
    fn revision_ids(&self) -> Vec<String> {
        self.history
            .revisions
            .iter()
            .map(|r| r.id.clone())
            .collect()
    }
    fn select(&mut self, row: usize, cx: &mut Context<Self>) {
        self.select_modified(row, false, false, cx);
    }
    fn select_modified(&mut self, row: usize, toggle: bool, extend: bool, cx: &mut Context<Self>) {
        if self.loading || self.visible.get(row).is_none() {
            return;
        }
        self.selected = row;
        self.graph_hover = None;
        if self.selection.is_none() {
            self.revisions
                .select(&self.revision_ids(), row, toggle, extend);
        }
        self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        self.refresh_preview(cx);
    }
    fn clear_details(&mut self) {
        self.details_token.cancel();
        self.details_generation += 1;
        self.details_task = Task::ready(());
        self.details_branches = None;
        self.details.clear();
        self.details_loading = false;
        self.details_error = None;
    }
    fn set_show_details(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.show_details==enabled{return;}
        self.show_details=enabled;
        let _=plico::layout_preferences::save_history_details(&self.repo.path,enabled);
        if enabled {self.load_details(cx);} else {self.clear_details();}
        cx.notify();
    }
    fn load_details(&mut self, cx: &mut Context<Self>) {
        self.clear_details();
        if self.selection.is_some() || !self.show_details { return; }
        let ids: Vec<_> = self.history.revisions.iter().filter(|r| self.revisions.contains(&r.id))
            .take(50).map(|r| r.id.clone()).collect();
        if ids.is_empty() { return; }
        self.details_token = Cancel::default();
        let token = self.details_token.clone();
        let generation = self.details_generation;
        let repo = self.repo.clone();
        self.details_loading = true;
        self.details_task = cx.spawn(async move |this, cx| {
            let query_repo = repo.clone();
            let result = cx.background_spawn(async move {
                let snapshot = query_repo.snapshot_commits(&ids, &ids, &token)?;
                let mut cards = Vec::new();
                let mut bytes = 0usize;
                for commit in snapshot.commits {
                    let details = query_repo.commit_metadata(&commit, &token)?;
                    let presentation = plico::commit_presentation::build(&details, Some(&commit));
                    bytes = bytes.saturating_add(presentation.message.len())
                        .saturating_add(presentation.metadata.iter().map(String::len).sum::<usize>())
                        .saturating_add(commit.refs.iter().map(|r| r.name.len() + 64).sum::<usize>());
                    if bytes > plico::git::OUTPUT_LIMIT {
                        return Err("Commit details exceed the preview output limit".to_owned());
                    }
                    cards.push((commit.id, commit.refs, presentation));
                }
                Ok::<_, String>(cards)
            }).await;
            let _ = this.update(cx, |s, cx| {
                if s.details_generation != generation { return; }
                s.details_loading = false;
                match result {
                    Ok(cards) => {
                        let mut branches = Vec::new();
                        s.details = cards.into_iter().map(|(id, refs, presentation)| {
                            let card = cx.new(crate::containing_branches::ContainingBranches::pending);
                            branches.push((id.clone(), card.downgrade()));
                            (id, refs, presentation, card)
                        }).collect();
                        s.details_branches = Some(cx.new(|cx| crate::containing_branches::Batch::new(repo, branches, cx)));
                    }
                    Err(error) => s.details_error = Some(error),
                }
                cx.notify();
            });
        });
    }
    fn details_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let overflow = self.history.revisions.iter().filter(|r| self.revisions.contains(&r.id)).count() > 50;
        div().relative().size_full().min_h_0().min_w_0().overflow_hidden()
            .child(v_flex().id("file-history-details").size_full().min_h_0().min_w_0()
                .overflow_y_scroll().track_scroll(&self.message_scroll).px(px(14.)).py(px(14.)).pr(px(24.))
                .when(self.details_loading, |v| v.child(t("Loading commit details…")))
                .when(!self.details_loading && self.details_error.is_none() && self.details.is_empty(), |v| v.child(t("Select commits to view changes")))
                .when_some(self.details_error.clone(), |v, error| v.child(crate::error_ui::notice(
                    "history-details-error", "Could not load commit details", error, self.details_loading,
                    cx.listener(|s, _, _, cx| s.load_details(cx)))))
                .children(self.details.iter().map(|(id, refs, presentation, branches)| {
                    let hash = id.clone();
                    v_flex().id(id.clone()).w_full().min_w_0().flex_shrink_0().gap_2().pb_3().mb_3()
                        .border_b_1().border_color(rgb(0x2a2b2d))
                        .child(TextView::html(format!("history-message-{id}"), presentation.message.clone())
                            .font_family("JetBrains Mono").text_size(rems(1.)).selectable(true))
                        .child(gpui_kit::base::TextView::html(format!("history-metadata-{id}"), presentation.metadata[usize::from(i18n::is_chinese())].clone())
                            .text_size(rems(1.)).style(gpui_kit::base::TextViewStyle::default()
                                .with_foreground(rgb(0xd1d3d9).into()).with_link(rgb(0x71a1fe).into())
                                .with_selection(rgb(0x2e436e).into())).selectable(true))
                        .child(crate::ui_style::commit_refs(refs)).child(branches.clone())
                        .context_menu(move |menu, _, _| {
                            let hash = hash.clone();
                            menu.item(PopupMenuItem::new(t("Copy Revision Number"))
                                .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(hash.clone()))))
                        })
                }))
                .when(overflow, |v| v.child(t("Showing the first 50 commits"))))
            .child(Scrollbar::vertical(&self.message_scroll).mode(ScrollbarMode::Always))
            .into_any_element()
    }
    fn refresh_preview(&mut self, cx: &mut Context<Self>) {
        self.load_details(cx);
        self.preview_token.cancel();
        self.preview_token = Cancel::default();
        self.preview_generation += 1;
        let generation = self.preview_generation;
        self.viewer = None;
        self.last_source_position = None;
        self.preview_before = None;
        self.preview_after = None;
        self.preview_versions.clear();
        self.preview_loading = false;
        self.error = None;
        if self.selection.is_some() {
            let Some(&index) = self.visible.get(self.selected) else {
                cx.notify();
                return;
            };
            // Block content is a document fragment, not a Git blob: avoid a
            // misleading file-level "no newline" warning on every selected block.
            let after = (!self.traced[index].lines.is_empty())
                .then(|| format!("{}\n", self.traced[index].block));
            let before = self
                .traced
                .get(index + 1)
                .filter(|r| !r.lines.is_empty())
                .map(|r| format!("{}\n", r.block));
            if before.is_some() || after.is_some() {
                self.set_diff(before.as_deref(), after.as_deref(), cx);
            }
            return;
        }
        let comparison = match file_history::selected_comparison(&self.history, &self.links, &self.revisions) {
            Ok(Some(comparison)) => comparison,
            Ok(None) => { cx.notify(); return; }
            Err(error) => { self.error = Some(error); cx.notify(); return; }
        };
        let versions = file_history::preview_revisions(comparison);
        self.preview_loading = true;
        let token = self.preview_token.clone();
        let repo = self.repo.clone();
        self.preview_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let texts = versions.iter().map(|version| version.as_ref()
                        .map(|r| file_history::contents(&repo, r, &token)).transpose())
                        .collect::<Result<Vec<_>, _>>()?;
                    let rows = if versions.len() == 3 {
                        let texts: [&str; 3] = std::array::from_fn(|i| texts[i].as_deref().unwrap_or_default());
                        {
                            let rows = plico::three_way::align(texts, &token)?;
                            let inner = plico::three_way::inner_ranges(texts, &rows, &token)?;
                            Some((rows, inner))
                        }
                    } else { None };
                    Ok::<_, String>((versions, texts, rows))
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.preview_generation {
                    return;
                }
                s.preview_loading = false;
                match result {
                    Ok((versions, texts, Some((rows, inner)))) => {
                        s.set_three_diff(versions, texts, rows, inner, cx);
                    }
                    Ok((mut versions, mut texts, None)) => {
                        s.preview_after = versions.pop().flatten();
                        s.preview_before = versions.pop().flatten();
                        let after = texts.pop().flatten();
                        let before = texts.pop().flatten();
                        s.set_diff(before.as_deref(), after.as_deref(), cx);
                    }
                    Err(e) => s.error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn set_three_diff(&mut self, versions: Vec<Option<Revision>>, texts: Vec<Option<String>>,
        rows: Vec<plico::three_way::Row>, inner: plico::three_way::InnerRanges, cx: &mut Context<Self>) {
        use gpui_kit::component::diff::{DiffTint, ThreeWayRow};
        let versions: Option<Vec<_>> = versions.into_iter().collect();
        let Some(versions) = versions.filter(|v| v.len() == 3) else {
            self.error = Some("Invalid three-way revisions".into()); return;
        };
        let files = versions.iter().enumerate().map(|(col, v)| DiffFile::unchanged(
            format!(".gitreview-three-{col}/{}", v.path.display()), texts[col].as_deref().unwrap_or_default())).collect::<Vec<_>>();
        let rows = rows.into_iter().map(|row| ThreeWayRow { lines: row.lines,
            tints: row.tints.map(|t| t.map(|t| match t {
                plico::three_way::Tint::Inserted => DiffTint::Inserted,
                plico::three_way::Tint::Deleted => DiffTint::Deleted,
                plico::three_way::Tint::Modified => DiffTint::Modified,
                plico::three_way::Tint::Conflict => DiffTint::Conflict,
            })) }).collect();
        let viewer = cx.new(|cx| DiffState::new(files, cx));
        let result = viewer.update(cx, |viewer, _| viewer.set_three_way(rows).and_then(|_| viewer.set_three_inner(inner)));
        match result {
            Ok(()) => {
                cx.subscribe(&viewer, |_, _, _: &gpui_kit::component::diff::DiffEvent, cx| cx.notify()).detach();
                self.preview_versions = versions;
                self.viewer = Some(viewer);
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }
    fn set_diff(&mut self, before: Option<&str>, after: Option<&str>, cx: &mut Context<Self>) {
        if before.is_none() && after.is_none() {
            cx.notify();
            return;
        }
        let path = self.path.to_string_lossy();
        match DiffFile::from_sources(
            before.map(|s| (path.as_ref(), s)),
            after.map(|s| (path.as_ref(), s)),
        ) {
            Ok(file) => {
                let mode = if before.is_none() || after.is_none() {
                    DiffMode::Unified
                } else {
                    self.mode
                };
                let viewer = cx.new(|cx| {
                    DiffState::new([file], cx)
                        .with_mode(mode)
                        .with_context_lines(None)
                });
                cx.subscribe(
                    &viewer,
                    |s, _, event: &gpui_kit::component::diff::DiffEvent, cx| {
                        if let gpui_kit::component::diff::DiffEvent::SelectionChanged(Some(range)) =
                            event
                        {
                            s.last_source_position = Some((
                                range.end_side() == gpui_kit::component::diff::DiffSide::Original,
                                range.end().saturating_sub(1),
                            ));
                            cx.notify();
                        }
                    },
                )
                .detach();
                self.viewer = Some(viewer);
            }
            Err(e) => self.error = Some(e.into()),
        }
        cx.notify();
    }
    fn table(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let cells: Vec<_> = self
            .visible
            .iter()
            .enumerate()
            .map(|(row, _)| {
                let r = self.revision(row);
                (
                    r.map(|r| r.id[..8].to_owned())
                        .unwrap_or_else(|| t("Local changes").into()),
                    r.map(|r| {
                        plico::commit_date::timestamp(r.timestamp)
                    })
                    .unwrap_or_default(),
                    r.map(|r| r.author.clone()).unwrap_or_default(),
                    r.map(|r| r.message.lines().next().unwrap_or_default().to_owned())
                        .unwrap_or_else(|| t("Local changes").into()),
                )
            })
            .collect();
        let selected: Vec<bool> = (0..self.visible.len())
            .map(|i| {
                if self.selection.is_some() {
                    i == self.selected
                } else {
                    self.revision(i)
                        .is_some_and(|r| self.revisions.contains(&r.id))
                }
            })
            .collect();
        let graph = self.graph.clone();
        let recommended_width = graph.iter().map(|r| r.width).max().unwrap_or(1);
        let graph_width = recommended_width.min(6) as f32 * 16. + 12.;
        let graph_hover = self.graph_hover;
        let row_height = crate::ui_style::log_row_height(cx);
        let head_rows: Vec<bool> = (0..self.visible.len()).map(|i| self.revision(i).is_some_and(|r| Some(&r.id) == self.head_id.as_ref())).collect();
        let selection = self.selection.is_some();
        v_flex()
            .size_full()
            .min_h_0()
            .px_3()
            .key_context("FileHistory")
            .track_focus(&self.focus)
            .on_action(cx.listener(|s, _: &PreviousRevision, _, cx| {
                s.select(s.selected.saturating_sub(1), cx)
            }))
            .on_action(cx.listener(|s, _: &NextRevision, _, cx| {
                s.select((s.selected + 1).min(s.visible.len().saturating_sub(1)), cx)
            }))
            .on_action(cx.listener(|s, _: &ExtendPreviousRevision, _, cx| {
                s.select_modified(s.selected.saturating_sub(1), false, true, cx)
            }))
            .on_action(cx.listener(|s, _: &ExtendNextRevision, _, cx| {
                s.select_modified(
                    (s.selected + 1).min(s.visible.len().saturating_sub(1)),
                    false,
                    true,
                    cx,
                )
            }))
            .when(self.selection.is_some(), |v| {
                v.child(
                    Checkbox::new("changed-revisions")
                        .label(t("Changes only"))
                        .checked(self.changes_only)
                        .on_click(cx.listener(|s, checked, _, cx| {
                            s.changes_only = *checked;
                            s.rebuild();
                            s.select(0, cx);
                        })),
                )
            })
            .child(if selection {
                h_flex()
                    .h(px(row_height))
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(0x393b40))
                    .child(div().w(px(100.)).child(t("Version")))
                    .child(div().w(px(145.)).child(t("Date")))
                    .child(div().w(px(130.)).child(t("Author")))
                    .child(t("Commit Message"))
                    .into_any_element()
            } else {
                h_flex()
                    .h(px(row_height))
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(0x393b40))
                    .child(h_flex().flex_1().min_w_0().overflow_hidden().gap_2()
                        .child(div().w(px(graph_width)).flex_shrink_0())
                        .child(div().flex_1().min_w_0().truncate().child(t("Commit Message"))))
                    .child(div().w(px(130.)).flex_shrink_0().child(t("Author")))
                    .child(div().w(px(145.)).flex_shrink_0().child(t("Date")))
                    .into_any_element()
            })
            .child(
                uniform_list("file-history-revisions", cells.len(), move |range, _, _| {
                    range
                        .map(|row| {
                            let (a, b, c, d) = cells[row].clone();
                            let owner = owner.clone();
                            h_flex()
                                .id(("history-row", row))
                                .w_full()
                                .h(px(row_height))
                                .gap_2()
                                .px_2()
                                .when(selected[row], |v| v.bg(rgb(0x2e4676)))
                                .when(selection, |v| {
                                    v.child(div().w(px(100.)).truncate().child(a.clone()))
                                        .child(div().w(px(145.)).truncate().child(b.clone()))
                                        .child(div().w(px(130.)).truncate().child(c.clone()))
                                        .child(div().flex_1().min_w_0().truncate().child(d.clone()))
                                })
                                .when(!selection, |v| {
                                    let mut commit = h_flex().flex_1().min_w_0().overflow_hidden().gap_2();
                                    if let Some(r) = graph.get(row).cloned() {
                                        let width = plico::graph::text_offset_columns(&r, recommended_width) * 16. + 12.;
                                        let hovered = graph_hover.filter(|(i, _)| *i == row).map(|(_, index)| index);
                                        let tooltip = hovered.map(|index| {
                                            r.arrow_target(index).and_then(|target| cells.get(target)).map(|(id,date,author,subject)| {
                                                crate::ui_graph::graph_arrow_tooltip(id, subject, author, date)
                                            }).unwrap_or_else(|| t("Commit is outside the loaded history window").into())
                                        });
                                        let hover_owner = owner.clone(); let click_owner = owner.clone();
                                        let hover_graph = graph.clone(); let click_graph = graph.clone();
                                        commit = commit.child(crate::ui_graph::graph_arrow_cell(r, width, row_height, row, crate::ui_graph::GraphNodeStyle { head: head_rows[row], background: if selected[row] { 0x2e4676 } else { 0x18191a } }, hovered, tooltip, move |segment,cx| {
                                            let _ = hover_owner.update(cx, |s,cx| {
                                                if s.loading || !std::sync::Arc::ptr_eq(&hover_graph,&s.graph) { return; }
                                                if segment.is_none() && s.graph_hover.is_some_and(|(i,_)| i != row) { return; }
                                                let hover = segment.map(|segment| (row,segment));
                                                if s.graph_hover != hover { s.graph_hover = hover; cx.notify(); }
                                            });
                                        }, move |target,w,cx| {
                                            let _ = click_owner.update(cx, |s,cx| {
                                                if s.loading || s.selection.is_some() || !std::sync::Arc::ptr_eq(&click_graph,&s.graph) { return; }
                                                s.select(target,cx);
                                                s.focus.focus(w,cx);
                                            });
                                        }));
                                    }
                                    v.child(commit.child(div().flex_1().min_w_0().truncate().child(d)))
                                        .child(div().w(px(130.)).flex_shrink_0().truncate().child(c))
                                        .child(div().w(px(145.)).flex_shrink_0().truncate().child(b))
                                })
                                .on_mouse_down(MouseButton::Left, move |event, w, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        s.focus.focus(w, cx);
                                        s.select_modified(
                                            row,
                                            event.modifiers.control,
                                            event.modifiers.shift,
                                            cx,
                                        );
                                    });
                                })
                                .into_any_element()
                        })
                        .collect()
                })
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0(),
            )
            .when(self.history.more, |v| {
                v.child(
                    h_flex()
                        .gap_2()
                        .child(t("More file history is available"))
                        .child(
                            Button::new("more-file-history")
                                .link()
                                .small()
                                .label(t("Load more"))
                                .disabled(self.limit >= file_history::MAX_REVISIONS || self.loading)
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.limit = (s.limit + file_history::PAGE)
                                        .min(file_history::MAX_REVISIONS);
                                    s.load(cx);
                                })),
                        ),
                )
            })
            .into_any_element()
    }
    pub fn source_request(&self, cx: &App) -> Option<SourceRequest> {
        if self.loading || self.error.is_some() {
            return None;
        }
        let viewer = self.viewer.as_ref()?.read(cx);
        if self.preview_versions.len() == 3 {
            let position = viewer.source_position();
            let col = position.as_ref().and_then(|p| viewer.files().iter().position(|f| f.path() == p.path())).unwrap_or(1);
            return Some(SourceRequest { repository: self.repo.path.clone(), path: self.path.clone(),
                revision: self.preview_versions.get(col).cloned(), line: position.map(|p| p.line().saturating_sub(1)).unwrap_or(0) });
        }
        let selected = viewer.selected_lines();
        let (original, line) = viewer.source_position().map(|p| (p.side() == gpui_kit::component::diff::DiffSide::Original, p.line().saturating_sub(1))).or_else(|| selected
            .map(|r| {
                (
                    r.end_side() == gpui_kit::component::diff::DiffSide::Original,
                    r.end().saturating_sub(1),
                )
            })
            ).or(self.last_source_position)
            .unwrap_or((false, 0));
        let index = *self.visible.get(self.selected)? + usize::from(original);
        if self.selection.is_some() {
            let row = self.traced.get(index)?;
            if row.lines.is_empty() {
                return None;
            }
            Some(SourceRequest {
                repository: self.repo.path.clone(),
                path: self.path.clone(),
                revision: row.revision.clone(),
                line: row.lines.start + line.min(row.lines.len().saturating_sub(1)),
            })
        } else {
            let revision = if original {
                self.preview_before.as_ref()?
            } else {
                self.preview_after.as_ref()?
            }
            .clone();
            if revision.deleted {
                return None;
            }
            Some(SourceRequest {
                repository: self.repo.path.clone(),
                path: self.path.clone(),
                revision: Some(revision),
                line,
            })
        }
    }
    fn choose_branch(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        if branch == self.branch {
            return;
        }
        self.branch = branch;
        self.limit = file_history::PAGE;
        self.restore = Some(ReadingState {
            revisions: Default::default(),
            branch: self.branch.clone(),
            revision: None,
            row: 0,
            mode: self.mode,
            limit: self.limit,
        });
        self.load(cx);
    }
    fn branch_filter(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let mut groups = std::collections::BTreeMap::<String, Vec<(String, String)>>::new();
        for r in &self.refs {
            for (prefix, group) in [
                ("refs/heads/", "Local"),
                ("refs/remotes/", "Remote"),
                ("refs/tags/", "Tags"),
            ] {
                if let Some(name) = r.name.strip_prefix(prefix) {
                    groups
                        .entry(t(group).into())
                        .or_default()
                        .push((name.into(), r.name.clone()));
                }
            }
        }
        let label = self
            .branch
            .as_deref()
            .map(|b| {
                let name = b
                    .strip_prefix("refs/heads/")
                    .or_else(|| b.strip_prefix("refs/remotes/"))
                    .or_else(|| b.strip_prefix("refs/tags/"))
                    .unwrap_or(b);
                format!("{}: {name}", t("Branch"))
            })
            .unwrap_or_else(|| t("Branch").into());
        h_flex()
            .gap_0()
            .child(
                Button::new("history-branch-filter")
                    .ghost()
                    .small()
                    .label(label)
                    .icon(Icon::default().path("jb/chevronDown_dark.svg"))
                    .disabled(self.loading)
                    .dropdown_menu(move |mut menu, window, cx| {
                        let clear = owner.clone();
                        menu = menu
                            .min_w(px(240.))
                            .item(PopupMenuItem::new(t("All branches")).on_click(
                                move |_, _, cx| {
                                    let _ = clear.update(cx, |s, cx| s.choose_branch(None, cx));
                                },
                            ))
                            .separator();
                        let head = owner.clone();
                        menu = menu
                            .item(PopupMenuItem::new("HEAD").on_click(move |_, _, cx| {
                                let _ = head
                                    .update(cx, |s, cx| s.choose_branch(Some("HEAD".into()), cx));
                            }))
                            .separator();
                        for (group, entries) in &groups {
                            let entries = entries.clone();
                            let owner = owner.clone();
                            menu = menu.submenu(group.clone(), window, cx, move |mut sub, _, _| {
                                for (label, name) in &entries {
                                    let owner = owner.clone();
                                    let name = name.clone();
                                    sub = sub.item(PopupMenuItem::new(label.clone()).on_click(
                                        move |_, _, cx| {
                                            let _ = owner.update(cx, |s, cx| {
                                                s.choose_branch(Some(name.clone()), cx)
                                            });
                                        },
                                    ));
                                }
                                sub
                            });
                        }
                        menu
                    }),
            )
            .when(self.branch.is_some(), |v| {
                v.child(
                    Button::new("clear-history-branch")
                        .ghost()
                        .small()
                        .icon(Icon::default().path("jb/closeSmall.svg"))
                        .tooltip(t("All branches"))
                        .disabled(self.loading)
                        .on_click(cx.listener(|s, _, _, cx| s.choose_branch(None, cx))),
                )
            })
    }
    fn preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let endpoints = (self.selection.is_none())
            .then(|| file_history::comparison_rows(&self.history, &self.revisions))
            .flatten();
        let title = self
            .revision(if self.selection.is_some() {
                self.selected
            } else {
                endpoints.map(|p| p.0).unwrap_or(self.selected)
            })
            .map(|r| format!("{} {} · {}", t("Revision"), &r.id[..8], r.path.display()))
            .unwrap_or_else(|| t("Local changes").into());
        let title = if self.preview_versions.len() == 3 {
            t("Merge result").to_owned()
        } else if let Some((after, Some(before))) = endpoints {
            format!(
                "{} {} → {}",
                t("Compare Versions"),
                &self.history.revisions[before].id[..8],
                &self.history.revisions[after].id[..8]
            )
        } else if self.selection.is_none() && endpoints.is_none() {
            t("Select commits to view changes").into()
        } else {
            title
        };
        let selection_lines = self
            .selection
            .as_ref()
            .and_then(|_| self.visible.get(self.selected))
            .and_then(|i| self.traced.get(*i))
            .map(|r| format!("{} {}–{}", t("Lines"), r.lines.start + 1, r.lines.end));
        v_flex()
            .size_full()
            .min_h_0()
            .child(
                h_flex()
                    .h(px(36.))
                    .px_3()
                    .gap_2()
                    .bg(rgb(0x212326))
                    .child(Button::new("history-previous-change").ghost().small()
                        .icon(Icon::default().path("jb/up_dark.svg")).tooltip(t("Previous change (Shift+F7)"))
                        .disabled(self.viewer.is_none()).on_click(cx.listener(|s, _, _, cx| {
                            if let Some(viewer) = &s.viewer { viewer.update(cx, |v, cx| v.previous_change(cx)); }
                        })))
                    .child(Button::new("history-next-change").ghost().small()
                        .icon(Icon::default().path("jb/down_dark.svg")).tooltip(t("Next change (F7)"))
                        .disabled(self.viewer.is_none()).on_click(cx.listener(|s, _, _, cx| {
                            if let Some(viewer) = &s.viewer { viewer.update(cx, |v, cx| v.next_change(cx)); }
                        })))
                    .child(
                        Button::new("history-jump-source")
                            .ghost()
                            .small()
                            .icon(Icon::default().path("jb/edit_dark.svg"))
                            .tooltip(t("Jump to Source · F4"))
                            .disabled(self.source_request(cx).is_none())
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(crate::ui_actions::JumpToSource), cx)
                            }),
                    )
                    .child(div().flex_1().truncate().child(title))
                    .children(selection_lines)
                    .when(self.preview_versions.len() != 3, |v| v.child({
                        let owner = cx.entity().downgrade();
                        crate::diff_settings::mode_selector(self.mode, move |mode, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                s.mode = mode;
                                if let Some(v) = &s.viewer {
                                    v.update(cx, |v, cx| v.set_mode(mode, cx));
                                }
                                cx.notify();
                            });
                        })
                    })),
            )
            .when(self.preview_versions.len() == 3, |v| v.child(
                h_flex().w_full().flex_shrink_0().children(self.preview_versions.iter().enumerate().map(|(col, revision)|
                    h_flex().w(relative(1. / 3.)).min_w_0().flex_shrink_0().h(px(28.)).px_2().gap_1()
                        .border_b_1().border_color(rgb(0x2a2b2d))
                        .child(Icon::default().path("jb/locked_dark.svg").size(px(14.)))
                        .child(div().truncate().child(format!("{} {} · {}", t(if col == 1 { "Merge result" } else { "Parent revision" }), &revision.id[..8], revision.path.display())))
                ))))
            .when_some(self.viewer.clone(), |v, viewer| {
                v.child(
                    Diff::new(&viewer)
                        .size_full()
                        .header_visible(false)
                        .change_indicator(DiffChangeIndicator::None),
                )
            })
            .when(
                self.viewer.is_none() && !self.loading && self.error.is_none(),
                |v| {
                    v.child(div().p_4().child(t(if self.visible.is_empty() {
                        "No history for this file"
                    } else if self.selection.is_none()
                        && file_history::comparison_rows(&self.history, &self.revisions).is_none()
                    {
                        "Select commits to view changes"
                    } else if self.preview_loading {
                        "Reading Git data…"
                    } else {
                        "No differences"
                    })))
                },
            )
            .into_any_element()
    }
}
impl Render for HistoryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content_height = (window.viewport_size().height - px(130.)).max(px(300.));
        let table = self.table(cx);
        let preview = self.preview(cx);
        let message = self.revision(self.selected).map(|r| r.message.clone()).unwrap_or_default();
        let selection = self.selection.is_some();
        v_flex()
            .size_full()
            .on_action(cx.listener(|s, _: &crate::ui_actions::NextChange, _, cx| { if let Some(v) = &s.viewer { v.update(cx, |v, cx| v.next_change(cx)); } }))
            .on_action(cx.listener(|s, _: &crate::ui_actions::PreviousChange, _, cx| { if let Some(v) = &s.viewer { v.update(cx, |v, cx| v.previous_change(cx)); } }))
            .bg(rgb(0x18191a))
            .text_size(rems(1.))
            .child(
                h_flex()
                    .h(px(34.))
                    .px_3()
                    .gap_2()
                    .when(!selection, |v| v.child(self.branch_filter(cx)))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .child(self.path.display().to_string()),
                    )
                    .when(!selection,|v| {
                        let owner=cx.entity().downgrade();let shown=self.show_details;
                        v.child(Button::new("file-history-view-options").ghost().small()
                            .icon(Icon::default().path("jb/history-show_dark.svg"))
                            .tooltip(t("View Options"))
                            .dropdown_menu(move|menu,_,_|{
                                let owner=owner.clone();
                                menu.min_w(px(200.)).item(PopupMenuItem::new(t("Show Details")).checked(shown)
                                    .on_click(move|_,_,cx|{let _=owner.update(cx,|s,cx|s.set_show_details(!shown,cx));}))
                            }))
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("refresh-file-history")
                            .ghost()
                            .small()
                            .icon(Icon::default().path("jb/refresh_dark.svg"))
                            .tooltip(t("Refresh"))
                            .on_click(cx.listener(|s, _, _, cx| s.load(cx))),
                    ),
            )
            .when(self.loading, |v| {
                v.child(div().p_3().child(t("Loading file history…")))
            })
            .when_some(self.error.clone(), |v, error| v.child(crate::error_ui::notice(
                "file-history-error", "Could not load file history", error, self.loading,
                cx.listener(|s, _, _, cx| s.load(cx)))))
            .child(
                div().flex_1().min_h_0().child(
                    v_resizable("file-history-layout")
                        .child(
                            resizable_panel()
                                .size(if selection { px(330.) } else { content_height * 0.7 })
                                .size_range(px(100.)..px(2000.))
                                .child(if selection { preview } else if !self.show_details { table } else {
                                    v_resizable("file-history-table-details")
                                        .child(resizable_panel().size(content_height * 0.49).size_range(px(100.)..px(2000.)).child(table))
                                        .child(resizable_panel().size_range(px(60.)..px(2000.)).child(self.details_panel(cx)))
                                        .into_any_element()
                                }),
                        )
                        .child(resizable_panel().size_range(px(100.)..px(2000.)).child(
                            if selection {
                                self.table(cx)
                            } else {
                                self.preview(cx)
                            },
                        )),
                ),
            )
            .when(selection, |v| v.child(
                v_flex()
                    .h(px(90.))
                    .flex_shrink_0()
                    .px_3()
                    .py_2()
                    .gap_1()
                    .child(t("Commit Message"))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .id("history-message")
                                    .size_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.message_scroll)
                                    .pr_3()
                                    .child(message),
                            )
                            .child(
                                Scrollbar::vertical(&self.message_scroll)
                                    .mode(ScrollbarMode::Always),
                            ),
                    ),
            ))
    }
}
pub fn open_selection(
    repo: Repository,
    path: std::path::PathBuf,
    selection: Selection,
    source_callback: SourceCallback,
    cx: &mut App,
) -> Result<(), String> {
    let view = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1080.), px(760.)), cx)),
                window_min_size: Some(size(px(720.), px(480.))),
                ..TitleBar::window_options()
            },
            move |window, cx| {
                window.set_window_title(t("History for Selection"));
                let history = cx.new(|cx| HistoryView::new(repo, path, Some(selection), None, cx));
                let shell = cx.new(|_| HistoryWindow {
                    history,
                    source_callback,
                });
                cx.new(|cx| Root::new(shell, window, cx))
            },
        )
        .map_err(|e| e.to_string())?;
    let _ = view;
    Ok(())
}
struct HistoryWindow {
    history: Entity<HistoryView>,
    source_callback: SourceCallback,
}
impl Render for HistoryWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .key_context("SelectionHistoryWindow")
            .on_action(cx.listener(|_, _: &CloseHistory, w, _| w.remove_window()))
            .on_action(cx.listener(|s, _: &crate::ui_actions::JumpToSource, _, cx| {
                if let Some(request) = s.history.read(cx).source_request(cx) {
                    (s.source_callback)(request, cx);
                }
            }))
            .size_full()
            .bg(rgb(0x18191a))
            .text_color(rgb(0xced0d6))
            .child(TitleBar::new().child(t("History for Selection")))
            .child(div().flex_1().min_h_0().child(self.history.clone()))
    }
}
