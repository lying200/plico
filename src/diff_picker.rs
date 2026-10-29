//! Popup rows share the tab's immutable file metadata; only visible rows render.
use crate::diff_tab::DiffTab;
use gpui_kit::component::{
    Icon, IndexPath, h_flex,
    list::{ListDelegate, ListItem, ListState},
};
use gpui_kit::*;
use plico::{
    diff_target::Chain,
    file_tree::{self, Row},
};
use std::{collections::HashSet, path::PathBuf};

pub struct FilePicker {
    chain: Chain,
    rows: Vec<Row>,
    collapsed: HashSet<PathBuf>,
    query: String,
    selected: Option<IndexPath>,
    owner: WeakEntity<DiffTab>,
}
impl FilePicker {
    pub fn new(chain: Chain, owner: WeakEntity<DiffTab>) -> Self {
        let rows = file_tree::rows(
            chain
                .changes()
                .iter()
                .enumerate()
                .map(|(i, c)| (i, c.path.as_path())),
            |_| false,
        );
        Self {
            chain,
            rows,
            collapsed: HashSet::new(),
            query: String::new(),
            selected: None,
            owner,
        }
    }
    pub fn initial_row(&self) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| matches!(r, Row::File { index, .. } if *index == self.chain.index()))
    }
    fn rebuild(&mut self) {
        self.rows = file_tree::rows(
            self.chain
                .changes()
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    c.path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&self.query)
                })
                .map(|(i, c)| (i, c.path.as_path())),
            |p| self.query.is_empty() && self.collapsed.contains(p),
        );
    }
}
impl ListDelegate for FilePicker {
    type Item = ListItem;
    fn items_count(&self, _: usize, _: &App) -> usize {
        self.rows.len()
    }
    fn confirm_on_single_click(&self) -> bool {
        false
    }
    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let row = self.rows.get(ix.row)?;
        let (content, depth) = match row {
            Row::File { index, depth } => {
                let change = &self.chain.changes()[*index];
                let label = change
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                (
                    h_flex()
                        .gap_2()
                        .pl(px(16.))
                        .child(img(crate::ui_style::file_icon(&change.path, cx)).size(px(16.)))
                        .child(
                            div()
                                .text_color(rgb(crate::ui_style::change_color(change, cx)))
                                .child(label),
                        ),
                    *depth,
                )
            }
            Row::Folder {
                path,
                depth,
                expanded,
                count,
                ..
            } => {
                let path = path.clone();
                let label = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let chevron = if *expanded {
                    "jb/chevronDown_dark.svg"
                } else {
                    "jb/chevronRight_dark.svg"
                };
                (
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .id(("expand-folder", ix.row))
                                .cursor_pointer()
                                .child(Icon::default().path(chevron).size(px(16.)))
                                .on_click(cx.listener(move |state, _, window, cx| {
                                    cx.stop_propagation();
                                    let picker = state.delegate_mut();
                                    if !picker.collapsed.remove(&path) {
                                        picker.collapsed.insert(path.clone());
                                    }
                                    picker.rebuild();
                                    let row = picker.rows.iter().position(
                                        |r| matches!(r, Row::Folder { path: p, .. } if p == &path),
                                    );
                                    state.set_selected_index(
                                        row.map(|row| IndexPath::default().row(row)),
                                        window,
                                        cx,
                                    );
                                    cx.notify();
                                })),
                        )
                        .child(
                            img(crate::ui_style::themed_asset("jb/folder_dark_4x.svg", cx))
                                .size(px(16.)),
                        )
                        .child(label)
                        .child(
                            div()
                                .text_color(rgb(crate::ui_theme::color(cx, 0x9fa2a8, 0x5f6269)))
                                .child(crate::ui_style::file_count(*count)),
                        ),
                    *depth,
                )
            }
        };
        Some(
            ListItem::new(ix.row)
                .h(px(28.))
                .pl(px(8. + depth as f32 * 16.))
                .child(content),
        )
    }
    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
    }
    fn perform_search(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_lowercase();
        self.rebuild();
        self.selected = None;
        let row = if self.query.is_empty() {
            self.initial_row()
        } else {
            None
        }
        .or_else(|| self.rows.iter().position(|r| matches!(r, Row::File { .. })));
        let query = self.query.clone();
        cx.notify();
        // List resets selection before awaiting this task. Apply the file choice afterwards.
        cx.spawn_in(window, async move |list, window| {
            let _ = list.update_in(window, |list, window, cx| {
                if list.delegate().query != query {
                    return;
                }
                list.set_selected_index(row.map(|row| IndexPath::default().row(row)), window, cx);
                cx.notify();
            });
        })
    }
    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(row) = self.selected.and_then(|ix| self.rows.get(ix.row)).cloned() else {
            return;
        };
        match row {
            Row::File { index, .. } => {
                let _ = self
                    .owner
                    .update(cx, |tab, cx| tab.choose_file(index, window, cx));
            }
            Row::Folder { path, .. } => {
                if !self.collapsed.remove(&path) {
                    self.collapsed.insert(path);
                }
                self.rebuild();
                cx.notify();
            }
        }
    }
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let _ = self
            .owner
            .update(cx, |tab, cx| tab.dismiss_picker(window, cx));
    }
}
