//! CompareWithLocalDialog / ChangesBrowserToolWindow (Rebased 4aec352,
//! JetBrains, Apache-2.0): fixed revisions, one replacement Changes tool window.
use gpui_kit::component::{
    Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    diff_target::{Chain, Target},
    file_tree,
    git::{Cancel, Repository},
    i18n::t,
    tree_selection::{Node, Selection},
};
use std::{collections::HashSet, path::PathBuf};
actions!(
    version_compare,
    [
        ShowDiff,
        Activate,
        Next,
        Previous,
        ExtendNext,
        ExtendPrevious,
        Expand,
        Collapse,
        ToggleDirectory
    ]
);
pub struct OpenDiff(pub Target, pub Chain);
pub struct SelectionChanged;
pub struct Reloaded;
pub struct Close;
pub struct Refresh;
#[derive(Clone)]
enum Source {
    Revisions { before: String, after: String },
    Branches { before: String, after: String },
    WorkingTree { reference: String, current: String },
}

pub struct CompareView {
    repo: Repository,
    source: Source,
    repository_color: Option<u32>,
    branch_context: Option<plico::branch_context::Context>,
    branch_token: Cancel,
    branch_task: Task<()>,
    branch_generation: u64,
    swapped: bool,
    chain: Option<Chain>,
    rows: Vec<file_tree::Row>,
    collapsed: HashSet<PathBuf>,
    group_files: bool,
    selection: Selection,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    token: Cancel,
    task: Task<()>,
    generation: u64,
    loading: bool,
    error: Option<String>,
}
impl EventEmitter<OpenDiff> for CompareView {}
impl EventEmitter<SelectionChanged> for CompareView {}
impl EventEmitter<Reloaded> for CompareView {}
impl EventEmitter<Close> for CompareView {}
impl EventEmitter<Refresh> for CompareView {}
impl Drop for CompareView {
    fn drop(&mut self) {
        self.token.cancel();
        self.branch_token.cancel();
    }
}
fn icon(name: &'static str) -> Icon {
    Icon::default().path(format!("jb/{name}")).size(px(16.))
}
impl CompareView {
    pub fn new(
        repo: Repository,
        before: String,
        after: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create(repo, Source::Revisions { before, after }, window, cx)
    }
    pub fn branches(
        repo: Repository,
        before: String,
        after: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create(repo, Source::Branches { before, after }, window, cx)
    }
    pub fn working_tree(
        repo: Repository,
        reference: String,
        current: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create(repo, Source::WorkingTree { reference, current }, window, cx)
    }
    fn create(
        repo: Repository,
        source: Source,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.bind_keys([
            KeyBinding::new("ctrl-alt-p", ToggleDirectory, Some("VersionCompare")),
            KeyBinding::new("enter", Activate, Some("VersionCompare")),
            KeyBinding::new("right", Expand, Some("VersionCompare")),
            KeyBinding::new("left", Collapse, Some("VersionCompare")),
            KeyBinding::new("ctrl-d", ShowDiff, Some("VersionCompare")),
            KeyBinding::new("down", Next, Some("VersionCompare")),
            KeyBinding::new("up", Previous, Some("VersionCompare")),
            KeyBinding::new("shift-down", ExtendNext, Some("VersionCompare")),
            KeyBinding::new("shift-up", ExtendPrevious, Some("VersionCompare")),
        ]);
        let swapped = plico::layout_preferences::load_comparison_swapped(&repo.path);
        let mut s = Self {
            repo,
            source,
            repository_color: None,
            branch_context: None,
            branch_token: Cancel::default(),
            branch_task: Task::ready(()),
            branch_generation: 0,
            swapped,
            chain: None,
            rows: Vec::new(),
            collapsed: HashSet::new(),
            group_files: true,
            selection: Selection::default(),
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            token: Cancel::default(),
            task: Task::ready(()),
            generation: 0,
            loading: false,
            error: None,
        };
        s.focus.focus(window, cx);
        s.refresh(cx);
        s.refresh_branch_context(cx);
        s
    }
    pub fn repository_changed(
        &mut self,
        root: &std::path::Path,
        batch: &plico::worktree_events::Batch,
        cx: &mut Context<Self>,
    ) {
        if batch.history_changed()
            && plico::recent_projects::identity(root)
                == plico::recent_projects::identity(&self.repo.path)
        {
            self.refresh_branch_context(cx);
        }
    }
    fn refresh_branch_context(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.source, Source::WorkingTree { .. }) {
            return;
        }
        self.branch_token.cancel();
        self.branch_token = Cancel::default();
        self.branch_generation += 1;
        let generation = self.branch_generation;
        let token = self.branch_token.clone();
        let repo = self.repo.clone();
        self.branch_task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { plico::branch_context::read(&repo, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.branch_generation != generation {
                    return;
                }
                s.branch_context = result.ok();
                cx.notify();
            });
        });
    }
    pub fn repository_path(&self) -> &std::path::Path {
        &self.repo.path
    }
    /// The project owns root membership and colors; the view owns tree state.
    pub fn set_repository_color(&mut self, color: Option<u32>, cx: &mut Context<Self>) {
        if self.repository_color == color {
            return;
        }
        self.repository_color = color;
        self.rebuild();
        cx.notify();
    }
    pub fn focus_files(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }
    pub fn follow_target(&mut self, target: &Target, cx: &mut Context<Self>) {
        if self.selection.includes_file(&target.change().path)
            && self.chain.as_ref().is_some_and(|chain| {
                chain
                    .changes()
                    .iter()
                    .filter(|c| self.selection.includes_file(&c.path))
                    .take(2)
                    .count()
                    > 1
            })
        {
            return;
        }
        self.select_target(target, cx);
    }
    pub fn select_target(&mut self, target: &Target, cx: &mut Context<Self>) {
        let Some(chain) = self.chain.as_ref().and_then(|c| c.fork_for(target)) else {
            return;
        };
        self.selection
            .replace(vec![Node::File(target.change().path.clone())]);
        for parent in target.change().path.ancestors().skip(1) {
            self.collapsed.remove(parent);
        }
        self.chain = Some(chain);
        self.rebuild();
        cx.notify();
    }
    fn selection_changed(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        cx.emit(SelectionChanged);
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.token.cancel();
        self.token = Cancel::default();
        self.generation += 1;
        self.loading = true;
        self.error = None;
        let initial = self.chain.is_none();
        let repo = self.repo.clone();
        let source = self.source.clone();
        let swapped = self.swapped;
        let token = self.token.clone();
        let generation = self.generation;
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    match source {
                        Source::Revisions { .. } | Source::Branches { .. } => {
                            let (before, after) = match source {
                                Source::Revisions { before, after } => (before, after),
                                Source::Branches { before, after } => {
                                    let pair = plico::branch_compare::Pair::resolve(
                                        &repo, &before, &after, &token,
                                    )?;
                                    (pair.first, pair.second)
                                }
                                Source::WorkingTree { .. } => unreachable!(),
                            };
                            let changes = repo.revision_changes(&before, &after, &[], &token)?;
                            if changes.is_empty() {
                                Ok(None)
                            } else {
                                Chain::combined(repo, changes, 0).map(|(_, chain)| Some(chain))
                            }
                        }
                        Source::WorkingTree { reference, .. } => {
                            let id = plico::branch_compare::resolve_reference(
                                &repo, &reference, &token,
                            )?;
                            let mut changes = plico::working_comparison::load(&repo, &id, &token)?;
                            if swapped {
                                changes = changes
                                    .iter()
                                    .map(plico::working_comparison::Change::reversed)
                                    .collect();
                            }
                            if changes.is_empty() {
                                Ok(None)
                            } else {
                                Chain::working_comparison(repo, changes, 0)
                                    .map(|(_, chain)| Some(chain))
                            }
                        }
                    }
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.generation != generation {
                    return;
                }
                s.loading = false;
                match result {
                    Ok(chain) => {
                        s.chain = chain;
                        s.selection.retain(|node| {
                            s.chain.as_ref().is_some_and(|chain| {
                                chain.changes().iter().any(|change| match node {
                                    Node::File(path) => &change.path == path,
                                    Node::Directory(path) => change.path.starts_with(path),
                                })
                            })
                        });
                        s.rebuild();
                        if initial && s.selection.is_empty() {
                            if let Some(node) = s
                                .row_nodes()
                                .into_iter()
                                .find(|n| matches!(n, Node::File(_)))
                            {
                                s.selection.replace(vec![node]);
                            }
                        }
                    }
                    Err(error) => {
                        s.error = Some(error);
                        s.chain = None;
                        s.selection = Selection::default();
                        s.rows.clear();
                    }
                }
                cx.emit(Reloaded);
                cx.notify();
            });
        });
        cx.notify();
    }
    fn swap_sides(&mut self, cx: &mut Context<Self>) {
        if self.loading || !matches!(self.source, Source::WorkingTree { .. }) {
            return;
        }
        if let Some(chain) = &self.chain {
            match chain.reversed_working_comparison() {
                Ok(chain) => self.chain = Some(chain),
                Err(error) => {
                    self.error = Some(error);
                    cx.notify();
                    return;
                }
            }
        }
        self.swapped = !self.swapped;
        self.rebuild();
        let preview = self.diff_selection();
        if let Some((target, _)) = &preview {
            for parent in target.change().path.ancestors().skip(1) {
                self.collapsed.remove(parent);
            }
            self.rebuild();
        }
        cx.emit(Reloaded);
        cx.notify();
    }
    fn direction_text(&self) -> Option<(StyledText, std::ops::Range<usize>)> {
        let Source::WorkingTree { reference, current } = &self.source else {
            return None;
        };
        let branch = reference
            .strip_prefix("refs/heads/")
            .or_else(|| reference.strip_prefix("refs/remotes/"))
            .unwrap_or(reference);
        let mut template = if self.swapped {
            t("Differences between files in {branch} and the current working tree on {current}.")
        } else {
            t("Differences between the current working tree on {current} and files in {branch}.")
        };
        let mut text = String::new();
        let mut highlights = Vec::new();
        // Only interpolate the template: braces in a real branch name stay literal.
        while let Some((start, token, value)) =
            [("{branch}", branch), ("{current}", current.as_str())]
                .into_iter()
                .filter_map(|(token, value)| {
                    template.find(token).map(|start| (start, token, value))
                })
                .min_by_key(|(start, _, _)| *start)
        {
            text.push_str(&template[..start]);
            let begin = text.len();
            text.push_str(value);
            highlights.push((
                begin..text.len(),
                HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
            ));
            template = &template[start + token.len()..];
        }
        text.push_str(template);
        text.push('\u{2003}');
        let start = text.len();
        text.push_str(t("Swap branches"));
        let link = start..text.len();
        highlights.push((
            link.clone(),
            HighlightStyle {
                color: Some(
                    rgb(if self.loading || self.error.is_some() {
                        0x6f737a
                    } else {
                        0x589df6
                    })
                    .into(),
                ),
                ..Default::default()
            },
        ));
        Some((StyledText::new(text).with_highlights(highlights), link))
    }
    fn title(&self) -> String {
        match &self.source {
            Source::Revisions { before, after } => format!(
                "{} {} → {}",
                t("Changes"),
                before.chars().take(8).collect::<String>(),
                after.chars().take(8).collect::<String>()
            ),
            Source::Branches { before, after } => {
                let short = |name: &str| {
                    name.strip_prefix("refs/heads/")
                        .or_else(|| name.strip_prefix("refs/remotes/"))
                        .unwrap_or(name)
                        .to_owned()
                };
                t("Changes Between {before} and {after}")
                    .replace("{before}", &short(before))
                    .replace("{after}", &short(after))
            }
            Source::WorkingTree { reference, .. } => {
                t("Changes Between {branch} and Current Working Tree").replace(
                    "{branch}",
                    reference
                        .strip_prefix("refs/heads/")
                        .or_else(|| reference.strip_prefix("refs/remotes/"))
                        .unwrap_or(reference),
                )
            }
        }
    }
    fn toggle_directory(&mut self, cx: &mut Context<Self>) {
        let files: Vec<_> = self
            .chain
            .as_ref()
            .into_iter()
            .flat_map(|c| c.changes())
            .filter(|c| self.selection.includes_file(&c.path))
            .map(|c| c.path.clone())
            .collect();
        self.selection
            .replace(files.iter().cloned().map(Node::File).collect());
        self.group_files = !self.group_files;
        if self.group_files {
            for path in &files {
                for parent in path.ancestors().skip(1) {
                    self.collapsed.remove(parent);
                }
            }
        }
        self.rebuild();
        self.selection_changed(cx);
        cx.notify();
    }
    fn set_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        if !self.group_files && self.repository_color.is_none() {
            return;
        }
        let lead = self.selection.lead_node().cloned();
        self.collapsed.clear();
        if !expanded {
            if let Some(chain) = &self.chain {
                for change in chain.changes() {
                    self.collapsed
                        .extend(change.path.ancestors().skip(1).map(PathBuf::from));
                }
            }
        }
        self.rebuild();
        if !expanded && let Some(lead) = lead {
            let path = match &lead {
                Node::File(path) | Node::Directory(path) => path,
            };
            let nodes = self.row_nodes();
            let visible = nodes
                .iter()
                .find(|node| *node == &lead)
                .cloned()
                .or_else(|| {
                    nodes
                        .iter()
                        .filter_map(|node| match node {
                            Node::Directory(folder) if path.starts_with(folder) => {
                                Some((folder.components().count(), node))
                            }
                            _ => None,
                        })
                        .max_by_key(|(depth, _)| *depth)
                        .map(|(_, node)| node.clone())
                });
            if let Some(node) = visible {
                self.selection.replace(vec![node]);
            }
            self.selection_changed(cx);
        }
        cx.notify();
    }
    fn rebuild(&mut self) {
        self.rows.clear();
        let Some(chain) = &self.chain else {
            return;
        };
        let changes = chain.changes();
        if changes.is_empty() {
            return;
        }
        let mut rows = if self.group_files {
            file_tree::compact_rows(
                changes
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.path.as_path())),
                |p| self.collapsed.contains(p),
            )
        } else {
            let mut indices = (0..changes.len()).collect::<Vec<_>>();
            indices.sort_by_key(|&i| (changes[i].path.file_name(), &changes[i].path));
            indices
                .into_iter()
                .map(|index| file_tree::Row::File { index, depth: 0 })
                .collect()
        };
        let common_directory = matches!(rows.first(), Some(file_tree::Row::Folder {depth:0,count,..}) if *count == changes.len());
        if self.repository_color.is_none() && (common_directory || !self.group_files) {
            self.rows = rows;
            return;
        }
        let expanded = !self.collapsed.contains(&PathBuf::new());
        self.rows.push(file_tree::Row::Folder {
            path: PathBuf::new(),
            label: PathBuf::new(),
            depth: 0,
            count: changes.len(),
            expanded,
        });
        if expanded {
            for row in &mut rows {
                match row {
                    file_tree::Row::Folder { depth, .. } | file_tree::Row::File { depth, .. } => {
                        *depth += 1
                    }
                }
            }
            self.rows.extend(rows);
        }
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let grouped = self.group_files;
        let has_groups = grouped || self.repository_color.is_some();
        let focus = self.focus.clone();
        let disabled = self.loading || self.chain.is_none();
        h_flex()
            .h(px(32.))
            .px_2()
            .gap_1()
            .when(matches!(self.source, Source::WorkingTree { .. }), |bar| {
                bar.child(
                    Button::new("compare-refresh")
                        .ghost()
                        .small()
                        .icon(icon("refresh_dark.svg"))
                        .tooltip(t("Refresh"))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(Refresh))),
                )
            })
            .child(
                Button::new("compare-show-diff")
                    .ghost()
                    .small()
                    .icon(icon("diff_dark.svg"))
                    .tooltip(t("Show Diff"))
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, _, cx| s.show_diff(cx))),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(rgb(0x393b40)))
            .child(
                Button::new("compare-options")
                    .ghost()
                    .small()
                    .icon(icon("history-show_dark.svg"))
                    .tooltip(t("View Options"))
                    .dropdown_menu(move |menu, _, _| {
                        let owner = owner.clone();
                        menu.action_context(focus.clone())
                            .label(t("Group By"))
                            .item(
                                PopupMenuItem::new(t("Directory"))
                                    .action(Box::new(ToggleDirectory))
                                    .checked(grouped)
                                    .on_click(move |_, _, cx| {
                                        let _ = owner.update(cx, |s, cx| s.toggle_directory(cx));
                                    }),
                            )
                    }),
            )
            .child(div().flex_1())
            .child(
                Button::new("compare-expand")
                    .ghost()
                    .small()
                    .icon(icon("expandAll_dark.svg"))
                    .tooltip(t("Expand All"))
                    .disabled(disabled || !has_groups)
                    .on_click(cx.listener(|s, _, _, cx| s.set_expanded(true, cx))),
            )
            .child(
                Button::new("compare-collapse")
                    .ghost()
                    .small()
                    .icon(icon("collapseAll_dark.svg"))
                    .tooltip(t("Collapse All"))
                    .disabled(disabled || !has_groups)
                    .on_click(cx.listener(|s, _, _, cx| s.set_expanded(false, cx))),
            )
    }
    fn diff_selection(&self) -> Option<(Target, Chain)> {
        self.resolve_selection(None, false)
    }
    pub fn reloaded_selection(&self, current: &Target) -> Option<(Target, Chain)> {
        fn paths(target: &Target) -> Option<(Option<&std::path::Path>, Option<&std::path::Path>)> {
            let id = target.identity();
            if let Some(change) = &id.combined {
                Some((
                    change.before.as_ref().map(|r| r.path.as_path()),
                    change.after.as_ref().map(|r| r.path.as_path()),
                ))
            } else {
                id.working_comparison.as_ref().map(|change| {
                    (
                        change.before.as_ref().map(|r| r.path()),
                        change.after.as_ref().map(|r| r.path()),
                    )
                })
            }
        }
        let mut chain = self.chain.clone()?;
        let replacement = (0..chain.len())
            .filter_map(|index| chain.select(index))
            .find(|candidate| {
                candidate.identity().repository == current.identity().repository
                    && paths(current).is_some()
                    && paths(candidate) == paths(current)
            });
        self.resolve_selection(replacement.as_ref(), true)
    }
    pub fn preview_selection(&self, current: &Target) -> Option<(Target, Chain)> {
        self.resolve_selection(Some(current), true)
    }
    fn resolve_selection(
        &self,
        current: Option<&Target>,
        allow_empty: bool,
    ) -> Option<(Target, Chain)> {
        if self.loading {
            return None;
        }
        let mut chain = self.chain.clone()?;
        let changes = chain.changes();
        let mut order: Vec<_> = if self.group_files {
            file_tree::rows(
                changes
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.path.as_path())),
                |_| false,
            )
            .into_iter()
            .filter_map(|row| match row {
                file_tree::Row::File { index, .. } => Some(index),
                _ => None,
            })
            .collect()
        } else {
            let mut order = (0..changes.len()).collect::<Vec<_>>();
            order.sort_by_key(|&i| (changes[i].path.file_name(), &changes[i].path));
            order
        };
        let mut seen = HashSet::new();
        let selected: Vec<_> = self
            .selection
            .ordered()
            .iter()
            .flat_map(|node| {
                order.iter().copied().filter(move |&i| match node {
                    Node::File(path) => &changes[i].path == path,
                    Node::Directory(path) => changes[i].path.starts_with(path),
                })
            })
            .filter(|i| seen.insert(*i))
            .collect();
        let retained = current
            .and_then(|target| chain.fork_for(target))
            .map(|chain| chain.index())
            .filter(|i| selected.contains(i) || (selected.is_empty() && allow_empty));
        let index = retained.or_else(|| selected.first().copied()).or_else(|| {
            (allow_empty && selected.is_empty())
                .then(|| order.first().copied())
                .flatten()
        })?;
        if selected.len() > 1 {
            order = selected;
        }
        let target = chain.select(index)?;
        Some((target, chain.subset(&order)?))
    }
    fn show_diff(&mut self, cx: &mut Context<Self>) {
        if let Some((target, chain)) = self.diff_selection() {
            cx.emit(OpenDiff(target, chain));
        }
    }
    fn row_nodes(&self) -> Vec<Node> {
        self.rows
            .iter()
            .map(|row| match row {
                file_tree::Row::Folder { path, .. } => Node::Directory(path.clone()),
                file_tree::Row::File { index, .. } => {
                    Node::File(self.chain.as_ref().unwrap().changes()[*index].path.clone())
                }
            })
            .collect()
    }
    #[cfg(test)]
    fn selected(&self) -> Option<usize> {
        match self.selection.lead_node()? {
            Node::File(path) => self
                .chain
                .as_ref()?
                .changes()
                .iter()
                .position(|c| &c.path == path),
            _ => None,
        }
    }
    fn selected_folder(&self) -> Option<PathBuf> {
        match self.selection.lead_node()? {
            Node::Directory(path) => Some(path.clone()),
            _ => None,
        }
    }
    fn active_row(&self) -> Option<usize> {
        self.selection.lead(&self.row_nodes())
    }
    fn choose_row(&mut self, row: usize, cx: &mut Context<Self>) {
        self.select_row(row, false, false, cx);
    }
    fn select_row(&mut self, row: usize, toggle: bool, extend: bool, cx: &mut Context<Self>) {
        let nodes = self.row_nodes();
        if row >= nodes.len() {
            return;
        }
        self.selection.select(&nodes, row, toggle, extend);
        self.selection_changed(cx);
        self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        cx.notify();
    }
    fn select_file(&mut self, index: usize, toggle: bool, extend: bool, cx: &mut Context<Self>) {
        if let Some(row) = self
            .rows
            .iter()
            .position(|r| matches!(r,file_tree::Row::File {index:i,..} if *i == index))
        {
            self.select_row(row, toggle, extend, cx);
        }
    }
    fn extend_step(&mut self, direction: isize, cx: &mut Context<Self>) {
        self.selection.step(&self.row_nodes(), direction, true);
        self.selection_changed(cx);
        if let Some(row) = self.active_row() {
            self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        }
        cx.notify();
    }
    fn activate(&mut self, cx: &mut Context<Self>) {
        if let Some(folder) = self.selected_folder() {
            if !self.collapsed.remove(&folder) {
                self.collapsed.insert(folder);
            }
            self.rebuild();
            cx.notify();
        } else {
            self.show_diff(cx);
        }
    }
    fn step(&mut self, direction: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let row = self
            .active_row()
            .map(|i| i.saturating_add_signed(direction).min(self.rows.len() - 1))
            .unwrap_or(0);
        self.choose_row(row, cx);
    }
    fn expand_row(&mut self, expand: bool, cx: &mut Context<Self>) {
        let Some(index) = self.active_row() else {
            self.choose_row(0, cx);
            return;
        };
        let row = self.rows[index].clone();
        let depth = match &row {
            file_tree::Row::Folder { depth, .. } | file_tree::Row::File { depth, .. } => *depth,
        };
        if let file_tree::Row::Folder { path, expanded, .. } = &row {
            if *expanded != expand {
                if expand {
                    self.collapsed.remove(path);
                } else {
                    self.collapsed.insert(path.clone());
                }
                self.rebuild();
                cx.notify();
                return;
            }
        }
        if expand {
            self.choose_row(index + 1, cx);
        } else if let Some(parent) = (0..index)
            .rev()
            .find(|&i| matches!(&self.rows[i],file_tree::Row::Folder {depth:d,..} if *d < depth))
        {
            self.choose_row(parent, cx);
        }
    }
}
impl Render for CompareView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full().min_w_0().rounded(px(10.)).overflow_hidden().bg(rgb(0x181a1b))
            .track_focus(&self.focus).key_context("VersionCompare")
                .on_action(cx.listener(|s, _: &ShowDiff, _, cx| s.show_diff(cx)))
                .on_action(cx.listener(|s, _: &Activate, _, cx| s.activate(cx)))
                .on_action(cx.listener(|s, _: &Expand, _, cx| s.expand_row(true,cx)))
                .on_action(cx.listener(|s, _: &Collapse, _, cx| s.expand_row(false,cx)))
                .on_action(cx.listener(|s, _: &ToggleDirectory, _, cx| s.toggle_directory(cx)))
                .on_action(cx.listener(|s, _: &Next, _, cx| s.step(1, cx)))
                .on_action(cx.listener(|s, _: &Previous, _, cx| s.step(-1, cx)))
                .on_action(cx.listener(|s, _: &ExtendNext, _, cx| s.extend_step(1, cx)))
                .on_action(cx.listener(|s, _: &ExtendPrevious, _, cx| s.extend_step(-1, cx)))

            .child(h_flex().h(px(36.)).px_2().gap_1().child(div().flex_1().min_w_0().truncate()
                .child(self.title()))
                .child(Button::new("compare-close").ghost().small().icon(icon("closeSmall.svg")).tooltip(t("Close"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Close)))))
            .when_some(self.direction_text(), |v, (text, link)| {
                let owner = cx.entity().downgrade();
                v.child(div().px_2().py_1().child(InteractiveText::new("compare-swap", text)
                    .on_click(if self.loading || self.error.is_some() { vec![] } else { vec![link] }, move |_, _, cx| {
                        let _ = owner.update(cx, |s, cx| {
                            if s.loading || s.error.is_some() { return; }
                            s.swap_sides(cx);
                            if let Err(error) = plico::layout_preferences::save_comparison_swapped(&s.repo.path, s.swapped) {
                                s.error = Some(error.to_string()); cx.notify();
                            }
                        });
                    })))
            })
            .child(self.toolbar(cx))
            .child(div().flex_1().min_h_0()
                .when(self.loading, |v| v.child(div().p_3().child(t("Loading…"))))
                .when_some(self.error.clone(), |v, error| v.child(crate::error_ui::notice(
                    "compare-error", "Could not load comparison. Refresh to retry.", error, self.loading,
                    cx.listener(|_, _, _, cx| cx.emit(Refresh)))))
                .when(!self.loading && self.error.is_none() && self.chain.is_none(), |v| v.child(div().p_3().child(t("No differences"))))
                .when(!self.loading && self.chain.is_some(), |v| v.child(uniform_list("compare-files", self.rows.len(), cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                    range.map(|row| {
                        let entry = s.rows[row].clone();
                        let (depth, name, image, selected) = match &entry {
                            file_tree::Row::Folder { path, label, depth, expanded, count, .. } => (*depth, if label.as_os_str().is_empty() {
                                if s.repository_color.is_some() { s.repo.path.file_name().unwrap_or(s.repo.path.as_os_str()).to_string_lossy().into_owned() }
                                else { crate::ui_style::file_count(*count) }
                            } else { label.to_string_lossy().into_owned() }, if *expanded { "chevronDown_dark.svg" } else { "chevronRight_dark.svg" }, s.selection.contains(&Node::Directory(path.clone()))),
                            file_tree::Row::File { index, depth } => (*depth, s.chain.as_ref().unwrap().changes()[*index].path.file_name().unwrap_or_default().to_string_lossy().into_owned(), "file-text_dark_4x.svg", s.selection.contains(&Node::File(s.chain.as_ref().unwrap().changes()[*index].path.clone()))),
                        };
                        let color = match &entry {
                            file_tree::Row::File { index, .. } => crate::ui_style::change_color(&s.chain.as_ref().unwrap().changes()[*index]),
                            _ => crate::ui_style::FG,
                        };
                        h_flex().text_color(rgb(color)).id(("compare-file", row)).w_full().h(px(28.)).pl(px(10. + depth as f32 * 16.)).gap_2().min_w_0()
                            .when(selected, |v| v.bg(rgb(0x2e4773))).child(match &entry { file_tree::Row::File { index, .. } => img(crate::ui_style::file_icon(&s.chain.as_ref().unwrap().changes()[*index].path)).size(px(16.)).flex_shrink_0().into_any_element(), _ => div().id(("compare-disclosure", row)).child(icon(image)).on_mouse_down(MouseButton::Left, cx.listener(move |s,_,w,cx| { cx.stop_propagation(); s.focus.focus(w,cx); s.choose_row(row,cx); s.activate(cx); })).into_any_element() }).when(matches!(entry, file_tree::Row::Folder { .. }), |v| {
                                let root_color = match &entry {
                                    file_tree::Row::Folder { path, .. } if path.as_os_str().is_empty() => s.repository_color,
                                    _ => None,
                                };
                                v.child(if let Some(color) = root_color {
                                    div().size(px(14.)).rounded(px(2.)).bg(rgb(color)).flex_shrink_0().into_any_element()
                                } else {
                                    img("jb/folder_dark.svg").size(px(16.)).flex_shrink_0().into_any_element()
                                })
                            })
                            .child(div().flex_shrink_0().child(name))
                            .when_some(match &entry { file_tree::Row::Folder {label,count,..} if !label.as_os_str().is_empty() || s.repository_color.is_some() => Some(*count), _ => None }, |v,count| v.child(div().text_color(rgb(0x6f737a)).child(crate::ui_style::file_count(count))))
                            .when_some(match &entry {
                                file_tree::Row::Folder {path, ..} if path.as_os_str().is_empty() && s.repository_color.is_some() => s.branch_context.as_ref(),
                                _ => None,
                            }, |v, branch| {
                                let tooltip = branch.branch.as_ref().map(|name| branch.upstream.as_ref().map(|upstream| format!("{name} → {upstream}")).unwrap_or_else(|| t("No tracking branch").to_owned()));
                                v.child(div().id(("compare-root-branch", row)).px(px(4.)).bg(rgb(0x2b2d30)).text_color(rgb(0x909090)).flex_shrink_0().child(branch.label().to_owned()).when_some(tooltip, |v, text| v.tooltip(move |w,cx| Tooltip::new(text.clone()).build(w,cx))))
                            })
                            .when(!s.group_files, |v| {
                                if let file_tree::Row::File { index, .. } = &entry {
                                    let path = &s.chain.as_ref().unwrap().changes()[*index].path;
                                    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(&s.repo.path);
                                    v.child(div().min_w_0().truncate().text_color(rgb(0x6f737a)).child(if parent == s.repo.path { parent.file_name().unwrap_or_default().to_string_lossy().into_owned() } else { parent.to_string_lossy().into_owned() }))
                                } else { v }
                            })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |s, event: &MouseDownEvent, window, cx| {
                                s.focus.focus(window, cx);
                                match &entry {
                                    file_tree::Row::Folder { .. } => { s.select_row(row,event.modifiers.control || event.modifiers.platform,event.modifiers.shift,cx); if event.click_count == 2 { s.show_diff(cx); } }
                                    file_tree::Row::File { index, .. } => { s.select_file(*index, event.modifiers.control || event.modifiers.platform, event.modifiers.shift, cx); if event.click_count == 2 { s.show_diff(cx); } }
                                }
                                cx.notify();
                            })).into_any_element()
                    }).collect()
                })).track_scroll(&self.scroll).size_full())))
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;

    #[gpui::test]
    fn nested_directory_diff_scope_survives_collapsed_children(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let repo = Repository::new(std::env::temp_dir());
        let mut comparison = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view =
                cx.new(|cx| CompareView::new(repo.clone(), "1".repeat(40), "2".repeat(40), w, cx));
            comparison = Some(view.clone());
            Root::new(view, w, cx)
        });
        cx.run_until_parked();
        comparison.unwrap().update(cx, |v,cx| {
            use plico::combined_changes::{Change,Revision};
            let paths = ["preview-nav/alpha.txt", "preview-nav/nested/beta.txt", "preview-nav/nested/gamma.txt", "preview-nav/only/solo.txt"];
            let (_,chain) = Chain::combined(repo, paths.iter().map(|p| Change {
                before: None, after: Some(Revision { revision: "2".repeat(40),path: (*p).into() })
            }).collect(),0).unwrap();
            v.chain = Some(chain);
            v.loading = false;
            v.error = None;
            v.rebuild();
            assert!(matches!(&v.rows[0], file_tree::Row::Folder {path,depth:0,count:4,..} if path == std::path::Path::new("preview-nav")));
            let nested = v.rows.iter().position(|r| matches!(r,file_tree::Row::Folder {path,..} if path == std::path::Path::new("preview-nav/nested"))).unwrap();
            v.choose_row(nested,cx);
            v.activate(cx);
            let (target,chain) = v.diff_selection().unwrap();
            assert_eq!(target.source_path(), std::path::Path::new(paths[1]));
            assert_eq!(chain.changes().iter().map(|c| c.path.as_path()).collect::<Vec<_>>(), vec![std::path::Path::new(paths[1]),std::path::Path::new(paths[2])]);
            assert_eq!(v.chain.as_ref().unwrap().len(),4);
            v.toggle_directory(cx);
            assert_eq!(v.row_nodes().iter().filter(|n| v.selection.contains(n)).count(),2);
            assert_eq!(v.diff_selection().unwrap().1.len(),2);
            assert!(v.selection.contains(&Node::File(paths[1].into())));
            assert!(v.selection.contains(&Node::File(paths[2].into())));
            v.toggle_directory(cx);
            assert_eq!(v.diff_selection().unwrap().1.len(),2);
            assert!(!v.collapsed.contains(std::path::Path::new("preview-nav/nested")));
            v.choose_row(nested,cx);
            v.extend_step(1,cx);
            assert!(v.selection.contains(&Node::Directory("preview-nav/nested".into())));
            assert!(v.selection.contains(&Node::File(paths[1].into())));
            v.extend_step(-1,cx);
            assert!(!v.selection.contains(&Node::File(paths[1].into())));
            assert_eq!(v.diff_selection().unwrap().1.len(),2);
            v.select_file(0,true,false,cx);
            assert!(v.selection.contains(&Node::Directory("preview-nav/nested".into())));
            assert!(!v.selection.contains(&Node::File(paths[2].into())));
            assert_eq!(v.diff_selection().unwrap().1.len(),3);
            let only = v.rows.iter().position(|r| matches!(r,file_tree::Row::Folder {path,..} if path == std::path::Path::new("preview-nav/only"))).unwrap();
            v.choose_row(only,cx);
            let (target,chain) = v.diff_selection().unwrap();
            assert_eq!(target.source_path(),std::path::Path::new(paths[3]));
            assert_eq!(chain.len(),4);
            v.select_target(&target,cx);
            assert_eq!(v.chain.as_ref().unwrap().len(),4);
            v.toggle_directory(cx);
            v.select_file(0,false,false,cx);
            v.select_file(2,true,false,cx);
            let (_,chain) = v.diff_selection().unwrap();
            assert_eq!(chain.len(),2);
            assert_eq!(chain.changes()[0].path,std::path::Path::new(paths[0]));
            assert_eq!(chain.changes()[1].path,std::path::Path::new(paths[2]));
            v.select_file(0,false,false,cx);
            v.select_file(2,false,true,cx);
            assert_eq!(v.diff_selection().unwrap().1.len(),3);
            v.select_file(0,false,false,cx);
            v.select_file(0,true,false,cx);
            assert!(v.diff_selection().is_none());
            let current = v.chain.as_mut().unwrap().select(2).unwrap();
            let (preview, scope) = v.preview_selection(&current).unwrap();
            assert_eq!(preview.identity(),current.identity());
            assert_eq!(scope.len(),4);
            v.select_file(2,false,false,cx);
            v.select_file(1,true,false,cx);
            let (preview, scope) = v.preview_selection(&current).unwrap();
            assert_eq!(preview.identity(),current.identity());
            assert_eq!(scope.len(),2);
            assert_eq!(scope.index(),0);
            assert_eq!(scope.changes()[0].path,std::path::Path::new(paths[2]));
            let next = v.chain.as_mut().unwrap().select(1).unwrap();
            v.follow_target(&next,cx);
            assert!(v.selection.contains(&Node::File(paths[2].into())));
            assert!(v.selection.contains(&Node::File(paths[1].into())));
            assert_eq!(v.preview_selection(&next).unwrap().1.index(),1);
            v.toggle_directory(cx);
            // Rebuilding grouping restores model order, retaining the current preview file.
            let (preview, scope) = v.preview_selection(&current).unwrap();
            assert_eq!(preview.identity(), current.identity());
            assert_eq!(scope.index(), 1);
            assert_eq!(scope.len(), 2);
            v.set_expanded(false,cx);
            assert_eq!(v.selected_folder(),Some(PathBuf::from("preview-nav")));
            assert_eq!(v.preview_selection(&next).unwrap().1.len(),4);
            assert_eq!(v.preview_selection(&next).unwrap().0.identity(),next.identity());
            v.set_expanded(true,cx);

            v.select_file(0,false,false,cx);
            assert_eq!(v.preview_selection(&current).unwrap().0.source_path(), std::path::Path::new(paths[0]));
            let repo = Repository::new(std::env::temp_dir());
            let (_,replacement) = Chain::combined(repo, paths.iter().map(|p| Change {
                before: None, after: Some(Revision { revision: "3".repeat(40),path: (*p).into() })
            }).collect(),0).unwrap();
            v.chain = Some(replacement);
            v.selection.replace(vec![Node::Directory("preview-nav".into())]);
            let (refreshed, scope) = v.reloaded_selection(&current).unwrap();
            assert_eq!(refreshed.source_path(),current.source_path());
            assert_ne!(refreshed.identity(),current.identity());
            assert_eq!(scope.len(),4);
            v.selection.replace(vec![Node::File(paths[0].into())]);
            assert_eq!(v.reloaded_selection(&current).unwrap().0.source_path(),std::path::Path::new(paths[0]));
            v.chain = None;
            assert!(v.reloaded_selection(&current).is_none());


        });
    }

    #[gpui::test]
    fn working_comparison_refresh_recovers_missing_ref_and_keeps_file_selection(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
                "plico-working-panel-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir(&fixture.0).unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-q", "-b", "main"]);
        for name in ["a.txt", "b.txt"] {
            std::fs::write(fixture.0.join(name), "base\n").unwrap();
        }
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Compare",
            "-c",
            "user.email=compare@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "base",
        ]);
        git(&["branch", "topic"]);
        for name in ["a.txt", "b.txt"] {
            std::fs::write(fixture.0.join(name), "local\n").unwrap();
        }
        let repo = Repository::new(fixture.0.clone());
        cx.update(gpui::component::init);
        let mut comparison = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| {
                CompareView::working_tree(repo, "refs/heads/topic".into(), "main".into(), w, cx)
            });
            comparison = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = comparison.unwrap();
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert_eq!(v.branch_context.as_ref().unwrap().label(), "main")
        });
        git(&["symbolic-ref", "HEAD", "refs/heads/topic"]);
        let refs = plico::worktree_events::Batch {
            refs: true,
            ..Default::default()
        };
        view.update(cx, |v, cx| {
            let generation = v.branch_generation;
            v.repository_changed(&fixture.0.join("other"), &refs, cx);
            assert_eq!(v.branch_generation, generation);
            v.repository_changed(&fixture.0, &refs, cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert_eq!(v.branch_context.as_ref().unwrap().label(), "topic");
            assert!(matches!(&v.source, Source::WorkingTree { current, .. } if current == "main"));
            assert_eq!(v.chain.as_ref().unwrap().changes().len(), 2);
        });
        git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
        view.update(cx, |v, cx| v.repository_changed(&fixture.0, &refs, cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("ctrl-alt-p");
        view.read_with(cx, |v, _| assert!(!v.group_files));
        cx.simulate_keystrokes("ctrl-alt-p");
        view.read_with(cx, |v, _| assert!(v.group_files));
        view.update(cx, |v, cx| {
            v.set_repository_color(Some(0x865762), cx);
            v.toggle_directory(cx);
            assert!(!v.group_files);
            assert!(matches!(v.rows.first(), Some(file_tree::Row::Folder { path, count: 2, .. }) if path.as_os_str().is_empty()));
            assert_eq!(v.rows.len(), 3, "flat directory mode still groups by repository");
            v.set_expanded(false, cx);
            assert_eq!(v.rows.len(), 1);
            v.set_expanded(true, cx);
            assert_eq!(v.rows.len(), 3);
            v.set_repository_color(None, cx);
            assert_eq!(v.rows.len(), 2, "returning to one root removes only the repository group");
            assert!(v.rows.iter().all(|row| matches!(row, file_tree::Row::File { depth: 0, .. })));
            v.toggle_directory(cx);
        });
        view.update(cx, |v, cx| {
            assert!(v.error.is_none(), "{:?}", v.error);
            assert_eq!(v.chain.as_ref().unwrap().changes().len(), 2);
            let target = v.chain.as_mut().unwrap().select(1).unwrap();
            assert_eq!(target.source_path(), std::path::Path::new("b.txt"));
            v.select_target(&target, cx);
            v.step(-1, cx);
            assert_eq!(v.selected(), Some(0));
            v.step(-1, cx);
            assert_eq!(v.selected_folder(), Some(PathBuf::new()));
            assert_eq!(v.selected(), None);
            let (folder_target, folder_chain) = v.diff_selection().unwrap();
            assert_eq!(folder_target.source_path(), std::path::Path::new("a.txt"));
            assert_eq!(folder_chain.len(), 2);
            v.expand_row(false, cx);
            assert_eq!(v.rows.len(), 1);
            v.activate(cx);
            assert_eq!(v.rows.len(), 3);
            v.expand_row(true, cx);
            assert_eq!(v.selected(), Some(0));
            v.expand_row(false, cx);
            assert_eq!(v.selected_folder(), Some(PathBuf::new()));
            v.select_target(&target, cx);
            v.set_expanded(false, cx);
            assert_eq!(v.rows.len(), 1);
            assert!(matches!(
                v.rows[0],
                file_tree::Row::Folder {
                    count: 2,
                    expanded: false,
                    ..
                }
            ));
            v.toggle_directory(cx);
            assert_eq!(v.rows.len(), 2);
            assert!(
                v.rows
                    .iter()
                    .all(|r| matches!(r, file_tree::Row::File { depth: 0, .. }))
            );
            assert_eq!(v.selected(), Some(0));
            assert_eq!(v.diff_selection().unwrap().1.len(), 2);
            v.toggle_directory(cx);
            // Restoring grouping reveals the selected file, as with a selected directory's descendants.
            assert_eq!(v.rows.len(), 3);
            v.select_target(&target, cx);
            assert_eq!(v.rows.len(), 3);
            assert_eq!(v.selected(), Some(1));
            v.set_expanded(false, cx);
            v.set_expanded(true, cx);
            assert_eq!(v.rows.len(), 3);
            assert_eq!(v.selected_folder(), Some(PathBuf::new()));
            v.select_target(&target, cx);
            let before = v
                .chain
                .as_mut()
                .unwrap()
                .select(1)
                .unwrap()
                .identity()
                .clone();
            let swapped = v.swapped;
            v.swap_sides(cx);
            assert_eq!(v.selected(), Some(1));
            assert_ne!(
                v.chain.as_mut().unwrap().select(1).unwrap().identity(),
                &before
            );
            assert_eq!(v.swapped, !swapped);
            v.swap_sides(cx);
            assert_eq!(
                v.chain.as_mut().unwrap().select(1).unwrap().identity(),
                &before
            );
            v.swap_sides(cx);
            assert!(v.swapped);
            v.refresh(cx);
            let superseded = v.token.clone();
            v.refresh(cx);
            assert!(superseded.cancelled());
        });
        cx.run_until_parked();
        view.update(cx, |v, _| {
            assert_eq!(v.selected(), Some(1));
            assert!(v.swapped);
            let target = v.chain.as_mut().unwrap().select(1).unwrap();
            assert!(matches!(
                target
                    .identity()
                    .working_comparison
                    .as_ref()
                    .unwrap()
                    .before,
                Some(plico::working_comparison::Endpoint::Revision { .. })
            ));
        });
        git(&["branch", "-D", "topic"]);
        view.update(cx, |v, cx| v.refresh(cx));
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(v.error.is_some());
            assert!(v.chain.is_none());
            assert!(v.selected().is_none());
        });
        git(&["branch", "topic"]);
        view.update(cx, |v, cx| v.refresh(cx));
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(v.error.is_none());
            assert_eq!(v.chain.as_ref().unwrap().changes().len(), 2);
        });
    }
    #[gpui::test]
    fn branch_files_compare_ignores_worktree_and_refreshes_references(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
                "plico-working-panel-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir(&fixture.0).unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-q", "-b", "main"]);
        for name in ["a.txt", "b.txt"] {
            std::fs::write(fixture.0.join(name), "base\n").unwrap();
        }
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Compare",
            "-c",
            "user.email=compare@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "base",
        ]);

        git(&["branch", "topic"]);
        std::fs::write(fixture.0.join("a.txt"), "committed main\n").unwrap();
        git(&["add", "a.txt"]);
        git(&[
            "-c",
            "user.name=Compare",
            "-c",
            "user.email=compare@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "main change",
        ]);
        std::fs::write(fixture.0.join("b.txt"), "uncommitted\n").unwrap();
        let repo = Repository::new(fixture.0.clone());
        cx.update(gpui::component::init);
        let mut comparison = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| {
                CompareView::branches(
                    repo,
                    "refs/heads/topic".into(),
                    "refs/heads/main".into(),
                    w,
                    cx,
                )
            });
            comparison = Some(view.clone());
            Root::new(view, w, cx)
        });
        let view = comparison.unwrap();
        cx.run_until_parked();
        view.update(cx, |v, _| {
            assert!(v.error.is_none(), "{:?}", v.error);
            let chain = v.chain.as_mut().unwrap();
            assert_eq!(chain.changes().len(), 1);
            assert_eq!(
                chain.select(0).unwrap().source_path(),
                std::path::Path::new("a.txt")
            );
            assert!(v.title().contains("topic"));
            assert!(v.title().contains("main"));
        });
        git(&["branch", "-f", "topic", "main"]);
        view.update(cx, |v, cx| v.refresh(cx));
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(v.error.is_none());
            assert!(v.chain.is_none(), "equal endpoints must ignore dirty b.txt");
        });
        git(&["branch", "-D", "topic"]);
        view.update(cx, |v, cx| v.refresh(cx));
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(v.error.is_some());
            assert!(v.chain.is_none());
        });
        git(&["branch", "topic", "main~1"]);
        view.update(cx, |v, cx| v.refresh(cx));
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(v.error.is_none());
            assert_eq!(v.chain.as_ref().unwrap().changes().len(), 1);
        });
    }
}
