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
pub struct Restore(pub stash::Entry, pub plico::stash_operation::Action);
pub struct Acknowledge(pub plico::rebase_saved::Pending);
pub struct ViewLocal;
pub struct DropStash(pub stash::Entry);
impl EventEmitter<DropStash> for StashView {}
impl EventEmitter<Acknowledge> for StashView {}
impl EventEmitter<Restore> for StashView {}
impl EventEmitter<ViewLocal> for StashView {}
impl EventEmitter<OpenDiff> for StashView {}
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
    entries: Vec<stash::Entry>,
    selected: Option<usize>,
    contents: Option<stash::Contents>,
    rows: Vec<Row>,
    selected_file: Option<usize>,
    pending_selection: Option<(String, Option<stash::FileIdentity>)>,
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
    pub fn new(repo: Repository, cx: &mut Context<Self>) -> Self {
        let mut s = Self {
            recovery: None,
            recovery_error: None,
            write_busy: false,
            notice: None,
            diagnostic: None,
            focus: cx.focus_handle(),
            files_scroll: UniformListScrollHandle::new(),
            repo,
            entries: vec![],
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
    pub fn result(&mut self, notice: String, diagnostic: Option<String>, cx: &mut Context<Self>) {
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
            .map(|e| e.oid.clone())
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
        self.selected = None;
        self.more = false;
        self.task = cx.spawn(async move |this, cx| {
            let (result, recovery) = cx
                .background_spawn(async move {
                    (
                        stash::list(&repo, stash::MAX_ENTRIES, &token),
                        plico::rebase_saved::pending(&repo, &token),
                    )
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.generation {
                    return;
                }
                s.busy = false;
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
                match result {
                    Ok(stack) => {
                        s.entries = stack.entries;
                        s.more = stack.more;
                        if !s.entries.is_empty() {
                            let found =
                                preferred.and_then(|id| s.entries.iter().position(|e| e.oid == id));
                            s.select_with_file(found.unwrap_or(0), found.and(preferred_file), cx);
                        } else {
                            s.pending_selection = None;
                        }
                    }
                    Err(e) => s.error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
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
        self.pending_selection = Some((entry.oid.clone(), preferred.clone()));
        let (repo, token, generation) = self.begin();
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
            Target::new(self.repo.clone(), Some(commit.clone()), change.clone()).and_then(
                |target| {
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
                },
            ),
        )
    }
    pub fn select_target(&mut self, target: &Target, cx: &mut Context<Self>) {
        if target.identity().repository != self.repo.path || self.busy {
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
                    Target::new(self.repo.clone(), Some(commit.clone()), change.clone()).ok()
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
                    self.entries.len(),
                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| {
                                let entry = &s.entries[index];
                                let menu_entry = entry.clone();
                                let owner = cx.entity().downgrade();
                                div()
                                    .id(("stash", index))
                                    .h(px(32.))
                                    .w_full()
                                    .px_3()
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
                            })
                            .collect()
                    }),
                )
                .flex_1(),
            )
            .when(
                self.entries.is_empty() && !self.busy && self.error.is_none(),
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
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(ViewLocal))),
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
                                            cx.emit(Acknowledge(pending.clone()));
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
