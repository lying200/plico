mod persistence;
mod roots;
use gpui_kit::component::{
    Disableable as _, Icon, Selectable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    input::{Enter, Escape, Textarea, TextareaState},
    popover::Popover,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::i18n::t;
use std::path::PathBuf;

pub struct Changed;
actions!(
    paths_filter_menu,
    [PreviousPathItem, NextPathItem, ChoosePathItem]
);
pub struct PathsMenu {
    root: PathBuf,
    roots: roots::Roots,
    paths: Vec<PathBuf>,
    recent: Vec<Vec<PathBuf>>,
    editor: Option<Entity<TextareaState>>,
    open: bool,
    popup_bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    error: Option<String>,
    focus: FocusHandle,
    menu_index: usize,
    persistent: bool,
    store: Option<plico::path_filter::store::Store>,
    restore_task: Task<()>,
    restore_version: u64,
    pending_restore: Option<plico::path_filter::store::State>,
    directory_paths: std::collections::BTreeSet<PathBuf>,
    persistence_error: Option<String>,
}
impl EventEmitter<Changed> for PathsMenu {}
impl PathsMenu {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("up", PreviousPathItem, Some("PathsFilterMenu")),
            KeyBinding::new("down", NextPathItem, Some("PathsFilterMenu")),
            KeyBinding::new("enter", ChoosePathItem, Some("PathsFilterMenu")),
        ]);
        Self {
            root,
            roots: Default::default(),
            paths: vec![],
            recent: vec![],
            editor: None,
            open: false,
            popup_bounds: Default::default(),
            error: None,
            focus: cx.focus_handle(),
            menu_index: 0,
            persistent: false,
            store: None,
            restore_task: Task::ready(()),
            restore_version: 0,
            pending_restore: None,
            directory_paths: Default::default(),
            persistence_error: None,
        }
    }
    pub fn contains_popup_position(&self, position: &Point<Pixels>) -> bool {
        self.open
            && self
                .popup_bounds
                .get()
                .is_some_and(|b| b.contains(position))
    }
    pub fn paths(&self) -> Vec<String> {
        self.paths
            .iter()
            .filter_map(|path| path.strip_prefix(&self.root).ok())
            .map(|path| {
                if path.as_os_str().is_empty() {
                    ".".into()
                } else {
                    path.to_string_lossy().replace('\\', "/")
                }
            })
            .collect()
    }
    pub fn absolute_paths(&self) -> Vec<PathBuf> {
        self.paths.clone()
    }
    fn repositories(&self) -> Vec<PathBuf> {
        if self.roots.entries.is_empty() {
            vec![self.root.clone()]
        } else {
            let mut roots = self
                .roots
                .entries
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>();
            if let Some(index) = roots.iter().position(|path| path == &self.root) {
                let primary = roots.remove(index);
                roots.insert(0, primary);
            }
            roots
        }
    }
    fn displayed(&self, paths: &[PathBuf]) -> Vec<String> {
        let roots = self.repositories();
        paths
            .iter()
            .map(|path| plico::path_filter::project::display(path, &roots))
            .collect()
    }
    pub fn reset(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.root = root;
        self.roots = Default::default();
        self.paths.clear();
        self.recent.clear();
        self.directory_paths.clear();
        self.dismiss(cx);
        if self.persistent {
            self.enable_persistence(cx);
        }
    }
    pub fn clear_filter(&mut self, cx: &mut Context<Self>) {
        self.roots.selected = None;
        self.paths.clear();
        self.directory_paths.clear();
        self.dismiss(cx);
        self.persist(cx);
    }
    pub fn set_roots(&mut self, entries: Vec<(PathBuf, u32)>, cx: &mut Context<Self>) {
        self.roots.replace(entries);
        self.apply_restored(cx);
        cx.notify();
    }
    pub fn selected_roots(&self) -> Option<std::collections::BTreeSet<PathBuf>> {
        self.roots.selected.clone()
    }
    pub fn has_filter(&self) -> bool {
        !self.paths.is_empty() || self.roots.selected.is_some()
    }
    pub fn choose_root(&mut self, path: PathBuf, only: bool, cx: &mut Context<Self>) {
        self.roots.choose(path, only || !self.paths.is_empty());
        self.paths.clear();
        self.directory_paths.clear();
        self.persist(cx);
        cx.emit(Changed);
        cx.notify();
    }
    fn root_count(&self) -> usize {
        if self.roots.entries.len() > 1 {
            self.roots.entries.len()
        } else {
            0
        }
    }
    fn root_start(&self) -> usize {
        if self.root_count() > 15 {
            2 + self.recent.len()
        } else {
            2
        }
    }
    fn recent_start(&self) -> usize {
        if self.root_count() > 15 {
            2
        } else {
            2 + self.root_count()
        }
    }
    fn filter_caption(&self) -> String {
        if !self.paths.is_empty() {
            return Self::caption(&self.displayed(&self.paths));
        }
        match &self.roots.selected {
            None => t("Paths").into(),
            Some(roots) if roots.is_empty() => format!("{}: {}", t("Paths"), t("No roots")),
            Some(roots)
                if self.roots.entries.iter().all(|(p, _)| roots.contains(p))
                    && roots.len() == self.roots.entries.len() =>
            {
                t("Paths").into()
            }
            Some(roots) => format!(
                "{}: {}",
                t("Paths"),
                Self::caption(
                    &roots
                        .iter()
                        .map(|p| {
                            p.file_name()
                                .unwrap_or(p.as_os_str())
                                .to_string_lossy()
                                .into_owned()
                        })
                        .collect::<Vec<_>>(),
                )
            ),
        }
    }
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.editor = None;
        self.error = None;
        cx.notify();
    }
    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut paths = self.displayed(&self.paths);
        paths.sort();
        let editor = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx);
            input.set_value(paths.join("\n"), window, cx);
            input
        });
        editor.read(cx).focus_handle(cx).focus(window, cx);
        self.editor = Some(editor);
        self.error = None;
        cx.notify();
    }
    fn select_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss(cx);
        let draft = cx.new(|cx| {
            crate::path_chooser::PathChooser::new(
                self.repositories()
                    .into_iter()
                    .map(plico::git::Repository::new)
                    .collect(),
                self.paths.clone(),
                window,
                cx,
            )
        });
        cx.observe_in(&draft, window, |_, _, window, _| window.refresh())
            .detach();
        let owner = cx.entity().downgrade();
        let focus_draft = draft.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let confirm = draft.clone();
            let owner = owner.clone();
            dialog
                .title(t("Select Paths to Filter by"))
                .w(px(432.))
                .overlay_closable(false)
                .margin_top(((window.viewport_size().height - px(410.)) / 2.).max(px(16.)))
                .child(draft.clone())
                .footer(
                    DialogFooter::new()
                        .child(
                            div().w(px(80.)).child(
                                DialogClose::new().trigger(|button| button.label(t("Cancel"))),
                            ),
                        )
                        .child(
                            div().w(px(80.)).child(
                                DialogAction::new().child(
                                    Button::new("paths-ok")
                                        .primary()
                                        .label(t("OK"))
                                        .disabled(!draft.read(cx).can_confirm()),
                                ),
                            ),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    let Some(paths) = confirm.read(cx).selected() else {
                        return false;
                    };
                    let _ = owner.update(cx, |s, cx| s.set_paths(paths, cx));
                    true
                })
        });
        focus_draft.update(cx, |s, cx| s.focus(window, cx));
    }
    pub(crate) fn set_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if !paths.is_empty() {
            self.recent.retain(|old| old != &paths);
            self.recent.insert(0, paths.clone());
            self.recent.truncate(10);
        }
        self.directory_paths.retain(|path| paths.contains(path));
        self.paths = paths;
        self.roots.selected = None;
        self.dismiss(cx);
        self.persist(cx);
        cx.emit(Changed);
    }
    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        match plico::path_filter::project::parse(&editor.read(cx).value(), &self.repositories()) {
            Ok(paths) => self.set_paths(paths, cx),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_index == 0 {
            self.edit(window, cx);
        } else if self.menu_index == 1 {
            self.select_tree(window, cx);
        } else if (self.root_start()..self.root_start() + self.root_count())
            .contains(&self.menu_index)
        {
            let root = self.roots.entries[self.menu_index - self.root_start()]
                .0
                .clone();
            self.choose_root(root, false, cx);
        } else if let Some(paths) = self
            .menu_index
            .checked_sub(self.recent_start())
            .and_then(|i| self.recent.get(i))
            .cloned()
        {
            self.set_paths(paths, cx);
        }
    }
    fn caption(paths: &[String]) -> String {
        let Some(first) = paths.iter().min() else {
            return t("Paths").into();
        };
        let label = if first.chars().count() > 30 {
            format!("{}…", first.chars().take(29).collect::<String>())
        } else {
            first.clone()
        };
        if paths.len() > 1 {
            format!("{label} + {}", paths.len() - 1)
        } else {
            label
        }
    }
    fn root_items(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex();
        if self.root_count() == 0 {
            return body.into_any_element();
        }
        body = body.child(
            div()
                .mt_1()
                .border_t_1()
                .border_color(rgb(crate::ui_theme::color(cx, 0x393b40, 0xdddfe4)))
                .px_2()
                .py_1()
                .text_color(rgb(crate::ui_theme::color(cx, 0x9fa2a8, 0x5f6269)))
                .child(t("Roots")),
        );
        for (i, (path, color)) in self.roots.entries.iter().enumerate() {
            let checked = self.paths.is_empty() && self.roots.includes(path);
            let path = path.clone();
            let name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned();
            body = body.child(
                Button::new(("paths-root", i))
                    .ghost()
                    .small()
                    .h(px(26.))
                    .w_full()
                    .tooltip(path.display().to_string())
                    .when(self.menu_index == self.root_start() + i, |b| {
                        b.bg(rgb(crate::ui_theme::color(cx, 0x2e4d89, 0xd0dffe)))
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .gap_1()
                            .child(div().size(px(16.)).rounded(px(3.)).bg(rgb(*color)).when(
                                checked,
                                |v| {
                                    v.child(
                                        Icon::default().path("jb/checked_dark.svg").size(px(16.)),
                                    )
                                },
                            ))
                            .child(div().min_w_0().truncate().child(name)),
                    )
                    .on_click(cx.listener(move |s, event: &ClickEvent, _, cx| {
                        s.choose_root(
                            path.clone(),
                            event.modifiers().control || event.modifiers().platform,
                            cx,
                        )
                    })),
            );
        }
        body.into_any_element()
    }
    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        let bounds = self.popup_bounds.clone();
        let mut body = v_flex()
            .relative()
            .child(
                canvas(move |rect, _, _| bounds.set(Some(rect)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .bg(rgb(crate::ui_theme::color(cx, 0x26282c, 0xe9eaee)))
            .border_1()
            .border_color(rgb(crate::ui_theme::color(cx, 0x393b40, 0xdddfe4)))
            .rounded(px(8.))
            .overflow_hidden()
            .text_size(rems(1.))
            .text_color(rgb(crate::ui_theme::color(cx, 0xd1d3d9, 0x000000)));
        if let Some(editor) = &self.editor {
            body = body.w(px(430.))
                .child(div().bg(rgb(crate::ui_theme::color(cx, 0x191a1c, 0xffffff))).p(px(2.)).child(Textarea::new(editor).h(px(52.)).appearance(false)))
                .child(div().border_t_1().border_color(rgb(crate::ui_theme::color(cx, 0x393b40, 0xdddfe4))).px_2().py_2().text_size(px(11.)).text_color(rgb(crate::ui_theme::color(cx, 0x9fa2a8, 0x5f6269)))
                    .child(t("Select one or more values separated with new lines, use Ctrl+Enter to finish")));
        } else {
            body = body
                .w(px(200.))
                .p_1()
                .key_context("PathsFilterMenu")
                .track_focus(&self.focus)
                .on_action(cx.listener(|s, _: &PreviousPathItem, _, cx| {
                    s.menu_index = s.menu_index.saturating_sub(1);
                    cx.notify();
                }))
                .on_action(cx.listener(|s, _: &NextPathItem, _, cx| {
                    s.menu_index = (s.menu_index + 1).min(s.recent.len() + s.root_count() + 1);
                    cx.notify();
                }))
                .on_action(cx.listener(|s, _: &ChoosePathItem, w, cx| s.choose(w, cx)))
                .child(
                    Button::new("paths-select")
                        .ghost()
                        .small()
                        .h(px(26.))
                        .w_full()
                        .child(div().w_full().child(t("Select…")))
                        .when(self.menu_index == 0, |b| {
                            b.bg(rgb(crate::ui_theme::color(cx, 0x2e4d89, 0xd0dffe)))
                        })
                        .on_click(cx.listener(|s, _, w, cx| s.edit(w, cx))),
                );
            body = body.child(
                Button::new("paths-tree")
                    .ghost()
                    .small()
                    .h(px(26.))
                    .w_full()
                    .child(div().w_full().child(t("Select in Tree…")))
                    .when(self.menu_index == 1, |b| {
                        b.bg(rgb(crate::ui_theme::color(cx, 0x2e4d89, 0xd0dffe)))
                    })
                    .on_click(cx.listener(|s, _, w, cx| s.select_tree(w, cx))),
            );
            if self.root_count() <= 15 {
                body = body.child(self.root_items(cx));
            }
            if !self.recent.is_empty() {
                body = body.child(
                    div()
                        .px_2()
                        .text_size(px(11.))
                        .text_color(rgb(crate::ui_theme::color(cx, 0x9fa2a8, 0x5f6269)))
                        .child(t("Recent")),
                );
                for (i, paths) in self.recent.iter().enumerate() {
                    let paths = paths.clone();
                    body = body.child(
                        Button::new(("recent-paths", i))
                            .ghost()
                            .small()
                            .h(px(26.))
                            .w_full()
                            .child(
                                div()
                                    .w_full()
                                    .overflow_hidden()
                                    .child(Self::caption(&self.displayed(&paths))),
                            )
                            .when(self.menu_index == i + self.recent_start(), |b| {
                                b.bg(rgb(crate::ui_theme::color(cx, 0x2e4d89, 0xd0dffe)))
                            })
                            .on_click(
                                cx.listener(move |s, _, _, cx| s.set_paths(paths.clone(), cx)),
                            ),
                    );
                }
            }
        }
        if self.editor.is_none() && self.root_count() > 15 {
            body = body.child(self.root_items(cx));
        }
        if let Some(error) = self.error.as_ref().or(self.persistence_error.as_ref()) {
            body = body.child(
                div()
                    .p_2()
                    .text_color(rgb(crate::ui_theme::color(cx, 0xf75464, 0xc54e58)))
                    .child(error.clone()),
            );
        }
        body.capture_action(cx.listener(|s, action: &Enter, _, cx| {
            if action.secondary {
                if s.editor.is_some() {
                    s.confirm(cx);
                } else if (s.root_start()..s.root_start() + s.root_count()).contains(&s.menu_index)
                {
                    let root = s.roots.entries[s.menu_index - s.root_start()].0.clone();
                    s.choose_root(root, true, cx);
                }
                cx.stop_propagation();
            } else {
                cx.propagate();
            }
        }))
        .capture_action(cx.listener(|s, _: &Escape, _, cx| {
            s.dismiss(cx);
            cx.stop_propagation();
        }))
        .capture_key_down(cx.listener(|s, event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "enter" && event.keystroke.modifiers.control {
                if s.editor.is_some() {
                    s.confirm(cx);
                } else if (s.root_start()..s.root_start() + s.root_count()).contains(&s.menu_index)
                {
                    let root = s.roots.entries[s.menu_index - s.root_start()].0.clone();
                    s.choose_root(root, true, cx);
                }
                cx.stop_propagation();
            } else if event.keystroke.key == "escape" {
                s.dismiss(cx);
                cx.stop_propagation();
            }
        }))
        .into_any_element()
    }
}
impl Render for PathsMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self
            .editor
            .as_ref()
            .map(|e| e.read(cx).focus_handle(cx))
            .unwrap_or(self.focus.clone());
        let owner = cx.entity().downgrade();
        h_flex()
            .gap_0()
            .child(
                Popover::new("paths-filter-popup")
                    .appearance(false)
                    .open(self.open)
                    .track_focus(&focus)
                    .trigger(
                        Button::new("paths-filter")
                            .ghost()
                            .small()
                            .selected(self.has_filter())
                            .child(
                                h_flex().gap_1().child(self.filter_caption()).child(
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
                            s.editor = None;
                            s.error = None;
                            s.focus.focus(w, cx);
                            cx.notify();
                        } else {
                            s.dismiss(cx);
                        }
                    }))
                    .content(move |_, _, cx| {
                        owner
                            .update(cx, |s, cx| s.body(cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
            )
            .when(self.has_filter(), |v| {
                v.child(
                    Button::new("clear-paths-filter")
                        .ghost()
                        .small()
                        .icon(Icon::default().path("jb/closeSmall.svg").size(px(16.)))
                        .tooltip(t("Clear paths filter"))
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.clear_filter(cx);
                            cx.emit(Changed);
                        })),
                )
            })
    }
}
