//! Disk observation for the shared document. Never replaces a dirty draft.
use super::*;
use plico::worktree_events::{Batch, Stamp};
use std::sync::Arc;

pub(super) struct Observation {
    pub stamp: Option<Stamp>,
    pub conflict: bool,
    checking: bool,
    token: Cancel,
    task: Task<()>,
}

impl Default for Observation {
    fn default() -> Self { Self { stamp: None, conflict: false, checking: false, token: Cancel::default(), task: Task::ready(()) } }
}
impl Drop for Observation { fn drop(&mut self) { self.token.cancel(); } }
impl Documents {
    pub fn observe_disk(&self, changes: &Batch, window: &mut Window, cx: &mut App) {
        if self.locked() { return; }
        for document in self.all() {
            document.update(cx, |d, cx| {
                d.observe_disk(changes.affects(&d.path), self.clone(), window, cx);
            });
        }
    }
}
impl Editing {
    pub fn error_banner(document: &Entity<Self>, documents: &Documents, cx: &App) -> Option<AnyElement> {
        use gpui_kit::component::{button::{Button, ButtonVariants as _}, h_flex, v_flex, Disableable as _, Sizable as _, WindowExt as _};
        let error = document.read(cx).error.clone()?;
        let copy = document.downgrade(); let reload = document.downgrade();
        let disabled = documents.locked() || document.read(cx).saving();
        let documents = documents.clone();
        Some(v_flex().flex_shrink_0().px_3().py_2().gap_1()
            .child(div().text_color(rgb(0xf2a6a6)).child(error))
            .child(h_flex().gap_3()
                .child(Button::new("copy-draft").link().small().label(t("Copy editor content"))
                    .on_click(move |_, _, cx| { if let Some(d) = copy.upgrade() {
                        cx.write_to_clipboard(ClipboardItem::new_string(d.read(cx).editor.read(cx).text().to_string()));
                    }}))
                .child(Button::new("reload-draft").link().small().disabled(disabled).label(t("Reload file"))
                    .on_click(move |_, window, cx| {
                        let owner = reload.clone(); let documents = documents.clone();
                        window.open_dialog(cx, move |dialog, _, _| {
                            let owner = owner.clone(); let documents = documents.clone();
                            dialog.title(t("Reload from Disk"))
                                .child(t("Discard your unsaved changes and reload this file from disk?"))
                                .on_ok(move |_, window, cx| {
                                    if documents.locked() { return false; }
                                    let _ = owner.update(cx, |d, cx| d.reload(window, cx)); true
                                })
                                .footer(h_flex().justify_end().gap_2()
                                    .child(gpui_kit::component::dialog::DialogAction::new().child(Button::new("reload-confirm").primary().label(t("Reload file"))))
                                    .child(gpui_kit::component::dialog::DialogClose::new().trigger(|b| b.label(t("Cancel")))))
                        });
                    })))
            .into_any_element())
    }
    fn observe_disk(&mut self, force: bool, documents: Documents, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving() || self.external.checking { return; }
        self.external.checking = true;
        let root = self.root.clone(); let path = self.path.clone();
        let previous = self.external.stamp.clone();
        let expected = self.document.disk();
        let token = self.external.token.clone();
        self.external.task = cx.spawn_in(window, async move |this, wx| {
            let result = wx.background_spawn(async move {
                let stamp = Stamp::read(&root.join(&path)).map_err(|e| e.to_string())?;
                if !force && previous.as_ref() == Some(&stamp) { return Ok(None); }
                let bytes = Project::new(root.clone()).and_then(|p| p.read_bytes(&path, &token))
                    .map_err(|e| format!("{e:?}"))?;
                // Do not associate an old byte snapshot with a newer stamp.
                if Stamp::read(&root.join(&path)).ok().as_ref() != Some(&stamp) {
                    return Ok(None);
                }
                Ok::<_, String>(Some((stamp, bytes)))
            }).await;
            let _ = this.update_in(wx, |s, window, cx| {
                s.finish_observation(&expected, documents.locked(), result, window, cx);
            });
        });
    }
    fn finish_observation(&mut self, expected: &Arc<[u8]>, locked: bool, result: Result<Option<(Stamp, Vec<u8>)>, String>, window: &mut Window, cx: &mut Context<Self>) {
        self.external.checking = false;
        if locked || self.saving() || !Arc::ptr_eq(expected, &self.document.disk()) { return; }
        self.accept_external(result, window, cx);
    }
    fn accept_external(&mut self, result: Result<Option<(Stamp, Vec<u8>)>, String>, window: &mut Window, cx: &mut Context<Self>) {
        let (stamp, bytes) = match result {
            Ok(Some(value)) => value,
            Ok(None) => return,
            Err(error) => {
                self.external.stamp = None;
                self.external.conflict = true;
                self.idle = Task::ready(());
                self.error = Some(format!("{}: {error}", t("File is unavailable on disk. Your content has been kept.")));
                cx.notify(); return;
            }
        };
        if bytes.as_slice() == self.document.disk().as_ref() {
            self.external.stamp = Some(stamp);
            if self.external.conflict {
                self.external.conflict = false; self.error = None; cx.notify();
                self.save(cx);
            }
            return;
        }
        if self.document.dirty() || self.invalid_edit
            || self.editor.read(cx).text().to_string() != self.document.text() {
            self.external.conflict = true;
            // Keep checking: if the writer restores our baseline, saving may resume.
            self.external.stamp = None;
            self.idle = Task::ready(());
            self.error = Some(t("File changed on disk. Your edits have been kept; reload or compare before saving.").into());
            cx.notify(); return;
        }
        let document = match Document::new(bytes) {
            Ok(document) => document,
            Err(error) => {
                self.external.conflict = true; self.external.stamp = None;
                self.error = Some(format!("{}: {error:?}", t("Could not reload file")));
                cx.notify(); return;
            }
        };
        let expected = self.editor.read(cx).text().to_string();
        self.idle = Task::ready(());
        self.finish_reload(&expected, Ok(document), window, cx);
        self.external.stamp = Some(stamp);
    }
}

#[cfg(all(test, feature="desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::{Documents, Document, Stamp};
    use gpui::{AppContext as _, Context, Render, Window, IntoElement};
    use gpui::component::{Root, input::{EditorState, Position}};
    struct Empty;
    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { gpui::div() }
    }
    #[gpui::test]
    fn external_reload_keeps_identity_position_and_physical_format(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let documents = Documents::default(); let mut document = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            document = Some(documents.get_or_create("fixture".into(), "a.txt".into(), Document::new(b"old\nline\n".to_vec()).unwrap(),
                |text,cx| cx.new(|cx| EditorState::new(window,cx).default_value(text.to_owned())), cx));
            let empty = cx.new(|_| Empty); Root::new(empty,window,cx)
        });
        let document = document.unwrap(); let identity = document.entity_id();
        cx.update(|window,cx| document.update(cx, |s,cx| {
            s.editor.update(cx,|e,cx| e.set_cursor_position(Position::new(1,2), window,cx));
            let editor_id = s.editor.entity_id(); let old = s.document.disk();
            let stamp = Stamp::read(std::path::Path::new("Cargo.toml")).unwrap();
            s.finish_observation(&old, false, Ok(Some((stamp.clone(), b"\xef\xbb\xbfnew\r\nline\r\n".to_vec()))), window,cx);
            assert_eq!(s.editor.entity_id(),editor_id);
            assert_eq!(s.editor.read(cx).cursor(),6);
            assert_eq!(s.editor.read(cx).text().to_string(),"new\nline\n");
            assert!(!s.pending(cx));
            assert_eq!(&*s.document.disk(), b"\xef\xbb\xbfnew\r\nline\r\n");
            // A worker started before the reload must not replace its new baseline.
            s.finish_observation(&old, false, Ok(Some((stamp.clone(), b"stale".to_vec()))),window,cx);
            assert_eq!(s.document.text(),"new\nline\n");
            let current = s.document.disk();
            s.finish_observation(&current, true, Ok(Some((stamp, b"locked".to_vec()))),window,cx);
            assert_eq!(s.document.text(),"new\nline\n");
        }));
        assert_eq!(documents.get(&"a.txt".into()).unwrap().entity_id(),identity);
    }
    #[gpui::test]
    fn external_conflict_preserves_draft_and_never_recreates_deleted_file(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let documents = Documents::default(); let mut document = None;
        let (_, cx) = cx.add_window_view(|window,cx| {
            document = Some(documents.get_or_create("fixture".into(), "a.txt".into(), Document::new(b"old".to_vec()).unwrap(),
                |text,cx| cx.new(|cx| EditorState::new(window,cx).default_value(text.to_owned())),cx));
            let empty=cx.new(|_|Empty);Root::new(empty,window,cx)
        });
        let document=document.unwrap();
        cx.update(|window,cx| document.update(cx,|s,cx| {
            s.editor.update(cx,|e,cx|e.set_value("my draft",window,cx));
            // Also covers input/preedit which has not yet emitted Change.
            let old=s.document.disk(); let stamp=Stamp::read(std::path::Path::new("Cargo.toml")).unwrap();
            s.finish_observation(&old,false,Ok(Some((stamp,b"AI changes".to_vec()))),window,cx);
            assert!(s.external.conflict);assert!(s.pending(cx));
            assert_eq!(s.editor.read(cx).text().to_string(),"my draft");
            s.save(cx); assert!(!s.document.saving());
            s.finish_observation(&old,false,Err("deleted".into()),window,cx);
            assert!(s.external.conflict); assert_eq!(&*s.document.disk(),b"old");
            assert_eq!(s.editor.read(cx).text().to_string(),"my draft");
            // A user-confirmed reload clears the conflict, retaining the same editor.
            s.finish_reload("my draft",Ok(Document::new(b"resolved".to_vec()).unwrap()),window,cx);
            assert!(!s.external.conflict); assert!(!s.pending(cx));
        }));
    }
}
