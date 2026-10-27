use super::*;
use core::prelude::v1::test;
extern crate gpui_kit as gpui;
use gpui::component::Root;
use plico::{
    git::Cancel,
    interactive_rebase::{
        interaction::Requests,
        model::{Action, Model, conversion::Item},
        todo::{Command, Entry},
    },
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
struct Background;
impl Render for Background {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}
fn pending(question: Question) -> (Pending, Requests, std::thread::JoinHandle<Option<Answer>>) {
    let requests = Requests::new(Cancel::default());
    let worker = requests.clone();
    let task = std::thread::spawn(move || worker.ask(question, &Cancel::default()).unwrap());
    let start = Instant::now();
    let pending = loop {
        if let Some(p) = requests.take_pending() {
            break p;
        }
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(1));
    };
    (pending, requests, task)
}
fn question() -> Question {
    Question::Plan(
        Model::new(vec![(
            Arc::new(Item {
                entry: Entry {
                    command: Command::Pick,
                    target: "a".repeat(40),
                    subject: "original".into(),
                },
                message: Some("original".into()),
            }),
            Action::Pick,
        )])
        .unwrap(),
        plico::editor::MessageEncoding::default(),
    )
}
#[gpui::test]
fn confirm_delivers_plan_once_and_close_cancels_waiting_worker(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let (_, cx) = cx.add_window_view(|w, cx| Root::new(cx.new(|_| Background), w, cx));
    for confirm in [false, true] {
        let (pending, _requests, worker) = pending(question());
        let dialog = cx.update(|w, cx| {
            show(
                Repository::new("missing-rebase-dialog-fixture"),
                "missing-plan-project".into(),
                pending,
                w,
                cx,
            )
            .unwrap()
        });
        cx.update(|w, cx| {
            dialog.update(cx, |s, cx| {
                assert!(s.active());
                assert!(s.can_confirm(cx));
                if confirm {
                    assert!(s.confirm(cx));
                    assert!(!s.confirm(cx));
                } else {
                    s.close(cx);
                }
                assert!(!s.active());
                assert_eq!(s.dismissed(), !confirm);
                w.close_dialog(cx);
            })
        });
        let answer = worker.join().unwrap();
        assert_eq!(answer.is_some(), confirm);
        if let Some(Answer::Plan(model)) = answer {
            assert_eq!(model.rows()[0].entry.entry.subject, "original");
        }
    }
}
#[gpui::test]
fn cancelled_operation_disables_still_open_noop_confirmation(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let (_, cx) = cx.add_window_view(|w, cx| Root::new(cx.new(|_| Background), w, cx));
    let (pending, requests, worker) = pending(Question::Noop);
    let dialog = cx.update(|w, cx| {
        show(
            Repository::new("missing-rebase-dialog-fixture"),
            "missing-plan-project".into(),
            pending,
            w,
            cx,
        )
        .unwrap()
    });
    requests.close();
    cx.update(|w, cx| {
        dialog.update(cx, |s, cx| {
            assert!(!s.can_confirm(cx));
            assert!(!s.confirm(cx));
            s.close(cx);
            w.close_dialog(cx);
        })
    });
    assert!(worker.join().unwrap().is_none());
}

#[gpui::test]
fn initial_split_uses_both_panel_proportions_without_changing_draft(cx: &mut TestAppContext) {
    cx.update(gpui::component::init);
    let (_, cx) = cx.add_window_view(|w, cx| Root::new(cx.new(|_| Background), w, cx));
    let (pending, _requests, worker) = pending(question());
    let dialog = cx.update(|w, cx| {
        let dialog = show(
            Repository::new("missing-split-fixture"),
            "missing-plan-project".into(),
            pending,
            w,
            cx,
        )
        .unwrap();
        dialog.update(cx, |s, _| s.fraction = 0.5);
        dialog
    });
    cx.run_until_parked();
    cx.update(|w, cx| {
        let _ = w.draw(cx);
        dialog.read_with(cx, |s, cx| {
            let sizes = s.split.read(cx).sizes();
            assert_eq!(sizes.len(), 2);
            assert!(sizes[0].as_f32() > 0.);
            assert!(
                (sizes[0].as_f32() - sizes[1].as_f32()).abs() < 3.,
                "{sizes:?}"
            );
            assert!(!s.table.as_ref().unwrap().read(cx).modified());
        });
        let before = dialog.read(cx).split.read(cx).sizes().to_vec();
        dialog.update(cx, |s, cx| {
            s.error = Some("Message cannot be represented in Git editor encoding".into());
            cx.notify();
        });
        let _ = w.draw(cx);
        let after = dialog.read(cx).split.read(cx).sizes().to_vec();
        for (before, after) in before.iter().zip(&after) {
            assert!(
                (before.as_f32() - after.as_f32()).abs() < 3.,
                "Error must not squeeze panels"
            );
        }
        dialog.update(cx, |s, cx| s.close(cx));
        w.close_dialog(cx);
    });
    assert!(worker.join().unwrap().is_none());
}

#[gpui::test]
fn unrepresentable_reword_keeps_plan_open_and_worker_waiting(cx: &mut TestAppContext) {
    let root = std::env::temp_dir().join(format!("plico-plan-encoding-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for args in [
        vec!["init", "--quiet"],
        vec!["config", "i18n.commitEncoding", "ISO-8859-1"],
    ] {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    let repo = Repository::new(&root);
    let encoding = plico::editor::MessageEncoding::load(&repo, &Cancel::default()).unwrap();
    let Question::Plan(mut model, _) = question() else {
        unreachable!()
    };
    assert!(model.set(&[0], Action::Reword("中文".into())));
    let (pending, _requests, worker) = pending(Question::Plan(model, encoding));
    cx.update(gpui::component::init);
    let (_, cx) = cx.add_window_view(|w, cx| Root::new(cx.new(|_| Background), w, cx));
    let dialog =
        cx.update(|w, cx| show(repo, "missing-plan-project".into(), pending, w, cx).unwrap());
    cx.update(|_, cx| {
        dialog.update(cx, |s, cx| {
            assert!(!s.confirm(cx));
            assert!(s.active());
            assert!(s.is_open());
            assert!(s.error.is_some());
            assert!(!worker.is_finished());
            assert_eq!(
                s.table.as_ref().unwrap().read(cx).model().rows()[0].action(),
                &Action::Reword("中文".into())
            );
            // A corrected draft can still answer the same pending request.
            let mut corrected = s.table.as_ref().unwrap().read(cx).model().clone();
            assert!(corrected.set(&[0], Action::Reword("café".into())));
            s.table = Some(cx.new(|cx| PlanTable::from_draft(Draft::new(corrected).unwrap(), cx)));
            assert!(s.confirm(cx));
        })
    });
    assert!(matches!(worker.join().unwrap(), Some(Answer::Plan(_))));
    std::fs::remove_dir_all(root).unwrap();
}
