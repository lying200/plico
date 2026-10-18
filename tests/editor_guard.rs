#![cfg(feature = "desktop-tests")]
extern crate gpui_kit as gpui;
use gpui::{prelude::*, Entity, Render, Window, Context, IntoElement, div, px, EntityInputHandler};
use gpui::component::{Root, input::{Editor, EditorState, Position}};
struct Probe(Entity<EditorState>);
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(Editor::new(&self.0).appearance(false).bordered(false).h_full().line_height(px(20.)))
    }
}
#[gpui::test]
fn guarded_editor_rejects_typing_selection_deletion_and_ime(cx: &mut gpui::TestAppContext) {
    cx.update(gpui::component::init);
    let mut editor=None;
    let observed=std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let edits=observed.clone();
    let (_,cx)=cx.add_window_view(|window,cx| {
        let state=cx.new(|cx| {
            let mut e=EditorState::new(window,cx).default_value("readonly\nworking");
            e.set_edit_guard(|edits,_|edits.iter().all(|(range,_)|range.start>=9));
            e.set_edit_observer(move |batch,composition,_,_| {
                edits.borrow_mut().push((batch.iter().map(|(r,t)|(r.clone(),t.to_string())).collect::<Vec<_>>(),composition));
            });
            e
        });
        editor=Some(state.clone());
        let probe=cx.new(|_|Probe(state));Root::new(probe,window,cx)
    });
    let editor=editor.unwrap();cx.run_until_parked();
    cx.update(|window,cx|editor.update(cx,|e,cx|e.set_cursor_position(Position::new(0,0),window,cx)));
    cx.simulate_input("X");
    assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"readonly\nworking");
    cx.simulate_keystrokes("ctrl-a backspace");
    assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"readonly\nworking");
    // Exercise the composition path directly, using native UTF-16 offsets.
    cx.update(|window,cx|editor.update(cx,|e,cx|e.replace_and_mark_text_in_range(Some(0..0),"中",Some(1..1),window,cx)));
    assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"readonly\nworking");
    cx.update(|window,cx|editor.update(cx,|e,cx|e.set_cursor_position(Position::new(1,0),window,cx)));
    cx.update(|_,cx|cx.write_to_clipboard(gpui::ClipboardItem::new_string("new\n".into())));
    cx.simulate_keystrokes("ctrl-v");
    assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"readonly\nnew\nworking");
    cx.simulate_keystrokes("ctrl-z");
    assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"readonly\nworking");
    assert_eq!(&*observed.borrow(), &[
        (vec![(9..9,"new\n".to_owned())],false),
        (vec![(9..13,"".to_owned())],false),
    ], "rejected input must never be forwarded; undo reports its exact inverse");
    cx.update(|window,cx|editor.update(cx,|e,cx|e.replace_and_mark_text_in_range(Some(9..9),"中",Some(1..1),window,cx)));
    assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"readonly\n中working");
    assert_eq!(observed.borrow().last().unwrap(),&(vec![(9..9,"中".to_owned())],true));
    cx.update(|window,cx|editor.update(cx,|e,cx|e.replace_text_in_range(None,"中文",window,cx)));
    assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"readonly\n中文working");
    assert_eq!(observed.borrow().last().unwrap(),&(vec![(9..12,"中文".to_owned())],false));
    let count=observed.borrow().len();
    // Programmatic rebuilds must still work even if old guards cover the text.
    cx.update(|window,cx|editor.update(cx,|e,cx|e.set_value("rebuilt",window,cx)));
    assert_eq!(editor.read_with(cx,|e,_|e.text().to_string()),"rebuilt");
    assert_eq!(observed.borrow().len(),count,"rebuild must not echo to the source");
}

#[gpui::test]
fn projected_input_updates_shared_source_and_undo_uses_source_history(cx: &mut gpui::TestAppContext) {
    use gpui::component::diff::{DiffFile,UnifiedEditProjection};
    use std::{rc::Rc,cell::RefCell};
    cx.update(gpui::component::init);
    let baseline="removed\nkeep\n";
    let projection=Rc::new(RefCell::new(UnifiedEditProjection::new(&DiffFile::from_sources(Some(("a",baseline)),Some(("a","keep\n"))).unwrap())));
    let mut source=None;let mut editor=None;
    let (_,cx)=cx.add_window_view(|window,cx| {
        let master=cx.new(|cx|EditorState::new(window,cx).default_value("keep\n"));
        source=Some(master.clone());
        let state=cx.new(|cx| {
            let mut e=EditorState::new(window,cx).default_value(projection.borrow().text().to_owned());
            let p=projection.clone();
            e.set_edit_guard(move |edits,_|edits.iter().all(|(r,_)|p.borrow().working_range(r.clone()).is_some()));
            let p=projection.clone();let m=master.clone();
            e.set_edit_observer(move |edits,composition,window,cx| {
                assert!(!composition,"this test covers committed input");
                for (r,text) in edits {
                    let mapped=p.borrow_mut().apply_edit(r.clone(),text).unwrap();
                    m.update(cx,|s,cx|{s.set_selected_range(mapped,cx);s.replace((*text).to_owned(),window,cx);});
                }
            });
            let p=projection.clone();let m=master.clone();let view=cx.weak_entity();
            e.set_history_delegate(move |undo,window,cx| {
                m.update(cx,|s,cx|s.step_history(undo,window,cx));
                let file=DiffFile::from_sources(Some(("a",baseline)),Some(("a",&m.read(cx).text().to_string()))).unwrap();
                *p.borrow_mut()=UnifiedEditProjection::new(&file);
                let text=p.borrow().text().to_owned();let view=view.clone();
                window.defer(cx,move |window,cx|{let _=view.update(cx,|s,cx|s.set_value(text,window,cx));});
            });
            e
        });
        editor=Some(state.clone());let probe=cx.new(|_|Probe(state));Root::new(probe,window,cx)
    });
    let source=source.unwrap();let editor=editor.unwrap();cx.run_until_parked();
    cx.update(|window,cx|editor.update(cx,|e,cx|e.set_cursor_position(Position::new(1,0),window,cx)));
    cx.simulate_input("X");cx.simulate_input("Y");
    assert_eq!(source.read_with(cx,|s,_|s.text().to_string()),"XYkeep\n");
    assert_eq!(editor.read_with(cx,|s,_|s.text().to_string()),"removed\nXYkeep\n");
    cx.simulate_keystrokes("ctrl-z");cx.run_until_parked();
    assert_eq!(source.read_with(cx,|s,_|s.text().to_string()),"Xkeep\n");
    assert_eq!(editor.read_with(cx,|s,_|s.text().to_string()),projection.borrow().text());
    cx.simulate_keystrokes("ctrl-y");cx.run_until_parked();
    assert_eq!(source.read_with(cx,|s,_|s.text().to_string()),"XYkeep\n");
    // Deleted content never reaches the source, including after rebuilding.
    cx.update(|window,cx|editor.update(cx,|e,cx|e.set_cursor_position(Position::new(0,0),window,cx)));
    cx.simulate_input("bad");
    assert_eq!(source.read_with(cx,|s,_|s.text().to_string()),"XYkeep\n");
}
