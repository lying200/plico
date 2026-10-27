//! Independent panes for Rebased GitCompareBranchesUi's two opposite ranges.
use super::*;
mod files;
mod preview;
use plico::branch_compare::{Pair, Query, Side};

actions!(
    branch_compare_files,
    [
        ShowSelectedDiff,
        FileNext,
        FilePrevious,
        FileLeft,
        FileRight,
        CommitNext,
        CommitPrevious,
        CommitExtendNext,
        CommitExtendPrevious
    ]
);

pub(super) struct PreviewChanged(Side, bool);
impl EventEmitter<PreviewChanged> for Pane {}
impl EventEmitter<PreviewChanged> for View {}

pub(super) struct OpenDiff(pub diff_target::Target, pub diff_target::Chain);
pub(super) struct View {
    pub pair: Pair,
    repo: Repository,
    refresh_request: Request,
    panes: [Entity<Pane>; 2],
    split: Entity<ResizableState>,
}
impl EventEmitter<OpenDiff> for View {}
impl View {
    pub fn new(repo: Repository, pair: Pair, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let panes = [Side::First, Side::Second]
            .map(|side| cx.new(|cx| Pane::new(repo.clone(), pair.clone(), side, window, cx)));
        for pane in &panes {
            cx.subscribe(pane, |_, _, event: &PreviewChanged, cx| {
                cx.emit(PreviewChanged(event.0, event.1));
            })
            .detach();
            cx.subscribe(pane, |_, _, event: &OpenDiff, cx| {
                cx.emit(OpenDiff(event.0.clone(), event.1.clone()));
            })
            .detach();
        }
        Self {
            pair,
            repo,
            refresh_request: Default::default(),
            panes,
            split: cx.new(|_| ResizableState::default()),
        }
    }
    pub(super) fn editor_preview_ids(&self, cx: &App) -> Vec<EntityId> {
        self.panes
            .iter()
            .filter_map(|pane| {
                pane.read(cx)
                    .editor_preview
                    .as_ref()?
                    .upgrade()
                    .map(|tab| tab.entity_id())
            })
            .collect()
    }
    pub(super) fn repository_changed(
        &mut self,
        root: &std::path::Path,
        batch: &plico::worktree_events::Batch,
        cx: &mut Context<Self>,
    ) {
        if batch.history_changed()
            && plico::recent_projects::identity(root)
                == plico::recent_projects::identity(&self.repo.path)
        {
            self.refresh(false, cx);
        }
    }
    pub(super) fn refresh(&mut self, force: bool, cx: &mut Context<Self>) {
        self.refresh_request = Default::default();
        let token = self.refresh_request.token.clone();
        let check = token.clone();
        let repo = self.repo.clone();
        let pair = self.pair.clone();
        self.refresh_request.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    Pair::resolve(&repo, &pair.first_ref, &pair.second_ref, &token)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if check.cancelled() {
                    return;
                }
                match result {
                    Ok(pair) => {
                        let changed = pair != s.pair;
                        s.pair = pair.clone();
                        for pane in &s.panes {
                            pane.update(cx, |p, cx| {
                                if force
                                    || changed
                                    || p.error.is_some()
                                    || p.filter.hashes().is_some()
                                {
                                    p.pair = pair.clone();
                                    p.reload(true, cx);
                                } else {
                                    p.refresh_card_refs(cx);
                                }
                            });
                        }
                    }
                    Err(error) => {
                        for pane in &s.panes {
                            pane.update(cx, |p, cx| {
                                p.token.cancel();
                                p.clear_window();
                                p.query = None;
                                p.total = None;
                                p.more = false;
                                p.loading = false;
                                p.error = Some(error.clone());
                                cx.notify();
                            });
                        }
                    }
                }
                cx.notify();
            });
        });
    }
}
impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_resizable("compare-ranges")
            .with_state(&self.split)
            .child(resizable_panel().child(self.panes[0].clone()))
            .child(resizable_panel().child(self.panes[1].clone()))
    }
}
struct Pane {
    repo: Repository,
    pair: Pair,
    side: Side,
    filter: git::LogFilter,
    search: Entity<InputState>,
    users: Entity<crate::user_menu::UserMenu>,
    paths: Entity<crate::paths_menu::PathsMenu>,
    dates: plico::date_filter::Bounds,
    toolbar_width: Pixels,
    query: Option<Arc<Query>>,
    commits: Vec<git::Commit>,
    graph: Vec<graph::Row>,
    more: bool,
    offset: usize,
    total: Option<usize>,
    loading: bool,
    replacing: bool,
    error: Option<String>,
    count_error: Option<String>,
    token: Cancel,
    task: Task<()>,
    count_task: Task<()>,
    selected: Option<String>,
    selection: plico::log_selection::Selection,
    presentations: Vec<DetailCard>,
    containing_branches: Option<Entity<crate::containing_branches::Batch>>,
    refs_request: Request,
    details_total: usize,
    chain: Option<diff_target::Chain>,
    selected_file: Option<usize>,
    selected_folder: Option<PathBuf>,
    files_scroll: UniformListScrollHandle,
    files_focus: FocusHandle,
    commits_focus: FocusHandle,
    file_rows: Vec<plico::file_tree::Row>,
    collapsed_files: std::collections::HashSet<PathBuf>,
    group_files: bool,
    show_details: bool,
    show_preview: bool,
    preview: Option<Entity<DiffTab>>,
    editor_preview: Option<WeakEntity<DiffTab>>,
    preview_group_files: bool,
    preview_folder: Option<PathBuf>,
    preview_split: Entity<ResizableState>,
    details_split: Entity<ResizableState>,
    files_fraction: f32,
    detail_loading: bool,
    detail_error: Option<String>,
    detail_token: Cancel,
    detail_task: Task<()>,
    detail_generation: u64,
    scroll: UniformListScrollHandle,
    split: Entity<ResizableState>,
}
struct DetailCard {
    id: String,
    presentation: plico::commit_presentation::Presentation,
    refs: Vec<git::RefLabel>,
    branches: Entity<crate::containing_branches::ContainingBranches>,
}
impl EventEmitter<OpenDiff> for Pane {}
impl Drop for Pane {
    fn drop(&mut self) {
        self.token.cancel();
        self.detail_token.cancel();
    }
}
impl Pane {
    fn new(
        repo: Repository,
        pair: Pair,
        side: Side,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.bind_keys([
            KeyBinding::new("down", CommitNext, Some("BranchCompareCommits")),
            KeyBinding::new("up", CommitPrevious, Some("BranchCompareCommits")),
            KeyBinding::new("shift-down", CommitExtendNext, Some("BranchCompareCommits")),
            KeyBinding::new(
                "shift-up",
                CommitExtendPrevious,
                Some("BranchCompareCommits"),
            ),
            KeyBinding::new("enter", ShowSelectedDiff, Some("BranchCompareFiles")),
            KeyBinding::new("ctrl-d", ShowSelectedDiff, Some("BranchCompareFiles")),
            KeyBinding::new("down", FileNext, Some("BranchCompareFiles")),
            KeyBinding::new("up", FilePrevious, Some("BranchCompareFiles")),
            KeyBinding::new("left", FileLeft, Some("BranchCompareFiles")),
            KeyBinding::new("right", FileRight, Some("BranchCompareFiles")),
        ]);
        let show_preview =
            layout_preferences::load_branch_preview(&repo.path, side == Side::Second);
        let mut pane = Self {
            users: cx.new(|cx| crate::user_menu::UserMenu::new(repo.clone(), cx)),
            paths: cx.new(|cx| crate::paths_menu::PathsMenu::new(repo.path.clone(), cx)),
            dates: Default::default(),
            toolbar_width: px(800.),
            repo,
            pair,
            side,
            filter: Default::default(),
            search: cx.new(|cx| InputState::new(window, cx).placeholder(t("Text or hash"))),
            query: None,
            commits: vec![],
            graph: vec![],
            more: true,
            offset: 0,
            total: None,
            loading: false,
            replacing: false,
            error: None,
            count_error: None,
            token: Cancel::default(),
            task: Task::ready(()),
            count_task: Task::ready(()),
            selected: None,
            selection: Default::default(),
            presentations: Vec::new(),
            containing_branches: None,
            refs_request: Default::default(),
            details_total: 0,
            chain: None,
            selected_file: None,
            selected_folder: None,
            files_scroll: UniformListScrollHandle::new(),
            files_focus: cx.focus_handle(),
            commits_focus: cx.focus_handle(),
            file_rows: Vec::new(),
            collapsed_files: Default::default(),
            group_files: true,
            show_details: true,
            show_preview,
            preview: None,
            editor_preview: None,
            preview_group_files: true,
            preview_folder: None,
            preview_split: cx.new(|_| ResizableState::default()),
            details_split: cx.new(|_| ResizableState::default()),
            files_fraction: layout_preferences::load_files_fraction(),
            detail_loading: false,
            detail_error: None,
            detail_token: Cancel::default(),
            detail_task: Task::ready(()),
            detail_generation: 0,
            scroll: UniformListScrollHandle::new(),
            split: cx.new(|_| ResizableState::default()),
        };
        cx.subscribe(&pane.search, |s, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                s.apply_filter(cx);
            }
        })
        .detach();
        cx.subscribe(
            &pane.users,
            |s, _, event: &crate::user_menu::Changed, cx| match event {
                crate::user_menu::Changed::Applied => s.apply_filter(cx),
                crate::user_menu::Changed::Failed(error) => {
                    s.error = Some(error.clone());
                    cx.notify();
                }
            },
        )
        .detach();
        cx.subscribe(&pane.paths, |s, _, _: &crate::paths_menu::Changed, cx| {
            s.apply_filter(cx)
        })
        .detach();
        pane.apply_filter(cx);
        pane
    }
    fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter = Default::default();
        self.dates = Default::default();
        self.users.update(cx, |menu, cx| menu.clear_filter(cx));
        self.paths.update(cx, |menu, cx| menu.clear_filter(cx));
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.apply_filter(cx);
    }
    fn apply_filter(&mut self, cx: &mut Context<Self>) {
        self.filter.text = self.search.read(cx).value().to_string();
        self.filter.authors = self.users.read(cx).authors();
        self.filter.paths = self.paths.read(cx).paths();
        self.filter.after = self.dates.after;
        self.filter.before = self.dates.before;
        self.reload(false, cx);
    }
    fn reload(&mut self, preserve: bool, cx: &mut Context<Self>) {
        self.token.cancel();
        self.token = Cancel::default();
        if !preserve {
            self.clear_window();
            self.offset = 0;
        }
        self.replacing = preserve;
        self.query = None;
        self.total = None;
        self.count_error = None;
        self.error = None;
        self.more = true;
        self.loading = true;
        let repo = self.repo.clone();
        let pair = self.pair.clone();
        let side = self.side;
        let filter = self.filter.clone();
        let token = self.token.clone();
        let check = token.clone();
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { Query::prepare(&repo, pair, side, filter, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if check.cancelled() {
                    return;
                }
                s.loading = false;
                match result {
                    Ok(query) => {
                        s.query = Some(Arc::new(query));
                        s.load_count(cx);
                        s.load_more(cx);
                    }
                    Err(e) => s.error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn load_count(&mut self, cx: &mut Context<Self>) {
        let Some(query) = self.query.clone() else {
            return;
        };
        let repo = self.repo.clone();
        let token = self.token.clone();
        self.count_task = cx.spawn(async move |this, cx| {
            let check = token.clone();
            let result = cx
                .background_spawn(async move { query.count(&repo, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if check.cancelled() {
                    return;
                }
                match result {
                    Ok(n) => s.total = Some(n),
                    Err(e) => s.count_error = Some(e),
                }
                cx.notify();
            });
        });
    }
    fn filter_overflow(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        Button::new("compare-filter-overflow")
            .ghost()
            .small()
            .w(px(28.))
            .flex_shrink_0()
            .icon(jb_icon("chevronRight_dark.svg"))
            .tooltip(t("Filters"))
            .accessibility_label(t("Filters"))
            .dropdown_menu(move |mut menu, _, _| {
                let hit_owner = owner.clone();
                menu = menu
                    .min_w(px(180.))
                    .label(t("Filters"))
                    .close_on_confirm(false)
                    .child_popup_hit_test(move |position, cx| {
                        hit_owner.upgrade().is_some_and(|view| {
                            let s = view.read(cx);
                            s.users.read(cx).contains_popup_position(position)
                                || s.paths.read(cx).contains_popup_position(position)
                        })
                    });
                for filter in 0..3 {
                    let owner = owner.clone();
                    menu = menu.item(PopupMenuItem::element(move |_, cx| {
                        owner
                            .update(cx, |s, cx| match filter {
                                0 => s.users.clone().into_any_element(),
                                1 => s.date_control(cx).into_any_element(),
                                _ => s.paths.clone().into_any_element(),
                            })
                            .unwrap_or_else(|_| div().into_any_element())
                    }));
                }
                menu
            })
    }
    fn date_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        h_flex()
            .gap_0()
            .child(
                Button::new("compare-date")
                    .ghost()
                    .small()
                    .selected(!self.dates.is_empty())
                    .child(
                        h_flex()
                            .gap_1()
                            .child(self.dates.caption())
                            .child(jb_icon("chevronDown_dark.svg")),
                    )
                    .dropdown_menu(move |mut menu, _, _| {
                        let select = owner.clone();
                        menu = menu.item(PopupMenuItem::new(t("Select…")).on_click(
                            move |_, w, cx| {
                                let _ = select.update(cx, |s, cx| s.select_period(w, cx));
                            },
                        ));
                        let presets = plico::date_filter::presets(chrono::Local::now());
                        for (label, index) in [("Last 24 hours", 0), ("Last 7 days", 1)] {
                            let owner = owner.clone();
                            let presets = presets.clone();
                            menu = menu.item(PopupMenuItem::new(t(label)).on_click(
                                move |_, _, cx| {
                                    let _ = owner.update(cx, |s, cx| match &presets {
                                        Ok(dates) => {
                                            s.dates = plico::date_filter::Bounds {
                                                after: Some(dates[index]),
                                                before: None,
                                            };
                                            s.apply_filter(cx);
                                        }
                                        Err(e) => {
                                            s.error = Some(e.clone());
                                            cx.notify();
                                        }
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
            .when(!self.dates.is_empty(), |v| {
                v.child(
                    Button::new("compare-clear-date")
                        .ghost()
                        .small()
                        .icon(jb_icon("closeSmall.svg"))
                        .tooltip(t("Clear date filter"))
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.dates = Default::default();
                            s.apply_filter(cx);
                        })),
                )
            })
    }
    fn select_period(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = cx.new(|cx| crate::date_dialog::DateDialog::new(self.dates, window, cx));
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let confirm = draft.clone();
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
                    let Ok(bounds) = confirm.update(cx, |s, cx| s.confirm(cx)) else {
                        return false;
                    };
                    if !bounds.is_empty() {
                        let _ = owner.update(cx, |s, cx| {
                            s.dates = bounds;
                            s.apply_filter(cx);
                        });
                    }
                    true
                })
        });
    }
    fn clear_window(&mut self) {
        self.commits.clear();
        self.graph.clear();
        self.replacing = false;
        self.clear_details();
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
    }
    fn clear_details(&mut self) {
        // A hidden pane may not render again after a refresh invalidates its selection.
        // Drop the old preview now so its request is cancelled and content released.
        self.preview = None;
        self.selected = None;
        self.selection = Default::default();
        self.presentations.clear();
        self.containing_branches = None;
        self.refs_request = Default::default();
        self.details_total = 0;
        self.chain = None;
        self.selected_file = None;
        self.selected_folder = None;
        self.files_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.file_rows.clear();
        self.detail_token.cancel();
        self.detail_generation += 1;
        self.detail_loading = false;
        self.detail_error = None;
    }
    fn previous_window(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.offset == 0 {
            return;
        }
        self.offset = self.offset.saturating_sub(git::HISTORY_MAX);
        self.clear_window();
        self.more = true;
        self.load_more(cx);
    }
    fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.loading || !self.more {
            return;
        }
        let Some(query) = self.query.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        let repo = self.repo.clone();
        let token = self.token.clone();
        if !self.replacing && self.commits.len() >= git::HISTORY_MAX {
            self.offset += self.commits.len();
            self.clear_window();
        }
        let replacing = self.replacing;
        let (offset, limit) = if replacing {
            (self.offset, self.commits.len().max(git::HISTORY_PAGE))
        } else {
            (self.offset + self.commits.len(), git::HISTORY_PAGE)
        };
        self.task = cx.spawn(async move |this, cx| {
            let check = token.clone();
            let result = cx
                .background_spawn(async move {
                    if query.needs_graph_projection() {
                        query
                            .graph_page(&repo, offset, limit, &token)
                            .map(|(page, rows)| (page, Some(rows)))
                    } else {
                        query
                            .page(&repo, offset, limit, &token)
                            .map(|page| (page, None))
                    }
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if check.cancelled() {
                    return;
                }
                s.loading = false;
                match result {
                    Ok((page, projected_rows)) => {
                        s.more = page.more;
                        if replacing {
                            super::log_viewport::restore(
                                &s.scroll,
                                &s.commits
                                    .iter()
                                    .map(|commit| &commit.id)
                                    .collect::<Vec<_>>(),
                                &page
                                    .commits
                                    .iter()
                                    .map(|commit| &commit.id)
                                    .collect::<Vec<_>>(),
                                s.offset,
                                28.,
                            );
                            let before = s
                                .selection
                                .ordered(
                                    &s.commits.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
                                )
                                .into_iter()
                                .map(str::to_owned)
                                .collect::<Vec<_>>();
                            s.commits = page.commits;
                            s.replacing = false;
                            let ids = s.commits.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
                            s.selection.reconcile(&ids);
                            if s.selected
                                .as_ref()
                                .is_some_and(|id| !s.selection.contains(id))
                            {
                                s.reload_selection(cx);
                            } else if before
                                != s.selection
                                    .ordered(&ids)
                                    .into_iter()
                                    .map(str::to_owned)
                                    .collect::<Vec<_>>()
                            {
                                s.reload_selection(cx);
                            } else {
                                s.refresh_card_refs(cx);
                            }
                        } else {
                            s.commits.extend(page.commits);
                        }
                        if let Some(rows) = projected_rows {
                            if replacing {
                                s.graph = rows;
                            } else {
                                s.graph.extend(rows);
                            }
                        } else {
                            match graph::layout_bounded(&s.commits, 500_000) {
                                Ok(rows) => s.graph = rows,
                                Err(e) => s.error = Some(e),
                            }
                        }
                    }
                    Err(e) => s.error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn step_commits(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let ids = self
            .commits
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return;
        }
        if let Some(lead) = self.selection.lead(&ids) {
            if lead
                .checked_add_signed(delta)
                .is_none_or(|index| index >= ids.len())
            {
                return;
            }
        }
        self.selection.step(&ids, delta, extend);
        if let Some(index) = self.selection.lead(&ids) {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
        self.reload_selection(cx);
    }
    fn select_commits(&mut self, index: usize, toggle: bool, extend: bool, cx: &mut Context<Self>) {
        let ids = self
            .commits
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>();
        self.selection.select(&ids, index, toggle, extend);
        self.reload_selection(cx);
    }
    fn reload_selection(&mut self, cx: &mut Context<Self>) {
        let ids = self
            .commits
            .iter()
            .filter(|c| self.selection.contains(&c.id))
            .map(|c| c.id.clone())
            .collect::<Vec<_>>();
        let Some(commit) = self
            .commits
            .iter()
            .find(|c| self.selection.contains(&c.id))
            .cloned()
        else {
            self.clear_details();
            cx.notify();
            return;
        };
        self.preview = None;
        self.selected = Some(commit.id.clone());
        self.presentations.clear();
        self.containing_branches = None;
        self.refs_request = Default::default();
        self.details_total = 0;
        self.chain = None;
        self.selected_file = None;
        self.selected_folder = None;
        self.files_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.file_rows.clear();
        self.detail_error = None;
        self.detail_loading = true;
        self.detail_token.cancel();
        self.detail_token = Cancel::default();
        self.detail_generation += 1;
        let generation = self.detail_generation;
        let token = self.detail_token.clone();
        let repo = self.repo.clone();
        self.details_total = ids.len();
        self.detail_task = cx.spawn(async move |this, cx| {
            let result: Result<_, String> = cx
                .background_spawn(async move {
                    let (commits, changes) = if ids.len() == 1 {
                        let changes = repo.log_commit_changes(&commit, &token)?;
                        let commits = repo.snapshot_commits(&ids, &ids, &token)?.commits;
                        (commits, changes)
                    } else {
                        let snapshot = plico::log_changes::load(&repo, &ids, &token)?;
                        (snapshot.commits, snapshot.files)
                    };
                    let mut presentations = Vec::new();
                    let mut bytes = 0usize;
                    for commit in commits.iter().take(50) {
                        let details = repo.commit_metadata(commit, &token)?;
                        let presentation =
                            plico::commit_presentation::build(&details, Some(commit));
                        bytes = bytes
                            .saturating_add(presentation.message.len())
                            .saturating_add(
                                presentation.metadata.iter().map(String::len).sum::<usize>(),
                            );
                        if bytes > git::OUTPUT_LIMIT {
                            return Err("Log details exceed the preview output limit".into());
                        }
                        presentations.push((presentation, commit.refs.clone(), commit.id.clone()));
                    }
                    let chain = if changes.is_empty() {
                        None
                    } else {
                        Some(diff_target::Chain::combined(repo, changes, 0)?.1)
                    };
                    Ok((presentations, chain))
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.detail_generation {
                    return;
                }
                s.detail_loading = false;
                match result {
                    Ok((presentations, chain)) => {
                        let mut requests = Vec::new();
                        s.presentations = presentations
                            .into_iter()
                            .map(|(presentation, refs, id)| {
                                let branches =
                                    cx.new(crate::containing_branches::ContainingBranches::pending);
                                requests.push((id.clone(), branches.downgrade()));
                                DetailCard {
                                    id,
                                    presentation,
                                    refs,
                                    branches,
                                }
                            })
                            .collect();
                        let repo = s.repo.clone();
                        s.containing_branches =
                            Some(cx.new(|cx| {
                                crate::containing_branches::Batch::new(repo, requests, cx)
                            }));
                        s.chain = chain;
                        s.collapsed_files.clear();
                        s.rebuild_file_rows();
                    }
                    Err(e) => s.detail_error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn refresh_card_refs(&mut self, cx: &mut Context<Self>) {
        self.refs_request = Default::default();
        if self.presentations.is_empty() {
            return;
        }
        let ids = self
            .presentations
            .iter()
            .map(|card| card.id.clone())
            .collect::<Vec<_>>();
        let repo = self.repo.clone();
        let token = self.refs_request.token.clone();
        let check = token.clone();
        self.refs_request.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    repo.snapshot_commits(&ids, &ids, &token)
                        .map(|snapshot| snapshot.commits)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if check.cancelled() {
                    return;
                }
                match result {
                    Ok(commits) => {
                        s.containing_branches = None;
                        let mut requests = Vec::new();
                        for card in &mut s.presentations {
                            if let Some(commit) = commits.iter().find(|commit| commit.id == card.id)
                            {
                                card.refs = commit.refs.clone();
                                card.branches =
                                    cx.new(crate::containing_branches::ContainingBranches::pending);
                                requests.push((card.id.clone(), card.branches.downgrade()));
                            }
                        }
                        let repo = s.repo.clone();
                        s.containing_branches =
                            Some(cx.new(|cx| {
                                crate::containing_branches::Batch::new(repo, requests, cx)
                            }));
                        cx.notify();
                    }
                    Err(error) => eprintln!("Cannot refresh comparison references: {error}"),
                }
            });
        });
    }
    fn open_file(&mut self, index: usize, cx: &mut Context<Self>) {
        let order = self.file_navigation_order();
        if let Some(chain) = &mut self.chain {
            if let Some(target) = chain.select(index) {
                if let Some(chain) = chain.reordered(&order) {
                    cx.emit(OpenDiff(target, chain));
                }
            }
        }
    }
}
impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_preview(window, cx);
        let empty = self.commits.is_empty() && !self.loading && self.error.is_none();
        let empty_filtered = empty
            && (!self.filter.text.trim().is_empty()
                || self.filter.authors.is_some()
                || !self.filter.paths.is_empty()
                || self.filter.after.is_some()
                || self.filter.before.is_some());
        let (included, excluded) = match self.side {
            Side::First => (&self.pair.first_ref, &self.pair.second_ref),
            Side::Second => (&self.pair.second_ref, &self.pair.first_ref),
        };
        let label = |s: &str| {
            s.strip_prefix("refs/heads/")
                .or_else(|| s.strip_prefix("refs/remotes/"))
                .unwrap_or(s)
                .to_owned()
        };
        let explanation = if i18n::is_chinese() {
            format!(
                "存在于 {}，但不存在于 {} 的提交",
                label(included),
                label(excluded)
            )
        } else {
            format!(
                "Commits that exist in {} but don't exist in {}",
                label(included),
                label(excluded)
            )
        };
        let toolbar_owner = cx.entity().downgrade();
        let list = v_flex()
            .size_full()
            .min_h_0()
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .on_prepaint(move |bounds, window, cx| {
                        let Some(owner) = toolbar_owner.upgrade() else {
                            return;
                        };
                        if (owner.read(cx).toolbar_width - bounds.size.width).abs() <= px(0.5) {
                            return;
                        }
                        let weak = owner.downgrade();
                        let width = bounds.size.width;
                        window.defer(cx, move |_, cx| {
                            let _ = weak.update(cx, |s, cx| {
                                s.toolbar_width = width;
                                cx.notify();
                            });
                        });
                    })
                    .h(px(LOG_TOOLBAR_HEIGHT))
                    .flex_shrink_0()
                    .px_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div().flex_1().min_w_0().max_w(px(260.)).child(
                            Input::new(&self.search)
                                .small()
                                .prefix(jb_icon("search_dark.svg"))
                                .suffix(
                                    h_flex()
                                        .child(
                                            Button::new("compare-regex")
                                                .ghost()
                                                .small()
                                                .label(".*")
                                                .selected(self.filter.regex)
                                                .tooltip(t("Regular expression"))
                                                .on_click(cx.listener(|s, _, _, cx| {
                                                    s.filter.regex = !s.filter.regex;
                                                    s.apply_filter(cx);
                                                })),
                                        )
                                        .child(
                                            Button::new("compare-case")
                                                .ghost()
                                                .small()
                                                .label("Cc")
                                                .selected(self.filter.match_case)
                                                .tooltip(t("Match case"))
                                                .on_click(cx.listener(|s, _, _, cx| {
                                                    s.filter.match_case = !s.filter.match_case;
                                                    s.apply_filter(cx);
                                                })),
                                        ),
                                ),
                        ),
                    )
                    .when(self.toolbar_width >= px(650.), |bar| {
                        bar.child(self.users.clone())
                            .child(self.date_control(cx))
                            .child(self.paths.clone())
                    })
                    .when(self.toolbar_width < px(650.), |bar| {
                        bar.child(self.filter_overflow(cx))
                    })
                    .child(
                        Button::new("compare-clear")
                            .ghost()
                            .small()
                            .icon(jb_icon("closeSmall.svg"))
                            .tooltip(t("Clear filters"))
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.clear_filters(w, cx);
                            })),
                    ),
            )
            .child(div().px_3().py_2().bg(rgb(0x45351c)).child(explanation))
            .when_some(self.error.clone(), |v, e| v.child(div().px_3().child(e)))
            .when(empty, |v| {
                let message = if empty_filtered {
                    t("No commits matching filters").to_owned()
                } else {
                    t("{containing} contains all commits from {included}")
                        .replace("{containing}", &label(excluded))
                        .replace("{included}", &label(included))
                };
                v.child(
                    v_flex()
                        .flex_1()
                        .w_full()
                        .min_w_0()
                        .px_3()
                        .items_center()
                        .pt(px(60.))
                        .gap_2()
                        .text_color(rgb(MUTED))
                        .child(div().w_full().text_center().child(message))
                        .when(empty_filtered, |v| {
                            v.child(
                                Button::new("compare-reset-empty")
                                    .link()
                                    .small()
                                    .label(t("Reset filters"))
                                    .on_click(cx.listener(|s, _, w, cx| s.clear_filters(w, cx))),
                            )
                        }),
                )
            })
            .when(!empty, |v| {
                v.child(
                    div()
                        .id("compare-commits-focus")
                        .flex_1()
                        .min_h_0()
                        .track_focus(&self.commits_focus)
                        .key_context("BranchCompareCommits")
                        .on_action(
                            cx.listener(|s, _: &CommitNext, _, cx| s.step_commits(1, false, cx)),
                        )
                        .on_action(
                            cx.listener(|s, _: &CommitPrevious, _, cx| {
                                s.step_commits(-1, false, cx)
                            }),
                        )
                        .on_action(
                            cx.listener(|s, _: &CommitExtendNext, _, cx| {
                                s.step_commits(1, true, cx)
                            }),
                        )
                        .on_action(cx.listener(|s, _: &CommitExtendPrevious, _, cx| {
                            s.step_commits(-1, true, cx)
                        }))
                        .child(
                            uniform_list(
                                "compare-commits",
                                self.commits.len(),
                                cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .filter_map(|i| {
                                            let commit = s.commits.get(i)?;
                                            let row = s.graph.get(i)?.clone();
                                            let selected = s.selection.contains(&commit.id);
                                            Some(
                                                h_flex()
                                                    .w_full()
                                                    .id(i)
                                                    .h(px(28.))
                                                    .gap_2()
                                                    .overflow_hidden()
                                                    .when(selected, |v| v.bg(rgb(0x2e4773)))
                                                    .child(graph_cell(
                                                        row,
                                                        56.,
                                                        28.,
                                                        GraphNodeStyle {
                                                            head: false,
                                                            background: if selected {
                                                                0x2e4773
                                                            } else {
                                                                BG
                                                            },
                                                        },
                                                        None,
                                                    ))
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .truncate()
                                                            .child(commit.subject.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(130.))
                                                            .truncate()
                                                            .text_color(rgb(MUTED))
                                                            .child(commit.author.clone()),
                                                    )
                                                    .on_click(cx.listener(
                                                        move |s, e: &ClickEvent, window, cx| {
                                                            s.commits_focus.focus(window, cx);
                                                            s.select_commits(
                                                                i,
                                                                e.modifiers().control
                                                                    || e.modifiers().platform,
                                                                e.modifiers().shift,
                                                                cx,
                                                            )
                                                        },
                                                    )),
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                }),
                            )
                            .size_full()
                            .track_scroll(&self.scroll),
                        ),
                )
            })
            .child(
                h_flex()
                    .px_3()
                    .py_1()
                    .gap_3()
                    .child(match self.total {
                        Some(n) => format!(
                            "{}–{} / {n}",
                            if self.commits.is_empty() {
                                0
                            } else {
                                self.offset + 1
                            },
                            self.offset + self.commits.len()
                        ),
                        None => self.commits.len().to_string(),
                    })
                    .when_some(self.count_error.clone(), |v, e| {
                        v.child(div().text_color(rgb(MUTED)).child(e))
                    })
                    .when(self.offset > 0, |v| {
                        v.child(
                            Button::new("compare-previous")
                                .ghost()
                                .label(t("Previous"))
                                .disabled(self.loading)
                                .on_click(cx.listener(|s, _, _, cx| s.previous_window(cx))),
                        )
                    })
                    .when(self.more, |v| {
                        v.child(
                            Button::new("compare-more")
                                .ghost()
                                .label(t("Load more"))
                                .disabled(self.loading)
                                .on_click(cx.listener(|s, _, _, cx| s.load_more(cx))),
                        )
                    }),
            );
        let files = v_flex()
            .size_full()
            .min_h_0()
            .px_3()
            .py_2()
            .child(self.files_toolbar(cx))
            .when(self.selected.is_none(), |v| {
                v.child(t("Select commits to view changes"))
            })
            .when(self.detail_loading, |v| {
                v.child(t("Loading commit details…"))
            })
            .when_some(self.detail_error.clone(), |v, e| v.child(e))
            .child(
                div()
                    .id("compare-files")
                    .track_focus(&self.files_focus)
                    .key_context("BranchCompareFiles")
                    .on_action(cx.listener(|s, _: &ShowSelectedDiff, _, cx| {
                        if let Some(index) = s.selected_file {
                            s.open_file(index, cx);
                        }
                    }))
                    .on_action(cx.listener(|s, _: &FileNext, _, cx| s.step_file_row(1, cx)))
                    .on_action(cx.listener(|s, _: &FilePrevious, _, cx| s.step_file_row(-1, cx)))
                    .on_action(cx.listener(|s, _: &FileLeft, _, cx| s.expand_file_row(false, cx)))
                    .on_action(cx.listener(|s, _: &FileRight, _, cx| s.expand_file_row(true, cx)))
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list(
                            "compare-file-rows",
                            self.file_rows.len(),
                            cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                                range.map(|row| s.file_row(row, cx)).collect::<Vec<_>>()
                            }),
                        )
                        .size_full()
                        .track_scroll(&self.files_scroll),
                    ),
            );
        let details = v_flex()
            .id("compare-message")
            .min_w_0()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .children(self.presentations.iter().enumerate().map(|(index, card)| {
                let presentation = &card.presentation;
                v_flex()
                    .id(("compare-detail-card", index))
                    .w_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .px(px(14.))
                    .py(px(14.))
                    .gap_2()
                    .when(index > 0, |v| v.border_t_1().border_color(rgb(BORDER)))
                    .child(
                        TextView::html(
                            format!("compare-message-{index}"),
                            presentation.message.clone(),
                        )
                        .font_family("JetBrains Mono")
                        .text_size(rems(1.))
                        .selectable(true),
                    )
                    .child(
                        TextView::html(
                            format!("compare-metadata-{index}"),
                            presentation.metadata[usize::from(i18n::is_chinese())].clone(),
                        )
                        .text_size(rems(1.))
                        .selectable(true),
                    )
                    .child(commit_refs(&card.refs))
                    .child(card.branches.clone())
            }))
            .when(self.details_total > self.presentations.len(), |v| {
                v.child(
                    div()
                        .px(px(14.))
                        .py_2()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .text_color(rgb(MUTED))
                        .child(t("Showing the first 50 commits")),
                )
            });
        let details = if self.presentations.is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child(t("Commit details"))
                .into_any_element()
        } else {
            details.into_any_element()
        };
        let files = if self.show_details {
            v_resizable("compare-files-details")
                .with_state(&self.details_split)
                .on_resize(cx.listener(|s, state: &Entity<ResizableState>, _, cx| {
                    if let [files, details] = state.read(cx).sizes().as_slice() {
                        let total = files.as_f32() + details.as_f32();
                        if total > 0. {
                            s.files_fraction = (files.as_f32() / total).clamp(0.1, 0.9);
                            if let Err(error) =
                                layout_preferences::save_files_fraction(s.files_fraction)
                            {
                                eprintln!("Cannot save layout preference: {error}");
                            }
                        }
                    }
                }))
                .child(
                    resizable_panel()
                        .flex_basis(relative(self.files_fraction))
                        .child(files),
                )
                .child(
                    resizable_panel()
                        .flex_basis(relative(1. - self.files_fraction))
                        .child(details),
                )
                .into_any_element()
        } else {
            files.into_any_element()
        };
        let content = h_resizable("compare-list-details")
            .with_state(&self.split)
            .child(resizable_panel().child(list))
            .child(
                resizable_panel()
                    .size(px(320.))
                    .size_range(px(200.)..px(1200.))
                    .child(files),
            );
        if self.show_preview {
            v_resizable("compare-diff-preview")
                .with_state(&self.preview_split)
                .child(resizable_panel().flex_basis(relative(0.7)).child(content))
                .child(
                    resizable_panel().flex_basis(relative(0.3)).child(
                        div()
                            .size_full()
                            .children(self.preview.clone())
                            .when(self.preview.is_none(), |v| {
                                v.child(div().p_3().child(t("Select a file to view its diff")))
                            }),
                    ),
                )
                .into_any_element()
        } else {
            content.into_any_element()
        }
    }
}

/// Own the resolve task and its cancellation together, including repository disposal.
pub(super) struct Request {
    token: Cancel,
    task: Task<()>,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            token: Cancel::default(),
            task: Task::ready(()),
        }
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Review {
    pub(super) fn branch_compare_item(
        owner: &WeakEntity<Self>,
        reference: &str,
        cx: &App,
    ) -> PopupMenuItem {
        let Some(review) = owner.upgrade() else {
            return PopupMenuItem::new(t("Compare Branches")).disabled(true);
        };
        let review = review.read(cx);
        Self::branch_compare_item_in(
            owner,
            review.repo.clone(),
            review.snapshot.as_ref(),
            reference,
        )
    }
    pub(super) fn branch_compare_item_in(
        owner: &WeakEntity<Self>,
        repo: Repository,
        snapshot: Option<&plico::git::Snapshot>,
        reference: &str,
    ) -> PopupMenuItem {
        let current = snapshot
            .and_then(|s| s.current_branch.as_deref())
            .unwrap_or("HEAD");
        let enabled = (reference.starts_with("refs/heads/")
            || reference.starts_with("refs/remotes/"))
            && reference != format!("refs/heads/{current}")
            && snapshot.is_some_and(|s| s.head_id.is_some());
        let label = t("Compare with {current}").replace("{current}", current);
        let current = snapshot
            .and_then(|s| s.current_branch.as_ref())
            .map(|name| format!("refs/heads/{name}"))
            .unwrap_or_else(|| "HEAD".into());
        let owner = owner.clone();
        let reference = reference.to_owned();
        PopupMenuItem::new(label)
            .disabled(!enabled)
            .on_click(move |_, window, cx| {
                let _ = owner.update(cx, |s, cx| {
                    s.prepare_branch_pair_in(
                        repo.clone(),
                        reference.clone(),
                        current.clone(),
                        window,
                        cx,
                    )
                });
            })
    }
    pub(super) fn branch_working_comparison_item(
        owner: &WeakEntity<Self>,
        reference: &str,
        cx: &App,
    ) -> PopupMenuItem {
        let Some(review) = owner.upgrade() else {
            return PopupMenuItem::new(t("Show Diff with Working Tree")).disabled(true);
        };
        let review = review.read(cx);
        Self::branch_working_comparison_item_in(
            owner,
            review.repo.clone(),
            review.snapshot.as_ref(),
            reference,
        )
    }
    pub(super) fn branch_working_comparison_item_in(
        owner: &WeakEntity<Self>,
        repo: Repository,
        snapshot: Option<&plico::git::Snapshot>,
        reference: &str,
    ) -> PopupMenuItem {
        let enabled = ["refs/heads/", "refs/remotes/", "refs/tags/"]
            .iter()
            .any(|prefix| reference.starts_with(prefix));
        let current = snapshot
            .and_then(|s| s.current_branch.clone())
            .unwrap_or_else(|| "HEAD".into());
        let owner = owner.clone();
        let reference = reference.to_owned();
        PopupMenuItem::new(t("Show Diff with Working Tree"))
            .disabled(!enabled)
            .on_click(move |_, window, cx| {
                let _ = owner.update(cx, |s, cx| {
                    s.prepare_working_comparison_in(
                        repo.clone(),
                        reference.clone(),
                        current.clone(),
                        window,
                        cx,
                    );
                });
            })
    }
    pub(super) fn prepare_working_comparison_in(
        &mut self,
        repo: Repository,
        reference: String,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_repository_menu(cx);
        self.with_saved_documents(window, cx, move |s, window, cx| {
            let view = cx.new(|cx| {
                crate::compare_view::CompareView::working_tree(repo, reference, current, window, cx)
            });
            s.show_comparison(view, window, cx);
        });
    }
    pub(super) fn prepare_branch_pair(
        &mut self,
        reference: String,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prepare_branch_pair_in(self.repo.clone(), reference, current, window, cx);
    }
    pub(super) fn prepare_branch_pair_in(
        &mut self,
        repo: Repository,
        reference: String,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_repository_menu(cx);
        if self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.prepare_branch_pair_in(repo, reference, current, w, cx);
            });
            return;
        }
        if let Some(tab) = self.branch_compare_tabs.iter().find(|tab| {
            let view = tab.read(cx);
            view.repo.path == repo.path
                && view.pair.first_ref == reference
                && view.pair.second_ref == current
        }) {
            self.branch_compare_request = Default::default();
            self.activate_editor_neighbor(Some(tab.entity_id()), window, cx);
            return;
        }
        self.branch_compare_request = Default::default();
        let token = self.branch_compare_request.token.clone();
        let worker_token = token.clone();
        let worker_repo = repo.clone();
        self.branch_compare_request.task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    Pair::resolve(&worker_repo, &reference, &current, &worker_token)
                })
                .await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    if token.cancelled() {
                        return;
                    }
                    match result {
                        Ok(pair) => s.open_branch_comparison_in(repo, pair, window, cx),
                        Err(error) => {
                            s.error = Some(error);
                            cx.notify();
                        }
                    }
                });
            });
        });
    }
    #[cfg(all(test, feature = "desktop-tests"))]
    pub(super) fn open_branch_comparison(
        &mut self,
        pair: Pair,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_branch_comparison_in(self.repo.clone(), pair, window, cx);
    }
    pub(super) fn open_branch_comparison_in(
        &mut self,
        repo: Repository,
        pair: Pair,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.branch_compare_tabs.iter().find(|tab| {
            let view = tab.read(cx);
            view.repo.path == repo.path
                && view.pair.first_ref == pair.first_ref
                && view.pair.second_ref == pair.second_ref
        }) {
            self.activate_editor_neighbor(Some(tab.entity_id()), window, cx);
            return;
        }
        let view = cx.new(|cx| View::new(repo, pair, window, cx));
        cx.subscribe_in(&view, window, |s, source, event: &OpenDiff, window, cx| {
            let panes = source.read(cx).panes.clone();
            let Some(pane) = panes.into_iter().find(|pane| {
                pane.read(cx)
                    .chain
                    .as_ref()
                    .and_then(|c| c.fork_for(&event.0))
                    .is_some()
            }) else {
                return;
            };
            let existing = pane
                .read(cx)
                .editor_preview
                .as_ref()
                .and_then(|tab| tab.upgrade());
            if let Some(index) = existing
                .and_then(|tab| s.tabs.iter().position(|t| t.entity_id() == tab.entity_id()))
            {
                s.tabs[index].update(cx, |tab, cx| {
                    tab.sync_source_selection(
                        Some((event.0.clone(), event.1.clone())),
                        false,
                        window,
                        cx,
                    );
                });
                s.activate_diff_tab(index, window, cx);
                return;
            }
            let tab = cx.new(|_| {
                let mut tab = DiffTab::new(event.0.clone(), event.1.clone());
                tab.documents = s.documents.clone();
                tab
            });
            {
                pane.update(cx, |p, cx| {
                    p.editor_preview = Some(tab.downgrade());
                    p.track_diff_navigation(&tab, cx);
                });
                let weak_tab = tab.downgrade();
                cx.observe_in(&pane, window, move |s, pane, window, cx| {
                    let Some(tab) = weak_tab.upgrade() else {
                        return;
                    };
                    let Some(index) = s.tabs.iter().position(|t| t.entity_id() == tab.entity_id())
                    else {
                        return;
                    };
                    let selection = pane
                        .read(cx)
                        .navigation_selection(Some(&tab.read(cx).target));
                    let active = s.active_diff_index() == Some(index);
                    tab.update(cx, |t, cx| {
                        t.sync_source_selection(selection, active, window, cx)
                    });
                })
                .detach();
            }
            cx.observe(&tab, |_, _, cx| cx.notify()).detach();
            s.insert_editor_tab(tab.entity_id());
            s.tabs.push(tab);
            s.activate_diff_tab(s.tabs.len() - 1, window, cx);
        })
        .detach();
        cx.subscribe(&view, |s, _, event: &PreviewChanged, cx| {
            for view in &s.branch_compare_tabs {
                view.update(cx, |v, cx| {
                    for pane in &v.panes {
                        pane.update(cx, |p, cx| {
                            if p.side == event.0 {
                                p.show_preview = event.1;
                                if !event.1 {
                                    if let Some(preview) = p.preview.take() {
                                        preview.update(cx, |p, cx| p.deactivate(cx));
                                    }
                                }
                                cx.notify();
                            }
                        });
                    }
                });
            }
        })
        .detach();
        let id = view.entity_id();
        self.insert_editor_tab(id);
        self.branch_compare_tabs.push(view);
        self.activate_editor_neighbor(Some(id), window, cx);
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;

    #[gpui::test]
    fn reference_refresh_keeps_applied_filters_and_recovers_deleted_reference(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-compare-refresh-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        std::fs::create_dir(&fixture.0).unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        let commit = |message: &str| {
            std::fs::write(fixture.0.join("review.txt"), format!("{message}\n")).unwrap();
            std::fs::create_dir_all(fixture.0.join("nested")).unwrap();
            std::fs::write(
                fixture.0.join("nested/second.txt"),
                format!("second {message}\n"),
            )
            .unwrap();
            git(&["add", "review.txt", "nested/second.txt"]);
            git(&[
                "-c",
                "user.name=Compare",
                "-c",
                "user.email=compare@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-qm",
                message,
            ])
        };
        git(&["init", "-q", "-b", "main"]);
        commit("base");
        git(&["checkout", "-qb", "topic"]);
        commit("topic one");
        let repo = Repository::new(fixture.0.clone());
        let pair = Pair::resolve(
            &repo,
            "refs/heads/topic",
            "refs/heads/main",
            &Cancel::default(),
        )
        .unwrap();
        cx.update(gpui::component::init);
        let mut comparison = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| View::new(repo, pair, window, cx));
            comparison = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = comparison.unwrap();
        cx.run_until_parked();
        cx.update(|window, cx| {
            let first = view.read(cx).panes[0].clone();
            first.update(cx, |pane, cx| {
                pane.search
                    .update(cx, |input, cx| input.set_value("topic", window, cx));
                pane.apply_filter(cx);
                pane.search.update(cx, |input, cx| {
                    input.set_value("unsubmitted draft", window, cx)
                });
            });
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.panes[0].update(cx, |pane, cx| pane.select_commits(0, false, false, cx));
        });
        cx.run_until_parked();
        let selected = view
            .read_with(cx, |v, cx| v.panes[0].read(cx).selected.clone())
            .unwrap();
        cx.update(|window, cx| {
            let first = view.read(cx).panes[0].clone();
            let second = view.read(cx).panes[1].clone();
            first.update(cx, |pane, cx| {
                pane.show_preview = true;
                pane.sync_preview(window, cx);
                let preview = pane
                    .preview
                    .as_ref()
                    .expect("selected commit has a preview")
                    .clone();
                assert_eq!(
                    preview.read(cx).target.identity().revision.as_ref(),
                    Some(&selected)
                );
                pane.sync_preview(window, cx);
                assert_eq!(
                    pane.preview.as_ref().unwrap().entity_id(),
                    preview.entity_id(),
                    "unchanged selection must reuse its viewer"
                );
                assert_eq!(
                    preview.read(cx).target.change().path,
                    PathBuf::from("nested/second.txt")
                );
                pane.selected_file = pane
                    .chain
                    .as_ref()
                    .unwrap()
                    .changes()
                    .iter()
                    .position(|c| c.path == PathBuf::from("review.txt"));
                pane.sync_preview(window, cx);
                assert_eq!(
                    pane.preview.as_ref().unwrap().read(cx).target.change().path,
                    PathBuf::from("review.txt")
                );
                pane.selected_file = None;
                pane.selected_folder = Some(PathBuf::from("nested"));
                pane.collapsed_files.insert(PathBuf::from("nested"));
                pane.rebuild_file_rows();
                pane.sync_preview(window, cx);
                let preview = pane.preview.as_ref().unwrap().clone();
                assert_eq!(
                    preview.read(cx).target.change().path,
                    PathBuf::from("nested/second.txt")
                );
                assert_eq!(
                    preview
                        .read(cx)
                        .fork_chain(&preview.read(cx).target)
                        .unwrap()
                        .len(),
                    2
                );
                pane.sync_preview(window, cx);
                assert_eq!(
                    pane.preview.as_ref().unwrap().entity_id(),
                    preview.entity_id()
                );
                pane.selected_folder = Some(PathBuf::new());
                pane.sync_preview(window, cx);
                assert_eq!(
                    pane.preview.as_ref().unwrap().entity_id(),
                    preview.entity_id(),
                    "parent selection must retain its existing child preview"
                );
                pane.selected_folder = None;
                let identity = preview.read(cx).target.identity().clone();
                pane.group_files = false;
                pane.rebuild_file_rows();
                pane.sync_preview(window, cx);
                assert_eq!(
                    pane.preview.as_ref().unwrap().entity_id(),
                    preview.entity_id()
                );
                assert_eq!(preview.read(cx).target.identity(), &identity);
                let flat_chain = preview
                    .read(cx)
                    .fork_chain(&preview.read(cx).target)
                    .unwrap();
                assert_eq!(
                    flat_chain.index(),
                    1,
                    "flat sorting updates the retained viewer's cursor"
                );
                assert_eq!(flat_chain.changes()[0].path, PathBuf::from("review.txt"));
                pane.group_files = true;
                pane.rebuild_file_rows();
                pane.sync_preview(window, cx);
                pane.show_preview = false;
                pane.sync_preview(window, cx);
                assert!(pane.preview.is_none());
                pane.show_preview = true;
                pane.sync_preview(window, cx);
                assert_ne!(
                    pane.preview.as_ref().unwrap().entity_id(),
                    preview.entity_id(),
                    "reopening creates a fresh viewer after disposal"
                );
                pane.clear_details();
                assert!(
                    pane.preview.is_none(),
                    "clear must release preview without a render pass"
                );
                assert!(
                    pane.show_preview,
                    "invalidating data must preserve the user's preview option"
                );
                pane.select_commits(0, false, false, cx);
                pane.show_preview = false;
            });
            assert!(!second.read(cx).show_preview);
            assert!(second.read(cx).preview.is_none());
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let first = view.read(cx).panes[0].clone();
            let preview = first.update(cx, |pane, cx| {
                pane.show_preview = true;
                pane.selected_file = Some(0);
                pane.sync_preview(window, cx);
                pane.collapsed_files.insert(PathBuf::new());
                pane.rebuild_file_rows();
                pane.preview.as_ref().unwrap().clone()
            });
            preview.update(cx, |p, cx| p.move_file(true, window, cx));
        });
        cx.run_until_parked();
        view.update(cx, |v, cx| {
            v.panes[0].update(cx, |pane, cx| {
                assert_eq!(pane.selected_file, Some(1));
                assert!(!pane.collapsed_files.contains(&PathBuf::new()));
                assert!(
                    pane.active_file_row().is_some(),
                    "navigation must reveal its selected file"
                );
                assert_eq!(
                    pane.preview.as_ref().unwrap().read(cx).target.change().path,
                    pane.chain.as_ref().unwrap().changes()[1].path
                );
                pane.show_preview = false;
                pane.preview = None;
            });
            assert!(v.panes[1].read(cx).preview.is_none());
        });
        let old_branches = view.read_with(cx, |v, cx| {
            let pane = v.panes[0].read(cx);
            assert!(pane.presentations[0].refs.iter().any(|r| r.name == "topic"));
            pane.presentations[0].branches.downgrade()
        });
        git(&["branch", "details-ref-probe", "topic"]);
        view.update(cx, |v, cx| v.refresh(false, cx));
        cx.run_until_parked();
        view.read_with(cx, |v, cx| {
            let pane = v.panes[0].read(cx);
            assert_eq!(pane.selected.as_ref(), Some(&selected));
            assert!(
                pane.presentations[0]
                    .refs
                    .iter()
                    .any(|r| r.name == "details-ref-probe")
            );
            assert!(
                old_branches.upgrade().is_none(),
                "old ancestry request belongs to an obsolete card"
            );
            assert!(
                pane.chain.is_some(),
                "reference refresh must retain the file chain"
            );
        });
        git(&["branch", "-D", "details-ref-probe"]);
        view.update(cx, |v, cx| v.refresh(false, cx));
        cx.run_until_parked();
        view.read_with(cx, |v, cx| {
            let pane = v.panes[0].read(cx);
            assert!(
                !pane.presentations[0]
                    .refs
                    .iter()
                    .any(|r| r.name == "details-ref-probe")
            );
            assert_eq!(pane.selected.as_ref(), Some(&selected));
        });
        commit("hidden intermediate");
        commit("topic two");
        let old_pair = view.read_with(cx, |view, _| view.pair.clone());
        view.update(cx, |view, cx| {
            use plico::worktree_events::Batch;
            view.repository_changed(
                &fixture.0.join("other"),
                &Batch {
                    refs: true,
                    ..Default::default()
                },
                cx,
            );
            view.repository_changed(
                &fixture.0,
                &Batch {
                    index: true,
                    paths: [PathBuf::from("review.txt")].into_iter().collect(),
                    ..Default::default()
                },
                cx,
            );
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.pair, old_pair,
                "unrelated roots and staging do not resolve branch ranges"
            )
        });
        view.update(cx, |view, cx| {
            view.repository_changed(
                &fixture.0,
                &plico::worktree_events::Batch {
                    refs: true,
                    ..Default::default()
                },
                cx,
            );
            let superseded = view.refresh_request.token.clone();
            view.refresh(false, cx);
            assert!(superseded.cancelled());
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let first = view.panes[0].read(cx);
            assert!(first.error.is_none(), "{:?}", first.error);
            assert_eq!(first.commits.len(), 2);
            assert_eq!(first.graph.len(), first.commits.len());
            assert!(first.graph.iter().flat_map(|row| row.segments.iter()).any(|edge| edge.dotted()),
                "filter refresh must project hidden ancestry instead of drawing disconnected raw parents");
            assert_eq!(first.total, Some(2));
            assert_eq!(first.selected.as_ref(), Some(&selected));
            assert_eq!(first.presentations.len(), 1);
            let presentation = &first.presentations[0].presentation;
            assert!(presentation.message.contains("topic one"));
            assert!(presentation.metadata[0].contains(&selected[..8]));
            assert_eq!(first.filter.text, "topic");
            assert_eq!(first.search.read(cx).value().as_str(), "unsubmitted draft");
            assert!(view.panes[1].read(cx).commits.is_empty());
        });
        view.update(cx, |view, cx| {
            view.panes[0].update(cx, |pane, cx| pane.select_commits(0, true, false, cx))
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let pane = view.panes[0].read(cx);
            assert!(pane.detail_error.is_none(), "{:?}", pane.detail_error);
            assert!(
                pane.commits
                    .iter()
                    .all(|commit| pane.selection.contains(&commit.id))
            );
            assert_eq!(pane.presentations.len(), 2);
            assert_eq!(pane.details_total, 2);
            assert!(pane.containing_branches.is_some());
            assert!(
                pane.presentations[0]
                    .refs
                    .iter()
                    .any(|label| label.name == "topic")
            );
            assert!(
                pane.presentations[0]
                    .presentation
                    .message
                    .contains("topic two")
            );
            assert!(
                pane.presentations[1]
                    .presentation
                    .message
                    .contains("topic one")
            );
            let chain = pane.chain.as_ref().unwrap();
            let target = chain.clone().select(0).unwrap();
            let contents = target.read_contents(&Cancel::default()).unwrap().unwrap();
            assert!(contents.before.unwrap().contains("base"));
            assert!(contents.after.unwrap().contains("topic two"));
        });
        view.update(cx, |view, cx| {
            view.panes[0].update(cx, |pane, cx| pane.select_commits(0, true, false, cx))
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let pane = view.panes[0].read(cx);
            assert!(!pane.selection.contains(&pane.commits[0].id));
            assert!(pane.selection.contains(&pane.commits[1].id));
            assert_eq!(pane.selected.as_ref(), Some(&selected));
        });
        cx.update(|window, cx| {
            let pane = view.read(cx).panes[0].clone();
            pane.update(cx, |pane, cx| {
                pane.select_commits(1, false, false, cx);
                pane.commits_focus.focus(window, cx);
            });
        });
        cx.simulate_keystrokes("shift-up");
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let pane = view.panes[0].read(cx);
            assert!(
                pane.commits
                    .iter()
                    .all(|commit| pane.selection.contains(&commit.id))
            );
        });
        cx.simulate_keystrokes("shift-down");
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let pane = view.panes[0].read(cx);
            assert!(!pane.selection.contains(&pane.commits[0].id));
            assert!(pane.selection.contains(&pane.commits[1].id));
        });
        cx.simulate_keystrokes("down");
        view.read_with(cx, |view, cx| {
            let pane = view.panes[0].read(cx);
            assert_eq!(
                pane.selected.as_ref(),
                Some(&selected),
                "last row must not wrap"
            );
        });
        cx.simulate_keystrokes("up");
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let pane = view.panes[0].read(cx);
            assert!(pane.selection.contains(&pane.commits[0].id));
            assert!(!pane.selection.contains(&pane.commits[1].id));
            assert!(view.panes[1].read(cx).selected.is_none());
        });
        git(&["update-ref", "refs/heads/topic", "refs/heads/main"]);
        view.update(cx, |view, cx| view.refresh(false, cx));
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let first = view.panes[0].read(cx);
            assert!(first.error.is_none());
            assert!(first.commits.is_empty());
            assert!(first.selected.is_none());
            assert!(first.presentations.is_empty());
            assert!(first.containing_branches.is_none());
        });
        git(&["checkout", "-q", "main"]);
        git(&["branch", "-D", "topic"]);
        view.update(cx, |view, cx| view.refresh(false, cx));
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            for pane in &view.panes {
                assert!(pane.read(cx).error.is_some());
                assert!(pane.read(cx).commits.is_empty());
                assert!(pane.read(cx).query.is_none());
                assert!(pane.read(cx).selected.is_none());
                assert!(pane.read(cx).presentations.is_empty());
            }
        });
        git(&["branch", "topic"]);
        view.update(cx, |view, cx| view.refresh(false, cx));
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            for pane in &view.panes {
                assert!(pane.read(cx).error.is_none());
                assert_eq!(pane.read(cx).total, Some(0));
            }
        });
    }

    #[gpui::test]
    fn pane_filters_cancel_old_work_without_touching_the_other_side(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut comparison = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let pair = Pair {
                first_ref: "refs/heads/topic".into(),
                second_ref: "refs/heads/main".into(),
                first: "a".repeat(40),
                second: "b".repeat(40),
            };
            let view = cx.new(|cx| View::new(Repository::new(PathBuf::new()), pair, window, cx));
            comparison = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = comparison.unwrap();
        cx.update(|window, cx| {
            let [first, second] = view.read(cx).panes.clone();
            let other_token = second.read(cx).token.clone();
            let old = first.read(cx).token.clone();
            first.update(cx, |pane, cx| {
                pane.search
                    .update(cx, |input, cx| input.set_value("topic", window, cx));
                pane.dates = plico::date_filter::Bounds {
                    after: Some(10),
                    before: Some(20),
                };
                pane.apply_filter(cx);
                assert_eq!(pane.filter.after, Some(10));
                assert_eq!(pane.filter.before, Some(20));
                assert_eq!(pane.filter.text, "topic");
                assert!(old.cancelled());
                let filtered = pane.token.clone();
                pane.clear_filters(window, cx);
                assert!(filtered.cancelled());
                assert_eq!(pane.filter, git::LogFilter::default());
                assert!(pane.dates.is_empty());
                assert_eq!(pane.pair.first_ref, "refs/heads/topic");
                assert_eq!(pane.side, Side::First);
            });
            assert!(!other_token.cancelled());
            assert_eq!(second.read(cx).filter, git::LogFilter::default());
            assert_eq!(second.read(cx).side, Side::Second);
        });
    }
}
