//! Native conflict drafts retain block decisions through iterative review.
use super::*;
use gpui_kit::component::input::{
    Editor, EditorState, RopeExt as _, TextDecoration, TextDecorationCollection,
};
use plico::{
    conflicts::{File, Snapshot},
    merge_result::Prepared,
    text_merge::{Choice, Merge, Side},
};

mod gutter;
mod history;
use std::{cell::RefCell, rc::Rc};

struct MergeEditor {
    owner: WeakEntity<Review>,
    repository: PathBuf,
    prepared: Arc<Prepared>,
    inputs: [Entity<EditorState>; 3],
    model: Rc<RefCell<Merge>>,
    marks: [TextDecorationCollection; 3],
    selected: usize,
    initial: String,
    initial_decisions: plico::text_merge::State,
    applying: bool,
    _subscription: Subscription,
}
impl MergeEditor {
    fn new(
        owner: WeakEntity<Review>,
        repository: PathBuf,
        prepared: Arc<Prepared>,
        texts: [String; 3],
        model: Merge,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_decisions = model.state();
        let model = Rc::new(RefCell::new(model));
        let language = crate::project_view::file_language(&prepared.file().path);
        let inputs = std::array::from_fn(|i| {
            cx.new(|cx| {
                let mut e = EditorState::new(w, cx)
                    .default_value(texts[i].clone())
                    .language(language)
                    .line_number(i == 1);
                e.set_readonly(i != 1, cx);
                e
            })
        });
        history::attach(&inputs[1], model.clone(), cx);
        for (i, input) in inputs.iter().enumerate() {
            cx.subscribe(input, move |s, _, event: &InputEvent, cx| match event {
                InputEvent::LayoutChanged => cx.notify(),
                InputEvent::ScrollChanged { previous, offset } => {
                    let ranges = s.line_ranges(cx);
                    for j in 0..3 {
                        if j == i {
                            continue;
                        }
                        let pairs = ranges
                            .iter()
                            .map(|r| (r[i].clone(), r[j].clone()))
                            .collect::<Vec<_>>();
                        let y = (previous.y != offset.y)
                            .then(|| {
                                crate::diff_scroll::vertical_offset(
                                    s.inputs[i].read(cx),
                                    s.inputs[j].read(cx),
                                    &pairs,
                                    true,
                                    offset.y,
                                )
                            })
                            .flatten();
                        s.inputs[j].update(cx, |e, cx| {
                            let mut target = e.scroll_offset();
                            if let Some(y) = y {
                                target.y = y;
                            }
                            if previous.x != offset.x {
                                target.x = offset.x;
                            }
                            e.set_peer_scroll_offset(target, cx);
                        });
                    }
                    cx.notify();
                }
                _ => {}
            })
            .detach();
        }
        let subscription = cx.subscribe(&inputs[1], |s, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                s.model.borrow_mut().edited(&input.read(cx).value());
                s.highlight(cx);
                cx.notify();
            }
        });
        let marks = std::array::from_fn(|i| {
            inputs[i].update(cx, |e, cx| e.create_decorations_collection(Vec::new(), cx))
        });
        let mut state = Self {
            owner,
            repository,
            prepared,
            inputs,
            initial: texts[1].clone(),
            initial_decisions,
            applying: false,
            model,
            marks,
            selected: 0,
            _subscription: subscription,
        };
        state.navigate(false, cx);
        state
    }
    fn highlight(&self, cx: &mut App) {
        for i in 0..3 {
            let marks = self
                .model
                .borrow()
                .changes
                .iter()
                .filter_map(|c| {
                    if (i == 0 && !c.changed(Side::Yours)) || (i == 2 && !c.changed(Side::Theirs)) {
                        return None;
                    }
                    let range = [
                        c.yours_range.clone(),
                        c.result.clone(),
                        c.theirs_range.clone(),
                    ][i]
                        .clone();
                    let resolved = match i {
                        0 => c.sides[0],
                        2 => c.sides[1],
                        _ => c.resolved,
                    };
                    if resolved || range.is_empty() {
                        return None;
                    }
                    let color = gutter::color(c.kind());
                    Some(
                        TextDecoration::new(
                            range,
                            HighlightStyle {
                                background_color: Some(color.into()),
                                ..Default::default()
                            },
                        )
                        .whole_line(),
                    )
                })
                .collect();
            self.marks[i].set(marks, cx);
        }
    }
    fn sync(&mut self, cx: &App) {
        self.model
            .borrow_mut()
            .edited(&self.inputs[1].read(cx).value());
    }
    fn line_ranges(&self, cx: &App) -> Vec<[std::ops::Range<usize>; 3]> {
        self.model
            .borrow()
            .changes
            .iter()
            .map(|c| {
                let offsets = [&c.yours_range, &c.result, &c.theirs_range];
                std::array::from_fn(|i| {
                    let rope = self.inputs[i].read(cx).text();
                    let start = rope.offset_to_position(offsets[i].start).line as usize;
                    let end = rope.offset_to_position(offsets[i].end);
                    start
                        ..end.line as usize
                            + usize::from(!offsets[i].is_empty() && end.character > 0)
                })
            })
            .collect()
    }
    fn decide(
        &mut self,
        index: usize,
        side: Side,
        ignore: bool,
        whole: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.applying {
            return;
        }
        self.sync(cx);
        self.selected = index;
        if ignore {
            self.model.borrow_mut().ignore(index, side, whole);
            self.inputs[1].update(cx, |e, cx| e.record_history_state(cx));
        } else {
            let replacement = self.model.borrow_mut().accept(index, side, whole);
            if let Some(replacement) = replacement {
                history::replace(&self.inputs[1], &self.model, replacement, w, cx);
            }
        }
        self.inputs[1].read(cx).focus_handle(cx).focus(w, cx);
        self.highlight(cx);
        cx.notify();
    }
    fn choose(&mut self, choice: Choice, w: &mut Window, cx: &mut Context<Self>) {
        self.sync(cx);
        let replacement = self.model.borrow_mut().choose(self.selected, choice);
        if let Some(replacement) = replacement {
            history::replace(&self.inputs[1], &self.model, replacement, w, cx);
            self.inputs[1].read(cx).focus_handle(cx).focus(w, cx);
        }
        self.highlight(cx);
        cx.notify();
    }
    fn navigate(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.sync(cx);
        self.selected = if forward {
            (self.selected + 1).min(self.model.borrow().changes.len().saturating_sub(1))
        } else {
            self.selected.saturating_sub(1)
        };
        if let Some(change) = self.model.borrow().changes.get(self.selected) {
            for (i, offset) in [
                change.yours_range.start,
                change.result.start,
                change.theirs_range.start,
            ]
            .into_iter()
            .enumerate()
            {
                let text = self.inputs[i].read(cx).value();
                let line = text[..offset].bytes().filter(|b| *b == b'\n').count();
                let y = crate::ui_font::editor_size(cx) * 1.5 * (line.saturating_sub(3) as f32);
                self.inputs[i].update(cx, |e, cx| e.set_peer_scroll_offset(point(px(0.), -y), cx));
            }
        }
        self.highlight(cx);
        cx.notify();
    }
    fn apply(&mut self, complete: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.applying {
            return;
        }
        let prepared = self.prepared.clone();
        self.sync(cx);
        if complete {
            self.model.borrow_mut().mark_all_resolved();
        }
        let decisions = self.model.borrow().state();
        let text = self.inputs[1].read(cx).value().to_string();
        let path = self.repository.clone();
        let editor = cx.entity();
        self.applying = true;
        let owner = self.owner.clone();
        // Defer so completion cannot re-enter this entity if the operation gate
        // rejects immediately. Keep the editor until write-back succeeds.
        w.defer(cx, move |w, cx| {
            let callback = Box::new(move |success, w: &mut Window, cx: &mut App| {
                editor.update(cx, |s, cx| {
                    s.applying = false;
                    cx.notify();
                });
                if success {
                    w.close_dialog(cx);
                    w.close_dialog(cx);
                }
            });
            let _ = owner.update(cx, |s, cx| {
                if s.repo.path == path {
                    s.apply_merge_result(prepared, text, decisions, callback, w, cx);
                } else {
                    callback(false, w, cx);
                }
            });
        });
        cx.notify();
    }
    fn save_and_close(&mut self, w: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.applying {
            return false;
        }
        self.sync(cx);
        if self.model.borrow().text() == self.initial
            && self.model.borrow().state() == self.initial_decisions
        {
            let prepared = self.prepared.clone();
            let owner = self.owner.clone();
            cx.defer_in(w, move |_, _, cx| {
                let _ = owner.update(cx, |s, cx| {
                    s.mark_conflict_reviewed(&prepared, cx);
                });
            });
            return true;
        }
        self.apply(false, w, cx);
        false
    }
    fn confirm(&mut self, apply: bool, w: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.applying {
            return false;
        }
        self.sync(cx);
        let needs_confirmation = if apply {
            self.model.borrow().remaining() > 0
        } else {
            self.model.borrow().text() != self.initial
        };
        if !needs_confirmation {
            if apply {
                self.apply(true, w, cx);
            }
            return !apply;
        }
        let editor = cx.entity();
        w.open_dialog(cx,move |d,_,_| {
            let editor=editor.clone();
            d.title(t(if apply {"Unresolved changes"} else {"Discard merge result?"})).w(px(480.)).centered(true).overlay_closable(false)
                .child(div().whitespace_normal().child(t(if apply {"Some changes are unresolved. Apply the current result and mark this file as resolved?"} else {"Discard your edits to the merge result? The working file will remain unchanged."})))
                .footer(crate::dialog_style::confirmation(Button::new("confirm-merge-result").primary().label(t(if apply {"Apply and Mark Resolved"} else {"Discard"}))))
                .on_ok(move |_,w,cx| {
                    w.close_dialog(cx);
                    if apply { editor.update(cx,|s,cx| s.apply(true,w,cx)); }
                    else { w.close_dialog(cx); }
                    false
                })
        });
        false
    }
}
impl Render for MergeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_selected = self.model.borrow().changes.get(self.selected).is_some();
        v_flex()
            .capture_key_down(cx.listener(|s, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "f7" {
                    if !s.applying {
                        s.navigate(!event.keystroke.modifiers.shift, cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .truncate()
                    .child(self.prepared.file().path.to_string_lossy().into_owned()),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("merge-prev")
                            .ghost()
                            .small()
                            .icon(
                                gpui_kit::component::Icon::default()
                                    .path("jb/up_dark.svg")
                                    .size(px(16.)),
                            )
                            .tooltip(t("Previous difference · Shift+F7"))
                            .disabled(self.applying || self.selected == 0)
                            .on_click(cx.listener(|s, _, _, cx| s.navigate(false, cx))),
                    )
                    .child(
                        Button::new("merge-next")
                            .ghost()
                            .small()
                            .icon(
                                gpui_kit::component::Icon::default()
                                    .path("jb/down_dark.svg")
                                    .size(px(16.)),
                            )
                            .tooltip(t("Next difference · F7"))
                            .disabled(
                                self.applying
                                    || self.selected + 1 >= self.model.borrow().changes.len(),
                            )
                            .on_click(cx.listener(|s, _, _, cx| s.navigate(true, cx))),
                    )
                    .child(div().child(format!(
                        "{} / {} · {} {}",
                        if has_selected { self.selected + 1 } else { 0 },
                        self.model.borrow().changes.len(),
                        self.model.borrow().remaining(),
                        t("unresolved")
                    )))
                    .child(
                        Button::new("merge-auto")
                            .ghost()
                            .small()
                            .label(t("Apply Non-Conflicting Changes"))
                            .disabled(
                                self.applying
                                    || !(0..self.model.borrow().changes.len())
                                        .any(|i| self.model.borrow().automatic_choice(i).is_some()),
                            )
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.sync(cx);
                                let selected = s.selected;
                                let count = s.model.borrow().changes.len();
                                for i in 0..count {
                                    let choice = s.model.borrow().automatic_choice(i);
                                    if let Some(choice) = choice {
                                        s.selected = i;
                                        s.choose(choice, w, cx);
                                    }
                                }
                                s.selected = selected;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                h_flex().w_full().children(
                    ["Yours", "Merge Result", "Theirs"]
                        .into_iter()
                        .map(|label| div().flex_1().px_2().child(t(label))),
                ),
            )
            .child(
                div()
                    .relative()
                    .overflow_hidden()
                    .w_full()
                    .h((window.viewport_size().height - px(210.)).max(px(160.)))
                    .child(
                        h_flex()
                            .size_full()
                            .children(self.inputs.iter().enumerate().map(|(i, input)| {
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .when(i == 2, |v| {
                                        v.child(div().w(px(70.)).h_full().flex_shrink_0())
                                    })
                                    .child(
                                        div().flex_1().min_w_0().h_full().child(
                                            Editor::new(input)
                                                .readonly(i != 1 || self.applying)
                                                .appearance(false)
                                                .bordered(false)
                                                .h_full()
                                                .font_family(crate::ui_font::editor_family(cx))
                                                .text_size(crate::ui_font::editor_size(cx)),
                                        ),
                                    )
                                    .when(i == 0, |v| {
                                        v.child(div().w(px(70.)).h_full().flex_shrink_0())
                                    })
                                    .when(i < 2, |v| {
                                        v.child(div().w(px(32.)).h_full().flex_shrink_0())
                                    })
                            })),
                    )
                    .child(self.gutters(cx)),
            )
    }
}
impl Review {
    pub(super) fn open_text_merge(
        &mut self,
        snapshot: Arc<Snapshot>,
        file: File,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) =
            self.begin_conflict_operation(plico::active_operation::Phase::Preparing, cx)
        else {
            return;
        };
        let saved = self.saved_conflict_resolution(&snapshot, &file.path, cx);
        let repo = snapshot.repository();
        let project_path = self.repo.path.clone();
        let access = id.access.clone();
        self.conflict_task = cx.spawn_in(w, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    let prepared = match &saved {
                        Some(saved) => saved.prepared.clone(),
                        None => access.prepare(&repo, &snapshot, &file, &token)?,
                    };
                    let content = match prepared.native_text_inputs()? {
                        None => None,
                        Some(mut texts) => {
                            let mut model = Merge::new(&texts[1], &texts[0], &texts[2]);
                            if let Some(saved) = saved {
                                texts[1] = access.saved_text(&prepared, &repo, &token)?;
                                model.restore(&saved.decisions, &texts[1]);
                            }
                            Some((texts, model))
                        }
                    };
                    Ok::<_, String>((prepared, content))
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_conflict_operation(id, cx) else {
                    return;
                };
                if cancelled || s.repo.path != project_path {
                    return;
                }
                match result {
                    Err(e) => {
                        crate::error_ui::show_details(e, "Could not open merge editor", w, cx)
                    }
                    Ok((prepared, None)) => {
                        super::binary_merge::show(
                            cx.entity().downgrade(),
                            s.repo.path.clone(),
                            prepared,
                            w,
                            cx,
                        );
                    }
                    Ok((prepared, Some((texts, model)))) => {
                        let iterative = s.conflict_result_session(&prepared, cx).is_some();
                        let owner = cx.entity().downgrade();
                        let path = s.repo.path.clone();
                        let editor = cx
                            .new(|cx| MergeEditor::new(owner, path, prepared, texts, model, w, cx));
                        let result_editor = editor.read(cx).inputs[1].clone();
                        w.open_dialog(cx, move |d, w, _| {
                            let apply = editor.clone();
                            let cancel = editor.clone();
                            d.title(t("Merge Conflicts"))
                                .w((w.viewport_size().width - px(40.)).max(px(600.)))
                                .centered(true)
                                .overlay_closable(false)
                                .child(editor.clone())
                                .footer(
                                    gpui_kit::component::dialog::DialogFooter::new()
                                        .child(
                                            gpui_kit::component::dialog::DialogAction::new().child(
                                                Button::new("apply-native-merge")
                                                    .primary()
                                                    .label(t("Apply Changes")),
                                            ),
                                        )
                                        .child(
                                            gpui_kit::component::dialog::DialogClose::new()
                                                .trigger(move |b| {
                                                    b.label(t(if iterative {
                                                        "Save and Close"
                                                    } else {
                                                        "Cancel"
                                                    }))
                                                }),
                                        ),
                                )
                                .on_ok(move |_, w, cx| {
                                    apply.update(cx, |s, cx| s.confirm(true, w, cx))
                                })
                                .on_cancel(move |_, w, cx| {
                                    cancel.update(cx, |s, cx| {
                                        if iterative {
                                            s.save_and_close(w, cx)
                                        } else {
                                            s.confirm(false, w, cx)
                                        }
                                    })
                                })
                        });
                        result_editor.read(cx).focus_handle(cx).focus(w, cx);
                    }
                }
            });
        });
    }
}
