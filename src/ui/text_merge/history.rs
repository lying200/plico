//! Merge metadata shares the editor's typing/IME transactions; text remains
//! owned by the editor undo stack, not duplicated in each metadata snapshot.
use super::*;

pub(super) fn attach(editor: &Entity<EditorState>, model: Rc<RefCell<Merge>>, cx: &mut App) {
    editor.update(cx, |e, _| {
        let capture = model.clone();
        e.set_history_state(
            move |text| {
                let mut model = capture.borrow_mut();
                model.edited(text);
                Rc::new(model.state())
            },
            move |state, text| {
                if let Some(state) = state.downcast_ref::<plico::text_merge::State>() {
                    model.borrow_mut().restore(state, text);
                }
            },
        );
    });
}

pub(super) fn replace(
    editor: &Entity<EditorState>,
    model: &Rc<RefCell<Merge>>,
    (range, text): (std::ops::Range<usize>, String),
    w: &mut Window,
    cx: &mut App,
) {
    editor.update(cx, |e, cx| {
        e.set_selected_range(range, cx);
        // Accepting a deletion/equal result can change decisions without text.
        if e.value().as_ref() == model.borrow().text() {
            e.record_history_state(cx);
        } else {
            e.replace(text, w, cx);
        }
    });
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::{attach, replace};
    use gpui::component::Root;
    use gpui::component::input::{Editor, EditorState};
    use gpui::{
        AppContext as _, Context, Entity, EntityInputHandler, IntoElement, ParentElement as _,
        Render, Styled as _, Window, div,
    };
    use plico::text_merge::{Merge, Side};
    use std::{cell::RefCell, rc::Rc};

    struct Probe(Entity<EditorState>);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(Editor::new(&self.0).h_full())
        }
    }

    #[gpui::test]
    fn decisions_typing_and_ime_share_undo_and_redo(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let model = Rc::new(RefCell::new(Merge::new("base\n", "left\n", "right\n")));
        let mut editor = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let e = cx.new(|cx| EditorState::new(w, cx).default_value("base\n"));
            attach(&e, model.clone(), cx);
            editor = Some(e.clone());
            let probe = cx.new(|_| Probe(e));
            Root::new(probe, w, cx)
        });
        let editor = editor.unwrap();
        cx.run_until_parked();
        let step = |undo, cx: &mut gpui::VisualTestContext| {
            cx.update(|w, cx| editor.update(cx, |e, cx| e.step_history(undo, w, cx)));
        };
        cx.update(|w, cx| {
            let edit = model.borrow_mut().accept(0, Side::Yours, false).unwrap();
            replace(&editor, &model, edit, w, cx);
        });
        let accepted = model.borrow().state();
        cx.update(|w, cx| {
            editor.update(cx, |e, cx| {
                e.set_selected_range(4..4, cx);
                e.replace_text_in_range(None, "1", w, cx);
                e.replace_text_in_range(None, "2", w, cx);
            })
        });
        assert_eq!(model.borrow().text(), "left12\n");
        step(true, cx);
        assert_eq!(model.borrow().text(), "left\n");
        assert_eq!(model.borrow().state(), accepted);
        step(false, cx);
        assert_eq!(model.borrow().text(), "left12\n");
        cx.update(|w, cx| {
            let edit = model.borrow_mut().accept(0, Side::Theirs, false).unwrap();
            replace(&editor, &model, edit, w, cx);
        });
        assert_eq!(model.borrow().text(), "left12\nright\n");
        assert_eq!(model.borrow().remaining(), 0);
        step(true, cx);
        assert_eq!(model.borrow().text(), "left12\n");
        assert_eq!(model.borrow().changes[0].sides, [true, false]);
        step(false, cx);
        assert_eq!(model.borrow().remaining(), 0);
        step(true, cx);
        cx.update(|_, cx| {
            model.borrow_mut().ignore(0, Side::Theirs, false);
            editor.update(cx, |e, cx| e.record_history_state(cx));
        });
        assert_eq!(model.borrow().remaining(), 0);
        step(true, cx);
        assert_eq!(model.borrow().remaining(), 1);
        assert_eq!(model.borrow().text(), "left12\n");
        step(false, cx);
        assert_eq!(model.borrow().remaining(), 0);
        // IME is one text transaction and retains already resolved metadata.
        cx.update(|w, cx| {
            editor.update(cx, |e, cx| {
                e.set_selected_range(6..6, cx);
                e.replace_and_mark_text_in_range(None, "zh", None, w, cx);
                e.replace_and_mark_text_in_range(None, "中", None, w, cx);
                e.replace_text_in_range(None, "中", w, cx);
            })
        });
        assert_eq!(model.borrow().text(), "left12中\n");
        step(true, cx);
        assert_eq!(model.borrow().text(), "left12\n");
        assert_eq!(model.borrow().remaining(), 0);
        step(false, cx);
        assert_eq!(model.borrow().text(), "left12中\n");
        assert_eq!(model.borrow().remaining(), 0);
        step(true, cx); // IME
        step(true, cx); // ignore (no text)
        step(true, cx); // coalesced typing
        step(true, cx); // first side
        assert_eq!(model.borrow().text(), "base\n");
        assert_eq!(model.borrow().changes[0].sides, [false, false]);
        assert!(!model.borrow().changes[0].appended);
        // An unchanged-text choice still owns a decision transaction.
        cx.update(|w, cx| {
            let edit = model
                .borrow_mut()
                .choose(0, plico::text_merge::Choice::Base)
                .unwrap();
            replace(&editor, &model, edit, w, cx);
        });
        assert_eq!(model.borrow().remaining(), 0);
        step(true, cx);
        assert_eq!(model.borrow().remaining(), 1);
        assert_eq!(model.borrow().text(), "base\n");
        step(false, cx);
        assert_eq!(model.borrow().remaining(), 0);
    }
}
