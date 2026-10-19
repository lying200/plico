use gpui_kit::base::ElementExt as _;
use plico::i18n::{self, t, Language};
use crate::diff_tab::DiffTab;
use plico::{
    branches, commit_presentation, diff_target, file_tree,
    git::{self, Area, Cancel, Change, Commit, Repository, Snapshot},
    graph::{self, Row},
    history, layout_preferences,
};
use gpui_kit::component::{
    Icon, Disableable as _, FocusableExt as _, Selectable as _, Sizable as _, Theme, ThemeMode, TitleBar, WindowExt as _,
    dialog::{DialogAction, DialogFooter},
    button::{Button, DropdownButton, ButtonCustomVariant, ButtonVariants as _},
    diff::{Diff, DiffFile, DiffMode, DiffState, DiffChangeIndicator, DiffFileStatus},
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
use std::{collections::HashSet, path::PathBuf, sync::Arc};
actions!(
    review,
    [
        Refresh,
        Open,
        OpenSettings,
        Older,
        Newer,
        ExtendOlder,
        ExtendNewer,
        GoParent,
        GoChild,
        NextFile,
        PreviousFile,
        Expand,
        Back,
        CloseFile,
        NextChange,
        PreviousChange,
        JumpToSource,
        CopyHash,
        Find
    ]
);
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-o", Open, Some("Review")),
        KeyBinding::new("ctrl-alt-s", OpenSettings, Some("Review")),
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
        KeyBinding::new("escape", Back, Some("Review")),
        KeyBinding::new("ctrl-f4", CloseFile, Some("Review")),
        KeyBinding::new("f4", JumpToSource, Some("Review")),
        KeyBinding::new("f7", NextChange, Some("Review")),
        KeyBinding::new("shift-f7", PreviousChange, Some("Review")),
        KeyBinding::new("ctrl-shift-c", CopyHash, Some("Review")),
        KeyBinding::new("ctrl-f", Find, Some("Review")),
    ]);
}
// Cover opaque child paint at island corners. GPUI overflow clips to a rectangle.
fn island_corners(top: bool) -> Vec<AnyElement> {
    [("ui/corner-bl.svg", false, false), ("ui/corner-br.svg", false, true),
     ("ui/corner-tl.svg", true, false), ("ui/corner-tr.svg", true, true)]
        .into_iter().filter(|(_, upper, _)| top || !upper)
        .map(|(path, upper, right)| svg().path(path).absolute().size(px(10.)).text_color(rgb(SHELL))
            .when(upper, |corner| corner.top_0()).when(!upper, |corner| corner.bottom_0())
            .when(right, |corner| corner.right_0()).when(!right, |corner| corner.left_0())
            .into_any_element()).collect()
}
const BG: u32 = 0x191a1c;
const SHELL: u32 = 0x26282c;
const PANEL: u32 = 0x191a1c;
const BORDER: u32 = 0x26282c;
// Islands popup-border: layer-1-border -> gray-40. Keep panel seams separate.
const POPUP_BORDER: u32 = 0x33353b;
const FG: u32 = 0xd1d3d9;
const MUTED: u32 = 0x9fa2a8;
const SELECT: u32 = 0x2e4d89;
fn project_path_text(path: &std::path::Path) -> String {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") { format!(r"\\{}", unc) }
    else { text.trim_start_matches(r"\\?\").to_owned() }
}
fn project_identity(path: &std::path::Path) -> String {
    plico::recent_projects::identity(path)
}
fn project_display_path(path: &std::path::Path) -> String {
    let text = project_path_text(path);
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = project_path_text(std::path::Path::new(&home));
        let home = home.trim_end_matches(['/', '\\']);
        if text.get(..home.len()).is_some_and(|prefix| prefix.eq_ignore_ascii_case(home)) {
            let rest = &text[home.len()..];
            if rest.is_empty() || rest.starts_with(['/', '\\']) { return format!("~{}", rest); }
        }
    }
    text
}

// Rebased AvatarUtils.initials / RecentProjectIconHelper: title and recent
// entries use the same name badge. Colors stay in app memory; never write .idea.
fn project_avatar(path: &std::path::Path, size: f32) -> impl IntoElement {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let filtered: String = name.chars().filter(|c| c.len_utf16() == 1).collect();
    let camel: String = name.chars().skip_while(|c| !c.is_alphabetic())
        .take_while(|c| c.is_alphanumeric()).enumerate()
        .filter(|(i,c)| *i == 0 || c.is_uppercase()).map(|(_,c)| c).take(2).collect();
    let initials = if camel.chars().count() == 2 { camel.to_uppercase() } else {
        [' ', ',', '-', '_', '.', '`', '\'', '"'].into_iter().find_map(|separator| {
            let words: Vec<String> = filtered.split(separator)
                .map(|word| word.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
                .filter(|word| !word.is_empty()).collect();
            (words.len() >= 2).then(|| format!("{}{}", words[0].chars().next().unwrap(), words.last().unwrap().chars().next().unwrap()).to_uppercase())
        }).unwrap_or_else(|| camel.to_uppercase())
    };
    let palette = [(0xce443c,0xe77e41),(0xe27237,0xe8a83e),(0x2dbcad,0x35d6a4),
        (0x3895e1,0x51c5ea),(0x7b75e8,0x7d99eb),(0x7854ad,0x897ae6),
        (0x8f4593,0xb572e3),(0xc840b9,0xe074ae),(0xd75370,0xe96fa3)];
    let seed = project_identity(path).encode_utf16().fold(0u32, |hash,c| hash.wrapping_mul(31).wrapping_add(c as u32));
    let (start,end) = palette[seed as usize % palette.len()];
    // AvatarUtils.getNewUiFont uses its own Mono demi-bold face, independent
    // of the system UI font. Reserve a 10% inset and an explicit line box so
    // inherited UI line height cannot push the initials against the badge.
    div().size(px(size)).flex_shrink_0().rounded(px(size * 0.2))
        .p(px(size * 0.1)).overflow_hidden()
        .flex().items_center().justify_center()
        .font_family("JetBrains Mono").font_weight(FontWeight::SEMIBOLD)
        .text_size(px((13. * size / 20.).floor())).line_height(px(size * 0.8))
        .whitespace_nowrap()
        .text_color(rgb(0xffffff)).bg(linear_gradient(45., linear_color_stop(rgb(end),0.), linear_color_stop(rgb(start),1.)))
        .child(initials)
}

fn row_selection(focus: &FocusHandle, window: &Window) -> u32 {
    if window.is_window_active() && focus.is_focused(window) { 0x2a4371 } else { 0x33353b }
}
fn tree_selection(color: u32) -> impl IntoElement {
    div().absolute().left(px(12.)).right(px(12.)).top_0().h_full().rounded(px(4.)).bg(rgb(color))
}

// Rebased MainFrame.setToolbarHeightReferent: all navigation panes start their
// rows below the same toolbar band. Keep this independent of the top tab strip.
const LOG_TOOLBAR_HEIGHT: f32 = 36.;
const COLORS: [u32; 8] = [
    0x6aab73, 0x548af7, 0xc77dbb, 0xe5b567, 0x56b6c2, 0xcc7832, 0x9876aa, 0xe06c75,
];
#[derive(Clone)]
enum FileRow {
    Group(Area, usize, bool),
    File(usize, usize),
    Folder(Area, PathBuf, usize, usize, bool),
}
#[derive(Clone)]
struct NavigationNotice {
    name: String,
    target: String,
    can_clear: bool,
}
struct SettingsDraft {
    section: usize,
    language: Language,
    bottom_preview: bool,
    file_preview: bool,
    git: Option<Entity<crate::update_settings::Settings>>,
    loading: bool,
    error: Option<String>,
}

pub struct Review {
    startup_report: Option<Arc<crate::startup_report::Probe>>,
    fetch_token: Option<Cancel>,
    fetch_task: Task<()>,
    reset_running: bool,
    fetch_status: Option<String>,
    rebase_diagnostic: Option<String>,
    squash_undo: Option<Arc<plico::squash_undo::Undo>>,
    operation_state: Option<plico::operation_state::State>,
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
    selection_details: Vec<(Vec<git::RefLabel>, commit_presentation::Presentation, Entity<crate::containing_branches::ContainingBranches>)>,
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
    preview_source_position: Option<(bool,usize)>,
    adjacent_task: Task<()>,
    adjacent_token: Cancel,
    adjacent_menu: Option<Entity<gpui_kit::component::menu::PopupMenu>>,
    adjacent_subscription: Option<Subscription>,
    adjacent_position: Point<Pixels>,
    branch_rows: Arc<Vec<branches::Row>>,
    collapsed_branches: HashSet<String>,
    local: bool,
    expanded: bool,
    tabs: Vec<Entity<DiffTab>>,
    active_tab: Option<usize>,
    local_preview: Option<Entity<DiffTab>>,
    local_visible: bool,
    local_view: Option<Entity<crate::local_view::LocalView>>,
    project_visible: bool,
    project_view: Option<Entity<crate::project_view::ProjectView>>,
    file_tabs: Vec<Entity<crate::project_view::FileTab>>,
    documents: crate::editing::Documents,
    document_action_pending: bool,
    editor_tab_order: Vec<EntityId>,
    active_file_tab: Option<usize>,
    bottom_preview: bool,
    file_preview: bool,
    preview_mode: DiffMode,
    preview_appearance: layout_preferences::DiffAppearance,
    preview_preference_error: Option<String>,
    saved_changes_available: bool,
    stash_open: bool,
    stash_view: Option<Entity<crate::stash_view::StashView>>,
    settings_open: bool,
    settings_session: u64,
    settings_draft: Option<SettingsDraft>,
    message: String,
    commit_metadata: [String; 2],
    containing_branches: Option<Entity<crate::containing_branches::ContainingBranches>>,
    base: Option<i18n::ComparisonTitle>,
    error: Option<String>,
    preview_error: Option<String>,
    busy: bool,
    history_loading: bool,
    loading_patch: bool,
    limit: usize,
    history_offset: usize,
    window_start: graph::Layout,
    window_end: graph::Layout,
    generation: u64,
    token: Cancel,
    task: Task<()>,
    diff: Entity<DiffState>,
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
    log_navigation: Entity<ResizableState>,
    compact_log_navigation: Entity<ResizableState>,
    navigation_width: Pixels,
    history_toolbar_width: Pixels,
    branches_visible: bool,
    local_navigation: Entity<ResizableState>,
    tool_windows_width: Entity<ResizableState>,
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
        if let Some(token) = &self.fetch_token { token.cancel(); }
        self.token.cancel();
        self.graph_print_token.cancel();
        self.adjacent_token.cancel();
        self.source_token.cancel();
        if let Some(token) = &self.repository_recent_token { token.cancel(); }
    }
}
// Measure the allocated pane, not the whole window: a tool window or a dragged
// divider can change this width without changing the native window bounds.
fn observe_review_width(owner: WeakEntity<Review>, navigation: bool)
    -> impl Fn(Bounds<Pixels>, &mut Window, &mut App) {
    move |bounds, window, cx| {
        let Some(view) = owner.upgrade() else { return; };
        let previous = if navigation { view.read(cx).navigation_width } else { view.read(cx).history_toolbar_width };
        if (previous - bounds.size.width).abs() <= px(0.5) { return; }
        let width = bounds.size.width;
        let owner = owner.clone();
        window.defer(cx, move |_,cx| {
            let _ = owner.update(cx, |s,cx| {
                if navigation { s.navigation_width = width; } else { s.history_toolbar_width = width; }
                cx.notify();
            });
        });
    }
}
// Interactive title-bar controls must hide the native caption hitbox beneath them.
// Otherwise Windows consumes physical left clicks as window dragging.
// Popover owns the bubbling mouse-down handler and stops propagation itself.
// Stopping at its child button prevents menus from ever seeing the click.
fn titlebar_menu_button(id: &'static str) -> Button { Button::new(id) }
fn titlebar_menu(menu: impl IntoElement) -> impl IntoElement {
    // Shield the native caption behind the whole popup trigger, not its child
    // button: occluding that child also hides the popup's mouse hitbox.
    div().occlude().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(menu)
}

fn titlebar_button(id: &'static str) -> Button {
    Button::new(id).occlude().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

impl Review {
    pub fn new(path: PathBuf, limit: usize, startup_report: Option<Arc<crate::startup_report::Probe>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title(&if path.as_os_str().is_empty() { "Plico".into() } else { format!("Plico – {}", path.file_name().unwrap_or_default().to_string_lossy()) });
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
            t.font_family = crate::ui_font::family().into();
            t.font_size = px(13.);
            t.mono_font_family = "JetBrains Mono".into();
            t.mono_font_size = px(13.);
            // Palette provenance and capture-mapping limits: assets/themes/SOURCE.txt.
            let highlight = Arc::make_mut(&mut t.highlight_theme);
            highlight.name = "Islands Dark".into();
            highlight.style.editor_background = Some(rgb(0x191a1c).into());
            highlight.style.editor_foreground = Some(rgb(0xbcbec4).into());
            highlight.style.editor_line_number = Some(rgb(0x4b5059).into());
            highlight.style.syntax = serde_json::from_str(include_str!("../assets/themes/islands-syntax.json"))
                .expect("Bundled Islands syntax palette");
        });
        let focus = cx.focus_handle();
        let history_focus = cx.focus_handle().tab_stop(true);
        let initial_focus = focus.clone();
        window.on_next_frame(move |window, cx| initial_focus.focus(window, cx));
        let mut this = Self {
            startup_report,
            welcome: path.as_os_str().is_empty(),
            ui_chinese: i18n::is_chinese(),
            recent_projects: plico::recent_projects::load(), recent_recorded: false,
            repo: Repository::new(path.clone()),
            snapshot: None,
            rows: Arc::default(),
            graph_model: None,
            show_long_edges: layout_preferences::load_log_long_edges(&path),
            graph_print_token: Cancel::default(), graph_print_task: Task::ready(()),
            graph_print_generation: 0, graph_print_loading: false,
            graph_hover: None,
            commits: Arc::default(),
            files: Arc::default(),
            file_rows: Arc::default(),
            collapsed_files: Vec::new(),
            collapsed_directories: HashSet::new(),
            comparison: None, comparison_visible: false, comparison_preview: None,
            selected: None,
            log_selection: Default::default(), aggregate_chain: None, selection_details: Vec::new(), selection_branches: None,
            selected_file: None,
            pending_preview_file: None,
            branch: None,
            selected_tree_ref: None,
            navigation_loading: None,
            navigation_notice: None,
            source_task: Task::ready(()), source_token: Cancel::default(), source_generation: 0, preview_source_position: None,
            adjacent_task: Task::ready(()), adjacent_token: Cancel::default(),
            adjacent_menu: None, adjacent_subscription: None, adjacent_position: point(px(0.), px(0.)),
            branch_rows: Arc::default(),
            collapsed_branches: HashSet::new(),
            local: false,
            expanded: false,
            tabs: Vec::new(),
            active_tab: None,
            local_preview: None, local_visible: false, local_view: None,
            project_visible: false, project_view: None, file_tabs: Vec::new(), documents: Self::new_documents(cx), document_action_pending: false, editor_tab_order: Vec::new(), active_file_tab: None,
            saved_changes_available: false,
            stash_open: false,
            stash_view: None,
            bottom_preview: layout_preferences::load_bottom_preview(),
            file_preview: layout_preferences::load_file_preview(),
            preview_mode: DiffMode::Split,
            preview_appearance: Default::default(),
            preview_preference_error: None,
            settings_open: false,
            settings_session: 0,
            settings_draft: None,
            message: String::new(),
            commit_metadata: Default::default(),
            containing_branches: None,
            base: None,
            error: None,
            preview_error: None,
            busy: false,
            history_loading: false,
            loading_patch: false,
            fetch_token: None, fetch_task: Task::ready(()), reset_running: false, fetch_status: None, rebase_diagnostic: None, squash_undo: None, operation_state: None,
            limit,
            history_offset: 0,
            window_start: graph::Layout::default(),
            window_end: graph::Layout::default(),
            generation: 0,
            token: Cancel::default(),
            task: Task::ready(()),
            diff: cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Split)),
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
            log_navigation: cx.new(|_| ResizableState::default()),
            compact_log_navigation: cx.new(|_| ResizableState::default()),
            navigation_width: px(1000.), history_toolbar_width: px(1000.),
            branches_visible: false,
            local_navigation: cx.new(|_| ResizableState::default()),
            tool_windows_width: cx.new(|_| ResizableState::default()),
            files_fraction: layout_preferences::load_files_fraction(),
            text_filter: None,
            filter_regex: false,
            filter_match_case: false,
            date_filter: Default::default(),
            repository_menu_open: false,
            repository_search_editing: false,
            repository_search_input: cx.new(|cx| InputState::new(window,cx).placeholder(t("Search for branches and actions"))),
            repository_search: None,
            repository_applied_query: String::new(),
            repository_search_task: Task::ready(()),
            repository_selection: None,
            repository_actions: vec!["commit".into(), "revision".into()],
            repository_pointer: None,
            repository_child_selection: None,
            repository_child_menu: None,
            repository_recent: Vec::new(),
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
            user_menu: cx.new(|cx| crate::user_menu::UserMenu::new(Repository::new(path.clone()), cx)),
            filter_search: cx
                .new(|cx| InputState::new(window, cx).placeholder(t("Text or hash"))),
            branch_search: cx.new(|cx| InputState::new(window, cx).placeholder(t("Branches and Tags"))),
        };
        for handle in [&this.tree_focus, &this.history_focus, &this.files_focus] {
            cx.on_focus(handle, window, |_, _, cx| cx.notify()).detach();
            cx.on_blur(handle, window, |_, _, cx| cx.notify()).detach();
        }
        cx.bind_keys([KeyBinding::new("ctrl-s", crate::project_view::SaveFile, Some("Review"))]);
        cx.observe_window_activation(window, |s, window, cx| {
            if !window.is_window_active() { s.documents.save_all(cx); }
            cx.notify();
        }).detach();
        let owner = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            owner.update(cx, |s,cx| {
                if !s.documents.pending(cx) && !s.document_action_pending { return true; }
                s.with_saved_documents(window, cx, |_,window,_|window.remove_window()); false
            }).unwrap_or(true)
        });
        cx.subscribe(&this.paths_menu, |s, _, _: &crate::paths_menu::Changed, cx| s.apply_text_filter(cx)).detach();
        cx.subscribe(&this.user_menu, |s, _, event: &crate::user_menu::Changed, cx| match event {
            crate::user_menu::Changed::Applied => s.apply_text_filter(cx),
            crate::user_menu::Changed::Failed(error) => {
                s.error = Some(format!("Could not refresh User filter: {error}")); cx.notify();
            }
        }).detach();
        // Diff is a RenderOnce element: its owner must redraw when retained
        // state changes (initial viewport measurement, syntax and selection).
        cx.observe(&this.diff, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&this.diff,|s,_,event:&gpui_kit::component::diff::DiffEvent,cx|{
            if let gpui_kit::component::diff::DiffEvent::SelectionChanged(Some(range))=event {
                s.preview_source_position=Some((range.end_side()==gpui_kit::component::diff::DiffSide::Original,range.end().saturating_sub(1)));cx.notify();
            }
        }).detach();
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
        cx.subscribe(&this.repository_search_input, |s,_,event: &InputEvent,cx| {
            if matches!(event,InputEvent::Change) && s.repository_menu_open {
                s.dismiss_repository_child(cx);
                s.repository_search_task=cx.spawn(async move |owner,cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
                    let _=owner.update(cx,|s,cx| { if s.repository_menu_open { s.apply_repository_search(cx); } });
                });
            }
        }).detach();
        if !this.welcome {
            this.refresh(false, cx);
        }
        this
    }
    fn reset_job(&mut self, cx: &mut Context<Self>) -> (u64, Cancel) {
        self.graph_hover = None;
        self.graph_print_token.cancel();
        self.graph_print_generation += 1;
        self.graph_print_task = Task::ready(());
        self.graph_print_loading = false;
        self.adjacent_token.cancel();
        self.adjacent_menu = None; self.adjacent_subscription = None;
        self.token.cancel();
        self.token = Cancel::default();
        self.generation += 1;
        self.error = None;
        self.preview_error = None;
        self.navigation_loading = None;
        self.navigation_notice = None;
        self.loading_patch = false;
        self.diff.update(cx, |d, cx| d.set_files([], cx));
        (self.generation, self.token.clone())
    }
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() { return; }
        let request = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Git repository".into()),
        });
        cx.spawn_in(window, async move |this, wx| {
            let Some(path) = request
                .await
                .ok()
                .and_then(|r| r.ok())
                .flatten()
                .and_then(|v| v.into_iter().next())
            else {
                return;
            };
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| s.choose_project_window(path, window, cx));
            });
        })
        .detach();
    }
    fn choose_project_window(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.settings_open { return; }
        if self.repo.path.as_os_str().is_empty() {
            if !path.is_dir() { self.error = Some(t("Project folder is unavailable").into()); cx.notify(); return; }
            let old = window.bounds();
            let mut bounds = Bounds::new(
                point(old.origin.x + (old.size.width - px(1440.)) / 2., old.origin.y + (old.size.height - px(900.)) / 2.),
                size(px(1440.), px(900.)),
            );
            if let Some(display) = window.display(cx) {
                let screen = display.bounds();
                bounds.size.width = bounds.size.width.min(screen.size.width);
                bounds.size.height = bounds.size.height.min(screen.size.height);
                bounds.origin.x = bounds.origin.x.max(screen.origin.x).min(screen.origin.x + screen.size.width - bounds.size.width);
                bounds.origin.y = bounds.origin.y.max(screen.origin.y).min(screen.origin.y + screen.size.height - bounds.size.height);
            }
            match crate::open_project_window_at(path, self.limit, None, Some(bounds), cx) {
                Ok(()) => window.remove_window(),
                Err(error) => { self.error = Some(error.to_string()); cx.notify(); }
            }
            return;
        }
        let owner = cx.entity().downgrade();
        let limit = self.limit;
        window.open_dialog(cx, move |dialog, _, _| {
            let current = owner.clone(); let new_owner = owner.clone(); let current_path = path.clone(); let new_path = path.clone();
            dialog.title(t("Open Project")).w(px(480.)).centered(true)
                .child(div().child(t("Where would you like to open the project?")))
                .child(div().text_color(rgb(MUTED)).truncate().child(project_display_path(&path)))
                .footer(DialogFooter::new()
                    .child(DialogAction::new().child(Button::new("project-this-window").primary().label(t("This Window"))))
                    .child(Button::new("project-new-window").label(t("New Window"))
                        .on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            if let Err(error) = crate::open_project_window(new_path.clone(), limit, cx) {
                                let _ = new_owner.update(cx, |s, cx| { s.error = Some(error.to_string()); cx.notify(); });
                            }
                        }))
                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.label(t("Cancel")))))
                .on_ok(move |_, w, cx| {
                    let _ = current.update(cx, |s, cx| s.open_repository(current_path.clone(), w, cx));
                    true
                })
        });
    }
    fn open_repository(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.open_repository(path,w,cx)); return; }
        if self.fetch_token.is_some() { return; }
        if !path.is_dir() { self.error = Some(t("Project folder is unavailable").into()); cx.notify(); return; }
        self.source_token.cancel();self.source_generation+=1;self.source_task=Task::ready(());
        self.recent_recorded = false;
                    self.welcome = false;
                    self.dismiss_repository_menu(cx);
                    self.repository_recent.clear();
                    self.repository_favorites = Default::default();
                    self.repository_favorites_path = None;
                    self.repository_favorites_saving = false;
                    self.repository_favorites_error = None;
                    self.repository_favorites_epoch += 1;
                    self.repository_expansion = Default::default();
                    window.set_window_title(&format!("Plico – {}", path.file_name().unwrap_or_default().to_string_lossy()));
                    self.repo = Repository::new(path);
                    self.show_long_edges = layout_preferences::load_log_long_edges(&self.repo.path);
                    self.graph_model = None;
                    self.log_selection = Default::default(); self.aggregate_chain = None; self.selection_details.clear(); self.selection_branches = None;
                    self.rebase_diagnostic = None; self.squash_undo = None;
                    self.operation_state = None;
                    self.paths_menu.update(cx, |menu, cx| menu.reset(self.repo.path.clone(), cx));
                    self.user_menu.update(cx, |menu, cx| menu.reset(self.repo.clone(), cx));
                    self.branch = None;
                    self.selected_tree_ref = None;
                    self.branch_rows = Arc::default();
                    self.collapsed_branches.clear();
                    self.snapshot = None;
                    self.commits = Arc::default();
                    self.rows = Arc::default();
                    self.selected = None;
                    self.selected_file = None;
                    self.pending_preview_file = None;
                    self.local = false;
                    self.expanded = false;
                    self.saved_changes_available = false;
                    self.stash_open = false;
                    self.stash_view = None;
                    self.comparison = None; self.comparison_visible = false; self.comparison_preview = None; self.local_preview = None; self.local_view = None; self.local_visible = false; self.project_view = None; self.project_visible = false; self.file_tabs.clear(); self.documents = Self::new_documents(cx); self.editor_tab_order.clear(); self.active_file_tab = None;
                    self.tabs.clear();
                    self.active_tab = None;
                    self.close_settings(cx);
                    self.collapsed_files.clear();
                    self.collapsed_directories.clear();
                    self.history_offset = 0;
                    self.window_start = graph::Layout::default();
                    self.window_end = graph::Layout::default();
                    self.history_scroll = UniformListScrollHandle::new();
                    self.tree_scroll = UniformListScrollHandle::new();
                    self.history_horizontal = ScrollHandle::new();
                    self.files_scroll = UniformListScrollHandle::new();
                    self.filter_search
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.text_filter = None;
                    self.date_filter = Default::default();
                    self.branch_search
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.limit = git::HISTORY_PAGE;
                    self.refresh(false, cx);
                    self.focus.focus(window, cx);
    }
    fn refresh(&mut self, more: bool, cx: &mut Context<Self>) {
        self.refresh_impl(more, false, None, cx);
    }
    fn refresh_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stash_open { if let Some(view) = &self.stash_view { view.update(cx, |s, cx| s.refresh(cx)); } }
        if let Some(tab) = self.active_tab.and_then(|i| self.tabs.get(i)) {
            let callback=self.diff_source_callback(cx);
            tab.update(cx, |tab, cx| {tab.source_callback=Some(callback);tab.activate(window, cx);});
            return;
        }
        // During a refresh the displayed file list is temporarily empty. A
        // second refresh must not replace the in-flight selection with None.
        if self.busy || self.history_loading {
            return;
        }
        let user_active = self.user_menu.read(cx).active();
        self.user_menu.update(cx, |menu, cx| menu.refresh(cx));
        if user_active { return; }
        self.refresh_impl(false, true, None, cx);
    }
    fn previous_window(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.history_loading || self.history_offset == 0 {
            return;
        }
        let offset = self.history_offset.saturating_sub(git::HISTORY_MAX);
        self.refresh_impl(false, false, Some(offset), cx);
    }
    fn refresh_impl(
        &mut self,
        more: bool,
        preserve: bool,
        requested_offset: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if !more { if let Some(view) = &self.local_view { view.update(cx, |s, cx| s.refresh(cx)); } }
        let preferred = self.selected_file.and_then(|i| self.files.get(i)).cloned()
            .or_else(|| self.pending_preview_file.clone());
        self.pending_preview_file = preferred.clone();
        let top_index = {
            let scroll = self.history_scroll.0.borrow();
            scroll
                .deferred_scroll_to_item
                .as_ref()
                .map(|item| item.item_index)
                .unwrap_or_else(|| {
                    ((-scroll.base_handle.offset().y / px(28.)).max(0.)).floor() as usize
                })
        };
        let anchor = self.commits.get(top_index).map(|c| c.id.clone());
        let previous = self
            .selected
            .and_then(|i| self.commits.get(i))
            .map(|c| c.id.clone());
        let (generation, token) = self.reset_job(cx);
        self.busy = true;
        self.history_loading = true;
        self.files = Arc::default();
        self.rebuild_files();
        self.selected_file = None;
        self.message.clear();
        self.commit_metadata = Default::default();
        self.containing_branches = None;
        self.base = None;
        let repo = self.repo.clone();
        let branch = self.branch.clone();
        let text_filter = self.text_filter.clone();
        let local = self.local;
        let show_long_edges = self.show_long_edges;
        let next_window = more && self.limit >= git::HISTORY_MAX;
        let offset = if let Some(offset) = requested_offset {
            offset
        } else if next_window {
            self.history_offset + self.commits.len()
        } else if more || preserve {
            self.history_offset
        } else {
            0
        };
        let start = if next_window {
            self.window_end.clone()
        } else if more {
            self.window_start.clone()
        } else {
            graph::Layout::default()
        };
        let limit = if requested_offset.is_some() {
            git::HISTORY_MAX
        } else if next_window {
            git::HISTORY_PAGE
        } else if more {
            (self.limit + git::HISTORY_PAGE).min(git::HISTORY_MAX)
        } else {
            self.limit
        };
        let layout_start = if preserve || requested_offset.is_some() {
            None
        } else {
            Some(start.clone())
        };
        let fixed = if more || requested_offset.is_some() {
            self.snapshot.as_ref().map(|s| s.tips.clone())
        } else {
            None
        };
        let filtering = text_filter.is_some();
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let state = repo.operation_state(&token);
                    let result = if local {
                        repo.local_changes(&token).map(|f| (None, Some(f)))
                    } else if let Some(filter) = &text_filter {
                        history::read_filtered(
                            &repo,
                            branch.as_deref(),
                            offset,
                            limit,
                            fixed.as_deref(),
                            filter,
                            &token,
                        )
                        .map(|window| (Some(window), None))
                    } else {
                        history::read(
                            &repo,
                            branch.as_deref(),
                            offset,
                            limit,
                            fixed.as_deref(),
                            layout_start,
                            &token,
                        )
                        .map(|window| (Some(window), None))
                    };
                    let mut graph_model = None;
                    let result = result.and_then(|(window, local)| {
                        let window = window.map(|mut window: history::Window| {
                            let model = Arc::new(plico::graph_print::Model::new(&window.rows, &token)?);
                            window.rows = model.project(show_long_edges, &token)?;
                            graph_model = Some(model);
                            Ok::<_, String>(window)
                        }).transpose()?;
                        Ok((window, local))
                    });
                    (result, state, graph_model)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                this.history_loading = false;
                this.operation_state = result.1.ok();
                match result.0 {
                    Ok((
                        Some(history::Window {
                            snapshot: mut s,
                            rows,
                            start,
                            end,
                        }),
                        _,
                    )) => {
                        if !this.recent_recorded {
                            this.recent_recorded = true;
                            match plico::recent_projects::opened(&this.repo.path) {
                                Ok(paths) => this.recent_projects = paths,
                                Err(error) => this.fetch_status = Some(format!("{}: {error}", t("Could not save recent projects"))),
                            }
                        }
                        if offset != this.history_offset {
                            this.history_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        }
                        if preserve {
                            if let Some(index) = anchor
                                .as_ref()
                                .and_then(|id| s.commits.iter().position(|c| &c.id == id))
                            {
                                this.history_scroll
                                    .scroll_to_item_strict(index, ScrollStrategy::Top);
                            }
                        }
                        this.history_offset = offset;
                        this.window_start = start;
                        this.window_end = end;
                        this.limit = limit;
                        let selected = previous
                            .and_then(|id| s.commits.iter().position(|c| c.id == id))
                            .unwrap_or(0);
                        this.rows = Arc::new(rows);
                        this.graph_model = result.2;
                        this.commits = Arc::new(std::mem::take(&mut s.commits));
                        if this.squash_undo.as_ref().is_some_and(|undo| s.head_id.as_deref() != Some(undo.result_head())) { this.squash_undo = None; }
                        this.snapshot = Some(s);
                        this.rebuild_branches(cx);
                        this.selected = None;
                        let ids = this.commit_ids();
                        this.log_selection.reconcile(&ids);
                        if this.log_selection.ordered(&ids).is_empty() && !ids.is_empty() && !preserve {
                            this.log_selection.select(&ids, selected, false, false);
                        }
                        this.pending_preview_file = preferred;
                        this.load_log_selection(cx);
                    }
                    Ok((_, Some(f))) => {
                        this.files = Arc::new(f);
                        this.prune_directory_state();
                        this.rebuild_files();
                        this.base = Some(i18n::ComparisonTitle::Label("Local changes · staged and unstaged are separate comparisons"));
                        this.pending_preview_file = None;
                        if let Some(index) = plico::preview_selection::index(&this.files, preferred.as_ref()) {
                            this.load_file(index, cx);
                        }
                    }
                    Err(e) => {
                        this.error = Some(e);
                        if filtering {
                            this.commits = Arc::default();
                            this.rows = Arc::default();
                            this.graph_model = None;
                            this.selected = None;
                            this.snapshot = None;
                        }
                    }
                    _ => {}
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn select_commit(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.history_loading { return; }
        let ids = self.commit_ids();
        self.log_selection.select(&ids, index, false, false);
        self.load_log_selection(cx);
    }
    fn commit_ids(&self) -> Vec<String> { self.commits.iter().map(|c| c.id.clone()).collect() }
    fn select_log_row(&mut self, index: usize, toggle: bool, extend: bool, context: bool, cx: &mut Context<Self>) {
        if self.history_loading { return; }
        let ids = self.commit_ids();
        let before = self.log_selection.ordered(&ids).into_iter().map(str::to_owned).collect::<Vec<_>>();
        if context { self.log_selection.context(&ids, index); }
        else { self.log_selection.select(&ids, index, toggle, extend); }
        if before.iter().map(String::as_str).collect::<Vec<_>>() != self.log_selection.ordered(&ids) { self.load_log_selection(cx); }
        cx.notify();
    }
    fn load_log_selection(&mut self, cx: &mut Context<Self>) {
        let rows = self.commit_ids();
        let ids = self.log_selection.ordered(&rows).into_iter().map(str::to_owned).collect::<Vec<_>>();
        let preferred = self.selected_file.and_then(|i| self.files.get(i)).cloned().or_else(|| self.pending_preview_file.clone());
        self.aggregate_chain = None; self.selection_details.clear(); self.selection_branches = None; self.local_preview = None;
        if ids.len() == 1 {
            if let Some(index) = rows.iter().position(|id| id == &ids[0]) { self.select_commit_with_file(index, preferred, cx); }
            return;
        }
        let (generation, token) = self.reset_job(cx);
        self.selected = None; self.selected_file = None; self.files = Arc::default(); self.rebuild_files();
        self.message.clear(); self.commit_metadata = Default::default(); self.containing_branches = None; self.base = None;
        self.pending_preview_file = preferred.clone(); self.busy = !ids.is_empty();
        if ids.is_empty() { self.pending_preview_file = None; cx.notify(); return; }
        let repo = self.repo.clone();
        self.task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                let snapshot = plico::log_changes::load(&repo, &ids, &token)?;
                let mut presentations = Vec::new();
                let mut presentation_bytes = 0usize;
                for commit in snapshot.commits.iter().take(50) {
                    let details = repo.commit_metadata(commit, &token)?;
                    let presentation = commit_presentation::build(&details, Some(commit));
                    presentation_bytes = presentation_bytes.saturating_add(presentation.message.len())
                        .saturating_add(presentation.metadata.iter().map(String::len).sum::<usize>());
                    if presentation_bytes > git::OUTPUT_LIMIT { return Err("Log details exceed the preview output limit".into()); }
                    presentations.push((commit.refs.clone(), presentation, commit.id.clone()));
                }
                let chain = if snapshot.files.is_empty() { None } else { Some(diff_target::Chain::combined(repo, snapshot.files, 0)?.1) };
                Ok::<_, String>((chain, presentations))
            }).await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation { return; }
                this.busy = false; this.pending_preview_file = None;
                match result {
                    Ok((chain, presentations)) => {
                        this.files = Arc::new(chain.as_ref().map(|c| c.changes().to_vec()).unwrap_or_default());
                        this.aggregate_chain = chain;
                        let mut cards = Vec::new();
                        this.selection_details = presentations.into_iter().map(|(refs, presentation, id)| {
                            let view = cx.new(crate::containing_branches::ContainingBranches::pending);
                            cards.push((id, view.downgrade()));
                            (refs, presentation, view)
                        }).collect();
                        let repo = this.repo.clone();
                        this.selection_branches = Some(cx.new(|cx| crate::containing_branches::Batch::new(repo, cards, cx)));
                        this.prune_directory_state(); this.rebuild_files();
                        if let Some(index) = plico::preview_selection::index(&this.files, preferred.as_ref()) { this.load_file(index, cx); }
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn select_commit_with_file(
        &mut self,
        index: usize,
        preferred: Option<Change>,
        cx: &mut Context<Self>,
    ) {
        if self.history_loading {
            return;
        }
        let Some(commit) = self.commits.get(index).cloned() else {
            return;
        };
        let (generation, token) = self.reset_job(cx);
        self.aggregate_chain = None; self.selection_details.clear();
        self.selected = Some(index);
        self.pending_preview_file = preferred.clone();
        self.selected_file = None;
        self.files = Arc::default();
        self.rebuild_files();
        self.busy = true;
        self.message.clear();
        self.commit_metadata = Default::default();
        self.containing_branches = None;
        self.base = None;
        self.containing_branches = Some(cx.new(|cx| crate::containing_branches::ContainingBranches::new(self.repo.clone(), commit.id.clone(), cx)));
        let repo = self.repo.clone();
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { repo.details(&commit, &token) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                match result {
                    Ok(d) => {
                        let commit = this.selected.and_then(|i| this.commits.get(i));
                        let presentation = commit_presentation::build(&d, commit);
                        this.message = presentation.message;
                        this.commit_metadata = presentation.metadata;
                        this.base = Some(if commit.is_some_and(|commit| commit.shallow_boundary) {
                            i18n::ComparisonTitle::Label("Shallow boundary · Parent comparison unavailable")
                        } else {
                            i18n::ComparisonTitle::Parent {
                                revision: d.base,
                                merge: commit.is_some_and(|commit| commit.parents.len() > 1),
                            }
                        });
                        this.files = Arc::new(d.files);
                        this.prune_directory_state();
                        this.rebuild_files();
                        this.pending_preview_file = None;
                        if let Some(index) = plico::preview_selection::index(&this.files, preferred.as_ref()) {
                            this.load_file(index, cx);
                        }
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn set_bottom_preview(&mut self, bottom: bool, cx: &mut Context<Self>) -> bool {
        if let Err(error) = layout_preferences::save_bottom_preview(bottom) {
            if let Some(draft) = &mut self.settings_draft {
                draft.section = 2;
                draft.error = Some(format!("{}: {error}", t("Settings could not be saved")));
            }
            cx.notify(); return false;
        }
        self.bottom_preview = bottom;
        if let Some(index) = self.selected_file { self.load_file(index, cx); }
        cx.notify();
        true
    }
    fn set_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Err(error) = layout_preferences::save_language(language) {
            if let Some(draft) = &mut self.settings_draft {
                draft.section = 0;
                draft.error = Some(format!("{}: {error}", t("Settings could not be saved")));
            }
            cx.notify();
            return false;
        }
        i18n::apply(language);
        gpui_kit::component::set_locale(if i18n::is_chinese() { "zh-CN" } else { "en" });
        self.sync_language(window, cx);
        cx.refresh_windows();
        cx.notify();
        true
    }
    // Language is application-wide. Each retained window synchronizes its
    // inputs on redraw, without rebuilding them or changing their text/focus.
    fn sync_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chinese = i18n::is_chinese();
        if self.ui_chinese == chinese { return; }
        self.ui_chinese = chinese;
        self.filter_search.update(cx, |input, cx| input.set_placeholder(t("Text or hash"), window, cx));
        self.branch_search.update(cx, |input, cx| input.set_placeholder(t("Branches and Tags"), window, cx));
        self.repository_search_input.update(cx, |input, cx| input.set_placeholder(t("Search for branches and actions"), window, cx));
        if self.repository_menu_open { self.apply_repository_search(cx); }
    }
    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        if self.settings_open { return; }
        self.settings_session = self.settings_session.wrapping_add(1);
        self.settings_draft = Some(SettingsDraft { section: 0, language: i18n::preference(), bottom_preview: self.bottom_preview, file_preview: self.file_preview, git: None, loading: false, error: None });
        self.settings_open = true;
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, cx| {
            let body_owner = owner.clone();
            let confirm_owner = owner.clone();
            let close_owner = owner.clone();
            let title = owner.update(cx, |s, _| {
                if s.welcome { t("Settings").to_owned() }
                else { format!("{} — {}", t("Settings"), s.repo.path.file_name().unwrap_or_default().to_string_lossy()) }
            }).unwrap_or_else(|_| t("Settings").to_owned());
            dialog.title(div().text_size(px(14.)).font_weight(FontWeight::NORMAL).child(title)).pb_0()
                .w(px(900.).min(window.viewport_size().width - px(32.)))
                .h(px(650.).min(window.viewport_size().height - px(48.)))
                .centered(true).overlay_closable(false)
                .content(move |body, _, cx| {
                    let panel = body_owner.update(cx, |s, cx| {
                        if s.settings_draft.is_some() { s.settings_panel(cx).into_any_element() }
                        else { div().into_any_element() }
                    }).unwrap_or_else(|_| div().into_any_element());
                    body.p_0().min_h_0().overflow_hidden().child(panel)
                })
                .on_ok(move |_, window, cx| confirm_owner.update(cx, |s, cx| {
                    s.apply_settings(true, window, cx); !s.settings_open
                }).unwrap_or(true))
                .on_close(move |_, _, cx| { let _ = close_owner.update(cx, |s, cx| s.close_settings(cx)); })
        });
        cx.notify();
    }
    fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = false;
        self.settings_draft = None;
        cx.notify();
    }
    fn apply_settings(&mut self, close: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &self.settings_draft else { return; };
        if draft.loading || self.fetch_token.is_some() { return; }
        let (language, bottom, file_preview, git) = (draft.language, draft.bottom_preview, draft.file_preview, draft.git.clone());
        if let Some(draft) = &mut self.settings_draft { draft.error = None; }
        if let Some(git) = git { if !git.update(cx, |s, cx| s.save(cx)) { if let Some(d) = &mut self.settings_draft { d.section = 1; } cx.notify(); return; } }
        if language != i18n::preference() && !self.set_language(language, window, cx) { return; }
        if bottom != self.bottom_preview && !self.set_bottom_preview(bottom, cx) { return; }
        if file_preview != self.file_preview {
            if let Err(error) = layout_preferences::save_file_preview(file_preview) {
                if let Some(d) = &mut self.settings_draft { d.section = 3; d.error = Some(format!("{}: {error}", t("Settings could not be saved"))); }
                cx.notify(); return;
            }
            self.file_preview = file_preview;
            if let Some(view) = &self.project_view { view.update(cx, |v, cx| v.set_preview_enabled(file_preview, cx)); }
            if !file_preview { for tab in &self.file_tabs { tab.update(cx, |t, _| t.preview = false); } }
        }
        if close { self.close_settings(cx); }
        cx.notify();
    }
    fn open_update_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.welcome || self.fetch_token.is_some() { return; }
        let Some(draft) = &mut self.settings_draft else { return; };
        if draft.git.is_some() || draft.loading { return; }
        draft.loading = true; draft.error = None;
        let session = self.settings_session;
        let repo = self.repo.clone(); let path = repo.path.clone();
        cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                plico::update_preferences::for_repository(&repo, &Cancel::default()).map(|(path, preferences)| {
                    let show = plico::project_update_preferences::load_show_options(&path.with_file_name("project-update-options"));
                    (path, preferences, show)
                })
            }).await;
            let _ = wx.update(|_, cx| { let _ = this.update(cx, |s, cx| {
                if s.repo.path != path || !s.settings_open || s.settings_session != session { return; }
                let Some(draft) = &mut s.settings_draft else { return; };
                draft.loading = false;
                match result {
                    Ok((path, preferences, show)) => {
                        let git = crate::update_settings::new(path, preferences, show, cx);
                        cx.observe(&git, |_, _, cx| cx.notify()).detach();
                        draft.git = Some(git);
                    },
                    Err(error) => draft.error = Some(error),
                }
                cx.notify();
            }); });
        }).detach();
        cx.notify();
    }
    fn settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let draft = self.settings_draft.as_ref().expect("Open settings have a draft");
        let modified = draft.language != i18n::preference() || draft.bottom_preview != self.bottom_preview || draft.file_preview != self.file_preview
            || draft.git.as_ref().is_some_and(|git| git.read(cx).modified());
        let mut navigation = v_flex().w(px(240.)).flex_shrink_0().py_3();
        for (section, label) in [(0, "Language"), (3, "Editor Tabs"), (1, "Git"), (2, "Diff")] {
            if section == 1 { navigation = navigation.child(div().mt_2().px_4().py_2().font_weight(FontWeight::SEMIBOLD).child(t("Version Control"))); }
            navigation = navigation.child(Button::new(("settings-section", section)).ghost().h(px(26.)).rounded(px(0.)).accessibility_label(t(label))
                .child(div().w_full().pl(if section == 0 { px(4.) } else { px(28.) }).child(t(label)))
                .selected(draft.section == section).when(draft.section == section, |b| b.bg(rgb(0x2d466f)))
                .on_click(cx.listener(move |s, _, w, cx| {
                    if let Some(draft) = &mut s.settings_draft { draft.section = section; }
                    if section == 1 { s.open_update_settings(w, cx); }
                    cx.notify();
                })));
        }
        let title = match draft.section { 3 => t("Editor Tabs").to_owned(), 1 => format!("{}  ›  Git", t("Version Control")), 2 => format!("{}  ›  {}", t("Version Control"), t("Diff")), _ => t("Language").to_owned() };
        let mut content = v_flex().flex_1().min_w_0().px_5().pt_3().pb_4().gap_5().child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(title));
        content = match draft.section {
            3 => content.child(gpui_kit::component::checkbox::Checkbox::new("file-preview-setting")
                .label(t("Enable preview tab")).checked(draft.file_preview)
                .on_click(cx.listener(|s, checked, _, cx| { if let Some(d) = &mut s.settings_draft { d.file_preview = *checked; } cx.notify(); })))
                .child(div().text_color(rgb(MUTED)).child(t("Select a file to preview it. Double-click or press Enter to keep it open."))),
            1 => {
                if let Some(git) = &draft.git { content.child(git.clone()) }
                else if draft.loading { content.child(t("Loading settings…")) }
                else { content.child(t(if self.welcome { "Open a repository to configure Git" } else { "Git settings are unavailable during an operation" })) }
            }
            2 => content.child(t("Open file differences in")).child(v_flex().gap_3()
                .children([(false, "Editor tab (default)"), (true, "Bottom preview")].into_iter().map(|(bottom, label)| {
                    gpui_kit::component::radio::Radio::new(if bottom { "diff-location-bottom" } else { "diff-location-tab" }).label(t(label)).checked(draft.bottom_preview == bottom)
                        .on_change(cx.listener(move |s, _, _, cx| { if let Some(d) = &mut s.settings_draft { d.bottom_preview = bottom; } cx.notify(); }))
                }))),
            _ => content.child(v_flex().gap_3().children([
                ("language-system", Language::System, t("Follow system")),
                ("language-chinese", Language::Chinese, "中文"),
                ("language-english", Language::English, "English"),
            ].into_iter().map(|(id, language, label)| gpui_kit::component::radio::Radio::new(id).label(label).checked(draft.language == language)
                .on_change(cx.listener(move |s, _, _, cx| { if let Some(d) = &mut s.settings_draft { d.language = language; } cx.notify(); }))))),
        };
        v_flex().size_full().bg(rgb(PANEL))
            .child(h_flex().flex_1().min_h_0().items_stretch().child(navigation).child(div().w(px(1.)).bg(rgb(BORDER))).child(content))
            .when_some(draft.error.clone(), |v, error| v.child(div().px_4().text_color(rgb(0xf2a6a6)).child(error)))
            .child(h_flex().h(px(50.)).px_4().gap_3().items_center().justify_end().border_t_1().border_color(rgb(BORDER))
                .child(Button::new("settings-ok").primary().min_w(px(72.)).label(t("OK")).disabled(draft.loading || self.fetch_token.is_some())
                    .on_click(cx.listener(|s, _, w, cx| { s.apply_settings(true, w, cx); if !s.settings_open { w.close_dialog(cx); } })))
                .child(Button::new("close-settings").min_w(px(72.)).label(t("Cancel")).on_click(cx.listener(|s, _, w, cx| { s.close_settings(cx); w.close_dialog(cx); })))
                .child(Button::new("settings-apply").min_w(px(72.)).label(t("Apply")).disabled(!modified || draft.loading || self.fetch_token.is_some())
                    .on_click(cx.listener(|s, _, w, cx| s.apply_settings(false, w, cx)))))
    }
    fn compare_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids = self.commit_ids(); let selected = self.log_selection.ordered(&ids);
        if selected.len() != 2 { return; }
        let before = selected[1].to_owned(); let after = selected[0].to_owned();
        self.close_comparison_preview(cx);
        let view = cx.new(|cx| crate::compare_view::CompareView::new(self.repo.clone(), before, after, window, cx));
        cx.subscribe_in(&view, window, |s, _, event: &crate::compare_view::OpenDiff, w, cx| {
            let index = s.comparison_preview_index(cx).unwrap_or_else(|| {
                let tab = cx.new(|_| { let mut tab=DiffTab::new(event.0.clone(), event.1.clone()); tab.documents=s.documents.clone(); tab });
                s.comparison_preview = Some(tab.downgrade());
                cx.subscribe(&tab, |s, tab, event: &crate::diff_tab::FileChanged, cx| {
                    if s.comparison_preview.as_ref().and_then(|p| p.upgrade()).is_some_and(|p| p.entity_id() == tab.entity_id()) {
                        if let Some(view) = &s.comparison { view.update(cx, |v, cx| v.select_target(&event.0, cx)); }
                    }
                }).detach();
                cx.observe(&tab, |_, _, cx| cx.notify()).detach();
                s.insert_editor_tab(tab.entity_id()); s.tabs.push(tab); s.tabs.len()-1
            });
            s.tabs[index].update(cx, |tab, cx| tab.replace_target(event.0.clone(), event.1.clone(), cx));
            s.activate_diff_tab(index, w, cx);
        }).detach();
        cx.subscribe_in(&view, window, |s, _, event: &crate::compare_view::SelectionChanged, w, cx| {
            let Some(index) = s.comparison_preview_index(cx) else { return; };
            if s.tabs[index].read(cx).target.identity() == event.0.identity() { return; }
            let active = s.active_tab == Some(index);
            s.tabs[index].update(cx, |tab, cx| {
                tab.replace_target(event.0.clone(), event.1.clone(), cx);
                if active { tab.activate_with_focus(false, w, cx); }
            });
        }).detach();
        cx.subscribe_in(&view, window, |s, _, event: &crate::compare_view::Reloaded, w, cx| {
            let Some(index) = s.comparison_preview_index(cx) else { return; };
            let active = s.active_tab == Some(index);
            s.tabs[index].update(cx, |tab, cx| match &event.0 {
                Some((target, chain)) => {
                    tab.replace_target(target.clone(), chain.clone(), cx);
                    if active { tab.activate_with_focus(false, w, cx); }
                }
                None => tab.invalidate_preview("Comparison unavailable. Refresh the Changes panel to retry.", cx),
            });
        }).detach();
        cx.subscribe(&view, |s, _, _: &crate::compare_view::Close, cx| {
            s.close_comparison_preview(cx);
            s.comparison = None; s.comparison_visible = false; cx.notify();
        }).detach();
        self.comparison = Some(view); self.comparison_visible = true;
        self.cancel_project_preview(cx);
        self.local_visible = false; self.project_visible = false; self.stash_open = false; cx.notify();
    }
    fn comparison_preview_index(&self, _cx: &App) -> Option<usize> {
        let preview = self.comparison_preview.as_ref()?.upgrade()?;
        self.tabs.iter().position(|tab| tab.entity_id() == preview.entity_id())
    }
    fn close_comparison_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.comparison_preview_index(cx) { self.close_diff_tab(index, cx); }
        self.comparison_preview = None;
    }
    fn open_local(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        if self.local_view.is_none() {
            let repo = self.repo.clone();
            let view = cx.new(|cx| crate::local_view::LocalView::new(repo, window, cx));
            cx.subscribe_in(&view, window, |s, _, event: &crate::local_view::OpenDiff, w, cx| {
                let index = s.tabs.iter().position(|t| t.read(cx).target.identity() == event.0.identity()).unwrap_or_else(|| {
                    let tab = cx.new(|_| { let mut tab=DiffTab::new(event.0.clone(), event.1.clone()); tab.documents=s.documents.clone(); tab });
                    cx.observe(&tab, |_, _, cx| cx.notify()).detach();
                    s.insert_editor_tab(tab.entity_id()); s.tabs.push(tab); s.tabs.len()-1
                });
                s.activate_diff_tab(index, w, cx);
            }).detach();
            cx.subscribe_in(&view, window, |s, view, event: &crate::local_view::Submit, w, cx| {
                let view=view.clone(); let (push,recover)=(event.0,event.1);
                s.with_saved_documents(w,cx,move|_,w,cx|view.update(cx,|v,cx|v.submit_saved(push,recover,w,cx)));
            }).detach();
            cx.subscribe_in(&view, window, |s, _, event: &crate::local_view::Changed, w, cx| {
                s.refresh_impl(false, true, None, cx);
                if event.push { s.open_push(w, cx); }
            }).detach();
            cx.subscribe(&view, |s, _, _: &crate::local_view::Hide, cx| { s.local_visible = false; cx.notify(); }).detach();
            cx.observe(&view, |_, _, cx| { cx.notify(); }).detach();
            self.local_view = Some(view);
        }
        self.comparison_visible = false; self.local_visible = true; self.project_visible = false; self.stash_open = false; cx.notify();
    }
    fn cancel_project_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = &self.project_view { view.update(cx, |v, _| v.cancel_preview()); }
    }
    fn toggle_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        if self.project_view.is_none() {
            let view = cx.new(|cx| crate::project_view::ProjectView::new(self.repo.path.clone(), cx));
            cx.subscribe_in(&view, window, |s, source, event: &crate::project_view::Open, w, cx| {
                let current = s.project_view.as_ref().is_some_and(|view| view.entity_id() == source.entity_id()
                    && (!event.1 || view.read(cx).is_preview_current(event.2)));
                if s.project_visible && current { s.open_project_file(event.0.clone(), event.1, w, cx); }
            }).detach();
            self.project_view = Some(view);
        }
        self.project_visible = !self.project_visible;
        if self.project_visible { self.comparison_visible = false; self.stash_open = false; self.local_visible = false; }
        cx.notify();
    }
    fn jump_to_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab)=self.active_file_tab.and_then(|i|self.file_tabs.get(i)) {
            if let Some(request)=tab.read(cx).history_source_request(cx){self.jump_history_source(request,window,cx);}
            return;
        }
        let request=if let Some(tab)=self.active_tab.and_then(|i|self.tabs.get(i)){tab.read(cx).source_request(cx)}
            else if let Some(tab)=&self.local_preview{tab.read(cx).source_request(cx)}
            else {self.selected_file.and_then(|i|self.files.get(i)).and_then(|change|{
                let target=if let Some(chain)=&self.aggregate_chain{let mut c=chain.clone();c.select(self.selected_file?)}else{diff_target::Target::new(self.repo.clone(),self.selected.and_then(|i|self.commits.get(i)).cloned(),change.clone()).ok()}?;
                let selection=self.diff.read(cx).selected_lines();let (original,line)=self.diff.read(cx).source_position().map(|p|(p.side()==gpui_kit::component::diff::DiffSide::Original,p.line().saturating_sub(1))).or_else(||selection.map(|r|(r.end_side()==gpui_kit::component::diff::DiffSide::Original,r.end().saturating_sub(1)))
                    ).or(self.preview_source_position).unwrap_or((change.status.starts_with('D'),0));
                Some((target,original,line))
            })};
        let Some((target,original,line))=request else{return;};
        self.jump_target_source(target,original,line,window,cx);
    }
    fn diff_source_callback(&self,cx:&Context<Self>)->crate::diff_tab::SourceCallback {
        let owner=cx.entity().downgrade();
        std::rc::Rc::new(move|(target,original,line),window,cx|{
            let _=owner.update(cx,|s,cx|{
                if s.repo.path==target.identity().repository {window.close_all_dialogs(cx);s.jump_target_source(target,original,line,window,cx);}
            });
        })
    }
    fn jump_target_source(&mut self,target:diff_target::Target,original:bool,line:usize,window:&mut Window,cx:&mut Context<Self>){
        if !original && target.working_tree_document().is_some() && self.documents.get(&target.identity().path).is_some() {
            self.source_token.cancel(); self.source_generation+=1;
            self.open_project_file(target.identity().path.clone(),false,window,cx);
            if let Some(tab)=self.active_file_tab.and_then(|i|self.file_tabs.get(i)) {tab.update(cx,|t,cx|t.jump_to_line(line,cx));}
            return;
        }
        self.source_token.cancel();self.source_token=Cancel::default();self.source_generation+=1;let generation=self.source_generation;
        let token=self.source_token.clone();let root=self.repo.path.clone();let expected_root=root.clone();let path=target.identity().path.clone();
        self.source_task=cx.spawn_in(window,async move|this,wx|{
            let result=wx.background_spawn(async move{
                use gpui_kit::component::diff::{DiffFile,DiffSide};
                let current=match plico::project_files::Project::new(root).map_err(|e|format!("{e:?}"))?.read(&path,&token).map_err(|e|format!("{e:?}"))? {
                    plico::project_files::Content::Text(text)=>text,_=>return Err("Source file cannot be displayed as text".to_owned())};
                let source=if let Some(contents)=target.read_combined(&token){let c=contents?;if original{c.before}else{c.after}.unwrap_or_default()}
                    else {let text=target.read_full(&token)?;let files=if target.change().area==git::Area::Untracked{vec![DiffFile::added(target.change().label(),&text)]}else{DiffFile::parse(&text).map_err(|e|e.to_string())?};
                        files.first().map(|f|f.source(if original{DiffSide::Original}else{DiffSide::Modified}).to_owned()).unwrap_or_default()};
                if token.cancelled(){return Err("Cancelled".into());}
                let mapped=plico::file_history::transfer_range(&source,&current,line..line+1)?.start;
                Ok((path,mapped))
            }).await;
            let _=this.update_in(wx,|s,window,cx|{if s.source_generation!=generation||s.repo.path!=expected_root{return;}
                match result {Ok((path,line))=>{s.open_project_file(path,false,window,cx);if let Some(tab)=s.active_file_tab.and_then(|i|s.file_tabs.get(i)){tab.update(cx,|t,cx|t.jump_to_line(line,cx));}},Err(error)=>{s.error=Some(error);s.fetch_status=Some(t("Could not jump to source").into());}}
                cx.notify();
            });
        });
    }
    fn jump_history_source(&mut self, request:crate::history_view::SourceRequest, window:&mut Window, cx:&mut Context<Self>){
        if request.repository!=self.repo.path {self.error=Some(t("History belongs to another repository; reopen its file").into());cx.notify();return;}
        self.source_token.cancel();self.source_token=Cancel::default();self.source_generation+=1;
        let token=self.source_token.clone();let generation=self.source_generation;let repo=self.repo.clone();let expected_root=repo.path.clone();
        self.source_task=cx.spawn_in(window,async move|this,wx|{
            let result=wx.background_spawn(async move{
                let current=match plico::project_files::Project::new(repo.path.clone()).map_err(|e|format!("{e:?}"))?.read(&request.path,&token).map_err(|e|format!("{e:?}"))?{
                    plico::project_files::Content::Text(text)=>text,_=>return Err("Source file cannot be displayed as text".to_owned())};
                let line=if let Some(revision)=request.revision{let source=plico::file_history::contents(&repo,&revision,&token)?;
                    plico::file_history::transfer_range(&source,&current,request.line..request.line+1)?.start}else{request.line};
                if token.cancelled(){return Err("Cancelled".into());}Ok((request.path,line))
            }).await;
            let _=this.update_in(wx,|s,window,cx|{if generation!=s.source_generation||s.repo.path!=expected_root{return;}
                match result{Ok((path,line))=>{s.open_project_file(path,false,window,cx);window.activate_window();if let Some(tab)=s.active_file_tab.and_then(|i|s.file_tabs.get(i)){tab.update(cx,|t,cx|t.jump_to_line(line,cx));}},Err(e)=>{s.error=Some(e);s.fetch_status=Some(t("Could not jump to source").into());}}
                cx.notify();
            });
        });
    }
    fn new_documents(cx: &mut Context<Self>) -> crate::editing::Documents {
        let documents=crate::editing::Documents::default();
        let owner=cx.entity().downgrade();
        documents.on_saved(move |cx| {
            let _=owner.update(cx, |s,cx| {
                if let Some(local)=&s.local_view {local.update(cx,|v,cx|v.refresh(cx));}
                cx.notify();
            });
        });
        documents
    }
    fn with_saved_documents(&mut self, window: &mut Window, cx: &mut Context<Self>, action: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static) {
        if self.document_action_pending { return; }
        self.document_action_pending = true;
        self.documents.set_locked(true,cx);
        self.documents.save_all(cx);
        let documents = self.documents.all(); // retain drafts until writes finish
        cx.spawn_in(window, async move |this, wx| {
            let mut action = Some(action);
            loop {
                let done = this.update_in(wx, |s,window,cx| {
                    if documents.iter().any(|d|d.read(cx).saving()) { return false; }
                    s.document_action_pending = false;
                    s.documents.set_locked(false,cx);
                    if let Some(error)=documents.iter().find_map(|d|d.read(cx).error.clone()) {
                        s.error=Some(error); s.fetch_status=Some(t("Could not save file").into());
                    } else if !documents.iter().any(|d|d.read(cx).pending(cx)) {
                        if let Some(action)=action.take() { action(s,window,cx); }
                    }
                    cx.notify(); true
                }).unwrap_or(true);
                if done { break; }
                wx.background_executor().timer(std::time::Duration::from_millis(25)).await;
            }
        }).detach();
        cx.notify();
    }
    fn subscribe_file_history(&self, tab: &Entity<crate::project_view::FileTab>, window: &mut Window, cx: &mut Context<Self>) {
        cx.subscribe_in(tab, window, |s, _, event: &crate::project_view::HistoryRequested, window, cx| {
            if let Some(selection)=event.selection.clone() {
                let owner=cx.entity().downgrade();let main_window=window.window_handle();
                let source_callback=std::rc::Rc::new(move|request:crate::history_view::SourceRequest,cx:&mut App|{
                    let _=main_window.update(cx,|_,window,cx|{let _=owner.update(cx,|s,cx|s.jump_history_source(request,window,cx));});
                });
                if let Err(error)=crate::history_view::open_selection(s.repo.clone(),event.path.clone(),selection,source_callback,cx) {s.error=Some(error);cx.notify();}
                return;
            }
            if let Some(index)=s.file_tabs.iter().position(|t|t.read(cx).history&&t.read(cx).path==event.path){s.activate_editor_neighbor(Some(s.file_tabs[index].entity_id()),window,cx);return;}
            let tab=cx.new(|_|crate::project_view::FileTab::history(s.repo.path.clone(),event.path.clone()));
            s.insert_editor_tab(tab.entity_id());s.leave_diff_tab(cx);s.file_tabs.push(tab);s.active_file_tab=Some(s.file_tabs.len()-1);
            s.file_tabs.last().unwrap().update(cx,|t,cx|t.activate(cx));cx.notify();
        }).detach();
    }
    fn open_project_file(&mut self, path: PathBuf, preview: bool, window: &mut Window, cx: &mut Context<Self>) {
        if preview && !self.file_preview { return; }
        if let Some(index) = self.file_tabs.iter().position(|tab| !tab.read(cx).history && tab.read(cx).path == path) {
            if !preview { self.file_tabs[index].update(cx, |tab, _| tab.preview = false); }
            if self.active_file_tab == Some(index) {
                if !preview { self.file_tabs[index].update(cx, |tab, cx| tab.focus_viewer(window, cx)); }
                cx.notify(); return;
            }
            self.leave_diff_tab(cx); self.active_file_tab = Some(index);
            self.file_tabs[index].update(cx, |tab, cx| tab.activate_with_focus(!preview, cx));
        } else {
            let tab = cx.new(|_| { let mut tab = crate::project_view::FileTab::new(self.repo.path.clone(), path); tab.documents = self.documents.clone(); tab.preview = preview; tab });
            self.subscribe_file_history(&tab, window, cx);
            cx.observe(&tab, |_, _, cx| cx.notify()).detach();

            let replacement = self.file_tabs.iter().position(|tab| tab.read(cx).preview && !tab.read(cx).dirty(cx));
            let index = if let Some(index) = replacement {
                let old = self.file_tabs[index].entity_id();
                if preview {
                    for id in &mut self.editor_tab_order { if *id == old { *id = tab.entity_id(); } }
                } else {
                    self.insert_editor_tab(tab.entity_id());
                    self.editor_tab_order.retain(|id| *id != old);
                }
                self.leave_diff_tab(cx);
                self.file_tabs[index] = tab; index
            } else {
                self.insert_editor_tab(tab.entity_id()); self.leave_diff_tab(cx);
                self.file_tabs.push(tab); self.file_tabs.len()-1
            };
            self.active_file_tab = Some(index);
            self.file_tabs[index].update(cx, |tab, cx| tab.activate_with_focus(!preview, cx));
        }
        cx.notify();
    }
    fn leave_diff_tab(&mut self, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        if let Some(index) = self.active_file_tab.take() {
            if let Some(tab) = self.file_tabs.get(index) { tab.update(cx, |t, cx| t.deactivate(cx)); }
        }
        if let Some(index) = self.active_tab.take() {
            if let Some(tab) = self.tabs.get(index) { tab.update(cx, |tab, cx| tab.deactivate(cx)); }
            if self.bottom_preview { if let Some(file) = self.selected_file { self.load_file(file, cx); } }
        }
        cx.notify();
    }
    fn insert_editor_tab(&mut self, id: EntityId) {
        let active = self.active_file_tab.and_then(|i| self.file_tabs.get(i).map(Entity::entity_id))
            .or_else(|| self.active_tab.and_then(|i| self.tabs.get(i).map(Entity::entity_id)));
        let index = active.and_then(|id| self.editor_tab_order.iter().position(|entry| *entry == id)).map_or(0, |i| i+1);
        self.editor_tab_order.insert(index, id);
    }
    fn editor_left_neighbor(&self, id: EntityId) -> Option<EntityId> {
        self.editor_tab_order.iter().position(|entry| *entry == id)
            .and_then(|i| i.checked_sub(1)).map(|i| self.editor_tab_order[i])
    }
    fn activate_editor_neighbor(&mut self, id: Option<EntityId>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = id.and_then(|id| self.file_tabs.iter().position(|tab| tab.entity_id() == id)) {
            self.leave_diff_tab(cx);
            self.active_file_tab = Some(index);
            self.file_tabs[index].update(cx, |tab, cx| tab.activate(cx));
        } else if let Some(index) = id.and_then(|id| self.tabs.iter().position(|tab| tab.entity_id() == id)) {
            self.activate_diff_tab(index, window, cx);
        } else { self.focus_after_diff(window, cx); }
    }
    fn close_file_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        let Some(tab) = self.file_tabs.get(index) else { return; };
        let id = tab.entity_id();
        if tab.read(cx).dirty(cx) {
            self.with_saved_documents(window, cx, move |s,w,cx| {
                if let Some(index) = s.file_tabs.iter().position(|t| t.entity_id()==id) { s.close_file_tab(index,w,cx); }
            }); return;
        }
        let previous = self.editor_left_neighbor(id);
        let was_active = self.active_file_tab == Some(index);
        if was_active { self.leave_diff_tab(cx); }
        self.file_tabs.remove(index);
        self.editor_tab_order.retain(|entry| *entry != id);
        if was_active { self.activate_editor_neighbor(previous, window, cx); }
        else if let Some(active) = self.active_file_tab.as_mut() {
            if *active > index { *active -= 1; }
        }
        cx.notify();
    }
    fn close_diff_tab_from_header(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) {
            let Some(id)=self.tabs.get(index).map(|t|t.entity_id()) else {return;};
            self.with_saved_documents(window,cx,move|s,w,cx| {
                if let Some(index)=s.tabs.iter().position(|t|t.entity_id()==id) {s.close_diff_tab_from_header(index,w,cx);}
            }); return;
        }
        self.cancel_project_preview(cx);
        let Some(tab) = self.tabs.get(index) else { return; };
        let previous = self.editor_left_neighbor(tab.entity_id());
        let was_active = self.active_tab == Some(index);
        self.close_diff_tab(index, cx);
        if was_active { self.activate_editor_neighbor(previous, window, cx); }
    }
    fn activate_diff_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.close_settings(cx);
        self.leave_diff_tab(cx);
        self.reset_job(cx);
        if let Some(tab) = self.tabs.get(index) {
            self.active_tab = Some(index);
            let callback=self.diff_source_callback(cx);
            tab.update(cx, |tab, cx| {tab.source_callback=Some(callback);tab.activate(window, cx);});
        }
    }
    fn focus_after_diff(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab.and_then(|index| self.tabs.get(index)) {
            tab.update(cx, |tab, cx| tab.focus_viewer(window, cx));
        } else if self.stash_open {
            if let Some(view) = &self.stash_view { view.update(cx, |s, cx| s.focus_files(window, cx)); }
        } else { self.history_focus.focus(window, cx); }
    }
    fn close_diff_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() { return; }
        if self.active_tab == Some(index) { self.leave_diff_tab(cx); }
        let removed = self.tabs.remove(index);
        self.editor_tab_order.retain(|id| *id != removed.entity_id());
        if let Some(active) = self.active_tab.as_mut() { if *active > index { *active -= 1; } }
        cx.notify();
    }
    fn restore_stash(&mut self, entry: plico::stash::Entry, action: plico::stash_operation::Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.restore_stash(entry,action,w,cx)); return; }
        if self.fetch_token.is_some() || self.settings_open || self.welcome { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Restoring saved changes…").into());
        let repo = self.repo.clone();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                plico::stash_operation::execute(&repo, &entry, action, false, &token, &Default::default())
            }).await;
            let _ = this.update(wx, |s, cx| {
                s.fetch_token = None;
                use plico::stash_operation::Status;
                let (message, diagnostic) = match result {
                    Ok(report) => {
                        let message = match report.status {
                            Status::Success if report.recovery_error.is_some() => "Saved changes restored; recovery record needs attention",
                            Status::Success => "Saved changes restored",
                            Status::Conflicts => "Saved changes have conflicts; review Local Changes",
                            Status::IndexConflict => "Could not restore the saved index; review Local Changes",
                            Status::LocalChanges => "Local changes would be overwritten; stash kept",
                            Status::UntrackedFiles => "Untracked files would be overwritten; stash kept",
                            Status::Cancelled => "Restore cancelled; check Local Changes before retrying",
                            Status::Failed => "Restore failed or incomplete; review Local Changes",
                        };
                        let diagnostic = if report.status == Status::Success { report.recovery_error } else {
                            Some(match report.command {
                                Ok(output) => format!("{}\n{}", String::from_utf8_lossy(&output.stderr.bytes()), String::from_utf8_lossy(&output.stdout.bytes())).chars().take(8000).collect(),
                                Err(error) => error,
                            })
                        };
                        s.operation_state = report.state.ok();
                        (message, diagnostic)
                    }
                    Err(error) => ("Could not restore saved changes", Some(error)),
                };
                s.fetch_status = Some(t(message).into());
                if let Some(view) = &s.stash_view { view.update(cx, |v, cx| v.result(t(message).into(), diagnostic, cx)); }
                s.refresh(false, cx); cx.notify();
            });
        }); cx.notify();
    }
    fn confirm_external_recovery(&mut self, pending: plico::squash_saved::Pending, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() { return; }
        let owner = cx.entity().downgrade(); let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone(); let path = path.clone(); let pending = pending.clone();
            dialog.title(t("Finish Squash recovery")).w(px(510.)).centered(true).overlay_closable(false)
                .child(div().child(t("Confirm only after handling the saved changes externally. This clears the app recovery record; it does not restore or delete any stash or change your files.")))
                .child(div().truncate().child(path.display().to_string()))
                .footer(gpui_kit::component::dialog::DialogFooter::new()
                    .child(Button::new("acknowledge-recovery-confirm").min_w(px(72.)).label(t("Clear recovery record"))
                        .on_click(move |_, w, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                if s.repo.path == path && s.fetch_token.is_none() { s.acknowledge_external_recovery(pending.clone(), cx); }
                            }); w.close_dialog(cx);
                        }))
                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.primary().min_w(px(72.)).label(t("Cancel")))))
                // The dialog-level Enter action is cancellation; only the explicit button clears a record.
                .on_ok(|_, _, _| true)
        });
    }
    fn acknowledge_external_recovery(&mut self, pending: plico::squash_saved::Pending, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Clearing recovery record…").into());
        let repo = self.repo.clone(); let path = repo.path.clone();
        self.fetch_task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { plico::squash_saved::acknowledge_external_recovery(&repo, &pending, &token) }).await;
            let _ = this.update(cx, |s, cx| {
                if s.repo.path != path { return; }
                s.fetch_token = None;
                let message = t(if result.is_ok() { "Recovery record cleared; saved changes kept" } else { "Recovery record was not cleared. Refresh and review the repository state." });
                s.fetch_status = Some(message.into());
                if let Some(view) = &s.stash_view { view.update(cx, |v, cx| v.result(message.into(), None, cx)); }
                s.rebase_diagnostic = result.err(); cx.notify();
            });
        }); cx.notify();
    }
    fn open_stashes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        self.close_settings(cx);
        if self.stash_view.is_none() {
            let repo = self.repo.clone();
            let view = cx.new(|cx| crate::stash_view::StashView::new(repo, cx));
            cx.subscribe_in(&view, window, |s, _, event: &crate::stash_view::OpenDiff, w, cx| {
                let index = s.tabs.iter().position(|t| t.read(cx).target.identity() == event.0.identity()).unwrap_or_else(|| {
                    let tab = cx.new(|_| { let mut tab=DiffTab::new(event.0.clone(), event.1.clone()); tab.documents=s.documents.clone(); tab });
                    cx.observe(&tab, |_, _, cx| cx.notify()).detach();
                    s.insert_editor_tab(tab.entity_id()); s.tabs.push(tab); s.tabs.len() - 1
                });
                s.tabs[index].update(cx, |tab, _| tab.stash_origin = Some(event.2.clone()));
                s.activate_diff_tab(index, w, cx);
            }).detach();
            cx.subscribe_in(&view, window, |s, _, event: &crate::stash_view::Restore, w, cx| {
                s.restore_stash(event.0.clone(), event.1, w, cx);
            }).detach();
            cx.subscribe_in(&view, window, |s, _, event: &crate::stash_view::Acknowledge, w, cx| {
                s.confirm_external_recovery(event.0.clone(), w, cx);
            }).detach();
            cx.subscribe_in(&view, window, |s, _, _: &crate::stash_view::ViewLocal, w, cx| {
                s.open_local(w, cx);
            }).detach();
            cx.observe(&view, |_, _, cx| cx.notify()).detach();
            self.stash_view = Some(view);
        }
        self.comparison_visible = false; self.project_visible = false; self.local_visible = false; self.stash_open = true; cx.notify();
    }
    fn open_diff_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(change) = self.selected_file.and_then(|i| self.files.get(i)).cloned() else { return; };
        let commit = if self.local { None } else { self.selected.and_then(|i| self.commits.get(i)).cloned() };
        let aggregate = self.aggregate_chain.clone().and_then(|mut chain| chain.select(self.selected_file?).map(|target| (target, chain)));
        let target = if let Some((target, _)) = &aggregate { target.clone() }
            else { let Ok(target) = diff_target::Target::new(self.repo.clone(), commit, change) else { return; }; target };
        let index = self.tabs.iter().position(|tab| tab.read(cx).target.identity() == target.identity()).unwrap_or_else(|| {
            let chain = aggregate.map(|(_, chain)| chain).or_else(|| self.tabs.iter().find_map(|tab| tab.read(cx).fork_chain(&target)))
                .unwrap_or_else(|| diff_target::Chain::new(&target, &self.files)
                    .expect("Selected file belongs to the current file list"));
            let tab = cx.new(|_| { let mut tab=DiffTab::new(target, chain); tab.documents=self.documents.clone(); tab });
            cx.observe(&tab, |_, _, cx| cx.notify()).detach();
            self.insert_editor_tab(tab.entity_id()); self.tabs.push(tab); self.tabs.len() - 1
        });
        self.activate_diff_tab(index, window, cx);
    }
    fn load_file(&mut self, index: usize, cx: &mut Context<Self>) {
        self.preview_source_position=None;
        self.preview_mode = if layout_preferences::load_diff_unified(self.preview_place()) { DiffMode::Unified } else { DiffMode::Split };
        self.preview_appearance = layout_preferences::load_diff_appearance(self.preview_place());
        self.preview_preference_error = None;
        let Some(change) = self.files.get(index).cloned() else {
            return;
        };
        let (generation, token) = self.reset_job(cx);
        self.selected_file = Some(index);
        self.pending_preview_file = None;
        // Clearing rows while loading can clamp the virtual list back to zero.
        // Reveal the restored selection after the new rows arrive as well.
        if let Some(row) = self
            .file_rows
            .iter()
            .position(|row| matches!(row, FileRow::File(i, _) if *i == index))
        {
            self.files_scroll.scroll_to_item(row, ScrollStrategy::Top);
        }
        if self.aggregate_chain.is_some() { cx.notify(); return; }
        if !self.bottom_preview {
            self.loading_patch = false;
            cx.notify();
            return;
        }
        self.diff.update(cx, |d, cx| d.set_inline_unit(crate::diff_settings::inline_unit(self.preview_appearance.emphasis), cx));
        self.loading_patch = true;
        if self.local {
            self.base = Some(i18n::ComparisonTitle::Label(match change.area {
                Area::Unstaged => "Unstaged · Index → Working tree",
                Area::WorkingTree => "Changes · HEAD → Working tree",
                Area::Staged => "Staged · HEAD → Index",
                Area::Untracked => "Untracked · New file preview",
                Area::Conflict => "Conflict · Unmerged index entries",
                Area::Commit => "Committed change",
            }));
        }
        let repo = self.repo.clone();
        let commit = if self.local {
            None
        } else {
            self.selected.and_then(|i| self.commits.get(i).cloned())
        };
        let target = match diff_target::Target::new(repo, commit, change) {
            Ok(target) => target,
            Err(error) => {
                self.loading_patch = false;
                self.preview_error = Some(error);
                cx.notify();
                return;
            }
        };
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let text = target.read(&token)?;
                    if token.cancelled() {
                        return Err("Cancelled".into());
                    }
                    if target.change().area == Area::Untracked {
                        Ok(vec![DiffFile::added(target.change().label(), &text)])
                    } else {
                        DiffFile::parse(&text).map_err(|e| e.to_string())
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading_patch = false;
                match result {
                    Ok(files) => this.diff.update(cx, |d, cx| {
                        d.set_mode(crate::diff_settings::effective_mode(&files, this.preview_mode), cx);
                        d.set_files(files, cx);
                    }),
                    Err(e) => this.preview_error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn move_commit(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.local || self.history_loading || self.commits.is_empty() {
            return;
        }
        let ids = self.commit_ids();
        self.log_selection.step(&ids, delta, false);
        let lead = self.log_selection.lead(&ids);
        self.load_log_selection(cx);
        if let Some(i) = lead { self.history_scroll.scroll_to_item(i, ScrollStrategy::Top); }
    }
    fn extend_commit(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.local || self.history_loading { return; }
        let ids = self.commit_ids();
        self.log_selection.step(&ids, delta, true);
        let lead = self.log_selection.lead(&ids);
        self.load_log_selection(cx);
        if let Some(i) = lead { self.history_scroll.scroll_to_item(i, ScrollStrategy::Top); }
    }

    fn prune_directory_state(&mut self) {
        file_tree::retain_collapsed(
            &mut self.collapsed_directories,
            self.files.iter().map(|f| (f.area, f.path.as_path())),
        );
    }
    fn rebuild_files(&mut self) {
        let mut rows = Vec::new();
        if self.local {
            for area in [
                Area::Conflict,
                Area::Unstaged,
                Area::Staged,
                Area::Untracked,
            ] {
                let count = self.files.iter().filter(|f| f.area == area).count();
                if count == 0 {
                    continue;
                }
                let expanded = !self.collapsed_files.contains(&area);
                rows.push(FileRow::Group(area, count, expanded));
                if expanded {
                    rows.extend(self.directory_rows(area));
                }
            }
        } else {
            rows.extend(self.directory_rows(Area::Commit));
        }
        self.file_rows = Arc::new(rows);
    }
    fn directory_rows(&self, area: Area) -> Vec<FileRow> {
        file_tree::rows(
            self.files
                .iter()
                .enumerate()
                .filter(|(_, f)| f.area == area)
                .map(|(i, f)| (i, f.path.as_path())),
            |path| {
                self.collapsed_directories
                    .contains(&(area, path.to_path_buf()))
            },
        )
        .into_iter()
        .map(|row| match row {
            file_tree::Row::File { index, depth } => FileRow::File(index, depth),
            file_tree::Row::Folder {
                path,
                depth,
                count,
                expanded,
            } => FileRow::Folder(area, path, depth, count, expanded),
        })
        .collect()
    }
    fn move_file(&mut self, delta: isize, cx: &mut Context<Self>) {
        let visible: Vec<_> = self
            .file_rows
            .iter()
            .enumerate()
            .filter_map(|(row, item)| {
                if let FileRow::File(i, _) = item {
                    Some((row, *i))
                } else {
                    None
                }
            })
            .collect();
        if visible.is_empty() {
            return;
        }
        let position = visible
            .iter()
            .position(|(_, i)| Some(*i) == self.selected_file);
        let next = position
            .map(|p| (p as isize + delta).clamp(0, visible.len() as isize - 1) as usize)
            .unwrap_or(if delta < 0 { visible.len() - 1 } else { 0 });
        let (row, index) = visible[next];
        self.load_file(index, cx);
        self.files_scroll.scroll_to_item(row, ScrollStrategy::Top);
    }
    fn apply_text_filter(&mut self, cx: &mut Context<Self>) {
        let text = self.filter_search.read(cx).value().to_string();
        let authors = self.user_menu.read(cx).authors();
        let paths = self.paths_menu.read(cx).paths();
        self.text_filter = if text.trim().is_empty() && self.date_filter.is_empty() && authors.is_none() && paths.is_empty() {
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
    fn open_push(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.welcome { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        let repo = self.repo.clone();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let worker_repo = repo.clone();
            let result = wx.background_spawn(async move { worker_repo.push_snapshot(&token) }).await;
            let _ = wx.update(|window, cx| { let _ = this.update(cx, |s, cx| {
                let cancelled = s.fetch_token.take().is_some_and(|t| t.cancelled());
                if cancelled { cx.notify(); return; }
                match result {
                    Ok(Some(snapshot)) => {
                        let draft = cx.new(|cx| crate::push_dialog::PushDialog::new(repo.clone(), snapshot, window, cx));
                        let callback=s.diff_source_callback(cx);draft.update(cx,|s,_|s.source_callback=Some(callback));
                        cx.observe_in(&draft, window, |_, _, w, _| w.refresh()).detach();
                        let owner = cx.entity().downgrade(); let path = repo.path.clone();
                        window.open_dialog(cx, move |dialog, _, cx| {
                            let owner = owner.clone(); let confirm = draft.clone(); let path = path.clone();
                            dialog.title(h_flex().gap_2().items_center().child(gpui_kit::img("branding/plico.svg").size(px(16.))).child(div().font_weight(FontWeight::NORMAL).child(format!("{} {}", t("Push Commits to"), repo.path.file_name().unwrap_or_default().to_string_lossy())))).w(px(800.)).h(px(540.)).pb_0().centered(true).overlay_closable(false)
                                .content({ let draft = draft.clone(); move |body, _, _| body.p_0().min_h_0().overflow_hidden().child(draft.clone()) })
                                .footer({
                                    let normal_owner = owner.clone(); let normal_draft = draft.clone(); let normal_path = path.clone();
                                    let force_owner = owner.clone(); let force_draft = draft.clone(); let force_path = path.clone();
                                    let can_force = draft.read(cx).spec().is_some_and(|s| plico::push::force_allowed(&s.target));
                                    let tags_draft = draft.clone(); let mode_draft = draft.clone();
                                    DialogFooter::new().items_center().gap(px(12.)).mx(px(-16.)).px(px(12.)).py(px(8.)).border_t_1().border_color(rgb(0x26282c))
                                        .child(Button::new("push-help").ghost().label("?").accessibility_label(t("Push help")).size(px(24.)).rounded(px(12.)).border_1().border_color(rgb(0x40434a)).mr(px(8.)).on_click(|_,w,cx| {
                                            w.open_dialog(cx,|d,_,_|d.title(t("Push")).w(px(460.)).child(t("Push selected commits to the target branch. Enable Push tags to include tags. Force Push checks the last known remote revision before replacing history.")).footer(DialogFooter::new().child(gpui_kit::component::dialog::DialogClose::new().trigger(|b|b.label(t("Close"))))));
                                        }))
                                        .child(h_flex().items_center().gap(px(8.))
                                        .child(gpui_kit::component::checkbox::Checkbox::new("push-tags").label(t("Push tags:"))
                                            .checked(draft.read(cx).push_tags).on_click(move|checked,_,cx|tags_draft.update(cx,|d,cx|{d.push_tags=*checked;cx.notify();})))
                                        .child(Button::new("push-tags-mode").w(px(132.)).h(px(28.)).with_size(px(28.)).dropdown_caret(true).rounded(px(4.)).label(t(match draft.read(cx).tag_mode { plico::push::TagMode::All=>"All",plico::push::TagMode::CurrentBranch=>"Current Branch" }))
                                            .border_color(rgb(if draft.read(cx).push_tags {0x40434a}else{0x33353b})).bg(rgb(if draft.read(cx).push_tags {0x26282c}else{0x191a1c})).text_color(rgb(if draft.read(cx).push_tags {FG}else{0x4c4f56})).disabled(!draft.read(cx).push_tags).dropdown_menu(move|mut menu,_,_|{
                                                for (mode,label) in [(plico::push::TagMode::All,"All"),(plico::push::TagMode::CurrentBranch,"Current Branch")] {
                                                    let draft=mode_draft.clone();menu=menu.item(PopupMenuItem::new(t(label)).on_click(move|_,_,cx|draft.update(cx,|d,cx|{d.tag_mode=mode;cx.notify();})));
                                                }menu
                                            })))
                                        .child(div().flex_1())
                                        .child(DropdownButton::new("push-actions").with_size(px(28.)).separator_color(rgba(0xffffff80).into())
                                            .button(Button::new("push-confirm").primary().h(px(28.)).min_w(px(72.)).font_weight(FontWeight::SEMIBOLD).label(t("Push"))
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
                                        .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.label(t("Cancel")).min_w(px(72.)).h(px(28.)).with_size(px(28.)).rounded(px(4.))))
                                })
                                .on_ok(move |_, window, cx| {
                                    let Some(spec) = confirm.read(cx).spec() else { return false; };
                                    let _ = owner.update(cx, |s, cx| { if s.repo.path == path { s.start_push(spec, window, cx); } }); true
                                })
                        });
                    }
                    Ok(None) => s.fetch_status = Some(t("Check out a local branch with commits before pushing").into()),
                    Err(error) => {
                        // A repository/configuration failure is not a detached HEAD.
                        // Keep the diagnostic available through the shared Details UI.
                        s.fetch_status = None;
                        s.error = Some(error);
                    }
                }
                cx.notify();
            }); });
        }); cx.notify();
    }
    fn open_project_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.settings_open || self.welcome { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Preparing project update…").into());
        let repo = self.repo.clone(); let path = repo.path.clone();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                let request = plico::project_update::prepare(&repo, &token)?;
                let preferences = plico::project_update_preferences::Preferences::for_repository(&repo, &token)?;
                Ok::<_, String>((request, preferences))
            }).await;
            let _ = wx.update(|window, cx| { let _ = this.update(cx, |s, cx| {
                let cancelled = s.fetch_token.take().is_some_and(|t| t.cancelled());
                s.fetch_status = None;
                if cancelled || s.repo.path != path { cx.notify(); return; }
                match result {
                    Ok((request, preferences)) => {
                        if !preferences.show_options {
                            s.start_project_update(request, preferences.method, window, cx); return;
                        }
                        let draft = cx.new(|_| crate::project_update_dialog::Options::new(request, preferences));
                        cx.observe_in(&draft, window, |_, _, w, _| w.refresh()).detach();
                        let owner = cx.entity().downgrade();
                        window.open_dialog(cx, move |dialog, _, cx| {
                            let owner = owner.clone(); let draft = draft.clone(); let path = path.clone();
                            let checkbox_draft = draft.clone();
                            dialog.title(t("Update Project")).w(px(560.)).centered(true).overlay_closable(false)
                                .child(draft.clone())
                                .footer(gpui_kit::component::dialog::DialogFooter::new()
                                    .child(gpui_kit::component::checkbox::Checkbox::new("hide-update-options")
                                        .label(t("Don't show again")).checked(!draft.read(cx).show_options)
                                        .on_click(move |checked, _, cx| checkbox_draft.update(cx, |d, cx| { d.show_options = !*checked; cx.notify(); })))
                                    .child(div().flex_1())
                                    .child(gpui_kit::component::dialog::DialogAction::new().child(Button::new("update-project-ok").primary().min_w(px(72.)).label(t("OK"))))
                                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.label(t("Cancel")).min_w(px(72.)))))
                                .on_ok(move |_, window, cx| {
                                    owner.update(cx, |s, cx| {
                                        if s.repo.path != path || s.fetch_token.is_some() { return false; }
                                        if !draft.update(cx, |d, cx| d.save(cx)) { return false; }
                                        let method = draft.read(cx).method;
                                        let request = draft.update(cx, |d, _| d.request.take());
                                        if let Some(request) = request { s.start_project_update(request, method, window, cx); }
                                        true
                                    }).unwrap_or(true)
                                })
                        });
                    }
                    Err(error) => { s.error = Some(error); s.fetch_status = Some(t("Cannot update project").into()); }
                }
                cx.notify();
            }); });
        }); cx.notify();
    }
    fn start_project_update(&mut self, request: plico::merge_update::Request, method: plico::update_method::Method, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_project_update(request,method,w,cx)); return; }
        if self.fetch_token.is_some() { return; }
        let Ok(exe) = std::env::current_exe() else { return; };
        let helper = exe.with_file_name(if cfg!(windows) { "gitui-askpass.exe" } else { "gitui-askpass" });
        if !helper.is_file() { self.fetch_status = Some(t("Authentication helper is unavailable").into()); cx.notify(); return; }
        let editor_helper = exe.with_file_name(if cfg!(windows) { "gitui-editor.exe" } else { "gitui-editor" });
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Updating project…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone(); let repo = self.repo.clone();
        let (mut choices, choice_ui) = plico::update_decision::channel(token.clone());
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = (|| -> Result<_, String> {
                let editor = plico::editor::command(&editor_helper, false).map_err(|e| e.to_string())?;
                use plico::push_flow::Decisions as _;
                let choices = std::cell::RefCell::new(&mut choices);
                plico::project_update::execute(&repo, &request, method, &editor, &token, &Default::default(),
                    Some((&worker_requests, &helper)), plico::published_commits::Protection::Defaults,
                    |problem| choices.borrow_mut().rebase_over_merge(problem),
                    |problem| choices.borrow_mut().published(problem))
            })();
            let _ = tx.send(result);
        }).detach();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
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
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("Update worker ended".into())),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = wx.update(|w, cx| {
                        if login.as_ref().is_some_and(|d| d.read(cx).active()) || editor.as_ref().is_some_and(|d| d.read(cx).active()) || decision.as_ref().is_some_and(|d| d.read(cx).active()) { w.close_dialog(cx); }
                        let _ = this.update(cx, |s, cx| {
                            let cancelled = s.fetch_token.take().is_some_and(|t| t.cancelled());
                            s.saved_changes_available |= result.as_ref().ok().is_some_and(|r| r.saved_oid().is_some());
                            let message = if cancelled || result.as_ref().is_ok_and(|r| r.result() == plico::update_session::UpdateResult::Cancelled) {
                                "Update cancelled; review local changes and saved stash"
                            } else if result.as_ref().is_ok_and(|r| r.complete()) {
                                if result.as_ref().is_ok_and(|r| r.result() == plico::update_session::UpdateResult::NothingToUpdate) {
                                    "Already up to date"
                                } else { "Project updated" }
                            } else { "Update incomplete; review conflicts and saved local changes" };
                            if let Err(error) = result { s.error = Some(error); }
                            s.fetch_status = Some(t(message).into());
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            s.refresh_impl(false, true, None, cx); cx.notify();
                        });
                    }); break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
        }); cx.notify();
    }
    fn confirm_force_push(&mut self, spec: plico::push::Spec, window: &mut Window, cx: &mut Context<Self>) {
        if !plico::push::force_allowed(&spec.target) || self.fetch_token.is_some() { return; }
        if layout_preferences::silent_force_push(&self.repo.path,&spec.target.remote,&spec.target.branch) {
            window.close_dialog(cx); self.start_push(spec,window,cx); return;
        }
        let owner = cx.entity().downgrade(); let path = self.repo.path.clone();
        let remember = cx.new(|_| false);
        cx.observe_in(&remember,window,|_,_,w,_|w.refresh()).detach();
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
                    if s.repo.path != path || s.fetch_token.is_some() { return false; }
                    if *saved.read(cx) {
                        if let Err(error)=layout_preferences::remember_force_push(&path,&spec.target.remote,&spec.target.branch) {
                            crate::error_ui::show_details(error.to_string(),"Settings could not be saved",w,cx);return false;
                        }
                    }
                    w.close_all_dialogs(cx); s.start_push(plico::push::Spec { tags: spec.tags, force: true, source_branch: spec.source_branch.clone(), source_oid: spec.source_oid.clone(), target: spec.target.clone(), set_upstream: spec.set_upstream }, w, cx); true
                }).unwrap_or(true))
        });
    }
    fn start_push(&mut self, spec: plico::push::Spec, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_push(spec,w,cx)); return; }
        if self.fetch_token.is_some() { return; }
        let Ok(exe) = std::env::current_exe() else { return; };
        let helper = exe.with_file_name(if cfg!(windows) { "gitui-askpass.exe" } else { "gitui-askpass" });
        if !helper.is_file() { self.fetch_status = Some(t("Authentication helper is unavailable").into()); cx.notify(); return; }
        let editor_helper = exe.with_file_name(if cfg!(windows) { "gitui-editor.exe" } else { "gitui-editor" });
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Pushing…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone(); let repo = self.repo.clone();
        let (mut choices, choice_ui) = plico::update_decision::channel(token.clone());
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = (|| -> Result<_, String> {
                let editor = plico::editor::command(&editor_helper, false).map_err(|e| e.to_string())?;
                let (path, preferences) = plico::update_preferences::for_repository(&repo, &token)?;
                choices.set_preferences(path, preferences);
                let mut report = plico::push_flow::execute(&repo, spec, &editor, &token, &Default::default(),
                    Some((&worker_requests, &helper)), preferences, plico::published_commits::Protection::Defaults, &mut choices);
                report.preferences_error = choices.persistence_error.clone();
                Ok(report)
            })();
            let _ = tx.send(result);
        }).detach();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
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
                            let cancelled = s.fetch_token.take().is_some_and(|t| t.cancelled());
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
                            s.fetch_status = Some(if preferences_error.is_some() {
                                format!("{} · {}", t(message), t("Settings could not be saved"))
                            } else { t(message).into() });
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            s.refresh_impl(false, true, None, cx); cx.notify();
                        });
                    }); break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
        }); cx.notify();
    }
    fn open_pull(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.welcome { return; }
        let token = Cancel::default();
        self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Loading pull targets…").into());
        let repo = self.repo.clone();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                Ok::<_, String>((repo.pull_targets(&token)?, repo.editor_directory(&token)?, plico::pull::source(&repo, &token)?))
            }).await;
            let _ = wx.update(|window, cx| { let _ = this.update(cx, |s, cx| {
                let cancelled = s.fetch_token.take().is_some_and(|token| token.cancelled());
                s.fetch_status = None;
                if !cancelled {
                    match result {
                        Ok((targets, directory, source)) => s.show_pull(targets, directory, source, window, cx),
                        Err(_) => s.fetch_status = Some(t("Could not read pull targets").into()),
                    }
                }
                cx.notify();
            }); });
        });
        cx.notify();
    }
    fn show_pull(&mut self, targets: plico::pull::Targets, directory: PathBuf, source: Option<plico::pull::Source>, window: &mut Window, cx: &mut Context<Self>) {
        let draft = cx.new(|cx| crate::pull_dialog::PullDialog::new(targets, source.is_some(), window, cx));
        cx.observe_in(&draft, window, |_, _, w, _| w.refresh()).detach();
        let owner = cx.entity().downgrade();
        let repo_path = self.repo.path.clone();
        let focus = draft.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let draft_confirm = draft.clone(); let owner = owner.clone();
            let directory = directory.clone(); let repo_path = repo_path.clone(); let source = source.clone();
            dialog.title(t("Pull")).w(px(620.)).overlay_closable(false)
                .centered(true)
                .child(draft.clone())
                .footer(h_flex().w_full().gap_2()
                    .child(draft.update(cx, |s, cx| s.options_button(cx)))
                    .child(div().flex_1())
                    .child(crate::dialog_style::confirmation(Button::new("pull-confirm").primary().label(t("Pull")).disabled(draft.read(cx).selection(cx).is_none()))))
                .on_ok(move |_, window, cx| {
                    let Some((target, options)) = draft_confirm.read(cx).selection(cx) else { return false; };
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == repo_path { s.start_pull(target, options, directory.clone(), source.clone(), window, cx); }
                    });
                    true
                })
        });
        focus.update(cx, |s, cx| s.focus(window, cx));
    }
    fn start_pull(&mut self, target: plico::pull::Target, options: plico::pull::Options,
        directory: PathBuf, source: Option<plico::pull::Source>, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_pull(target,options,directory,source,w,cx)); return; }
        if self.fetch_token.is_some() { return; }
        let Ok(exe) = std::env::current_exe() else { return; };
        let helper = exe.with_file_name(if cfg!(windows) { "gitui-askpass.exe" } else { "gitui-askpass" });
        let editor = exe.with_file_name(if cfg!(windows) { "gitui-editor.exe" } else { "gitui-editor" });
        let (Ok(commit_editor), Ok(sequence_editor)) = (plico::editor::command(&editor, false), plico::editor::command(&editor, true)) else {
            self.fetch_status = Some(t("Git editor helper is unavailable").into()); cx.notify(); return;
        };
        if !helper.is_file() { self.fetch_status = Some(t("Authentication helper is unavailable").into()); cx.notify(); return; }
        let spec = plico::pull::CommandSpec { target, options, commit_editor, sequence_editor: Some(sequence_editor) };
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Pulling…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone(); let repo = self.repo.clone();
        enum PullResult {
            Command(plico::pull::CommandResult),
            Rebase(Box<plico::rebase_update::Report>),
        }
        let (choices, choice_ui) = plico::update_decision::channel(token.clone());
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = (|| -> Result<PullResult, String> {
                if !spec.options.contains(plico::pull::OptionFlag::Rebase) {
                    return plico::pull::execute_command(&repo, &spec, &token, &Default::default(), Some((&worker_requests, &helper))).map(PullResult::Command);
                }
                let source = source.ok_or("Explicit Rebase requires an attached branch")?;
                let request = plico::merge_update::Request {
                    branch: source.branch, head: source.head, target: spec.target, reinstate_index: true,
                };
                use plico::{push_flow::Decisions, update_method::Method};
                let choices = std::cell::RefCell::new(choices);
                plico::rebase_update::execute_selected(&repo, &request, &spec.commit_editor,
                    &token, &Default::default(), Some((&worker_requests, &helper)),
                    plico::published_commits::Protection::Defaults,
                    |problem| if matches!(problem, Ok(false)) { Ok(Some(Method::Rebase)) }
                        else { choices.borrow_mut().rebase_over_merge(problem) },
                    |problem| choices.borrow_mut().published(problem))
                    .map(|report| PullResult::Rebase(Box::new(report)))
            })();
            let _ = tx.send(result);
        }).detach();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            let mut decision: Option<Entity<crate::update_dialog::UpdateDialog>> = None;
            loop {
                if let Some(pending) = choice_ui.take_pending() {
                    if let Ok(dialog) = wx.update(|w, cx| crate::update_dialog::show(pending, w, cx)) { decision = Some(dialog); }
                }
                if let Some(pending) = requests.take_pending() {
                    use plico::auth_request::Kind;
                    if matches!(pending.kind, Kind::CommitEditor | Kind::SequenceEditor) {
                        let dir = directory.clone();
                        let document = wx.background_spawn(async move { plico::editor::Document::open(pending, &dir) }).await;
                        if let Ok(document) = document {
                            if let Ok(dialog) = wx.update(|w, cx| crate::editor_dialog::show(document, w, cx)) { editor = Some(dialog); }
                        }
                    } else if let Ok(dialog) = wx.update(|w, cx| crate::auth_dialog::show(pending, w, cx)) { login = Some(dialog); }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("Pull worker ended".into())),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = wx.update(|w, cx| {
                        if login.as_ref().is_some_and(|d| d.read(cx).active()) || editor.as_ref().is_some_and(|d| d.read(cx).active()) || decision.as_ref().is_some_and(|d| d.read(cx).active()) { w.close_dialog(cx); }
                        let _ = this.update(cx, |s, cx| {
                            let cancelled = s.fetch_token.take().is_some_and(|token| token.cancelled());
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
                            s.fetch_status = Some(t(message).into());
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
    fn confirm_abort_merge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() { return; }
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone(); let path = path.clone();
            dialog.title(t("Abort Merge")).w(px(460.)).overlay_closable(false)
                .child(div().child(t("Abort the merge in this repository?")))
                .child(div().truncate().child(path.display().to_string()))
                .footer(crate::dialog_style::confirmation(Button::new("abort-merge-confirm").primary().label(t("Abort"))))
                .on_ok(move |_, window, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path { s.start_merge_operation(plico::merge_operation::Action::Abort, window, cx); }
                    });
                    true
                })
        });
    }
    fn open_checkout_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.settings_open || self.welcome || !self.snapshot.as_ref().is_some_and(|s| s.head_id.is_some()) { return; }
        let Some(snapshot) = &self.snapshot else { return; };
        let input = cx.new(|cx| crate::revision_input::RevisionInput::new(snapshot, window, cx));
        let focus = input.clone();
        let owner = cx.entity().downgrade(); let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let input = input.clone(); let confirm = input.clone();
            let owner = owner.clone(); let path = path.clone();
            dialog.title(t("Checkout Tag or Revision")).w(px(480.)).overlay_closable(false)
                .centered(true)
                .child(input.clone())
                .footer(crate::dialog_style::confirmation(Button::new("checkout-input-ok").primary().label(t("OK"))))
                .on_ok(move |_, window, cx| {
                    let text = confirm.read(cx).value(cx);
                    if text.is_empty() { return false; }
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path { s.resolve_checkout_input(text, window, cx); }
                    }); true
                })
        });
        focus.update(cx, |s, cx| s.focus(window, cx));
    }
    fn resolve_checkout_input(&mut self, input: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Resolving revision…").into());
        let repo = self.repo.clone();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move { plico::checkout::resolve_input(&repo, &input, &token) }).await;
            let _ = wx.update(|window, cx| { let _ = this.update(cx, |s, cx| {
                let cancelled = s.fetch_token.take().is_some_and(|t| t.cancelled());
                if cancelled { s.fetch_status = Some(t("Checkout cancelled; repository refreshed").into()); }
                else { match result {
                    Ok(resolved) => s.start_checkout(resolved.target, resolved.expected_head, window, cx),
                    Err(error) => s.fetch_status = Some(format!("{}: {}", t("Could not resolve revision"), error)),
                } }
                cx.notify();
            }); });
        }); cx.notify();
    }
    fn dismiss_repository_child(&mut self, cx: &mut Context<Self>) {
        self.repository_child_selection=None;
        if let Some(menu)=self.repository_child_menu.take() {
            // An action can close its parent while PopupMenu is itself being
            // updated. Defer the event to avoid re-borrowing that entity.
            cx.defer(move |cx| {
                if let Some(menu)=menu.upgrade() { menu.update(cx,|_,cx|cx.emit(DismissEvent)); }
            });
        }
    }
    fn dismiss_repository_menu(&mut self, cx: &mut Context<Self>) {
        self.dismiss_repository_child(cx);
        self.repository_menu_open = false;
        self.repository_pointer=None;
        self.repository_search_task=Task::ready(());
        self.repository_search=None;
        self.repository_selection=None;
        if let Some(token) = self.repository_recent_token.take() { token.cancel(); }
        self.repository_recent_generation += 1;
        self.repository_recent_task = Task::ready(());
    }
    fn load_repository_recents(&mut self, cx: &mut Context<Self>) {
        if let Some(token) = self.repository_recent_token.take() { token.cancel(); }
        self.repository_recent_generation += 1;
        let generation = self.repository_recent_generation;
        let Some(snapshot) = &self.snapshot else { return; };
        let branches = snapshot.branches.clone(); let repo = self.repo.clone();
        let load_favorites = !self.repository_favorites_saving;
        let path = repo.path.clone(); let token = Cancel::default();
        self.repository_recent_token = Some(token.clone());
        self.repository_recent_task = cx.spawn(async move |this,cx| {
            let result = cx.background_spawn(async move {
                let favorites = load_favorites.then(|| plico::favorite_preferences::for_repository(&repo,&token));
                (plico::branch_popup::recent_branches(&repo,&branches,None,&token), favorites)
            }).await;
            let _ = this.update(cx, |s,cx| {
                if s.repository_recent_generation != generation || s.repo.path != path || !s.repository_menu_open { return; }
                s.repository_recent_token = None;
                if let Some(favorites) = result.1 { match favorites {
                    Ok((path,prefs)) => { s.repository_favorites_path = Some(path); s.repository_favorites = prefs; s.repository_favorites_error = None; }
                    Err(error) => { s.repository_favorites_path = None; s.repository_favorites_error = Some(error); }
                } }
                match result.0 {
                    Ok(recent) => {
                        for name in &recent {
                            if let Some(name) = name.strip_prefix("refs/heads/") {
                                s.repository_expansion.reveal(plico::branch_popup::Section::Recent, name, true);
                            }
                        }
                        s.repository_recent = recent;
                    }
                    Err(_) => { s.repository_recent.clear(); }
                }
                s.apply_repository_search(cx);
            });
        });
    }
    fn toggle_repository_favorite(&mut self, reference: String, cx: &mut Context<Self>) {
        if self.repository_favorites_saving || self.repository_recent_token.is_some() { return; }
        let Some(file) = self.repository_favorites_path.clone() else { return; };
        let previous = self.repository_favorites.clone();
        if !self.repository_favorites.toggle(&reference) { return; }
        let next = self.repository_favorites.clone(); let epoch = self.repository_favorites_epoch;
        self.repository_favorites_saving = true; self.repository_favorites_error = None;
        cx.spawn(async move |this,cx| {
            let result = cx.background_spawn(async move { plico::favorite_preferences::save(&file,&next) }).await;
            let _ = this.update(cx,|s,cx| {
                if s.repository_favorites_epoch != epoch { return; }
                s.repository_favorites_saving = false;
                if let Err(error) = result { s.repository_favorites = previous; s.repository_favorites_error = Some(error.to_string()); }
                cx.notify();
            });
        }).detach(); cx.notify();
    }
    fn apply_repository_search(&mut self, cx: &mut Context<Self>) {
        use plico::{branch_popup,branch_search};
        let value=self.repository_search_input.read(cx).value();
        self.repository_applied_query=value.to_string();
        self.repository_search=branch_search::Search::new(branch_search::popup_pattern(value.as_str()),&Default::default(),cfg!(target_os="linux"));
        let Some(snapshot)=&self.snapshot else { return; };
        let current=snapshot.current_branch.as_ref().map(|b|format!("refs/heads/{b}"));
        let mut groups=branch_popup::groups(&snapshot.branches,&snapshot.tags,current.as_deref(),&self.repository_favorites.present(&snapshot.branches,&snapshot.tags),&self.repository_recent,true,None);
        let result=branch_search::apply_to_popup(self.repository_search.as_ref(),&mut groups,&[("commit",t("Commit…")), ("revision",t("Checkout Tag or Revision…"))],true,current.as_deref(),&mut self.repository_expansion,true);
        self.repository_actions=result.actions;
        self.repository_selection=result.selected;
        if let Some(branch_search::Selection::Reference(section,name))=&self.repository_selection {
            let rows=branch_popup::visible_rows(&groups,&self.repository_expansion,true);
            if let Some(index)=rows.iter().position(|row| row.section==*section && matches!(&row.node,branch_popup::VisibleNode::Entry(branch_popup::Node::Ref {reference,..}) if reference.full_name==name)) {
                self.repository_scroll.scroll_to_item(index,ScrollStrategy::Top);
            }
        }
        cx.notify();
    }
    fn repository_groups(&self) -> Vec<plico::branch_popup::Group<'_>> {
        let Some(snapshot)=&self.snapshot else { return Vec::new(); };
        let current=snapshot.current_branch.as_ref().map(|b|format!("refs/heads/{b}"));
        let mut groups=plico::branch_popup::groups(&snapshot.branches,&snapshot.tags,current.as_deref(),&self.repository_favorites.present(&snapshot.branches,&snapshot.tags),&self.repository_recent,true,None);
        if let Some(search)=&self.repository_search { search.filter_groups(&mut groups); }
        groups
    }
    fn repository_branch_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let head = self.snapshot.as_ref().and_then(|s| s.head_id.as_ref());
        let label = self.snapshot.as_ref().and_then(|s| s.current_branch.clone())
            .or_else(|| self.snapshot.as_ref().and_then(|s| s.tags.iter().find(|tag| Some(&tag.id) == head).map(|tag| tag.name.trim_start_matches("refs/tags/").to_owned())))
            .unwrap_or_else(|| head.map(|id| format!("{} ({})", &id[..8], t("detached HEAD"))).unwrap_or_else(|| t("Branch").into()));
        gpui_kit::component::popover::Popover::new("repository-branch-popup").appearance(false)
            .overlay_closable(false)
            .open(self.repository_menu_open)
            .trigger(titlebar_menu_button("repository-branch-menu").ghost()
                .disabled(self.fetch_token.is_some() || self.settings_open || head.is_none())
                .child(h_flex().items_center().gap_1().child(jb_icon(if self.operation_state.as_ref().is_some_and(|state| state.in_progress()) {
                    "warning_dark.svg"
                } else if self.snapshot.as_ref().is_some_and(|snapshot| snapshot.current_branch.is_none() && snapshot.head_id.is_some()) {
                    if self.snapshot.as_ref().is_some_and(|snapshot| snapshot.tags.iter().any(|tag| Some(&tag.id) == snapshot.head_id.as_ref())) { "branchLabel_dark.svg" } else { "warning_dark.svg" }
                } else { "vcs_dark.svg" }).size(px(16.)))
                    .child(div().max_w(px(220.)).truncate().text_size(px(13.)).font_weight(FontWeight::NORMAL).child(label)).child(jb_icon("chevronDown_dark.svg"))))
            .on_open_change(cx.listener(|s, open, window, cx| {
                s.repository_menu_open = *open;
                if *open {
                    s.repository_search_editing = false;
                    s.repository_search_input.update(cx,|input,cx| { input.set_value("",window,cx); input.focus(window,cx); });
                    s.apply_repository_search(cx);
                    s.load_repository_recents(cx);
                } else { s.dismiss_repository_menu(cx); }
                if *open && s.repository_expansion.expanded.is_empty() {
                    if let Some(name) = s.snapshot.as_ref().and_then(|s| s.current_branch.as_ref()) {
                        s.repository_expansion.reveal(plico::branch_popup::Section::Local, name, true);
                    }
                }
                cx.notify();
            }))
            .content(move |_, _, cx| owner.update(cx, |s,cx| s.repository_menu_body(cx)).unwrap_or_else(|_| div().into_any_element()))
            .map(titlebar_menu)
    }
    fn repository_pointer_selection(&mut self, event: &MouseMoveEvent, selection: plico::branch_search::Selection, window: &mut Window, cx: &mut Context<Self>) {
        let previous=self.repository_pointer.replace(event.position);
        if previous.is_none() || previous==Some(event.position) || window.is_a11y_active() { return; }
        if self.repository_selection.as_ref()!=Some(&selection) {
            self.repository_selection=Some(selection);
            cx.notify();
        }
    }
    fn repository_visible_move(&mut self, direction: plico::branch_navigation::Move, cx: &mut Context<Self>) {
        use plico::branch_navigation;
        let groups=self.repository_groups();
        let rows=plico::branch_popup::visible_rows(&groups,&self.repository_expansion,true);
        let actions=self.repository_actions.iter().map(String::as_str).collect::<Vec<_>>();
        if let Some(next)=branch_navigation::navigate_visible(&rows,&actions,self.repository_selection.as_ref(),direction,false) {
            if let Some(index)=rows.iter().position(|row|branch_navigation::row_matches(row,&next)) {
                self.repository_scroll.scroll_to_item(index,ScrollStrategy::Top);
            }
            self.repository_selection=Some(next);
            cx.notify();
        }
    }
    fn activate_repository_action(&mut self, action: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.settings_open || self.welcome { return; }
        self.dismiss_repository_menu(cx);
        match action {
            "commit" => {
                self.open_local(window, cx);
                if let Some(view) = &self.local_view {
                    view.update(cx, |view,cx| view.focus_message(window,cx));
                }
            }
            "revision" => self.open_checkout_input(window,cx),
            _ => {},
        }
        cx.notify();
    }
    fn repository_activate_selection(&mut self, final_choice: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.repository_search_input.focus_handle(cx).is_focused(window) { cx.propagate(); return; }
        self.repository_search_editing = false;
        self.repository_search_task=Task::ready(());
        if self.repository_search_input.read(cx).value().as_str()!=self.repository_applied_query {
            self.apply_repository_search(cx);
        }
        match self.repository_selection.clone() {
            Some(plico::branch_search::Selection::Folder(key)) => {
                self.dismiss_repository_child(cx);
                if final_choice { self.repository_expansion.toggle(key); }
                else if !self.repository_expansion.expanded.contains(&key) { self.repository_expansion.expanded.insert(key); }
                else { self.repository_visible_move(plico::branch_navigation::Move::Next,cx); }
            }
            Some(plico::branch_search::Selection::Action(id)) if final_choice => {
                self.activate_repository_action(&id, window, cx);
            }
            Some(selection @ plico::branch_search::Selection::Reference(..)) => {
                if self.repository_child_selection.as_ref()!=Some(&selection) {
                    self.dismiss_repository_child(cx);
                    self.repository_child_selection=Some(selection);
                }
            }
            None if !final_choice => self.repository_visible_move(plico::branch_navigation::Move::First,cx),
            _ => {},
        }
        window.prevent_default(); cx.stop_propagation(); cx.notify();
    }
    fn repository_tree_left(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use plico::{branch_navigation,branch_search::Selection};
        if !self.repository_search_input.focus_handle(cx).is_focused(window) { cx.propagate(); return; }
        self.repository_search_editing = false;
        self.repository_search_task=Task::ready(());
        if self.repository_search_input.read(cx).value().as_str()!=self.repository_applied_query { self.apply_repository_search(cx); }
        self.dismiss_repository_child(cx);
        let groups=self.repository_groups();
        let rows=plico::branch_popup::visible_rows(&groups,&self.repository_expansion,true);
        let index=self.repository_selection.as_ref().and_then(|selected|rows.iter().position(|row|branch_navigation::row_matches(row,selected)));
        if let Some(index)=index {
            let row=&rows[index];
            if row.expanded {
                if let Selection::Folder(key)=branch_navigation::row_selection(row) { self.repository_expansion.expanded.remove(&key); }
            } else if let Some(parent)=rows[..index].iter().rposition(|r|r.depth<row.depth) {
                self.repository_selection=Some(branch_navigation::row_selection(&rows[parent]));
                self.repository_scroll.scroll_to_item(parent,ScrollStrategy::Top);
            } else if let Some(previous)=rows[..index].iter().rposition(|r|r.expanded) {
                self.repository_selection=Some(branch_navigation::row_selection(&rows[previous]));
                self.repository_scroll.scroll_to_item(previous,ScrollStrategy::Top);
            } else if let Some(action) = self.repository_actions.last() {
                self.repository_selection=Some(Selection::Action(action.clone()));
            }
            cx.notify();
        } else { self.repository_visible_move(branch_navigation::Move::First,cx); }
        window.prevent_default(); cx.stop_propagation();
    }
    fn repository_search_move(&mut self, direction: plico::branch_navigation::Move, window: &mut Window, cx: &mut Context<Self>) {
        use plico::branch_navigation;
        // Input bindings consume navigation before raw key-down dispatch.
        // Capture navigation while this input owns the branch tree.
        if !self.repository_search_input.focus_handle(cx).is_focused(window) { cx.propagate(); return; }
        // A pending query must be applied before navigating, not overwrite this
        // selection when its debounce fires afterwards.
        self.repository_search_editing = false;
        self.repository_search_task=Task::ready(());
        if self.repository_search_input.read(cx).value().as_str()!=self.repository_applied_query {
            self.apply_repository_search(cx);
        }
        let actions=self.repository_actions.iter().map(String::as_str).collect::<Vec<_>>();
        let groups=self.repository_groups();
        let next=if self.repository_search.is_some() {
            branch_navigation::navigate(&groups,&actions,self.repository_selection.as_ref(),direction,true,false)
        } else {
            let rows=plico::branch_popup::visible_rows(&groups,&self.repository_expansion,true);
            branch_navigation::navigate_visible(&rows,&actions,self.repository_selection.as_ref(),direction,false)
        };
        if let Some(next)=next {
            self.dismiss_repository_child(cx);
            if let plico::branch_search::Selection::Reference(section,name)=&next {
                if let Some(reference)=self.repository_groups().into_iter().filter(|g|g.section==*section).flat_map(|g|g.references).find(|r|r.full_name==name) {
                    let short=reference.name.to_owned();
                    self.repository_expansion.reveal(*section,&short,true);
                }
            }
            self.repository_selection=Some(next);
            let groups=self.repository_groups();
            let rows=plico::branch_popup::visible_rows(&groups,&self.repository_expansion,true);
            if let Some(selected)=&self.repository_selection {
                if let Some(index)=rows.iter().position(|row|branch_navigation::row_matches(row,selected)) {
                    self.repository_scroll.scroll_to_item(index,ScrollStrategy::Top);
                }
            }
            cx.notify();
        }
        window.prevent_default(); cx.stop_propagation();
    }
    fn repository_search_label(&self, label: &str) -> AnyElement {
        let ranges=self.repository_search.as_ref().and_then(|search|search.find(label))
            .map(|found|plico::branch_search::byte_ranges(label,&found.fragments)).unwrap_or_default();
        StyledText::new(label.to_owned()).with_highlights(ranges.into_iter().map(|range| (range,HighlightStyle {
            color:Some(rgb(0x000000).into()), background_color:Some(rgba(0xffd042b3).into()), ..Default::default()
        }))).into_any_element()
    }
    fn repository_menu_body(&self, cx: &mut Context<Self>) -> AnyElement {
        use plico::branch_popup::{self, VisibleNode, Node, FolderKey, Section};
        let count=branch_popup::visible_rows(&self.repository_groups(),&self.repository_expansion,true).len();
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
            .child(div().on_mouse_down(MouseButton::Left, cx.listener(|s, _, _, cx| {
                s.repository_search_editing = true; cx.notify();
            })).child(Input::new(&self.repository_search_input).prefix(jb_icon("search_dark.svg"))))
            .children(self.repository_actions.iter().map(|id| {
                let action = id.clone();
                let hovered_action = id.clone();
                let label = t(if id == "commit" { "Commit…" } else { "Checkout Tag or Revision…" });
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
                let Some(snapshot) = &s.snapshot else { return Vec::new(); };
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
                    let full_ref = reference.full_name.to_owned(); let target = reference.id.to_owned();
                    let head = snapshot.head_id.clone(); let current = reference.current;
                    let local = full_ref.starts_with("refs/heads/"); let tag = full_ref.starts_with("refs/tags/");
                    let owner = cx.entity().downgrade();
                    let section = row.section;
                    let identity=plico::branch_search::Selection::Reference(section,full_ref.clone());
                    let hover_identity=identity.clone();
                    let child_open=s.repository_child_selection.as_ref()==Some(&identity);
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
                            if !current && (local || tag) {
                                let owner = owner.clone(); let reference = full_ref.clone(); let target = target.clone(); let head = head.clone();
                                menu = menu.item(PopupMenuItem::new(t("Checkout")).on_click(move |_,w,cx| {
                                    let _ = owner.update(cx, |s,cx| {
                                        s.dismiss_repository_menu(cx);
                                        if let Some(head) = &head { if local { s.start_local_checkout(reference.clone(),target.clone(),head.clone(),w,cx); }
                                            else { s.start_checkout(target.clone(),head.clone(),w,cx); } }
                                        cx.notify();
                                    });
                                }));
                            }
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
                        }))).into_any_element()
                })).collect::<Vec<_>>()
            })).track_scroll(&self.repository_scroll).h(px(count.min(12) as f32 * 28.))))
            .into_any_element()
    }
    fn start_local_checkout(&mut self, reference: String, target: String, head: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_local_checkout(reference,target,head,w,cx)); return; }
        if self.fetch_token.is_some() || self.welcome { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Checking out branch…").into());
        let repo = self.repo.clone();
        self.fetch_task = cx.spawn_in(window, async move |this,wx| {
            let checked_ref = reference.clone();
            let checked_target = target.clone();
            let recovery_head = head.clone();
            let result = wx.background_spawn(async move {
                plico::checkout::execute_local(&repo,&reference,&target,&head,&token,&Default::default())
            }).await;
            let _ = wx.update(|window,cx| { let _ = this.update(cx,|s,cx| {
                use plico::checkout::Status;
                let cancelled = s.fetch_token.take().is_some_and(|token| token.cancelled());
                let status = result.as_ref().map(|r| r.status(&checked_ref,&checked_target));
                let message = if cancelled { "Checkout cancelled; repository refreshed" } else { match status {
                    Ok(Status::Success) => "Branch checked out",
                    Ok(Status::Cancelled) => "Checkout cancelled; repository refreshed",
                    Ok(Status::TimedOut) => "Checkout timed out; repository refreshed",
                    Ok(Status::LocalChanges) => "Local changes would be overwritten by checkout",
                    Ok(Status::UntrackedFiles) => "Untracked files would be overwritten by checkout",
                    Ok(Status::Unmerged) => "Resolve existing index conflicts before checkout",
                    _ => "Could not check out branch; repository refreshed",
                }};
                if !cancelled && status == Ok(Status::LocalChanges) {
                    let files = result.as_ref().unwrap().affected_files(&checked_ref,&checked_target);
                    s.show_checkout_recovery(checked_target, recovery_head, Some(checked_ref), files, window, cx);
                }
                s.fetch_token.take(); s.operation_state = None; s.fetch_status = Some(t(message).into());
                s.user_menu.update(cx, |menu,cx| menu.refresh(cx)); s.refresh_impl(false,true,None,cx); cx.notify();
            }); });
        }); cx.notify();
    }
    fn start_checkout(&mut self, target: String, expected_head: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_checkout(target,expected_head,w,cx)); return; }
        if self.fetch_token.is_some() || self.welcome { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Checking out commit…").into());
        let repo = self.repo.clone();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let checked_target = target.clone();
            let recovery_head = expected_head.clone();
            let result = wx.background_spawn(async move {
                plico::checkout::execute(&repo, &target, &expected_head, &token, &Default::default())
            }).await;
            let _ = wx.update(|window, cx| { let _ = this.update(cx, |s, cx| {
                let cancelled = s.fetch_token.take().is_some_and(|token| token.cancelled());
                let message = if cancelled { "Checkout cancelled; repository refreshed" }
                    else {
                        use plico::checkout::Status;
                        match result.as_ref().map(|result| result.status(&checked_target)) {
                            Ok(Status::Success) => "Commit checked out (detached HEAD)",
                            Ok(Status::Cancelled) => "Checkout cancelled; repository refreshed",
                            Ok(Status::TimedOut) => "Checkout timed out; repository refreshed",
                            Ok(Status::Unmerged) => "Resolve existing index conflicts before checkout",
                            Ok(Status::LocalChanges) => "Local changes would be overwritten by checkout",
                            Ok(Status::UntrackedFiles) => "Untracked files would be overwritten by checkout",
                            _ => "Could not check out commit; repository refreshed",
                        }
                    };
                let recovery = if !cancelled { result.as_ref().ok().filter(|r|
                    r.status(&checked_target) == plico::checkout::Status::LocalChanges)
                    .map(|r| r.affected_files(&checked_target)) } else { None };
                if let Some(files) = recovery { s.show_checkout_recovery(checked_target.clone(), recovery_head, None, files, window, cx); }
                s.operation_state = None;
                s.fetch_status = Some(t(message).into());
                s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                s.refresh_impl(false, true, None, cx); cx.notify();
            }); });
        }); cx.notify();
    }
    fn show_checkout_recovery(&mut self, target: String, head: String, reference: Option<String>, files: plico::checkout_files::Files,
        window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade(); let path = self.repo.path.clone();
        let cancel_focus = cx.focus_handle();
        let force_focus = cx.focus_handle();
        let initial_focus = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) { cancel_focus.focus(window, cx); }
            let mut footer = h_flex().w_full().gap_2();
            for smart in [false, true] {
                let owner = owner.clone(); let target = target.clone(); let head = head.clone(); let path = path.clone();
                let reference = reference.clone();
                let button = Button::new(if smart { "smart-checkout" } else { "force-checkout" })
                    .when(smart, |button| button.primary())
                    .when(!smart, |button| button.focus_handle(force_focus.clone()))
                    .min_w(px(72.))
                    .label(t(if smart { "Smart Checkout" } else { "Force Checkout" }))
                    .tooltip(t(if smart { "Save local changes, check out, then restore them" } else { "Check out and overwrite local changes" }))
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        let _ = owner.update(cx, |s, cx| { if s.repo.path == path {
                            s.start_checkout_recovery(target.clone(), head.clone(), reference.clone(), smart, window, cx);
                        } });
                    });
                footer = footer.child(button);
                if !smart { footer = footer.child(div().flex_1()); }
            }
            footer = footer.child(Button::new("cancel-checkout").focus_handle(cancel_focus.clone()).min_w(px(72.)).label(t("Don't Checkout"))
                .on_click(|_, window, cx| window.close_dialog(cx)));
            let owner = owner.clone(); let target = target.clone(); let head = head.clone();
            let reference = reference.clone(); let path = path.clone();
            let cancel_focus = cancel_focus.clone(); let force_focus = force_focus.clone();
            dialog.title(t("Checkout Problem")).w(px(640.)).centered(true).overlay_closable(false)
                .child(div().child(t("Local changes would be overwritten. Smart Checkout saves and restores them.")))
                .child(div().id("checkout-affected-files").h(px(220.)).overflow_y_scroll()
                    .children(files.paths.iter().map(|path| div().child(path.clone()))))
                .when(!files.complete, |d| d.child(t("The affected file list may be incomplete")))
                .footer(footer).on_ok(move |_, window, cx| {
                    if cancel_focus.is_focused(window) { return true; }
                    let smart = !force_focus.is_focused(window);
                    let _ = owner.update(cx, |s, cx| { if s.repo.path == path {
                        s.start_checkout_recovery(target.clone(), head.clone(), reference.clone(), smart, window, cx);
                    } });
                    true
                })
        });
    }
    fn start_checkout_recovery(&mut self, target: String, head: String, reference: Option<String>, smart: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_checkout_recovery(target,head,reference,smart,w,cx)); return; }
        if self.fetch_token.is_some() || self.welcome { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t(if reference.is_some() { "Checking out branch…" } else { "Checking out commit…" }).into()); let repo = self.repo.clone();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let (message, saved) = wx.background_spawn(async move {
                if let Some(reference) = reference {
                    use plico::checkout;
                    if !smart {
                        let result = checkout::force_execute_local(&repo, &reference, &target, &head, &token, &Default::default());
                        return (if result.as_ref().is_ok_and(|r| r.checked_out(&reference, &target)) { "Branch checked out" }
                            else { "Could not check out branch; repository refreshed" }, None);
                    }
                    return match checkout::smart_execute_local(&repo, &reference, &target, &head, true, &token, &Default::default()) {
                        Ok(result) => {
                            let restored = result.restore.as_ref().is_ok_and(|r| r.as_ref().is_none_or(|out| out.status.success() && out.stopped.is_none()));
                            let checked = result.checkout.as_ref().is_some_and(|r| r.as_ref().is_ok_and(|r| r.checked_out(&reference, &target)));
                            let clean = result.state.as_ref().is_ok_and(|s| s.conflicts.is_empty());
                            let message = if !restored || result.saved_oid.is_some() || !clean { "Local changes were not fully restored; review conflicts and saved stash" }
                                else if checked && result.head.as_ref().is_ok_and(|id| id == &target)
                                    && result.branch.as_ref().is_ok_and(|name| name == &reference) { "Branch checked out" }
                                else { "Could not check out branch; repository refreshed" };
                            (message, result.saved_oid)
                        }
                        Err(_) => ("Could not check out branch; repository refreshed", None),
                    };
                }
                if !smart {
                    let result = plico::checkout::force_execute(&repo, &target, &head, &token, &Default::default());
                    return (if result.as_ref().is_ok_and(|r| r.checked_out(&target)) { "Commit checked out (detached HEAD)" }
                        else { "Could not check out commit; repository refreshed" }, None);
                }
                match plico::checkout::smart_execute(&repo, &target, &head, true, &token, &Default::default()) {
                    Ok(result) => {
                        let restored = result.restore.as_ref().is_ok_and(|r| r.as_ref().is_none_or(|out| out.status.success() && out.stopped.is_none()));
                        let checked = result.checkout.as_ref().is_some_and(|r| r.as_ref().is_ok_and(|r| r.checked_out(&target)));
                        let clean_state = result.state.as_ref().is_ok_and(|s| s.conflicts.is_empty());
                        let message = if !restored || result.saved_oid.is_some() || !clean_state { "Local changes were not fully restored; review conflicts and saved stash" }
                            else if checked && result.head.as_ref().is_ok_and(|id| id == &target) { "Commit checked out; local changes restored" }
                            else { "Could not check out commit; repository refreshed" };
                        (message, result.saved_oid)
                    }
                    Err(_) => ("Could not check out commit; repository refreshed", None),
                }
            }).await;
            let _ = wx.update(|_, cx| { let _ = this.update(cx, |s, cx| {
                s.fetch_token.take(); s.operation_state = None;
                s.saved_changes_available |= saved.is_some();
                s.fetch_status = Some(match saved { Some(oid) => format!("{} (stash: {})", t(message), oid), None => t(message).into() });
                s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                s.refresh_impl(false, true, None, cx); cx.notify();
            }); });
        }); cx.notify();
    }
    fn confirm_abort_rebase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() { return; }
        let owner = cx.entity().downgrade(); let path = self.repo.path.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone(); let path = path.clone();
            dialog.title(t("Abort Rebase")).w(px(460.)).centered(true).overlay_closable(false)
                .child(div().child(t("Abort the rebase in this repository?")))
                .child(div().truncate().child(path.display().to_string()))
                .footer(crate::dialog_style::confirmation(Button::new("abort-rebase-confirm").primary().label(t("Abort"))))
                .on_ok(move |_, window, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if s.repo.path == path { s.start_rebase_operation(true, window, cx); }
                    }); true
                })
        });
    }
    fn prepare_squash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.settings_open || self.welcome { return; }
        let ids = self.commit_ids(); let selected = self.log_selection.ordered(&ids).into_iter().map(str::to_owned).collect::<Vec<_>>();
        if selected.len() < 2 { return; }
        let Some(head) = self.snapshot.as_ref().and_then(|s| s.head_id.clone()) else { return; };
        let repo = self.repo.clone(); let path = repo.path.clone();
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Preparing Squash…").into()); self.rebase_diagnostic = None;
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                use plico::{squash_check, published_commits::Protection};
                let prepared = squash_check::prepare(&repo, &head, &selected, Protection::Defaults, &token)?;
                if prepared.state.in_progress() || !prepared.state.conflicts.is_empty() {
                    return Err("Finish the current Git operation before starting Squash".to_owned());
                }
                let draft = squash_check::message(&repo, &selected, &token)?;
                Ok((std::sync::Arc::new(prepared), draft))
            }).await;
            let _ = wx.update(|window, cx| { let _ = this.update(cx, |s, cx| {
                let cancelled = s.fetch_token.take().is_some_and(|t| t.cancelled());
                if s.repo.path != path || cancelled { cx.notify(); return; }
                match result {
                    Ok((prepared, draft)) => {
                        s.fetch_status = None;
                        let owner = cx.entity().downgrade(); let path = path.clone();
                        crate::squash_dialog::show(s.repo.clone(), prepared.clone(), draft, window, cx, move |message, window, cx| {
                            let _ = owner.update(cx, |s, cx| { if s.repo.path == path {
                                s.start_squash(prepared.clone(), message, window, cx);
                            } });
                        });
                    }
                    Err(error) => { s.fetch_status = Some(t("Could not squash commits; repository refreshed").into()); s.rebase_diagnostic = Some(error); }
                }
                cx.notify();
            }); });
        }); cx.notify();
    }
    fn start_squash(&mut self, prepared: std::sync::Arc<plico::squash_check::Prepared>, message: String,
        window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_squash(prepared,message,w,cx)); return; }
        if self.fetch_token.is_some() || self.welcome { return; }
        let Ok(exe) = std::env::current_exe() else { return; };
        let helper = exe.with_file_name(if cfg!(windows) { "gitui-editor.exe" } else { "gitui-editor" });
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Squashing commits…").into()); self.rebase_diagnostic = None; self.squash_undo = None;
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone(); let repo = self.repo.clone(); let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = plico::squash_operation::execute(&repo, &prepared, &message, &helper,
                plico::published_commits::Protection::Defaults, &token, &Default::default(), Some(&worker_requests));
            let undo = result.as_ref().ok().filter(|r| r.succeeded())
                .map(|report| plico::squash_undo::Undo::capture(&repo, report, &Cancel::default()).map(Arc::new));
            let _ = tx.send((result, undo));
        }).detach();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            let directory = wx.background_spawn(async move { directory_repo.editor_directory(&Cancel::default()) }).await;
            loop {
                if let Some(pending) = requests.take_pending() {
                    if let Ok(dir) = &directory {
                        let dir = dir.clone();
                        if let Ok(document) = wx.background_spawn(async move { plico::editor::Document::open(pending, &dir) }).await {
                            if let Ok(dialog) = wx.update(|w, cx| crate::editor_dialog::show(document, w, cx)) { editor = Some(dialog); }
                        }
                    }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some((Err("Squash worker ended".into()), None)),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some((result, undo)) = result {
                    let _ = wx.update(|w, cx| {
                        if editor.as_ref().is_some_and(|d| d.read(cx).active()) { w.close_dialog(cx); }
                        let _ = this.update(cx, |s, cx| {
                            s.fetch_token.take();
                            let success = result.as_ref().is_ok_and(|r| r.succeeded());
                            s.rebase_diagnostic = match result {
                                Err(error) => Some(error),
                                Ok(report) => {
                                    let mut notes = Vec::new();
                                    if let Some(error) = report.editor_error { notes.push(error); }
                                    if let Some(error) = report.recovery_error { notes.push(error); }
                                    if let Some(oid) = report.saved_oid { notes.push(format!("Saved local changes: {oid}")); }
                                    if let Some(Err(error)) = report.restore { notes.push(error); }
                                    let diagnostic = plico::rebase_operation::diagnostic(&Ok(plico::rebase_operation::ResultState {
                                        squash_undo: None, command: report.command, state: report.state, recovery_error: None }));
                                    if let Some(error) = diagnostic { notes.push(error); }
                                    (!notes.is_empty()).then(|| notes.join("\n"))
                                }
                            };
                            let focus = undo.as_ref().and_then(|r| r.as_ref().ok()).and_then(|u| u.commit_to_focus()).map(str::to_owned);
                            match undo {
                                Some(Ok(undo)) => s.squash_undo = Some(undo),
                                Some(Err(error)) => s.rebase_diagnostic = Some(error),
                                None => {},
                            }
                            s.fetch_status = Some(t(if success { "Commits squashed" } else { "Could not squash commits; repository refreshed" }).into());
                            s.operation_state = None;
                            s.user_menu.update(cx, |m, cx| m.refresh(cx));
                            if let Some(target) = focus { s.navigate_commit_impl(t("Commits squashed").into(), target, false, true, w, cx); }
                            else { s.refresh_impl(false, true, None, cx); }
                            cx.notify();
                        });
                    }); break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
        }); cx.notify();
    }
    fn prepare_reset(&mut self, target: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.document_action_pending || self.welcome { return; }
        let repo=self.repo.clone(); let token=Cancel::default(); self.fetch_token=Some(token.clone());
        self.fetch_status=Some(t("Checking…").into()); self.rebase_diagnostic=None;
        self.fetch_task=cx.spawn_in(window,async move|this,wx| {
            let result=wx.background_spawn(async move {plico::reset_operation::Prepared::prepare(&repo,target,&token)}).await;
            let _=wx.update(|w,cx| {let _=this.update(cx,|s,cx| {
                let cancelled=s.fetch_token.take().is_some_and(|t|t.cancelled());s.fetch_status=None;
                if cancelled { cx.notify(); return; }
                match result {
                    Err(e)=>{s.rebase_diagnostic=Some(e);s.fetch_status=Some(t("Reset failed").into());},
                    Ok(prepared)=>{
                        let prepared=Arc::new(prepared); let owner=cx.entity().downgrade();
                        crate::reset_dialog::show(prepared.clone(),w,cx,move|mode,w,cx| {
                            let _=owner.update(cx,|s,cx|s.start_reset(prepared.clone(),mode,w,cx));
                        });
                    }
                }
                cx.notify();
            });});
        });cx.notify();
    }
    fn start_reset(&mut self, prepared: Arc<plico::reset_operation::Prepared>, mode: plico::reset_operation::Mode, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending {return;}
        if self.documents.pending(cx) {self.with_saved_documents(window,cx,move|s,w,cx|s.start_reset(prepared,mode,w,cx));return;}
        if self.fetch_token.is_some() || self.welcome {return;}
        let repo=self.repo.clone();let token=Cancel::default();self.fetch_token=Some(token.clone());
        self.reset_running=true;
        self.fetch_status=Some(t("Resetting… This operation cannot be cancelled once started.").into());self.rebase_diagnostic=None;
        self.fetch_task=cx.spawn_in(window,async move|this,wx| {
            let result=wx.background_spawn(async move {prepared.execute(&repo,mode,&token,&Default::default())}).await;
            let _=wx.update(|w,cx| {let _=this.update(cx,|s,cx| {
                s.fetch_token.take();s.reset_running=false;
                let success=result.as_ref().is_ok_and(|r|r.command.as_ref().is_ok_and(|o|o.status.success() && o.stopped.is_none()) && r.recovery_error.is_none() && r.state.is_ok());
                s.rebase_diagnostic=plico::rebase_operation::diagnostic(&result);
                s.fetch_status=Some(t(if success {"Reset successful"} else {"Reset failed"}).into());
                s.squash_undo=None;s.operation_state=None;
                for doc in s.documents.all() {doc.update(cx,|d,cx|{if !d.pending(cx){d.reload(w,cx);}});}
                if let Some(tree)=&s.project_view { tree.update(cx,|v,cx|v.refresh(cx)); }
                s.user_menu.update(cx,|m,cx|m.refresh(cx));s.refresh_impl(false,true,None,cx);cx.notify();
            });});
        });cx.notify();
    }
    fn undo_squash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.undo_squash(w,cx)); return; }
        if self.fetch_token.is_some() || self.welcome { return; }
        let Some(undo) = self.squash_undo.clone() else { return; };
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Undoing Squash…").into()); self.rebase_diagnostic = None;
        let repo = self.repo.clone(); let target = undo.commit_to_focus_on_undo().to_owned();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                undo.execute(&repo, plico::published_commits::Protection::Defaults, &token, &Default::default())
            }).await;
            let _ = wx.update(|w, cx| { let _ = this.update(cx, |s, cx| {
                s.fetch_token.take();
                let success = result.as_ref().is_ok_and(|r| r.succeeded());
                s.rebase_diagnostic = match result {
                    Err(error) => { s.squash_undo = None; Some(error) },
                    Ok(report) => plico::rebase_operation::diagnostic(&Ok(plico::rebase_operation::ResultState {
                        squash_undo: None, command: report.command, state: report.state, recovery_error: report.head.err() })),
                };
                s.fetch_status = Some(t(if success { "Squash undone" } else { "Could not undo Squash" }).into());
                s.operation_state = None;
                s.user_menu.update(cx, |m, cx| m.refresh(cx));
                if success {
                    s.squash_undo = None;
                    s.navigate_commit_impl(t("Squash undone").into(), target, false, true, w, cx);
                } else { s.refresh_impl(false, true, None, cx); }
                cx.notify();
            }); });
        }); cx.notify();
    }
    fn start_rebase_operation(&mut self, abort: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_rebase_operation(abort,w,cx)); return; }
        if self.fetch_token.is_some() || self.welcome { return; }
        self.rebase_diagnostic = None;
        let editor_command = if abort { None } else {
            let command = std::env::current_exe().and_then(|exe| plico::editor::command(
                &exe.with_file_name(if cfg!(windows) { "gitui-editor.exe" } else { "gitui-editor" }), false));
            match command {
                Ok(command) => Some(command),
                Err(_) => { self.fetch_status = Some(t("Git editor helper is unavailable").into()); cx.notify(); return; }
            }
        };
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Updating rebase state…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone(); let repo = self.repo.clone();
        let directory_repo = repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            use plico::rebase_operation::{self, Action};
            let action = match editor_command.as_ref() {
                Some(editor) => Action::Continue { editor, requests: Some(&worker_requests) },
                None => Action::Abort,
            };
            let _ = tx.send(rebase_operation::execute(&repo, action, &token, &Default::default()));
        }).detach();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let mut editor: Option<Entity<crate::editor_dialog::Editor>> = None;
            let directory = wx.background_spawn(async move { directory_repo.editor_directory(&Cancel::default()) }).await;
            loop {
                if let Some(pending) = requests.take_pending() {
                    if let Ok(dir) = &directory {
                        let dir = dir.clone();
                        if let Ok(document) = wx.background_spawn(async move { plico::editor::Document::open(pending, &dir) }).await {
                            if let Ok(dialog) = wx.update(|w, cx| crate::editor_dialog::show(document, w, cx)) { editor = Some(dialog); }
                        }
                    }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("Rebase worker ended".into())),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(mut result) = result {
                    let _ = wx.update(|w, cx| {
                        if editor.as_ref().is_some_and(|d| d.read(cx).active()) { w.close_dialog(cx); }
                        let _ = this.update(cx, |s, cx| {
                            let cancelled = s.fetch_token.take().is_some_and(|token| token.cancelled());
                            s.rebase_diagnostic = plico::rebase_operation::diagnostic(&result);
                            let undo = result.as_mut().ok().and_then(|r| r.squash_undo.take());
                            let focus = undo.as_ref().and_then(|u| u.commit_to_focus()).map(str::to_owned);
                            if let Some(undo) = undo { s.squash_undo = Some(Arc::new(undo)); }
                            let message = if cancelled { "Rebase operation cancelled; repository refreshed" } else {
                                match result {
                                    Ok(result) if result.recovery_error.is_some() => "Rebase operation failed",
                                    Ok(result) => match (result.command, result.state) {
                                        (Err(_), _) if abort => "Rebase operation failed",
                                        (Ok(outcome), _) if abort && (!outcome.status.success() || outcome.stopped.is_some()) => "Rebase operation failed",
                                        (_, Ok(state)) if !state.conflicts.is_empty() => "Rebase requires conflict resolution",
                                        (_, Ok(state)) if state.in_progress() => "Rebase is still in progress",
                                        (Ok(outcome), Ok(_)) if outcome.status.success() && outcome.stopped.is_none() => {
                                            if abort { "Rebase aborted" } else { "Rebase completed" }
                                        }
                                        _ => "Rebase operation failed",
                                    },
                                    Err(_) => "Rebase operation failed",
                                }
                            };
                            s.operation_state = None;
                            s.fetch_status = Some(t(message).into());
                            s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                            if let Some(target) = focus { s.navigate_commit_impl(t("Commits squashed").into(), target, false, true, w, cx); }
                            else { s.refresh_impl(false, true, None, cx); }
                            cx.notify();
                        });
                    }); break;
                }
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
        }); cx.notify();
    }
    fn start_merge_operation(&mut self, action: plico::merge_operation::Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) { self.with_saved_documents(window,cx,move|s,w,cx|s.start_merge_operation(action,w,cx)); return; }
        if self.fetch_token.is_some() || self.welcome { return; }
        let token = Cancel::default(); self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Updating merge state…").into());
        let repo = self.repo.clone();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                plico::merge_operation::execute(&repo, action, &token, &Default::default())
            }).await;
            let _ = wx.update(|_, cx| { let _ = this.update(cx, |s, cx| {
                let cancelled = s.fetch_token.take().is_some_and(|token| token.cancelled());
                let message = if cancelled { "Merge operation cancelled" } else {
                    match result {
                        Ok(result) => match (result.command, result.state) {
                            (Ok(outcome), Ok(state)) if outcome.status.success() && outcome.stopped.is_none() && !state.in_progress() => {
                                match action { plico::merge_operation::Action::Commit => "Merge completed", plico::merge_operation::Action::Abort => "Merge aborted" }
                            }
                            _ => "Merge operation failed",
                        },
                        Err(_) => "Merge operation failed",
                    }
                };
                s.operation_state = None;
                s.fetch_status = Some(t(message).into());
                s.user_menu.update(cx, |menu, cx| menu.refresh(cx));
                s.refresh_impl(false, true, None, cx); cx.notify();
            }); });
        });
        cx.notify();
    }
    fn start_fetch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_token.is_some() || self.welcome { return; }
        let helper = match std::env::current_exe() {
            Ok(path) => path.with_file_name(if cfg!(windows) { "gitui-askpass.exe" } else { "gitui-askpass" }),
            Err(_) => { self.fetch_status = Some(t("Authentication helper is unavailable").into()); cx.notify(); return; }
        };
        if !helper.is_file() { self.fetch_status = Some(t("Authentication helper is unavailable").into()); cx.notify(); return; }
        let token = Cancel::default();
        self.fetch_token = Some(token.clone());
        self.fetch_status = Some(t("Fetching…").into());
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let repo = self.repo.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = repo.fetch_specs(&token).and_then(|specs| {
                plico::fetch::execute_authenticated(&repo, &specs, &token, &Default::default(), &worker_requests, &helper)
            });
            let _ = tx.send(result);
        }).detach();
        self.fetch_task = cx.spawn_in(window, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            loop {
                if let Some(pending) = requests.take_pending() {
                    match wx.update(|window, cx| crate::auth_dialog::show(pending, window, cx)) {
                        Ok(dialog) => login = Some(dialog), Err(_) => break,
                    }
                }
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("Fetch worker ended".into())),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some(result) = result {
                    let _ = wx.update(|window, cx| {
                        if login.as_ref().is_some_and(|dialog| dialog.read(cx).active()) { window.close_dialog(cx); }
                        let _ = this.update(cx, |s, cx| {
                            let cancelled = s.fetch_token.take().is_some_and(|token| token.cancelled());
                            s.fetch_status = Some(if cancelled { t("Fetch cancelled").into() } else {
                                match result {
                                    Ok(result) if result.remotes.is_empty() => t("No remotes configured").into(),
                                    Ok(result) if result.successful() => t("Fetch completed").into(),
                                    Ok(result) => {
                                        let names = result.remotes.iter().filter(|r| r.state != plico::fetch::RemoteState::Succeeded)
                                            .map(|r| r.remote.as_str()).collect::<Vec<_>>().join(", ");
                                        format!("{}: {names}", t("Fetch failed"))
                                    }
                                    Err(_) => t("Fetch failed").into(),
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
                wx.background_executor().timer(std::time::Duration::from_millis(50)).await;
            }
        });
        cx.notify();
    }
    fn main_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let disabled = self.fetch_token.is_some() || self.settings_open;
        titlebar_menu_button("main-menu").ghost()
            .icon(jb_icon("windowsMenu@20x20_dark.svg")).icon_size(px(16.))
            .tooltip(t("Main menu")).accessibility_label(t("Main menu"))
            .dropdown_menu(move |menu, window, cx| {
                let owner = owner.clone();
                menu.submenu("Git", window, cx, move |mut menu, _, _| {
                    for (label, action) in [("Push", 0), ("Update Project", 3), ("Pull", 1), ("Fetch", 2)] {
                        let owner = owner.clone();
                        menu = menu.item(PopupMenuItem::new(t(label)).disabled(disabled)
                            .on_click(move |_, window, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    // Recheck after menu creation: background state can change.
                                    if s.fetch_token.is_some() || s.settings_open { return; }
                                    match action {
                                        0 => s.open_push(window, cx),
                                        1 => s.open_pull(window, cx),
                                        3 => s.open_project_update(window, cx),
                                        _ => s.start_fetch(window, cx),
                                    }
                                });
                            }));
                    }
                    menu
                })
            })
            .map(titlebar_menu)
    }
    fn project_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let name = self.repo.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        titlebar_menu_button("project-menu").ghost().min_w_0().flex_shrink_1().max_w(px(340.))
            .disabled(self.fetch_token.is_some() || self.settings_open)
            .tooltip(self.repo.path.to_string_lossy().trim_start_matches("\\\\?\\").to_owned())
            .accessibility_label(format!("{}: {}", t("Project"), name))
            .child(h_flex().min_w_0().gap_2().items_center()
                .child(project_avatar(&self.repo.path, 20.))
                .child(div().min_w_0().truncate().text_size(px(13.)).font_weight(FontWeight::NORMAL).child(name))
                .child(jb_icon("chevronDown_dark.svg").size(px(10.)).flex_shrink_0()))
            .dropdown_menu(move |mut menu, _, cx| {
                let Some(view) = owner.upgrade() else { return menu; };
                let state = view.read(cx);
                let disabled = state.fetch_token.is_some() || state.settings_open;
                let projects = state.recent_projects.clone();
                let current = state.repo.path.clone();
                let open_owner = owner.clone();
                menu = menu.min_w(px(280.)).max_w(px(360.)).item(PopupMenuItem::new(t("Open…"))
                    .icon(jb_icon("open_dark.svg")).disabled(disabled).on_click(move |_, window, cx| {
                        let _ = open_owner.update(cx, |s,cx| {
                            if s.fetch_token.is_none() && !s.settings_open { s.open(window,cx); }
                        });
                    }));
                let mut seen = HashSet::from([project_identity(&current)]);
                let recent = projects.into_iter().filter(|path| seen.insert(project_identity(path))).collect::<Vec<_>>();
                for (i,path) in std::iter::once(current).chain(recent).enumerate() {
                    if i == 0 { menu = menu.separator().label(t("Open Projects")); }
                    if i == 1 { menu = menu.separator().label(t("Recent Projects")); }
                    let selected = i == 0;
                    let item_path = path.clone();
                    let select_owner = owner.clone();
                    menu = menu.item(PopupMenuItem::element(move |_, _| {
                        h_flex().w_full().min_w_0().gap_2().py(px(4.)).items_start()
                            .child(project_avatar(&item_path, 20.))
                            .child(v_flex().min_w_0().flex_1().font_weight(FontWeight::NORMAL)
                                .child(div().text_size(px(13.)).line_height(px(18.)).truncate()
                                    .child(item_path.file_name().unwrap_or_default().to_string_lossy().into_owned()))
                                .child(div().text_size(px(12.)).line_height(px(16.)).text_color(rgb(MUTED)).truncate()
                                    .child(project_display_path(&item_path))))
                    }).disabled(disabled).on_click(move |_, window, cx| {
                        let _ = select_owner.update(cx, |s,cx| {
                            if !selected && s.fetch_token.is_none() && !s.settings_open { s.choose_project_window(path.clone(),window,cx); }
                        });
                    }));
                }
                menu
            })
            .map(titlebar_menu)
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h_full()
            .items_center()
            .flex_shrink_0()
            .text_size(px(14.))
            .w_full()
            .pr_3()
            .gap_2()
            .bg(rgb(SHELL))
            .child(div().w(px(48.)).h_full().flex_shrink_0().flex().items_center().justify_center()
                .child(gpui_kit::img("branding/plico.svg").size(px(20.))))
            .child(self.main_menu(cx))
            .child(self.project_menu(cx))
            .child(titlebar_button("update-project").ghost().icon(jb_icon("update_dark.svg")).icon_size(px(16.))
                .tooltip(t("Update Project")).accessibility_label(t("Update Project"))
                .disabled(self.fetch_token.is_some() || self.settings_open)
                .on_click(cx.listener(|s, _, w, cx| s.open_project_update(w, cx))))
            .child(titlebar_button("push").ghost().icon(jb_icon("push_dark.svg").size(px(16.))).icon_size(px(16.)).tooltip(t("Push")).accessibility_label(t("Push")).disabled(self.fetch_token.is_some() || self.settings_open)
                .on_click(cx.listener(|s, _, w, cx| s.open_push(w, cx))))
            .child(self.repository_branch_menu(cx))
            .when(self.operation_state.as_ref().is_some_and(|state| state.operation == plico::operation_state::Operation::Merge), |v| v
                .when(self.operation_state.as_ref().is_some_and(|state| state.conflicts.is_empty()), |v| v.child(
                    titlebar_button("complete-merge").primary().label(t("Complete Merge")).disabled(self.fetch_token.is_some() || self.settings_open)
                        .on_click(cx.listener(|s, _, w, cx| s.start_merge_operation(plico::merge_operation::Action::Commit, w, cx)))))
                .child(titlebar_button("abort-merge").label(t("Abort Merge")).disabled(self.fetch_token.is_some() || self.settings_open)
                    .on_click(cx.listener(|s, _, w, cx| s.confirm_abort_merge(w, cx)))))
            .when(self.operation_state.as_ref().is_some_and(|state| matches!(state.operation,
                plico::operation_state::Operation::RebaseApply | plico::operation_state::Operation::RebaseMerge)), |v| v
                .child(titlebar_button("continue-rebase").primary().label(t("Continue Rebase"))
                    .disabled(self.fetch_token.is_some() || self.settings_open || self.operation_state.as_ref().is_some_and(|state| !state.conflicts.is_empty()))
                    .on_click(cx.listener(|s, _, w, cx| s.start_rebase_operation(false, w, cx))))
                .child(titlebar_button("abort-rebase").label(t("Abort Rebase")).disabled(self.fetch_token.is_some() || self.settings_open)
                    .on_click(cx.listener(|s, _, w, cx| s.confirm_abort_rebase(w, cx)))))
            .child(div().flex_1())
            .child(titlebar_button("settings").ghost().icon(jb_icon("settings_dark.svg").size(px(14.))).icon_size(px(14.)).tooltip(format!("{} (Ctrl+Alt+S)", t("Settings"))).accessibility_label(t("Settings"))
                .on_click(cx.listener(|s, _, w, cx| { s.open_settings(w, cx); })))
    }
    fn left_tool_windows(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.comparison_visible { return div().size_full().children(self.comparison.clone()).into_any_element(); }
        if self.local_visible { return div().size_full().children(self.local_view.clone()).into_any_element(); }
        if self.project_visible {
            return div().size_full().children(self.project_view.clone()).into_any_element();
        }
        v_flex().size_full().rounded(px(10.)).overflow_hidden().bg(rgb(PANEL))
            .child(h_flex().h(px(36.)).px_3().child(t("Stashes")).child(div().flex_1())
                .child(Button::new("stashes-refresh").ghost().small().icon(jb_icon("refresh_dark.svg")).tooltip(t("Refresh"))
                    .on_click(cx.listener(|s, _, _, cx| { if let Some(v) = &s.stash_view { v.update(cx, |v, cx| v.refresh(cx)); } })))
                .child(Button::new("stashes-hide").ghost().small().icon(jb_icon("closeSmall.svg")).tooltip(t("Hide"))
                    .on_click(cx.listener(|s, _, _, cx| { s.stash_open = false; cx.notify(); }))))
            .child(div().flex_1().min_h_0().children(self.stash_view.clone())).into_any_element()
    }
    fn close_editor_ids(&mut self, mut ids: Vec<EntityId>, clicked: Option<EntityId>, window: &mut Window, cx: &mut Context<Self>) {
        if self.document_action_pending { return; }
        if self.documents.pending(cx) {
            self.with_saved_documents(window,cx,move|s,w,cx|s.close_editor_ids(ids,clicked,w,cx)); return;
        }
        let active = self.active_file_tab.and_then(|i| self.file_tabs.get(i).map(Entity::entity_id))
            .or_else(|| self.active_tab.and_then(|i| self.tabs.get(i).map(Entity::entity_id)));
        ids.sort_by_key(|id| Some(*id) == active);
        let closes_active = active.is_some_and(|id| ids.contains(&id));
        for id in ids {
            if let Some(i) = self.file_tabs.iter().position(|t| t.entity_id() == id) { self.close_file_tab(i, window, cx); }
            else if let Some(i) = self.tabs.iter().position(|t| t.entity_id() == id) { self.close_diff_tab_from_header(i, window, cx); }
        }
        if closes_active && self.active_file_tab.is_none() && self.active_tab.is_none() {
            let remaining = clicked.filter(|id| self.editor_tab_order.contains(id)).or_else(|| self.editor_tab_order.last().copied());
            if remaining.is_some() { self.activate_editor_neighbor(remaining, window, cx); }
        }
    }
    fn editor_tab_menu(&self, mut menu: gpui_kit::component::menu::PopupMenu, clicked: Option<EntityId>, cx: &mut Context<Self>) -> gpui_kit::component::menu::PopupMenu {
        let position = clicked.and_then(|id| self.editor_tab_order.iter().position(|p| *p == id));
        for (label, ids) in [
            ("Close", clicked.into_iter().collect::<Vec<_>>()),
            ("Close Other Tabs", self.editor_tab_order.iter().copied().filter(|id| Some(*id) != clicked).collect()),
            ("Close All Tabs", self.editor_tab_order.clone()),
            ("Close Tabs to the Right", self.editor_tab_order.iter().copied().skip(position.map_or(0, |i| i+1)).collect()),
            ("Close All Read-Only", self.editor_tab_order.iter().copied().filter(|id| !self.file_tabs.iter().any(|tab|tab.entity_id()==*id && !tab.read(cx).history)).collect()),
        ] {
            let owner = cx.entity().downgrade();
            menu = menu.item(PopupMenuItem::new(t(label)).disabled(ids.is_empty()).on_click(move |_, window, cx| {
                let _ = owner.update(cx, |s, cx| {
                    s.close_editor_ids(ids.clone(),clicked,window,cx);
                });
            }));
        }
        menu
    }
    fn git_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let log_menu_owner = cx.entity().downgrade();
        h_flex()
            .h(px(36.))
            .flex_shrink_0()
            .px(px(11.))
            .gap(px(8.))
            .bg(rgb(PANEL))
            .rounded_t(px(10.))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                islands_tab("log-tab", !self.local && self.active_tab.is_none() && self.active_file_tab.is_none())
                    .child(jb_icon("vcs_dark.svg"))
                    .child(t("Log"))
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.close_settings(cx);
                        s.leave_diff_tab(cx);
                        s.focus.focus(w, cx);
                        if s.local {
                            s.local = false;
                            s.refresh(false, cx);
                        }
                    }))
                    .context_menu(move |menu, _, cx| log_menu_owner.update(cx, |s, cx| s.editor_tab_menu(menu, None, cx)).unwrap()),
            )
            .children(self.editor_tab_order.iter().filter_map(|id| {
                if let Some((index, tab)) = self.file_tabs.iter().enumerate().find(|(_, tab)| tab.entity_id() == *id) {
                let preview = tab.read(cx).preview;
                let menu_owner = cx.entity().downgrade(); let tab_id = tab.entity_id();
                let name = tab.read(cx).path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                let name=if tab.read(cx).dirty(cx){format!("{name} •")}else{name};
                let name=if tab.read(cx).history{format!("{}: {name}",t("History"))}else{name};
                Some(islands_tab(("file-tab", index), self.active_file_tab == Some(index))
                    .child(img(file_icon(&tab.read(cx).path)).size(px(16.)).flex_shrink_0()).child(div().max_w(px(200.)).truncate().when(preview, |d| d.italic()).child(name))
                    .on_click(cx.listener(move |s, event: &ClickEvent, w, cx| {
                        s.cancel_project_preview(cx);
                        if event.click_count() == 2 { s.file_tabs[index].update(cx, |tab, _| tab.preview = false); }
                        if s.active_file_tab == Some(index) { s.file_tabs[index].update(cx, |tab, cx| tab.focus_viewer(w, cx)); }
                        else { s.leave_diff_tab(cx); s.active_file_tab = Some(index); s.file_tabs[index].update(cx, |tab, cx| tab.activate(cx)); }
                        cx.notify();
                    }))
                    .child(Button::new(("close-file", index)).ghost().small().w(px(16.)).h(px(16.))
                        .icon(jb_icon("closeSmall.svg")).tooltip(t("Close file"))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            cx.stop_propagation();
                            s.close_file_tab(index, w, cx);
                        })))
                    .context_menu(move |menu, _, cx| menu_owner.update(cx, |s, cx| s.editor_tab_menu(menu, Some(tab_id), cx)).unwrap()))
                } else if let Some((index, tab)) = self.tabs.iter().enumerate().find(|(_, tab)| tab.entity_id() == *id) {
                let caption = tab.read(cx).caption();
                let menu_owner = cx.entity().downgrade(); let tab_id = tab.entity_id();
                Some(islands_tab(("diff-tab", index), self.active_tab == Some(index))
                    .child(jb_icon("diff_dark.svg"))
                    .child(div().max_w(px(280.)).truncate().child(caption))
                    .on_click(cx.listener(move |s, _, w, cx| s.activate_diff_tab(index, w, cx)))
                    .child(Button::new(("close-diff", index)).ghost().small().w(px(16.)).h(px(16.))
                        .icon(jb_icon("closeSmall.svg")).tooltip(t("Close diff · Esc"))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            cx.stop_propagation(); s.close_diff_tab_from_header(index, w, cx);
                        })))
                    .context_menu(move |menu, _, cx| menu_owner.update(cx, |s, cx| s.editor_tab_menu(menu, Some(tab_id), cx)).unwrap()))
                } else { None }
            }))
            .child(div().flex_1())
    }
    fn rebuild_branches(&mut self, cx: &mut Context<Self>) {
        if let Some(snapshot) = &self.snapshot {
            self.branch_rows = Arc::new(branches::filtered_ref_rows(
                &snapshot.branches,
                &snapshot.tags,
                &self.collapsed_branches,
                &self.branch_search.read(cx).value(),
                snapshot.current_branch.as_ref().map(|name| format!("refs/heads/{name}")).as_deref(),
            ));
        }
        cx.notify();
    }
    fn select_branch(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        self.selected_tree_ref = branch.clone();
        self.branch = branch;
        self.local = false;
        self.limit = git::HISTORY_PAGE;
        self.refresh(false, cx);
    }
    fn navigate_tree_ref(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.snapshot.as_ref().and_then(|snapshot| {
            (if name == "HEAD" { snapshot.head_id.clone() } else { None })
                .or_else(|| snapshot.tags.iter().find(|r| r.name == name).map(|r| r.id.clone()))
                .or_else(|| snapshot.branches.iter().find(|r| r.name == name).map(|r| r.id.clone()))
        });
        if let Some(target) = target {
            self.selected_tree_ref = Some(name.to_owned());
            let label = name.strip_prefix("refs/tags/").or_else(|| name.strip_prefix("refs/heads/"))
                .or_else(|| name.strip_prefix("refs/remotes/")).unwrap_or(name);
            self.navigate_commit(label.to_owned(), target, false, window, cx);
        }
    }
    fn navigate_adjacent(&mut self, parent: bool, keyboard: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.history_loading || self.settings_open || self.welcome { return; }
        let ids = self.commit_ids();
        let selected = self.log_selection.ordered(&ids);
        if selected.len() != 1 { return; }
        let selected = selected[0].to_owned();
        let Some(roots) = self.snapshot.as_ref().map(|s| s.tips.clone()) else { return; };
        self.adjacent_token.cancel(); self.adjacent_token = Cancel::default();
        let token = self.adjacent_token.clone(); let repo = self.repo.clone();
        let generation = self.generation; let filter = self.text_filter.clone();
        self.adjacent_position = if keyboard {
            let scroll = self.history_scroll.0.borrow();
            let bounds = scroll.base_handle.bounds();
            let index = self.selected.unwrap_or(0);
            let y = bounds.origin.y + scroll.base_handle.offset().y + px((index + 1) as f32 * 28.);
            point(bounds.origin.x + px(80.), y.max(bounds.origin.y).min(bounds.bottom()))
        } else { window.mouse_position() };
        self.adjacent_task = cx.spawn_in(window, async move |this, wx| {
            let target = selected.clone(); let worker_token = token.clone();
            let result = wx.background_spawn(async move { history::adjacent(&repo, &roots, &target, parent, filter.as_ref(), &worker_token) }).await;
            let _ = wx.update(|window, cx| { let _ = this.update(cx, |s, cx| {
                if token.cancelled() || s.generation != generation || s.log_selection.ordered(&s.commit_ids()) != [selected.as_str()] { return; }
                let targets = match result { Ok(targets) => targets, Err(error) => { s.error = Some(error); cx.notify(); return; } };
                let title = t(if parent { "Select Parent Commit" } else { "Select Child Commit" });
                if targets.len() == 1 { s.navigate_commit(title.into(), targets[0].clone(), false, window, cx); return; }
                if targets.is_empty() { return; }
                let owner = cx.entity().downgrade(); let focus = s.history_focus.clone();
                let labels = targets.into_iter().map(|target| {
                    let label = s.commits.iter().find(|c| c.id == target).map(|c| {
                        let subject = c.subject.chars().take(40).collect::<String>();
                        format!("{}  {}  {}  {}", &target[..8], subject, c.author, c.date)
                    }).unwrap_or_else(|| target[..8].to_owned());
                    (target, label)
                }).collect::<Vec<_>>();
                let menu = gpui_kit::component::menu::PopupMenu::build(window, cx, move |mut menu, _, _| {
                    menu = menu.action_context(focus).max_w(px(620.)).max_h(px(320.)).scrollable(true).item(PopupMenuItem::label(title));
                    for (index, (target, label)) in labels.into_iter().enumerate() {
                        let label = if index < 10 { format!("{}.  {}", (index + 1) % 10, label) } else { label };
                        let owner = owner.clone(); let selected = selected.clone();
                        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                if s.generation == generation && s.log_selection.ordered(&s.commit_ids()) == [selected.as_str()] {
                                    s.navigate_commit(title.into(), target.clone(), false, window, cx);
                                }
                            });
                        }));
                    }
                    menu.numbered_navigation()
                });
                s.adjacent_subscription = Some(cx.subscribe(&menu, |s, _, _: &DismissEvent, cx| {
                    s.adjacent_menu = None; cx.notify();
                }));
                menu.read(cx).focus_handle(cx).focus(window, cx);
                s.adjacent_menu = Some(menu); cx.notify();
            }); });
        });
    }
    fn navigate_commit(&mut self, name: String, target: String, clear_filters: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate_commit_impl(name, target, clear_filters, false, window, cx);
    }
    fn navigate_commit_impl(&mut self, name: String, target: String, clear_filters: bool, fresh: bool, window: &mut Window, cx: &mut Context<Self>) {
        if fresh { self.history_loading = false; }
        if self.history_loading { return; }
        if !clear_filters && !fresh {
            if let Some(index) = self.commits.iter().position(|commit| commit.id == target) {
                self.local = false;
                self.select_commit(index, cx);
                self.history_scroll.scroll_to_item(index, ScrollStrategy::Top);
                self.history_focus.focus(window, cx);
                return;
            }
        }
        // Cancel old work without clearing the visible comparison. A hidden
        // target must leave the user's current view intact until explicit reset.
        self.token.cancel();
        self.token = Cancel::default();
        self.generation += 1;
        let generation = self.generation;
        let token = self.token.clone();
        self.busy = false;
        self.loading_patch = false;
        self.error = None;
        self.navigation_notice = None;
        self.navigation_loading = Some(name.clone());
        let repo = self.repo.clone();
        let branch = if clear_filters { None } else { self.branch.clone() };
        let fixed = if clear_filters || fresh { None } else { self.snapshot.as_ref().map(|s| s.tips.clone()) };
        let filter = if clear_filters { None } else { self.text_filter.clone() };
        let limit = self.limit;
        let query_target = target.clone();
        self.task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                let roots = match fixed { Some(roots) => roots, None => repo.snapshot(branch.as_deref(), 1, None, &token)?.tips };
                let located = match filter {
                    Some(filter) => history::locate_filtered(&repo, &roots, &query_target, limit, &filter, &token)?,
                    None => match history::locate(&repo, &roots, &query_target, limit, &token)? {
                        Some(found) => history::FilteredLocation::Found(found),
                        None => history::FilteredLocation::OutsideRoots,
                    },
                };
                if matches!(located, history::FilteredLocation::OutsideRoots) && !clear_filters {
                    let all_roots = repo.snapshot(None, 1, None, &token)?.tips;
                    if repo.commit_position(&all_roots, &query_target, &token)?.is_some() {
                        return Ok(history::FilteredLocation::Hidden);
                    }
                }
                Ok::<_, String>(located)
            }).await;
            let _ = wx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    if s.generation != generation { return; }
                    s.navigation_loading = None;
                    match result {
                        Ok(history::FilteredLocation::Found(found)) => {
                            s.local = false;
                            if clear_filters {
                                s.branch = None;
                                s.text_filter = None;
                                s.date_filter = Default::default();
                                s.filter_search.update(cx, |input, cx| input.set_value("", window, cx));
                                s.user_menu.update(cx, |menu, cx| menu.clear_filter(cx));
                                s.paths_menu.update(cx, |menu, cx| menu.clear_filter(cx));
                            }
                            let history::Window { mut snapshot, rows, start, end } = found.window;
                            s.history_offset = found.offset;
                            s.window_start = start; s.window_end = end;
                            s.rows = Arc::new(rows);
                            s.commits = Arc::new(std::mem::take(&mut snapshot.commits));
                            s.snapshot = Some(snapshot);
                            s.rebuild_branches(cx);
                            s.select_commit(found.selected, cx);
                            s.history_scroll.scroll_to_item(found.selected, ScrollStrategy::Top);
                            s.history_focus.focus(window, cx);
                        }
                        Ok(outcome) => s.navigation_notice = Some(NavigationNotice {
                            name, target, can_clear: matches!(outcome, history::FilteredLocation::Hidden),
                        }),
                        Err(error) => s.error = Some(error),
                    }
                    cx.notify();
                });
            });
        });
        cx.notify();
    }
    fn set_date_filter(&mut self, after: Option<i64>, cx: &mut Context<Self>) {
        self.date_filter = plico::date_filter::Bounds { after, before: None };
        self.apply_text_filter(cx);
    }
    fn select_date_period(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = cx.new(|cx| crate::date_dialog::DateDialog::new(self.date_filter, window, cx));
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let draft_confirm = draft.clone();
            let owner = owner.clone();
            dialog.title(div().font_weight(FontWeight::NORMAL).child(t("Select Period")))
                .w(px(514.)).min_h(px(132.))
                .margin_top(((window.viewport_size().height - px(132.)) / 2.).max(px(16.)))
                .overlay_closable(false)
                .child(draft.clone())
                .footer(DialogFooter::new()
                    .child(div().w(px(72.)).child(DialogAction::new().child(Button::new("date-ok").primary().label(t("OK"))))))
                .on_ok(move |_, _, cx| {
                    let Ok(bounds) = draft_confirm.update(cx, |s, cx| s.confirm(cx)) else { return false; };
                    if !bounds.is_empty() {
                        let _ = owner.update(cx, |s, cx| { s.date_filter = bounds; s.apply_text_filter(cx); });
                    }
                    true
                })
        });
    }
    fn date_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let caption = self.date_filter.caption();
        Button::new("date-filter").ghost().small().selected(!self.date_filter.is_empty())
            .child(h_flex().gap_1().child(caption).child(jb_icon("chevronDown_dark.svg")))
            .dropdown_menu(move |mut menu, _, _| {
                let select = owner.clone();
                menu = menu.item(PopupMenuItem::new(t("Select…")).on_click(move |_, window, cx| {
                    let _ = select.update(cx, |s, cx| s.select_date_period(window, cx));
                }));
                let bounds = plico::date_filter::presets(chrono::Local::now());
                for (label, index) in [("All dates", None), ("Last 24 hours", Some(0)), ("Last 7 days", Some(1))] {
                    let owner = owner.clone();
                    let bounds = bounds.clone();
                    menu = menu.item(PopupMenuItem::new(t(label)).on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |s, cx| match index {
                            None => s.set_date_filter(None, cx),
                            Some(index) => match &bounds {
                                Ok(bounds) => s.set_date_filter(Some(bounds[index]), cx),
                                Err(error) => { s.error = Some(error.clone()); cx.notify(); }
                            }
                        });
                    }));
                }
                menu
            })
    }
    fn branch_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut groups = std::collections::BTreeMap::<String, Vec<(String, String)>>::new();
        if let Some(snapshot) = &self.snapshot {
            for branch in &snapshot.branches {
                if let Some(name) = branch.name.strip_prefix("refs/heads/") {
                    groups.entry(t("Local").into()).or_default().push((name.into(), branch.name.clone()));
                } else if let Some(name) = branch.name.strip_prefix("refs/remotes/") {
                    if let Some((remote, _)) = name.split_once('/') {
                        groups.entry(format!("{remote}/…")).or_default().push((name.into(), branch.name.clone()));
                    }
                }
            }
        }
        let has_head = self.snapshot.as_ref().is_some_and(|s| s.head_id.is_some());
        let owner = cx.entity().downgrade();
        Button::new("branch-filter").ghost().small()
            .child(h_flex().gap_1().child(self.branch.as_deref().map(|b| format!("{}: {}", t("Branch"), b.strip_prefix("refs/heads/").or_else(|| b.strip_prefix("refs/remotes/")).unwrap_or(b))).unwrap_or_else(|| t("Branch").into())).child(jb_icon("chevronDown_dark.svg")))
            .selected(self.branch.is_some())
            .tooltip(self.branch.clone().unwrap_or_else(|| t("All branches").into()))
            .dropdown_menu(move |mut menu, window, cx| {
                let clear = owner.clone();
                menu = menu.min_w(px(250.)).item(PopupMenuItem::new(t("All branches"))
                    .on_click(move |_, _, cx| { let _ = clear.update(cx, |s, cx| s.select_branch(None, cx)); })).separator();
                if has_head {
                    let owner = owner.clone();
                    menu = menu.item(PopupMenuItem::new("HEAD").on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |s, cx| s.select_branch(Some("HEAD".into()), cx));
                    })).separator();
                }
                for (group, entries) in &groups {
                    let entries = entries.clone();
                    let owner = owner.clone();
                    menu = menu.submenu(group.clone(), window, cx, move |mut submenu, _, _| {
                        submenu = submenu.min_w(px(210.));
                        for (label, name) in &entries {
                            let owner = owner.clone(); let name = name.clone();
                            submenu = submenu.item(PopupMenuItem::new(label.clone()).on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |s, cx| s.select_branch(Some(name.clone()), cx));
                            }));
                        }
                        submenu
                    });
                }
                menu
            })
    }
    fn branches_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (label_asset, label_height) = if i18n::is_chinese() {
            ("ui/branches-stripe-zh.svg", 24.)
        } else {
            ("ui/branches-stripe.svg", 60.)
        };
        v_flex().id("branches-rail").w(px(28.)).h_full().flex_shrink_0()
            .bg(rgb(PANEL)).border_r_1().border_color(rgb(BORDER))
            .child(Button::new("toggle-branches").ghost().small().w(px(28.)).h(px(LOG_TOOLBAR_HEIGHT))
                .icon(jb_icon(if self.branches_visible { "left_dark.svg" } else { "chevronRight_dark.svg" }))
                .tooltip(t(if self.branches_visible { "Hide branches" } else { "Show branches" }))
                .on_click(cx.listener(|s, _, _, cx| { s.branches_visible = !s.branches_visible; cx.notify(); })))
            .when(!self.branches_visible, |rail| rail.child(
                div().id("expand-branches-label").w_full().h(px(65.)).flex().justify_center().cursor_pointer()
                    .hover(|d| d.bg(rgb(0x292b2e)))
                    .child(svg().path(label_asset).w(px(14.)).h(px(label_height)).text_color(rgb(FG)))
                    .on_click(cx.listener(|s, _, _, cx| { s.branches_visible = true; cx.notify(); }))))
    }
    fn head_in_tree(&self, cx: &App) -> bool {
        let query = self.branch_search.read(cx).value().trim().to_lowercase();
        self.snapshot.as_ref().is_some_and(|s| s.head_id.is_some())
            && (query.is_empty() || "head (current branch)".contains(&query))
    }
    fn filter_selected_tree_ref(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected_tree_ref.clone() else { return; };
        if name == "HEAD" || self.snapshot.as_ref().is_some_and(|s| s.branches.iter().any(|r| r.name == name)) {
            self.select_branch(Some(name), cx);
        }
    }
    fn tree_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tree_focus.is_focused(window) { return; }
        let modifiers = &event.keystroke.modifiers;
        if modifiers.alt && !modifiers.control && !modifiers.shift && !modifiers.platform && event.keystroke.key == "f1" {
            if let Some(name) = self.selected_tree_ref.clone() { self.navigate_tree_ref(&name, window, cx); }
            cx.stop_propagation();
            return;
        }
        if modifiers.alt || modifiers.control || modifiers.shift || modifiers.platform { return; }
        let key = event.keystroke.key.as_str();
        if key == "enter" {
            self.filter_selected_tree_ref(cx);
        } else if matches!(key, "up" | "down" | "home" | "end") {
            let head = self.head_in_tree(cx) as usize;
            let count = self.branch_rows.len() + head;
            if count == 0 { return; }
            let current = self.selected_tree_ref.as_ref().and_then(|selected| {
                if head == 1 && selected == "HEAD" { Some(0) }
                else { self.branch_rows.iter().position(|row| &row.key == selected).map(|i| i + head) }
            });
            let next = match key {
                "home" => 0,
                "end" => count - 1,
                "up" => current.map(|i| i.saturating_sub(1)).unwrap_or(count - 1),
                _ => current.map(|i| (i + 1).min(count - 1)).unwrap_or(0),
            };
            self.selected_tree_ref = Some(if head == 1 && next == 0 { "HEAD".into() } else {
                self.tree_scroll.scroll_to_item(next - head, ScrollStrategy::Top);
                self.branch_rows[next - head].key.clone()
            });
        } else if matches!(key, "left" | "right") {
            let Some(index) = self.branch_rows.iter().position(|row| Some(&row.key) == self.selected_tree_ref.as_ref()) else { return; };
            let row = self.branch_rows[index].clone();
            if row.branch.is_none() && ((key == "right" && !row.expanded) || (key == "left" && row.expanded)) {
                if self.branch_search.read(cx).value().trim().is_empty() {
                    if key == "right" { self.collapsed_branches.remove(&row.key); }
                    else { self.collapsed_branches.insert(row.key); }
                    self.rebuild_branches(cx);
                }
            } else if key == "right" && row.branch.is_none() && row.expanded {
                if let Some(child) = self.branch_rows.get(index + 1).filter(|child| child.depth > row.depth) {
                    self.selected_tree_ref = Some(child.key.clone());
                    self.tree_scroll.scroll_to_item(index + 1, ScrollStrategy::Top);
                }
            } else if key == "left" {
                if let Some(parent) = self.branch_rows[..index].iter().rposition(|parent| parent.depth < row.depth) {
                    self.selected_tree_ref = Some(self.branch_rows[parent].key.clone());
                    self.tree_scroll.scroll_to_item(parent, ScrollStrategy::Top);
                }
            }
        } else { return; }
        cx.stop_propagation();
        cx.notify();
    }
    fn branches(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection_color = row_selection(&self.tree_focus, window);
        let owner = cx.entity().downgrade();
        let branches = self.branch_rows.clone();
        let selected = self.selected_tree_ref.clone();
        let current = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.current_branch.as_ref())
            .map(|name| format!("refs/heads/{name}"));
        v_flex().id("branches-tree")
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
            .when(self.head_in_tree(cx), |panel| panel.child(
                h_flex().id("head-branch").relative().h(px(28.)).w_full().pl(px(12.)).pr_2().gap(px(6.))
                    .text_size(px(13.)).overflow_hidden().cursor_pointer()
                    .when(selected.as_deref() == Some("HEAD"), |row| row.child(tree_selection(selection_color)))
                    .child(jb_icon("branchLabel_dark.svg")).child(t("HEAD (Current Branch)"))
                    .on_click(cx.listener(|s, event: &ClickEvent, window, cx| {
                        s.tree_focus.focus(window, cx);
                        s.selected_tree_ref = Some("HEAD".into());
                        if event.click_count() == 2 { s.filter_selected_tree_ref(cx); }
                        cx.notify();
                    })),
            ))
            .when(
                branches.is_empty() && !self.branch_search.read(cx).value().trim().is_empty()
                    && !(self.snapshot.as_ref().is_some_and(|snapshot| snapshot.head_id.is_some())
                        && "head (current branch)".contains(&self.branch_search.read(cx).value().trim().to_lowercase())),
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
                uniform_list("branches", branches.len(), move |range, _, _| {
                    range
                        .map(|i| {
                            let branch = branches[i].clone();
                            let owner = owner.clone();
                            let menu_owner = owner.clone();
                            let menu_ref = branch.branch.clone();
                            let is_current = branch.branch.is_some() && branch.branch == current;
                            let icon = if branch.branch.is_some() {
                                "branchLabel_dark.svg"
                            } else if branch.expanded {
                                "chevronDown_dark.svg"
                            } else {
                                "chevronRight_dark.svg"
                            };
                            div().relative()
                                .id(i)
                                .w_full()
                                .h(px(28.))
                                .pl(px(12. + branch.depth as f32 * 12.))
                                .pr_2()
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .text_size(px(13.))
                                .when(selected.as_ref() == Some(&branch.key), |d| {
                                    d.child(tree_selection(selection_color))
                                })
                                .gap(px(6.))
                                .child(jb_icon(icon).text_color(rgb(if is_current {
                                    // Rebased new/currentBranchLabel_dark.svg uses
                                    // the same label geometry with this gold color.
                                    0xf2c55c
                                } else if branch.branch.as_deref().is_some_and(|name| name.starts_with("refs/tags/")) {
                                    0x868a91
                                } else {
                                    FG
                                })))
                                .when(branch.branch.is_none() && branch.depth > 0, |d| {
                                    d.child(
                                        img("jb/folder_dark_4x.svg").size(px(16.)).flex_shrink_0(),
                                    )
                                })
                                .child(div().truncate().child(match branch.key.as_str() { "refs/heads" => t("Local").to_owned(), "refs/remotes" => t("Remote").to_owned(), "refs/tags" => t("Tags").to_owned(), _ => branch.label.clone() }))
                                .on_click(move |event, window, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        s.tree_focus.focus(window, cx);
                                        s.selected_tree_ref = Some(branch.key.clone());
                                        cx.notify();
                                        if branch.branch.is_some() {
                                            if event.click_count() == 2 { s.filter_selected_tree_ref(cx); }
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
                                .context_menu(move |menu, _, _| {
                                    let Some(name) = &menu_ref else { return menu; };
                                    let name = name.clone();
                                    let owner = menu_owner.clone();
                                    let filter_owner = owner.clone();
                                    let filter_name = name.clone();
                                    menu.when(!name.starts_with("refs/tags/"), |menu| menu.item(PopupMenuItem::new(t("Update Branch Filter in Log"))
                                        .on_click(move |_, _, cx| { let _ = filter_owner.update(cx, |s, cx| s.select_branch(Some(filter_name.clone()), cx)); })))
                                    .item(PopupMenuItem::new(t("Navigate Log to Selected Branch Head")).on_click(move |_, window, cx| {
                                        let _ = owner.update(cx, |s, cx| s.navigate_tree_ref(&name, window, cx));
                                    }))
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
    fn set_long_edges(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.history_loading || self.graph_print_loading || enabled == self.show_long_edges { return; }
        let Some(model) = self.graph_model.clone() else { return; };
        if self.rows.is_empty() { return; }
        self.graph_print_token.cancel();
        self.graph_print_token = Cancel::default();
        self.graph_print_generation += 1;
        let generation = self.graph_print_generation;
        let token = self.graph_print_token.clone();
        let current = model.clone();
        self.graph_print_loading = true;
        self.graph_print_task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { model.project(enabled, &token) }).await;
            let _ = this.update(cx, |s,cx| {
                if generation != s.graph_print_generation || !s.graph_model.as_ref().is_some_and(|m| Arc::ptr_eq(m, &current)) { return; }
                s.graph_print_loading = false;
                match result {
                    Ok(rows) => match layout_preferences::save_log_long_edges(&s.repo.path, enabled) {
                        Ok(()) => { s.rows = Arc::new(rows); s.show_long_edges = enabled; s.graph_hover = None; }
                        Err(error) => s.error = Some(format!("{}: {error}", t("Settings could not be saved"))),
                    },
                    Err(error) => s.error = Some(error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn log_view_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let path = self.repo.path.clone();
        let checked = self.show_long_edges;
        let disabled = self.history_loading || self.graph_print_loading || self.rows.is_empty() || self.graph_model.is_none();
        Button::new("log-view-options").ghost().small().w(px(28.)).flex_shrink_0()
            .icon(jb_icon("history-show_dark.svg")).tooltip(t("View Options")).accessibility_label(t("View Options"))
            .dropdown_menu(move |mut menu,_,cx| {
                if let Some(view) = owner.upgrade() {
                    let current_path = path.clone();
                    let subscription = cx.observe(&view, move |menu, view, cx| {
                        let s = view.read(cx);
                        let checked = s.show_long_edges;
                        let enabled = s.repo.path == current_path && !s.history_loading && !s.graph_print_loading && !s.rows.is_empty() && s.graph_model.is_some();
                        menu.set_item_state(0, checked, enabled, cx);
                    });
                    menu = menu.with_subscription(subscription);
                }
                menu.min_w(px(200.)).close_on_confirm(false).item(
                PopupMenuItem::new(t("Long Edges")).checked(checked).disabled(disabled)
                    .on_click({ let owner = owner.clone(); let path = path.clone(); move |_,_,cx| { let _ = owner.update(cx, |s,cx| {
                        if s.repo.path == path { s.set_long_edges(!s.show_long_edges,cx); }
                    }); } }))
            })
    }
    fn startup_frame(&self) -> Option<(Arc<crate::startup_report::Probe>, crate::startup_report::Frame)> {
        let probe = self.startup_report.as_ref().filter(|probe| !probe.done())?;
        if !self.welcome && (self.snapshot.is_none() || self.history_loading || self.graph_print_loading || self.error.is_some()) { return None; }
        Some((probe.clone(), crate::startup_report::Frame {
            welcome: self.welcome, requested: self.limit, commits: self.commits.len(), graph_rows: self.rows.len(),
            max_columns: self.rows.iter().map(|row| row.width).max().unwrap_or(0),
            offset: self.history_offset, more: self.snapshot.as_ref().is_some_and(|s| s.more),
            filtered: self.branch.is_some() || self.text_filter.is_some(),
        }))
    }
    fn history(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection_color = row_selection(&self.history_focus, window);
        let checkout_head = self.snapshot.as_ref().and_then(|s| s.head_id.clone());
        let checkout_enabled = self.fetch_token.is_none() && checkout_head.is_some();
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
        let included = self.snapshot.as_ref().map(|s| s.included_in_head.clone()).unwrap_or_default();
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
                                        .h(px(28.))
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
                                            .child(interactive_graph_cell(rows[i].clone(), graph::text_offset_columns(&rows[i], recommended_width) * 16. + 12., i,
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
                                            let reset_owner=checkout_owner.clone(); let reset_target=target.clone(); let reset_path=path.clone();
                                            let reset_enabled=checkout_owner.upgrade().is_some_and(|e|{let s=e.read(cx);s.fetch_token.is_none() && !s.document_action_pending && s.log_selection.ordered(&s.commit_ids()).len()==1});
                                            let parent_owner = checkout_owner.clone(); let child_owner = checkout_owner.clone();
                                            let adjacent_enabled = checkout_owner.upgrade().is_some_and(|e| {
                                                let s = e.read(cx); !s.history_loading && s.log_selection.ordered(&s.commit_ids()).len() == 1
                                            });
                                            let compare_owner = checkout_owner.clone();
                                            let compare_enabled = checkout_owner.upgrade().is_some_and(|e| e.read(cx).log_selection.ordered(&e.read(cx).commit_ids()).len() == 2);
                                            let squash_owner = checkout_owner.clone();
                                            let squash_enabled = checkout_owner.upgrade().is_some_and(|entity| {
                                                let s = entity.read(cx); s.fetch_token.is_none() && s.log_selection.ordered(&s.commit_ids()).len() >= 2
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
                                                        s.snapshot.as_ref().map(|snapshot| snapshot.branches.iter()
                                                            .filter(|b| b.id == target && b.name.starts_with("refs/heads/") && snapshot.current_branch.as_ref().is_none_or(|n| b.name != format!("refs/heads/{n}")))
                                                            .map(|b| b.name.clone()).collect::<Vec<_>>())
                                                    }).unwrap_or_default();
                                                    if branches.is_empty() {
                                                        menu.item(PopupMenuItem::new(format!("{} {}", t("Checkout Revision"), &target[..8])).disabled(!checkout_enabled)
                                                            .on_click(move |_, window, cx| { let _ = owner.update(cx, |s, cx| {
                                                                if s.repo.path == path { if let Some(head) = &head { s.start_checkout(target.clone(), head.clone(), window, cx); } }
                                                            }); }))
                                                    } else {
                                                        menu.submenu(t("Checkout"), window, cx, move |mut sub, _, _| {
                                                            for reference in &branches {
                                                                let reference = reference.clone(); let owner = owner.clone(); let target = target.clone(); let head = head.clone(); let path = path.clone();
                                                                sub = sub.item(PopupMenuItem::new(reference.strip_prefix("refs/heads/").unwrap_or(&reference).to_owned()).disabled(!checkout_enabled)
                                                                    .on_click(move |_, w, cx| { let _ = owner.update(cx, |s, cx| {
                                                                        if s.repo.path == path { if let Some(head) = &head { s.start_local_checkout(reference.clone(), target.clone(), head.clone(), w, cx); } }
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
                                                .item(PopupMenuItem::new(t("Reset Current Branch to Here…")).disabled(!reset_enabled).on_click(move|_,w,cx| {
                                                    let _=reset_owner.update(cx,|s,cx|{if s.repo.path==reset_path{s.prepare_reset(reset_target.clone(),w,cx);}});
                                                })).separator()
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
    fn set_files_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        self.collapsed_files.clear();
        self.collapsed_directories.clear();
        if !expanded {
            for file in self.files.iter() {
                if self.local && !self.collapsed_files.contains(&file.area) {
                    self.collapsed_files.push(file.area);
                }
                for directory in file.path.ancestors().skip(1).filter(|p| !p.as_os_str().is_empty()) {
                    self.collapsed_directories.insert((file.area, directory.to_path_buf()));
                }
            }
        }
        self.rebuild_files();
        // Retain the review target; expanding restores the same selected file.
        cx.notify();
    }
    fn branch_filter_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().gap_0().child(self.branch_menu(cx)).when(self.branch.is_some(), |v| v.child(
            Button::new("clear-branch-filter").ghost().small().icon(jb_icon("closeSmall.svg"))
                .tooltip(t("Clear branch filter")).on_click(cx.listener(|s,_,_,cx| s.select_branch(None,cx)))))
    }
    fn date_filter_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().gap_0().child(self.date_menu(cx)).when(!self.date_filter.is_empty(), |v| v.child(
            Button::new("clear-date-filter").ghost().small().icon(jb_icon("closeSmall.svg"))
                .tooltip(t("Clear date filter")).on_click(cx.listener(|s,_,_,cx| s.set_date_filter(None,cx)))))
    }
    fn filter_overflow(&self, include_branch: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        Button::new("log-filter-overflow").ghost().small().w(px(28.)).flex_shrink_0()
            .icon(jb_icon("chevronRight_dark.svg")).tooltip(t("Filters")).accessibility_label(t("Filters"))
            .dropdown_menu(move |mut menu,_,_| {
                let hit_owner = owner.clone();
                menu = menu.min_w(px(180.)).label(t("Filters")).close_on_confirm(false)
                    .child_popup_hit_test(move |position,cx| hit_owner.upgrade().is_some_and(|view| {
                        let s = view.read(cx);
                        s.user_menu.read(cx).contains_popup_position(position)
                            || s.paths_menu.read(cx).contains_popup_position(position)
                    }));
                for filter in 0..4 {
                    if filter == 0 && !include_branch { continue; }
                    let owner = owner.clone();
                    menu = menu.item(PopupMenuItem::element(move |_,cx| {
                        owner.update(cx, |s,cx| match filter {
                            0 => s.branch_filter_control(cx).into_any_element(),
                            1 => s.user_menu.clone().into_any_element(),
                            2 => s.date_filter_control(cx).into_any_element(),
                            _ => s.paths_menu.clone().into_any_element(),
                        }).unwrap_or_else(|_| div().into_any_element())
                    }));
                }
                menu
            })
    }
    fn files_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_diff = self.selected_file.is_some_and(|i| self.files.get(i).is_some());
        let has_tree = self.local && !self.files.is_empty() || self.files.iter()
            .any(|file| file.path.parent().is_some_and(|p| !p.as_os_str().is_empty()));
        h_flex().h(px(LOG_TOOLBAR_HEIGHT)).flex_shrink_0().px(px(4.)).gap(px(2.))
            .border_b_1().border_color(rgb(BORDER))
            .child(Button::new("files-show-diff").ghost().small().w(px(28.)).h(px(28.))
                .icon(jb_icon("diff_dark.svg")).tooltip(t("Show Diff · Ctrl+D")).disabled(!can_diff)
                .on_click(cx.listener(|s, _, w, cx| s.open_diff_tab(w, cx))))
            .child(div().flex_1())
            .child(Button::new("files-expand-all").ghost().small().w(px(28.)).h(px(28.))
                .icon(jb_icon("expandAll_dark.svg")).tooltip(t("Expand All")).disabled(!has_tree)
                .on_click(cx.listener(|s, _, _, cx| s.set_files_expanded(true, cx))))
            .child(Button::new("files-collapse-all").ghost().small().w(px(28.)).h(px(28.))
                .icon(jb_icon("collapseAll_dark.svg")).tooltip(t("Collapse All")).disabled(!has_tree)
                .on_click(cx.listener(|s, _, _, cx| s.set_files_expanded(false, cx))))
    }
    fn files(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection_color = row_selection(&self.files_focus, window);
        let files = self.files.clone();
        let rows = self.file_rows.clone();
        let selected = self.selected_file;
        let owner = cx.entity().downgrade();
        let file_list = v_flex()
            .size_full()
            .min_h_0()
            .bg(rgb(PANEL))
            .child(self.files_toolbar(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .track_focus(&self.files_focus)
                    .key_context("Files")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, _, w, cx| s.files_focus.focus(w, cx)),
                    )
                    .child(
                        uniform_list("files", rows.len(), move |range, _, _| {
                            range
                                .map(|row| {
                                    let (i, depth) = match &rows[row] {
                                        FileRow::File(i, depth) => (*i, *depth),
                                        FileRow::Folder(area, path, depth, count, expanded) => {
                                            let area = *area;
                                            let path = path.clone();
                                            let owner = owner.clone();
                                            return h_flex()
                                                .id(row)
                                                .w_full()
                                                .h(px(28.))
                                                .pr_2()
                                                .pl(px(8.
                                                    + *depth as f32 * 16.
                                                    + if area == Area::Commit { 0. } else { 24. }))
                                                .gap_2()
                                                .child(jb_icon(if *expanded {
                                                    "chevronDown_dark.svg"
                                                } else {
                                                    "chevronRight_dark.svg"
                                                }))
                                                .child(img("jb/folder_dark_4x.svg").size(px(16.)))
                                                .child(
                                                    div().truncate().child(
                                                        path.file_name()
                                                            .unwrap_or_default()
                                                            .to_string_lossy()
                                                            .into_owned(),
                                                    ),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(rgb(MUTED))
                                                        .child(count.to_string()),
                                                )
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        let key = (area, path.clone());
                                                        if !s.collapsed_directories.remove(&key) {
                                                            s.collapsed_directories.insert(key);
                                                        }
                                                        s.rebuild_files();
                                                        cx.notify();
                                                    });
                                                })
                                                .into_any_element();
                                        }
                                        FileRow::Group(area, count, expanded) => {
                                            let (area, count, expanded) =
                                                (*area, *count, *expanded);
                                            let owner = owner.clone();
                                            return h_flex()
                                                .id(row)
                                                .w_full()
                                                .h(px(28.))
                                                .px_2()
                                                .gap_2()
                                                .child(
                                                    jb_icon(if expanded {
                                                        "chevronDown_dark.svg"
                                                    } else {
                                                        "chevronRight_dark.svg"
                                                    })
                                                    .size(px(16.)),
                                                )
                                                .child(area.label())
                                                .child(
                                                    div()
                                                        .text_color(rgb(MUTED))
                                                        .child(count.to_string()),
                                                )
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        if s.collapsed_files.contains(&area) {
                                                            s.collapsed_files
                                                                .retain(|a| *a != area);
                                                        } else {
                                                            s.collapsed_files.push(area);
                                                        }
                                                        s.rebuild_files();
                                                        cx.notify();
                                                    });
                                                })
                                                .into_any_element();
                                        }
                                    };
                                    let f = &files[i];
                                    let owner = owner.clone();
                                    h_flex().relative()
                                        .id(row)
                                        .w_full()
                                        .h(px(28.))
                                        .px_2()
                                        .gap_2()
                                        .pl(px(32.
                                            + depth as f32 * 16.
                                            + if f.area == Area::Commit { 0. } else { 24. }))
                                        .when(selected == Some(i), |d| d.child(tree_selection(selection_color)))
                                        .tooltip({
                                            let label = format!(
                                                "{} · {}\n{}",
                                                f.area.label(),
                                                f.status,
                                                f.path.display()
                                            );
                                            move |window, cx| {
                                                Tooltip::new(label.clone()).build(window, cx)
                                            }
                                        })
                                        .child(
                                            img(file_icon(&f.path)).size(px(16.)).flex_shrink_0(),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .flex_1()
                                                .text_color(rgb(change_color(f)))
                                                .child(
                                                    f.path
                                                        .file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy()
                                                        .into_owned(),
                                                ),
                                        )
                                        .on_click(move |event, window, cx| {
                                            let _ = owner.update(cx, |s, cx| {
                                                s.load_file(i, cx);
                                                if event.click_count() == 2 {
                                                    s.open_diff_tab(window, cx);
                                                }
                                            });
                                        })
                                        .into_any_element()
                                })
                                .collect()
                        })
                        .size_full()
                        .track_scroll(&self.files_scroll),
                    )
                    .child(Scrollbar::vertical(&self.files_scroll))
                    // Selection clears the previous tree immediately; keep visible
                    // feedback in the changes pane even when the Diff lives in a tab.
                    .when(self.busy, |pane| pane.child(
                        div().absolute().inset_0().p_4().bg(rgb(PANEL))
                            .text_color(rgb(MUTED)).child(t("Reading Git data…"))
                    )),
            );
        if self.local || (self.selected.is_none() && self.selection_details.is_empty()) {
            return file_list.into_any_element();
        }
        let detail_commit = self.selected.and_then(|i| self.commits.get(i)).cloned();
        let details = v_flex().relative()
            .overflow_hidden()
            .bg(rgb(PANEL))
            .size_full()
            .min_h_0()
            .min_w_0()
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                v_flex()
                    .id(("commit-details", self.selected.unwrap_or_default()))
                    .track_scroll(&self.details_scroll)
                    .size_full()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .px(px(14.))
                    .pt(px(14.))
                    .pb(px(14.))
                    .gap_0()
                    .children(self.selection_details.iter().enumerate().map(|(i, (refs, detail, branches))| {
                        v_flex().id(("selected-commit-detail", i)).w_full().flex_shrink_0().gap_2().pb_3().mb_3().border_b_1().border_color(rgb(BORDER))
                            .child(TextView::html(format!("multi-message-{i}"), detail.message.clone()).font_family("JetBrains Mono").text_size(px(13.)).selectable(true))
                            .child(gpui_kit::base::TextView::html(format!("multi-metadata-{i}"), detail.metadata[usize::from(i18n::is_chinese())].clone()).text_size(px(13.)).style(gpui_kit::base::TextViewStyle::default().with_foreground(rgb(FG).into()).with_link(rgb(0x71a1fe).into()).with_selection(rgb(SELECT).into())).selectable(true))
                            .child(commit_refs(refs))
                            .child(branches.clone())
                    }))
                    .when(self.log_selection.ordered(&self.commit_ids()).len() > 50, |v| v.child(div().child(t("Showing the first 50 commits"))))
                    .child(TextView::html("commit-message", self.message.clone())
                        .mb(px(20.))
                        .w_full().flex_shrink_0()
                        .font_family("JetBrains Mono")
                        .text_size(px(13.))
                        .style(TextViewStyle::default().paragraph_gap(rems(0.)))
                        .selectable(true))
                    .child(gpui_kit::base::TextView::html("commit-metadata", self.commit_metadata[usize::from(i18n::is_chinese())].clone())
                        .mb(px(10.))
                        .w_full().flex_shrink_0()
                        .font_family(crate::ui_font::family())
                        .text_size(px(13.))
                        .style(gpui_kit::base::TextViewStyle::default()
                            .with_foreground(rgb(FG).into())
                            .with_link(rgb(0x71a1fe).into())
                            .with_selection(rgb(SELECT).into())
                            .with_paragraph_gap(rems(0.)))
                        .selectable(true))
                    .children(detail_commit.as_ref().map(|commit| commit_refs(&commit.refs)))
                    .children(self.containing_branches.clone())
                    .context_menu(move |mut menu, _, _| {
                        if let Some(commit) = &detail_commit {
                            let hash = commit.id.clone();
                            menu = menu.item(PopupMenuItem::new("Copy Commit Hash").on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(hash.clone()));
                            })).separator().label(t("Parents"));
                            if commit.shallow_boundary {
                                menu = menu.label(t("Unavailable (shallow boundary)"));
                            } else if commit.parents.is_empty() {
                                menu = menu.label(t("Root commit"));
                            } else {
                                for parent in &commit.parents {
                                    let parent = parent.clone();
                                    menu = menu.item(PopupMenuItem::new(parent.clone()).on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(parent.clone()));
                                    }));
                                }
                            }
                        }
                        menu
                    }),
            ).child(Scrollbar::vertical(&self.details_scroll).mode(gpui_kit::component::scroll::ScrollbarMode::Always));
        v_resizable("files-details")
            .with_state(&self.details_split)
            .on_resize(cx.listener(|s, state: &Entity<ResizableState>, _, cx| {
                let sizes = state.read(cx).sizes();
                if let [files, details] = sizes.as_slice() {
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
                    .size_range(px(48.)..px(4000.))
                    .child(file_list),
            )
            .child(
                resizable_panel()
                    .flex_basis(relative(1. - self.files_fraction))
                    .size_range(px(60.)..px(4000.))
                    .child(details),
            )
            .into_any_element()
    }
    fn show_error_details(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(error) = self.error.clone() else { return; };
        crate::error_ui::show_details(error, "Git error details", window, cx);
    }
    fn empty_diff_message(&self) -> &'static str {
        if self.files.is_empty() {
            if !self.local && self.text_filter.is_some() && self.commits.is_empty() {
                return "No commits match the message filter. Clear the filter field and press Enter to show all history.";
            }
            if self.local {
                "No local changes. The index and working tree are clean."
            } else if let Some(commit) = self.selected.and_then(|i| self.commits.get(i)) {
                if commit.shallow_boundary {
                    "Parent history is unavailable at this shallow boundary. Deepen the clone with your Git tool, then refresh to review its changes."
                } else if commit.parents.len() > 1 {
                    "No file changes compared with the first parent. Other merge parents may differ."
                } else {
                    "This commit has no file changes compared with its parent or the empty tree."
                }
            } else {
                if !self.selection_details.is_empty() { "No changes" }
                else if !self.commits.is_empty() { "Select commits to view changes" }
                else { "No commits in this history. Use Local Changes to review files before the first commit." }
            }
        } else if self.selected_file.is_some() {
            "No patch was returned for this file. Refresh to check for changes made outside this viewer."
        } else {
            "Select a changed file to review its diff"
        }
    }
    fn preview_place(&self) -> layout_preferences::DiffPlace {
        if self.local { layout_preferences::DiffPlace::Changes } else { layout_preferences::DiffPlace::Log }
    }
    fn set_preview_appearance(&mut self, value: layout_preferences::DiffAppearance, cx: &mut Context<Self>) {
        if let Err(error) = layout_preferences::save_diff_appearance(self.preview_place(), value) {
            self.preview_preference_error = Some(format!("{}: {error}", t("Cannot save Diff settings")));
            cx.notify(); return;
        }
        self.preview_preference_error = None;
        self.preview_appearance = value;
        self.diff.update(cx, |d, cx| d.set_inline_unit(crate::diff_settings::inline_unit(value.emphasis), cx));
        cx.notify();
    }
    fn set_preview_mode(&mut self, mode: DiffMode, cx: &mut Context<Self>) {
        if let Err(error) = layout_preferences::save_diff_unified(self.preview_place(), mode == DiffMode::Unified) {
            self.preview_preference_error = Some(format!("{}: {error}", t("Cannot save Diff view")));
            cx.notify(); return;
        }
        self.preview_preference_error = None;
        self.preview_mode = mode;
        self.diff.update(cx, |d, cx| d.set_mode(crate::diff_settings::effective_mode(d.files(), mode), cx));
        cx.notify();
    }
    fn preview_settings_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        crate::diff_settings::appearance_button(self.preview_appearance, move |value, cx| {
            let _ = owner.update(cx, |s, cx| s.set_preview_appearance(value, cx));
        })
    }
    fn diff_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let side_titles = self
            .selected_file
            .and_then(|index| self.files.get(index))
            .and_then(|file| {
                let revisions = match file.area {
                    Area::Commit => {
                        let commit = self.selected.and_then(|index| self.commits.get(index))?;
                        let before = commit
                            .parents
                            .first()
                            .map(|id| {
                                let short = &id[..8.min(id.len())];
                                if commit.parents.len() > 1 { format!("{} {short}", t("First parent")) }
                                else { short.to_owned() }
                            })
                            .unwrap_or_else(|| t("Empty tree").into());
                        (before, commit.id[..8.min(commit.id.len())].to_owned())
                    }
                    Area::WorkingTree => ("HEAD".into(), t("Working tree").into()),
                    Area::Staged => ("HEAD".into(), t("Staged").into()),
                    Area::Unstaged => (t("Staged").into(), t("Working tree").into()),
                    Area::Untracked => (t("Empty file").into(), t("Untracked working file").into()),
                    Area::Conflict => return None,
                };
                let before = file
                    .old_path
                    .as_ref()
                    .unwrap_or(&file.path)
                    .to_string_lossy();
                Some((
                    format!("{before} · {}", revisions.0),
                    format!("{} · {}", file.path.display(), revisions.1),
                ))
            });
        let content = if let Some(error) = self.preview_error.clone() {
            v_flex().size_full().items_center().justify_center().gap_2().p_4()
                .child(t("Could not load the diff."))
                .child(h_flex().gap_3()
                    .child(Button::new("reload-preview").link().small().label(t("Reload diff"))
                        .on_click(cx.listener(|s, _, _, cx| { if let Some(index) = s.selected_file { s.load_file(index, cx); } })))
                    .child(Button::new("copy-preview-error").link().small().label(t("Copy error details"))
                        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(error.clone())))))
                .into_any_element()
        } else if self.error.is_some() {
            v_flex().size_full().items_center().justify_center().gap_2().p_4()
                .child(t("Could not load changes. Refresh to retry."))
                .child(Button::new("reload-changes").link().small().label(t("Refresh"))
                    .disabled(self.busy || self.history_loading)
                    .on_click(cx.listener(|s, _, w, cx| s.refresh_current(w, cx))))
                .into_any_element()
        } else if self.busy || self.loading_patch {
            div()
                .p_5()
                .text_color(rgb(MUTED))
                .child(t("Reading Git data…"))
                .into_any_element()
        } else if self.diff.read(cx).files().is_empty() {
            div()
                .p_5()
                .text_color(rgb(MUTED))
                .child(t(self.empty_diff_message()))
                .into_any_element()
        } else {
            Diff::new(&self.diff)
                .size_full()
                .soft_wrap(self.preview_appearance.soft_wrap)
                .line_number(self.preview_appearance.line_numbers)
                .change_indicator(DiffChangeIndicator::None)
                .into_any_element()
        };
        v_flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .bg(rgb(BG))
            .rounded(px(10.))
            .child(
                h_flex()
                    .h(px(40.)).flex_shrink_0().mx(px(6.)).mt(px(2.))
                    .px_3()
                    .gap_2()
                    .bg(rgb(0x212326))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .truncate()
                            .flex_1()
                            .text_size(px(12.))
                            .child(self.base.as_ref().map(|base| base.text()).unwrap_or_default()),
                    )
                    .child(
                        Button::new("prev-change")
                            .ghost()
                            .small()
                            .icon(jb_icon("up_dark.svg"))
                            .tooltip(t("Previous change (Shift+F7)"))
                            .disabled(self.preview_error.is_some() || self.error.is_some() || self.loading_patch || self.busy)
                            .on_click(cx.listener(|s, _, w, cx| {
                                if let Some(tab) = s.active_tab.and_then(|i| s.tabs.get(i)) { tab.update(cx, |t, cx| t.navigate(false, w, cx)); } else { if let Some(p) = &s.local_preview { p.update(cx, |p, cx| p.navigate(false, w, cx)); } else { s.diff.update(cx, |d, cx| d.previous_change(cx)); } }
                            })),
                    )
                    .child(
                        Button::new("next-change")
                            .ghost()
                            .small()
                            .icon(jb_icon("down_dark.svg"))
                            .tooltip(t("Next change (F7)"))
                            .disabled(self.preview_error.is_some() || self.error.is_some() || self.loading_patch || self.busy)
                            .on_click(cx.listener(|s, _, w, cx| {
                                if let Some(tab) = s.active_tab.and_then(|i| s.tabs.get(i)) { tab.update(cx, |t, cx| t.navigate(true, w, cx)); } else { if let Some(p) = &s.local_preview { p.update(cx, |p, cx| p.navigate(true, w, cx)); } else { s.diff.update(cx, |d, cx| d.next_change(cx)); } }
                            })),
                    )
                    .child(
                        Button::new("preview-jump-source").ghost().small().icon(jb_icon("edit_dark.svg"))
                            .tooltip(t("Jump to Source · F4")).disabled(self.loading_patch || self.busy || self.selected_file.is_none())
                            .on_click(cx.listener(|s,_,w,cx|s.jump_to_source(w,cx)))
                    )
                    .child(
                        Button::new("expand")
                            .ghost()
                            .small()
                            .label(t(if self.expanded { "Restore" } else { "Expand" }))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.expanded = !s.expanded;
                                cx.notify();
                            })),
                    )
                    .child({
                        let owner = cx.entity().downgrade();
                        crate::diff_settings::mode_selector(self.preview_mode, move |mode, cx| {
                            let _ = owner.update(cx, |s, cx| s.set_preview_mode(mode, cx));
                        })
                    })
                    .child(self.preview_settings_button(cx)),
            )
            .when_some(self.preview_preference_error.clone(), |panel, error| panel.child(
                h_flex().px_3().py_1().gap_2()
                    .child(div().text_color(rgb(0xf2a6a6)).child(t("Could not save Diff preferences. Try the setting again.")))
                    .child(Button::new("copy-preview-preference-error").link().small().label(t("Copy error details"))
                        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(error.clone()))))))
            .when(
                self.error.is_none()
                    && self.diff.read(cx).files().iter().any(|file| {
                        file.extended_headers().iter().any(|header| {
                            header.split_whitespace().last() == Some("160000")
                        })
                    }),
                |panel| panel.child(
                    div().px_3().py_2().text_size(px(12.)).text_color(rgb(MUTED))
                        .child(t("Submodule reference changed · The diff shows commit IDs, not the files inside the submodule.")),
                ),
            )
            .when_some(side_titles.filter(|_| self.preview_error.is_none() && self.error.is_none()), |panel, (before, after)| {
                let status = self.diff.read(cx).files().first().map(|file| file.status());
                let unified = self.preview_mode == DiffMode::Unified;
                let titles = [(status != Some(DiffFileStatus::Added)).then_some(before),
                    (status != Some(DiffFileStatus::Deleted)).then_some(after)];
                panel.child(
                    div().flex().when(unified, |row| row.flex_col()).w_full().flex_shrink_0()
                        .border_b_1().border_color(rgb(BORDER))
                        .text_size(px(12.)).text_color(rgb(MUTED))
                        .children(titles.into_iter().flatten().enumerate().map(|(side, title)| {
                            div().id(("diff-version", side)).when(unified, |cell| cell.w_full())
                                .when(!unified, |cell| cell.flex_1()).min_w_0().px_3().py_1().truncate()
                                .tooltip({ let title = title.clone(); move |window, cx| Tooltip::new(title.clone()).build(window, cx) })
                                .child(title)
                        })),
                )
            })
            .child(div().flex_1().w_full().min_h_0().min_w_0().child(content))
            .relative().children(island_corners(!self.expanded))
    }
}
// IntelliJ IDEA Islands Dark: themes/islands/IslandSchemeDark.xml.
pub(crate) fn change_color(change: &Change) -> u32 {
    if change.area == Area::Conflict {
        return 0xde6a66;
    }
    if change.area == Area::Untracked {
        return 0xe88f89;
    }
    match change.status.as_bytes().first() {
        Some(b'A' | b'C') => 0x73bd79,
        Some(b'M' | b'R' | b'T') => 0x70aeff,
        Some(b'D') => 0x6f737a,
        _ => FG,
    }
}
fn jb_icon(name: &'static str) -> Icon {
    Icon::default()
        .path(format!("jb/{name}"))
        .size(px(16.))
        .text_color(rgb(FG))
}
pub(crate) fn file_icon(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" | "log" => "jb/file-text_dark_4x.svg",
        "rs" => "jb/file-rust_dark_4x.svg",
        "py" | "pyi" => "jb/file-python_dark_4x.svg",
        "c" | "h" => "jb/file-c_dark_4x.svg",
        "cpp" | "cc" | "cxx" | "hpp" => "jb/file-cpp_dark_4x.svg",
        "sh" | "bash" | "zsh" => "jb/file-shell_dark_4x.svg",
        "md" | "markdown" => "jb/file-markdown_dark_4x.svg",
        "json" | "jsonc" => "jb/file-json_dark_4x.svg",
        "yaml" | "yml" => "jb/file-yaml_dark_4x.svg",
        "toml" | "ini" | "properties" | "cfg" | "conf" => "jb/file-config_dark_4x.svg",
        "java" => "jb/file-java_dark_4x.svg",
        "js" | "jsx" | "mjs" | "cjs" => "jb/file-javaScript_dark_4x.svg",
        "ts" | "tsx" => "jb/file-typeScript_dark_4x.svg",
        "xml" | "svg" => "jb/file-xml_dark_4x.svg",
        _ => "jb/file-anyType_dark_4x.svg",
    }
}
// Copyright 2000-2023 JetBrains s.r.o. and contributors.
// Geometry and layering adapted from Rebased LabelIcon / TagPainter (Apache-2.0).
fn ref_color(kind: git::RefKind) -> u32 {
    match kind {
        git::RefKind::Head => 0xf5d273,
        git::RefKind::Local => 0x5fad65,
        git::RefKind::Remote => 0xb589ec,
        git::RefKind::Tag => 0x868a91,
    }
}
fn reference_icon(colors: Vec<u32>, background: u32) -> impl IntoElement {
    let width = 16. + 5.12 * colors.len().saturating_sub(1) as f32;
    let mut layers = Vec::new();
    for (i, color) in colors.iter().rev().enumerate() {
        let offset = 5.12 * i as f32;
        let last = i + 1 == colors.len();
        // Cached SVG masks retain TagPainter's curves and opaque layering,
        // without adding per-frame path-rendering resources on Windows.
        layers.push(jb_icon("reference-mask.svg").absolute().left(px(offset)).top_0()
            .text_color(rgb(background)).into_any_element());
        layers.push(jb_icon(if last { "reference-front.svg" } else { "reference-outline.svg" })
            .absolute().left(px(offset)).top_0().text_color(rgb(*color)).into_any_element());
        if !last {
            layers.push(jb_icon("reference-mask.svg").absolute()
                .left(px(5.12 * (i + 1) as f32 - std::f32::consts::SQRT_2)).top_0()
                .text_color(rgb(background)).into_any_element());
        }
    }
    div().relative().w(px(width)).h(px(16.)).flex_shrink_0().children(layers)
}

fn interactive_graph_cell(row: Row, width: f32, index: usize, node_style: GraphNodeStyle, owner: WeakEntity<Review>, rows: Arc<Vec<Row>>, commits: Arc<Vec<Commit>>, hovered: Option<(usize, usize)>) -> AnyElement {
    let hovered = hovered.filter(|(r, _)| *r == index).map(|(_, segment)| segment);
    let tooltip = hovered.and_then(|segment| row.segments.get(segment).filter(|s| s.arrow()).map(|_| {
        row.arrow_target(segment).and_then(|target| commits.get(target)).map(|commit| {
            graph_arrow_tooltip(&commit.id, &commit.subject, &commit.author, &plico::commit_date::format(&commit.date))
        }).unwrap_or_else(|| t("Commit is outside the loaded history window").into())
    }));
    let hover_owner = owner.clone();
    let hover_rows = rows.clone();
    graph_arrow_cell(row, width, index, node_style, hovered, tooltip, move |segment, cx| {
        let _ = hover_owner.update(cx, |s, cx| {
            if !Arc::ptr_eq(&hover_rows, &s.rows) || s.history_loading || s.graph_print_loading { return; }
            if segment.is_none() && s.graph_hover.is_some_and(|(row, _)| row != index) { return; }
            let hovered = segment.map(|segment| (index, segment));
            if s.graph_hover != hovered { s.graph_hover = hovered; cx.notify(); }
        });
    }, move |target, window, cx| {
        let _ = owner.update(cx, |s,cx| {
            if !Arc::ptr_eq(&rows, &s.rows) || s.history_loading || s.graph_print_loading || target >= s.commits.len() { return; }
            s.local = false;
            s.graph_hover = None;
            s.select_commit(target, cx);
            s.history_scroll.scroll_to_item(target, ScrollStrategy::Nearest);
            s.history_focus.focus(window, cx);
        });
    })
}

pub(crate) fn graph_arrow_tooltip(id: &str, subject: &str, author: &str, date: &str) -> String {
    let chars: Vec<_> = subject.chars().take(51).collect();
    let title: String = if chars.len() > 50 { chars[..47].iter().collect::<String>() + "..." } else { chars.iter().collect() };
    format!("{} {}\n{title}\n{author} · {date}", t("Jump to Commit"), &id[..id.len().min(8)])
}

/// Shared hit regions, cursor and tooltips; owners supply their own guarded
/// selection/preview callbacks. No history queries or owner lifetimes live here.
#[derive(Clone, Copy)]
pub(crate) struct GraphNodeStyle { pub head: bool, pub background: u32 }

pub(crate) fn graph_arrow_cell(row: Row, width: f32, index: usize, node_style: GraphNodeStyle, hovered: Option<usize>, tooltip: Option<String>,
    on_hover: impl Fn(Option<usize>, &mut App) + 'static,
    on_jump: impl Fn(usize, &mut Window, &mut App) + 'static) -> AnyElement {
    if !row.segments.iter().any(|edge| edge.arrow()) { return graph_cell(row, width, node_style, None).into_any_element(); }
    let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::default()));
    let actionable = hovered.is_some_and(|segment| row.arrow_target(segment).is_some());
    let hit = {
        let bounds = bounds.clone(); let row = row.clone();
        move |position: Point<Pixels>| {
            let bounds = bounds.get();
            if !bounds.contains(&position) { return None; }
            let point = position - bounds.origin;
            graph::arrow_at(&row, point.x.as_f32(), point.y.as_f32())
        }
    };
    let hover_hit = hit.clone(); let down_hit = hit.clone();
    let on_hover = std::rc::Rc::new(on_hover); let leave = on_hover.clone();
    graph_cell(row, width, node_style, hovered).id(("graph-arrow", index))
        // Explicit anchoring avoids the default static position below the
        // drawing canvas, which would offset the hit region by a whole row.
        .child(canvas(move |value, _, _| bounds.set(value), |_, _, _, _| {}).absolute().inset_0())
        .when(actionable, |v| v.cursor_pointer())
        .when_some(tooltip, |v, text| v.tooltip(move |w, cx| Tooltip::new(text.clone()).build(w, cx)))
        .on_mouse_move(move |event, _, cx| on_hover(hover_hit(event.position).map(|hit| hit.segment), cx))
        .on_hover(move |hovered, _, cx| { if !hovered { leave(None, cx); } })
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            if down_hit(event.position).is_some() { cx.stop_propagation(); }
        })
        .on_click(move |event, window, cx| {
            let Some(hit) = hit(event.position()) else { return; };
            cx.stop_propagation();
            if let Some(target) = hit.target { on_jump(target, window, cx); }
        }).into_any_element()
}

pub(crate) fn graph_cell(row: Row, width: f32, node_style: GraphNodeStyle, hovered: Option<usize>) -> Div {
    let node = row.node;
    let color = row.color;
    div()
        .relative()
        .overflow_hidden()
        .w(px(width))
        .h(px(28.))
        .flex_none()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let visible = bounds.intersect(&window.content_mask().bounds);
                    // FromPrintElement selects this exact row/segment. Like
                    // SimpleGraphCellPainter, paint it last: themed outline,
                    // then the ordinary branch color (including arrow wings).
                    let selected = hovered.filter(|&index| row.segments.get(index).is_some_and(|edge| edge.arrow()));
                    let selected_edge = selected.and_then(|index| row.segments.get(index));
                    let passes = row.segments.iter().enumerate()
                        .filter(|(index, _)| Some(*index) != selected).map(|(_, edge)| (edge, false))
                        .chain(selected_edge.map(|edge| (edge, true)))
                        .chain(selected_edge.map(|edge| (edge, false)));
                    for (edge, outline) in passes {
                        // Preserve the existing 1.5 logical pixel ordinary
                        // geometry; guarantee one physical pixel on each side.
                        let thickness = if outline { 2.5_f32.max(1.5 + 2. / window.scale_factor()) } else { 1.5 };
                        let ink = if outline { FG } else { COLORS[edge.color % COLORS.len()] };
                        let x = |lane: f32| bounds.origin.x + px(8. + lane * 16.);
                        let middle = (edge.from() as f32 + edge.to() as f32) / 2.;
                        let from = point(
                            x(if edge.upper() && edge.adjacent() { middle } else { edge.from() as f32 }),
                            bounds.origin.y + px(if edge.upper() { 0. } else { 14. }),
                        );
                        let to = point(
                            x(if !edge.upper() && edge.adjacent() { middle } else { edge.to() as f32 }),
                            bounds.origin.y + px(if edge.full() || !edge.upper() { 28. } else { 14. }),
                        );
                        // The Commit cell clips wide graphs before the author
                        // column. Avoid creating GPU paths wholly outside that
                        // clip; keep crossing edges and the arrow's stroke halo.
                        let halo = px(if edge.arrow() { 9. } else { 2. });
                        if from.x.min(to.x) > visible.right() + halo
                            || from.x.max(to.x) < visible.left() - halo
                            || from.y.min(to.y) > visible.bottom() + halo
                            || from.y.max(to.y) < visible.top() - halo { continue; }
                        let mut p = PathBuilder::stroke(px(thickness));
                        if edge.dotted() {
                            // Rebased uses one dash/space per vertical row,
                            // centered on the node; our paths are half-row segments.
                            let dx = (to.x - from.x).as_f32();
                            let dy = (to.y - from.y).as_f32();
                            let length = dx.hypot(dy);
                            let period = length * 2. / (length * 2. / 28.).floor().max(1.);
                            let dash = period - 12.;
                            let phase = dash / 2. + if edge.upper() { length } else { 0. };
                            let mut at = 0.;
                            while at < length {
                                let position = (at + phase) % period;
                                let on = position < dash;
                                let step = (if on {
                                    dash - position
                                } else {
                                    period - position
                                })
                                .max(0.01);
                                let end = (at + step).min(length);
                                if on {
                                    p.move_to(point(
                                        from.x + px(dx * at / length),
                                        from.y + px(dy * at / length),
                                    ));
                                    p.line_to(point(
                                        from.x + px(dx * end / length),
                                        from.y + px(dy * end / length),
                                    ));
                                }
                                at = end;
                            }
                        } else {
                            p.move_to(from);
                            p.line_to(to);
                        }
                        if let Ok(path) = p.build() {
                            window.paint_path(path, rgb(ink));
                        }
                        if edge.arrow() {
                            let tip = if edge.upper() { from } else { to };
                            let center = point(x(if edge.upper() { edge.to() } else { edge.from() } as f32), bounds.origin.y + px(14.));
                            let dx = (tip.x - center.x).as_f32();
                            let dy = (tip.y - center.y).as_f32();
                            let length = dx.hypot(dy).max(0.01);
                            // SimpleGraphCellPainter: length=.3*rowHeight,
                            // cos²(angle)=.7, oriented along the actual edge.
                            let back = 0.7_f32.sqrt() * 8.4;
                            let side = 0.3_f32.sqrt() * 8.4;
                            let mut arrow = PathBuilder::stroke(px(thickness));
                            for sign in [-1., 1.] {
                                arrow.move_to(point(tip.x - px(dx / length * back) + px(-dy / length * side * sign),
                                    tip.y - px(dy / length * back) + px(dx / length * side * sign)));
                                arrow.line_to(tip);
                            }
                            if let Ok(path) = arrow.build() { window.paint_path(path, rgb(ink)); }
                        }
                    }
                },
            )
            .size_full(),
        )
        .child(if node_style.head {
            // HeadNodePainter: colored outer ring, commit-row background,
            // colored center. Keep this presentation out of shared geometry.
            div().absolute().left(px(2. + node as f32 * 16.)).top(px(8.))
                .size(px(12.)).rounded_full().bg(rgb(COLORS[color % COLORS.len()]))
                .child(div().absolute().left(px(2.)).top(px(2.)).size(px(8.)).rounded_full().bg(rgb(node_style.background))
                    .child(div().absolute().left(px(2.)).top(px(2.)).size(px(4.)).rounded_full().bg(rgb(COLORS[color % COLORS.len()]))))
        } else {
            div().absolute().left(px(4. + node as f32 * 16.)).top(px(10.))
                .size(px(8.)).rounded_full().bg(rgb(COLORS[color % COLORS.len()]))
        })
}
pub(crate) fn commit_refs(references: &[git::RefLabel]) -> AnyElement {
    v_flex().flex_shrink_0().gap_0().when(!references.is_empty(), |v| v.mb(px(10.))).children(
                        [false, true].into_iter().filter_map(|tags| {
                            let refs: Vec<_> = references.iter().filter(|r| (r.kind == git::RefKind::Tag) == tags).take(100).collect();
                            if refs.is_empty() { return None; }
                            let total = references.iter().filter(|r| (r.kind == git::RefKind::Tag) == tags).count();
                            let mut items = Vec::new();
                            for kind in [git::RefKind::Head, git::RefKind::Local, git::RefKind::Remote, git::RefKind::Tag] {
                                let group: Vec<_> = refs.iter().filter(|r| r.kind == kind).collect();
                                for (index, reference) in group.iter().enumerate() {
                                    let text = format!("{}{}", reference.name, if index + 1 < group.len() { "," } else { "" });
                                    items.push(h_flex().gap(px(2.)).max_w_full().min_w_0()
                                        .when(index == 0, |d| d.child(reference_icon(vec![ref_color(kind); group.len().min(2)], PANEL)))
                                        .child(TextView::html(format!("detail-ref-{kind:?}-{}", reference.name), text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")).selectable(true))
                                        .into_any_element());
                                }
                            }
                            if total > refs.len() { items.push(div().child(i18n::message(i18n::Message::MoreReferences(total - refs.len()))).into_any_element()); }
                            Some(h_flex().w_full().flex_shrink_0().flex_wrap().gap_x(px(4.)).text_size(px(13.)).children(items))
                        }).collect::<Vec<_>>()
    ).into_any_element()
}
// IslandsTabPainter: rounded selected fill/border, no underline, 28 px content.
fn islands_tab(id: impl Into<ElementId>, selected: bool) -> Stateful<Div> {
    h_flex().id(id).h(px(28.)).flex_shrink_0().px(px(8.)).gap(px(6.))
        .rounded(px(5.)).border_1().border_color(transparent_black()).cursor_pointer()
        .text_color(rgb(FG))
        .when(selected, |tab| tab.bg(rgb(0x233558)).border_color(rgb(0x2e4d89)))
        .when(!selected, |tab| tab.hover(|style| style.bg(rgb(0x26282c))))
}
impl Render for Review {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.documents.set_locked(self.document_action_pending || self.fetch_token.is_some() || self.local_view.as_ref().is_some_and(|v|v.read(cx).writing()), cx);
        self.sync_language(_window, cx);
        let show_local_preview = self.bottom_preview && (self.local_visible || self.stash_open || self.aggregate_chain.is_some()) && self.active_tab.is_none() && self.active_file_tab.is_none();
        if !show_local_preview { self.local_preview = None; }
        else if self.stash_open {
            if let Some(view) = &self.stash_view {
                if !view.read(cx).preview_loading() {
                    match view.update(cx, |view, _| view.preview_target()) {
                        Some(Ok(crate::stash_view::OpenDiff(target, chain, origin))) => {
                            let unchanged = self.local_preview.as_ref().is_some_and(|p| p.read(cx).target.identity() == target.identity());
                            if !unchanged {
                                let preview = cx.new(|_| { let mut p = DiffTab::preview(target, chain); p.documents=self.documents.clone(); p.stash_origin = Some(origin); p });
                                let callback=self.diff_source_callback(cx);
                    preview.update(cx, |p, cx| {p.source_callback=Some(callback);p.activate(_window, cx);});
                                self.local_preview = Some(preview);
                            } else if let Some(preview) = &self.local_preview {
                                preview.update(cx, |p, _| p.stash_origin = Some(origin));
                            }
                        }
                        _ => self.local_preview = None,
                    }
                }
            }
        }
        else if !self.local_visible && self.aggregate_chain.is_some() {
            let pair = self.aggregate_chain.clone().and_then(|mut chain| chain.select(self.selected_file?).map(|target| (target, chain)));
            if let Some((target, chain)) = pair {
                if !self.local_preview.as_ref().is_some_and(|p| p.read(cx).target.identity() == target.identity()) {
                    let preview = cx.new(|_| {let mut p=DiffTab::preview(target,chain);p.documents=self.documents.clone();p});
                    let callback=self.diff_source_callback(cx);
                    preview.update(cx, |p, cx| {p.source_callback=Some(callback);p.activate(_window, cx);});
                    self.local_preview = Some(preview);
                }
            } else { self.local_preview = None; }
        }
        else if let Some(view) = &self.local_view {
            let view=view.read(cx);
            // Keep the viewer while a listing refresh is in flight. Ordinary
            // sidebar notifications (draft edits/inclusion) do not invalidate it.
            if !view.preview_loading() {
                let revision=view.preview_revision();
                match view.preview_change() {
                    Some(change) => {
                        let unchanged=self.local_preview.as_ref().is_some_and(|p| {
                            let p=p.read(cx);
                            p.snapshot_revision==revision && p.target.change().area==change.area
                                && p.target.change().path==change.path && p.target.change().old_path==change.old_path
                                && p.target.change().status==change.status
                        });
                        if let Some((target,chain))=(!unchanged).then(||view.preview_target()).flatten() {
                            let callback=self.diff_source_callback(cx);
                            let reusable=self.local_preview.as_ref().filter(|p|p.read(cx).target.identity()==target.identity()).cloned();
                            let preview=if let Some(preview)=reusable {
                                preview.update(cx,|p,cx|{p.replace_target(target,chain,cx);p.snapshot_revision=revision;});
                                preview
                            } else {cx.new(|_| {let mut p=DiffTab::preview(target,chain);p.documents=self.documents.clone();p.snapshot_revision=revision;p})};
                            preview.update(cx, |p,cx|{p.source_callback=Some(callback);p.activate_with_focus(false,_window,cx);});
                            self.local_preview=Some(preview);
                        }
                    },
                    None=>self.local_preview=None,
                }
            }
        }
        if let Some(view) = &self.stash_view { let busy = self.fetch_token.is_some() || self.settings_open; view.update(cx, |v, cx| v.set_write_busy(busy, cx)); }
        let minimum_navigation = if self.local { 340. } else if self.branches_visible { 640. } else { 500. };
        let navigation_scale = if self.navigation_width > px(0.) {
            (self.navigation_width.as_f32() / minimum_navigation).min(1.)
        } else { 1. };
        if self.welcome {
            return v_flex()
                .size_full()
                .bg(rgb(PANEL))
                .when_some(self.startup_frame(), |v, (probe, frame)| v.on_prepaint(move |_, _, _| probe.record(frame)))
                .text_color(rgb(FG)).text_size(px(13.)).font_family(crate::ui_font::family())
                .track_focus(&self.focus)
                .key_context("Review")
                .on_action(cx.listener(|s, _: &Open, w, cx| s.open(w, cx)))
                .on_action(cx.listener(|s, _: &OpenSettings, w, cx| s.open_settings(w, cx)))
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
                        .child(h_flex().h(px(40.)).flex_shrink_0().border_b_1().border_color(rgb(BORDER)).child(div().flex_1().child(t("Recent Projects")))
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
            .text_size(px(13.))
            .text_color(rgb(FG))
            .font_family(crate::ui_font::family())
            .bg(rgb(SHELL))
            .track_focus(&self.focus)
            .key_context("Review")
            .on_action(cx.listener(|s, _: &Open, w, cx| s.open(w, cx)))
            .on_action(cx.listener(|s, _: &OpenSettings, w, cx| s.open_settings(w, cx)))
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
                if let Some(index) = s.active_file_tab { s.close_file_tab(index, w, cx); }
                else if let Some(index) = s.active_tab { s.close_diff_tab_from_header(index, w, cx); }
            }))
            .on_action(cx.listener(|s, _: &Back, w, cx| {
                s.close_settings(cx);
                if let Some(tab) = s.active_file_tab.and_then(|index| s.file_tabs.get(index)) {
                    tab.update(cx, |tab, cx| tab.escape(w, cx));
                    return;
                }
                let return_to_comparison = s.active_tab.is_some() && s.active_tab == s.comparison_preview_index(cx);
                if let Some(index) = s.active_tab { s.close_diff_tab(index, cx); }
                if return_to_comparison {
                    s.cancel_project_preview(cx);
                    s.comparison_visible = true; s.local_visible = false; s.project_visible = false; s.stash_open = false;
                    if let Some(view) = &s.comparison { view.update(cx, |v, cx| v.focus_files(w, cx)); }
                    cx.notify(); return;
                }
                s.expanded = false;
                s.focus_after_diff(w, cx);
                cx.notify();
            }))
            .on_action(
                cx.listener(|s, _: &NextChange, w, cx| {
                    if let Some(tab) = s.active_tab.and_then(|i| s.tabs.get(i)) { tab.update(cx, |t, cx| t.navigate(true, w, cx)); } else { if let Some(p) = &s.local_preview { p.update(cx, |p, cx| p.navigate(true, w, cx)); } else { s.diff.update(cx, |d, cx| d.next_change(cx)); } }
                }),
            )
            .on_action(cx.listener(|s, _: &PreviousChange, w, cx| {
                if let Some(tab) = s.active_tab.and_then(|i| s.tabs.get(i)) { tab.update(cx, |t, cx| t.navigate(false, w, cx)); } else { if let Some(p) = &s.local_preview { p.update(cx, |p, cx| p.navigate(false, w, cx)); } else { s.diff.update(cx, |d, cx| d.previous_change(cx)); } }
            }))
            .on_action(cx.listener(|s, _: &crate::project_view::SaveFile, _, cx| { s.documents.save_all(cx); }))
            .on_action(cx.listener(|s, _: &Find, w, cx| {
                if s.repository_menu_open {
                    s.repository_search_editing = true;
                    s.dismiss_repository_child(cx);
                    s.repository_search_input.read(cx).focus_handle(cx).focus(w, cx);
                    cx.stop_propagation(); cx.notify(); return;
                }
                s.leave_diff_tab(cx);
                s.expanded = false;
                if s.local {
                    s.local = false;
                    s.refresh(false, cx);
                }
                s.filter_search.read(cx).focus_handle(cx).focus(w, cx);
                cx.notify();
            }))
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
                    .child(v_flex().w(px(36.)).gap_1()
                        .child(Button::new("local-tool-window").ghost().selected(self.local_visible)
                            .w(px(32.)).h(px(32.)).icon_size(px(20.)).icon(jb_icon("tool-commit_dark.svg")).tooltip(t("Local Changes")).accessibility_label(t("Local Changes"))
                            .on_click(cx.listener(|s, _, w, cx| { if s.local_visible { s.local_visible = false; cx.notify(); } else { s.open_local(w, cx); } })))
                        .when(self.comparison.is_some(), |v| v.child(Button::new("comparison-tool-window").ghost().selected(self.comparison_visible)
                            .w(px(32.)).h(px(32.)).icon_size(px(20.)).icon(jb_icon("tool-changes_dark.svg")).tooltip(t("Compare Versions"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.comparison_visible = !s.comparison_visible;
                                if s.comparison_visible { s.cancel_project_preview(cx); s.local_visible = false; s.project_visible = false; s.stash_open = false; }
                                cx.notify();
                            }))))
                        .child(Button::new("project-tool-window").ghost().selected(self.project_visible)
                        .w(px(32.)).h(px(32.)).icon_size(px(20.)).icon(jb_icon("tool-project_dark.svg")).tooltip(t("Project"))
                        .accessibility_label(t("Project"))
                        .on_click(cx.listener(|s, _, w, cx| s.toggle_project(w, cx))))
                        .child(Button::new("stashes-tool-window").ghost().selected(self.stash_open)
                            .w(px(32.)).h(px(32.)).icon_size(px(20.)).icon(jb_icon("shelve_dark.svg")).tooltip(t("Stashes")).accessibility_label(t("Stashes"))
                            .on_click(cx.listener(|s, _, w, cx| { if s.stash_open { s.stash_open = false; cx.notify(); } else { s.open_stashes(w, cx); } }))))
                    .child(div().flex_1().min_w_0().h_full().child(h_resizable("tool-windows-width").with_state(&self.tool_windows_width)
                    .child(resizable_panel().visible(self.local_visible || self.project_visible || self.stash_open || self.comparison_visible).flex_none().size(px(300.)).size_range(px(240.)..px(700.))
                        .child(div().size_full().pr(px(6.)).child(div().relative().size_full().overflow_hidden().child(self.left_tool_windows(cx)).children(island_corners(true)))))
                    .child(resizable_panel().child(v_flex().flex_1().min_w_0().h_full().child(self.git_toolbar(cx))
                    .child(div().flex_1().min_h_0().child(if let Some(tab) = self.active_file_tab.and_then(|i| self.file_tabs.get(i)) {
                        div().relative().size_full().rounded_b(px(10.)).overflow_hidden().bg(rgb(BG)).child(tab.clone()).children(island_corners(false)).into_any_element()
                    } else if let Some(tab) = self.active_tab.and_then(|i| self.tabs.get(i)) {
                        div().relative().size_full().rounded_b(px(10.)).overflow_hidden().bg(rgb(BG)).child(tab.clone()).children(island_corners(false)).into_any_element()
                    } else if self.expanded {
                        div().relative().size_full().rounded_b(px(10.)).overflow_hidden().bg(rgb(BG)).child(self.diff_panel(cx)).children(island_corners(false)).into_any_element()
                    } else {
                        v_resizable("vertical")
                            .child(
                                resizable_panel()
                                    .size(px(380.))
                                    .size_range(px(160.)..px(1800.))
                                    .child(
                                        v_flex()
                                            .relative().size_full()
                                            .rounded_b(px(10.))
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
                                            .children(island_corners(false)),
                                    ),
                            )
                            .when(self.bottom_preview && !(self.commits.is_empty() && self.error.is_some() && !self.local_visible && !self.stash_open), |layout| layout.child(
                                resizable_panel().size_range(px(150.)..px(2400.)).child(
                                    div().size_full().pt(px(8.)).child(
                                        div()
                                            .relative().size_full()
                                            .rounded(px(10.))
                                            .overflow_hidden()
                                            .child(if self.local_visible || self.stash_open || self.aggregate_chain.is_some() {
                                                div().size_full().children(self.local_preview.clone())
                                                    .when(self.local_preview.is_none(), |v| v.child(div().p_3().child(t("Select a file to view its diff")))).into_any_element()
                                            } else { self.diff_panel(cx).into_any_element() })
                                            .children(island_corners(true)),
                                    ),
                                ),
                            ))
                            .into_any_element()
                    })))))),
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
                    .when(self.snapshot.as_ref().is_some_and(|s| s.more || s.shallow), |v| v.child(
                        div().min_w_0().truncate().child(t("History is partially loaded"))))
                    .child(div().flex_1())
            .when_some(self.error.clone(), |v, error| v.child(h_flex().gap_2().min_w_0()
                .child(div().text_color(rgb(0xf2a6a6)).child(t(if plico::git_error::is_ownership_error(&error) { "Repository access blocked" } else { "Operation failed" })))
                .child(Button::new("copy-operation-error").link().xsmall().label(t("Details"))
                    .on_click(cx.listener(|s, _, w, cx| s.show_error_details(w, cx))))
                .child(Button::new("dismiss-operation-error").ghost().xsmall().label(t("Dismiss"))
                    .on_click(cx.listener(|s, _, _, cx| { s.error = None; cx.notify(); })))))
            .when_some(self.fetch_status.clone(), |v, text| v.child(div().max_w_full().min_w_0().truncate().child(text)))
            .when(self.squash_undo.is_some(), |v| v.child(Button::new("squash-undo").ghost().xsmall().label(t("Undo Squash"))
                .disabled(self.fetch_token.is_some()).on_click(cx.listener(|s, _, w, cx| s.undo_squash(w, cx)))))
            .when(self.rebase_diagnostic.is_some(), |v| v.child(Button::new("rebase-details").ghost().xsmall().label(t("Rebase details"))
                .on_click(cx.listener(|s, _, w, cx| {
                    let Some(text) = s.rebase_diagnostic.clone() else { return; };
                    w.open_dialog(cx, move |dialog, _, _| dialog.title(t("Rebase details")).w(px(640.)).centered(true)
                        .child(div().id("rebase-diagnostic").max_h(px(300.)).overflow_y_scroll().text_size(px(13.)).child(text.clone())));
                }))))
            .when(self.saved_changes_available, |v| v.child(Button::new("view-saved-changes").ghost().xsmall().label(t("Show saved changes"))
                .on_click(cx.listener(|s, _, w, cx| s.open_stashes(w, cx)))))
            .when(self.fetch_token.is_some() && !self.reset_running, |v| v.child(Button::new("cancel-fetch").ghost().xsmall().label(t("Cancel"))
                .on_click(cx.listener(|s, _, _, _| { if let Some(token) = &s.fetch_token { token.cancel(); } })))),
            )
            .when_some(self.adjacent_menu.clone(), |v, menu| v.child(deferred(anchored()
                .position(self.adjacent_position).snap_to_window_with_margin(px(8.)).child(menu)).with_priority(100)))
            .into_any_element()
    }
}
