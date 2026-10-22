use gpui_kit::component::{
    Disableable as _, Icon, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{
        Enter, Escape, IndentInline, MoveDown, MoveUp, ShowCompletions, Textarea, TextareaState,
    },
    popover::Popover,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::i18n::t;
use plico::{
    git::{Author, Cancel, Repository},
    user_filter::{Directory, Selection, completion_matches, completion_range},
};
use std::{cell::Cell, rc::Rc};

pub enum Changed {
    Applied,
    Failed(String),
}
actions!(
    user_filter_menu,
    [PreviousUserItem, NextUserItem, ChooseUserItem]
);
pub struct UserMenu {
    repo: Repository,
    selection: Selection,
    resolved: Option<Vec<Author>>,
    directory: Option<Directory>,
    recent: Vec<Selection>,
    editor: Option<Entity<TextareaState>>,
    editor_observer: Option<Subscription>,
    candidates: Vec<String>,
    matches: Vec<usize>,
    completion_index: usize,
    completion_snapshot: Option<(SharedString, usize)>,
    completion_scroll: UniformListScrollHandle,
    completion_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    popup_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    open: bool,
    loading: bool,
    error: Option<String>,
    focus: FocusHandle,
    menu_index: usize,
    generation: u64,
    token: Cancel,
    task: Task<()>,
}
impl EventEmitter<Changed> for UserMenu {}
impl Drop for UserMenu {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl UserMenu {
    pub fn new(repo: Repository, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("up", PreviousUserItem, Some("UserFilterMenu")),
            KeyBinding::new("down", NextUserItem, Some("UserFilterMenu")),
            KeyBinding::new("enter", ChooseUserItem, Some("UserFilterMenu")),
        ]);
        Self {
            repo,
            selection: Selection::default(),
            resolved: None,
            directory: None,
            recent: vec![],
            editor: None,
            editor_observer: None,
            candidates: vec![],
            matches: vec![],
            completion_index: 0,
            completion_snapshot: None,
            completion_scroll: UniformListScrollHandle::new(),
            completion_bounds: Rc::new(Cell::new(None)),
            popup_bounds: Rc::new(Cell::new(None)),
            open: false,
            loading: false,
            error: None,
            focus: cx.focus_handle(),
            menu_index: 0,
            generation: 0,
            token: Cancel::default(),
            task: Task::ready(()),
        }
    }
    pub fn contains_popup_position(&self, position: &Point<Pixels>) -> bool {
        self.open
            && (self
                .popup_bounds
                .get()
                .is_some_and(|b| b.contains(position))
                || self
                    .completion_bounds
                    .get()
                    .is_some_and(|b| b.contains(position)))
    }
    pub fn authors(&self) -> Option<Vec<Author>> {
        self.resolved.clone()
    }
    pub fn active(&self) -> bool {
        !self.selection.is_empty()
    }
    pub fn reset(&mut self, repo: Repository, cx: &mut Context<Self>) {
        self.cancel();
        self.repo = repo;
        self.selection = Selection::default();
        self.resolved = None;
        self.directory = None;
        self.recent.clear();
        self.clear_editor();
        self.open = false;
        self.error = None;
        cx.notify();
    }
    pub fn clear_filter(&mut self, cx: &mut Context<Self>) {
        self.cancel();
        self.selection = Selection::default();
        self.resolved = None;
        self.clear_editor();
        self.open = false;
        self.error = None;
        cx.notify();
    }
    fn cancel(&mut self) {
        self.token.cancel();
        self.generation += 1;
        self.task = Task::ready(());
        self.loading = false;
    }
    /// Refresh invalidates identities/config even when refs have not moved.
    /// An active selection is resolved again before notifying the Log owner.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.cancel();
        self.directory = None;
        self.error = None;
        if self.active() || self.open {
            self.load(cx);
        }
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.directory.is_some() {
            return;
        }
        self.cancel();
        self.loading = true;
        self.token = Cancel::default();
        let generation = self.generation;
        let token = self.token.clone();
        let repo = self.repo.clone();
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { Directory::read(&repo, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.generation != generation {
                    return;
                }
                s.loading = false;
                match result {
                    Ok(directory) => {
                        s.resolved = s.selection.resolve(&directory);
                        s.directory = Some(directory);
                        if s.active() {
                            cx.emit(Changed::Applied);
                        }
                    }
                    Err(error) => {
                        if s.active() {
                            cx.emit(Changed::Failed(error.clone()));
                        }
                        s.error = Some(error);
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.clear_editor();
        // An active-filter refresh must finish even if its popup is dismissed.
        if self.loading && !self.active() {
            self.cancel();
        }
        cx.notify();
    }
    fn clear_editor(&mut self) {
        self.editor_observer = None;
        self.editor = None;
        self.completion_bounds.set(None);
        self.candidates = Vec::new();
        self.matches = Vec::new();
        self.completion_snapshot = None;
    }
    fn update_completions(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        let state = editor.read(cx);
        let snapshot = (state.value(), state.cursor());
        if !force && self.completion_snapshot.as_ref() == Some(&snapshot) {
            return;
        }
        self.matches.clear();
        self.completion_index = 0;
        if snapshot.0.len() <= 64 * 1024 {
            if let Some(range) = completion_range(&snapshot.0, snapshot.1) {
                let prefix = &snapshot.0[range];
                // A separator dismisses auto-completion; explicit Ctrl+Space can show all.
                if force || !prefix.is_empty() {
                    self.matches.extend(
                        self.candidates
                            .iter()
                            .enumerate()
                            .filter(|(_, name)| completion_matches(name, prefix))
                            .map(|(i, _)| i),
                    );
                }
            }
        }
        self.completion_snapshot = Some(snapshot);
        self.completion_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }
    fn move_completion(&mut self, down: bool, cx: &mut Context<Self>) {
        if self.matches.is_empty() {
            cx.propagate();
            return;
        }
        self.completion_index = if down {
            (self.completion_index + 1).min(self.matches.len() - 1)
        } else {
            self.completion_index.saturating_sub(1)
        };
        self.completion_scroll
            .scroll_to_item(self.completion_index, ScrollStrategy::Top);
        cx.notify();
        cx.stop_propagation();
    }
    fn accept_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.update_completions(false, cx);
        let Some(&index) = self.matches.get(self.completion_index) else {
            return false;
        };
        let value = self.candidates[index].clone();
        let Some(editor) = self.editor.clone() else {
            return false;
        };
        let state = editor.read(cx);
        let Some(range) = completion_range(&state.value(), state.cursor()) else {
            return false;
        };
        editor.update(cx, |state, cx| {
            state.set_selected_range(range, cx);
            state.replace_text_in_range(None, &value, window, cx);
        });
        let state = editor.read(cx);
        self.completion_snapshot = Some((state.value(), state.cursor()));
        self.matches.clear();
        cx.notify();
        true
    }
    fn escape(&mut self, cx: &mut Context<Self>) {
        if self.matches.is_empty() {
            self.dismiss(cx);
        } else {
            self.matches.clear();
            cx.notify();
        }
    }
    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.selection.draft();
        let editor = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx);
            input.set_value(value, window, cx);
            input
        });
        editor.read(cx).focus_handle(cx).focus(window, cx);
        self.clear_editor();
        self.candidates = self
            .directory
            .as_ref()
            .map(|d| d.catalog.presentations())
            .unwrap_or_default();
        if self.has_me() && !self.candidates.iter().any(|name| name == "me") {
            self.candidates.push("me".into());
        }
        self.candidates
            .sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        self.editor_observer =
            Some(cx.observe(&editor, |s, _, cx| s.update_completions(false, cx)));
        self.editor = Some(editor);
        self.error = None;
        // Do not open lookup merely by opening the draft.
        let state = self.editor.as_ref().unwrap().read(cx);
        self.completion_snapshot = Some((state.value(), state.cursor()));
        cx.notify();
    }
    fn commit(&mut self, selection: Selection, cx: &mut Context<Self>) {
        let Some(directory) = &self.directory else {
            return;
        };
        self.resolved = selection.resolve(directory);
        if !selection.is_empty() {
            self.recent.retain(|old| old != &selection);
            self.recent.insert(0, selection.clone());
            self.recent.truncate(10);
        }
        self.selection = selection;
        self.error = None;
        self.dismiss(cx);
        cx.emit(Changed::Applied);
    }
    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        match Selection::parse(&editor.read(cx).value()) {
            Ok(selection) => self.commit(selection, cx),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
    fn has_me(&self) -> bool {
        self.directory.as_ref().is_some_and(|d| d.current.is_some())
    }
    fn move_item(&mut self, down: bool, cx: &mut Context<Self>) {
        if self.loading || self.directory.is_none() {
            return;
        }
        let last = usize::from(self.has_me()) + self.recent.len();
        self.menu_index = if down {
            (self.menu_index + 1).min(last)
        } else {
            self.menu_index.saturating_sub(1)
        };
        cx.notify();
    }
    fn choose_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.directory.is_none() {
            return;
        }
        if self.menu_index == 0 {
            self.edit(window, cx);
        } else if self.has_me() && self.menu_index == 1 {
            self.commit(Selection::parse("me").unwrap(), cx);
        } else if let Some(selection) = self
            .recent
            .get(self.menu_index - 1 - usize::from(self.has_me()))
            .cloned()
        {
            self.commit(selection, cx);
        }
    }
    fn body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.completion_bounds.set(None);
        let bounds = self.popup_bounds.clone();
        let mut body = v_flex()
            .relative()
            .child(
                canvas(move |rect, _, _| bounds.set(Some(rect)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .bg(rgb(0x26282c))
            .border_1()
            .border_color(rgb(0x393b40))
            .rounded(px(8.))
            .overflow_hidden()
            .text_size(rems(1.))
            .text_color(rgb(0xd1d3d9));
        if let Some(editor) = &self.editor {
            body = body.w(px(430.)).child(div().bg(rgb(0x191a1c)).p(px(2.)).child(Textarea::new(editor).h(px(52.)).appearance(false)))
                .child(div().border_t_1().border_color(rgb(0x393b40)).px_2().py_2().text_size(px(11.)).text_color(rgb(0x9fa2a8))
                    .child(t("Select one or more values separated with | or new lines, use Ctrl+Enter to finish")));
            if !self.matches.is_empty() {
                let list = uniform_list(
                    "user-completions",
                    self.matches.len(),
                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|i| {
                                let name = s.candidates[s.matches[i]].clone();
                                div()
                                    .id(("user-completion", i))
                                    .h(px(26.))
                                    .px_2()
                                    .w_full()
                                    .overflow_hidden()
                                    .when(i == s.completion_index, |row| row.bg(rgb(0x2e4d89)))
                                    .child(name)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |s, _, w, cx| {
                                            s.completion_index = i;
                                            s.accept_completion(w, cx);
                                            cx.stop_propagation();
                                        }),
                                    )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .h(px(26. * self.matches.len().min(8) as f32))
                .track_scroll(&self.completion_scroll);
                // LookupUi.calculatePosition aligns lookup text with the prefix start.
                // Use actual shaped text geometry, not character-count estimates.
                let input = editor.read(cx);
                let start = completion_range(&input.value(), input.cursor()).map(|r| r.start);
                let bounds = start.and_then(|start| input.range_to_bounds(&(start..start)));
                if let Some(bounds) = bounds {
                    let viewport = window.viewport_size();
                    let layout = plico::lookup_layout::place(
                        [
                            f32::from(bounds.left() - px(13.)),
                            f32::from(bounds.top()),
                            f32::from(bounds.bottom()),
                        ],
                        [200., 26. * self.matches.len().min(8) as f32 + 10.],
                        [f32::from(viewport.width), f32::from(viewport.height)],
                    );
                    let list = list.h(px((layout.height - 10.).max(0.)));
                    body = body.child(
                        deferred(
                            anchored()
                                .position(point(px(layout.x), px(layout.y)))
                                .snap_to_window_with_margin(px(8.))
                                .child(
                                    div()
                                        .w(px(layout.width))
                                        .p_1()
                                        .bg(rgb(0x26282c))
                                        .border_1()
                                        .border_color(rgb(0x393b40))
                                        .rounded(px(8.))
                                        .shadow_lg()
                                        .text_size(rems(1.))
                                        .text_color(rgb(0xd1d3d9))
                                        .child(
                                            canvas(
                                                {
                                                    let bounds = self.completion_bounds.clone();
                                                    move |rect, _, _| bounds.set(Some(rect))
                                                },
                                                |_, _, _, _| {},
                                            )
                                            .absolute()
                                            .size_full(),
                                        )
                                        .child(list),
                                ),
                        )
                        .with_priority(101),
                    ); // Above the parent popup (GPUI popup priority 100).
                }
            }
        } else {
            body = body
                .w(px(200.))
                .p_1()
                .key_context("UserFilterMenu")
                .track_focus(&self.focus)
                .on_action(cx.listener(|s, _: &PreviousUserItem, _, cx| s.move_item(false, cx)))
                .on_action(cx.listener(|s, _: &NextUserItem, _, cx| s.move_item(true, cx)))
                .on_action(cx.listener(|s, _: &ChooseUserItem, w, cx| s.choose_item(w, cx)))
                .child(
                    Button::new("user-select")
                        .ghost()
                        .small()
                        .h(px(26.))
                        .w_full()
                        .child(div().w_full().child(t("Select…")))
                        .when(self.menu_index == 0 && self.directory.is_some(), |b| {
                            b.bg(rgb(0x2e4d89))
                        })
                        .disabled(self.loading || self.directory.is_none())
                        .on_click(cx.listener(|s, _, w, cx| s.edit(w, cx))),
                );
            if self.has_me() {
                body =
                    body.child(
                        Button::new("user-me")
                            .ghost()
                            .small()
                            .h(px(26.))
                            .w_full()
                            .child(div().w_full().child(t("me")))
                            .when(self.menu_index == 1, |b| b.bg(rgb(0x2e4d89)))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.commit(Selection::parse("me").unwrap(), cx)
                            })),
                    );
            }
            if !self.recent.is_empty() {
                body = body.child(
                    div()
                        .px_2()
                        .text_size(px(11.))
                        .text_color(rgb(0x9fa2a8))
                        .child(t("Recent")),
                );
                for (i, selection) in self.recent.iter().enumerate() {
                    let selection = selection.clone();
                    body = body.child(
                        Button::new(("recent-user", i))
                            .ghost()
                            .small()
                            .h(px(26.))
                            .w_full()
                            .child(div().w_full().child(selection.values().join("|")))
                            .when(self.menu_index == i + 1 + usize::from(self.has_me()), |b| {
                                b.bg(rgb(0x2e4d89))
                            })
                            .on_click(
                                cx.listener(move |s, _, _, cx| s.commit(selection.clone(), cx)),
                            ),
                    );
                }
            }
        }
        if self.loading {
            body = body.child(div().p_2().child(t("Loading users…")));
        }
        if let Some(error) = &self.error {
            body = body.child(div().p_2().text_color(rgb(0xf75464)).child(error.clone()));
        }
        body.id("user-filter-body")
            .on_mouse_down_out(cx.listener(|s, event: &MouseDownEvent, _, cx| {
                if !s.matches.is_empty()
                    && !s
                        .completion_bounds
                        .get()
                        .is_some_and(|bounds| bounds.contains(&event.position))
                {
                    s.dismiss(cx);
                }
            }))
            .capture_action(cx.listener(|s, _: &MoveDown, _, cx| s.move_completion(true, cx)))
            .capture_action(cx.listener(|s, _: &MoveUp, _, cx| s.move_completion(false, cx)))
            .capture_action(cx.listener(|s, _: &ShowCompletions, _, cx| {
                s.update_completions(true, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|s, _: &IndentInline, w, cx| {
                if s.accept_completion(w, cx) {
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .capture_action(cx.listener(|s, action: &Enter, w, cx| {
                if action.secondary {
                    s.confirm(cx);
                    cx.stop_propagation();
                } else if s.accept_completion(w, cx) {
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            }))
            .capture_action(cx.listener(|s, _: &Escape, _, cx| {
                s.escape(cx);
                cx.stop_propagation();
            }))
            .capture_key_down(cx.listener(|s, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" && event.keystroke.modifiers.control {
                    s.confirm(cx);
                    cx.stop_propagation();
                } else if event.keystroke.key == "escape" {
                    s.escape(cx);
                    cx.stop_propagation();
                }
            }))
            .into_any_element()
    }
}
impl Render for UserMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self
            .editor
            .as_ref()
            .map(|e| e.read(cx).focus_handle(cx))
            .unwrap_or(self.focus.clone());
        let caption = if self.selection.is_empty() {
            t("User").to_owned()
        } else {
            let value = self.selection.values().join("|");
            let value = if value.chars().count() > 24 {
                format!("{}…", value.chars().take(23).collect::<String>())
            } else {
                value
            };
            format!("{}: {value}", t("User"))
        };
        let owner = cx.entity().downgrade();
        h_flex()
            .gap_0()
            .child(
                Popover::new("user-filter-popup")
                    .appearance(false)
                    .open(self.open)
                    .track_focus(&focus)
                    .overlay_closable(self.matches.is_empty())
                    .trigger(
                        Button::new("user-filter")
                            .ghost()
                            .small()
                            .selected(self.active())
                            .child(
                                h_flex().gap_1().child(caption).child(
                                    Icon::default()
                                        .path("jb/chevronDown_dark.svg")
                                        .size(px(16.)),
                                ),
                            ),
                    )
                    .on_open_change(cx.listener(|s, open, w, cx| {
                        if *open {
                            s.open = true;
                            s.menu_index = 0;
                            s.clear_editor();
                            s.error = None;
                            s.focus.focus(w, cx);
                            s.load(cx);
                            cx.notify();
                        } else {
                            s.dismiss(cx);
                        }
                    }))
                    .content(move |_, window, cx| {
                        owner
                            .update(cx, |s, cx| s.body(window, cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
            )
            .when(self.active(), |v| {
                v.child(
                    Button::new("clear-user-filter")
                        .ghost()
                        .small()
                        .icon(Icon::default().path("jb/closeSmall.svg").size(px(16.)))
                        .tooltip(t("Clear user filter"))
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.clear_filter(cx);
                            cx.emit(Changed::Applied);
                        })),
                )
            })
    }
}
