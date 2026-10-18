//! CompareWithLocalDialog / ChangesBrowserToolWindow (Rebased 4aec352,
//! JetBrains, Apache-2.0): fixed revisions, one replacement Changes tool window.
use plico::{git::{Repository, Cancel}, diff_target::{Target, Chain}, file_tree, i18n::t};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _, Disableable as _, button::{Button, ButtonVariants as _}};
use std::{path::PathBuf, collections::HashSet};
actions!(version_compare, [ShowDiff, Next, Previous]);
pub struct OpenDiff(pub Target, pub Chain);
pub struct SelectionChanged(pub Target, pub Chain);
pub struct Reloaded(pub Option<(Target, Chain)>);
pub struct Close;
pub struct CompareView {
    repo: Repository, before: String, after: String, chain: Option<Chain>,
    rows: Vec<file_tree::Row>, collapsed: HashSet<PathBuf>, selected: Option<usize>,
    focus: FocusHandle, token: Cancel, task: Task<()>, generation: u64, loading: bool, error: Option<String>,
}
impl EventEmitter<OpenDiff> for CompareView {}
impl EventEmitter<SelectionChanged> for CompareView {}
impl EventEmitter<Reloaded> for CompareView {}
impl EventEmitter<Close> for CompareView {}
impl Drop for CompareView { fn drop(&mut self) { self.token.cancel(); } }
fn icon(name: &'static str) -> Icon { Icon::default().path(format!("jb/{name}")).size(px(16.)) }
impl CompareView {
    pub fn new(repo: Repository, before: String, after: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new("enter", ShowDiff, Some("VersionCompare")),
            KeyBinding::new("ctrl-d", ShowDiff, Some("VersionCompare")),
            KeyBinding::new("down", Next, Some("VersionCompare")), KeyBinding::new("up", Previous, Some("VersionCompare"))]);
        let mut s = Self { repo, before, after, chain: None, rows: Vec::new(), collapsed: HashSet::new(), selected: None,
            focus: cx.focus_handle(), token: Cancel::default(), task: Task::ready(()), generation: 0, loading: false, error: None };
        s.focus.focus(window, cx); s.refresh(cx); s
    }
    pub fn focus_files(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }
    pub fn select_target(&mut self, target: &Target, cx: &mut Context<Self>) {
        let Some(chain) = self.chain.as_ref().and_then(|c| c.fork_for(target)) else { return; };
        self.selected = Some(chain.index());
        for parent in target.change().path.ancestors().skip(1) { self.collapsed.remove(parent); }
        self.chain = Some(chain); self.rebuild(); cx.notify();
    }
    fn selection_changed(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        if let Some(chain) = &mut self.chain {
            if let Some(target) = self.selected.and_then(|i| chain.select(i)) {
                cx.emit(SelectionChanged(target, chain.clone()));
            }
        }
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.token.cancel(); self.token = Cancel::default(); self.generation += 1;
        self.loading = true; self.error = None;
        let preferred = self.chain.as_ref().and_then(|c| self.selected.and_then(|i| c.changes().get(i))).map(|c| (c.path.clone(), c.old_path.clone()));
        let repo = self.repo.clone(); let before = self.before.clone(); let after = self.after.clone();
        let token = self.token.clone(); let generation = self.generation;
        self.task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                let changes = repo.revision_changes(&before, &after, &[], &token)?;
                if changes.is_empty() { Ok(None) } else { Chain::combined(repo, changes, 0).map(|(_, chain)| Some(chain)) }
            }).await;
            let _ = this.update(cx, |s, cx| {
                if s.generation != generation { return; }
                s.loading = false;
                match result {
                    Ok(chain) => {
                        s.chain = chain;
                        s.selected = s.chain.as_ref().map(|c| preferred.as_ref().and_then(|p|
                            c.changes().iter().position(|f| (&f.path, &f.old_path) == (&p.0, &p.1))).unwrap_or(0));
                        s.rebuild();
                        if preferred.is_none() {
                            s.selected = s.rows.iter().find_map(|r| match r { file_tree::Row::File { index, .. } => Some(*index), _ => None });
                        }
                    }
                    Err(error) => { s.error = Some(error); s.chain = None; s.selected = None; s.rows.clear(); }
                }
                let preview = s.chain.as_mut().and_then(|c| s.selected.and_then(|i| c.select(i)).map(|target| (target, c.clone())));
                cx.emit(Reloaded(preview));
                cx.notify();
            });
        }); cx.notify();
    }
    fn rebuild(&mut self) {
        self.rows = self.chain.as_ref().map(|c| file_tree::rows(c.changes().iter().enumerate().map(|(i, c)| (i, c.path.as_path())),
            |p| self.collapsed.contains(p))).unwrap_or_default();
    }
    fn show_diff(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        if let Some(chain) = &mut self.chain {
            if let Some(target) = chain.select(self.selected.unwrap_or(0)) { cx.emit(OpenDiff(target, chain.clone())); }
        }
    }
    fn step(&mut self, direction: isize, cx: &mut Context<Self>) {
        let files = self.rows.iter().filter_map(|r| if let file_tree::Row::File { index, .. } = r { Some(*index) } else { None }).collect::<Vec<_>>();
        if files.is_empty() { return; }
        let i = files.iter().position(|i| Some(*i) == self.selected).unwrap_or(0);
        self.selected = Some(files[i.saturating_add_signed(direction).min(files.len()-1)]); self.selection_changed(cx); cx.notify();
    }
}
impl Render for CompareView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full().min_w_0().rounded(px(10.)).overflow_hidden().bg(rgb(0x181a1b))
            .child(h_flex().h(px(36.)).px_2().gap_1().child(div().flex_1().min_w_0().truncate()
                .child(format!("{} {} → {}", t("Changes"), &self.before[..8], &self.after[..8])))
                .child(Button::new("compare-close").ghost().small().icon(icon("closeSmall.svg")).tooltip(t("Close"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))))
            .child(h_flex().h(px(32.)).px_2().gap_1()
                .child(Button::new("compare-refresh").ghost().small().icon(icon("refresh_dark.svg")).tooltip(t("Refresh"))
                    .on_click(cx.listener(|s, _, _, cx| s.refresh(cx))))
                .child(Button::new("compare-show-diff").ghost().small().label(t("Show Diff")).disabled(self.loading || self.chain.is_none())
                    .on_click(cx.listener(|s, _, _, cx| s.show_diff(cx)))))
            .child(div().flex_1().min_h_0().track_focus(&self.focus).key_context("VersionCompare")
                .on_action(cx.listener(|s, _: &ShowDiff, _, cx| s.show_diff(cx)))
                .on_action(cx.listener(|s, _: &Next, _, cx| s.step(1, cx)))
                .on_action(cx.listener(|s, _: &Previous, _, cx| s.step(-1, cx)))
                .when(self.loading, |v| v.child(div().p_3().child(t("Loading…"))))
                .when_some(self.error.clone(), |v, error| v.child(crate::error_ui::notice(
                    "compare-error", "Could not load comparison. Refresh to retry.", error, self.loading,
                    cx.listener(|s, _, _, cx| s.refresh(cx)))))
                .when(!self.loading && self.error.is_none() && self.chain.is_none(), |v| v.child(div().p_3().child(t("No differences"))))
                .when(!self.loading && self.chain.is_some(), |v| v.child(uniform_list("compare-files", self.rows.len(), cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                    range.map(|row| {
                        let entry = s.rows[row].clone();
                        let (depth, name, image, selected) = match &entry {
                            file_tree::Row::Folder { path, depth, expanded, .. } => (*depth, path.file_name().unwrap_or_default().to_string_lossy().into_owned(), if *expanded { "chevronDown_dark.svg" } else { "chevronRight_dark.svg" }, false),
                            file_tree::Row::File { index, depth } => (*depth, s.chain.as_ref().unwrap().changes()[*index].path.file_name().unwrap_or_default().to_string_lossy().into_owned(), "file-text_dark_4x.svg", s.selected == Some(*index)),
                        };
                        h_flex().id(("compare-file", row)).w_full().h(px(28.)).pl(px(10. + depth as f32 * 16.)).gap_2().min_w_0()
                            .when(selected, |v| v.bg(rgb(0x2e4773))).child(match &entry { file_tree::Row::File { index, .. } => img(crate::ui::file_icon(&s.chain.as_ref().unwrap().changes()[*index].path)).size(px(16.)).flex_shrink_0().into_any_element(), _ => icon(image).into_any_element() }).child(div().min_w_0().truncate().child(name))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |s, event: &MouseDownEvent, window, cx| {
                                s.focus.focus(window, cx);
                                match &entry {
                                    file_tree::Row::Folder { path, .. } => { if !s.collapsed.remove(path) { s.collapsed.insert(path.clone()); } s.rebuild(); }
                                    file_tree::Row::File { index, .. } => { s.selected = Some(*index); if event.click_count == 2 { s.show_diff(cx); } else { s.selection_changed(cx); } }
                                }
                                cx.notify();
                            })).into_any_element()
                    }).collect()
                })).size_full())))
    }
}
