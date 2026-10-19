//! Shared working-tree documents. Source tabs and local diffs use the same
//! editor entity, save queue and disk baseline for a repository-relative path.
use std::{cell::{Cell, RefCell}, collections::BTreeMap, path::PathBuf, rc::Rc, time::Duration};
use gpui_kit::*;
use gpui_kit::component::input::{EditorState, InputEvent, RopeExt as _};
use plico::i18n::t;
use plico::{file_document::Document, file_save, git::Cancel, project_files::Project};

#[derive(Clone, Default)]
pub struct Documents(Rc<RefCell<BTreeMap<PathBuf, WeakEntity<Editing>>>>, Rc<Cell<bool>>, Rc<RefCell<BTreeMap<PathBuf, Entity<Editing>>>>, Rc<RefCell<Option<Rc<dyn Fn(&mut App)>>>>);
impl Documents {
    pub fn on_saved(&self, listener: impl Fn(&mut App) + 'static) { *self.3.borrow_mut()=Some(Rc::new(listener)); }
    pub fn locked(&self) -> bool { self.1.get() }
    pub fn set_locked(&self, locked: bool, cx: &mut App) {
        if self.1.replace(locked) == locked { return; }
        for document in self.all() {
            document.read(cx).editor.clone().update(cx, |editor,cx|editor.set_readonly(locked,cx));
        }
    }
    pub fn get(&self, path: &PathBuf) -> Option<Entity<Editing>> {
        self.0.borrow().get(path).and_then(WeakEntity::upgrade)
    }
    /// Resolve on the UI thread after asynchronous IO, so a second surface
    /// cannot replace a live document with a stale disk snapshot.
    pub fn get_or_create(
        &self, root: PathBuf, path: PathBuf, document: Document,
        make_editor: impl FnOnce(&str, &mut App) -> Entity<EditorState>, cx: &mut App,
    ) -> Entity<Editing> {
        if let Some(existing) = self.get(&path) { return existing; }
        let editor = make_editor(document.text(), cx);
        editor.update(cx, |state, cx| state.set_readonly(self.locked(), cx));
        let editing = cx.new(|cx| Editing::new(root, path.clone(), document, editor, cx));
        self.insert(path.clone(), &editing);
        let listener = Rc::downgrade(&self.3);
        cx.subscribe(&editing, move |_, _: &Saved, cx| {
            let callback=listener.upgrade().and_then(|slot|slot.borrow().clone());
            if let Some(callback)=callback {callback(cx);}
        }).detach();
        let listener = Rc::downgrade(&self.3);
        cx.subscribe(&editing, move |_, _: &Reloaded, cx| {
            let callback=listener.upgrade().and_then(|slot|slot.borrow().clone());
            if let Some(callback)=callback {callback(cx);}
        }).detach();
        let pending = Rc::downgrade(&self.2);
        cx.observe(&editing, move |document, cx| {
            let Some(pending) = pending.upgrade() else { return; };
            if document.read(cx).pending(cx) { pending.borrow_mut().insert(path.clone(), document); }
            else { pending.borrow_mut().remove(&path); }
        }).detach();
        editing
    }
    pub fn insert(&self, path: PathBuf, document: &Entity<Editing>) {
        let mut entries = self.0.borrow_mut();
        entries.retain(|_, weak| weak.upgrade().is_some());
        entries.insert(path, document.downgrade());
    }
    pub fn retain_pending(&self, path: PathBuf, document: &Entity<Editing>, cx: &App) {
        if document.read(cx).pending(cx) { self.2.borrow_mut().insert(path, document.clone()); }
    }
    pub fn all(&self) -> Vec<Entity<Editing>> {
        self.0.borrow().values().filter_map(WeakEntity::upgrade).collect()
    }
    pub fn pending(&self, cx: &App) -> bool {
        self.all().iter().any(|d| d.read(cx).pending(cx))
    }
    pub fn save_all(&self, cx: &mut App) {
        for document in self.all() { document.update(cx, |d, cx| d.save(cx)); }
    }
}

pub struct Saved;
pub struct Reloaded;
pub struct Editing {
    pub editor: Entity<EditorState>,
    pub error: Option<String>,
    pub edited: bool,
    invalid_edit: bool,
    root: PathBuf,
    path: PathBuf,
    document: Document,
    idle: Task<()>,
    writing: Task<()>,
    reloading: bool,
    reading: Task<()>,
}
impl EventEmitter<Saved> for Editing {}
impl EventEmitter<Reloaded> for Editing {}
impl Editing {
    pub fn comparison_text(&self,cx:&App)->String {
        self.document.comparison_text(&self.editor.read(cx).text().to_string())
    }

    pub fn new(root: PathBuf, path: PathBuf, document: Document, editor: Entity<EditorState>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&editor, |s, editor, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) { return; }
            let text = editor.read(cx).text().to_string();
            match s.document.edit(text) {
                Ok(()) => { s.error = None; s.invalid_edit = false; s.edited = true; }
                Err(error) => {
                    s.invalid_edit = true;
                    s.error = Some(format!("{}: {error:?}", t("Could not save file")));
                    // The editor still owns the complete draft; never close it
                    // merely because the bounded document refused the new text.
                    s.edited = true;
                    cx.notify(); return;
                }
            }
            s.idle = cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let _ = this.update(cx, |s, cx| s.save(cx));
            });
            cx.notify();
        }).detach();
        Self { editor, error: None, edited: false, invalid_edit: false, root, path, document, idle: Task::ready(()), writing: Task::ready(()), reloading: false, reading: Task::ready(()) }
    }
    pub fn pending(&self, cx: &App) -> bool {
        self.reloading || self.document.dirty() || self.document.saving()
            || self.error.is_some() || self.invalid_edit
            // IME preedit changes the editor before InputEvent::Change. Close
            // and repository switching must not mistake that draft for clean.
            || !self.editor.read(cx).text().chars().eq(self.document.text().chars())
    }
    pub fn saving(&self) -> bool { self.reloading || self.document.saving() }
    /// Explicit, confirmed reload of the shared document, never a new registry entry.
    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving() { return; }
        self.idle=Task::ready(());
        self.reloading=true;
        let expected=self.editor.read(cx).text().to_string();
        let root=self.root.clone(); let path=self.path.clone();
        self.reading=cx.spawn_in(window,async move|this,wx| {
            let result=wx.background_spawn(async move {
                Project::new(root).and_then(|p|p.read_bytes(&path,&Cancel::default()))
                    .map_err(|e|format!("{e:?}"))
                    .and_then(|bytes|Document::new(bytes).map_err(|e|format!("{e:?}")))
            }).await;
            let _=this.update_in(wx,|s,window,cx|s.finish_reload(&expected,result,window,cx));
        });
        cx.notify();
    }
    fn finish_reload(&mut self, expected: &str, result: Result<Document,String>, window: &mut Window, cx: &mut Context<Self>) {
        self.reloading=false;
        if self.editor.read(cx).text().to_string()!=expected {
            self.error=Some(t("File changed while reloading. Your edits have been kept.").into());
            cx.notify(); return;
        }
        let document=match result {
            Ok(document)=>document,
            Err(error)=>{self.error=Some(format!("{}: {error}",t("Could not reload file")));cx.notify();return;}
        };
        let text=document.text().to_owned();
        self.editor.update(cx,|editor,cx| {
            let selected=editor.selected_range();
            let start=editor.text().offset_to_position(selected.start);
            let end=editor.text().offset_to_position(selected.end);
            let reversed=editor.cursor()==selected.start&&!selected.is_empty();
            let scroll=editor.scroll_offset();
            editor.set_value(text,window,cx);
            let start=editor.text().position_to_offset(&start);
            let end=editor.text().position_to_offset(&end);
            editor.set_selected_range_with_direction(start..end,reversed,cx);
            editor.set_scroll_offset(scroll,cx);
        });
        self.document=document;
        self.invalid_edit=false;self.error=None;
        cx.emit(Reloaded);cx.notify();
    }
    pub fn save(&mut self, cx: &mut Context<Self>) {
        self.idle = Task::ready(());
        if self.saving() { return; }
        // Re-read the editor, including a previously rejected oversized draft.
        if let Err(error) = self.document.edit(self.editor.read(cx).text().to_string()) {
            self.invalid_edit = true;
            self.error = Some(format!("{}: {error:?}", t("Could not save file"))); cx.notify(); return;
        }
        self.invalid_edit = false;
        self.error = None;
        let Ok(Some(request)) = self.document.begin_save() else { cx.notify(); return; };
        let root = self.root.clone(); let path = self.path.clone();
        self.writing = cx.spawn(async move |this, cx| {
            let (request, result) = cx.background_spawn(async move {
                let result = Project::new(root).map_err(file_save::Error::from)
                    .and_then(|project| file_save::save(&project, &path, &request, &Cancel::default()));
                (request, result)
            }).await;
            let _ = this.update(cx, |s, cx| {
                let succeeded = result.is_ok();
                s.document.finish_save(request, succeeded);
                let save_error = result.err().map(|error| match error {
                    file_save::Error::Conflict => t("File changed on disk. Your edits have been kept; reload or compare before saving.").into(),
                    file_save::Error::ReadOnly => t("File is read-only. Your edits have been kept.").into(),
                    error => format!("{}: {error:?}", t("Could not save file")),
                });
                if !s.invalid_edit { s.error = save_error; }
                if succeeded { cx.emit(Saved); }
                cx.notify();
                if succeeded && !s.invalid_edit && s.document.dirty() { s.save(cx); }
            });
        });
        cx.notify();
    }
}

#[cfg(all(test,feature="desktop-tests"))]
mod reload_tests {
    extern crate gpui_kit as gpui;
    use super::{Documents,Document};
    use gpui::{AppContext as _, Context, Render, Window, IntoElement};
    use gpui::component::{Root,input::EditorState};
    struct Empty;
    impl Render for Empty {
        fn render(&mut self,_:&mut Window,_:&mut Context<Self>)->impl IntoElement {gpui::div()}
    }
    #[gpui::test]
    fn preedit_is_pending_and_retained_before_change_event(cx:&mut gpui::TestAppContext) {
        use gpui::EntityInputHandler as _;
        cx.update(gpui::component::init);
        let documents=Documents::default(); let mut editing=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            editing=Some(documents.get_or_create("fixture".into(),"a.txt".into(),Document::new(b"old".to_vec()).unwrap(),
                |text,cx|cx.new(|cx|EditorState::new(window,cx).default_value(text.to_owned())),cx));
            let empty=cx.new(|_|Empty);Root::new(empty,window,cx)
        });
        let editing=editing.unwrap();
        let editor=editing.read_with(cx,|s,_|s.editor.clone());
        cx.update(|_,cx|assert!(!documents.pending(cx)));
        cx.update(|window,cx|editor.update(cx,|e,cx|e.replace_and_mark_text_in_range(Some(0..0),"中",Some(1..1),window,cx)));
        cx.update(|_,cx| {
            // The autosave model has not received a committed Change event.
            assert!(!editing.read(cx).document.dirty());
            assert!(documents.pending(cx));
            documents.retain_pending("a.txt".into(),&editing,cx);
        });
        drop(editing);
        cx.update(|_,cx|assert!(documents.get(&"a.txt".into()).is_some_and(|d|d.read(cx).pending(cx))));
    }
    #[gpui::test]
    fn reload_keeps_shared_identity_and_rejects_late_results(cx:&mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let documents=Documents::default(); let mut editing=None;
        let (_,cx)=cx.add_window_view(|window,cx| {
            let document=documents.get_or_create("fixture".into(),"a.txt".into(),Document::new(b"old\n".to_vec()).unwrap(),
                |text,cx|cx.new(|cx|EditorState::new(window,cx).default_value(text.to_owned())),cx);
            editing=Some(document);
            let empty=cx.new(|_|Empty);Root::new(empty,window,cx)
        });
        let editing=editing.unwrap();
        let shared=documents.get(&"a.txt".into()).unwrap();
        let editor=editing.read_with(cx,|s,_|s.editor.clone());
        cx.update(|window,cx|editing.update(cx,|s,cx| {
            s.reloading=true;
            s.finish_reload("old\n",Ok(Document::new(b"\xef\xbb\xbfnew\r\n".to_vec()).unwrap()),window,cx);
            assert!(!s.pending(cx));
            assert_eq!(s.editor.entity_id(),editor.entity_id());
            s.document.edit("updated\n".into()).unwrap();
            let snapshot=s.document.begin_save().unwrap().unwrap();
            assert_eq!(&*snapshot.expected,b"\xef\xbb\xbfnew\r\n");
            assert_eq!(&*snapshot.bytes,b"\xef\xbb\xbfupdated\r\n");
            s.document.finish_save(snapshot,false);
        }));
        assert_eq!(editing.entity_id(),shared.entity_id());
        shared.read_with(cx,|s,cx|assert_eq!(s.editor.read(cx).text().to_string(),"new\n"));
        cx.update(|window,cx|editing.update(cx,|s,cx| {
            s.editor.update(cx,|e,cx|e.set_value("typed while reading",window,cx));
            s.reloading=true;
            s.finish_reload("new\n",Ok(Document::new(b"late disk snapshot".to_vec()).unwrap()),window,cx);
            assert!(s.error.is_some());assert!(s.pending(cx));assert!(!s.saving());
            assert_eq!(s.editor.read(cx).text().to_string(),"typed while reading");
            // Failed IO also keeps the draft and the existing shared editor.
            s.finish_reload("typed while reading",Err("read failed".into()),window,cx);
            assert_eq!(s.editor.read(cx).text().to_string(),"typed while reading");
            assert_eq!(s.editor.entity_id(),editor.entity_id());
        }));
    }
}
