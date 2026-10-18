#![cfg(feature = "desktop-tests")]
extern crate gpui_kit as gpui;
use gpui::prelude::*;
use gpui::{Entity, InteractiveElement, IntoElement, Window, div};
#[gpui::test]
fn focused_input_registry_releases_removed_editor(cx: &mut gpui::TestAppContext) {
    use gpui::component::{
        Root,
        input::{Editor, EditorState},
    };
    use gpui::{AppContext as _, Focusable as _, Render};
    struct Probe {
        editor: Option<Entity<EditorState>>,
        other: gpui::FocusHandle,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div()
                .child(div().track_focus(&self.other))
                .when_some(self.editor.clone(), |v, editor| {
                    v.child(Editor::new(&editor))
                })
        }
    }
    cx.update(gpui::component::init);
    let mut weak = None;
    let mut probe = None;
    let mut other = None;
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let editor =
                cx.new(|cx| EditorState::new(window, cx).default_value("read-only source"));
            weak = Some(editor.downgrade());
            let focus = cx.focus_handle();
            other = Some(focus.clone());
            let view = cx.new(|_| Probe {
                editor: Some(editor),
                other: focus,
            });
            probe = Some(view.clone());
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap()
    });
    let weak = weak.unwrap();
    let probe = probe.unwrap();
    let other = other.unwrap();
    let mut cx = gpui::VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.update(|window, cx| {
        weak.upgrade()
            .unwrap()
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.update(|window, cx| {
        probe.update(cx, |view, cx| {
            view.editor = None;
            cx.notify();
        });
        other.focus(window, cx);
    });
    cx.run_until_parked();
    for _ in 0..2 {
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    // A removed editor must be released without another input rendering or
    // calling focused_input(), which would hide a stale strong registration.
    assert!(
        weak.upgrade().is_none(),
        "window focus registry retained the closed editor"
    );
}
