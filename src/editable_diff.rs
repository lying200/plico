//! Editable working-tree comparison. Both surfaces share the working document;
//! the original revision has a separate, immutable editor.
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::component::{h_flex, v_flex, input::{Editor, EditorState, InputEvent, TextDecoration, TextDecorationCollection}, diff::{DiffFile, DiffSide}};
use crate::editing::{Documents, Editing};
use plico::{working_copy::Contents, diff_target::Target};
use std::{sync::Arc, time::Duration};

pub struct Compared(pub DiffFile);
impl EventEmitter<Compared> for EditableDiff {}
pub struct EditableDiff {
    pub working: Entity<Editing>,
    original: Entity<EditorState>,
    baseline: Arc<str>,
    path: String,
    original_marks: TextDecorationCollection,
    working_marks: TextDecorationCollection,
    documents: Documents,
    generation: u64,
    original_active: bool,
    active: bool,
    alignment_initialized: bool,
    unified_mode: bool,
    unified: Option<crate::unified_editing::UnifiedEditing>,
    last_file: Option<DiffFile>,
    changes: Vec<(usize, usize)>,
    line_ranges: Vec<(std::ops::Range<usize>,std::ops::Range<usize>)>,
    task: Task<()>,
}
impl EditableDiff {
    pub fn new(target: &Target, contents: Contents, documents: Documents, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let id = target.identity();
        let language = crate::project_view::file_language(&id.path);
        let baseline: Arc<str> = contents.original.into();
        let original = cx.new(|cx| {
            let mut editor = EditorState::new(window,cx).tab_size(gpui_kit::component::input::TabSize { tab_size: 4, hard_tabs: false }).default_value(baseline.to_string()).line_number(true).language(language);
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
        for (is_original, editor) in [(false,&editor),(true,&original)] {
            cx.subscribe(editor, move |s,_,event:&InputEvent,cx| {
                if !s.active || s.unified_mode { return; }
                match event {
                    InputEvent::LayoutChanged => s.realign(cx),
                    InputEvent::ScrollChanged { previous, offset } => {
                        let peer = if is_original {s.working.read(cx).editor.clone()} else {s.original.clone()};
                        peer.update(cx,|e,cx| {
                            let mut target=e.scroll_offset();
                            if previous.x != offset.x { target.x=offset.x; }
                            if previous.y != offset.y { target.y=offset.y; }
                            e.set_peer_scroll_offset(target,cx);
                        });
                    }
                    _ => {}
                }
            }).detach();
        }
        cx.subscribe_in(&cx.entity(),window,|s,_,event:&Compared,window,cx| {
            if !s.active {return;}
            s.last_file=Some(event.0.clone());
            s.update_unified(window,cx);
            cx.notify();
        }).detach();
        cx.on_release(|s,cx| s.clear_view(cx)).detach();
        cx.subscribe(&working,|s,_,_:&crate::editing::Reloaded,cx|s.recompare(cx)).detach();
        cx.observe(&working,|_,_,cx|cx.notify()).detach();
        let mut view=Self { working,original,baseline,path:id.path.to_string_lossy().into_owned(),original_marks,working_marks,documents,generation:0,original_active:false,active:true,alignment_initialized:false,unified_mode:false,unified:None,last_file:None,changes:Vec::new(),line_ranges:Vec::new(),task:Task::ready(()) };
        view.recompare(cx); view
    }
    fn update_unified(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        let Some(file)=&self.last_file else {return;};
        let source=self.working.read(cx).editor.clone();
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
            self.working.read(cx).editor.clone().update(cx,|e,cx|{let _=e.set_block_gaps_preserving_position(Vec::new(),cx);});
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
        let editor = self.working.read(cx).editor.clone();
        editor.update(cx,|e,cx| {let _=e.set_block_gaps_preserving_position(Vec::new(),cx);});
    }
    pub fn focus(&self,window:&mut Window,cx:&mut App) { if self.unified_mode {if let Some(v)=&self.unified {v.editor.read(cx).focus_handle(cx).focus(window,cx);return;}} self.working.read(cx).editor.read(cx).focus_handle(cx).focus(window,cx); }
    pub fn source_position(&self, cx: &App) -> (bool, usize) {
        if self.unified_mode {if let Some(v)=&self.unified {return v.source_position(&self.baseline,cx);}}
        let editor = if self.original_active { &self.original } else { &self.working.read(cx).editor };
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
        let original=self.original_active;
        let (old,new)=self.changes[index];
        if self.unified_mode {if let Some(v)=&self.unified {v.navigate_to(new,window,cx);return;}}
        let working=self.working.read(cx).editor.clone();
        // Move both carets but restore the user's active side last.
        let mut positions=[(self.original.clone(),old),(working,new)];
        if original {positions.swap(0,1);}
        for (editor,line) in positions {editor.update(cx,|e,cx|e.set_cursor_position(gpui_kit::component::input::Position::new(line as u32,0),window,cx));}
    }
    fn realign(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.unified_mode {return;}
        let working = self.working.read(cx).editor.clone();
        let left = self.original.read(cx);
        let right = working.read(cx);
        let mut gaps = [Vec::new(),Vec::new()];
        let (mut old_end,mut new_end)=(0,0);
        for (a,b) in &self.line_ranges {
            // Match both the unchanged prefix and the body using actual wrapped
            // row heights, just as Rebased's top/bottom alignment inlays do.
            for (old,new) in [(a.start,b.start),(a.end,b.end)] {
                let old_height=left.unpadded_line_boundary(old).saturating_sub(left.unpadded_line_boundary(old_end));
                let new_height=right.unpadded_line_boundary(new).saturating_sub(right.unpadded_line_boundary(new_end));
                if old_height < new_height {gaps[0].push((old,new_height-old_height));}
                if new_height < old_height {gaps[1].push((new,old_height-new_height));}
                old_end=old; new_end=new;
            }
        }
        let [a,b]=gaps;
        // The initial comparison must show leading deletions rather than scroll
        // them out of view just to keep the initial caret at viewport zero.
        if !self.alignment_initialized {
            self.original.update(cx,|e,cx| {let _=e.set_block_gaps(a,cx);});
            working.update(cx,|e,cx| {let _=e.set_block_gaps(b,cx);});
            return;
        }
        let (active, active_gaps, peer, peer_gaps) = if self.original_active {
            (self.original.clone(),a,working,b)
        } else { (working,b,self.original.clone(),a) };
        // One keeper for the active side; two independent restores would fight
        // through synchronization when one side gains more padding.
        let restored=active.update(cx,|e,cx|e.set_block_gaps_preserving_position(active_gaps,cx));
        peer.update(cx,|e,cx| {
            if e.set_block_gaps(peer_gaps,cx).is_ok() {
                if let Ok(restored)=restored {
                    let mut offset=e.scroll_offset();offset.y=restored.y;
                    e.set_peer_scroll_offset(offset,cx);
                }
            }
        });
    }
    fn recompare(&mut self,cx:&mut Context<Self>) {
        if !self.active {return;}
        if let Some(view)=&self.unified {view.invalidate_if_changed(cx);}
        self.generation+=1; let generation=self.generation;
        let original=self.baseline.clone(); let path=self.path.clone();
        let modified=self.working.read(cx).editor.read(cx).text().to_string();
        self.task=cx.spawn(async move|this,cx| {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let result=cx.background_spawn(async move { DiffFile::from_sources(Some((&path,&original)),Some((&path,&modified))) }).await;
            let _=this.update(cx,|s,cx| {
                if s.generation!=generation {return;}
                if let Ok(file)=result {
                    if s.working.read(cx).editor.read(cx).text().to_string()!=file.source(DiffSide::Modified) {return;}
                    s.changes=file.change_starts();
                    s.line_ranges=file.change_line_ranges();
                    s.realign(cx);
                    s.alignment_initialized=true;
                    let marks=|side,color|file.changed_ranges(side).into_iter().map(|range|TextDecoration::new(range,HighlightStyle {background_color:Some(rgb(color).into()),..Default::default()})).collect();
                    s.original_marks.set(marks(DiffSide::Original,0x472b2d),cx);
                    s.working_marks.set(marks(DiffSide::Modified,0x294436),cx);
                    cx.emit(Compared(file));
                }
                cx.notify();
            });
        });
    }
}
impl Render for EditableDiff {
    fn render(&mut self,_:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        if self.unified_mode {
            return if let Some(view)=&self.unified {
                v_flex().size_full().min_h_0()
                    .when_some(self.working.read(cx).error.clone(),|v,error|v.child(div().px_3().py_2().text_color(rgb(0xf2a6a6)).child(error)))
                    .child(Editor::new(&view.editor).readonly(self.documents.locked() || !view.ready()).appearance(false).bordered(false).h_full().font_family("JetBrains Mono").text_size(px(13.))).into_any_element()
            } else {div().p_4().child(plico::i18n::t("Reading Git data…")).into_any_element()};
        }
        let editor=self.working.read(cx).editor.clone();
        v_flex().size_full().min_h_0()
            .when_some(self.working.read(cx).error.clone(),|v,error|v.child(div().px_3().py_2().text_color(rgb(0xf2a6a6)).child(error)))
            .child(h_flex().flex_1().min_h_0().w_full().items_stretch()
                .child(div().flex_1().min_w_0().h_full().border_r_1().border_color(rgb(0x26282c))
                    .child(Editor::new(&self.original).readonly(true).appearance(false).bordered(false).h_full().font_family("JetBrains Mono").text_size(px(13.))))
                .child(div().flex_1().min_w_0().h_full()
                    .child(Editor::new(&editor).readonly(self.documents.locked()).appearance(false).bordered(false).h_full().font_family("JetBrains Mono").text_size(px(13.))))).into_any_element()
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
    fn releasing_diff_clears_padding_from_retained_source(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut diff=None; let mut editor=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let target=Target::new(plico::git::Repository::new("fixture"),None,
                plico::git::Change {area:plico::git::Area::Unstaged,path:"a.txt".into(),old_path:None,status:"M".into()}).unwrap();
            let contents=Contents {original:"one\ntwo\nthree\nfour".into(),working:plico::file_document::Document::new(b"one\ntwo\nthree\nfour".to_vec()).unwrap()};
            let view=cx.new(|cx|EditableDiff::new(&target,contents,Documents::default(),window,cx));
            let source=view.read(cx).working.read(cx).editor.clone();
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
