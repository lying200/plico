//! Application window, shared state, construction and top-level composition.
mod auto_refresh;
mod binary_merge;
mod branch_management;
mod branch_merge;
mod branch_rebase;
mod branch_rename;
mod checkout;
mod chrome;
mod clone;
mod conflicts;
mod editor_tabs;
mod files;
mod files_options;
mod get_revision;
mod history_operations;
mod identity;
mod init;
mod log_branches;
mod log_count;
mod log_history;
mod log_query;
mod log_selection;
mod log_viewport;
mod merge_recovery;
mod merge_result;
mod operation;
mod preview;
mod push_remote;
mod reference_checkout;
mod reflog;
mod remote;
mod remote_management;
mod repository;
mod repository_popup;
mod revert_recovery;
mod revert_saved;
mod reword;
mod saved_changes;
mod sequencer;
mod settings;
mod tag;
mod tag_details;
mod tag_general;
mod text_merge;
mod tool_rail;
mod worktree_sync;
mod worktrees;

use crate::diff_tab::DiffTab;
use crate::{ui_actions::*, ui_graph::*, ui_style::*};
use gpui_kit::base::ElementExt as _;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::{
    Disableable as _, FocusableExt as _, Selectable as _, Sizable as _, Theme, ThemeMode, TitleBar,
    WindowExt as _,
    button::{Button, ButtonCustomVariant, ButtonVariants as _, DropdownButton},
    dialog::{DialogAction, DialogFooter},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem},
    resizable::{ResizableState, h_resizable, resizable_panel, v_resizable},
    scroll::Scrollbar,
    text::{TextView, TextViewStyle},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::i18n::{self, Language, t};
use plico::{
    branches, commit_presentation, diff_target, file_tree,
    git::{self, Area, Cancel, Change, Commit, Repository, Snapshot},
    graph::{self, Row},
    history, layout_preferences,
};
use settings::SettingsDraft;
use std::{collections::HashSet, path::PathBuf, sync::Arc};
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f2", EditCommitMessage, Some("History")),
        KeyBinding::new("f2", RenameBranch, Some("BranchPopup")),
        KeyBinding::new("ctrl-o", Open, Some("Review")),
        KeyBinding::new("ctrl-alt-s", OpenSettings, Some("Review")),
        KeyBinding::new("ctrl-alt-n", CreateBranch, Some("Review")),
        KeyBinding::new("ctrl-f5", Refresh, Some("Review")),
        KeyBinding::new("down", Older, Some("History")),
        KeyBinding::new("up", Newer, Some("History")),
        KeyBinding::new("shift-down", ExtendOlder, Some("History")),
        KeyBinding::new("shift-up", ExtendNewer, Some("History")),
        KeyBinding::new("right", GoParent, Some("History")),
        KeyBinding::new("left", GoChild, Some("History")),
        KeyBinding::new("down", NextFile, Some("Files")),
        KeyBinding::new("up", PreviousFile, Some("Files")),
        KeyBinding::new("ctrl-d", Expand, Some("Review")),
        KeyBinding::new("enter", Expand, Some("Files")),
        KeyBinding::new("ctrl-alt-p", ToggleLogDirectory, Some("Files")),
        KeyBinding::new("escape", Back, Some("Review")),
        KeyBinding::new("ctrl-f4", CloseFile, Some("Review")),
        KeyBinding::new("f4", JumpToSource, Some("Review")),
        KeyBinding::new("f7", NextChange, Some("Review")),
        KeyBinding::new("shift-f7", PreviousChange, Some("Review")),
        KeyBinding::new("ctrl-shift-c", CopyHash, Some("Review")),
        KeyBinding::new("ctrl-f", Find, Some("Review")),
    ]);
}
#[derive(Clone)]
enum FileRow {
    Group(Area, usize, bool),
    File(usize, usize),
    Folder(Area, PathBuf, PathBuf, usize, usize, bool),
}
#[derive(Clone)]
struct NavigationNotice {
    name: String,
    target: String,
    can_clear: bool,
}

pub struct Review {
    reword_gate: Option<Entity<reword::Gate>>,
    updates: auto_refresh::AutoRefresh,
    startup_report: Option<Arc<crate::startup_report::Probe>>,
    operation: plico::active_operation::Lifecycle,
    operation_task: Task<()>,
    status_message: Option<String>,
    merged_branch: Option<plico::branch_merge::Prepared>,
    pending_branch_merge: Option<plico::branch_merge::Prepared>,
    deleted_branch: Option<(PathBuf, Arc<plico::branch_delete::Prepared>)>,
    rebase_diagnostic: Option<String>,
    squash_undo: Option<Arc<plico::squash_undo::Undo>>,
    operation_state: Option<plico::operation_state::State>,
    merge_recovery_available: bool,
    revert_recovery_available: bool,
    revert_saved_available: bool,
    revert_recovery: Option<plico::revert_operation::Recovery>,
    repo: Repository,
    welcome: bool,
    ui_chinese: bool,
    recent_projects: Vec<PathBuf>,
    recent_recorded: bool,
    snapshot: Option<Snapshot>,
    rows: Arc<Vec<Row>>,
    graph_model: Option<Arc<plico::graph_print::Model>>,
    show_long_edges: bool,
    graph_print_token: Cancel,
    graph_print_task: Task<()>,
    graph_print_generation: u64,
    graph_print_loading: bool,
    graph_hover: Option<(usize, usize)>,
    commits: Arc<Vec<Commit>>,
    files: Arc<Vec<Change>>,
    file_rows: Arc<Vec<FileRow>>,
    collapsed_files: Vec<Area>,
    collapsed_directories: HashSet<(Area, PathBuf)>,
    comparison: Option<Entity<crate::compare_view::CompareView>>,
    comparison_visible: bool,
    comparison_preview: Option<WeakEntity<DiffTab>>,
    selected: Option<usize>,
    log_selection: plico::log_selection::Selection,
    aggregate_chain: Option<diff_target::Chain>,
    selection_details: Vec<(
        Vec<git::RefLabel>,
        commit_presentation::Presentation,
        Entity<crate::containing_branches::ContainingBranches>,
    )>,
    selection_branches: Option<Entity<crate::containing_branches::Batch>>,
    selected_file: Option<usize>,
    // Survives the temporary empty file model while commit details load.
    pending_preview_file: Option<Change>,
    branch: Option<String>,
    selected_tree_ref: Option<String>,
    navigation_loading: Option<String>,
    navigation_notice: Option<NavigationNotice>,
    source_task: Task<()>,
    source_token: Cancel,
    source_generation: u64,
    adjacent_task: Task<()>,
    adjacent_token: Cancel,
    adjacent_menu: Option<Entity<gpui_kit::component::menu::PopupMenu>>,
    adjacent_subscription: Option<Subscription>,
    adjacent_position: Point<Pixels>,
    branch_rows: Arc<Vec<branches::Row>>,
    collapsed_branches: HashSet<String>,
    local: bool,
    tabs: Vec<Entity<DiffTab>>,
    local_preview: Option<Entity<DiffTab>>,
    local_preview_requested: bool,
    local_visible: bool,
    local_view: Option<Entity<crate::local_view::LocalView>>,
    project_visible: bool,
    project_view: Option<Entity<crate::project_view::ProjectView>>,
    file_tabs: Vec<Entity<crate::project_view::FileTab>>,
    documents: crate::editing::Documents,
    editor_tabs: plico::editor_tabs_state::Tabs<EntityId>,
    bottom_preview: bool,
    log_files_options: layout_preferences::LogFilesOptions,
    file_preview: bool,
    saved_changes_available: bool,
    stash_open: bool,
    commit_stash_selected: bool,
    stash_view: Option<Entity<crate::stash_view::StashView>>,
    worktrees: Option<Entity<worktrees::Browser>>,
    worktrees_visible: bool,
    worktree_open_task: Task<()>,
    git_tool_split: Entity<ResizableState>,
    settings_open: bool,
    settings_session: u64,
    settings_draft: Option<SettingsDraft>,
    message: String,
    commit_metadata: [String; 2],
    containing_branches: Option<Entity<crate::containing_branches::ContainingBranches>>,
    base: Option<i18n::ComparisonTitle>,
    error: Option<String>,
    preview_error: Option<String>,
    details_cache: plico::detail_cache::Cache,
    limit: usize,
    log_query: log_query::Query,
    history_count: log_count::Count,
    history_offset: usize,
    window_start: graph::Layout,
    window_end: graph::Layout,
    focus: FocusHandle,
    history_focus: FocusHandle,
    tree_focus: FocusHandle,
    tree_scroll: UniformListScrollHandle,
    files_focus: FocusHandle,
    history_scroll: UniformListScrollHandle,
    history_horizontal: ScrollHandle,
    files_scroll: UniformListScrollHandle,
    details_scroll: ScrollHandle,
    details_split: Entity<ResizableState>,
    preview_split: Entity<ResizableState>,
    preview_fraction: f32,
    log_navigation: Entity<ResizableState>,
    compact_log_navigation: Entity<ResizableState>,
    navigation_width: Pixels,
    history_toolbar_width: Pixels,
    branches_visible: bool,
    local_navigation: Entity<ResizableState>,
    tool_windows_width: Entity<ResizableState>,
    tool_order: plico::tool_window_order::Order,
    tool_drag: Option<tool_rail::DragSession>,
    files_fraction: f32,
    branch_search: Entity<InputState>,
    filter_search: Entity<InputState>,
    text_filter: Option<git::LogFilter>,
    filter_regex: bool,
    filter_match_case: bool,
    date_filter: plico::date_filter::Bounds,
    user_menu: Entity<crate::user_menu::UserMenu>,
    paths_menu: Entity<crate::paths_menu::PathsMenu>,
    repository_menu_open: bool,
    // Logical search focus; the input retains platform text/IME handling in both modes.
    repository_search_editing: bool,
    repository_search_input: Entity<InputState>,
    repository_search: Option<plico::branch_search::Search>,
    repository_applied_query: String,
    repository_search_task: Task<()>,
    repository_selection: Option<plico::branch_search::Selection>,
    repository_actions: Vec<String>,
    repository_pointer: Option<Point<Pixels>>,
    repository_child_selection: Option<plico::branch_search::Selection>,
    repository_child_menu: Option<WeakEntity<gpui_kit::component::menu::PopupMenu>>,
    repository_recent: Vec<String>,
    repository_push_remotes: Vec<String>,
    repository_favorites: plico::branch_popup::Favorites,
    repository_favorites_path: Option<PathBuf>,
    repository_favorites_saving: bool,
    repository_favorites_error: Option<String>,
    repository_favorites_epoch: u64,
    repository_recent_token: Option<Cancel>,
    repository_recent_generation: u64,
    repository_recent_task: Task<()>,
    repository_expansion: plico::branch_popup::Expansion,
    repository_scroll: UniformListScrollHandle,
}
impl Drop for Review {
    fn drop(&mut self) {
        self.graph_print_token.cancel();
        self.adjacent_token.cancel();
        self.source_token.cancel();
        if let Some(token) = &self.repository_recent_token {
            token.cancel();
        }
    }
}
// Measure the allocated pane, not the whole window: a tool window or a dragged
// divider can change this width without changing the native window bounds.
fn observe_review_width(
    owner: WeakEntity<Review>,
    navigation: bool,
) -> impl Fn(Bounds<Pixels>, &mut Window, &mut App) {
    move |bounds, window, cx| {
        let Some(view) = owner.upgrade() else {
            return;
        };
        let previous = if navigation {
            view.read(cx).navigation_width
        } else {
            view.read(cx).history_toolbar_width
        };
        if (previous - bounds.size.width).abs() <= px(0.5) {
            return;
        }
        let width = bounds.size.width;
        let owner = owner.clone();
        window.defer(cx, move |_, cx| {
            let _ = owner.update(cx, |s, cx| {
                if navigation {
                    s.navigation_width = width;
                } else {
                    s.history_toolbar_width = width;
                }
                cx.notify();
            });
        });
    }
}
// Interactive title-bar controls must hide the native caption hitbox beneath them.
// Otherwise Windows consumes physical left clicks as window dragging.
// Popover owns the bubbling mouse-down handler and stops propagation itself.
// Stopping at its child button prevents menus from ever seeing the click.
fn titlebar_menu_button(id: &'static str) -> Button {
    Button::new(id)
}
fn titlebar_menu(menu: impl IntoElement) -> impl IntoElement {
    // Shield the native caption behind the whole popup trigger, not its child
    // button: occluding that child also hides the popup's mouse hitbox.
    div()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(menu)
}

fn titlebar_button(id: &'static str) -> Button {
    Button::new(id)
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

impl Review {
    pub fn new(
        path: PathBuf,
        limit: usize,
        startup_report: Option<Arc<crate::startup_report::Probe>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title(&if path.as_os_str().is_empty() {
            "Plico".into()
        } else {
            format!(
                "Plico – {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        });
        Theme::change(ThemeMode::Dark, Some(window), cx);
        Theme::update(cx, |t| {
            t.background = rgb(BG).into();
            t.foreground = rgb(FG).into();
            t.border = rgb(POPUP_BORDER).into();
            t.popover = rgb(SHELL).into();
            t.popover_foreground = rgb(FG).into();
            t.accent = rgb(0x2a4371).into();
            t.primary = rgb(0x3574f0).into();
            t.primary_foreground = rgb(0xffffff).into();
            t.button_primary = rgb(0x3574f0).into();
            t.button_primary_foreground = rgb(0xffffff).into();
            t.button_primary_hover = rgb(0x467ff2).into();
            // Islands split-button pressed surface must retain the brand color.
            t.button_primary_active = rgb(0x315fbd).into();
            t.ring = rgb(0x3574f0).into();
            let fonts = crate::ui_font::Preferences::load();
            t.font_family = fonts.ui_family.into();
            t.font_size = px(fonts.ui_size as f32);
            t.mono_font_family = fonts.editor_family.into();
            t.mono_font_size = px(fonts.editor_size as f32);
            // Palette provenance and capture-mapping limits: assets/themes/SOURCE.txt.
            let highlight = Arc::make_mut(&mut t.highlight_theme);
            highlight.name = "Islands Dark".into();
            highlight.style.editor_background = Some(rgb(0x191a1c).into());
            highlight.style.editor_foreground = Some(rgb(0xbcbec4).into());
            highlight.style.editor_line_number = Some(rgb(0x4b5059).into());
            highlight.style.syntax =
                serde_json::from_str(include_str!("../assets/themes/islands-syntax.json"))
                    .expect("Bundled Islands syntax palette");
        });
        let focus = cx.focus_handle();
        let history_focus = cx.focus_handle().tab_stop(true);
        let initial_focus = focus.clone();
        window.on_next_frame(move |window, cx| initial_focus.focus(window, cx));
        let mut this = Self {
            updates: Default::default(),
            startup_report,
            welcome: path.as_os_str().is_empty(),
            ui_chinese: i18n::is_chinese(),
            recent_projects: plico::recent_projects::load(),
            recent_recorded: false,
            repo: Repository::new(path.clone()),
            snapshot: None,
            rows: Arc::default(),
            graph_model: None,
            show_long_edges: layout_preferences::load_log_long_edges(&path),
            graph_print_token: Cancel::default(),
            graph_print_task: Task::ready(()),
            graph_print_generation: 0,
            graph_print_loading: false,
            graph_hover: None,
            commits: Arc::default(),
            files: Arc::default(),
            file_rows: Arc::default(),
            collapsed_files: Vec::new(),
            collapsed_directories: HashSet::new(),
            comparison: None,
            comparison_visible: false,
            comparison_preview: None,
            selected: None,
            log_selection: Default::default(),
            aggregate_chain: None,
            selection_details: Vec::new(),
            selection_branches: None,
            selected_file: None,
            pending_preview_file: None,
            branch: None,
            selected_tree_ref: None,
            navigation_loading: None,
            navigation_notice: None,
            source_task: Task::ready(()),
            source_token: Cancel::default(),
            source_generation: 0,
            adjacent_task: Task::ready(()),
            adjacent_token: Cancel::default(),
            adjacent_menu: None,
            adjacent_subscription: None,
            adjacent_position: point(px(0.), px(0.)),
            branch_rows: Arc::default(),
            collapsed_branches: HashSet::new(),
            local: false,
            tabs: Vec::new(),
            local_preview: None,
            local_preview_requested: false,
            local_visible: false,
            local_view: None,
            project_visible: false,
            project_view: None,
            file_tabs: Vec::new(),
            documents: Self::new_documents(cx),
            editor_tabs: Default::default(),
            saved_changes_available: false,
            stash_open: false,
            commit_stash_selected: false,
            stash_view: None,
            worktrees: None,
            worktrees_visible: false,
            worktree_open_task: Task::ready(()),
            git_tool_split: cx.new(|_| ResizableState::default()),
            bottom_preview: layout_preferences::load_bottom_preview(),
            log_files_options: layout_preferences::load_log_files_options(),
            file_preview: layout_preferences::load_file_preview(),
            settings_open: false,
            settings_session: 0,
            settings_draft: None,
            message: String::new(),
            commit_metadata: Default::default(),
            containing_branches: None,
            reword_gate: None,
            base: None,
            error: None,
            preview_error: None,
            details_cache: Default::default(),
            operation: Default::default(),
            operation_task: Task::ready(()),
            status_message: None,
            merged_branch: None,
            pending_branch_merge: None,
            deleted_branch: None,
            rebase_diagnostic: None,
            squash_undo: None,
            operation_state: None,
            revert_recovery: None,
            merge_recovery_available: false,
            revert_recovery_available: false,
            revert_saved_available: false,
            limit,
            log_query: Default::default(),
            history_count: Default::default(),
            history_offset: 0,
            window_start: graph::Layout::default(),
            window_end: graph::Layout::default(),
            focus,
            history_focus,
            tree_focus: cx.focus_handle().tab_stop(true),
            tree_scroll: UniformListScrollHandle::new(),
            files_focus: cx.focus_handle().tab_stop(true),
            history_scroll: UniformListScrollHandle::new(),
            history_horizontal: ScrollHandle::new(),
            files_scroll: UniformListScrollHandle::new(),
            details_scroll: ScrollHandle::new(),
            details_split: cx.new(|_| ResizableState::default()),
            preview_split: cx.new(|_| ResizableState::default()),
            preview_fraction: layout_preferences::load_preview_fraction(),
            log_navigation: cx.new(|_| ResizableState::default()),
            compact_log_navigation: cx.new(|_| ResizableState::default()),
            navigation_width: px(1000.),
            history_toolbar_width: px(1000.),
            branches_visible: false,
            local_navigation: cx.new(|_| ResizableState::default()),
            tool_windows_width: cx.new(|_| ResizableState::default()),
            tool_order: layout_preferences::load_tool_window_order(),
            tool_drag: None,
            files_fraction: layout_preferences::load_files_fraction(),
            text_filter: None,
            filter_regex: false,
            filter_match_case: false,
            date_filter: Default::default(),
            repository_menu_open: false,
            repository_search_editing: false,
            repository_search_input: cx.new(|cx| {
                InputState::new(window, cx).placeholder(t("Search for branches and actions"))
            }),
            repository_search: None,
            repository_applied_query: String::new(),
            repository_search_task: Task::ready(()),
            repository_selection: None,
            repository_actions: vec!["commit".into(), "new-branch".into(), "revision".into()],
            repository_pointer: None,
            repository_child_selection: None,
            repository_child_menu: None,
            repository_recent: Vec::new(),
            repository_push_remotes: Vec::new(),
            repository_favorites: Default::default(),
            repository_favorites_path: None,
            repository_favorites_saving: false,
            repository_favorites_error: None,
            repository_favorites_epoch: 0,
            repository_recent_token: None,
            repository_recent_generation: 0,
            repository_recent_task: Task::ready(()),
            repository_expansion: Default::default(),
            repository_scroll: UniformListScrollHandle::new(),
            paths_menu: cx.new(|cx| crate::paths_menu::PathsMenu::new(path.clone(), cx)),
            user_menu: cx
                .new(|cx| crate::user_menu::UserMenu::new(Repository::new(path.clone()), cx)),
            filter_search: cx.new(|cx| InputState::new(window, cx).placeholder(t("Text or hash"))),
            branch_search: cx
                .new(|cx| InputState::new(window, cx).placeholder(t("Branches and Tags"))),
        };
        for handle in [&this.tree_focus, &this.history_focus, &this.files_focus] {
            cx.on_focus(handle, window, |_, _, cx| cx.notify()).detach();
            cx.on_blur(handle, window, |_, _, cx| cx.notify()).detach();
        }
        cx.bind_keys([KeyBinding::new(
            "ctrl-s",
            crate::project_view::SaveFile,
            Some("Review"),
        )]);
        cx.observe_window_activation(window, |s, window, cx| {
            if !window.is_window_active() {
                drop(s.documents.save_all(cx));
            }
            cx.notify();
        })
        .detach();
        let owner = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            owner
                .update(cx, |s, cx| {
                    if !s.documents.pending(cx) && !s.operation.saving_documents() {
                        return true;
                    }
                    s.with_saved_documents(window, cx, |_, window, _| window.remove_window());
                    false
                })
                .unwrap_or(true)
        });
        cx.subscribe(
            &this.paths_menu,
            |s, _, _: &crate::paths_menu::Changed, cx| s.apply_text_filter(cx),
        )
        .detach();
        cx.subscribe(
            &this.user_menu,
            |s, _, event: &crate::user_menu::Changed, cx| match event {
                crate::user_menu::Changed::Applied => s.apply_text_filter(cx),
                crate::user_menu::Changed::Failed(error) => {
                    s.error = Some(format!("Could not refresh User filter: {error}"));
                    cx.notify();
                }
            },
        )
        .detach();
        cx.subscribe(&this.filter_search, |s, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                s.apply_text_filter(cx);
            }
        })
        .detach();

        cx.subscribe(&this.branch_search, |s, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                s.rebuild_branches(cx);
            }
        })
        .detach();
        cx.subscribe(
            &this.repository_search_input,
            |s, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) && s.repository_menu_open {
                    s.dismiss_repository_child(cx);
                    s.repository_search_task = cx.spawn(async move |owner, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(100))
                            .await;
                        let _ = owner.update(cx, |s, cx| {
                            if s.repository_menu_open {
                                s.apply_repository_search(cx);
                            }
                        });
                    });
                }
            },
        )
        .detach();
        if !this.welcome {
            this.refresh(false, cx);
            this.start_auto_refresh(window, cx);
        }
        this
    }
}
impl Render for Review {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_language(_window, cx);
        let show_local_preview = self.bottom_preview
            && self.active_diff_index().is_none()
            && self.active_file_index().is_none();
        if !show_local_preview {
            if let Some(preview) = self.local_preview.take() {
                preview.update(cx, |p, cx| p.deactivate(cx));
            }
        } else if self.stash_open {
            if let Some(view) = &self.stash_view {
                if !view.read(cx).preview_loading() {
                    match view.update(cx, |view, _| view.preview_target()) {
                        Some(Ok(crate::stash_view::OpenDiff(target, chain, origin))) => {
                            let unchanged = self
                                .local_preview
                                .as_ref()
                                .is_some_and(|p| p.read(cx).target.identity() == target.identity());
                            if !unchanged {
                                let preview = cx.new(|_| {
                                    let mut p = DiffTab::preview(target, chain);
                                    p.documents = self.documents.clone();
                                    p.stash_origin = Some(origin);
                                    p
                                });
                                self.subscribe_diff_navigation(&preview, cx);
                                let callback = self.diff_source_callback(cx);
                                preview.update(cx, |p, cx| {
                                    p.source_callback = Some(callback);
                                    p.activate_with_focus(false, _window, cx);
                                });
                                self.local_preview = Some(preview);
                            } else if let Some(preview) = &self.local_preview {
                                preview.update(cx, |p, _| p.stash_origin = Some(origin));
                            }
                        }
                        _ => self.local_preview = None,
                    }
                }
            }
        } else if !self.uses_local_changes_preview() {
            let pair = if self.log_query.busy() {
                None
            } else {
                self.log_preview_target(cx)
            };
            if let Some((target, chain)) = pair {
                if !self
                    .local_preview
                    .as_ref()
                    .is_some_and(|p| p.read(cx).target.identity() == target.identity())
                {
                    let preview = cx.new(|_| {
                        let mut p = DiffTab::preview(target, chain);
                        p.documents = self.documents.clone();
                        p
                    });
                    self.subscribe_diff_navigation(&preview, cx);
                    let callback = self.diff_source_callback(cx);
                    preview.update(cx, |p, cx| {
                        p.source_callback = Some(callback);
                        p.activate_with_focus(false, _window, cx);
                    });
                    self.local_preview = Some(preview);
                }
            } else {
                self.local_preview = None;
            }
        } else if let Some(view) = &self.local_view {
            let view = view.read(cx);
            // Keep the viewer while a listing refresh is in flight. Ordinary
            // sidebar notifications (draft edits/inclusion) do not invalidate it.
            if !view.preview_loading(cx) {
                let revision = view.preview_revision(cx);
                match view.preview_change(cx) {
                    Some(change) => {
                        let unchanged = self.local_preview.as_ref().is_some_and(|p| {
                            let p = p.read(cx);
                            p.snapshot_revision == revision
                                && p.target.change().area == change.area
                                && p.target.change().path == change.path
                                && p.target.change().old_path == change.old_path
                                && p.target.change().status == change.status
                        });
                        if let Some((target, chain)) =
                            (!unchanged).then(|| view.preview_target(cx)).flatten()
                        {
                            let callback = self.diff_source_callback(cx);
                            let reusable = self
                                .local_preview
                                .as_ref()
                                .filter(|p| p.read(cx).target.identity() == target.identity())
                                .cloned();
                            let preview = if let Some(preview) = reusable {
                                preview.update(cx, |p, cx| {
                                    p.replace_target(target, chain, cx);
                                    p.snapshot_revision = revision;
                                });
                                preview
                            } else {
                                let scope = view.scope.clone();
                                let preview = cx.new(|_| {
                                    let mut p = DiffTab::preview(target, chain);
                                    p.documents = self.documents.clone();
                                    p.commit_scope = Some(scope);
                                    p.snapshot_revision = revision;
                                    p
                                });
                                self.subscribe_diff_navigation(&preview, cx);
                                preview
                            };
                            preview.update(cx, |p, cx| {
                                p.source_callback = Some(callback);
                                p.activate_with_focus(false, _window, cx);
                            });
                            self.local_preview = Some(preview);
                        }
                    }
                    None => self.local_preview = None,
                }
            }
        }
        if let Some(view) = &self.stash_view {
            let busy = self.operation.busy() || self.settings_open;
            view.update(cx, |v, cx| v.set_write_busy(busy, cx));
        }
        let minimum_navigation = if self.local {
            340.
        } else if self.branches_visible {
            640.
        } else {
            500.
        };
        let navigation_scale = if self.navigation_width > px(0.) {
            (self.navigation_width.as_f32() / minimum_navigation).min(1.)
        } else {
            1.
        };
        if self.welcome {
            return v_flex()
                .size_full()
                .bg(rgb(PANEL))
                .when_some(self.startup_frame(), |v, (probe, frame)| v.on_prepaint(move |_, _, _| probe.record(frame)))
                .text_color(rgb(FG)).text_size(cx.theme().font_size).font_family(cx.theme().font_family.clone())
                .track_focus(&self.focus)
                .key_context("Review")
                .on_action(cx.listener(|s, _: &Open, w, cx| s.open(w, cx)))
                .on_action(cx.listener(|s, _: &OpenSettings, w, cx| s.open_settings(w, cx)))
            .on_action(cx.listener(|s, _: &CreateBranch, w, cx| s.new_branch_dialog("HEAD".into(),w,cx)))
                .on_action(cx.listener(|s, _: &Back, _, cx| { s.close_settings(cx); cx.notify(); }))
                .child(TitleBar::new().bg(rgb(PANEL)).border_color(rgb(PANEL)).child(
                    h_flex().w_full().px_2().gap_2().items_center().child(gpui_kit::img("branding/plico.svg").size(px(20.))).child("Plico").child(div().flex_1())
                        .child(titlebar_button("welcome-settings").ghost().small().icon(jb_icon("settings_dark.svg").size(px(14.))).icon_size(px(14.))
                            .tooltip(format!("{} (Ctrl+Alt+S)", t("Settings"))).accessibility_label(t("Settings"))
                            .on_click(cx.listener(|s, _, w, cx| { s.open_settings(w, cx); })))))
                .child(h_flex().flex_1().min_h_0().items_stretch()
                    .child(v_flex().w(px(220.)).h_full().p_4().gap_4().border_r_1().border_color(rgb(BORDER))
                        .child(h_flex().gap_2().items_center().child(gpui_kit::img("branding/plico.svg").size(px(32.))).child(div().text_size(px(16.)).child("Plico")))
                        .child(div().text_color(rgb(MUTED)).child("0.1.0"))
                        .child(div().px_3().py_2().rounded(px(6.)).bg(rgb(SELECT)).child(t("Projects"))))
                    .child(v_flex().flex_1().h_full().min_w_0().p_4().gap_3()
                        .child(h_flex().h(px(40.)).gap_2().flex_shrink_0().border_b_1().border_color(rgb(BORDER)).child(div().flex_1().child(t("Recent Projects")))
                            .child(Button::new("init-repository").label(t("Create Git Repository…")).disabled(self.operation.busy()).on_click(cx.listener(|s,_,w,cx|s.open_init(w,cx))))
                            .child(Button::new("clone-repository").label(t("Clone Repository…")).disabled(self.operation.busy()).on_click(cx.listener(|s,_,w,cx|s.open_clone(w,cx))))
                            .child(Button::new("open-repository").label(t("Open Repository…")).on_click(cx.listener(|s, _, w, cx| s.open(w, cx)))))
                        .when(self.recent_projects.is_empty(), |v| v.child(div().text_color(rgb(MUTED)).child(t("Browse history. Review changes."))))
                        .when_some(self.error.clone(), |v, error| v.child(div().text_color(rgb(0xf2a6a6)).child(error)))
                        .child(v_flex().id("recent-projects-list").flex_1().min_h_0().overflow_y_scroll().children(self.recent_projects.iter().enumerate().map(|(i, path)| {
                            let open = path.clone(); let remove = path.clone();
                            h_flex().id(("recent-project", i)).w_full().min_w_0().p_3().gap_3().rounded(px(6.)).hover(|s| s.bg(rgb(0x2b2d30)))
                                .child(project_avatar(path, 24.))
                                .child(Button::new(("open-recent", i)).ghost().flex_1().min_w_0().h(px(50.))
                                    .child(v_flex().w_full().items_start().min_w_0()
                                        .child(div().truncate().child(path.file_name().unwrap_or_default().to_string_lossy().into_owned()))
                                        .child(div().text_size(px(12.)).text_color(rgb(MUTED)).truncate().child(path.to_string_lossy().trim_start_matches("\\\\?\\").to_owned())))
                                    .on_click(cx.listener(move |s, _, w, cx| s.choose_project_window(open.clone(), w, cx))))
                                .child(Button::new(("remove-recent", i)).ghost().small().icon(jb_icon("closeSmall.svg")).tooltip(t("Remove from Recent Projects"))
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        match plico::recent_projects::remove(&remove) { Ok(paths) => s.recent_projects = paths, Err(e) => s.error = Some(e.to_string()) }
                                        cx.notify();
                                    })))
                        })))))
                .into_any_element();
        }

        v_flex()
            .size_full()
            .text_size(cx.theme().font_size)
            .text_color(rgb(FG))
            .font_family(cx.theme().font_family.clone())
            .bg(rgb(SHELL))
            .track_focus(&self.focus)
            .key_context("Review")
            .capture_key_down(cx.listener(|s, event: &KeyDownEvent, w, cx| {
                if event.keystroke.key == "escape" && s.cancel_tool_drag(w, cx) { cx.stop_propagation(); }
            }))
            .on_action(cx.listener(|s, _: &Open, w, cx| s.open(w, cx)))
            .on_action(cx.listener(|s, _: &OpenSettings, w, cx| s.open_settings(w, cx)))
            .on_action(cx.listener(|s, _: &CreateBranch, w, cx| s.new_branch_dialog("HEAD".into(),w,cx)))
            .on_action(cx.listener(|s, _: &JumpToSource, w, cx| s.jump_to_source(w, cx)))
            .on_action(cx.listener(|s, _: &Refresh, w, cx| s.refresh_current(w, cx)))
            .on_action(cx.listener(|s, _: &Older, _, cx| s.move_commit(1, cx)))
            .on_action(cx.listener(|s, _: &Newer, _, cx| s.move_commit(-1, cx)))
            .on_action(cx.listener(|s, _: &ExtendOlder, _, cx| s.extend_commit(1, cx)))
            .on_action(cx.listener(|s, _: &ExtendNewer, _, cx| s.extend_commit(-1, cx)))
            .on_action(cx.listener(|s, _: &GoParent, w, cx| { if s.history_focus.is_focused(w) { s.navigate_adjacent(true, true, w, cx); } }))
            .on_action(cx.listener(|s, _: &GoChild, w, cx| { if s.history_focus.is_focused(w) { s.navigate_adjacent(false, true, w, cx); } }))
            .on_action(cx.listener(|s, _: &NextFile, _, cx| s.move_file(1, cx)))
            .on_action(cx.listener(|s, _: &PreviousFile, _, cx| s.move_file(-1, cx)))
            .on_action(cx.listener(|s, _: &Expand, w, cx| {
                s.open_diff_tab(w, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &CloseFile, w, cx| {
                if w.has_active_dialog(cx) || w.has_active_sheet(cx) { return; }
                if let Some(index) = s.active_file_index() { s.close_file_tab(index, w, cx); }
                else if let Some(index) = s.active_diff_index() { s.close_diff_tab_from_header(index, w, cx); }
            }))
            .on_action(cx.listener(|s, _: &Back, w, cx| {
                s.close_settings(cx);
                if let Some(tab) = s.active_file_index().and_then(|index| s.file_tabs.get(index)) {
                    tab.update(cx, |tab, cx| tab.escape(w, cx));
                    return;
                }
                let return_to_comparison = s.active_diff_index().is_some() && s.active_diff_index() == s.comparison_preview_index(cx);
                if let Some(index) = s.active_diff_index() { s.close_diff_tab(index, cx); }
                if return_to_comparison {
                    s.cancel_project_preview(cx);
                    s.comparison_visible = true; s.local_visible = false; s.project_visible = false; s.stash_open = false;
                    if let Some(view) = &s.comparison { view.update(cx, |v, cx| v.focus_files(w, cx)); }
                    cx.notify(); return;
                }
                s.focus_after_diff(w, cx);
                cx.notify();
            }))
            .on_action(
                cx.listener(|s, _: &NextChange, w, cx| {
                    if let Some(tab) = s.active_diff_index().and_then(|i| s.tabs.get(i)) { tab.update(cx, |t, cx| t.navigate(true, w, cx)); } else { if let Some(p) = &s.local_preview { p.update(cx, |p, cx| p.navigate(true, w, cx)); } }
                }),
            )
            .on_action(cx.listener(|s, _: &PreviousChange, w, cx| {
                if let Some(tab) = s.active_diff_index().and_then(|i| s.tabs.get(i)) { tab.update(cx, |t, cx| t.navigate(false, w, cx)); } else { if let Some(p) = &s.local_preview { p.update(cx, |p, cx| p.navigate(false, w, cx)); } }
            }))
            .on_action(cx.listener(|s, _: &crate::project_view::SaveFile, _, cx| { drop(s.documents.save_all(cx)); }))
            .on_action(cx.listener(|s, _: &Find, w, cx| {
                if s.repository_menu_open {
                    s.repository_search_editing = true;
                    s.dismiss_repository_child(cx);
                    s.repository_search_input.read(cx).focus_handle(cx).focus(w, cx);
                    cx.stop_propagation(); cx.notify(); return;
                }
                s.leave_diff_tab(cx);
                if s.local {
                    s.local = false;
                    s.refresh(false, cx);
                }
                s.filter_search.read(cx).focus_handle(cx).focus(w, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|s, _: &EditCommitMessage, w, cx| s.edit_commit_message(w, cx)))
            .on_action(cx.listener(|s, _: &CopyHash, _, cx| {
                let ids = s.commit_ids();
                let mut selected = s.log_selection.ordered(&ids);
                selected.reverse();
                if !selected.is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(selected.join(" "))); }
            }))
            .child(TitleBar::new().pl_0().h(px(40.)).bg(rgb(SHELL)).border_color(rgb(SHELL)).child(self.toolbar(cx)))


            .when_some(self.navigation_loading.clone(), |layout, name| layout.child(
                div().px_3().py_2().child(i18n::message(i18n::Message::Locating(&name)))))
            .when_some(self.navigation_notice.clone(), |layout, notice| layout.child(
                h_flex().px_3().py_2().gap_3()
                    .child(if notice.can_clear { i18n::message(i18n::Message::HiddenInLog(&notice.name)) }
                           else { i18n::message(i18n::Message::NotInHistory(&notice.name)) })
                    .when(notice.can_clear, |row| row.child(Button::new("navigate-clear-filters").link().small().label(t("Clear filters and show"))
                        .on_click(cx.listener(move |s, _, window, cx| s.navigate_commit(notice.name.clone(), notice.target.clone(), true, window, cx)))))
                    .child(Button::new("dismiss-navigation").ghost().small().label(t("Dismiss"))
                        .on_click(cx.listener(|s, _, _, cx| { s.navigation_notice = None; cx.notify(); })))
            ))
            .child(
                h_flex().flex_1().min_h_0().items_stretch().px(px(8.)).gap(px(6.)).bg(rgb(SHELL))
                    .child(self.tool_rail(cx))
                    .child(div().flex_1().min_w_0().h_full().child(v_resizable("git-tool-window-height").with_state(&self.git_tool_split)
                    .child(resizable_panel().size_range(px(160.)..px(4000.)).child(h_resizable("tool-windows-width").with_state(&self.tool_windows_width)
                    .child(resizable_panel().visible(self.local_visible || self.project_visible || self.stash_open || self.comparison_visible).flex_none().size(px(300.)).size_range(px(240.)..px(700.))
                        .child(div().size_full().pr(px(6.)).child(div().relative().size_full().overflow_hidden().child(self.left_tool_windows(cx)).children(island_corners(true)))))
                    .child(resizable_panel().child(v_flex().flex_1().min_w_0().h_full().child(self.git_toolbar(cx))
                    .child(div().flex_1().min_h_0().child(if let Some(tab) = self.active_file_index().and_then(|i| self.file_tabs.get(i)) {
                        div().relative().size_full().rounded_b(px(10.)).overflow_hidden().bg(rgb(BG)).child(tab.clone()).children(island_corners(false)).into_any_element()
                    } else if let Some(tab) = self.active_diff_index().and_then(|i| self.tabs.get(i)) {
                        div().relative().size_full().rounded_b(px(10.)).overflow_hidden().bg(rgb(BG)).child(tab.clone()).children(island_corners(false)).into_any_element()
                    } else {
                        v_resizable("vertical").with_state(&self.preview_split)
                            .on_resize(cx.listener(|s, state: &Entity<ResizableState>, _, cx| {
                                if let [main, preview] = state.read(cx).sizes().as_slice() {
                                    let total = main.as_f32() + preview.as_f32();
                                    if total > 0. {
                                        s.preview_fraction = (main.as_f32() / total).clamp(0.1, 0.9);
                                        if let Err(error) = layout_preferences::save_preview_fraction(s.preview_fraction) {
                                            eprintln!("Cannot save preview split: {error}");
                                        }
                                    }
                                }
                            }))
                            .child(
                                resizable_panel()
                                    .flex_basis(relative(self.preview_fraction))
                                    .size_range(px(160.)..px(1800.))
                                    .child(
                                        v_flex()
                                            .relative().size_full()
                                            .when(!self.bottom_preview, |pane| pane.rounded_b(px(10.)))
                                            .overflow_hidden()
                                            .bg(rgb(BG))
                                            .child(
                                                h_flex().flex_1().min_h_0()
                                                    .when(!self.local, |row| row.child(self.branches_rail(cx)))
                                                    .child(div().flex_1().min_w_0().h_full()
                                                        .on_prepaint(observe_review_width(cx.entity().downgrade(), true)).child(
                                                    // ResizableState identifies panels by index.
                                                    // Log has three panels; Local Changes has two.
                                                    // Keep separate retained state so removing the
                                                    // history panel cannot reinterpret its size as
                                                    // the files panel's size.
                                                    h_resizable(if self.local {
                                                        "local-navigation"
                                                    } else if !self.branches_visible {
                                                        "compact-log-navigation"
                                                    } else {
                                                        "log-navigation"
                                                    })
                                                    .with_state(if self.local {
                                                        &self.local_navigation
                                                    } else if !self.branches_visible {
                                                        &self.compact_log_navigation
                                                    } else {
                                                        &self.log_navigation
                                                    })
                                                    .when(self.local || self.branches_visible, |layout| layout.child(
                                                        resizable_panel()
                                                            .size(if navigation_scale < 1. { px(140. * navigation_scale) } else { px(200.) })
                                                            .size_range(px(140. * navigation_scale)..px(500.))
                                                            .child(self.branches(_window, cx)),
                                                    ))
                                                    .when(!self.local, |d| {
                                                        d.child(
                                                            resizable_panel()
                                                                .size_range(px(300. * navigation_scale)..px(2400.))
                                                                .child(self.history(_window, cx)),
                                                        )
                                                    })
                                                    .child(
                                                        resizable_panel()
                                                            .when(!self.local, |panel| {
                                                                panel.size(if navigation_scale < 1. { px(200. * navigation_scale) } else { px(320.) })
                                                            })
                                                            .size_range(px(200. * navigation_scale)..px(1400.))
                                                            .child(self.files(_window, cx)),
                                                    ),
                                                )),
                                            )
                                            .when(!self.bottom_preview, |pane| pane.children(island_corners(false))),
                                    ),
                            )
                            .when(self.bottom_preview && !(self.commits.is_empty() && self.error.is_some() && !self.local_visible && !self.stash_open), |layout| layout.child(
                                resizable_panel().flex_basis(relative(1. - self.preview_fraction)).size_range(px(60.)..px(4000.)).child(
                                    div().size_full().child(
                                        div()
                                            .relative().size_full()
                                            .rounded_b(px(10.))
                                            .overflow_hidden()
                                            .child(if self.local_preview.is_some() {
                                                div().size_full().children(self.local_preview.clone())
                                                    .when(self.local_preview.is_none(), |v| v.child(div().p_3().child(t("Select a file to view its diff")))).into_any_element()
                                            } else { self.diff_panel(cx).into_any_element() })
                                            .children(island_corners(false)),
                                    ),
                                ),
                            ))
                            .into_any_element()
                    }))))))
                    .when(self.worktrees_visible, |layout| layout.child(resizable_panel().flex_none().size(px(280.)).size_range(px(100.)..px(1400.)).child(
                        div().size_full().pt(px(6.)).children(self.worktrees.clone())
                    ))))),
            )
            .child(
                h_flex()
                    .h(px(20.))
                    .px_3()
                    .bg(rgb(SHELL))
                    .text_size(px(12.)).line_height(px(16.))
                    .text_color(rgb(MUTED))
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .when(self.snapshot.as_ref().is_some_and(|s| s.shallow), |v| v.child(
                        div().min_w_0().truncate().child(t("Shallow repository · totals cover available history"))))
                    .child(div().flex_1())
            .when_some(self.error.clone(), |v, error| v.child(h_flex().gap_2().min_w_0()
                .child(div().text_color(rgb(0xf2a6a6)).child(t(if plico::git_error::is_ownership_error(&error) { "Repository access blocked" } else { "Operation failed" })))
                .child(Button::new("copy-operation-error").link().xsmall().label(t("Details"))
                    .on_click(cx.listener(|s, _, w, cx| s.show_error_details(w, cx))))
                .child(Button::new("dismiss-operation-error").ghost().xsmall().label(t("Dismiss"))
                    .on_click(cx.listener(|s, _, _, cx| { s.error = None; cx.notify(); })))))
            .when(self.operation.busy() || self.log_query.busy() || self.log_query.history_loading(), |v| v.child(div().size(px(16.)).flex_shrink_0().child(gpui_kit::component::spinner::Spinner::new().small().color(rgb(MUTED).into()))))
            .when_some(self.status_message.clone(), |v, text| v.child(div().max_w_full().min_w_0().truncate().child(text)))
            .when_some(self.merged_branch.as_ref(), |v, merged| v.child(
                Button::new("delete-merged-branch").link().xsmall()
                    .label(format!("{} ‘{}’", t("Delete branch"), merged.reference.strip_prefix("refs/heads/").unwrap_or(&merged.reference)))
                    .disabled(self.operation.busy()).on_click(cx.listener(|s,_,w,cx|s.delete_merged_branch(w,cx))))
                .child(Button::new("dismiss-merge-deletion").ghost().xsmall().label(t("Dismiss"))
                    .on_click(cx.listener(|s,_,_,cx| {s.merged_branch=None;cx.notify();}))))
            .when(self.deleted_branch.as_ref().is_some_and(|(path,_)|path==&self.repo.path), |v| v.child(
                Button::new("restore-deleted-branch").link().xsmall().label(t("Restore deleted branch"))
                    .disabled(self.operation.busy()).on_click(cx.listener(|s,_,w,cx|s.restore_deleted_branch(w,cx)))))
            .when(self.squash_undo.is_some(), |v| v.child(Button::new("squash-undo").ghost().xsmall().label(t("Undo Squash"))
                .disabled(self.operation.busy()).on_click(cx.listener(|s, _, w, cx| s.undo_squash(w, cx)))))
            .when(self.rebase_diagnostic.is_some(), |v| v.child(Button::new("rebase-details").ghost().xsmall().label(t("Rebase details"))
                .on_click(cx.listener(|s, _, w, cx| {
                    let Some(text) = s.rebase_diagnostic.clone() else { return; };
                    w.open_dialog(cx, move |dialog, _, cx| dialog.title(t("Rebase details")).w(px(640.)).centered(true)
                        .child(div().id("rebase-diagnostic").max_h(px(300.)).overflow_y_scroll().text_size(cx.theme().font_size).child(text.clone())));
                }))))
            .when(self.saved_changes_available, |v| v.child(Button::new("view-saved-changes").ghost().xsmall().label(t("Show saved changes"))
                .on_click(cx.listener(|s, _, w, cx| s.open_stashes(w, cx)))))
            .when(self.operation.can_cancel(), |v| v.child(Button::new("cancel-fetch").ghost().xsmall().label(t("Cancel"))
                .on_click(cx.listener(|s, _, _, _| { s.operation.request_cancel(); })))),
            )
            .when_some(self.adjacent_menu.clone(), |v, menu| v.child(deferred(anchored()
                .position(self.adjacent_position).snap_to_window_with_margin(px(8.)).child(menu)).with_priority(100)))
            .into_any_element()
    }
}
