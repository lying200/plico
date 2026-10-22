#![cfg(feature = "desktop-tests")]
extern crate gpui_kit as gpui;
use gpui::component::{
    Root,
    input::{Editor, EditorState, Position},
};
use gpui::{
    Context, Entity, IntoElement, Modifiers, MouseButton, Render, Window, div, point, prelude::*,
    px,
};
struct Probe(Entity<EditorState>);
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            Editor::new(&self.0)
                .appearance(false)
                .bordered(false)
                .h_full()
                .text_size(px(13.))
                .line_height(px(20.)),
        )
    }
}
#[gpui::test]
fn alignment_preserves_source_and_maps_clicks_and_vertical_navigation(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui::component::init);
    let mut editor = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value("first\nsecond\nthird")
                .soft_wrap(false)
                .folding(false)
        });
        editor = Some(state.clone());
        let probe = cx.new(|_| Probe(state));
        Root::new(probe, window, cx)
    });
    let editor = editor.unwrap();
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    // Locate a stable pixel in the second row with the actual editor padding.
    let mut second_y = None;
    for y in (20..65).step_by(5) {
        cx.simulate_mouse_down(
            point(px(100.), px(y as f32)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_up(
            point(px(100.), px(y as f32)),
            MouseButton::Left,
            Modifiers::default(),
        );
        if editor.read_with(cx, |e, _| e.cursor_position().line) == 1 {
            second_y = Some(y as f32);
            break;
        }
    }
    let second_y = second_y.expect("second source line must be visible");
    cx.update(|_, cx| editor.update(cx, |e, cx| e.set_block_gaps(vec![(1, 2)], cx).unwrap()));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    // The same real line is now exactly two visual rows lower.
    cx.simulate_mouse_down(
        point(px(100.), px(second_y + 40.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(100.), px(second_y + 40.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(editor.read_with(cx, |e, _| e.cursor_position().line), 1);
    cx.update(|window, cx| {
        editor.update(cx, |e, cx| {
            e.set_cursor_position(Position::new(0, 0), window, cx)
        })
    });
    cx.simulate_keystrokes("down");
    assert_eq!(
        editor.read_with(cx, |e, _| e.cursor_position().line),
        1,
        "down must cross padding without getting stuck"
    );
    cx.simulate_keystrokes("up");
    assert_eq!(editor.read_with(cx, |e, _| e.cursor_position().line), 0);
    editor.read_with(cx, |e, _| {
        assert_eq!(e.text().to_string(), "first\nsecond\nthird")
    });
    cx.simulate_input("X");
    editor.read_with(cx, |e, _| {
        assert_eq!(e.text().to_string(), "Xfirst\nsecond\nthird")
    });
    cx.simulate_keystrokes("ctrl-z");
    editor.read_with(cx, |e, _| {
        assert_eq!(e.text().to_string(), "first\nsecond\nthird")
    });

    cx.update(|_, cx| editor.update(cx, |e, cx| e.set_block_gaps(vec![], cx).unwrap()));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.simulate_mouse_down(
        point(px(100.), px(second_y)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(100.), px(second_y)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(
        editor.read_with(cx, |e, _| e.cursor_position().line),
        1,
        "clearing diff padding restores source layout"
    );
}

#[gpui::test]
fn peer_viewport_updates_do_not_echo_after_layout(cx: &mut gpui::TestAppContext) {
    use gpui::component::input::InputEvent;
    use std::{cell::RefCell, rc::Rc};
    cx.update(gpui::component::init);
    let mut editor = None;
    let events = Rc::new(RefCell::new(Vec::new()));
    let recorded = events.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value("line\n".repeat(300))
                .soft_wrap(false)
        });
        editor = Some(state.clone());
        cx.subscribe(&state, move |_, _, event: &InputEvent, _| {
            if let InputEvent::ScrollChanged { offset, .. } = event {
                recorded.borrow_mut().push(*offset);
            }
        })
        .detach();
        let probe = cx.new(|_| Probe(state));
        Root::new(probe, window, cx)
    });
    let editor = editor.unwrap();
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    events.borrow_mut().clear();
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            e.set_scroll_offset(point(px(0.), px(-200.)), cx)
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    assert_eq!(events.borrow().last().unwrap().y, px(-200.));
    events.borrow_mut().clear();
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            e.set_peer_scroll_offset(point(px(0.), px(-400.)), cx)
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    assert_eq!(editor.read_with(cx, |e, _| e.scroll_offset().y), px(-400.));
    assert!(
        events.borrow().is_empty(),
        "a mirrored viewport must not trigger feedback"
    );
    // A peer requesting past EOF must also remain silent after clamping.
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            e.set_peer_scroll_offset(point(px(0.), px(-100000.)), cx)
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    assert!(editor.read_with(cx, |e, _| e.scroll_offset().y) > px(-100000.));
    assert!(events.borrow().is_empty());
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            e.set_scroll_offset(point(px(0.), px(-600.)), cx)
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    assert_eq!(
        events.borrow().last().unwrap().y,
        px(-600.),
        "normal navigation resumes broadcasting"
    );
}

#[gpui::test]
fn realignment_preserves_visible_caret_or_top_source_row(cx: &mut gpui::TestAppContext) {
    cx.update(gpui::component::init);
    let mut editor = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value("line\n".repeat(300))
                .soft_wrap(false)
                .folding(false)
        });
        editor = Some(state.clone());
        let probe = cx.new(|_| Probe(state));
        Root::new(probe, window, cx)
    });
    let editor = editor.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    cx.update(|w, cx| {
        editor.update(cx, |e, cx| {
            e.set_cursor_position(Position::new(30, 0), w, cx);
            e.set_scroll_offset(point(px(0.), px(-400.)), cx);
        })
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    let before = editor.read_with(cx, |e, _| e.scroll_offset());
    assert_eq!(before.y, px(-400.));
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            let restored = e
                .set_block_gaps_preserving_position(vec![(25, 3)], cx)
                .unwrap();
            assert_eq!(restored.y, px(-460.));
        })
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    assert_eq!(editor.read_with(cx, |e, _| e.scroll_offset().y), px(-460.));
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            assert_eq!(
                e.set_block_gaps_preserving_position(vec![], cx).unwrap(),
                before
            );
        })
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    // Put caret outside the viewport without wheel input: the top source row,
    // not the invisible caret, must determine the adjustment.
    cx.update(|w, cx| {
        editor.update(cx, |e, cx| {
            e.set_cursor_position(Position::new(0, 0), w, cx);
            e.set_scroll_offset(before, cx);
        })
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            assert_eq!(
                e.set_block_gaps_preserving_position(vec![(10, 4)], cx)
                    .unwrap()
                    .y,
                px(-480.)
            );
            assert_eq!(e.text().to_string(), "line\n".repeat(300));
            assert_eq!(e.cursor_position().line, 0);
        })
    });
}
