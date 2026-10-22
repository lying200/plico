#![cfg(feature = "desktop-tests")]
extern crate gpui_kit as gpui;
use gpui::component::{
    Root,
    diff::{Diff, DiffFile, DiffMode, DiffSide, DiffState},
};
use gpui::{
    Context, Entity, IntoElement, Modifiers, MouseButton, Render, Window, div, point, prelude::*,
    px,
};

struct Probe(Entity<DiffState>);
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(Diff::new(&self.0).header_visible(false))
    }
}
#[gpui::test]
fn body_press_records_source_without_selecting_a_whole_line(cx: &mut gpui::TestAppContext) {
    cx.update(gpui::component::init);
    let mut state = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let diff = cx.new(|cx| DiffState::new(DiffFile::parse("diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n first\n-old\n+new\n last\n").unwrap(), cx).with_mode(DiffMode::Split));
        state = Some(diff.clone());
        let probe = cx.new(|_| Probe(diff));
        Root::new(probe, window, cx)
    });
    let state = state.unwrap();
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    // Headerless split Diff: press the second source row, away from its gutter.
    for (mode, x_fraction, y, side) in [
        (DiffMode::Split, 0.75, 60., DiffSide::Modified),
        (DiffMode::Split, 0.25, 60., DiffSide::Original),
        (DiffMode::Unified, 0.5, 60., DiffSide::Original),
        (DiffMode::Unified, 0.5, 85., DiffSide::Modified),
    ] {
        cx.update(|_, cx| state.update(cx, |state, cx| state.set_mode(mode, cx)));
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let x = cx.update(|window, _| window.viewport_size().width * x_fraction);
        cx.simulate_mouse_down(point(x, px(y)), MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(point(x, px(y)), MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        state.read_with(cx, |diff, _| {
            let position = diff
                .source_position()
                .expect("body click must retain a navigation position");
            assert_eq!(position.side(), side);
            assert_eq!(position.line(), 2);
            assert!(
                diff.selected_lines().is_none(),
                "body click must not select the entire row"
            );
        });
    }
    cx.update(|_, cx| state.update(cx, |diff, cx| diff.set_files([], cx)));
    state.read_with(cx, |diff, _| {
        assert!(
            diff.source_position().is_none(),
            "replacement must discard stale source coordinates"
        )
    });
}

#[test]
fn editable_navigation_anchors_include_one_sided_changes_and_utf8() {
    let file = DiffFile::from_sources(
        Some(("a", "删除\n共同\n旧\n尾\n")),
        Some(("a", "共同\n新\n尾\n新增\n")),
    )
    .unwrap();
    assert_eq!(file.change_starts(), vec![(0, 0), (2, 1), (4, 3)]);
    let old = file.changed_ranges(DiffSide::Original);
    let new = file.changed_ranges(DiffSide::Modified);
    assert_eq!(
        old.iter()
            .map(|r| &file.source(DiffSide::Original)[r.clone()])
            .collect::<Vec<_>>(),
        vec!["删除\n", "旧\n"]
    );
    assert_eq!(
        new.iter()
            .map(|r| &file.source(DiffSide::Modified)[r.clone()])
            .collect::<Vec<_>>(),
        vec!["新\n", "新增\n"]
    );
    let unchanged = DiffFile::from_sources(Some(("a", "same")), Some(("a", "same"))).unwrap();
    assert!(unchanged.change_starts().is_empty());
}

#[test]
fn editable_change_ranges_preserve_unchanged_text_and_empty_boundaries() {
    for (old, new, expected) in [
        ("", "新\n", vec![(0..0, 0..1)]),
        ("旧\n", "", vec![(0..1, 0..0)]),
        ("a\n旧一\n旧二\nz\n", "a\n新\nz\n", vec![(1..3, 1..2)]),
        ("a\n", "头\na\n尾", vec![(0..0, 0..1), (1..1, 2..3)]),
        ("a", "a\n", vec![(0..1, 0..1)]),
        ("same\n", "same\n", vec![]),
    ] {
        let file = DiffFile::from_sources(Some(("a", old)), Some(("a", new))).unwrap();
        assert_eq!(file.change_line_ranges(), expected, "{old:?} -> {new:?}");
        assert_eq!(file.source(DiffSide::Original), old);
        assert_eq!(file.source(DiffSide::Modified), new);
    }
    // Verify boundaries against actual text, not just the number of ranges.
    // Every stretch outside the changed ranges must be equal on both sides.
    let documents = [
        "",
        "a",
        "a\n",
        "a\nb\n",
        "b\na\n",
        "中文\na\n末",
        "a\na\nb\n",
        "\n\na\n",
    ];
    for old in documents {
        for new in documents {
            let file = DiffFile::from_sources(Some(("a", old)), Some(("a", new))).unwrap();
            let old_lines: Vec<_> = old.split_inclusive('\n').collect();
            let new_lines: Vec<_> = new.split_inclusive('\n').collect();
            let (mut a, mut b) = (0, 0);
            for (left, right) in file.change_line_ranges() {
                assert_eq!(old_lines[a..left.start], new_lines[b..right.start]);
                assert!(left.end <= old_lines.len() && right.end <= new_lines.len());
                assert!(!left.is_empty() || !right.is_empty());
                a = left.end;
                b = right.end;
            }
            assert_eq!(old_lines[a..], new_lines[b..]);
        }
    }
}
