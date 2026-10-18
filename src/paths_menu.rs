use plico::i18n::t;
use std::path::PathBuf;
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{Icon, Disableable as _, WindowExt as _, dialog::{DialogAction, DialogClose, DialogFooter}, Sizable as _, Selectable as _, button::{Button, ButtonVariants as _},
    h_flex, v_flex, input::{Enter, Escape, Textarea, TextareaState}, popover::Popover};

pub struct Changed;
actions!(paths_filter_menu, [PreviousPathItem, NextPathItem, ChoosePathItem]);
pub struct PathsMenu {
    root: PathBuf,
    paths: Vec<String>,
    recent: Vec<Vec<String>>,
    editor: Option<Entity<TextareaState>>,
    open: bool,
    popup_bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    error: Option<String>,
    focus: FocusHandle,
    menu_index: usize,
}
impl EventEmitter<Changed> for PathsMenu {}
impl PathsMenu {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("up", PreviousPathItem, Some("PathsFilterMenu")),
            KeyBinding::new("down", NextPathItem, Some("PathsFilterMenu")),
            KeyBinding::new("enter", ChoosePathItem, Some("PathsFilterMenu")),
        ]);
        Self { root, paths: vec![], recent: vec![], editor: None, open: false, popup_bounds: Default::default(), error: None,
            focus: cx.focus_handle(), menu_index: 0 }
    }
    pub fn contains_popup_position(&self, position: &Point<Pixels>) -> bool {
        self.open && self.popup_bounds.get().is_some_and(|b| b.contains(position))
    }
    pub fn paths(&self) -> Vec<String> { self.paths.clone() }
    pub fn reset(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.root = root; self.paths.clear(); self.recent.clear(); self.dismiss(cx);
    }
    pub fn clear_filter(&mut self, cx: &mut Context<Self>) {
        self.paths.clear(); self.dismiss(cx);
    }
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.open = false; self.editor = None; self.error = None; cx.notify();
    }
    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut paths = self.paths.clone(); paths.sort();
        let editor = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx);
            input.set_value(paths.join("\n"), window, cx); input
        });
        editor.read(cx).focus_handle(cx).focus(window, cx);
        self.editor = Some(editor); self.error = None; cx.notify();
    }
    fn select_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss(cx);
        let draft = cx.new(|cx| crate::path_chooser::PathChooser::new(
            plico::git::Repository::new(self.root.clone()), self.paths.clone(), window, cx));
        cx.observe_in(&draft, window, |_, _, window, _| window.refresh()).detach();
        let owner = cx.entity().downgrade();
        let focus_draft = draft.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let confirm = draft.clone(); let owner = owner.clone();
            dialog.title(t("Select Paths to Filter by")).w(px(432.)).overlay_closable(false)
                .margin_top(((window.viewport_size().height - px(410.)) / 2.).max(px(16.)))
                .child(draft.clone())
                .footer(DialogFooter::new()
                    .child(div().w(px(80.)).child(DialogClose::new().trigger(|button| button.label(t("Cancel")))))
                    .child(div().w(px(80.)).child(DialogAction::new().child(Button::new("paths-ok").primary().label(t("OK")).disabled(!draft.read(cx).can_confirm())))))
                .on_ok(move |_, _, cx| {
                    let Some(paths) = confirm.read(cx).selected() else { return false; };
                    let _ = owner.update(cx, |s, cx| s.commit(paths, cx)); true
                })
        });
        focus_draft.update(cx, |s, cx| s.focus(window, cx));
    }
    fn commit(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        if !paths.is_empty() {
            self.recent.retain(|old| old != &paths);
            self.recent.insert(0, paths.clone()); self.recent.truncate(10);
        }
        self.paths = paths; self.dismiss(cx); cx.emit(Changed);
    }
    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return; };
        match plico::path_filter::parse(&editor.read(cx).value(), &self.root) {
            Ok(paths) => self.commit(paths, cx),
            Err(error) => { self.error = Some(error); cx.notify(); }
        }
    }
    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_index == 0 { self.edit(window, cx); }
        else if self.menu_index == 1 { self.select_tree(window, cx); }
        else if let Some(paths) = self.recent.get(self.menu_index - 2).cloned() { self.commit(paths, cx); }
    }
    fn caption(paths: &[String]) -> String {
        let Some(first) = paths.iter().min() else { return t("Paths").into(); };
        let label = if first.chars().count() > 30 { format!("{}…", first.chars().take(29).collect::<String>()) } else { first.clone() };
        if paths.len() > 1 { format!("{label} + {}", paths.len() - 1) } else { label }
    }
    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        let bounds = self.popup_bounds.clone();
        let mut body = v_flex().relative()
            .child(canvas(move |rect,_,_| bounds.set(Some(rect)), |_,_,_,_| {}).absolute().size_full())
            .bg(rgb(0x26282c)).border_1().border_color(rgb(0x393b40))
            .rounded(px(8.)).overflow_hidden().text_size(px(13.)).text_color(rgb(0xd1d3d9));
        if let Some(editor) = &self.editor {
            body = body.w(px(430.))
                .child(div().bg(rgb(0x191a1c)).p(px(2.)).child(Textarea::new(editor).h(px(52.)).appearance(false)))
                .child(div().border_t_1().border_color(rgb(0x393b40)).px_2().py_2().text_size(px(11.)).text_color(rgb(0x9fa2a8))
                    .child(t("Select one or more values separated with new lines, use Ctrl+Enter to finish")));
        } else {
            body = body.w(px(200.)).p_1().key_context("PathsFilterMenu").track_focus(&self.focus)
                .on_action(cx.listener(|s, _: &PreviousPathItem, _, cx| { s.menu_index = s.menu_index.saturating_sub(1); cx.notify(); }))
                .on_action(cx.listener(|s, _: &NextPathItem, _, cx| { s.menu_index = (s.menu_index + 1).min(s.recent.len() + 1); cx.notify(); }))
                .on_action(cx.listener(|s, _: &ChoosePathItem, w, cx| s.choose(w, cx)))
                .child(Button::new("paths-select").ghost().small().h(px(26.)).w_full().child(div().w_full().child(t("Select…")))
                    .when(self.menu_index == 0, |b| b.bg(rgb(0x2e4d89)))
                    .on_click(cx.listener(|s, _, w, cx| s.edit(w, cx))));
            body = body.child(Button::new("paths-tree").ghost().small().h(px(26.)).w_full().child(div().w_full().child(t("Select in Tree…")))
                .when(self.menu_index == 1, |b| b.bg(rgb(0x2e4d89)))
                .on_click(cx.listener(|s, _, w, cx| s.select_tree(w, cx))));
            if !self.recent.is_empty() {
                body = body.child(div().px_2().text_size(px(11.)).text_color(rgb(0x9fa2a8)).child(t("Recent")));
                for (i, paths) in self.recent.iter().enumerate() {
                    let paths = paths.clone();
                    body = body.child(Button::new(("recent-paths", i)).ghost().small().h(px(26.)).w_full()
                        .child(div().w_full().overflow_hidden().child(Self::caption(&paths)))
                        .when(self.menu_index == i + 2, |b| b.bg(rgb(0x2e4d89)))
                        .on_click(cx.listener(move |s, _, _, cx| s.commit(paths.clone(), cx))));
                }
            }
        }
        if let Some(error) = &self.error { body = body.child(div().p_2().text_color(rgb(0xf75464)).child(error.clone())); }
        body.capture_action(cx.listener(|s, action: &Enter, _, cx| {
            if action.secondary { s.confirm(cx); cx.stop_propagation(); } else { cx.propagate(); }
        })).capture_action(cx.listener(|s, _: &Escape, _, cx| { s.dismiss(cx); cx.stop_propagation(); }))
        .capture_key_down(cx.listener(|s, event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "enter" && event.keystroke.modifiers.control { s.confirm(cx); cx.stop_propagation(); }
            else if event.keystroke.key == "escape" { s.dismiss(cx); cx.stop_propagation(); }
        })).into_any_element()
    }
}
impl Render for PathsMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.editor.as_ref().map(|e| e.read(cx).focus_handle(cx)).unwrap_or(self.focus.clone());
        let owner = cx.entity().downgrade();
        h_flex().gap_0().child(Popover::new("paths-filter-popup").appearance(false).open(self.open).track_focus(&focus)
            .trigger(Button::new("paths-filter").ghost().small().selected(!self.paths.is_empty())
                .child(h_flex().gap_1().child(Self::caption(&self.paths)).child(Icon::default().path("jb/chevronDown_dark.svg").size(px(16.)))))
            .on_open_change(cx.listener(|s, open, w, cx| {
                if *open { s.open = true; s.menu_index = 0; s.editor = None; s.error = None; s.focus.focus(w, cx); cx.notify(); }
                else { s.dismiss(cx); }
            }))
            .content(move |_, _, cx| owner.update(cx, |s, cx| s.body(cx)).unwrap_or_else(|_| div().into_any_element())))
            .when(!self.paths.is_empty(), |v| v.child(Button::new("clear-paths-filter").ghost().small()
                .icon(Icon::default().path("jb/closeSmall.svg").size(px(16.))).tooltip(t("Clear paths filter"))
                .on_click(cx.listener(|s, _, _, cx| { s.clear_filter(cx); cx.emit(Changed); }))))
    }
}
