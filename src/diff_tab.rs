use crate::diff_picker::FilePicker;
// A review tab owns its target and only retains a viewer while active.
use plico::i18n::{self, t};
use plico::{diff_target::{Target, Chain}, git::{Area, Cancel}, layout_preferences::{self, DiffPlace, DiffAppearance}};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::component::{popover::Popover, list::{List, ListState}, IndexPath, Disableable as _, Icon, v_flex, h_flex, button::{Button, ButtonVariants as _}, Sizable as _, diff::{Diff, DiffFile, DiffMode, DiffState, DiffHunkSeparator, DiffFileStatus, DiffChangeIndicator}};

#[derive(Clone)]
pub struct StashOrigin {
    pub selector: String,
    pub group: &'static str,
}
pub struct FileChanged(pub Target);
impl EventEmitter<FileChanged> for DiffTab {}
pub type SourceCallback = std::rc::Rc<dyn Fn((Target,bool,usize), &mut Window, &mut App)>;
pub struct DiffTab {
    pub documents: crate::editing::Documents,
    editable: Option<Entity<crate::editable_diff::EditableDiff>>,
    retained_document: Option<Entity<crate::editing::Editing>>,
    pending_editable: Option<plico::working_copy::Contents>,
    pub source_callback: Option<SourceCallback>,
    embedded: bool,
    split_ratio: f32,
    split_dragging: std::rc::Rc<std::cell::Cell<bool>>,
    navigation_boundary: Option<bool>,
    pending_navigation: Option<bool>,
    source_available: bool,
    last_source_position: Option<(bool,usize)>,
    unavailable: Option<&'static str>,
    pub snapshot_revision: u64,
    pub stash_origin: Option<StashOrigin>,
    pub target: Target,
    chain: Chain,
    picker: Option<Entity<ListState<FilePicker>>>,
    viewer: Option<Entity<DiffState>>,
    token: Cancel,
    generation: u64,
    task: Task<()>,
    loading: bool,
    preferred_mode: DiffMode,
    appearance: DiffAppearance,
    error: Option<String>,
    preference_error: Option<String>,
}
impl Drop for DiffTab { fn drop(&mut self) { self.token.cancel(); } }
impl DiffTab {
    pub fn new(target: Target, chain: Chain) -> Self {
        Self { navigation_boundary: None, pending_navigation: None, documents: Default::default(), editable: None, retained_document: None, pending_editable: None, source_callback: None, embedded: false, split_ratio: 0.5, split_dragging: Default::default(), source_available: false, last_source_position: None, unavailable: None, snapshot_revision: 0, stash_origin: None, target, chain, picker: None, viewer: None, token: Cancel::default(), generation: 0, task: Task::ready(()), loading: false, preferred_mode: DiffMode::Split, appearance: DiffAppearance::default(), error: None, preference_error: None }
    }
    fn set_split_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if self.split_ratio == ratio { return; }
        self.split_ratio = ratio;
        if let Some(editable) = &self.editable {
            editable.update(cx, |view, cx| { view.split_ratio = ratio; cx.notify(); });
        }
        cx.notify();
    }
    pub fn preview(target: Target, chain: Chain) -> Self {
        let mut tab = Self::new(target, chain); tab.embedded = true; tab
    }
    pub fn replace_target(&mut self, target: Target, chain: Chain, cx: &mut Context<Self>) {
        self.deactivate(cx);
        if self.target.identity().path != target.identity().path {self.retained_document=None;}
        self.target = target; self.chain = chain; self.unavailable = None;
    }
    pub fn invalidate_preview(&mut self, message: &'static str, cx: &mut Context<Self>) {
        self.deactivate(cx); self.unavailable = Some(message); cx.notify();
    }
    pub fn caption(&self) -> String {
        let name = self.target.change().path.file_name().unwrap_or_default().to_string_lossy();
        match &self.stash_origin {
            Some(origin) => format!("{} · {}: {}", origin.selector, t(origin.group), name),
            None => i18n::message(i18n::Message::RepositoryDiff(&name)),
        }
    }
    pub fn fork_chain(&self, target: &Target) -> Option<Chain> {
        self.chain.fork_for(target)
    }
    pub fn deactivate(&mut self, cx: &mut Context<Self>) {
        self.split_dragging.set(false);
        if let Some(view) = self.editable.take() {
            view.update(cx, |v,cx|v.deactivate(cx));
            if let Some(document) = view.read(cx).working.clone() {
            document.update(cx, |d,cx|d.save(cx));
            self.documents.retain_pending(self.target.identity().path.clone(), &document, cx);
            if document.read(cx).edited {self.retained_document=Some(document);}
            }
        }
        self.pending_editable = None;
        self.navigation_boundary = None;
        self.pending_navigation = None;
        self.token.cancel(); self.generation += 1; self.task = Task::ready(());
        self.picker = None; self.viewer = None; self.last_source_position=None; self.loading = false; cx.notify();
    }
    pub fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_with_focus(true, window, cx);
    }
    pub fn activate_with_focus(&mut self, focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.deactivate(cx);
        if self.unavailable.is_some() { return; }
        self.preferred_mode = if layout_preferences::load_diff_unified(self.place()) { DiffMode::Unified } else { DiffMode::Split };
        self.appearance = layout_preferences::load_diff_appearance(self.place());
        self.token = Cancel::default(); self.loading = true; self.error = None; self.source_available=false;
        let viewer = cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Split).with_context_lines(None).with_inline_unit(crate::diff_settings::inline_unit(self.appearance.emphasis)));
        if focus && !self.embedded { viewer.read(cx).focus_handle(cx).focus(window, cx); }
        cx.observe(&viewer, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&viewer,|s,_,event:&gpui_kit::component::diff::DiffEvent,cx|{
            if let gpui_kit::component::diff::DiffEvent::SelectionChanged(Some(range))=event {
                s.last_source_position=Some((range.end_side()==gpui_kit::component::diff::DiffSide::Original,range.end().saturating_sub(1)));cx.notify();
            }
        }).detach();
        self.viewer = Some(viewer);
        let editable = self.stash_origin.is_none() && self.target.working_tree_document().is_some();
        let target = self.target.clone(); let token = self.token.clone(); let generation = self.generation;
        self.task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                if editable {
                    let contents = plico::working_copy::read(&target,&token)?;
                    let path = target.identity().path.to_string_lossy();
                    let file = DiffFile::from_editor_sources(Some((&path,&contents.original)),Some((&path,&contents.working.comparison_text(contents.working.text()))))
                        .map_err(str::to_owned)?;
                    return Ok((vec![file],true,Some(contents)));
                }
                let source_available=std::fs::symlink_metadata(target.identity().repository.join(&target.identity().path)).is_ok_and(|m|m.file_type().is_file());
                if let Some(contents) = target.read_combined(&token) {
                    let contents = contents?;
                    let endpoints = target.identity().combined.as_ref().ok_or("Missing combined revisions")?;
                    let before_path = endpoints.before.as_ref().map(|r| r.path.to_string_lossy().into_owned());
                    let after_path = endpoints.after.as_ref().map(|r| r.path.to_string_lossy().into_owned());
                    let file = DiffFile::from_sources(before_path.as_deref().zip(contents.before.as_deref()),
                        after_path.as_deref().zip(contents.after.as_deref())).map_err(str::to_owned)?;
                    if token.cancelled() { return Err("Cancelled".to_owned()); }
                    return Ok((vec![file],source_available,None));
                }
                let text = target.read(&token)?;
                if token.cancelled() { return Err("Cancelled".to_owned()); }
                let files=if target.change().area == Area::Untracked { vec![DiffFile::added(target.change().label(), &text)] }
                else { DiffFile::parse(&text).map_err(|e| e.to_string())? };
                Ok((files,source_available,None))
            }).await;
            let _ = this.update(cx, |s, cx| {
                if s.generation != generation { return; }
                s.loading = false;
                match result {
                    Ok((files,available,editable)) => { s.pending_editable=editable; s.source_available=available&&!files.iter().any(DiffFile::is_binary); if let Some(viewer) = &s.viewer { viewer.update(cx, |v, cx| {
                        let single_side = files.len() == 1 && matches!(files[0].status(), DiffFileStatus::Added | DiffFileStatus::Deleted);
                        v.set_mode(if single_side { DiffMode::Unified } else { s.preferred_mode }, cx);
                        v.set_files(files, cx);
                    }); } },
                    Err(error) => { s.viewer = None; s.error = Some(error); },
                }
                cx.notify();
            });
        });
    }
    pub fn choose_file(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.unavailable.is_some() { return; }
        if let Some(target) = self.chain.select(index) {
            self.deactivate(cx);
            self.retained_document=None;
            self.target = target;
            cx.emit(FileChanged(self.target.clone()));
            self.activate(window, cx);
        }
    }
    pub fn dismiss_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picker = None;
        self.focus_viewer(window, cx);
        cx.notify();
    }
    pub fn focus_viewer(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editable.is_some() {
            if let Some(editable)=&self.editable {editable.update(cx,|v,cx|v.focus(window,cx)); return;}
        }
        if let Some(viewer) = &self.viewer { viewer.read(cx).focus_handle(cx).focus(window, cx); }
    }
    fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.unavailable.is_some() { return; }
        let delegate = FilePicker::new(self.chain.clone(), cx.entity().downgrade());
        let selected = delegate.initial_row();
        let picker = cx.new(|cx| {
            let mut state = ListState::new(delegate, window, cx).searchable(true);
            state.set_selected_index(selected.map(|row| IndexPath::default().row(row)), window, cx);
            state
        });
        picker.update(cx, |list, cx| list.focus(window, cx));
        self.picker = Some(picker); cx.notify();
    }
    pub fn move_file(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.unavailable.is_some() { return; }
        if let Some(target) = self.chain.step(next) {
            self.deactivate(cx);
            self.retained_document=None;
            self.target = target;
            cx.emit(FileChanged(self.target.clone()));
            self.activate(window, cx);
        }
    }
    fn place(&self) -> DiffPlace {
        if self.target.change().area == Area::Commit { DiffPlace::Log } else { DiffPlace::Changes }
    }
    fn set_view_mode(&mut self, mode: DiffMode, cx: &mut Context<Self>) {
        self.split_dragging.set(false);
        if let Err(error) = layout_preferences::save_diff_unified(self.place(), mode == DiffMode::Unified) {
            self.preference_error = Some(format!("Cannot save Diff view: {error}"));
            cx.notify();
            return;
        }
        self.preference_error = None;
        self.preferred_mode = mode;
        if let Some(viewer) = &self.viewer {
            viewer.update(cx, |v, cx| {
                // Rebased uses the same oneside viewer for additions/deletions in both tools.
                let single_side = v.files().len() == 1 && matches!(v.files()[0].status(), DiffFileStatus::Added | DiffFileStatus::Deleted);
                v.set_mode(if single_side { DiffMode::Unified } else { mode }, cx);
            });
        }
        cx.notify();
    }
    fn set_appearance(&mut self, value: DiffAppearance, cx: &mut Context<Self>) {
        if let Err(error) = layout_preferences::save_diff_appearance(self.place(), value) {
            self.preference_error = Some(format!("Cannot save Diff settings: {error}"));
            cx.notify(); return;
        }
        self.preference_error = None;
        self.appearance = value;
        if let Some(viewer) = &self.viewer {
            viewer.update(cx, |v, cx| v.set_inline_unit(crate::diff_settings::inline_unit(value.emphasis), cx));
        }
        cx.notify();
    }
    fn settings_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        crate::diff_settings::appearance_button(self.appearance, move |value, cx| {
            let _ = owner.update(cx, |s, cx| s.set_appearance(value, cx));
        })
    }
    fn open_source(&self,window:&mut Window,cx:&mut Context<Self>){
        if let (Some(callback),Some(request))=(self.source_callback.clone(),self.source_request(cx)) {
            // The owner may deactivate this DiffTab while opening Source. Finish
            // the toolbar's entity update before calling back into that owner.
            window.defer(cx,move|window,cx|callback(request,window,cx));
        }
    }
    pub fn source_request(&self, cx: &App) -> Option<(Target, bool, usize)> {
        if self.loading || self.error.is_some() || self.unavailable.is_some() || !self.source_available { return None; }
        if self.editable.is_some() {
            if let Some(editable)=&self.editable {
                let (original,line)=editable.read(cx).source_position(cx);
                return Some((self.target.clone(),original,line));
            }
        }
        let viewer=self.viewer.as_ref()?.read(cx);
        let selected=viewer.selected_lines();
        // Toolbar pointer focus clears the visual selection before click.
        // Navigation still uses the last explicit source position.
        let (original,line)=viewer.source_position().map(|p|(p.side()==gpui_kit::component::diff::DiffSide::Original,p.line().saturating_sub(1))).or_else(||selected.map(|r|(r.end_side()==gpui_kit::component::diff::DiffSide::Original,r.end().saturating_sub(1)))
            ).or(self.last_source_position).unwrap_or((self.target.change().status.starts_with('D'),0));
        Some((self.target.clone(),original,line))
    }
    pub fn navigate(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.error.is_some() || self.unavailable.is_some() { return; }
        let moved = if let Some(editable)=&self.editable {
            editable.update(cx,|v,cx|v.try_navigate(next,window,cx))
        } else if let Some(viewer)=&self.viewer {
            viewer.update(cx,|v,cx|v.try_move_change(next,cx))
        } else {false};
        if moved { self.navigation_boundary=None; cx.notify(); return; }
        let has_file=if next {self.chain.index()+1<self.chain.len()} else {self.chain.index()>0};
        if !has_file {self.navigation_boundary=None; cx.notify();return;}
        if self.navigation_boundary==Some(next) {
            self.move_file(next,window,cx);
            self.pending_navigation=Some(next);
        } else { self.navigation_boundary=Some(next); }
        cx.notify();
    }

}
impl Render for DiffTab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(contents) = self.pending_editable.take() {
            let editable = cx.new(|cx|crate::editable_diff::EditableDiff::new(&self.target,contents,self.documents.clone(),window,cx));
            cx.subscribe(&editable,|s,_,event:&crate::editable_diff::Compared,cx| {
                if let Some(viewer)=&s.viewer {viewer.update(cx,|v,cx|v.set_files(vec![event.0.clone()],cx));}
                cx.notify();
            }).detach();
            self.editable = Some(editable);
        }
        // Unified/one-sided/three-way comparisons keep the specialized viewer.
        // Ordinary historical split Diff uses two independent read-only editors.
        if self.preferred_mode != DiffMode::Split && self.editable.as_ref().is_some_and(|v|v.read(cx).working.is_none()) {
            self.editable.take();
        }
        if self.editable.is_none() && !self.loading && self.preferred_mode == DiffMode::Split {
            let file=self.viewer.as_ref().filter(|v|!v.read(cx).is_three_way())
                .and_then(|v|v.read(cx).files().first()).filter(|f|!matches!(f.status(),DiffFileStatus::Added|DiffFileStatus::Deleted))
                .filter(|f|!f.source(gpui_kit::component::diff::DiffSide::Original).is_empty() || !f.source(gpui_kit::component::diff::DiffSide::Modified).is_empty()).cloned();
            if let Some(file)=file {
                let focus=self.viewer.as_ref().is_some_and(|v|v.read(cx).focus_handle(cx).is_focused(window));
                let surface=cx.new(|cx|crate::editable_diff::EditableDiff::snapshot(&file,&self.target.identity().path,window,cx));
                if focus {surface.update(cx,|v,cx|v.focus(window,cx));}
                self.editable=Some(surface);
            }
        }
        if let Some(editable)=&self.editable {editable.update(cx,|v,cx| {v.appearance(self.appearance,window,cx);v.set_unified(self.preferred_mode==DiffMode::Unified,window,cx);});}
        if !self.loading {
            if let Some(first)=self.pending_navigation {
                if let Some(editable)=&self.editable {
                    if editable.update(cx,|v,cx|v.navigate_edge(first,window,cx)) {self.pending_navigation=None;}
                } else if let Some(viewer)=&self.viewer {
                    viewer.update(cx,|v,cx|v.move_to_edge_change(first,cx));
                    self.pending_navigation=None;
                }
            }
        }
        let id = self.target.identity();
        let short = |r: &str| r[..8.min(r.len())].to_owned();
        let (before, after) = if let Some(endpoints) = &id.combined {
            (endpoints.before.as_ref().map(|r| short(&r.revision)).unwrap_or_default(),
                endpoints.after.as_ref().map(|r| short(&r.revision)).unwrap_or_default())
        } else { match id.area {
            Area::Commit => (
                id.parents.first().map(|p| if id.parents.len() > 1 { format!("{} {}", t("First parent"), short(p)) } else { short(p) }).unwrap_or_else(|| t("Empty tree").into()),
                id.revision.as_deref().map(short).unwrap_or_default()),
            Area::WorkingTree => ("HEAD".into(), t("Working tree").into()),
            Area::Staged => ("HEAD".into(), t("Index").into()),
            Area::Unstaged => (t("Index").into(), t("Working tree").into()),
            Area::Untracked => (t("Empty file").into(), t("Working tree").into()),
            Area::Conflict => (t("Unmerged").into(), t("Working tree").into()),
        } };
        let after = match &self.stash_origin {
            Some(origin) => format!("{} · {} · {}", origin.selector, t(origin.group), after),
            None => after,
        };
        let old_path = id.old_path.as_deref().unwrap_or(&id.path);
        let file = self.viewer.as_ref().and_then(|v| v.read(cx).files().first());
        let status = file.map(|f| f.status());
        let metadata = file.map(|f| f.extended_headers().iter()
            .filter(|h| h.starts_with("old mode ") || h.starts_with("new mode "))
            .map(|h| h.as_ref()).collect::<Vec<&str>>().join(" · ")).unwrap_or_default();
        // UnifiedDiffViewer stacks both non-empty revision titles vertically.
        let unified = self.preferred_mode == DiffMode::Unified;
        let title_cell = |revision: String, path: Option<String>, locked: bool, fraction: f32| h_flex()
            .when(!unified, |cell| cell.w(relative(fraction)).flex_shrink_0()).when(unified, |cell| cell.w_full())
            .min_w_0().overflow_hidden().px_1().pt(px(2.)).pb(px(2.))
            .when(locked,|cell|cell.child(Icon::default().path("jb/locked_dark.svg").size(px(16.)).text_color(rgb(0xced0d6))))
            .child(div().flex_shrink_0().ml_1().text_color(rgb(0xbcbec4)).child(revision))
            .when_some(path, |cell, path| {
                let full_path = id.repository.join(&path).display().to_string();
                cell.child(div().flex_1().min_w_0().h_full().ml_2().text_color(rgb(0x73767c))
                    .child(crate::diff_path_title::path_title(path, full_path)))
            });
        let after_path = (status == Some(DiffFileStatus::Added) || old_path != id.path)
            .then(|| id.path.display().to_string());
        let titles = div().flex().when(unified, |row| row.flex_col())
            .w_full().flex_shrink_0().border_b_1().border_color(rgb(0x26282c))
            .when(status != Some(DiffFileStatus::Added), |row| row.child(title_cell(before, Some(old_path.display().to_string()), true, if status == Some(DiffFileStatus::Deleted) { 1. } else { self.split_ratio })))
            .when(status != Some(DiffFileStatus::Deleted), |row| row.child(title_cell(after, after_path, self.editable.as_ref().is_none_or(|v|v.read(cx).working.is_none()), if status == Some(DiffFileStatus::Added) { 1. } else { 1. - self.split_ratio })));
        let content = if let Some(message) = self.unavailable { div().p_4().child(t(message)).into_any_element() }
        else if self.loading { div().p_4().child(t("Reading Git data…")).into_any_element() }
        else if let Some(error) = self.error.clone() {
            v_flex().size_full().items_center().justify_center().gap_2().p_4()
                .child(t("Could not load the diff."))
                .child(h_flex().gap_3()
                    .child(Button::new("reload-diff").link().small().label(t("Reload diff"))
                        .on_click(cx.listener(|s, _, w, cx| s.activate(w, cx))))
                    .child(Button::new("copy-diff-error").link().small().label(t("Copy error details"))
                        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(error.clone())))))
                .into_any_element()
        }
        else if let Some(editable) = self.editable.as_ref() {
            editable.update(cx, |view, _| view.split_ratio = self.split_ratio);
            editable.clone().into_any_element()
        }
        else if let Some(viewer) = &self.viewer {
            if viewer.read(cx).files().is_empty() { div().p_4().child(t("No textual differences")).into_any_element() }
            else { Diff::new(viewer).split_ratio(self.split_ratio).size_full().soft_wrap(self.appearance.soft_wrap).line_number(self.appearance.line_numbers).change_indicator(DiffChangeIndicator::None).header_visible(false).hunk_separator(DiffHunkSeparator::LineInfo).into_any_element() }
        } else { div().into_any_element() };
        v_flex().relative().size_full().min_h_0().bg(rgb(0x191a1c))
            .on_action(cx.listener(|s,_:&crate::ui_actions::JumpToSource,window,cx|{cx.stop_propagation();s.open_source(window,cx);} ))
            .child(h_flex().h(px(40.)).mx(px(6.)).mt(px(2.)).rounded(px(6.)).border_1().border_color(rgb(0x26282c)).flex_shrink_0().gap_2().px_2().bg(rgb(0x212326))
                .child(Button::new("previous-diff").ghost().small().icon(Icon::default().path("jb/up_dark.svg").size(px(16.))).tooltip(t("Previous difference · Shift+F7")).disabled(self.loading || self.error.is_some() || self.unavailable.is_some()).on_click(cx.listener(|s, _, w, cx| s.navigate(false, w, cx))))
                .child(Button::new("next-diff").ghost().small().icon(Icon::default().path("jb/down_dark.svg").size(px(16.))).tooltip(t("Next difference · F7")).disabled(self.loading || self.error.is_some() || self.unavailable.is_some()).on_click(cx.listener(|s, _, w, cx| s.navigate(true, w, cx))))
                .child(Button::new("jump-to-source").ghost().small().icon(Icon::default().path("jb/edit_dark.svg"))
                    .tooltip(t("Jump to Source · F4")).disabled(self.loading || self.error.is_some() || self.unavailable.is_some() || self.viewer.is_none() || !self.source_available || self.source_callback.is_none())
                    .on_click(cx.listener(|s,_,window,cx|s.open_source(window,cx))))
                .child(div().h(px(20.)).border_l_1().border_color(rgb(0x43454a)))
                .when(!self.embedded, |toolbar| toolbar.child(Button::new("previous-file").ghost().small().icon(Icon::default().path("jb/left_dark.svg").size(px(16.)))
                    .tooltip(t("Previous file")).disabled(self.unavailable.is_some() || self.chain.index() == 0)
                    .on_click(cx.listener(|s, _, w, cx| s.move_file(false, w, cx))))
                .child(Popover::new("file-picker").open(self.picker.is_some())
                    .when_some(self.picker.as_ref().map(|list| list.read(cx).focus_handle(cx)), |popup, focus| popup.track_focus(&focus))
                    .trigger(Button::new("choose-file").link().small().child(div().text_color(rgb(0x6b9bfa)).child(i18n::message(i18n::Message::FilePosition { current: self.chain.index() + 1, total: self.chain.len() }))).disabled(self.unavailable.is_some() || self.chain.len() < 2))
                    .on_open_change(cx.listener(|s, open, w, cx| if *open { s.open_picker(w, cx); } else { s.dismiss_picker(w, cx); }))
                    .content({ let owner = cx.entity().downgrade(); move |_, _, cx| {
                        let picker = owner.upgrade().and_then(|tab| tab.read(cx).picker.clone());
                        div().w(px(360.)).h(px(320.)).children(picker.map(|list| List::new(&list).search_placeholder(t("Search files"))))
                    }}))
                .child(Button::new("next-file").ghost().small().icon(Icon::default().path("jb/right_dark.svg").size(px(16.)))
                    .tooltip(t("Next file")).disabled(self.unavailable.is_some() || self.chain.index() + 1 >= self.chain.len())
                    .on_click(cx.listener(|s, _, w, cx| s.move_file(true, w, cx)))))
                .child(div().flex_1())
                .child({
                    let owner = cx.entity().downgrade();
                    crate::diff_settings::mode_selector(self.preferred_mode, move |mode, cx| {
                        let _ = owner.update(cx, |s, cx| s.set_view_mode(mode, cx));
                    })
                })
                .child(self.settings_button(cx)))
            .when_some(self.preference_error.clone(), |view, error| view.child(h_flex().px_3().py_1().gap_2()
                .child(div().text_color(rgb(0xf2a6a6)).child(t("Could not save Diff preferences. Try the setting again.")))
                .child(Button::new("copy-diff-preference-error").link().small().label(t("Copy error details"))
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(error.clone()))))))
            .child(v_flex().relative().flex_1().min_h_0().w_full()
                .when(self.unavailable.is_none() && self.error.is_none(), |view| view.child(titles))
                .when(self.unavailable.is_none() && !metadata.is_empty(), |view| view.child(div().px_3().py_1().text_sm().child(metadata)))
                .child(div().flex_1().min_h_0().child(content))
                .when(!unified && !self.loading && self.error.is_none() && self.unavailable.is_none()
                    && (self.editable.is_some() || self.viewer.as_ref().is_some_and(|v| !v.read(cx).is_three_way() && v.read(cx).files().iter().any(|f| !matches!(f.status(), DiffFileStatus::Added | DiffFileStatus::Deleted)))), |view| {
                    let owner = cx.entity().downgrade();
                    view.child(crate::diff_splitter::handle(self.split_ratio, self.split_dragging.clone(), std::rc::Rc::new(move |ratio, cx| {
                        let _ = owner.update(cx, |s, cx| s.set_split_ratio(ratio, cx));
                    })))
                }))
            .when_some(self.navigation_boundary, |view,next|view.child(div().absolute().bottom(px(12.)).left(px(24.)).px_3().py_1().rounded(px(4.)).bg(rgb(0x34373c)).text_size(px(12.)).child(t(if next {"Press F7 again to go to the next file"} else {"Press Shift+F7 again to go to the previous file"}))))
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod editable_navigation_tests {
    extern crate gpui_kit as gpui;
    use super::{DiffTab, Target, Chain, Area, DiffState, DiffFile};
    use gpui::AppContext as _;
    struct Empty;
    impl gpui::Render for Empty {
        fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {gpui::div()}
    }
    #[gpui::test]
    fn readonly_tab_has_full_scroll_extent_without_f7(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut viewer = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let before = (0..500).map(|n| format!("const line{n} = {n};\n")).collect::<String>();
            let after = before.replace("= 250;", "= 999;");
            let state = cx.new(|cx| DiffState::new([DiffFile::from_sources(
                Some(("index.tsx", before.as_str())), Some(("index.tsx", after.as_str()))).unwrap()], cx));
            state.update(cx, |s, cx| {
                s.set_context_lines(None, cx);
                s.set_mode(gpui::component::diff::DiffMode::Split, cx);
            });
            viewer = Some(state.clone());
            struct Body(gpui::Entity<DiffState>);
            impl gpui::Render for Body {
                fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
                    use gpui::{Styled as _, ParentElement as _};
                    gpui::component::v_flex().size_full().min_h_0()
                        .child(gpui::div().h(gpui::px(64.)).flex_shrink_0())
                        .child(gpui::div().flex_1().min_h_0().child(
                            gpui::component::diff::Diff::new(&self.0).split_ratio(0.3).size_full()
                                .soft_wrap(false).header_visible(false)))
                }
            }
            let body = cx.new(|_| Body(state));
            gpui::component::Root::new(body, window, cx)
        });
        let viewer = viewer.unwrap();
        for (mode, width) in [
            (gpui::component::diff::DiffMode::Split, 1200.),
            (gpui::component::diff::DiffMode::Split, 900.),
            (gpui::component::diff::DiffMode::Unified, 900.),
        ] {
            cx.simulate_resize(gpui::size(gpui::px(width), gpui::px(800.)));
            viewer.update(cx, |s, cx| s.set_mode(mode, cx));
            for _ in 0..3 {
                cx.run_until_parked();
                cx.update(|window, cx| { let _ = window.draw(cx); });
            }
            viewer.read_with(cx, |state, _| {
                let (rows, viewport, travel) = state.test_scroll_metrics();
                assert!(rows >= 500, "full file must be projected: {rows}");
                assert!(viewport.height > gpui::px(0.) && viewport.height < gpui::px(800.), "bounded viewport: {viewport:?}");
                assert!(travel > viewport.height * 3., "500 lines must be scrollable without F7 ({mode:?}, width {width}): viewport={viewport:?}, travel={travel:?}");
            });
        }
    }
    #[gpui::test]
    fn readonly_f7_stops_at_last_change(cx: &mut gpui::TestAppContext) {
        let state=cx.new(|cx|DiffState::new([DiffFile::from_sources(
            Some(("a.txt","head\nold first\nmiddle\nold last\ntail\n")),
            Some(("a.txt","head\nnew first\nmiddle\nnew last\ntail\n"))).unwrap()],cx));
        state.update(cx,|s,cx| {
            s.set_context_lines(None,cx);
            assert!(s.try_move_change(true,cx));
            assert!(s.try_move_change(true,cx));
            let end=s.source_position();
            assert!(!s.try_move_change(true,cx));
            assert_eq!(s.source_position(),end);
            assert!(s.try_move_change(false,cx));
            assert!(!s.try_move_change(false,cx));
        });
    }
    #[gpui::test]
    fn f7_boundary_requires_second_press_and_does_not_wrap(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let (_,cx)=cx.add_window_view(|window,cx| {
            let change=|name:&str|plico::git::Change {area:Area::Unstaged,path:name.into(),old_path:None,status:"M".into()};
            let target=Target::new(plico::git::Repository::new("missing-fixture"),None,change("a.txt")).unwrap();
            let chain=Chain::new(&target,&[change("a.txt"),change("b.txt")]).unwrap();
            let tab=cx.new(|_|DiffTab::new(target,chain));
            tab.update(cx,|s,cx| {
                s.navigate(true,window,cx);
                assert_eq!(s.chain.index(),0);
                assert_eq!(s.navigation_boundary,Some(true));
                s.navigate(true,window,cx);
                assert_eq!(s.chain.index(),1);
                s.deactivate(cx);
                s.navigate(true,window,cx);
                s.navigate(true,window,cx);
                assert_eq!(s.chain.index(),1);
                assert_eq!(s.navigation_boundary,None);
            });
            let empty=cx.new(|_|Empty);
            gpui::component::Root::new(empty,window,cx)
        });
        cx.run_until_parked();
    }
    #[gpui::test]
    fn jump_to_source_defers_owner_switch_until_diff_update_ends(cx: &mut gpui::TestAppContext) {
        use std::{cell::Cell, rc::Rc};
        cx.update(gpui::component::init);
        let switched=Rc::new(Cell::new(false));
        let mut tab=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let target=Target::new(plico::git::Repository::new("fixture"),None,
                plico::git::Change { area:Area::Unstaged,path:"a.txt".into(),old_path:None,status:"M".into() }).unwrap();
            let chain=Chain::new(&target,&[target.change().clone()]).unwrap();
            let viewer=cx.new(|cx|DiffState::new([DiffFile::from_sources(Some(("a.txt","old\n")),Some(("a.txt","new\n"))).unwrap()],cx));
            let view=cx.new(|_| {let mut s=DiffTab::new(target,chain);s.viewer=Some(viewer);s.source_available=true;s});
            let weak=view.downgrade();let switched=switched.clone();
            view.update(cx,|s,_|s.source_callback=Some(Rc::new(move|_,_,cx| {
                weak.update(cx,|s,cx|s.deactivate(cx)).unwrap();
                switched.set(true);
            })));
            tab=Some(view.clone());
            let empty=cx.new(|_|Empty);
            gpui::component::Root::new(empty,window,cx)
        });
        cx.update(|window,cx|tab.as_ref().unwrap().update(cx,|s,cx|s.open_source(window,cx)));
        cx.run_until_parked();
        assert!(switched.get());
    }
}
