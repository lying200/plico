//! Source-only Git gutter and anchored original-content popup.
use crate::editing::{Documents, Editing};
use gpui_kit::base::Disableable as _;
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Editor, EditorState, InputEvent, Position},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::{Cancel, Repository},
    i18n::t,
    line_changes::{self, Block, Kind},
};
use std::{cell::Cell, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

struct Snapshot {
    original: Arc<str>,
    current: Arc<str>,
    blocks: Vec<Block>,
}
struct Popup {
    index: usize,
    position: Point<Pixels>,
    preview: Option<Entity<EditorState>>,
}
pub struct SourceChanges {
    editor: Entity<EditorState>,
    documents: Documents,
    repo: Repository,
    path: PathBuf,
    language: String,
    oid: Option<String>,
    original: Option<Arc<str>>,
    snapshot: Option<Arc<Snapshot>>,
    popup: Option<Popup>,
    generation: u64,
    token: Cancel,
    _watching: Task<()>,
    computing: Task<()>,
    reverting: Task<()>,
    busy: bool,
    error: Option<String>,
    scroll: Point<Pixels>,
    viewport: Rc<Cell<Option<Bounds<Pixels>>>>,
}
impl Drop for SourceChanges {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl SourceChanges {
    pub fn new(
        root: PathBuf,
        path: PathBuf,
        editing: &Entity<Editing>,
        documents: Documents,
        language: String,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(editing, |s, _, _: &crate::editing::Reloaded, cx| s.recompute(cx)).detach();
        let editor = editing.read(cx).editor.clone();
        let scroll = editor.read(cx).scroll_offset();
        cx.subscribe(&editor, |s, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                s.recompute(cx);
            }
        })
        .detach();
        cx.observe(&editor, |s, e, cx| {
            let scroll = e.read(cx).scroll_offset();
            if scroll != s.scroll {
                s.scroll = scroll;
                s.popup = None;
            }
            cx.notify();
        })
        .detach();
        let token = Cancel::default();
        let stop = token.clone();
        let repo = Repository::new(root);
        let worker_repo = repo.clone();
        let worker_path = path.clone();
        let watching = cx.spawn(async move |this, cx| {
            loop {
                if stop.cancelled() {
                    break;
                }
                if this
                    .read_with(cx, |s, _| s.documents.locked())
                    .unwrap_or(true)
                {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    continue;
                }
                let previous = this.read_with(cx, |s, _| s.oid.clone()).ok().flatten();
                let repo = worker_repo.clone();
                let path = worker_path.clone();
                let token = stop.clone();
                let result = cx
                    .background_spawn(async move {
                        let oid = line_changes::base_oid(&repo, &path, &token)?;
                        let text = if oid != previous {
                            oid.as_ref()
                                .map(|id| line_changes::base_text(&repo, id, &token))
                                .transpose()?
                        } else {
                            None
                        };
                        Ok::<_, String>((oid, text))
                    })
                    .await;
                if this
                    .update(cx, |s, cx| {
                        if s.token.cancelled() {
                            return;
                        }
                        match result {
                            Ok((oid, text)) => {
                                if oid != s.oid || s.original.is_none() && text.is_some() {
                                    s.oid = oid;
                                    s.original = text.map(Arc::from);
                                    s.recompute(cx);
                                }
                            }
                            Err(_) => {
                                s.oid = None;
                                s.original = None;
                                s.recompute(cx);
                            }
                        }
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(5)).await;
            }
        });
        Self {
            editor,
            documents,
            repo,
            path,
            language,
            oid: None,
            original: None,
            snapshot: None,
            popup: None,
            generation: 0,
            token,
            _watching: watching,
            computing: Task::ready(()),
            reverting: Task::ready(()),
            busy: false,
            error: None,
            scroll,
            viewport: Rc::new(Cell::new(None)),
        }
    }
    pub fn dismiss(&mut self, cx: &mut Context<Self>) -> bool {
        let had = self.popup.take().is_some();
        if had {
            cx.notify();
        }
        had
    }
    fn recompute(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        self.snapshot = None;
        self.popup = None;
        self.computing = Task::ready(());
        let Some(original) = self.original.clone() else {
            cx.notify();
            return;
        };
        let generation = self.generation;
        let editor = self.editor.clone();
        self.computing = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(80))
                .await;
            let current = editor.read_with(cx, |s, _| Arc::<str>::from(s.text().to_string()));
            let snapshot = cx
                .background_spawn(async move {
                    let blocks = line_changes::blocks(&original, &current);
                    Arc::new(Snapshot {
                        original,
                        current,
                        blocks,
                    })
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.generation == generation {
                    s.snapshot = Some(snapshot);
                    cx.notify();
                }
            });
        });
        cx.notify();
    }
    fn show(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(block) = snapshot.blocks.get(index) else {
            return;
        };
        let text = line_changes::fragment(&snapshot.original, block.original.clone());
        let preview = (!block.original.is_empty()).then(|| {
            cx.new(|cx| {
                let mut state = EditorState::new(window, cx)
                    .default_value(text)
                    .line_number(false)
                    .language(self.language.clone());
                state.set_readonly(true, cx);
                state
            })
        });
        self.error = None;
        self.popup = Some(Popup {
            index,
            position,
            preview,
        });
        cx.notify();
    }
    fn navigate(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(popup) = &self.popup else {
            return;
        };
        let index = if next {
            popup.index + 1
        } else {
            popup.index.saturating_sub(1)
        };
        let Some(block) = self.snapshot.as_ref().and_then(|s| s.blocks.get(index)) else {
            return;
        };
        let line = block
            .current
            .start
            .min(self.editor.read(cx).source_line_count().saturating_sub(1));
        self.editor.update(cx, |e, cx| {
            e.set_cursor_position(Position::new(line as u32, 0), window, cx)
        });
        self.popup = None;
        let owner = cx.entity().downgrade();
        let generation = self.generation;
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |s, cx| {
                if generation != s.generation {
                    return;
                }
                let Some(bounds) = s.viewport.get() else {
                    return;
                };
                let Some(block) = s.snapshot.as_ref().and_then(|s| s.blocks.get(index)) else {
                    return;
                };
                let editor = s.editor.read(cx);
                let Some((top, _)) = editor.source_line_extent(block.current.clone()) else {
                    return;
                };
                let Some(x) = editor.source_gutter_right() else {
                    return;
                };
                let position = point(
                    (x - bounds.left()).max(px(0.)),
                    (top - bounds.top() + px(22.))
                        .max(px(0.))
                        .min((bounds.size.height - px(310.)).max(px(0.))),
                );
                s.scroll = editor.scroll_offset();
                s.show(index, position, window, cx);
            });
        });
        cx.notify();
    }
    fn revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.documents.locked() {
            return;
        }
        let Some(popup) = &self.popup else {
            return;
        };
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        let Some(block) = snapshot.blocks.get(popup.index).cloned() else {
            return;
        };
        let generation = self.generation;
        let repo = self.repo.clone();
        let path = self.path.clone();
        let token = self.token.clone();
        let oid = self.oid.clone();
        self.busy = true;
        self.reverting = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move { line_changes::base_oid(&repo, &path, &token) })
                .await;
            let _ = this.update_in(wx, |s, window, cx| {
                s.busy = false;
                if generation != s.generation || s.documents.locked() || s.token.cancelled() {
                    cx.notify();
                    return;
                }
                if result.as_ref().ok() != Some(&oid) {
                    s.error = Some(t("Git baseline changed. Reopen the change marker.").into());
                    s.oid = None;
                    s.original = None;
                    s.snapshot = None;
                    cx.notify();
                    return;
                }
                s.apply_rollback(&snapshot, block, window, cx);
                cx.notify();
            });
        });
        cx.notify();
    }
    fn apply_rollback(
        &mut self,
        snapshot: &Snapshot,
        block: Block,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.documents.locked()
            || self.editor.read(cx).text().to_string() != snapshot.current.as_ref()
        {
            self.recompute(cx);
            return;
        }
        if let Some((range, text)) = crate::diff_revert::replacement(
            &snapshot.original,
            &snapshot.current,
            block.original,
            block.current,
        ) {
            self.popup = None;
            self.editor.update(cx, |e, cx| {
                e.set_selected_range(range, cx);
                e.replace(text, window, cx);
                e.focus_handle(cx).focus(window, cx);
            });
        }
    }
}
impl Render for SourceChanges {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.clone();
        let snapshot = self.snapshot.clone();
        let owner = cx.entity().downgrade();
        let generation = self.generation;
        let viewport = self.viewport.clone();
        let gutter = canvas(
            move |bounds, window, cx| {
                viewport.set(Some(bounds));
                let state = editor.read(cx);
                let mut marks = Vec::new();
                if let (Some(x), Some(snapshot)) = (state.source_gutter_right(), snapshot.as_ref())
                {
                    for (index, block) in snapshot.blocks.iter().enumerate() {
                        if let Some((top, bottom)) = state.source_line_extent(block.current.clone())
                        {
                            if bottom < bounds.top() || top > bounds.bottom() {
                                continue;
                            }
                            let y = top.max(bounds.top() + px(3.)).min(bounds.bottom() - px(3.));
                            let rect = Bounds::new(
                                point(x - px(6.), y),
                                size(px(4.), (bottom - y).max(px(3.))),
                            );
                            let hit = Bounds::new(
                                point(x - px(10.), (y - px(4.)).max(bounds.top())),
                                size(
                                    px(12.),
                                    (bottom - y + px(8.))
                                        .max(px(10.))
                                        .min(bounds.bottom() - y + px(4.)),
                                ),
                            );
                            marks.push((
                                index,
                                block.kind(),
                                rect,
                                window.insert_hitbox(hit, HitboxBehavior::Normal),
                            ));
                        }
                    }
                }
                (bounds, marks)
            },
            move |_, (bounds, marks), window, _cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    for (_, kind, rect, hit) in &marks {
                        let color = rgb(match kind {
                            Kind::Added => 0x549159,
                            Kind::Modified => 0x4a88c7,
                            Kind::Deleted => 0x868a91,
                        });
                        if *kind == Kind::Deleted {
                            // LineStatusMarkerDrawUtil.paintTriangle uses a rounded
                            // capsule in the modern UI, despite its historical name.
                            let dot = Bounds::new(
                                point(rect.left(), rect.top() - px(4.)),
                                size(rect.size.width, px(8.)),
                            );
                            window.paint_quad(quad(
                                dot,
                                px(4.),
                                color,
                                px(0.),
                                color,
                                BorderStyle::Solid,
                            ));
                        } else {
                            window.paint_quad(quad(
                                *rect,
                                px(2.),
                                color,
                                px(0.),
                                color,
                                BorderStyle::Solid,
                            ));
                        }
                        window.set_cursor_style(CursorStyle::PointingHand, hit);
                    }
                });
                let owner = owner.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if !phase.bubble() || event.button != MouseButton::Left {
                        return;
                    }
                    for (index, _, rect, hit) in &marks {
                        if hit.is_hovered(window) {
                            let position = point(
                                (rect.right() - bounds.left() + px(4.)).max(px(0.)),
                                (event.position.y - bounds.top() + px(8.))
                                    .min((bounds.size.height - px(310.)).max(px(0.))),
                            );
                            let _ = owner.update(cx, |s, cx| {
                                if generation == s.generation {
                                    s.show(*index, position, window, cx);
                                }
                            });
                            cx.stop_propagation();
                            break;
                        }
                    }
                });
            },
        )
        .size_full();
        let mut layer = div().absolute().inset_0().child(gutter);
        if let Some(popup) = &self.popup {
            let index = popup.index;
            let count = self.snapshot.as_ref().map_or(0, |s| s.blocks.len());
            let copy = self
                .snapshot
                .as_ref()
                .and_then(|s| {
                    s.blocks
                        .get(index)
                        .map(|b| line_changes::fragment(&s.original, b.original.clone()))
                })
                .unwrap_or_default();
            let panel = v_flex()
                .id("source-change-popup")
                .absolute()
                .left(popup.position.x)
                .top(popup.position.y)
                .w(px(530.))
                .max_w(relative(0.85))
                .bg(rgb(0x202124))
                .border_1()
                .border_color(rgb(0x43454a))
                .shadow_md()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                    s.dismiss(cx);
                }))
                .child(
                    h_flex()
                        .h(px(34.))
                        .px_1()
                        .gap_1()
                        .child(
                            Button::new("marker-prev")
                                .ghost()
                                .small()
                                .icon(Icon::default().path("jb/up_dark.svg"))
                                .tooltip(t("Previous change"))
                                .disabled(index == 0)
                                .on_click(cx.listener(|s, _, w, cx| s.navigate(false, w, cx))),
                        )
                        .child(
                            Button::new("marker-next")
                                .ghost()
                                .small()
                                .icon(Icon::default().path("jb/down_dark.svg"))
                                .tooltip(t("Next change"))
                                .disabled(index + 1 >= count)
                                .on_click(cx.listener(|s, _, w, cx| s.navigate(true, w, cx))),
                        )
                        .child(
                            Button::new("marker-revert")
                                .ghost()
                                .small()
                                .icon(Icon::default().path("jb/rollback_dark.svg"))
                                .tooltip(t("Rollback this change"))
                                .disabled(self.busy || self.documents.locked())
                                .on_click(cx.listener(|s, _, w, cx| s.revert(w, cx))),
                        )
                        .child(
                            Button::new("marker-copy")
                                .ghost()
                                .small()
                                .icon(Icon::default().path("jb/copy_dark.svg"))
                                .tooltip(t("Copy original lines"))
                                .disabled(copy.is_empty())
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                                }),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("marker-close")
                                .ghost()
                                .small()
                                .icon(Icon::default().path("jb/closeSmall.svg"))
                                .tooltip(t("Close"))
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.dismiss(cx);
                                })),
                        ),
                )
                .when_some(popup.preview.clone(), |v, preview| {
                    v.child(
                        div()
                            .h(crate::ui_font::editor_size(cx)
                                * 1.5
                                * (preview.read(cx).source_line_count().clamp(1, 12) as f32)
                                + px(8.))
                            .border_t_1()
                            .border_color(rgb(0x43454a))
                            .child(
                                Editor::new(&preview)
                                    .readonly(true)
                                    .appearance(false)
                                    .bordered(false)
                                    .h_full()
                                    .font_family(crate::ui_font::editor_family(cx))
                                    .text_size(crate::ui_font::editor_size(cx)),
                            ),
                    )
                })
                .when_some(self.error.clone(), |v, error| {
                    v.child(div().p_2().text_color(rgb(0xf2a6a6)).child(error))
                });
            layer = layer.child(deferred(panel).with_priority(2));
        }
        layer
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::{Block, Documents, EditorState, Snapshot, SourceChanges};
    use gpui::component::Root;
    use gpui::{AppContext as _, Context, IntoElement, Render, Task, Window, div};
    use plico::file_document::Document;
    use std::sync::Arc;
    struct Empty;
    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
    #[gpui::test]
    fn rollback_is_undoable_and_rejects_changed_or_locked_document(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let docs = Documents::default();
        let mut markers = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let editing = docs.get_or_create(
                "fixture".into(),
                "sample.txt".into(),
                Document::new(b"a\nnew\nz\nextra\n".to_vec()).unwrap(),
                |text, cx| cx.new(|cx| EditorState::new(window, cx).default_value(text.to_owned())),
                cx,
            );
            markers = Some(cx.new(|cx| {
                SourceChanges::new(
                    "fixture".into(),
                    "sample.txt".into(),
                    &editing,
                    docs.clone(),
                    "text".into(),
                    cx,
                )
            }));
            Root::new(cx.new(|_| Empty), window, cx)
        });
        let markers = markers.unwrap();
        let snapshot = Snapshot {
            original: Arc::from("a\nold\nz\n"),
            current: Arc::from("a\nnew\nz\nextra\n"),
            blocks: vec![],
        };
        let block = Block {
            original: 1..2,
            current: 1..2,
        };
        cx.update(|window, cx| {
            markers.update(cx, |s, cx| {
                s.token.cancel();
                s._watching = Task::ready(());
                s.apply_rollback(&snapshot, block.clone(), window, cx);
                assert_eq!(s.editor.read(cx).text().to_string(), "a\nold\nz\nextra\n");
                s.editor
                    .update(cx, |e, cx| e.step_history(true, window, cx));
                assert_eq!(
                    s.editor.read(cx).text().to_string(),
                    snapshot.current.as_ref()
                );
                s.documents.set_locked(true, cx);
                s.apply_rollback(&snapshot, block.clone(), window, cx);
                assert_eq!(
                    s.editor.read(cx).text().to_string(),
                    snapshot.current.as_ref()
                );
                s.documents.set_locked(false, cx);
                s.editor
                    .update(cx, |e, cx| e.set_value("external draft", window, cx));
                s.apply_rollback(&snapshot, block.clone(), window, cx);
                assert_eq!(s.editor.read(cx).text().to_string(), "external draft");
            })
        });
    }
}
