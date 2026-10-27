use super::*;
use core::prelude::v1::test;
extern crate gpui_kit as gpui;
use gpui::component::Root;
use plico::interactive_rebase::todo::{Command, Entry};
fn model() -> Model<Arc<Item>> {
    model_with_count(4)
}
fn model_with_count(count: usize) -> Model<Arc<Item>> {
    Model::new(
        (0..count)
            .map(|i| {
                (
                    Arc::new(Item {
                        entry: Entry {
                            command: Command::Pick,
                            target: format!("{i:040x}"),
                            subject: format!("commit {i}"),
                        },
                        message: Some(format!("commit {i}\n\nbody {i}").into()),
                    }),
                    Action::Pick,
                )
            })
            .collect(),
    )
    .unwrap()
}
#[gpui::test]
fn inline_edit_commit_cancel_and_unchanged_text_release_start_gate(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let mut table = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| PlanTable::from_draft(Draft::new(model()).unwrap(), cx));
        table = Some(view.clone());
        Root::new(view, w, cx)
    });
    let table = table.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            let id = s.model().rows()[0].id;
            s.begin_message(id, w, cx);
            assert!(s.editing());
            assert!(s.apply_message(w, cx));
            assert!(!s.editing());
            assert!(!s.modified());
            s.begin_message(id, w, cx);
            s.editor.as_ref().unwrap().input.update(cx, |input, cx| {
                input.set_value("new subject\n\nnew body", w, cx)
            });
            assert!(s.apply_message(w, cx));
            assert!(s.modified());
            assert_eq!(
                s.model().rows()[0].action(),
                &Action::Reword("new subject\n\nnew body".into())
            );
            s.begin_message(id, w, cx);
            assert!(s.apply_message(w, cx));
            assert!(!s.editing());
        })
    });
    cx.run_until_parked();
}
#[gpui::test]
fn selection_tracks_reordered_group_and_squash_opens_root_editor(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let mut table = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| PlanTable::from_draft(Draft::new(model()).unwrap(), cx));
        table = Some(view.clone());
        Root::new(view, w, cx)
    });
    let table = table.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            s.select(1, Modifiers::default(), w, cx);
            let child = s.selected[0];
            s.unite(false, w, cx);
            assert_eq!(s.selected, vec![child]);
            assert_eq!(s.model().rows()[1].action(), &Action::Fixup);
            s.select(0, Modifiers::default(), w, cx);
            let root = s.selected[0];
            s.move_rows(1, w, cx);
            assert_eq!(s.selected, vec![root]);
            let index = s.model().index(root).unwrap();
            assert_eq!(s.model().rows()[index + 1].id, child);
            s.draft.reset();
            s.changed(cx);
            s.select(2, Modifiers::default(), w, cx);
            s.unite(true, w, cx);
            assert!(s.editing());
            assert_eq!(s.indices(), vec![1]);
            assert_eq!(s.model().rows()[2].action(), &Action::Fixup);
            assert_eq!(s.selected_items()[0].entry.subject, "commit 1");
            let before = s.model().rows()[1].action().clone();
            s.set_action(Action::Drop, cx);
            assert_eq!(s.model().rows()[1].action(), &before);
        })
    });
    cx.run_until_parked();
}
#[gpui::test]
fn keyboard_applies_inline_message_and_cancel_waits_for_choice(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let mut table = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| PlanTable::from_draft(Draft::new(model()).unwrap(), cx));
        table = Some(view.clone());
        Root::new(view, w, cx)
    });
    let table = table.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            s.begin_message(s.selected[0], w, cx);
            s.editor
                .as_ref()
                .unwrap()
                .input
                .update(cx, |input, cx| input.set_value("edited", w, cx));
        })
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    cx.simulate_keystrokes("ctrl-enter");
    cx.run_until_parked();
    table.read_with(cx, |s, _| assert!(!s.editing()));
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            s.begin_message(s.selected[0], w, cx);
            s.editor
                .as_ref()
                .unwrap()
                .input
                .update(cx, |input, cx| input.set_value("unsaved", w, cx));
            s.dismiss_editor(false, w, cx);
            assert!(s.editing());
            assert_eq!(
                s.model().rows()[0].action(),
                &Action::Reword("edited".into())
            );
        })
    });
    cx.run_until_parked();
    // Closing the confirmation keeps the uncommitted text available.
    cx.update(|w, cx| w.close_dialog(cx));
    table.read_with(cx, |s, cx| {
        assert_eq!(
            s.editor.as_ref().unwrap().input.read(cx).value().as_str(),
            "unsaved"
        )
    });
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            s.discard_editor(false, w, cx);
            assert!(!s.editing());
            assert_eq!(
                s.model().rows()[0].action(),
                &Action::Reword("edited".into())
            );
        })
    });
}

#[gpui::test]
fn reset_hides_link_without_clearing_cancel_guard(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let mut table = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| PlanTable::from_draft(Draft::new(model()).unwrap(), cx));
        table = Some(view.clone());
        Root::new(view, w, cx)
    });
    let table = table.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            assert!(!s.reset_visible);
            s.select(1, Modifiers::default(), w, cx);
            s.set_action(Action::Drop, cx);
            assert!(s.reset_visible);
            s.dismiss_editor(true, w, cx);
            assert_eq!(s.model().rows()[1].action(), &Action::Pick);
            assert!(!s.reset_visible);
            assert!(s.modified(), "Cancel must still confirm after Reset");
            s.select(2, Modifiers::default(), w, cx);
            s.begin_message(s.selected[0], w, cx);
            assert!(s.apply_message(w, cx));
            assert!(
                !s.reset_visible,
                "Selection and unchanged editing are not mutations"
            );
            s.focus.focus(w, cx);
        })
    });
    cx.simulate_keystrokes("ctrl-z");
    cx.run_until_parked();
    cx.update(|_, cx| {
        table.update(cx, |s, _| {
            assert!(s.reset_visible);
            assert_eq!(s.model().rows()[1].action(), &Action::Drop);
        })
    });
}

#[gpui::test]
fn mouse_drag_reorders_one_row_and_escape_preserves_draft(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let mut table = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| PlanTable::from_draft(Draft::new(model()).unwrap(), cx));
        table = Some(view.clone());
        Root::new(view, w, cx)
    });
    let table = table.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    cx.simulate_mouse_move(point(px(100.), px(41.)), None, Modifiers::default());
    cx.simulate_mouse_down(
        point(px(100.), px(41.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(px(100.), px(60.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(px(100.), px(85.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(100.), px(85.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    table.read_with(cx, |s, _| {
        assert_eq!(
            s.model()
                .rows()
                .iter()
                .map(|r| r.entry.entry.subject.as_str())
                .collect::<Vec<_>>(),
            vec!["commit 1", "commit 2", "commit 0", "commit 3"]
        );
        assert_eq!(s.indices(), vec![2]);
    });
    cx.simulate_mouse_down(
        point(px(100.), px(41.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(px(100.), px(85.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.simulate_keystrokes("escape");
    cx.simulate_mouse_up(
        point(px(100.), px(85.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    table.read_with(cx, |s, _| {
        assert_eq!(s.model().rows()[0].entry.entry.subject, "commit 1");
        assert!(s.drag_line(2).is_none());
    });
}

#[gpui::test]
fn reword_action_marks_cancel_guard_and_redo_restores_message(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let mut table = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| PlanTable::from_draft(Draft::new(model()).unwrap(), cx));
        table = Some(view.clone());
        Root::new(view, w, cx)
    });
    let table = table.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        table.update(cx, |s, cx| s.focus.focus(w, cx));
    });
    cx.simulate_keystrokes("f2");
    cx.run_until_parked();
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            assert!(s.editing() && s.modified() && s.reset_visible);
            s.editor.as_ref().unwrap().input.update(cx, |input, cx| {
                input.set_value("changed native message", w, cx);
            });
        })
    });
    cx.simulate_keystrokes("ctrl-enter ctrl-z");
    cx.run_until_parked();
    table.read_with(cx, |s, _| {
        assert_eq!(s.model().rows()[0].action(), &Action::Pick)
    });
    cx.simulate_keystrokes("ctrl-shift-z");
    cx.run_until_parked();
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            assert_eq!(
                s.model().rows()[0].action(),
                &Action::Reword("changed native message".into())
            );
            s.dismiss_editor(true, w, cx);
            assert!(!s.reset_visible);
            assert!(s.modified());
        })
    });
}

#[gpui::test]
fn drag_from_another_row_applies_message_without_reordering(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let mut table = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view = cx.new(|cx| PlanTable::from_draft(Draft::new(model()).unwrap(), cx));
        table = Some(view.clone());
        Root::new(view, w, cx)
    });
    let table = table.unwrap();
    cx.update(|w, cx| {
        table.update(cx, |s, cx| {
            s.begin_message(s.selected[0], w, cx);
            s.editor
                .as_ref()
                .unwrap()
                .input
                .update(cx, |input, cx| input.set_value("edited draft", w, cx));
        })
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    // The second row starts below the expanded message editor.
    cx.simulate_mouse_down(
        point(px(100.), px(170.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(px(100.), px(215.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(px(100.), px(215.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    table.read_with(cx, |s, _| {
        assert!(!s.editing());
        assert_eq!(
            s.model().rows()[0].action(),
            &Action::Reword("edited draft".into())
        );
        assert_eq!(
            s.model()
                .rows()
                .iter()
                .map(|r| r.entry.entry.subject.as_str())
                .collect::<Vec<_>>(),
            ["commit 0", "commit 1", "commit 2", "commit 3"]
        );
        assert!(s.drag_line(3).is_none());
        assert_eq!(s.indices(), vec![0]);
    });
}

#[gpui::test]
fn ending_message_edit_hit_tests_collapsed_rows(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let mut table = None;
    let (_, cx) = cx.add_window_view(|w, cx| {
        let view =
            cx.new(|cx| PlanTable::from_draft(Draft::new(model_with_count(12)).unwrap(), cx));
        table = Some(view.clone());
        Root::new(view, w, cx)
    });
    let table = table.unwrap();
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
    });
    let collapsed = table.read_with(cx, |s, _| {
        (0..12)
            .map(|i| s.list.bounds_for_item(i).unwrap())
            .collect::<Vec<_>>()
    });
    for edited in [0, 3] {
        cx.update(|w, cx| {
            table.update(cx, |s, cx| {
                s.begin_message(s.model().rows()[edited].id, w, cx);
                s.editor.as_ref().unwrap().input.update(cx, |input, cx| {
                    input.set_value("changed message", w, cx);
                });
            });
        });
        cx.run_until_parked();
        cx.update(|w, cx| {
            let _ = w.draw(cx);
        });
        let position = table.read_with(cx, |s, _| s.list.bounds_for_item(1).unwrap().center());
        let expected = collapsed
            .iter()
            .position(|b| b.contains(&position))
            .unwrap();
        if edited == 0 {
            assert!(
                expected > 1,
                "expanded row must displace the original click target"
            );
        } else {
            assert_eq!(expected, 1);
        }
        cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
        cx.update(|w, cx| {
            let _ = w.draw(cx);
        });
        cx.run_until_parked();
        table.read_with(cx, |s, _| {
            assert!(!s.editing());
            assert_eq!(s.indices(), vec![expected]);
            assert_eq!(
                s.model().rows()[edited].action(),
                &Action::Reword("changed message".into())
            );
            assert!(s.pending_mouse_selection.is_none());
        });
    }
}
