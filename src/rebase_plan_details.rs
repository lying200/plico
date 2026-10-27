//! FullCommitDetailsListPanel layout: changes above metadata (0.67 split).
//! Each selection owns cancellation and generation; late results cannot repaint it.
use crate::ui_style::{BG, BORDER, FG, MUTED, SELECT};
use gpui_kit::component::{
    Disableable as _, Icon, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    resizable::{ResizableState, resizable_panel, v_resizable},
    text::TextView,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    file_tree::{self, Row},
    git::{Cancel, Repository},
    i18n::{self, t},
    interactive_rebase::details::{self, Selection},
};
use std::{collections::HashSet, path::PathBuf};

pub struct Details {
    repo: Repository,
    project: PathBuf,
    ids: Vec<String>,
    generation: u64,
    token: Cancel,
    task: Task<()>,
    data: Option<Selection>,
    loading: bool,
    error: Option<String>,
    closed: bool,
    selected: Option<usize>,
    files_focus: FocusHandle,
    cursor: Option<navigation::Key>,
    selection: HashSet<navigation::Key>,
    anchor: Option<navigation::Key>,
    scroll: UniformListScrollHandle,
    collapsed: HashSet<PathBuf>,
    rows: Vec<Row>,
    grouped: bool,
    cards: ListState,
    split: Entity<ResizableState>,
    fraction: f32,
}
impl Details {
    pub fn new(repo: Repository, project: PathBuf, cx: &mut Context<Self>) -> Self {
        let grouped = plico::layout_preferences::load_rebase_grouping(&project);
        Self {
            repo,
            project,
            split: cx.new(|_| ResizableState::default()),
            fraction: plico::layout_preferences::RebaseSplit::Details.load(),
            ids: vec![],
            generation: 0,
            token: Cancel::default(),
            task: Task::ready(()),
            data: None,
            loading: false,
            error: None,
            closed: false,
            selected: None,
            files_focus: cx.focus_handle(),
            cursor: None,
            selection: HashSet::new(),
            anchor: None,
            scroll: UniformListScrollHandle::new(),
            collapsed: HashSet::new(),
            rows: vec![],
            grouped,
            cards: ListState::new(0, ListAlignment::Top, px(100.)),
        }
    }
    fn toggle_grouping(&mut self, cx: &mut Context<Self>) {
        self.set_grouped(!self.grouped);
        if let Err(error) =
            plico::layout_preferences::save_rebase_grouping(&self.project, self.grouped)
        {
            self.error = Some(error.to_string());
        }
        cx.notify();
    }
    pub fn select(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        if self.closed || self.ids == ids {
            return;
        }
        self.ids = ids;
        self.reload(cx);
    }
    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        self.token.cancel();
        self.token = Cancel::default();
        self.generation += 1;
        self.data = None;
        self.selected = None;
        self.cursor = None;
        self.selection.clear();
        self.anchor = None;
        self.rows.clear();
        self.cards.reset(0);
        self.error = None;
        self.loading = !self.ids.is_empty();
        if self.loading {
            let repo = self.repo.clone();
            let ids = self.ids.clone();
            let token = self.token.clone();
            let generation = self.generation;
            self.task = cx.spawn(async move |this, cx| {
                let result = cx
                    .background_spawn(async move { details::load(&repo, &ids, &token) })
                    .await;
                let _ = this.update(cx, |s, cx| s.accept(generation, result, cx));
            });
        }
        cx.notify();
    }
    fn accept(
        &mut self,
        generation: u64,
        result: Result<Selection, String>,
        cx: &mut Context<Self>,
    ) {
        if self.closed || generation != self.generation || self.token.cancelled() {
            return;
        }
        self.loading = false;
        match result {
            Ok(data) => {
                self.cards.reset(data.cards.len());
                self.data = Some(data);
                self.rebuild();
            }
            Err(e) => self.error = Some(e),
        }
        cx.notify();
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.token.cancel();
        self.task = Task::ready(());
    }
    fn rebuild(&mut self) {
        let Some(data) = &self.data else {
            return;
        };
        self.rows = if self.grouped {
            file_tree::compact_rows(
                data.files.iter().enumerate().filter_map(|(i, f)| {
                    f.after
                        .as_ref()
                        .or(f.before.as_ref())
                        .map(|r| (i, r.path.as_path()))
                }),
                |p| self.collapsed.contains(p),
            )
        } else {
            (0..data.files.len())
                .map(|index| Row::File { index, depth: 0 })
                .collect()
        };
        if self.grouped && !data.files.is_empty() {
            for row in &mut self.rows {
                match row {
                    Row::Folder { depth, .. } | Row::File { depth, .. } => *depth += 1,
                }
            }
            let expanded = !self.collapsed.contains(&PathBuf::new());
            if !expanded {
                self.rows.clear();
            }
            self.rows.insert(
                0,
                Row::Folder {
                    path: PathBuf::new(),
                    label: PathBuf::new(),
                    depth: 0,
                    count: data.files.len(),
                    expanded,
                },
            );
        }
        self.restore_cursor();
    }
    fn diff_selection(&self) -> Option<(Vec<plico::combined_changes::Change>, usize)> {
        let data = self.data.as_ref().filter(|d| !d.files.is_empty())?;
        let indices = self.selected_files();
        if indices.len() > 1 {
            return Some((
                indices.into_iter().map(|i| data.files[i].clone()).collect(),
                0,
            ));
        }
        Some((data.files.clone(), indices.first().copied().unwrap_or(0)))
    }
    fn confirm_selection(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.selection.len() > 1 {
            self.show_diff(w, cx);
        } else if let Some(navigation::Key::Folder(path)) = self.selection.iter().next() {
            let path = path.clone();
            if !self.collapsed.remove(&path) {
                self.collapsed.insert(path);
            }
            self.rebuild();
            cx.notify();
        } else if !self.selection.is_empty() {
            self.show_diff(w, cx);
        }
    }
    fn show_diff(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some((files, index)) = self.diff_selection() else {
            return;
        };
        let (target, chain) =
            match plico::diff_target::Chain::combined(self.repo.clone(), files, index) {
                Ok(pair) => pair,
                Err(e) => {
                    self.error = Some(e);
                    cx.notify();
                    return;
                }
            };
        let viewer = cx.new(|_| crate::diff_tab::DiffTab::new(target, chain));
        let activate = viewer.clone();
        w.open_dialog(cx, move |dialog, w, _| {
            let next = viewer.clone();
            let previous = viewer.clone();
            dialog
                .title(t("Diff"))
                .w(px(1200.))
                .overlay_closable(false)
                .child(
                    div()
                        .h((w.viewport_size().height - px(170.)).clamp(px(240.), px(700.)))
                        .child(viewer.clone())
                        .capture_key_down(move |e: &KeyDownEvent, w, cx| {
                            if e.keystroke.key == "f7" {
                                if e.keystroke.modifiers.shift {
                                    previous.update(cx, |s, cx| s.navigate(false, w, cx));
                                } else {
                                    next.update(cx, |s, cx| s.navigate(true, w, cx));
                                }
                                cx.stop_propagation();
                            }
                        }),
                )
        });
        activate.update(cx, |s, cx| s.activate(w, cx));
    }
}
impl Drop for Details {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests;
mod view;

mod navigation;
