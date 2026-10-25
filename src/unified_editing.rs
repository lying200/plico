//! Unified Diff is a projection; the working document owns text and Undo.
use crate::editing::Documents;
use gpui_kit::component::diff::{DiffFile, DiffSide, UnifiedEditProjection};
use gpui_kit::component::input::{EditorState, Position, TextDecoration, TextDecorationCollection};
use gpui_kit::*;
use std::{cell::RefCell, rc::Rc};

struct Mapping {
    projection: UnifiedEditProjection,
    source: String,
    valid: bool,
    composing: bool,
}
pub struct UnifiedEditing {
    pub editor: Entity<EditorState>,
    source: Entity<EditorState>,
    working_side: DiffSide,
    change_rows: Vec<usize>,
    mapping: Rc<RefCell<Mapping>>,
    marks: TextDecorationCollection,
}
impl UnifiedEditing {
    pub fn new(
        file: &DiffFile,
        working_side: DiffSide,
        source: Entity<EditorState>,
        documents: Documents,
        language: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let mapping = Rc::new(RefCell::new(Mapping {
            projection: UnifiedEditProjection::for_side(file, working_side),
            source: file.source(working_side).to_owned(),
            valid: true,
            composing: false,
        }));
        let editor = cx.new(|cx| {
            let mut editor = EditorState::new(window, cx)
                .tab_size(gpui_kit::component::input::TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .default_value(mapping.borrow().projection.text().to_owned())
                .language(language.to_owned())
                .line_number(false);
            let state = mapping.clone();
            let working = source.clone();
            let docs = documents.clone();
            editor.set_edit_guard(move |edits, cx| {
                let state = state.borrow();
                state.valid
                    && !docs.locked()
                    && working.read(cx).text().to_string() == state.source
                    && edits
                        .iter()
                        .all(|(r, _)| state.projection.working_range(r.clone()).is_some())
            });
            let state = mapping.clone();
            let working = source.clone();
            editor.set_edit_observer(move |edits, composition, window, cx| {
                let mut state = state.borrow_mut();
                for (range, text) in edits {
                    let Some(mapped) = state.projection.apply_edit(range.clone(), text) else {
                        state.valid = false;
                        return;
                    };
                    working.update(cx, |s, cx| {
                        s.apply_projected_edit(mapped.clone(), text, composition, window, cx)
                    });
                    state.source.replace_range(mapped, text);
                }
                state.composing = composition;
                // An unexpected source-side transformation must not leave an
                // editable mapping pointing at different text.
                state.valid = working.read(cx).text().to_string() == state.source;
            });
            let state = mapping.clone();
            let working = source.clone();
            editor.set_history_delegate(move |undo, window, cx| {
                state.borrow_mut().composing = false;
                working.update(cx, |s, cx| s.step_history(undo, window, cx));
                let mut state = state.borrow_mut();
                state.valid = working.read(cx).text().to_string() == state.source;
            });
            editor
        });
        let marks = editor.update(cx, |e, cx| e.create_decorations_collection(Vec::new(), cx));
        let mut view = Self {
            editor,
            source,
            working_side,
            change_rows: Vec::new(),
            mapping,
            marks,
        };
        view.refresh(file, window, cx);
        view
    }
    pub fn invalidate_if_changed(&self, cx: &App) {
        let mut m = self.mapping.borrow_mut();
        if self.source.read(cx).text().to_string() != m.source {
            m.valid = false;
        }
    }
    pub fn ready(&self) -> bool {
        self.mapping.borrow().valid
    }
    pub fn refresh(&mut self, file: &DiffFile, window: &mut Window, cx: &mut App) {
        let source = self.source.read(cx).text().to_string();
        if source != file.source(self.working_side) || self.mapping.borrow().composing {
            return;
        }
        let display_selection = self.editor.read(cx).selected_range();
        let source_selection = {
            let m = self.mapping.borrow();
            if m.valid {
                m.projection.working_range(display_selection)
            } else {
                None
            }
        }
        .unwrap_or_else(|| self.source.read(cx).selected_range());
        let mut inserted = 0;
        self.change_rows = file
            .change_line_ranges()
            .into_iter()
            .map(|(old, new)| {
                let row = old.start + inserted;
                inserted += new.len();
                row
            })
            .collect();
        let projection = UnifiedEditProjection::for_side(file, self.working_side);
        let selection = projection
            .display_offset(source_selection.start)
            .zip(projection.display_offset(source_selection.end));
        let offset = self.editor.read(cx).scroll_offset();
        let mut marks = Vec::new();
        for side in [DiffSide::Original, DiffSide::Modified] {
            for (range, style, whole) in file.unified_editor_highlights(side) {
                let ranges = if side != self.working_side {
                    projection.original_display_ranges(range)
                } else {
                    projection.display_ranges(range)
                };
                for shown in ranges {
                    let decoration = TextDecoration::new(shown, style);
                    marks.push(if whole {
                        decoration.whole_line()
                    } else {
                        decoration
                    });
                }
            }
        }
        self.editor.update(cx, |e, cx| {
            if e.text().to_string() != projection.text() {
                e.set_value(projection.text().to_owned(), window, cx);
            }
            if let Some((a, b)) = selection {
                e.set_selected_range(a..b, cx);
            }
            e.set_scroll_offset(offset, cx);
        });
        *self.mapping.borrow_mut() = Mapping {
            projection,
            source,
            valid: true,
            composing: false,
        };
        self.marks.set(marks, cx);
    }
    pub fn change_rows(&self) -> &[usize] {
        &self.change_rows
    }

    pub fn gutter(&self, cx: &App) -> impl IntoElement {
        let editor = self.editor.clone();
        let mapping = self.mapping.clone();
        let font_size = crate::ui_font::editor_size(cx);
        let font = gpui_kit::font(crate::ui_font::editor_family(cx));
        let digits = self
            .mapping
            .borrow()
            .projection
            .max_line_number()
            .to_string()
            .len()
            .max(2);
        let column = font_size * (digits as f32 * 0.65 + 0.6);
        canvas(
            move |bounds, _, cx| {
                let state = mapping.borrow();
                let rows = editor
                    .read(cx)
                    .visible_source_line_extents()
                    .into_iter()
                    .filter_map(|(line, top, bottom)| {
                        state
                            .projection
                            .line_numbers(line)
                            .map(|numbers| (numbers, top, bottom))
                    })
                    .collect::<Vec<_>>();
                (
                    bounds,
                    rows,
                    editor.read(cx).line_height().unwrap_or(font_size * 1.5),
                )
            },
            move |_, (bounds, rows, height), window, cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    for (numbers, top, _) in rows {
                        for (column_index, number) in numbers.into_iter().enumerate() {
                            let Some(number) = number else {
                                continue;
                            };
                            let text: SharedString = number.to_string().into();
                            let shaped = window.text_system().shape_line(
                                text.clone(),
                                font_size,
                                &[TextRun {
                                    len: text.len(),
                                    font: font.clone(),
                                    color: rgb(0x6f737a).into(),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                }],
                                None,
                            );
                            let _ = shaped.paint(
                                point(
                                    bounds.left() + column * (column_index + 1) as f32
                                        - shaped.width
                                        - px(3.),
                                    top,
                                ),
                                height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    }
                });
            },
        )
        .w(column * 2.)
        .h_full()
        .flex_shrink_0()
    }

    pub fn source_position(&self, baseline: &str, cx: &App) -> (bool, usize) {
        let offset = self.editor.read(cx).selected_range().end;
        let m = self.mapping.borrow();
        if let Some(offset) = m.projection.original_offset(offset) {
            return (
                self.working_side != DiffSide::Original,
                baseline[..offset].bytes().filter(|&c| c == b'\n').count(),
            );
        }
        let offset = m
            .projection
            .working_range(offset..offset)
            .map(|r| r.start)
            .unwrap_or(0);
        (
            self.working_side == DiffSide::Original,
            m.source[..offset].bytes().filter(|&c| c == b'\n').count(),
        )
    }
    pub fn navigate_to(&self, line: usize, window: &mut Window, cx: &mut App) -> bool {
        let m = self.mapping.borrow();
        let source = if line == 0 {
            0
        } else {
            m.source
                .match_indices('\n')
                .nth(line - 1)
                .map(|(i, _)| i + 1)
                .unwrap_or(m.source.len())
        };
        let Some(offset) = m.projection.display_offset(source) else {
            return true;
        };
        let row = m.projection.text()[..offset]
            .bytes()
            .filter(|&c| c == b'\n')
            .count();
        let Some(offsets) =
            crate::diff_scroll::navigation_offsets(&[self.editor.read(cx)], &[row..row + 1])
        else {
            return false;
        };
        self.editor.update(cx, |e, cx| {
            e.set_cursor_position(Position::new(row as u32, 0), window, cx);
            let mut offset = e.scroll_offset();
            offset.y = offsets[0];
            e.set_peer_scroll_offset(offset, cx);
        });
        true
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::{Documents, UnifiedEditing};
    use gpui::component::{
        Root,
        diff::{DiffFile, DiffSide},
        input::{Editor, EditorState, Position},
    };
    use gpui::{
        AppContext as _, Context, Entity, EntityInputHandler, IntoElement, ParentElement as _,
        Render, Styled as _, Window, div,
    };
    struct Probe(Entity<EditorState>);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Editor::new(&self.0)
                    .appearance(false)
                    .bordered(false)
                    .h_full(),
            )
        }
    }
    #[gpui::test]
    fn left_working_projection_edits_shared_source_and_protects_fixed_rows(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let baseline = "fixed\nkeep\n";
        let make_file =
            |text: &str| DiffFile::from_sources(Some(("a", text)), Some(("a", baseline))).unwrap();
        let mut state = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let source = cx.new(|cx| EditorState::new(window, cx).default_value("local\nkeep\n"));
            let view = UnifiedEditing::new(
                &make_file("local\nkeep\n"),
                DiffSide::Original,
                source.clone(),
                Documents::default(),
                "plain_text",
                window,
                cx,
            );
            let probe = cx.new(|_| Probe(view.editor.clone()));
            state = Some((source, view));
            Root::new(probe, window, cx)
        });
        let (source, mut view) = state.unwrap();
        let display = view.editor.clone();
        cx.run_until_parked();
        cx.update(|window, cx| {
            display.update(cx, |e, cx| {
                e.set_cursor_position(Position::new(0, 0), window, cx);
            })
        });
        cx.simulate_input("中");
        assert_eq!(
            source.read_with(cx, |e, _| e.text().to_string()),
            "中local\nkeep\n"
        );
        cx.update(|_, cx| assert_eq!(view.source_position(baseline, cx), (true, 0)));
        cx.update(|window, cx| {
            display.update(cx, |e, cx| {
                e.set_cursor_position(Position::new(1, 0), window, cx);
            })
        });
        cx.update(|_, cx| assert_eq!(view.source_position(baseline, cx), (false, 0)));
        cx.simulate_input("bad");
        assert_eq!(
            display.read_with(cx, |e, _| e.text().to_string()),
            "中local\nfixed\nkeep\n"
        );
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(
            source.read_with(cx, |e, _| e.text().to_string()),
            "local\nkeep\n"
        );
        cx.update(|window, cx| view.refresh(&make_file("local\nkeep\n"), window, cx));
        assert!(view.ready());
        cx.update(|window, cx| source.update(cx, |e, cx| e.set_value("external\n", window, cx)));
        cx.update(|_, cx| view.invalidate_if_changed(cx));
        assert!(!view.ready());
        cx.simulate_input("stale");
        assert_eq!(
            source.read_with(cx, |e, _| e.text().to_string()),
            "external\n"
        );
    }

    #[gpui::test]
    fn production_projection_shares_input_ime_and_history_and_rejects_external_stale_text(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let baseline = "removed\nkeep\n";
        let file = DiffFile::from_sources(Some(("a", baseline)), Some(("a", "keep\n"))).unwrap();
        let mut source = None;
        let mut unified = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let working = cx.new(|cx| {
                EditorState::new(window, cx)
                    .tab_size(gpui_kit::component::input::TabSize {
                        tab_size: 4,
                        hard_tabs: false,
                    })
                    .default_value("keep\n")
            });
            let view = UnifiedEditing::new(
                &file,
                DiffSide::Modified,
                working.clone(),
                Documents::default(),
                "plain_text",
                window,
                cx,
            );
            let probe = cx.new(|_| Probe(view.editor.clone()));
            source = Some(working);
            unified = Some(view);
            Root::new(probe, window, cx)
        });
        let source = source.unwrap();
        let mut unified = unified.unwrap();
        let display = unified.editor.clone();
        cx.run_until_parked();
        cx.update(|window, cx| {
            display.update(cx, |e, cx| {
                e.set_cursor_position(Position::new(1, 0), window, cx)
            })
        });
        cx.simulate_input("X");
        cx.simulate_input("Y");
        assert_eq!(
            source.read_with(cx, |s, _| s.text().to_string()),
            "XYkeep\n"
        );
        assert_eq!(
            display.read_with(cx, |s, _| s.text().to_string()),
            "removed\nXYkeep\n"
        );
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(
            source.read_with(cx, |s, _| s.text().to_string()),
            "keep\n",
            "typing uses shared coalesced history"
        );
        let file = DiffFile::from_sources(Some(("a", baseline)), Some(("a", "keep\n"))).unwrap();
        cx.update(|window, cx| unified.refresh(&file, window, cx));
        cx.simulate_keystrokes("ctrl-y");
        assert_eq!(
            source.read_with(cx, |s, _| s.text().to_string()),
            "XYkeep\n"
        );
        let file = DiffFile::from_sources(Some(("a", baseline)), Some(("a", "XYkeep\n"))).unwrap();
        cx.update(|window, cx| unified.refresh(&file, window, cx));
        // External edits must invalidate the old projection even before rediff.
        cx.update(|window, cx| source.update(cx, |s, cx| s.set_value("external\n", window, cx)));
        cx.update(|_, cx| unified.invalidate_if_changed(cx));
        assert!(!unified.ready());
        cx.simulate_input("bad");
        assert_eq!(
            source.read_with(cx, |s, _| s.text().to_string()),
            "external\n"
        );
        let file =
            DiffFile::from_sources(Some(("a", baseline)), Some(("a", "external\n"))).unwrap();
        cx.update(|window, cx| unified.refresh(&file, window, cx));
        assert!(unified.ready());
        let offset = unified
            .mapping
            .borrow()
            .projection
            .display_offset(0)
            .unwrap();
        let utf16 = unified.mapping.borrow().projection.text()[..offset]
            .encode_utf16()
            .count();
        cx.update(|window, cx| {
            display.update(cx, |e, cx| {
                e.replace_and_mark_text_in_range(Some(utf16..utf16), "中", Some(1..1), window, cx)
            })
        });
        assert_eq!(
            source.read_with(cx, |s, _| s.text().to_string()),
            "中external\n"
        );
        cx.update(|window, cx| {
            display.update(cx, |e, cx| {
                e.replace_text_in_range(None, "中文", window, cx)
            })
        });
        assert_eq!(
            source.read_with(cx, |s, _| s.text().to_string()),
            "中文external\n"
        );
        cx.simulate_keystrokes("ctrl-z");
        assert_eq!(
            source.read_with(cx, |s, _| s.text().to_string()),
            "external\n",
            "one shared IME transaction"
        );
    }
}
