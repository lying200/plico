use super::*;
use core::prelude::v1::test;
extern crate gpui_kit as gpui;
#[gpui::test]
fn changed_selection_and_close_reject_previous_results(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let view = cx.new(|cx| {
            Details::new(
                Repository::new("missing-rebase-details-fixture"),
                "missing-details-project".into(),
                cx,
            )
        });
        view.update(cx, |s, cx| {
            s.select(vec!["a".repeat(40)], cx);
            let old = s.generation;
            let old_token = s.token.clone();
            s.select(vec!["b".repeat(40)], cx);
            assert!(old_token.cancelled());
            s.accept(old, Err("old selection failure".into()), cx);
            assert!(s.error.is_none());
            assert!(s.loading);
            let current = s.generation;
            s.accept(
                current,
                Ok(Selection {
                    cards: vec![],
                    files: vec![],
                }),
                cx,
            );
            assert!(!s.loading);
            assert!(s.data.is_some());
            s.close();
            s.accept(current, Err("late failure".into()), cx);
            assert!(s.error.is_none());
            s.select(vec!["c".repeat(40)], cx);
            assert_eq!(s.generation, current);
        });
    });
}

#[gpui::test]
fn file_list_enter_does_not_confirm_parent_rebase(cx: &mut TestAppContext) {
    use gpui::component::{Root, WindowExt as _};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    cx.update(gpui::component::init);
    let mut details = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| {
            Details::new(
                Repository::new("missing-details-keyboard"),
                "missing-details-project".into(),
                cx,
            )
        });
        details = Some(view.clone());
        Root::new(view, w, cx)
    });
    let view = details.unwrap();
    let confirmed = Arc::new(AtomicBool::new(false));
    cx.update(|w, cx| {
        let view = view.clone();
        let confirmed = confirmed.clone();
        w.open_dialog(cx, move |dialog, _, _| {
            let confirmed = confirmed.clone();
            dialog.child(view.clone()).on_ok(move |_, _, _| {
                confirmed.store(true, Ordering::SeqCst);
                true
            })
        });
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let focus = view.read(cx).files_focus.clone();
        focus.focus(w, cx);
        let _ = w.draw(cx);
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(
        !confirmed.load(Ordering::SeqCst),
        "file-list Enter submitted the parent dialog"
    );
}

#[gpui::test]
fn visible_tree_navigation_preserves_parent_and_clears_hidden_file(cx: &mut TestAppContext) {
    use gpui::component::WindowExt as _;
    use plico::combined_changes::{Change, Revision};
    cx.update(gpui::component::init);
    let mut details = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| {
            Details::new(
                Repository::new("missing-navigation-fixture"),
                "missing-details-project".into(),
                cx,
            )
        });
        details = Some(view.clone());
        gpui::component::Root::new(view, w, cx)
    });
    let view = details.unwrap();
    cx.update(|w, cx| {
        let view = view.clone();
        w.open_dialog(cx, move |dialog, _, _| dialog.child(view.clone()));
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        view.update(cx, |s, cx| {
            s.data = Some(Selection {
                cards: vec![],
                files: ["src/a.txt", "src/b.txt"]
                    .into_iter()
                    .map(|path| Change {
                        before: None,
                        after: Some(Revision {
                            revision: "a".repeat(40),
                            path: path.into(),
                        }),
                    })
                    .collect(),
            });
            s.rebuild();
            s.files_focus.focus(w, cx);
        });
        let _ = w.draw(cx);
    });
    cx.simulate_keystrokes("home enter");
    cx.run_until_parked();
    view.read_with(cx, |s, _| assert_eq!(s.rows.len(), 1));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |s, _| assert_eq!(s.rows.len(), 4));
    cx.simulate_keystrokes("home right right down");
    cx.run_until_parked();
    view.read_with(cx, |s, _| assert_eq!(s.selected, Some(1)));
    cx.simulate_keystrokes("left left");
    cx.run_until_parked();
    view.read_with(cx, |s, _| {
        assert_eq!(s.selected, None);
        assert_eq!(s.rows.len(), 2);
        assert_eq!(s.focused_row(), Some(1));
    });
    cx.simulate_keystrokes("right right");
    cx.run_until_parked();
    view.read_with(cx, |s, _| assert_eq!(s.selected, Some(0)));
    cx.simulate_keystrokes("right");
    view.update(cx, |s, _| {
        assert_eq!(s.selected, Some(1));
        s.set_grouped(false);
        assert_eq!(s.selected, Some(1));
        assert_eq!(s.focused_row(), Some(1));
    });
}

#[gpui::test]
fn directory_diff_uses_descendants_even_when_collapsed(cx: &mut TestAppContext) {
    use plico::combined_changes::{Change, Revision};
    cx.update(|cx| {
        let view = cx.new(|cx| {
            Details::new(
                Repository::new("missing-directory-fixture"),
                "missing-details-project".into(),
                cx,
            )
        });
        view.update(cx, |s, _| {
            s.data = Some(Selection {
                cards: vec![],
                files: ["other.txt", "src/a.txt", "src/b.txt", "src-other/c.txt"]
                    .into_iter()
                    .map(|path| Change {
                        before: None,
                        after: Some(Revision {
                            revision: "a".repeat(40),
                            path: path.into(),
                        }),
                    })
                    .collect(),
            });
            s.rebuild();
            assert_eq!(s.diff_selection().unwrap().0.len(), 4);
            let folder = s
                .rows
                .iter()
                .position(|row| navigation::Key::Folder("src".into()).matches(row))
                .unwrap();
            s.choose_row(folder);
            s.collapsed.insert("src".into());
            s.rebuild();
            let (files, index) = s.diff_selection().unwrap();
            assert_eq!(index, 0);
            assert_eq!(
                files
                    .iter()
                    .map(|f| f.after.as_ref().unwrap().path.to_str().unwrap())
                    .collect::<Vec<_>>(),
                ["src/a.txt", "src/b.txt"]
            );
            s.choose_row(0);
            assert_eq!(s.diff_selection().unwrap().0.len(), 4);
            s.set_grouped(false);
            s.choose_row(2);
            let (files, index) = s.diff_selection().unwrap();
            assert_eq!((files.len(), index), (4, 2));
        });
    });
}

#[gpui::test]
fn multiple_selection_limits_diff_and_shift_replaces_range(cx: &mut TestAppContext) {
    use plico::combined_changes::{Change, Revision};
    cx.update(gpui::component::init);
    let mut details = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| {
            Details::new(
                Repository::new("missing-multiple"),
                "missing-multiple-project".into(),
                cx,
            )
        });
        details = Some(view.clone());
        gpui::component::Root::new(view, w, cx)
    });
    let view = details.unwrap();
    cx.update(|w, cx| {
        view.update(cx, |s, cx| {
            s.data = Some(Selection {
                cards: vec![],
                files: ["a.txt", "b.txt", "c.txt"]
                    .into_iter()
                    .map(|p| Change {
                        before: None,
                        after: Some(Revision {
                            revision: "a".repeat(40),
                            path: p.into(),
                        }),
                    })
                    .collect(),
            });
            s.set_grouped(false);
            s.files_focus.focus(w, cx);
        });
        let _ = w.draw(cx);
    });
    // The real row mouse handlers: select a, then Ctrl-select c.
    cx.simulate_mouse_down(
        point(px(100.), px(41.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(100.), px(41.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let control = Modifiers {
        control: true,
        ..Modifiers::default()
    };
    cx.simulate_mouse_down(point(px(100.), px(85.)), MouseButton::Left, control);
    cx.simulate_mouse_up(point(px(100.), px(85.)), MouseButton::Left, control);
    cx.run_until_parked();
    view.read_with(cx, |s, _| {
        assert_eq!(s.selected_files(), [0, 2]);
        let (files, index) = s.diff_selection().unwrap();
        assert_eq!(index, 0);
        assert_eq!(
            files
                .iter()
                .map(|f| f.after.as_ref().unwrap().path.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["a.txt", "c.txt"]
        );
    });
    cx.simulate_keystrokes("shift-up");
    cx.run_until_parked();
    view.update(cx, |s, _| {
        assert_eq!(s.selected_files(), [1, 2]);
        s.set_grouped(true);
        assert_eq!(s.selected_files(), [1, 2]);
        // Selecting the root and changing grouping preserves its descendant set.
        s.choose_row(0);
        s.set_grouped(false);
        assert_eq!(s.selected_files(), [0, 1, 2]);
        s.choose_row(0);
        s.select_row(2, true, false);
        s.select_row(2, true, false);
        assert_eq!(s.selected_files(), [0]);
        assert_eq!(s.diff_selection().unwrap().1, 0);
    });
}

#[gpui::test]
fn folder_label_selects_and_disclosure_alone_collapses(cx: &mut TestAppContext) {
    use plico::combined_changes::{Change, Revision};
    cx.update(gpui::component::init);
    let mut details = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| {
            Details::new(
                Repository::new("missing-folder-click"),
                "missing-folder-project".into(),
                cx,
            )
        });
        details = Some(view.clone());
        gpui::component::Root::new(view, w, cx)
    });
    let view = details.unwrap();
    cx.update(|w, cx| {
        view.update(cx, |s, _| {
            s.data = Some(Selection {
                cards: vec![],
                files: vec![Change {
                    before: None,
                    after: Some(Revision {
                        revision: "a".repeat(40),
                        path: "a.txt".into(),
                    }),
                }],
            });
            s.set_grouped(true);
        });
        let _ = w.draw(cx);
    });
    let label = point(px(100.), px(41.));
    cx.simulate_mouse_down(label, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(label, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |s, _| {
        assert_eq!(
            s.rows.len(),
            2,
            "label click must not collapse the directory"
        );
        assert_eq!(s.selected_files(), [0]);
    });
    let arrow = point(px(12.), px(41.));
    for expected_rows in [1, 2] {
        cx.simulate_mouse_down(arrow, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(arrow, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        view.read_with(cx, |s, _| {
            assert_eq!(s.rows.len(), expected_rows);
            assert_eq!(s.selected_files(), [0]);
        });
    }
}
