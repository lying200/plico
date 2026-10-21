//! Two-sided source comparison. Working-tree views share the live document;
//! historical snapshots own read-only editors without a save-capable document.
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::component::{h_flex, v_flex, input::{Editor, EditorState, InputEvent, TextDecoration, TextDecorationCollection, RopeExt as _}, diff::{DiffFile, DiffSide}};
use crate::editing::{Documents, Editing};
use plico::{working_copy::Contents, diff_target::Target};
use std::{sync::Arc, time::Duration};

mod partial;
pub struct Compared(pub DiffFile);
impl EventEmitter<Compared> for EditableDiff {}
pub struct EditableDiff {
    partial_template: Option<plico::partial_selection::Selection>,
    commit_scope: Option<(Entity<crate::commit_scope::Scope>,plico::commit_inclusion::Identity)>,
    pub working: Option<Entity<Editing>>,
    modified: Entity<EditorState>,
    source_layout: Option<(bool,bool)>,
    original: Entity<EditorState>,
    baseline: Arc<str>,
    comparison_baseline: Arc<str>,
    eol_only: bool,
    path: String,
    original_marks: TextDecorationCollection,
    working_marks: TextDecorationCollection,
    documents: Documents,
    generation: u64,
    pub split_ratio: f32,
    original_active: bool,
    pending_reveal: Option<usize>,
    active: bool,
    unified_mode: bool,
    unified: Option<crate::unified_editing::UnifiedEditing>,
    last_file: Option<DiffFile>,
    changes: Vec<(usize, usize)>,
    line_ranges: Vec<(std::ops::Range<usize>,std::ops::Range<usize>)>,
    appearance: Option<plico::layout_preferences::DiffAppearance>,
    task: Task<()>,
}
impl EditableDiff {
    pub fn update_baseline(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.comparison_baseline.as_ref() == text { return; }
        self.comparison_baseline = text.clone().into();
        self.baseline = text.replace("\r\n", "\n").replace('\r', "\n").into();
        self.original.update(cx, |e, cx| {
            let scroll = e.scroll_offset(); let selection = e.selected_range();
            let start = e.text().offset_to_position(selection.start);
            let end = e.text().offset_to_position(selection.end);
            let reversed = e.cursor() == selection.start && !selection.is_empty();
            e.set_value(self.baseline.to_string(), window, cx);
            let start = e.text().position_to_offset(&start); let end = e.text().position_to_offset(&end);
            e.set_selected_range_with_direction(start..end, reversed, cx);
            e.set_scroll_offset(scroll, cx);
        });
        self.recompare(cx);
    }
    pub fn update_snapshot(&mut self, file: &DiffFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.working.is_some() { return; }
        self.update_baseline(file.source(DiffSide::Original).to_string(), window, cx);
        self.modified.update(cx, |e, cx| {
            let scroll = e.scroll_offset(); let selection = e.selected_range();
            let start = e.text().offset_to_position(selection.start);
            let end = e.text().offset_to_position(selection.end);
            let reversed = e.cursor() == selection.start && !selection.is_empty();
            e.set_value(file.source(DiffSide::Modified).to_string(), window, cx);
            let start = e.text().position_to_offset(&start); let end = e.text().position_to_offset(&end);
            e.set_selected_range_with_direction(start..end, reversed, cx);
            e.set_scroll_offset(scroll, cx);
        });
        self.recompare(cx);
    }
    pub fn new(target: &Target, contents: Contents, documents: Documents, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let id = target.identity();
        let language = crate::project_view::file_language(&id.path);
        let comparison_baseline: Arc<str> = contents.original.into();
        let baseline: Arc<str> = if comparison_baseline.contains('\r') {comparison_baseline.replace("\r\n","\n").replace('\r',"\n").into()}else{comparison_baseline.clone()};
        let original = cx.new(|cx| {
            let mut editor = EditorState::new(window,cx).tab_size(gpui_kit::component::input::TabSize { tab_size: 4, hard_tabs: false }).default_value(baseline.to_string()).line_number(false).language(language);
            editor.set_readonly(true,cx); editor
        });
        let working = documents.get_or_create(id.repository.clone(),id.path.clone(),contents.working,|text,cx|cx.new(|cx|
            EditorState::new(window,cx).tab_size(gpui_kit::component::input::TabSize { tab_size: 4, hard_tabs: false }).default_value(text.to_owned()).line_number(true).language(language)),cx);
        let editor = working.read(cx).editor.clone();
        let original_marks=original.update(cx,|s,cx|s.create_decorations_collection(Vec::new(),cx));
        let working_marks=editor.update(cx,|s,cx|s.create_decorations_collection(Vec::new(),cx));
        cx.subscribe(&editor,|s,_,event:&InputEvent,cx| { if matches!(event,InputEvent::Change) { s.recompare(cx); } }).detach();
        cx.subscribe(&editor,|s,_,event:&InputEvent,_| { if matches!(event,InputEvent::Focus) { s.original_active=false; } }).detach();
        cx.subscribe(&original,|s,_,event:&InputEvent,_| { if matches!(event,InputEvent::Focus) { s.original_active=true; } }).detach();
        Self::bind_scroll(&editor,&original,cx);
        cx.subscribe_in(&cx.entity(),window,|s,_,event:&Compared,window,cx| {
            if !s.active {return;}
            s.last_file=Some(event.0.clone());
            s.update_unified(window,cx);
            cx.notify();
        }).detach();
        cx.on_release(|s,cx| s.clear_view(cx)).detach();
        cx.subscribe(&working,|s,_,_:&crate::editing::Reloaded,cx|s.recompare(cx)).detach();
        cx.observe(&working,|_,_,cx|cx.notify()).detach();
        let source_layout=Some(editor.read(cx).source_layout());
        let mut view=Self { commit_scope:None,partial_template:None, working:Some(working),source_layout,modified:editor,original,baseline,comparison_baseline,eol_only:false,path:id.path.to_string_lossy().into_owned(),original_marks,working_marks,documents,generation:0,split_ratio:0.5,original_active:false,pending_reveal:None,active:true,unified_mode:false,unified:None,last_file:None,changes:Vec::new(),line_ranges:Vec::new(),appearance:None,task:Task::ready(()) };
        view.recompare(cx); view
    }
    fn bind_scroll(editor:&Entity<EditorState>, original:&Entity<EditorState>, cx:&mut Context<Self>) {
        for (is_original, editor) in [(false,editor),(true,original)] {
            cx.subscribe(editor, move |s,_,event:&InputEvent,cx| {
                if !s.active || s.unified_mode { return; }
                match event {
                    InputEvent::LayoutChanged => cx.notify(),
                    InputEvent::ScrollChanged { previous, offset } => {
                        let peer = if is_original {s.modified.clone()} else {s.original.clone()};
                        let master = if is_original {s.original.clone()} else {s.modified.clone()};
                        let mapped = if previous.y != offset.y {
                            crate::diff_scroll::vertical_offset(master.read(cx),peer.read(cx),&s.line_ranges,is_original,offset.y)
                        } else {None};
                        peer.update(cx,|e,cx| {
                            let mut target=e.scroll_offset();
                            if previous.x != offset.x { target.x=offset.x; }
                            if let Some(y)=mapped { target.y=y; }
                            e.set_peer_scroll_offset(target,cx);
                        });
                        cx.notify();
                    }
                    _ => {}
                }
            }).detach();
        }
    }

    /// Historical snapshots never acquire an Editing document or save listener.
    pub fn snapshot(file:&DiffFile, path:&std::path::Path, window:&mut Window,cx:&mut Context<Self>) -> Self {
        let file=file.editor_coordinates();
        let language=crate::project_view::file_language(path);
        let baseline:Arc<str>=file.source(DiffSide::Original).into();
        let modified_text=file.source(DiffSide::Modified);
        let make=|text:String,numbers,window:&mut Window,cx:&mut App| cx.new(|cx| {
            let mut editor=EditorState::new(window,cx).tab_size(gpui_kit::component::input::TabSize {tab_size:4,hard_tabs:false})
                .default_value(text).line_number(numbers).language(language);
            editor.set_readonly(true,cx);editor
        });
        let original=make(baseline.to_string(),false,window,cx);
        let modified=make(modified_text.to_owned(),true,window,cx);
        let marks=|side| file.editor_change_highlights(side).into_iter().map(|(r,s,whole)| {
            let d=TextDecoration::new(r,s);if whole {d.whole_line()}else{d}
        }).collect();
        let original_marks=original.update(cx,|e,cx|e.create_decorations_collection(marks(DiffSide::Original),cx));
        let working_marks=modified.update(cx,|e,cx|e.create_decorations_collection(marks(DiffSide::Modified),cx));
        Self::bind_scroll(&modified,&original,cx);
        for (is_original,editor) in [(true,&original),(false,&modified)] {
            cx.subscribe(editor,move |s,_,event:&InputEvent,_|if matches!(event,InputEvent::Focus) {s.original_active=is_original;}).detach();
        }
        Self {commit_scope:None,partial_template:None,working:None,source_layout:None,modified,original,comparison_baseline:baseline.clone(),baseline,eol_only:false,
            path:path.to_string_lossy().into_owned(),original_marks,working_marks,documents:Documents::default(),
            generation:0,split_ratio:0.5,original_active:false,pending_reveal:None,active:true,unified_mode:false,unified:None,
            last_file:Some(file.clone()),changes:file.change_starts(),line_ranges:file.change_line_ranges(),appearance:None,task:Task::ready(())}
    }
    pub fn appearance(&mut self,value:plico::layout_preferences::DiffAppearance,window:&mut Window,cx:&mut Context<Self>) {
        if self.appearance==Some(value) {return;}
        let previous=self.appearance.replace(value);
        for (original,editor) in [(true,self.original.clone()),(false,self.modified.clone())] {
            editor.update(cx,|e,cx| {
                if previous.is_none_or(|p|p.soft_wrap!=value.soft_wrap) {e.set_soft_wrap(value.soft_wrap,window,cx);}
                if previous.is_none_or(|p|p.line_numbers!=value.line_numbers) {e.set_line_number(!original && value.line_numbers,window,cx);}
            });
        }
        if previous.is_none_or(|p|p.emphasis!=value.emphasis) {
            if self.working.is_some() {self.recompare(cx);}
            if let Some(file)=&self.last_file {
                for (side,collection) in [(DiffSide::Original,&self.original_marks),(DiffSide::Modified,&self.working_marks)] {
                    collection.set(file.editor_change_highlights_with_unit(side,crate::diff_settings::inline_unit(value.emphasis)).into_iter().map(|(r,s,whole)| {
                        let d=TextDecoration::new(r,s);if whole {d.whole_line()}else{d}
                    }).collect(),cx);
                }
            }
        }
    }
    fn update_unified(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        let Some(file)=&self.last_file else {return;};
        let source=self.modified.clone();
        if source.read(cx).text().to_string()!=file.source(DiffSide::Modified) {return;}
        if let Some(view)=&mut self.unified {view.refresh(file,window,cx);}
        else if self.unified_mode {
            self.unified=Some(crate::unified_editing::UnifiedEditing::new(file,source,self.documents.clone(),crate::project_view::file_language(std::path::Path::new(&self.path)),window,cx));
        }
    }
    pub fn set_unified(&mut self,unified:bool,window:&mut Window,cx:&mut Context<Self>) {
        if self.unified_mode==unified {return;}
        self.unified_mode=unified;
        if unified {
            self.modified.clone().update(cx,|e,cx|{let _=e.set_block_gaps_preserving_position(Vec::new(),cx);});
            self.update_unified(window,cx);
        } else {self.realign(cx);}
        cx.notify();
    }
    pub fn deactivate(&mut self, cx: &mut Context<Self>) {
        self.task=Task::ready(());
        self.clear_view(cx);
    }
    fn clear_view(&mut self, cx: &mut App) {
        if !self.active {return;}
        self.active=false;
        self.original_marks.clear(cx); self.working_marks.clear(cx);
        let editor = self.modified.clone();
        editor.update(cx,|e,cx| {
            let _=e.set_block_gaps_preserving_position(Vec::new(),cx);
            if let Some(layout)=self.source_layout.take() {e.restore_source_layout(layout,cx);}
        });
    }
    pub fn focus(&self,window:&mut Window,cx:&mut App) { if self.unified_mode {if let Some(v)=&self.unified {v.editor.read(cx).focus_handle(cx).focus(window,cx);return;}} self.modified.read(cx).focus_handle(cx).focus(window,cx); }
    pub fn source_position(&self, cx: &App) -> (bool, usize) {
        if self.unified_mode {if let Some(v)=&self.unified {return v.source_position(&self.baseline,cx);}}
        let editor = if self.original_active { &self.original } else { &self.modified };
        (self.original_active, editor.read(cx).cursor_position().line as usize)
    }
    pub fn try_navigate(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (original,line)=self.source_position(cx);
        let index = if next { self.changes.iter().position(|&(a,b)| if original {a > line} else {b > line}) }
            else { self.changes.iter().rposition(|&(a,b)| if original {a < line} else {b < line}) };
        let Some(index)=index else {return false;};
        self.navigate_index(index, window, cx);
        true
    }
    pub fn navigate_edge(&mut self, first: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.changes.is_empty() { return false; }
        self.navigate_index(if first {0} else {self.changes.len()-1}, window, cx);
        true
    }
    fn navigate_index(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_reveal=Some(index);
        cx.notify();
        let original=self.original_active;
        let (old,new)=self.changes[index];
        if self.unified_mode {if let Some(v)=&self.unified {if v.navigate_to(new,window,cx) {self.pending_reveal=None;}return;}}
        let working=self.modified.clone();
        // Move both carets but restore the user's active side last.
        let mut positions=[(self.original.clone(),old),(working,new)];
        if original {positions.swap(0,1);}
        for (editor,line) in positions {editor.update(cx,|e,cx|e.set_cursor_position(gpui_kit::component::input::Position::new(line as u32,0),window,cx));}
    }
    fn reveal_pending(&mut self, window:&mut Window, cx: &mut Context<Self>) {
        let Some(index)=self.pending_reveal else {return;};
        let Some(&(_,line))=self.changes.get(index) else {self.pending_reveal=None;return;};
        if self.unified_mode {
            let focused=window.focused(cx);
            if let Some(v)=&self.unified {
                if v.navigate_to(line,window,cx) {self.pending_reveal=None;}
            }
            if let Some(focused)=focused {focused.focus(window,cx);}
            return;
        }
        let Some((a,b))=self.line_ranges.get(index) else {self.pending_reveal=None;return;};
        let Some(offsets)=crate::diff_scroll::navigation_offsets(
            &[self.original.read(cx),self.modified.read(cx)], &[a.clone(),b.clone()]) else {return;};
        self.pending_reveal=None;
        for (editor,y) in [self.original.clone(),self.modified.clone()].into_iter().zip(offsets) {
            editor.update(cx,|e,cx| {
                let mut offset=e.scroll_offset();offset.y=y;
                // Both destinations were computed together; don't remap either
                // one through the ordinary wheel-scroll anchor afterwards.
                e.set_peer_scroll_offset(offset,cx);
            });
        }
    }
    fn realign(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.unified_mode {return;}
        // Ordinary two-sided Diff keeps each source's real layout. Padding
        // would force equal offsets and hide the changing connector geometry.
        let working = self.modified.clone();
        let (master,peer)=if self.original_active {(self.original.clone(),working)}else{(working,self.original.clone())};
        let offset=master.read(cx).scroll_offset().y;
        if let Some(y)=crate::diff_scroll::vertical_offset(master.read(cx),peer.read(cx),&self.line_ranges,self.original_active,offset) {
            peer.update(cx,|e,cx| {let mut offset=e.scroll_offset();offset.y=y;e.set_peer_scroll_offset(offset,cx);});
        }
    }

    fn revert_change(&mut self, index: usize, generation: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.working.is_none() || !self.active || self.documents.locked() || generation != self.generation { return; }
        let Some(file) = &self.last_file else { return; };
        let editor = self.modified.clone();
        let current = editor.read(cx).text().to_string();
        if current != file.source(DiffSide::Modified) { return; }
        let Some((a,b)) = self.line_ranges.get(index) else { return; };
        let Some((range,text)) = crate::diff_revert::replacement(&self.baseline, &current, a.clone(), b.clone()) else { return; };
        editor.update(cx, |e,cx| {
            e.set_selected_range(range,cx);
            e.replace(text,window,cx);
            e.focus_handle(cx).focus(window,cx);
        });
        self.original_active = false;
    }

    fn original_gutter(&self, cx: &Context<Self>) -> impl IntoElement {
        let editor = self.original.clone();
        let changes = self.line_ranges.clone();
        let font_size = crate::ui_font::editor_size(cx);
        let font = gpui_kit::font(crate::ui_font::editor_family(cx));
        let digits = self.baseline.lines().count().max(1).to_string().len().max(3);
        let width = font_size * (digits as f32 * 0.65 + 1.);
        canvas(move |bounds, _, cx| {
            let marks = changes.iter().filter_map(|(a,b)| {
                if a.is_empty() {return None;}
                editor.read(cx).source_line_extent(a.clone()).map(|(top,bottom)|(top,bottom,if b.is_empty() {0x484a4a} else {0x385570}))
            }).collect::<Vec<_>>();
            (bounds, editor.read(cx).visible_source_line_extents(), editor.read(cx).line_height().unwrap_or(font_size * 1.5), marks)
        }, move |_, (bounds, lines, height, marks), window, cx| {
            window.with_content_mask(Some(ContentMask {bounds}), |window| {
                for (top,bottom,color) in marks {
                    window.paint_quad(fill(Bounds::new(point(bounds.left(),top),size(bounds.size.width,bottom-top)),rgb(color)));
                }
                for (line, top, _) in lines {
                    let text: SharedString = (line + 1).to_string().into();
                    let shaped = window.text_system().shape_line(text.clone(), font_size,
                        &[TextRun { len: text.len(), font: font.clone(), color: rgb(0x6f737a).into(), background_color: None, underline: None, strikethrough: None }], None);
                    let _ = shaped.paint(point(bounds.right()-shaped.width-px(6.),top), height, TextAlign::Left, None, window, cx);
                }
            });
        }).w(width).h_full().flex_shrink_0()
    }

    fn divider(&self, cx: &Context<Self>) -> impl IntoElement {
        let original = self.original.clone();
        let working = self.modified.clone();
        let ranges = self.line_ranges.clone();
        let ratio = self.split_ratio;
        let generation = self.generation;
        let partially_split = self.partial_selection(cx).map(|s|s.blocks.into_iter().zip(s.included).filter_map(|(b,i)|matches!(i,plico::partial_selection::Included::Lines{..}).then_some(b)).collect::<Vec<_>>()).unwrap_or_default();
        let editable = self.working.is_some() && !self.documents.locked();
        let owner = cx.entity().downgrade();
        canvas(move |bounds, window, cx| {
            let x = bounds.left() + bounds.size.width * ratio;
            let mut blocks = Vec::new();
            for (i,(a,b)) in ranges.iter().enumerate() {
                let Some((top_a,bottom_a)) = original.read(cx).source_line_extent(a.clone()) else {continue;};
                let Some((top_b,bottom_b)) = working.read(cx).source_line_extent(b.clone()) else {continue;};
                if bottom_a.max(bottom_b) < bounds.top() || top_a.min(top_b) > bounds.bottom() {continue;}
                let color = if a.is_empty() {0x294436} else if b.is_empty() {0x484a4a} else {0x385570};
                let arrow = Bounds::new(point(x-px(21.),top_a.max(bounds.top())),size(px(16.),px(18.)));
                let hitbox = (editable && !partially_split.iter().any(|p|p.original==*a && p.current==*b)).then(||window.insert_hitbox(arrow, HitboxBehavior::Normal));
                blocks.push((i,top_a,bottom_a,top_b,bottom_b,color,arrow,hitbox));
            }
            (bounds,x,blocks)
        }, move |_,(bounds,x,blocks),window,cx| {
            window.with_content_mask(Some(ContentMask {bounds}), |window| {
                window.paint_quad(fill(Bounds::new(point(x-px(24.),bounds.top()),size(px(48.),bounds.size.height)),rgb(0x181a1b)));
                for (_,a,z,b,y,color,arrow,hitbox) in &blocks {
                    let mut path = PathBuilder::fill();
                    // Rebased DiffDrawUtil: horizontal tangents, control points
                    // at 30% / 70%; empty intervals terminate in a thin marker.
                    let left = x-px(24.);
                    let right = x+px(24.);
                    let top_a = if a == z { *a-px(1.) } else { *a };
                    let top_b = if b == y { *b-px(1.) } else { *b };
                    path.move_to(point(left,top_a));
                    path.cubic_bezier_to(point(right,top_b),point(left+px(14.4),top_a),point(right-px(14.4),top_b));
                    path.line_to(point(right,*y));
                    path.cubic_bezier_to(point(left,*z),point(right-px(14.4),*y),point(left+px(14.4),*z));
                    path.close();
                    if a == z && *a >= bounds.top() {
                        window.paint_quad(fill(Bounds::new(point(bounds.left(),top_a.max(bounds.top())),size(left-bounds.left(),px(1.))),rgb(*color)));
                    }
                    if b == y && *b >= bounds.top() {
                        window.paint_quad(fill(Bounds::new(point(right,top_b.max(bounds.top())),size(bounds.right()-right,px(1.))),rgb(*color)));
                    }
                    if let Ok(path)=path.build() { window.paint_path(path,rgb(*color)); }
                    if let Some(hitbox) = hitbox {
                        window.set_cursor_style(CursorStyle::PointingHand,hitbox);
                        let icon = Bounds::new(arrow.origin + point(px(1.), px(2.)), size(px(14.), px(14.)));
                        let _ = window.paint_svg(icon, "jb/diff-apply-right_dark.svg".into(), None,
                            TransformationMatrix::unit(), rgb(0xced0d6).into(), cx);
                    }
                }
            });
            let owner=owner.clone();
            window.on_mouse_event(move |event:&MouseDownEvent,phase,window,cx| {
                if !phase.bubble() || event.button != MouseButton::Left {return;}
                for (index,_,_,_,_,_,_,hitbox) in &blocks {
                    if hitbox.as_ref().is_some_and(|h|h.is_hovered(window)) {
                        let _=owner.update(cx,|s,cx|s.revert_change(*index,generation,window,cx));
                        cx.stop_propagation(); break;
                    }
                }
            });
        }).absolute().size_full()
    }

    fn recompare(&mut self,cx:&mut Context<Self>) {
        if let Some((scope,id))=&self.commit_scope {let current=self.modified.read(cx).text().to_string();scope.update(cx,|s,cx|s.invalidate(&id.path,&self.baseline,&current,cx));}

        if !self.active || self.working.is_none() {return;}
        if let Some(view)=&self.unified {view.invalidate_if_changed(cx);}
        self.partial_template=None;
        self.generation+=1; let generation=self.generation;
        let original=self.comparison_baseline.clone(); let path=self.path.clone();
        let modified=self.working.as_ref().unwrap().read(cx).comparison_text(cx);
        let unit=crate::diff_settings::inline_unit(self.appearance.unwrap_or_default().emphasis);
        let partial=self.commit_scope.is_some();
        self.task=cx.spawn(async move|this,cx| {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let result=cx.background_spawn(async move {
                let file=DiffFile::from_editor_sources(Some((&path,&original)),Some((&path,&modified)))?;
                let eol_only=original.as_ref()!=modified && file.source(DiffSide::Original)==file.source(DiffSide::Modified);
                let marks=[file.editor_change_highlights_with_unit(DiffSide::Original,unit),file.editor_change_highlights_with_unit(DiffSide::Modified,unit)];
                let selection=partial.then(||plico::partial_selection::Selection::new(file.source(DiffSide::Original).into(),file.source(DiffSide::Modified).into(),false));
                Ok::<_, &'static str>((file,eol_only,marks,selection))
            }).await;
            let _=this.update(cx,|s,cx| {
                if s.generation!=generation {return;}
                if let Ok((file,eol_only,marks,selection))=result {
                    if s.modified.read(cx).text().to_string()!=file.source(DiffSide::Modified) {return;}
                    s.partial_template=selection;
                    s.eol_only=eol_only;
                    s.changes=file.change_starts();
                    s.line_ranges=file.change_line_ranges();
                    s.realign(cx);
                    let [old,new]=marks;
                    s.original_marks.set(old.into_iter().map(|(r,s,whole)| { let d=TextDecoration::new(r,s); if whole {d.whole_line()}else{d} }).collect(),cx);
                    s.working_marks.set(new.into_iter().map(|(r,s,whole)| { let d=TextDecoration::new(r,s); if whole {d.whole_line()}else{d} }).collect(),cx);
                    cx.emit(Compared(file));
                }
                cx.notify();
            });
        });
    }
}
impl Render for EditableDiff {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        self.reveal_pending(window,cx);
        if self.unified_mode {
            return if let Some(view)=&self.unified {
                v_flex().size_full().min_h_0()
                    .when(self.eol_only,|v|v.child(div().px_3().py_1().bg(rgb(0x253447)).child(plico::i18n::t("Only line separators differ"))))
                    .children(self.working.as_ref().and_then(|d|Editing::error_banner(d, &self.documents, cx)))
                    .child(Editor::new(&view.editor).readonly(self.documents.locked() || !view.ready()).appearance(false).bordered(false).h_full().font_family(crate::ui_font::editor_family(cx)).text_size(crate::ui_font::editor_size(cx))).into_any_element()
            } else {div().p_4().child(plico::i18n::t("Reading Git data…")).into_any_element()};
        }
        let editor=self.modified.clone();
        v_flex().size_full().min_h_0()
                    .when(self.eol_only,|v|v.child(div().px_3().py_1().bg(rgb(0x253447)).child(plico::i18n::t("Only line separators differ"))))
            .children(self.working.as_ref().and_then(|d|Editing::error_banner(d, &self.documents, cx)))
            .child(h_flex().relative().flex_1().min_h_0().w_full().items_stretch().overflow_hidden()
                .child(h_flex().w(relative(self.split_ratio)).flex_shrink_0().min_w_0().h_full().pr(px(24.))
                    .child(div().flex_1().min_w_0().h_full().child(self.selection_menu(Editor::new(&self.original).readonly(true).appearance(false).bordered(false).h_full().font_family(crate::ui_font::editor_family(cx)).text_size(crate::ui_font::editor_size(cx)),true,cx)))
                    .when(self.appearance.is_none_or(|p|p.line_numbers),|v|v.child(self.original_gutter(cx))))
                .child(div().w(relative(1. - self.split_ratio)).flex_shrink_0().min_w_0().h_full().pl(px(24.))
                    .child(self.selection_menu(Editor::new(&editor).readonly(self.working.is_none() || self.documents.locked()).appearance(false).bordered(false).h_full().font_family(crate::ui_font::editor_family(cx)).text_size(crate::ui_font::editor_size(cx)),false,cx)))
                .child(self.divider(cx)).child(self.partial_gutter(cx))).into_any_element()
    }
}

#[cfg(all(test, feature="desktop-tests"))]
mod lifecycle_tests {
    extern crate gpui_kit as gpui;
    use super::{EditableDiff, Documents, Contents, Target};
    use gpui::{AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window, div, px, point, MouseButton, Modifiers};
    use gpui::component::{Root,input::{Editor,EditorState}};
    struct Source(Entity<EditorState>);
    impl Render for Source {
        fn render(&mut self,_:&mut Window,_:&mut Context<Self>)->impl IntoElement {
            div().size_full().child(Editor::new(&self.0).appearance(false).bordered(false).h_full().line_height(px(20.)))
        }
    }
    #[gpui::test]
    fn baseline_update_preserves_working_draft_and_cursor(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut diff=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let target=Target::new(plico::git::Repository::new("fixture"),None,plico::git::Change {
                area:plico::git::Area::Unstaged,path:"a.txt".into(),old_path:None,status:"M".into()
            }).unwrap();
            let contents=Contents { original:"old\n".into(),working:plico::file_document::Document::new(b"my draft\n".to_vec()).unwrap() };
            let view=cx.new(|cx|EditableDiff::new(&target,contents,Documents::default(),window,cx));
            diff=Some(view.clone());Root::new(view,window,cx)
        });
        let diff=diff.unwrap();
        cx.update(|w,cx|diff.update(cx,|v,cx| {
            let document=v.working.as_ref().unwrap().entity_id();
            v.modified.update(cx,|e,cx|e.set_cursor_position(gpui::component::input::Position::new(0,3),w,cx));
            v.update_baseline("new staged\n".into(),w,cx);
            assert_eq!(v.working.as_ref().unwrap().entity_id(),document);
            assert_eq!(v.modified.read(cx).text().to_string(),"my draft\n");
            assert_eq!(v.modified.read(cx).cursor(),3);
            assert_eq!(v.original.read(cx).text().to_string(),"new staged\n");
        }));
    }
    // A divider is prepainted after its editor, before any editor paint runs.
    // It must see this frame's geometry on the very first frame as well.
    struct GeometryProbe(Entity<EditorState>);
    impl Render for GeometryProbe {
        fn render(&mut self,_:&mut Window,_:&mut Context<Self>)->impl IntoElement {
            let editor=self.0.clone();
            div().size_full().child(Editor::new(&self.0).h_full())
                .child(gpui::canvas(move |_,_,cx| {
                    assert!(editor.read(cx).source_line_extent(0..1).is_some(),
                        "divider prepaint must see current editor geometry");
                },|_,_,_,_|{}).absolute().size_full())
        }
    }
    #[gpui::test]
    fn divider_geometry_is_available_before_paint(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let (_,cx)=cx.add_window_view(|window,cx| {
            let editor=cx.new(|cx|EditorState::new(window,cx).default_value("first\nsecond"));
            let view=cx.new(|_|GeometryProbe(editor));Root::new(view,window,cx)
        });
        cx.update(|window,cx|{let _=window.draw(cx);});
    }
    type Extents = ((gpui::Pixels, gpui::Pixels), (gpui::Pixels, gpui::Pixels));
    struct SyncProbe {
        diff: Entity<EditableDiff>,
        frames: std::rc::Rc<std::cell::RefCell<Vec<Extents>>>,
    }
    impl Render for SyncProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let diff=self.diff.clone(); let frames=self.frames.clone();
            div().size_full().child(self.diff.clone()).child(gpui::canvas(move |_,_,cx| {
                let view=diff.read(cx);
                if let (Some(a),Some(b))=(view.original.read(cx).source_line_extent(8..9), view.modified.read(cx).source_line_extent(8..9)) {
                    frames.borrow_mut().push((a,b));
                }
            },|_,_,_,_|{}).absolute().size_full())
        }
    }
    #[gpui::test]
    fn equal_line_changes_scroll_together_on_first_frame(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut diff = None;
        let frames=std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let text = (0..200).map(|i| format!("line {i}\n")).collect::<String>();
            let modified = text.replace("line 8\n", "changed 8\n");
            let file = gpui::component::diff::DiffFile::from_editor_sources(
                Some(("a.txt", &text)), Some(("a.txt", &modified))).unwrap();
            let view = cx.new(|cx| EditableDiff::snapshot(&file, std::path::Path::new("a.txt"), window, cx));
            diff = Some(view.clone()); let probe=cx.new(|_|SyncProbe {diff:view, frames:frames.clone()}); Root::new(probe, window, cx)
        });
        let diff = diff.unwrap();
        for _ in 0..3 { cx.update(|w,cx| { let _=w.draw(cx); }); }
        for original in [true, false] {
            for offset in [-20., -40., -60., -10.] {
                frames.borrow_mut().clear();
                cx.update(|_,cx| {
                    let view=diff.read(cx);
                    let editor=if original {view.original.clone()} else {view.modified.clone()};
                    editor.update(cx,|e,cx|e.set_scroll_offset(point(px(0.),px(offset)),cx));
                });
                cx.update(|w,cx| { let _=w.draw(cx); });
                assert!(!frames.borrow().is_empty());
                for (a,b) in frames.borrow().iter() {
                    assert_eq!(a,b,"equal-line change must stay aligned on every frame, source left={original}, offset={offset}");
                }
            }
        }
    }
    // Sample the same prepaint API used by SourceChanges, before editor paint.
    struct GutterProbe {
        editor: Entity<EditorState>,
        inset: f32,
        samples: std::rc::Rc<std::cell::RefCell<Vec<Option<gpui::Pixels>>>>,
    }
    impl Render for GutterProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let editor = self.editor.clone();
            let samples = self.samples.clone();
            div().size_full().pl(px(self.inset))
                .child(div().relative().size_full()
                    .child(Editor::new(&self.editor).h_full())
                    .child(gpui::canvas(move |_, _, cx| {
                        samples.borrow_mut().push(editor.read(cx).source_gutter_right());
                    }, |_, _, _, _| {}).absolute().size_full()))
        }
    }
    #[gpui::test]
    fn source_gutter_moves_with_sidebar_in_same_frame(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let samples = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut probe = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx)
                .default_value("first\nsecond").line_number(true));
            let view = cx.new(|_| GutterProbe { editor, inset: 300., samples: samples.clone() });
            probe = Some(view.clone());
            Root::new(view, window, cx)
        });
        for _ in 0..2 { cx.update(|w, cx| { let _ = w.draw(cx); }); }
        let before = samples.borrow().last().unwrap().unwrap();
        let probe = probe.unwrap();
        for inset in [0., 300., 120.] {
            cx.update(|w, cx| {
                probe.update(cx, |p, cx| { p.inset = inset; cx.notify(); });
                samples.borrow_mut().clear();
                let _ = w.draw(cx);
            });
            let actual = samples.borrow()[0].expect("gutter available during prepaint");
            assert!((actual - before - px(inset - 300.)).abs() < px(0.01),
                "marker must follow layout immediately, without another repaint: {actual:?}");
        }
    }
    #[gpui::test]
    fn navigation_waits_for_layout_and_reveals_changes_at_upper_third(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut diff=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let old=(0..400).map(|i|format!("line {i}\n")).collect::<String>();
            let new=old.replace("line 120\n","changed 120\n").replace("line 240\n","changed 240\n");
            let file=gpui::component::diff::DiffFile::from_editor_sources(Some(("a.txt",&old)),Some(("a.txt",&new))).unwrap();
            let view=cx.new(|cx|EditableDiff::snapshot(&file,std::path::Path::new("a.txt"),window,cx));
            view.update(cx,|v,cx|assert!(v.navigate_edge(true,window,cx)));
            diff=Some(view.clone());Root::new(view,window,cx)
        });
        let diff=diff.unwrap();
        for expected in [120,240,120] {
            if expected==240 {cx.update(|w,cx|diff.update(cx,|v,cx|assert!(v.try_navigate(true,w,cx))));}
            else if diff.read_with(cx,|v,cx|v.source_position(cx).1)==240 {
                cx.update(|w,cx|diff.update(cx,|v,cx|assert!(v.try_navigate(false,w,cx))));
            }
            for _ in 0..4 {cx.run_until_parked();cx.update(|w,cx|{let _=w.draw(cx);});}
            diff.read_with(cx,|v,cx| {
                assert_eq!(v.source_position(cx).1,expected);
                assert!(v.pending_reveal.is_none());
                for e in [&v.original,&v.modified] {
                    let e=e.read(cx);let viewport=e.input_bounds();
                    let top=e.source_line_extent(expected..expected+1).unwrap().0-viewport.top();
                    assert!((top-viewport.size.height/3.).abs()<px(2.),"change {expected} must be framed, got {top:?}, viewport {viewport:?}, height {:?}, offset {:?}",e.line_height(),e.scroll_offset());
                }
            });
        }
    }
    #[gpui::test]
    fn historical_snapshot_is_readonly_and_navigates_source_lines(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut diff=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let file=gpui::component::diff::DiffFile::from_editor_sources(Some(("a.txt","one\ntwo\nthree\n")),Some(("a.txt","one\ninserted\ntwo\nthree\n"))).unwrap();
            let view=cx.new(|cx|EditableDiff::snapshot(&file,std::path::Path::new("a.txt"),window,cx));
            diff=Some(view.clone());Root::new(view,window,cx)
        });
        let diff=diff.unwrap();
        cx.update(|window,cx|diff.update(cx,|s,cx| {
            assert!(s.working.is_none(),"snapshot must not own a save-capable document");
            assert!(s.navigate_edge(true,window,cx));
            assert_eq!(s.source_position(cx),(false,1));
            let before=s.modified.read(cx).text().to_string();
            s.revert_change(0,s.generation,window,cx);
            assert_eq!(s.modified.read(cx).text().to_string(),before);
            assert_eq!(s.original.read(cx).source_boundary_row(3),3);
            assert_eq!(s.modified.read(cx).source_boundary_row(4),4);
        }));
    }

    #[gpui::test]
    fn block_revert_is_undoable_and_rejects_stale_comparison(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut diff=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let target=Target::new(plico::git::Repository::new("fixture"),None,
                plico::git::Change {area:plico::git::Area::Unstaged,path:"a.txt".into(),old_path:None,status:"M".into()}).unwrap();
            let contents=Contents {original:"old\nkeep\nleft\n".into(),working:plico::file_document::Document::new(b"new\nkeep\nright\n".to_vec()).unwrap()};
            let view=cx.new(|cx|EditableDiff::new(&target,contents,Documents::default(),window,cx));
            diff=Some(view.clone()); Root::new(view,window,cx)
        });
        let diff=diff.unwrap();
        cx.update(|window,cx|diff.update(cx,|s,cx| {
            let file=gpui::component::diff::DiffFile::from_editor_sources(Some(("a.txt","old\nkeep\nleft\n")),Some(("a.txt","new\nkeep\nright\n"))).unwrap();
            s.line_ranges=file.change_line_ranges(); s.last_file=Some(file);
            s.revert_change(0,s.generation,window,cx);
            assert_eq!(s.modified.read(cx).text().to_string(),"old\nkeep\nright\n");
            // The cached comparison is stale until the next background result.
            s.revert_change(1,s.generation,window,cx);
            assert_eq!(s.modified.read(cx).text().to_string(),"old\nkeep\nright\n");
            s.modified.clone().update(cx,|e,cx|e.step_history(true,window,cx));
            assert_eq!(s.modified.read(cx).text().to_string(),"new\nkeep\nright\n");
            let source_layout=s.source_layout.unwrap();
            s.appearance(plico::layout_preferences::DiffAppearance {soft_wrap:false,line_numbers:false,..Default::default()},window,cx);
            assert_eq!(s.modified.read(cx).source_layout(),(false,false));
            s.deactivate(cx);
            assert_eq!(s.modified.read(cx).source_layout(),source_layout);
        }));
    }

    #[gpui::test]
    fn empty_side_marker_precedes_alignment_padding(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut editor=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let state=cx.new(|cx|EditorState::new(window,cx).default_value("one\ntwo\nthree"));
            editor=Some(state.clone());
            let view=cx.new(|_|Source(state));Root::new(view,window,cx)
        });
        let editor=editor.unwrap();
        cx.update(|_,cx|editor.update(cx,|e,cx|e.set_block_gaps(vec![(1,3)],cx).unwrap()));
        cx.run_until_parked();cx.update(|window,cx|{let _=window.draw(cx);});
        editor.read_with(cx,|e,_| {
            let first=e.source_line_extent(0..1).unwrap();
            let empty=e.source_line_extent(1..1).unwrap();
            let next=e.source_line_extent(1..2).unwrap();
            assert_eq!(empty,(first.1,first.1));
            assert!(next.0>empty.0,"padding must remain below the marker");
            assert_eq!(e.source_line_extent(0..0).unwrap().0,first.0);
            let last=e.source_line_extent(2..3).unwrap();
            assert_eq!(e.source_line_extent(3..3).unwrap(),(last.1,last.1));
        });
    }

    #[gpui::test]
    fn releasing_diff_clears_padding_from_retained_source(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut diff=None; let mut editor=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let target=Target::new(plico::git::Repository::new("fixture"),None,
                plico::git::Change {area:plico::git::Area::Unstaged,path:"a.txt".into(),old_path:None,status:"M".into()}).unwrap();
            let contents=Contents {original:"one\ntwo\nthree\nfour".into(),working:plico::file_document::Document::new(b"one\ntwo\nthree\nfour".to_vec()).unwrap()};
            let view=cx.new(|cx|EditableDiff::new(&target,contents,Documents::default(),window,cx));
            let source=view.read(cx).modified.clone();
            editor=Some(source.clone());diff=Some(view);
            let source=cx.new(|_|Source(source));Root::new(source,window,cx)
        });
        let editor=editor.unwrap();
        cx.run_until_parked();cx.update(|window,cx|{let _=window.draw(cx);});
        let mut third_y=None;
        for y in (35..100).step_by(5) {
            cx.simulate_mouse_down(point(px(100.),px(y as f32)),MouseButton::Left,Modifiers::default());
            cx.simulate_mouse_up(point(px(100.),px(y as f32)),MouseButton::Left,Modifiers::default());
            if editor.read_with(cx,|e,_|e.cursor_position().line)==2 {third_y=Some(y as f32);break;}
        }
        let third_y=third_y.unwrap();
        cx.update(|_,cx|editor.update(cx,|e,cx|e.set_block_gaps(vec![(1,5)],cx).unwrap()));
        drop(diff.take());
        cx.run_until_parked();cx.update(|window,cx|{let _=window.draw(cx);});
        cx.simulate_mouse_down(point(px(100.),px(third_y)),MouseButton::Left,Modifiers::default());
        cx.simulate_mouse_up(point(px(100.),px(third_y)),MouseButton::Left,Modifiers::default());
        assert_eq!(editor.read_with(cx,|e,_|e.cursor_position().line),2,"Diff release must remove its view-only padding");
        assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"one\ntwo\nthree\nfour");
    }
}
