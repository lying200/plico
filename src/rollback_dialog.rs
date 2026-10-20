//! Rebased RollbackChangesDialog: inclusion list, legend, delete-new-files option.
use gpui_kit::component::{
    Disableable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    i18n::t,
    layout_preferences::{self, ChangesOption},
    rollback_operation::Prepared,
};
use std::{collections::BTreeSet, path::PathBuf, sync::Arc};
struct Options {
    prepared: Arc<Prepared>,
    selected: BTreeSet<usize>,
    delete_added: bool,
    error: Option<String>,
}
impl Render for Options {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_new = self
            .selected
            .iter()
            .any(|i| self.prepared.changes[*i].status == "A");
        let count = self.prepared.changes.len();
        let list = uniform_list(
            "rollback-files",
            count,
            cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|i| {
                        let change = &s.prepared.changes[i];
                        let label = change
                            .old_path
                            .as_ref()
                            .map(|old| format!("{} → {}", old.display(), change.path.display()))
                            .unwrap_or_else(|| change.path.display().to_string());
                        h_flex().h(px(28.)).px_2().gap_2().child(
                            Checkbox::new(("rollback-file", i))
                                .label(label)
                                .checked(s.selected.contains(&i))
                                .on_click(cx.listener(move |s, checked: &bool, _, cx| {
                                    if *checked {
                                        s.selected.insert(i);
                                    } else {
                                        s.selected.remove(&i);
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .h(px(280.))
        .border_1()
        .border_color(rgb(0x393b40));
        v_flex()
            .gap_3()
            .child(
                Checkbox::new("rollback-all")
                    .label(t("Changes"))
                    .checked(count > 0 && self.selected.len() == count)
                    .indeterminate(!self.selected.is_empty() && self.selected.len() < count)
                    .on_click(cx.listener(|s, checked: &bool, _, cx| {
                        s.selected = if *checked {
                            (0..s.prepared.changes.len()).collect()
                        } else {
                            BTreeSet::new()
                        };
                        cx.notify();
                    })),
            )
            .child(list)
            .child(format!(
                "{} / {} {}",
                self.selected.len(),
                count,
                t("changes selected")
            ))
            .child(
                Checkbox::new("rollback-delete-added")
                    .label(t("Delete locally added files"))
                    .checked(self.delete_added)
                    .disabled(!has_new)
                    .on_click(cx.listener(|s, v: &bool, _, cx| {
                        s.delete_added = *v;
                        cx.notify();
                    })),
            )
            .when_some(self.error.clone(), |v, e| {
                v.child(div().text_color(rgb(0xf2a6a6)).child(e))
            })
    }
}
pub fn show(
    root: PathBuf,
    prepared: Arc<Prepared>,
    selected: BTreeSet<usize>,
    window: &mut Window,
    cx: &mut App,
    submit: impl Fn(BTreeSet<usize>, bool, &mut Window, &mut App) + 'static,
) {
    let state = cx.new(|_| Options {
        prepared,
        selected,
        delete_added: layout_preferences::load_changes_option(&root, ChangesOption::DeleteAdded),
        error: None,
    });
    let submit = std::rc::Rc::new(submit);
    window.open_dialog(cx, move |dialog, _, cx| {
        let state = state.clone();
        let confirm = state.clone();
        let submit = submit.clone();
        let root = root.clone();
        dialog
            .title(t("Rollback Changes"))
            .w(px(620.))
            .centered(true)
            .overlay_closable(false)
            .child(t(
                "Selected changes will be discarded. This cannot be undone in Plico.",
            ))
            .child(state.clone())
            .footer(crate::dialog_style::confirmation(
                Button::new("rollback-ok")
                    .primary()
                    .label(t("Rollback"))
                    .disabled(state.read(cx).selected.is_empty()),
            ))
            .on_ok(move |_, w, cx| {
                let state = confirm.read(cx);
                if state.selected.is_empty() {
                    return false;
                }
                let selected = state.selected.clone();
                let delete = state.delete_added;
                if let Err(e) = layout_preferences::save_changes_option(
                    &root,
                    ChangesOption::DeleteAdded,
                    delete,
                ) {
                    confirm.update(cx, |s, cx| {
                        s.error = Some(e.to_string());
                        cx.notify();
                    });
                    return false;
                }
                submit(selected, delete, w, cx);
                true
            })
    });
}
