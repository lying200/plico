#![cfg(feature = "desktop-tests")]
extern crate gpui_kit as gpui;
use gpui::component::{
    Root,
    diff::{Diff, DiffFile, DiffLineRange, DiffSide, DiffState, DiffTint, ThreeWayRow},
};
use gpui::{
    Context, Entity, IntoElement, Modifiers, MouseButton, Render, Window, div, point, prelude::*,
    px, size,
};
struct Probe(Option<Entity<DiffState>>);
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().when_some(self.0.clone(), |v, state| {
            v.child(
                Diff::new(&state)
                    .size_full()
                    .header_visible(false)
                    .soft_wrap(false),
            )
        })
    }
}
#[gpui::test]
fn three_columns_copy_real_source_and_keep_navigation_and_selection_in_their_column(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui::component::init);
    let mut state = None;
    let mut probe = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let files = [
            DiffFile::unchanged("left.rs", "a\nLEFT\nextra\nz\n"),
            DiffFile::unchanged("result.rs", "a\nCENTER\nz\n"),
            DiffFile::unchanged("right.rs", "a\nRIGHT\nz\n"),
        ];
        let rows = vec![
            ThreeWayRow {
                lines: [Some(0); 3],
                tints: [None; 3],
            },
            ThreeWayRow {
                lines: [Some(1); 3],
                tints: [Some(DiffTint::Conflict); 3],
            },
            ThreeWayRow {
                lines: [Some(2), None, None],
                tints: [Some(DiffTint::Conflict); 3],
            },
            ThreeWayRow {
                lines: [Some(3), Some(2), Some(2)],
                tints: [None; 3],
            },
        ];
        let diff = cx.new(|cx| DiffState::new(files, cx));
        diff.update(cx, |diff, _| diff.set_three_way(rows).unwrap());
        state = Some(diff.clone());
        let view = cx.new(|_| Probe(Some(diff)));
        probe = Some(view.clone());
        Root::new(view, window, cx)
    });
    let state = state.unwrap();
    let probe = probe.unwrap();
    let weak = state.downgrade();
    cx.simulate_resize(size(px(900.), px(400.)));
    cx.run_until_parked();
    for _ in 0..3 {
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }
    cx.simulate_mouse_down(
        point(px(610.), px(35.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(610.), px(35.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    state.read_with(cx, |s, cx| {
        assert_eq!(s.selected_text(cx), "RIGHT\n");
        assert_eq!(s.source_position().unwrap().path().as_str(), "right.rs");
    });
    // A drag leaving the right pane must not include the center/left files.
    cx.simulate_mouse_down(
        point(px(670.), px(35.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(px(350.), px(35.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(350.), px(35.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    state.read_with(cx, |s, cx| {
        let selected = s.selected_text(cx);
        assert!(!selected.is_empty());
        assert!(
            "RIGHT".contains(&selected),
            "cross-pane drag copied another column: {selected:?}"
        );
    });
    cx.simulate_mouse_down(
        point(px(360.), px(35.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(360.), px(35.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    state.read_with(cx, |s, _| {
        let position = s.source_position().unwrap();
        assert_eq!(position.path().as_str(), "result.rs");
        assert_eq!(position.line(), 2);
    });
    cx.simulate_keystrokes("ctrl-a");
    state.read_with(cx, |s, cx| {
        assert_eq!(
            s.selected_text(cx),
            "a\nCENTER\nz\n",
            "Select All must act on the active column"
        )
    });
    cx.update(|_, cx| {
        state.update(cx, |s, cx| {
            s.set_selected_lines(
                Some(DiffLineRange::new("result.rs", DiffSide::Modified, 1, 3)),
                cx,
            )
        })
    });
    state.read_with(cx, |s, cx| {
        assert_eq!(
            s.selected_text(cx),
            "a\nCENTER\nz\n",
            "alignment padding must not enter source copies"
        )
    });
    cx.update(|_, cx| state.update(cx, |s, cx| s.next_change(cx)));
    state.read_with(cx, |s, _| {
        assert_eq!(
            s.source_position().unwrap().path().as_str(),
            "result.rs",
            "change navigation must focus the merged result"
        );
        assert!(s.selected_lines().is_none());
    });
    drop(state);
    cx.update(|_, cx| {
        probe.update(cx, |p, cx| {
            p.0 = None;
            cx.notify();
        })
    });
    for _ in 0..3 {
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }
    assert!(
        weak.upgrade().is_none(),
        "closing three-way view retained its documents"
    );
}
#[gpui::test]
fn three_way_rejects_duplicate_missing_or_out_of_order_source_lines(cx: &mut gpui::TestAppContext) {
    cx.update(gpui::component::init);
    let s = cx.new(|cx| {
        DiffState::new(
            [
                DiffFile::unchanged("a", "a\nb\n"),
                DiffFile::unchanged("b", "b\nc\n"),
                DiffFile::unchanged("c", "c\nd\n"),
            ],
            cx,
        )
    });
    s.update(cx, |s, _| {
        assert!(s.set_three_way(vec![]).is_err());
        assert!(
            s.set_three_way(vec![ThreeWayRow {
                lines: [Some(1), Some(0), Some(0)],
                tints: [None; 3]
            }])
            .is_err()
        );
        for indices in [[0, 0], [1, 0]] {
            let rows = indices
                .into_iter()
                .map(|i| ThreeWayRow {
                    lines: [Some(i); 3],
                    tints: [None; 3],
                })
                .collect();
            assert!(s.set_three_way(rows).is_err());
        }
        assert!(!s.is_three_way());
    });
}

#[gpui::test]
fn three_way_emphasis_validates_utf8_and_is_cleared_on_source_replacement(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui::component::init);
    cx.update(|cx| {
        let state = cx.new(|cx| {
            DiffState::new(
                [
                    DiffFile::unchanged("a.rs", "\t旧值\n"),
                    DiffFile::unchanged("b.rs", "\t新值\n"),
                    DiffFile::unchanged("c.rs", "\t新值\n"),
                ],
                cx,
            )
        });
        state.update(cx, |s, cx| {
            s.set_three_way(vec![ThreeWayRow {
                lines: [Some(0); 3],
                tints: [Some(DiffTint::Modified), Some(DiffTint::Modified), None],
            }])
            .unwrap();
            let mut inner: gpui::component::diff::ThreeWayInner = Default::default();
            inner[0].insert(0, vec![1..4]);
            inner[1].insert(0, vec![1..4]);
            s.set_three_inner(inner.clone()).unwrap();
            inner[0].insert(0, vec![2..4]);
            assert!(
                s.set_three_inner(inner.clone()).is_err(),
                "reject byte inside a Chinese codepoint"
            );
            inner[0].insert(0, vec![1..100]);
            assert!(s.set_three_inner(inner.clone()).is_err());
            inner[0].insert(0, vec![1..4]);
            inner[2].insert(0, vec![1..4]);
            assert!(
                s.set_three_inner(inner).is_err(),
                "unchanged source must not acquire accents"
            );
            s.set_files([DiffFile::unchanged("replacement.rs", "new text")], cx);
            assert!(!s.is_three_way());
            assert!(s.set_three_inner(Default::default()).is_err());
        });
    });
}
