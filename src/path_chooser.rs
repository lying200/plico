use std::collections::BTreeMap;
use plico::{git::{Cancel, Repository}, path_tree::{self, Node, Selection, State}};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{Disableable as _, Icon, checkbox::Checkbox, h_flex, v_flex};

actions!(path_chooser, [PreviousPath, NextPath, TogglePath, ExpandPath, CollapsePath]);
pub struct PathChooser {
    repo: Repository,
    selection: Selection,
    directories: BTreeMap<String, Vec<Node>>,
    rows: Vec<(Node, usize)>,
    active: usize,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    loading: Option<String>,
    error: Option<String>,
    initializing: bool,
    generation: u64,
    token: Cancel,
    task: Task<()>,
}
impl Drop for PathChooser { fn drop(&mut self) { self.token.cancel(); } }
impl PathChooser {
    pub fn new(repo: Repository, initial: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("up", PreviousPath, Some("PathChooser")),
            KeyBinding::new("down", NextPath, Some("PathChooser")),
            KeyBinding::new("space", TogglePath, Some("PathChooser")),
            KeyBinding::new("right", ExpandPath, Some("PathChooser")),
            KeyBinding::new("left", CollapsePath, Some("PathChooser")),
        ]);
        let focus = cx.focus_handle(); focus.focus(window, cx);
        let mut this = Self { repo, selection: Selection::default(), directories: BTreeMap::new(),
            rows: vec![(Node { path: ".".into(), directory: true }, 0)], active: 0, focus,
            scroll: UniformListScrollHandle::new(), loading: None, error: None,
            initializing: true, generation: 0, token: Cancel::default(), task: Task::ready(()) };
        this.expand(".".into(), Some(initial), cx); this
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) { self.focus.focus(window, cx); }
    pub fn can_confirm(&self) -> bool { self.loading.is_none() && !self.selection.is_empty() && self.error.is_none() }
    pub fn selected(&self) -> Option<Vec<String>> { self.can_confirm().then(|| self.selection.paths()) }
    fn rebuild(&mut self) {
        fn walk(path: &str, depth: usize, dirs: &BTreeMap<String, Vec<Node>>, rows: &mut Vec<(Node, usize)>) {
            if let Some(children) = dirs.get(path) {
                for node in children {
                    rows.push((node.clone(), depth));
                    if node.directory { walk(&node.path, depth + 1, dirs, rows); }
                }
            }
        }
        let active = self.rows.get(self.active).map(|(n, _)| n.path.clone());
        self.rows = vec![(Node { path: ".".into(), directory: true }, 0)];
        walk(".", 1, &self.directories, &mut self.rows);
        self.active = active.and_then(|p| self.rows.iter().position(|(n, _)| n.path == p)).unwrap_or(0);
    }
    fn expand(&mut self, path: String, initial: Option<Vec<String>>, cx: &mut Context<Self>) {
        if self.initializing && initial.is_none() { return; }
        if self.directories.contains_key(&path) { return; }
        self.token.cancel(); self.generation += 1; self.token = Cancel::default();
        self.loading = Some(path.clone()); self.error = None;
        let generation = self.generation; let token = self.token.clone(); let repo = self.repo.clone();
        self.task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                let nodes = path_tree::children(&repo, &path, &token)?;
                let selected = if let Some(initial) = initial {
                    let mut selection = Selection::default();
                    for value in initial {
                        if token.cancelled() { return Err("Cancelled".to_owned()); }
                        if std::fs::symlink_metadata(repo.path.join(&value)).is_ok() { selection.toggle(&value)?; }
                    }
                    Some(selection)
                } else { None };
                Ok::<_, String>((path, nodes, selected))
            }).await;
            let _ = this.update(cx, |s, cx| {
                if s.generation != generation { return; }
                s.loading = None; s.initializing = false;
                match result {
                    Ok((path, nodes, selected)) => {
                        if s.directories.values().map(Vec::len).sum::<usize>() + nodes.len() > 16_384 {
                            s.error = Some("Collapse folders before expanding more than 16,384 nodes".into());
                        } else {
                            if let Some(selection) = selected { s.selection = selection; }
                            s.directories.insert(path, nodes); s.rebuild();
                        }
                    }
                    Err(error) => s.error = Some(error),
                }
                cx.notify();
            });
        }); cx.notify();
    }
    fn collapse(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.initializing { return; }
        if self.loading.as_deref().is_some_and(|p| p == path || p.starts_with(&format!("{path}/")) || path == ".") {
            self.token.cancel(); self.generation += 1; self.task = Task::ready(()); self.loading = None;
        }
        self.directories.retain(|p, _| path != "." && p != path && !p.starts_with(&format!("{path}/")));
        self.error = None; self.rebuild(); cx.notify();
    }
    fn toggle(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.initializing { return; }
        self.error = self.selection.toggle(path).err(); cx.notify();
    }
    fn move_row(&mut self, down: bool, cx: &mut Context<Self>) {
        self.active = if down { (self.active + 1).min(self.rows.len().saturating_sub(1)) } else { self.active.saturating_sub(1) };
        self.scroll.scroll_to_item(self.active, ScrollStrategy::Top); cx.notify();
    }
}
impl Render for PathChooser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().w_full().h(px(300.)).gap_1().key_context("PathChooser").track_focus(&self.focus)
            .on_action(cx.listener(|s, _: &PreviousPath, _, cx| s.move_row(false, cx)))
            .on_action(cx.listener(|s, _: &NextPath, _, cx| s.move_row(true, cx)))
            .on_action(cx.listener(|s, _: &TogglePath, _, cx| {
                if let Some((node, _)) = s.rows.get(s.active).cloned() { s.toggle(&node.path, cx); }
            }))
            .on_action(cx.listener(|s, _: &ExpandPath, _, cx| {
                if let Some((node, _)) = s.rows.get(s.active).cloned().filter(|(n, _)| n.directory) { s.expand(node.path, None, cx); }
            }))
            .on_action(cx.listener(|s, _: &CollapsePath, _, cx| {
                if let Some((node, _)) = s.rows.get(s.active).cloned() { s.collapse(&node.path, cx); }
            }))
            .child(uniform_list("path-chooser-tree", self.rows.len(), cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                range.map(|i| {
                    let (node, depth) = s.rows[i].clone(); let state = s.selection.state(&node.path);
                    let expanded = s.directories.contains_key(&node.path);
                    let checkbox_path = node.path.clone(); let expand_path = node.path.clone();
                    let name = if node.path == "." { s.repo.path.file_name().unwrap_or_default().to_string_lossy().into_owned() }
                        else { node.path.rsplit('/').next().unwrap_or(&node.path).to_owned() };
                    h_flex().id(("path-row", i)).h(px(26.)).w_full().pl(px(depth as f32 * 16.)).gap_1()
                        .when(i == s.active, |row| row.bg(rgb(0x2e4d89)))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |s, _, w, cx| { s.active = i; s.focus.focus(w, cx); cx.notify(); }))
                        .child(div().id(("path-expand", i)).w(px(16.)).when(node.directory, |d| d.cursor_pointer()
                            .child(Icon::default().path(if expanded { "jb/chevronDown_dark.svg" } else { "jb/chevronRight_dark.svg" }).size(px(16.)))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                cx.stop_propagation(); if expanded { s.collapse(&expand_path, cx); } else { s.expand(expand_path.clone(), None, cx); }
                            }))))
                        .child(Checkbox::new(("path-check", i)).checked(state != State::Clear)
                            .disabled(matches!(state, State::CoveredByAncestor | State::ContainsSelected))
                            .on_click(cx.listener(move |s, _, _, cx| s.toggle(&checkbox_path, cx))))
                        .child(img(if node.directory { "jb/folder_dark_4x.svg" } else { crate::ui::file_icon(std::path::Path::new(&node.path)) }).size(px(16.)))
                        .child(div().overflow_hidden().child(name))
                }).collect::<Vec<_>>()
            })).flex_1().min_h_0().track_scroll(&self.scroll))
            .when_some(self.loading.clone(), |view, path| view.child(div().text_sm().child(format!("Loading {path}…"))))
            .when_some(self.error.clone(), |view, error| view.child(div().text_sm().text_color(rgb(0xf75464)).child(error)))
    }
}
