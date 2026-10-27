//! SavedPatchesUi: stash list beside grouped changes, opening editor Diff tabs.
use gpui_kit::component::{
    Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{ContextMenuExt as _, PopupMenuItem},
    resizable::{resizable_panel, v_resizable},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    diff_target::{Chain, Target},
    git::{Cancel, Repository},
    i18n::t,
    stash,
    stash::project::SavedEntry,
};

actions!(stash_review, [ShowDiff, NextFile, PreviousFile]);
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", ShowDiff, Some("StashFiles")),
        KeyBinding::new("ctrl-d", ShowDiff, Some("StashFiles")),
        KeyBinding::new("down", NextFile, Some("StashFiles")),
        KeyBinding::new("up", PreviousFile, Some("StashFiles")),
    ]);
}
pub struct OpenDiff(pub Target, pub Chain, pub crate::diff_tab::StashOrigin);
pub struct Restore(pub SavedEntry, pub plico::stash_operation::Action);
pub struct Acknowledge(pub Repository, pub plico::rebase_saved::Pending);
pub struct ViewLocal(pub Repository);
pub struct DropStash(pub SavedEntry);
impl EventEmitter<DropStash> for StashView {}
impl EventEmitter<Acknowledge> for StashView {}
impl EventEmitter<Restore> for StashView {}
impl EventEmitter<ViewLocal> for StashView {}
impl EventEmitter<OpenDiff> for StashView {}
#[derive(Clone)]
enum ListRow {
    Repository(std::path::PathBuf, usize),
    Entry(usize),
    Error(std::path::PathBuf, String),
}
#[derive(Clone)]
enum Row {
    Group(&'static str),
    File { group: usize, index: usize },
}
pub struct StashView {
    recovery: Option<plico::rebase_saved::Pending>,
    recovery_error: Option<String>,
    write_busy: bool,
    notice: Option<String>,
    diagnostic: Option<String>,
    focus: FocusHandle,
    files_scroll: UniformListScrollHandle,
    repo: Repository,
    project_roots: Entity<crate::ui::project_roots::ProjectRoots>,
    _roots_subscription: Subscription,
    entries: Vec<SavedEntry>,
    list_rows: Vec<ListRow>,
    list_errors: Vec<(std::path::PathBuf, String)>,
    grouped: bool,
    collapsed: std::collections::BTreeSet<std::path::PathBuf>,
    notice_repo: Option<Repository>,
    selected: Option<usize>,
    contents: Option<stash::Contents>,
    rows: Vec<Row>,
    selected_file: Option<usize>,
    pending_selection: Option<((std::path::PathBuf, String), Option<stash::FileIdentity>)>,
    preview_chains: Vec<Option<Chain>>,
    busy: bool,
    more: bool,
    error: Option<String>,
    token: Cancel,
    generation: u64,
    task: Task<()>,
}
impl Drop for StashView {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl StashView {
    pub fn new(
        repo: Repository,
        project_roots: Entity<crate::ui::project_roots::ProjectRoots>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(
            &project_roots,
            |s, _, _: &crate::ui::project_roots::Changed, cx| s.refresh(cx),
        );
        let mut s = Self {
            recovery: None,
            recovery_error: None,
            write_busy: false,
            notice: None,
            diagnostic: None,
            focus: cx.focus_handle(),
            files_scroll: UniformListScrollHandle::new(),
            repo,
            project_roots,
            _roots_subscription: subscription,
            entries: vec![],
            list_rows: vec![],
            list_errors: vec![],
            grouped: false,
            collapsed: Default::default(),
            notice_repo: None,
            selected: None,
            contents: None,
            rows: vec![],
            selected_file: None,
            pending_selection: None,
            preview_chains: Vec::new(),
            busy: false,
            more: false,
            error: None,
            token: Cancel::default(),
            generation: 0,
            task: Task::ready(()),
        };
        s.refresh(cx);
        s
    }
    pub fn focus_files(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }
    pub fn set_write_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        if self.write_busy != busy {
            self.write_busy = busy;
            cx.notify();
        }
    }
    pub fn result(
        &mut self,
        repository: Repository,
        notice: String,
        diagnostic: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.notice_repo = Some(repository);
        self.notice = Some(notice);
        self.diagnostic = diagnostic;
        self.refresh(cx);
    }
    fn restore(&mut self, action: plico::stash_operation::Action, cx: &mut Context<Self>) {
        if self.write_busy || self.busy {
            return;
        }
        if let Some(entry) = self.selected.and_then(|i| self.entries.get(i)).cloned() {
            self.notice = None;
            self.diagnostic = None;
            cx.emit(Restore(entry, action));
        }
    }
    fn begin(&mut self) -> (Repository, Cancel, u64) {
        self.token.cancel();
        self.token = Cancel::default();
        self.generation += 1;
        self.preview_chains.clear();
        self.busy = true;
        self.error = None;
        self.contents = None;
        self.rows.clear();
        self.selected_file = None;
        (self.repo.clone(), self.token.clone(), self.generation)
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let current = self.selected.and_then(|i| self.entries.get(i));
        let preferred = current
            .map(SavedEntry::identity)
            .or_else(|| self.pending_selection.as_ref().map(|(id, _)| id.clone()));
        let preferred_file = self.current_file_identity().or_else(|| {
            self.pending_selection
                .as_ref()
                .filter(|(id, _)| Some(id) == preferred.as_ref())
                .and_then(|(_, file)| file.clone())
        });
        self.pending_selection = preferred.clone().map(|id| (id, preferred_file.clone()));
        let (repo, token, generation) = self.begin();
        self.entries.clear();
        self.list_rows.clear();
        self.list_errors.clear();
        self.selected = None;
        self.more = false;
        let Some(roots) = self.project_roots.read(cx).snapshot() else {
            cx.notify();
            return;
        };
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    roots.and_then(|roots| stash::project::load_in(&repo, &roots, &token))
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.generation {
                    return;
                }
                s.busy = false;
                match result {
                    Ok(stash::project::Stack {
                        entries,
                        more,
                        grouped,
                        errors,
                        recovery,
                    }) => {
                        s.entries = entries;
                        s.more = more;
                        s.grouped = grouped;
                        s.list_errors = errors;
                        s.rebuild_list_rows();
                        match recovery {
                            Ok(value) => {
                                s.recovery = value;
                                s.recovery_error = None;
                            }
                            Err(error) => {
                                s.recovery = None;
                                s.recovery_error = Some(error);
                            }
                        }
                        if !s.entries.is_empty() {
                            let found = preferred
                                .and_then(|id| s.entries.iter().position(|e| e.identity() == id));
                            s.select_with_file(found.unwrap_or(0), found.and(preferred_file), cx);
                        } else {
                            s.pending_selection = None;
                        }
                    }
                    Err(error) => s.error = Some(error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn rebuild_list_rows(&mut self) {
        self.list_rows.clear();
        let mut index = 0;
        while index < self.entries.len() {
            let path = self.entries[index].repository.path.clone();
            let end = index
                + self.entries[index..]
                    .iter()
                    .take_while(|e| e.repository.path == path)
                    .count();
            if self.grouped {
                self.list_rows
                    .push(ListRow::Repository(path.clone(), end - index));
            }
            if !self.grouped || !self.collapsed.contains(&path) {
                self.list_rows.extend((index..end).map(ListRow::Entry));
            }
            index = end;
        }
        self.list_rows.extend(
            self.list_errors
                .iter()
                .map(|(path, error)| ListRow::Error(path.clone(), error.clone())),
        );
    }
    fn selected_repository(&self) -> &Repository {
        self.selected
            .and_then(|i| self.entries.get(i))
            .map(|e| &e.repository)
            .unwrap_or(&self.repo)
    }
    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.selected == Some(index) && self.contents.is_some() {
            return;
        }
        self.select_with_file(index, None, cx);
    }
    fn current_file_identity(&self) -> Option<stash::FileIdentity> {
        let Row::File { group, index } = self.rows.get(self.selected_file?)? else {
            return None;
        };
        self.contents.as_ref()?.file_identity(*group, *index)
    }
    fn select_with_file(
        &mut self,
        index: usize,
        preferred: Option<stash::FileIdentity>,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.entries.get(index).cloned() else {
            return;
        };
        // An explicit new stash selection replaces an older refresh preference.
        self.pending_selection = Some((entry.identity(), preferred.clone()));
        let (_, token, generation) = self.begin();
        let repo = entry.repository.clone();
        self.selected = Some(index);
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { stash::contents(&repo, &entry, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.generation {
                    return;
                }
                s.busy = false;
                match result {
                    Ok(contents) => {
                        for (group, details) in std::iter::once(&contents.working)
                            .chain(contents.parents.iter().map(|(_, d)| d))
                            .enumerate()
                        {
                            s.rows.push(Row::Group(match group {
                                0 => "Working tree",
                                1 => "Index",
                                _ => "Untracked files",
                            }));
                            s.rows.extend(
                                (0..details.files.len()).map(|index| Row::File { group, index }),
                            );
                        }
                        s.selected_file = preferred
                            .as_ref()
                            .and_then(|wanted| {
                                s.rows.iter().position(|r| {
                                    let Row::File { group, index } = r else {
                                        return false;
                                    };
                                    contents.file_identity(*group, *index).as_ref() == Some(wanted)
                                })
                            })
                            .or_else(|| s.rows.iter().position(|r| matches!(r, Row::File { .. })));
                        s.preview_chains = vec![None; 1 + contents.parents.len()];
                        s.contents = Some(contents);
                        s.pending_selection = None;
                    }
                    Err(e) => s.error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn move_file(&mut self, forward: bool, cx: &mut Context<Self>) {
        let current = self.selected_file.unwrap_or(0);
        self.selected_file = if forward {
            ((current + 1)..self.rows.len()).find(|i| matches!(self.rows[*i], Row::File { .. }))
        } else {
            (0..current)
                .rev()
                .find(|i| matches!(self.rows[*i], Row::File { .. }))
        }
        .or(self.selected_file);
        cx.notify();
    }
    pub fn preview_loading(&self) -> bool {
        self.busy
    }
    pub fn preview_target(&mut self) -> Option<Result<OpenDiff, String>> {
        let Row::File { group, index } = self.rows.get(self.selected_file?)? else {
            return None;
        };
        let contents = self.contents.as_ref()?;
        let (commit, details) = if *group == 0 {
            (&contents.commit, &contents.working)
        } else {
            let (c, d) = contents.parents.get(group - 1)?;
            (c, d)
        };
        let change = details.files.get(*index)?;
        let entry = self.entries.get(self.selected?)?;
        let origin = crate::diff_tab::StashOrigin {
            selector: entry.selector.clone(),
            group: match group {
                0 => "Working tree",
                1 => "Index",
                _ => "Untracked files",
            },
        };
        Some(
            Target::new(
                entry.repository.clone(),
                Some(commit.clone()),
                change.clone(),
            )
            .and_then(|target| {
                let cached = self
                    .preview_chains
                    .get_mut(*group)
                    .ok_or("Missing stash comparison group")?;
                let chain = match cached.as_ref().and_then(|chain| chain.fork_for(&target)) {
                    Some(chain) => chain,
                    None => {
                        let chain = Chain::new(&target, &details.files)?;
                        *cached = Some(chain.clone());
                        chain
                    }
                };
                Ok(OpenDiff(target, chain, origin))
            }),
        )
    }
    pub fn select_target(&mut self, target: &Target, cx: &mut Context<Self>) {
        if target.identity().repository != self.selected_repository().path || self.busy {
            return;
        }
        let Some(contents) = &self.contents else {
            return;
        };
        let row = self.rows.iter().position(|row| {
            let Row::File { group, index } = row else {
                return false;
            };
            let (commit, details) = if *group == 0 {
                (&contents.commit, &contents.working)
            } else {
                let Some(pair) = contents.parents.get(*group - 1) else {
                    return false;
                };
                (&pair.0, &pair.1)
            };
            details
                .files
                .get(*index)
                .and_then(|change| {
                    Target::new(
                        self.selected_repository().clone(),
                        Some(commit.clone()),
                        change.clone(),
                    )
                    .ok()
                })
                .is_some_and(|candidate| candidate.identity() == target.identity())
        });
        if let Some(row) = row {
            self.selected_file = Some(row);
            self.files_scroll.scroll_to_item(row, ScrollStrategy::Top);
            cx.notify();
        }
    }
    fn show_diff(&mut self, cx: &mut Context<Self>) {
        match self.preview_target() {
            Some(Ok(event)) => cx.emit(event),
            Some(Err(error)) => {
                self.error = Some(error);
                cx.notify();
            }
            None => {}
        }
    }
}
impl Render for StashView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let left = v_flex()
            .size_full()
            .min_h_0()
            .child(
                h_flex()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px_2()
                    .child(
                        Button::new("stash-refresh")
                            .ghost()
                            .small()
                            .icon(crate::ui_style::jb_icon("refresh_dark.svg"))
                            .tooltip(t("Refresh"))
                            .disabled(self.write_busy)
                            .on_click(cx.listener(|s, _, _, cx| s.refresh(cx))),
                    )
                    .when(self.busy, |v| {
                        v.child(div().flex_1().truncate().child(t("Loading saved changes…")))
                    }),
            )
            .when_some(
                self.error.clone().filter(|_| self.entries.is_empty()),
                |v, error| {
                    v.child(crate::error_ui::notice(
                        "stash-list-error",
                        "Could not load saved changes",
                        error,
                        self.busy || self.write_busy,
                        cx.listener(|s, _, _, cx| s.refresh(cx)),
                    ))
                },
            )
            .child(
                uniform_list(
                    "stash-entries",
                    self.list_rows.len(),
                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|row| {
                                let index = match &s.list_rows[row] {
                                    ListRow::Repository(path, count) => {
                                        let path = path.clone();
                                        let closed = s.collapsed.contains(&path);
                                        return h_flex()
                                            .id(("stash-root", row))
                                            .h(px(32.))
                                            .px_3()
                                            .gap_2()
                                            .child(
                                                crate::ui_style::jb_icon(if closed {
                                                    "chevronRight_dark.svg"
                                                } else {
                                                    "chevronDown_dark.svg"
                                                })
                                                .size(px(12.)),
                                            )
                                            .child(
                                                crate::ui_style::jb_icon("folder_dark.svg")
                                                    .size(px(16.)),
                                            )
                                            .child(
                                                div().truncate().child(
                                                    path.file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy()
                                                        .into_owned(),
                                                ),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x8b8e95))
                                                    .child(count.to_string()),
                                            )
                                            .on_click(cx.listener(move |s, _, _, cx| {
                                                if !s.collapsed.remove(&path) {
                                                    s.collapsed.insert(path.clone());
                                                }
                                                s.rebuild_list_rows();
                                                cx.notify();
                                            }))
                                            .into_any_element();
                                    }
                                    ListRow::Error(path, error) => {
                                        return div()
                                            .h(px(32.))
                                            .px_3()
                                            .truncate()
                                            .text_color(rgb(0xf2a6a6))
                                            .child(format!("{}: {}", path.display(), error))
                                            .into_any_element();
                                    }
                                    ListRow::Entry(index) => *index,
                                };
                                let entry = &s.entries[index];
                                let menu_entry = entry.clone();
                                let owner = cx.entity().downgrade();
                                div()
                                    .id(("stash", index))
                                    .h(px(32.))
                                    .w_full()
                                    .pl(px(if s.grouped { 36. } else { 12. }))
                                    .pr_3()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .rounded(px(4.))
                                    .when(s.selected == Some(index), |d| d.bg(rgb(0x2e436e)))
                                    .child(
                                        div()
                                            .text_color(rgb(0x8b8e95))
                                            .child(entry.selector.clone()),
                                    )
                                    .child(div().flex_1().truncate().child(entry.message.clone()))
                                    .on_click(cx.listener(move |s, _, _, cx| s.select(index, cx)))
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |s, _, _, cx| s.select(index, cx)),
                                    )
                                    .context_menu(move |mut menu, _, cx| {
                                        let disabled =
                                            owner.upgrade().is_none_or(|s| s.read(cx).write_busy);
                                        for (label, action) in [
                                            ("Pop", plico::stash_operation::Action::Pop),
                                            ("Apply", plico::stash_operation::Action::Apply),
                                        ] {
                                            let owner = owner.clone();
                                            let entry = menu_entry.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(t(label))
                                                    .disabled(disabled)
                                                    .on_click(move |_, _, cx| {
                                                        let _ = owner.update(cx, |s, cx| {
                                                            if !s.write_busy {
                                                                cx.emit(Restore(
                                                                    entry.clone(),
                                                                    action,
                                                                ));
                                                            }
                                                        });
                                                    }),
                                            );
                                        }
                                        let owner = owner.clone();
                                        let entry = menu_entry.clone();
                                        menu.separator().item(
                                            PopupMenuItem::new(t("Drop Stash…"))
                                                .disabled(disabled)
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        if !s.write_busy {
                                                            cx.emit(DropStash(entry.clone()));
                                                        }
                                                    });
                                                }),
                                        )
                                    })
                                    .into_any_element()
                            })
                            .collect()
                    }),
                )
                .flex_1(),
            )
            .when(
                self.entries.is_empty()
                    && self.list_errors.is_empty()
                    && !self.busy
                    && self.error.is_none(),
                |v| v.child(div().p_3().child(t("No stashes"))),
            )
            .when(self.more, |v| {
                v.child(
                    div()
                        .p_3()
                        .child(t("Only the first 1000 stashes are shown")),
                )
            });
        let right = v_flex()
            .size_full()
            .min_h_0()
            .key_context("StashFiles")
            .track_focus(&self.focus)
            .on_action(cx.listener(|s, _: &ShowDiff, _, cx| s.show_diff(cx)))
            .on_action(cx.listener(|s, _: &NextFile, _, cx| s.move_file(true, cx)))
            .on_action(cx.listener(|s, _: &PreviousFile, _, cx| s.move_file(false, cx)))
            .child(
                h_flex()
                    .h(px(36.))
                    .px_2()
                    .border_b_1()
                    .border_color(rgb(0x2b2d30))
                    .child(
                        Button::new("stash-show-diff")
                            .ghost()
                            .small()
                            .label(t("Show Diff"))
                            .disabled(self.selected_file.is_none() || self.busy)
                            .on_click(cx.listener(|s, _, _, cx| s.show_diff(cx))),
                    ),
            )
            .when_some(
                self.error.clone().filter(|_| !self.entries.is_empty()),
                |v, error| {
                    v.child(crate::error_ui::notice(
                        "stash-content-error",
                        "Could not load saved changes",
                        error,
                        self.busy || self.write_busy,
                        cx.listener(|s, _, _, cx| s.refresh(cx)),
                    ))
                },
            )
            .child(
                uniform_list(
                    "stash-files",
                    self.rows.len(),
                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|row| {
                                let content = match s.rows[row] {
                                    Row::Group(label) => div()
                                        .h(px(28.))
                                        .px_3()
                                        .flex()
                                        .items_center()
                                        .text_color(rgb(0x8b8e95))
                                        .child(t(label))
                                        .into_any_element(),
                                    Row::File { group, index } => {
                                        let c = s.contents.as_ref().unwrap();
                                        let details = if group == 0 {
                                            &c.working
                                        } else {
                                            &c.parents[group - 1].1
                                        };
                                        let file = &details.files[index];
                                        div()
                                            .id(("stash-file", row))
                                            .h(px(28.))
                                            .w_full()
                                            .px_4()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .rounded(px(4.))
                                            .when(s.selected_file == Some(row), |v| {
                                                v.bg(rgb(0x2e436e))
                                            })
                                            .child(
                                                img(crate::ui_style::file_icon(&file.path))
                                                    .size(px(16.))
                                                    .flex_shrink_0(),
                                            )
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_color(rgb(crate::ui_style::change_color(
                                                        file,
                                                    )))
                                                    .child(file.label()),
                                            )
                                            .on_click(cx.listener(
                                                move |s, event: &ClickEvent, w, cx| {
                                                    s.focus.focus(w, cx);
                                                    s.selected_file = Some(row);
                                                    if event.click_count() == 2 {
                                                        s.show_diff(cx);
                                                    }
                                                    cx.notify();
                                                },
                                            ))
                                            .into_any_element()
                                    }
                                };
                                content
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.files_scroll)
                .flex_1(),
            )
            .when_some(self.notice.clone(), |v, notice| {
                v.child(div().px_3().py_2().child(notice))
            })
            .when_some(self.diagnostic.clone(), |v, diagnostic| {
                v.child(
                    h_flex()
                        .px_3()
                        .py_1()
                        .child(crate::error_ui::details_button(
                            "stash-operation",
                            diagnostic,
                        )),
                )
            })
            .child(
                h_flex()
                    .gap_2()
                    .p_2()
                    .border_t_1()
                    .border_color(rgb(0x2b2d30))
                    .child(
                        Button::new("stash-apply")
                            .primary()
                            .label(t("Apply"))
                            .disabled(self.busy || self.write_busy || self.selected.is_none())
                            .tooltip(t("Restore changes and keep the stash"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.restore(plico::stash_operation::Action::Apply, cx)
                            })),
                    )
                    .child(
                        Button::new("stash-pop")
                            .label(t("Pop"))
                            .disabled(self.busy || self.write_busy || self.selected.is_none())
                            .tooltip(t("Restore changes and remove the stash on success"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.restore(plico::stash_operation::Action::Pop, cx)
                            })),
                    )
                    .when(self.notice.is_some(), |v| {
                        v.child(
                            Button::new("stash-view-local")
                                .ghost()
                                .label(t("Local Changes"))
                                .on_click(cx.listener(|s, _, _, cx| {
                                    cx.emit(ViewLocal(
                                        s.notice_repo
                                            .as_ref()
                                            .unwrap_or(s.selected_repository())
                                            .clone(),
                                    ))
                                })),
                        )
                    }),
            );
        v_flex()
            .size_full()
            .min_h_0()
            .bg(rgb(0x191a1c))
            .when_some(self.recovery.clone(), |v, pending| {
                v.child(
                    v_flex()
                        .px_3()
                        .py_2()
                        .gap_2()
                        .child(div().child(t("Saved changes from Rebase still need review.")))
                        .child(
                            h_flex().child(
                                Button::new("stash-acknowledge")
                                    .ghost()
                                    .small()
                                    .label(t("Already handled externally…"))
                                    .disabled(self.busy || self.write_busy)
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        if !s.busy && !s.write_busy {
                                            cx.emit(Acknowledge(s.repo.clone(), pending.clone()));
                                        }
                                    })),
                            ),
                        ),
                )
            })
            .when_some(
                self.recovery_error
                    .clone()
                    .filter(|error| self.error.as_ref() != Some(error)),
                |v, error| {
                    v.child(crate::error_ui::notice(
                        "stash-recovery-error",
                        "Could not inspect the Rebase recovery record.",
                        error,
                        self.busy || self.write_busy,
                        cx.listener(|s, _, _, cx| s.refresh(cx)),
                    ))
                },
            )
            .child(
                div().flex_1().min_h_0().child(
                    v_resizable("stash-split")
                        .child(
                            resizable_panel()
                                .size(px(150.))
                                .size_range(px(60.)..px(1000.))
                                .child(left),
                        )
                        .child(resizable_panel().child(right)),
                ),
            )
    }
}
