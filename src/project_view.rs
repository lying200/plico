use std::{collections::BTreeMap, path::{Path, PathBuf}};
use plico::{git::Cancel, i18n::t, project_files::{self, Entry, Kind, Project}};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{button::{Button, ButtonVariants as _}, input::{Editor, EditorState, Position, RopeExt as _}, Icon, menu::ContextMenuExt as _, Sizable as _, WindowExt as _, h_flex, v_flex};

actions!(project_browser, [Up, Down, First, Last, OpenFile, Expand, Collapse, ShowFileHistory, ShowSelectionHistory, SaveFile]);
pub struct FileSaved;
pub struct HistoryRequested { pub path: PathBuf, pub selection: Option<plico::file_history::Selection> }
pub struct Open(pub PathBuf, pub bool, pub u64);
pub struct ProjectView {
    root: PathBuf,
    preview_enabled: bool,
    preview_task: Task<()>,
    preview_generation: u64,
    directories: BTreeMap<PathBuf, Vec<Entry>>,
    rows: Vec<(Entry, usize, bool)>,
    cursors: BTreeMap<PathBuf, project_files::Cursor>,
    refresh_restore: Option<project_files::ExpansionState>,
    restore_selection: Option<(PathBuf, bool)>,
    active: usize,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    loading: Option<PathBuf>,
    error: Option<Notice>,
    token: Cancel,
    generation: u64,
    task: Task<()>,
}
impl EventEmitter<Open> for ProjectView {}
impl Drop for ProjectView { fn drop(&mut self) { self.token.cancel(); } }
impl ProjectView {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("up", Up, Some("ProjectBrowser")), KeyBinding::new("down", Down, Some("ProjectBrowser")),
            KeyBinding::new("home", First, Some("ProjectBrowser")), KeyBinding::new("end", Last, Some("ProjectBrowser")),
            KeyBinding::new("enter", OpenFile, Some("ProjectBrowser")), KeyBinding::new("right", Expand, Some("ProjectBrowser")),
            KeyBinding::new("left", Collapse, Some("ProjectBrowser")),
        ]);
        let mut s = Self { root, preview_enabled: plico::layout_preferences::load_file_preview(), preview_task: Task::ready(()), preview_generation: 0, directories: BTreeMap::new(), cursors: BTreeMap::new(), refresh_restore: None, restore_selection: None, rows: vec![], active: 0, focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(), loading: None, error: None, token: Cancel::default(), generation: 0, task: Task::ready(()) };
        s.rebuild(); s.expand(PathBuf::new(), cx); s
    }
    fn rebuild(&mut self) {
        fn walk(path: &Path, depth: usize, dirs: &BTreeMap<PathBuf, Vec<Entry>>, cursors: &BTreeMap<PathBuf, project_files::Cursor>, rows: &mut Vec<(Entry, usize, bool)>) {
            if let Some(entries) = dirs.get(path) {
                for entry in entries {
                    rows.push((entry.clone(), depth, false));
                    if entry.kind == Kind::Directory { walk(&entry.path, depth+1, dirs, cursors, rows); }
                }
                if cursors.contains_key(path) { rows.push((Entry { path: path.to_owned(), kind: Kind::Directory }, depth, true)); }
            }
        }
        let selected = self.rows.get(self.active).map(|(e, _, more)| (e.path.clone(), *more));
        self.rows = vec![(Entry { path: PathBuf::new(), kind: Kind::Directory }, 0, false)];
        walk(Path::new(""), 1, &self.directories, &self.cursors, &mut self.rows);
        self.restore_path(selected);
    }
    fn restore_path(&mut self, selected: Option<(PathBuf, bool)>) {
        self.active = selected.and_then(|(mut path, more)| {
            if let Some(index) = self.rows.iter().position(|(e, _, m)| e.path == path && *m == more) { return Some(index); }
            loop {
                if let Some(index) = self.rows.iter().position(|(e, _, m)| e.path == path && !*m) { return Some(index); }
                if !path.pop() { return None; }
            }
        }).unwrap_or(0);
    }
    fn select(&mut self, index: usize) {
        self.active = index;
        // A later explicit selection takes precedence over a failed refresh's
        // saved selection, even when it is the same visible fallback row.
        self.restore_selection = None;
    }
    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.cancel_preview();
        self.token.cancel(); self.generation += 1; self.token = Cancel::default();
        let generation = self.generation; let token = self.token.clone(); let root = self.root.clone();
        let mut expanded = self.refresh_restore.take().unwrap_or_else(|| {
            let mut expanded: project_files::ExpansionState = self.directories.iter()
                .map(|(path, entries)| (path.clone(), entries.last().cloned())).collect();
            // Refresh while the first page is still loading preserves the
            // requested expansion instead of mistaking it for a closed tree.
            if let Some(path) = &self.loading { expanded.entry(path.clone()).or_insert(None); }
            expanded
        });
        // A failed initial/root read has no directory state but must be retryable.
        if expanded.is_empty() && self.error.is_some() { expanded.insert(PathBuf::new(), None); }
        self.refresh_restore = Some(expanded.clone());
        if self.restore_selection.is_none() {
            self.restore_selection = self.rows.get(self.active).map(|(e, _, more)| (e.path.clone(), *more));
        }
        self.loading = Some(PathBuf::new()); self.error = None;
        self.task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                Project::new(root)?.refresh_expanded(&expanded, project_files::DISPLAY_NODE_LIMIT, &token)
            }).await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.generation { return; }
                s.loading = None;
                match result {
                    Ok(tree) => {
                        s.directories = tree.directories; s.cursors = tree.cursors;
                        s.error = tree.error.map(directory_error_text).or_else(|| tree.limited.then(|| "Collapse folders before opening more files".into()));
                    }
                    Err(error) => { s.directories.clear(); s.cursors.clear(); s.error = Some(directory_error_text(error)); }
                }
                s.rebuild();
                if let Some(selected) = s.restore_selection.clone() { s.restore_path(Some(selected)); }
                if s.error.is_none() { s.refresh_restore = None; s.restore_selection = None; }
                cx.notify();
            });
        }); cx.notify();
    }
    fn expand(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.directories.contains_key(&path) { return; }
        self.load_directory(path, false, cx);
    }
    fn load_directory(&mut self, path: PathBuf, more: bool, cx: &mut Context<Self>) {
        if self.loading.as_ref() == Some(&path) { return; }
        let cursor = if more { let Some(cursor) = self.cursors.get(&path).cloned() else { return; }; Some(cursor) } else { None };
        self.refresh_restore = None; self.restore_selection = None;
        self.token.cancel(); self.generation += 1; self.token = Cancel::default();
        let generation = self.generation; let token = self.token.clone(); let root = self.root.clone();
        self.loading = Some(path.clone()); self.error = None;
        self.task = cx.spawn(async move |this, cx| {
            let query = path.clone();
            let result = cx.background_spawn(async move { Project::new(root)?.children_page(&query, cursor.as_ref(), &token) }).await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.generation { return; }
                s.loading = None;
                match result {
                    Ok(page) => {
                        let used: usize = s.directories.values().map(Vec::len).sum();
                        if used + page.entries.len() > project_files::DISPLAY_NODE_LIMIT { s.error = Some("Collapse folders before opening more files".into()); }
                        else {
                            let first_new = if more { page.entries.first().map(|e| e.path.clone()) } else { None };
                            if let Some(cursor) = page.next { s.cursors.insert(path.clone(), cursor); } else { s.cursors.remove(&path); }
                            s.directories.entry(path).or_default().extend(page.entries); s.rebuild();
                            if let Some(first) = first_new {
                                if let Some(index) = s.rows.iter().position(|(e, _, more)| !more && e.path == first) {
                                    s.active = index; s.scroll.scroll_to_item(index, ScrollStrategy::Top);
                                }
                            }
                        }
                    }
                    Err(project_files::Error::ChangedDuringRead) => {
                        s.directories.retain(|p, _| !p.starts_with(&path));
                        s.cursors.retain(|p, _| !p.starts_with(&path)); s.rebuild();
                        s.error = Some("Folder changed. Refresh to reload its files.".into());
                    }
                    Err(e) => s.error = Some(error_text(e)),
                }
                cx.notify();
            });
        }); cx.notify();
    }
    fn collapse(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.refresh_restore = None; self.restore_selection = None;
        self.cancel_preview();
        self.token.cancel(); self.generation += 1; self.task = Task::ready(()); self.loading = None;
        self.directories.retain(|p, _| !p.starts_with(path)); self.cursors.retain(|p, _| !p.starts_with(path)); self.rebuild(); cx.notify();
    }
    fn open(&mut self, cx: &mut Context<Self>) {
        self.restore_selection = None;
        self.cancel_preview();
        let Some((entry, _, more)) = self.rows.get(self.active).cloned() else { return; };
        if more { self.load_directory(entry.path, true, cx); return; }
        match entry.kind {
            Kind::Directory => if self.directories.contains_key(&entry.path) { self.collapse(&entry.path, cx); } else { self.expand(entry.path, cx); },
            Kind::File => cx.emit(Open(entry.path, false, self.preview_generation)),
            _ => { self.error = Some("Linked or special files are not opened".into()); cx.notify(); }
        }
    }
    pub fn cancel_preview(&mut self) {
        self.preview_generation = self.preview_generation.wrapping_add(1);
        self.preview_task = Task::ready(());
    }
    pub fn is_preview_current(&self, generation: u64) -> bool {
        self.preview_enabled && self.preview_generation == generation
    }
    pub fn set_preview_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.preview_enabled = enabled; self.cancel_preview(); cx.notify();
    }
    fn preview(&mut self, delayed: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_preview();
        if !self.preview_enabled { return; }
        let Some((entry, _, false)) = self.rows.get(self.active) else { return; };
        if entry.kind != Kind::File { return; }
        let path = entry.path.clone();
        let generation = self.preview_generation;
        if !delayed { cx.emit(Open(path, true, generation)); return; }
        self.preview_task = cx.spawn_in(window, async move |this, wx| {
            wx.background_executor().timer(std::time::Duration::from_millis(100)).await;
            let _ = this.update_in(wx, |s, window, cx| {
                if s.is_preview_current(generation) && s.focus.is_focused(window) { cx.emit(Open(path, true, generation)); }
            });
        });
    }
    fn step(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.select(if next { (self.active+1).min(self.rows.len().saturating_sub(1)) } else { self.active.saturating_sub(1) });
        self.scroll.scroll_to_item(self.active, ScrollStrategy::Top); self.preview(true, window, cx); cx.notify();
    }
}
impl Render for ProjectView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full().bg(rgb(0x18191a)).rounded(px(10.)).overflow_hidden().key_context("ProjectBrowser").track_focus(&self.focus)
            .child(h_flex().h(px(36.)).px_3().child(t("Project")).child(div().flex_1())
                .child(Button::new("project-refresh").ghost().icon(Icon::default().path("jb/refresh_dark.svg").size(px(16.)))
                    .tooltip(t("Refresh")).on_click(cx.listener(|s, _, _, cx| s.refresh(cx)))))
            .on_action(cx.listener(|s, _: &Up, w, cx| s.step(false, w, cx)))
            .on_action(cx.listener(|s, _: &Down, w, cx| s.step(true, w, cx)))
            .on_action(cx.listener(|s, _: &First, w, cx| { s.select(0); s.scroll.scroll_to_item(0, ScrollStrategy::Top); s.preview(true, w, cx); cx.notify(); }))
            .on_action(cx.listener(|s, _: &Last, w, cx| { s.select(s.rows.len().saturating_sub(1)); s.scroll.scroll_to_item(s.active, ScrollStrategy::Top); s.preview(true, w, cx); cx.notify(); }))
            .on_action(cx.listener(|s, _: &OpenFile, _, cx| s.open(cx)))
            .on_action(cx.listener(|s, _: &Expand, _, cx| { if let Some((e, _, more)) = s.rows.get(s.active).cloned() { if more { s.load_directory(e.path, true, cx); } else if e.kind == Kind::Directory { s.expand(e.path, cx); } } }))
            .on_action(cx.listener(|s, _: &Collapse, _, cx| { if let Some((e, _, _)) = s.rows.get(s.active).cloned() { s.collapse(&e.path, cx); } }))
            .child(uniform_list("project-tree", self.rows.len(), cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                range.map(|i| {
                    let (entry, depth, more) = s.rows[i].clone();
                    let expanded = s.directories.contains_key(&entry.path);
                    let name = if entry.path.as_os_str().is_empty() { s.root.file_name().unwrap_or_default() } else { entry.path.file_name().unwrap_or_default() }.to_string_lossy().into_owned();
                    let name = if more { t("Load more files…").to_owned() } else { name };
                    h_flex().id(("project-row", i)).w_full().h(px(26.)).pl(px(8. + depth as f32 * 14.)).gap_1().min_w_0()
                        .when(i == s.active, |row| row.bg(rgb(0x2e4d89)))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |s, _, w, cx| { s.select(i); s.focus.focus(w, cx); cx.notify(); }))
                        .on_click(cx.listener(move |s, e: &ClickEvent, w, cx| { s.select(i); if more || e.click_count() == 2 { s.open(cx); } else { s.preview(false, w, cx); } }))
                        .child(div().id(("project-expand", i)).w(px(16.)).when(!more && entry.kind == Kind::Directory, |d| d
                            .child(Icon::default().path(if expanded {"jb/chevronDown_dark.svg"}else{"jb/chevronRight_dark.svg"}).size(px(16.)))
                            .on_click(cx.listener(move |s, _, _, cx| { cx.stop_propagation(); s.select(i); s.open(cx); }))))
                        .when(!more, |row| row.child(img(if entry.kind == Kind::Directory {"jb/folder_dark_4x.svg"}else{crate::ui::file_icon(&entry.path)}).size(px(16.)).flex_shrink_0()))
                        .child(div().truncate().child(name))
                }).collect::<Vec<_>>()
            })).track_scroll(&self.scroll).flex_1().min_h_0())
            .when(self.loading.is_some(), |v| v.child(div().px_3().child(t("Loading files…"))))
            .when_some(self.error.clone(), |v, error| v.child(v_flex().px_3().py_2().gap_1()
                .child(div().text_color(rgb(0xf2a6a6)).child(if matches!(error, Notice::Io(_)) { t("Could not read project folder").to_owned() } else { error.text() }))
                .when(error.retryable(), |v| v.child(Button::new("retry-project-tree").link().small().label(t("Refresh"))
                    .on_click(cx.listener(|s, _, _, cx| s.refresh(cx)))))
                .when_some(error.details(), |v, details| v.child(crate::error_ui::details_button("project-tree-error", details)))))
    }
}

#[derive(Clone, Copy)]
struct ReadingPosition {
    start: Position,
    end: Position,
    reversed: bool,
    scroll: gpui_kit::Point<Pixels>,
}

pub struct FileTab {
    pub history: bool,
    history_view: Option<Entity<crate::history_view::HistoryView>>,
    history_position: Option<crate::history_view::ReadingState>,
    jump_line: Option<usize>,
    pub path: PathBuf,
    pub preview: bool,
    request_focus: bool,
    root: PathBuf,
    viewer: Option<Entity<EditorState>>,
    reading_position: Option<ReadingPosition>,
    pending: Option<Vec<u8>>,
    pub documents: crate::editing::Documents,
    editing: Option<Entity<crate::editing::Editing>>,
    editing_subscriptions: Vec<Subscription>,
    status: Notice,
    token: Cancel,
    generation: u64,
    task: Task<()>,
}
impl EventEmitter<HistoryRequested> for FileTab {}
impl EventEmitter<FileSaved> for FileTab {}
impl Drop for FileTab { fn drop(&mut self) { self.token.cancel(); } }
impl FileTab {
    pub fn new(root: PathBuf, path: PathBuf) -> Self { Self { root, path, history: false, history_view: None, history_position: None, jump_line: None, preview: false, request_focus: true, viewer: None, reading_position: None, pending: None, documents: Default::default(), editing: None, editing_subscriptions: Vec::new(), status: Notice::Label(""), token: Cancel::default(), generation: 0, task: Task::ready(()) } }
    fn attach_document(&mut self, editing: Entity<crate::editing::Editing>, cx: &mut Context<Self>) {
        self.editing_subscriptions.clear();
        self.viewer = Some(editing.read(cx).editor.clone());
        if editing.read(cx).edited { self.preview = false; }
        self.editing_subscriptions.push(cx.observe(&editing, |s, _, cx| {
            if s.editing.as_ref().is_some_and(|d| d.read(cx).edited) { s.preview = false; }
            cx.notify();
        }));
        self.editing_subscriptions.push(cx.subscribe(&editing, |_, _, _: &crate::editing::Saved, cx| cx.emit(FileSaved)));
        self.editing = Some(editing);
    }
    pub fn dirty(&self, cx: &App) -> bool { self.editing.as_ref().is_some_and(|d| d.read(cx).pending(cx)) }
    pub fn save(&mut self, cx: &mut Context<Self>) { if let Some(d) = &self.editing { d.update(cx, |d,cx| d.save(cx)); } }
    pub fn history(root: PathBuf, path: PathBuf) -> Self { let mut tab = Self::new(root, path); tab.history = true; tab }
    pub fn history_source_request(&self,cx:&App)->Option<crate::history_view::SourceRequest>{self.history_view.as_ref()?.read(cx).source_request(cx)}
    pub fn jump_to_line(&mut self, line: usize, cx: &mut Context<Self>) { self.jump_line=Some(line); self.reading_position=None;cx.notify(); }
    fn history_requested(&mut self, selected: bool, cx: &mut Context<Self>) {
        if self.history { return; }
        let selection = if selected {
            let Some(viewer)=&self.viewer else{return;};let state=viewer.read(cx);let range=state.selected_range();
            let start=state.text().offset_to_position(range.start).line as usize;
            let end=state.text().offset_to_position(range.end).line as usize;
            Some(plico::file_history::Selection{text:state.text().to_string(),lines:start..end+1})
        } else { None };
        cx.emit(HistoryRequested{path:self.path.clone(),selection});
    }
    fn discard_reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(document)=&self.editing {
            document.update(cx,|d,cx|d.reload(window,cx));
        } else { self.activate(cx); }
    }
    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.as_ref().is_some_and(|d| d.read(cx).saving()) { return; }
        if !self.dirty(cx) { self.discard_reload(window,cx); return; }
        let owner=cx.entity().downgrade();
        window.open_dialog(cx, move |dialog,_,_| { dialog
            .title(t("Reload from Disk"))
            .child(t("Discard your unsaved changes and reload this file from disk?"))
            .on_ok({ let owner=owner.clone(); move|_,window,cx| {
                let _=owner.update(cx,|s,cx|s.discard_reload(window,cx)); true
            }})
            .footer(gpui_kit::component::h_flex().justify_end().gap_2()
                .child(gpui_kit::component::dialog::DialogAction::new().child(Button::new("reload-confirm").primary().label(t("Reload file"))))
                .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b|b.label(t("Cancel")))))
        });
    }
    pub fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(viewer) = &self.viewer {
            if viewer.read(cx).focus_handle(cx).is_focused(window) {
                viewer.update(cx, |state, cx| state.unselect(window, cx));
            }
        }
    }
    pub fn deactivate(&mut self, cx: &mut Context<Self>) {
        if let Some(view)=&self.history_view {self.history_position=Some(view.read(cx).reading_state());}
        if let Some(viewer) = &self.viewer {
            let state = viewer.read(cx);
            let range = state.selected_range();
            self.reading_position = Some(ReadingPosition {
                start: state.text().offset_to_position(range.start),
                end: state.text().offset_to_position(range.end),
                reversed: state.cursor() == range.start && !range.is_empty(),
                scroll: state.scroll_offset(),
            });
        }
        self.token.cancel(); self.generation += 1; self.task = Task::ready(());
        self.save(cx);
        if !self.editing.as_ref().is_some_and(|d| d.read(cx).edited || d.read(cx).pending(cx)) {
            self.viewer = None; self.editing = None; self.editing_subscriptions.clear();
        }
        self.pending = None; self.history_view=None; cx.notify();
    }
    pub fn activate(&mut self, cx: &mut Context<Self>) { self.activate_with_focus(true, cx); }
    pub fn focus_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_focus = true;
        if let Some(viewer) = &self.viewer { viewer.read(cx).focus_handle(cx).focus(window, cx); }
    }
    pub fn activate_with_focus(&mut self, request_focus: bool, cx: &mut Context<Self>) {
        self.request_focus = request_focus;
        self.deactivate(cx);
        if self.history { self.history_view=Some(cx.new(|cx|crate::history_view::HistoryView::new(plico::git::Repository::new(self.root.clone()),self.path.clone(),None,self.history_position.clone(),cx)));self.status=Notice::Label("");return; }
        if let Some(editing) = self.editing.clone().or_else(|| self.documents.get(&self.path)) {
            self.attach_document(editing, cx);
            self.status = Notice::Label(""); cx.notify(); return;
        }
        self.token = Cancel::default(); self.status = "Loading file…".into();
        let token = self.token.clone(); let root = self.root.clone(); let path = self.path.clone(); let generation = self.generation;
        self.task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { Project::new(root)?.read_bytes(&path, &token) }).await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.generation { return; }
                match result {
                    Ok(bytes) => { s.pending = Some(bytes); s.status = Notice::Label(""); }
                    Err(e) => s.status = error_text(e),
                }
                cx.notify();
            });
        }); cx.notify();
    }
}
impl Render for FileTab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(bytes) = self.pending.take() {
            match plico::file_document::Document::new(bytes) {
            Err(error) => { self.status = match error {
                plico::file_document::Error::Binary => "Binary file cannot be displayed as text".into(),
                _ => "File encoding is not supported".into(),
            }; }
            Ok(document) => {
            let position = self.reading_position;
            let language = file_language(&self.path);
            let editing = self.documents.get_or_create(self.root.clone(), self.path.clone(), document, |text, cx| cx.new(|cx| {
                let mut state = EditorState::new(window, cx).tab_size(gpui_kit::component::input::TabSize { tab_size: 4, hard_tabs: false }).default_value(text.to_owned()).line_number(true).language(language);
                state.set_readonly(false, cx);
                if let Some(position) = position {
                    let start = state.text().position_to_offset(&position.start);
                    let end = state.text().position_to_offset(&position.end);
                    state.set_selected_range_with_direction(start..end, position.reversed, cx);
                    state.set_scroll_offset(position.scroll, cx);
                }
                state
            }), cx);
            let viewer = editing.read(cx).editor.clone();
            self.attach_document(editing, cx);
            if self.request_focus { viewer.read(cx).focus_handle(cx).focus(window, cx); } self.viewer = Some(viewer);
            }
            }
        }
        if self.history {
            if self.request_focus {
                if let Some(history) = &self.history_view { history.read(cx).focus_handle(cx).focus(window, cx); }
                self.request_focus = false;
            }
            return div().size_full().key_context("ProjectFileTab").children(self.history_view.clone()).into_any_element();
        }
        // Keep the requested line until asynchronous file loading creates the editor.
        if let Some(viewer) = self.viewer.as_ref() {
            if self.request_focus { viewer.read(cx).focus_handle(cx).focus(window, cx); self.request_focus = false; }
            if let Some(line) = self.jump_line.take() {
                viewer.update(cx, |s, cx| s.set_cursor_position(Position::new(line as u32, 0), window, cx));
            }
        }
        v_flex().key_context("ProjectFileTab").size_full().bg(rgb(0x18191a))
            .on_action(cx.listener(|s,_:&SaveFile,_,cx|s.save(cx)))
            .on_action(cx.listener(|s,_:&ShowFileHistory,_,cx|s.history_requested(false,cx)))
            .on_action(cx.listener(|s,_:&ShowSelectionHistory,_,cx|s.history_requested(true,cx)))
            .child(h_flex().h(px(36.)).flex_shrink_0().px_3().gap_2().child(self.path.to_string_lossy().into_owned()).child(div().flex_1())
                .child(Button::new("reload-file").ghost().icon(Icon::default().path("jb/refresh_dark.svg").size(px(16.))).tooltip(t("Reload file"))
                    .on_click(cx.listener(|s, _, w, cx| s.reload(w,cx)))))
            .when_some(self.editing.as_ref().and_then(|d|d.read(cx).error.clone()), |v, error| v.child(div().flex_shrink_0().px_3().py_2().text_color(rgb(0xf2a6a6)).child(error)))
            .when_some(self.viewer.clone(), |v, viewer| v.child(
                div().id("source-file-editor").w_full().flex_1().min_h_0()
                    .child(Editor::new(&viewer).context_menu(|_,_,_|gpui_kit::component::native_menu::NativeMenu::new())
                        .readonly(self.documents.locked()).appearance(false).bordered(false).h_full().font_family("JetBrains Mono").text_size(px(13.)))
                    .context_menu(|menu,_,_|menu
                        .menu(t("Cut"),Box::new(gpui_kit::base::input::Cut))
                        .menu(t("Copy"),Box::new(gpui_kit::base::input::Copy))
                        .menu(t("Paste"),Box::new(gpui_kit::base::input::Paste))
                        .menu(t("Select All"),Box::new(gpui_kit::base::input::SelectAll)).separator()
                        .menu(t("Show History"),Box::new(ShowFileHistory))
                        .menu(t("Show History for Selection…"),Box::new(ShowSelectionHistory)))
            ))
            .when(!matches!(self.status, Notice::Label("")), |v| v.child(v_flex().flex_1().min_h_0().items_center().justify_center().gap_3().px_4()
                .child(div().text_center().child(self.status.text()))
                .when(self.status.retryable(), |v| v.child(Button::new("retry-project-file").link().label(t("Reload file"))
                    .on_click(cx.listener(|s, _, _, cx| s.activate(cx)))))
                .when_some(self.status.details(), |v, details| v.child(crate::error_ui::details_button("project-file-error", details)))))
            .into_any_element()
    }
}
fn directory_error_text(error: project_files::Error) -> Notice {
    match error {
        project_files::Error::ChangedDuringRead => "Folder changed. Refresh to reload its files.".into(),
        error => error_text(error),
    }
}
fn error_text(error: project_files::Error) -> Notice {
    use project_files::Error::*;
    match error {
        Cancelled => "Cancelled".into(), InvalidPath => "Invalid project path".into(), LinkedPath => "Linked or special files are not opened".into(),
        NotDirectory => "Folder is no longer available".into(), NotFile => "Not a regular file".into(),
        DirectoryTooLarge => "Folder exceeds the display limit".into(), FileTooLarge => "File exceeds the 5 MiB preview limit".into(),
        ChangedDuringRead => "File changed while reading; reload it".into(), Io(e) => Notice::Io(e.to_string()),
    }
}

#[derive(Clone)]
enum Notice { Label(&'static str), Io(String) }
impl From<&'static str> for Notice { fn from(label: &'static str) -> Self { Self::Label(label) } }
impl Notice {
    fn details(&self) -> Option<String> { if let Self::Io(error) = self { Some(error.clone()) } else { None } }
    fn retryable(&self) -> bool {
        matches!(self, Self::Io(_) | Self::Label("File changed while reading; reload it" | "Folder changed. Refresh to reload its files." | "Folder is no longer available" | "Not a regular file"))
    }
    fn text(&self) -> String {
        match self { Self::Label(label) => t(label).to_owned(), Self::Io(_) => t("Could not read project file").to_owned() }
    }
}

pub(crate) fn file_language(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "rs" => "rust", "py" | "pyi" => "python", "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" => "typescript", "json" | "jsonc" => "json", "toml" => "toml", "yml" | "yaml" => "yaml",
        "md" | "markdown" => "markdown", "html" | "htm" => "html", "css" => "css", "go" => "go", "java" => "java",
        "c" | "h" => "c", "cpp" | "cc" | "cxx" | "hpp" => "cpp", "sh" | "bash" => "bash", "xml" | "svg" => "xml",
        _ => "plaintext",
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod source_navigation_tests {
    extern crate gpui_kit as gpui;
    use super::FileTab;
    use std::path::PathBuf;
    use gpui::AppContext as _;
    #[gpui::test]
    fn source_jump_waits_for_the_editor_to_load(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut tab = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| {
                let mut tab = FileTab::new(PathBuf::new(), PathBuf::from("source.txt"));
                tab.jump_to_line(2, cx);
                tab
            });
            tab = Some(view.clone());
            gpui::component::Root::new(view, window, cx)
        });
        let tab = tab.unwrap();
        cx.update(|window, cx| { let _ = window.draw(cx); });
        tab.read_with(cx, |tab, _| assert_eq!(tab.jump_line, Some(2), "loading must not consume the requested line"));
        cx.update(|_, cx| tab.update(cx, |tab, cx| {
            tab.pending = Some("first\nsecond\nselected\nlast\n".into());
            cx.notify();
        }));
        cx.run_until_parked();
        cx.update(|window, cx| { let _ = window.draw(cx); });
        tab.read_with(cx, |tab, cx| {
            assert!(tab.jump_line.is_none());
            assert_eq!(tab.viewer.as_ref().unwrap().read(cx).cursor(), 13);
        });
    }
}
