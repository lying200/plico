//! Rebased VcsPushDialog / PushLog: editable source→target row above commits,
//! changes and optional single-commit details on the right. Load on selection.
use plico::{git::{Cancel, Commit, Details, Repository}, i18n::t, push, push_target::{Snapshot, Target}};
use gpui_kit::{prelude::FluentBuilder as _, component::{Disableable as _, Selectable as _}, *};
use gpui_kit::component::{WindowExt as _, checkbox::Checkbox, button::{Button, ButtonVariants as _}, input::{Input, InputState, Enter, Escape}, menu::{DropdownMenu as _, PopupMenuItem}, h_flex, v_flex};

actions!(push_review, [ShowFileDiff, NextDifference, PreviousDifference]);
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", ShowFileDiff, Some("PushFiles")),
        KeyBinding::new("ctrl-d", ShowFileDiff, Some("PushFiles")),
        KeyBinding::new("f7", NextDifference, Some("PushDiff")),
        KeyBinding::new("shift-f7", PreviousDifference, Some("PushDiff")),
    ]);
}
pub struct PushDialog {
    pub push_tags: bool,
    pub tag_mode: push::TagMode,
    collapsed_directories: std::collections::HashSet<std::path::PathBuf>,
    show_details: bool,
    pub source_callback: Option<crate::diff_tab::SourceCallback>,
    repo: Repository,
    snapshot: Snapshot,
    target: Option<Target>,
    remote: String,
    branch: Entity<InputState>,
    editing: bool,
    explicit_upstream: bool,
    applied_upstream: bool,
    _input_watch: Subscription,
    commits: Vec<Commit>,
    references: Option<std::sync::Arc<str>>,
    offset: usize,
    more: bool,
    capped: bool,
    selected: Option<usize>,
    selected_file: Option<usize>,
    file_focus: FocusHandle,
    details: Option<Details>,
    aggregate: Option<plico::push_preview::Snapshot>,
    files: Vec<plico::combined_changes::Change>,
    busy: bool,
    error: Option<&'static str>,
    generation: u64,
    token: Cancel,
    task: Task<()>,
}
impl Drop for PushDialog { fn drop(&mut self) { self.token.cancel(); } }
impl PushDialog {
    pub fn new(repo: Repository, snapshot: Snapshot, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let target = snapshot.default.clone();
        let remote = target.as_ref().map(|t| t.remote.clone()).unwrap_or_default();
        let value = target.as_ref().map(|t| t.branch.clone()).unwrap_or_default();
        let branch = cx.new(|cx| { let mut s = InputState::new(window, cx); s.set_value(value, window, cx); s });
        let watch = cx.observe(&branch, |_, _, cx| cx.notify());
        let mut s = Self { push_tags: false, tag_mode: push::TagMode::All, collapsed_directories: Default::default(), show_details: true, source_callback: None, repo, snapshot, target, remote, branch, editing: false, explicit_upstream: false, applied_upstream: false, _input_watch: watch,
            commits: Vec::new(), references: None, offset: 0, more: false, capped: false, selected: None, selected_file: None, file_focus: cx.focus_handle(), details: None, aggregate: None, files: Vec::new(),
            busy: false, error: None, generation: 0, token: Cancel::default(), task: Task::ready(()) };
        s.load(0, false, cx); s
    }
    pub fn spec(&self) -> Option<push::Spec> {
        if self.busy || self.editing || self.error.is_some() { return None; }
        let target = self.target.clone()?;
        Some(push::Spec { tags: self.push_tags.then_some(self.tag_mode), force: false, source_branch: self.snapshot.source_branch.clone(), source_oid: self.snapshot.source_oid.clone(),
            set_upstream: target.should_set_upstream(true, self.snapshot.targets.tracked.is_some(), self.applied_upstream), target })
    }
    fn load(&mut self, offset: usize, apply: bool, cx: &mut Context<Self>) {
        self.token.cancel(); self.token = Cancel::default(); self.generation += 1;
        let generation = self.generation; let token = self.token.clone(); let repo = self.repo.clone();
        let target = self.target.clone(); let remote = self.remote.clone(); let branch = self.branch.read(cx).value().to_string();
        let upstream = self.explicit_upstream && plico::push_target::show_upstream_checkbox(self.snapshot.default.as_ref(), &remote, &branch);
        let references = if apply { None } else { self.references.clone() };
        let source = self.snapshot.source_oid.clone();
        self.busy = true; self.error = None; self.aggregate = None; self.files.clear(); self.collapsed_directories.clear(); self.commits.clear(); self.details = None; self.selected = None; self.selected_file = None;
        self.task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                let target = if apply && !target.as_ref().is_some_and(|t| t.remote == remote && t.branch == branch) {
                    repo.custom_push_target(&remote, &branch, &token)?
                } else { target.ok_or("No push target")? };
                let page = repo.outgoing_commits_checked(&source, &target, offset, 100, references.as_deref(), &token)?;
                let aggregate = plico::push_preview::load(&repo, &source, &target, Some(&page.references),
                    plico::git::HISTORY_MAX, &token)?;
                Ok::<_, String>((target, page, aggregate))
            }).await;
            let _ = this.update(cx, |s, cx| {
                if generation != s.generation { return; }
                s.busy = false;
                match result {
                    Ok((target, page, aggregate)) => { if apply { s.applied_upstream = upstream; } s.target = Some(target); s.editing = false; s.offset = offset;
                        s.references = Some(page.references); s.commits = page.commits; s.more = page.more; s.capped = aggregate.capped; s.files = aggregate.files.clone(); s.aggregate = Some(aggregate); }
                    Err(error) => s.error = Some(if error == "Push references changed; reopen the push dialog" {
                        "Push references changed; reopen the push dialog"
                    } else { "Could not load outgoing commits" }),
                }
                cx.notify();
            });
        }); cx.notify();
    }
    fn show_file_diff(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.editing { return; }
        let index = self.selected_file.unwrap_or(0);
        if self.files.is_empty() { return; }
        let (target, chain) = match plico::diff_target::Chain::combined(self.repo.clone(), self.files.clone(), index) {
            Ok(result) => result, Err(_) => { self.error = Some("Could not open file diff"); cx.notify(); return; }
        };
        let viewer = cx.new(|_| { let mut tab=crate::diff_tab::DiffTab::new(target, chain);tab.source_callback=self.source_callback.clone();tab });
        let activate = viewer.clone();
        // GPUI's dialog stack preserves the parent Push window and its selection.
        // The closure is the sole owner; closing releases the viewer and its job.
        window.open_dialog(cx, move |dialog, window, _| {
            let next = viewer.clone(); let previous = viewer.clone();
            let height = (window.viewport_size().height - px(170.)).max(px(240.)).min(px(700.));
            dialog.title(t("Diff")).w(px(1200.)).overlay_closable(false)
                .child(div().key_context("PushDiff").h(height).child(viewer.clone())
                    .on_action(move |_: &NextDifference, w, cx| next.update(cx, |s, cx| s.navigate(true, w, cx)))
                    .on_action(move |_: &PreviousDifference, w, cx| previous.update(cx, |s, cx| s.navigate(false, w, cx))))
        });
        activate.update(cx, |s, cx| s.activate(window, cx));
    }
    fn jump_to_source(&self, window:&mut Window,cx:&mut Context<Self>) {
        let Some(file)=self.files.get(self.selected_file.unwrap_or(0)) else{return;};
        let Some(callback)=self.source_callback.clone() else{return;};
        if let Ok(target)=plico::diff_target::Target::combined(self.repo.clone(),file.clone()) {
            callback((target,file.after.is_none(),1),window,cx);
        }
    }
    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.explicit_upstream = self.applied_upstream;
        if let Some(target) = &self.target {
            self.remote = target.remote.clone();
            let name = target.branch.clone();
            self.branch.update(cx, |s, cx| s.set_value(name, window, cx));
        }
        self.editing = false;
        self.load(self.offset, false, cx);
    }
    fn select_repository(&mut self, cx: &mut Context<Self>) {
        if self.editing || self.aggregate.is_none() { return; }
        self.token.cancel(); self.generation += 1; self.task = Task::ready(());
        self.selected = None; self.selected_file = None; self.details = None; self.collapsed_directories.clear();
        self.files = self.aggregate.as_ref().unwrap().files.clone();
        self.busy = false; self.error = None; cx.notify();
    }
    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.editing || self.busy { return; }
        let Some(commit) = self.commits.get(index).cloned() else { return; };
        self.token.cancel(); self.token = Cancel::default(); self.generation += 1;
        let generation = self.generation; let token = self.token.clone(); let repo = self.repo.clone();
        self.selected = Some(index); self.selected_file = None; self.details = None; self.files.clear(); self.collapsed_directories.clear(); self.error = None; self.busy = true;
        self.task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                let details = repo.details(&commit, &token)?;
                let files = repo.push_commit_changes(&commit, &token)?;
                Ok::<_, String>((details, files))
            }).await;
            let _ = this.update(cx, |s, cx| { if s.generation == generation {
                s.busy = false;
                match result { Ok((details, files)) => { s.details = Some(details); s.files = files; }, Err(_) => s.error = Some("Could not load commit details") }
                cx.notify();
            } });
        }); cx.notify();
    }
}
impl Render for PushDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut header = h_flex().h(px(28.)).items_center().px_2().flex_shrink_0().gap_1()
            .when(self.selected.is_none() && !self.editing, |v| v.bg(rgb(0x2a4371)))
            .on_action(cx.listener(|s, action: &Enter, _, cx| {
                if s.editing {
                    if !s.busy && !action.secondary && !action.shift { s.load(0, true, cx); }
                    cx.stop_propagation();
                } else { cx.propagate(); }
            }))
            .on_action(cx.listener(|s, _: &Escape, window, cx| {
                if s.editing { s.cancel_edit(window, cx); cx.stop_propagation(); }
                else { cx.propagate(); }
            }))
            .child(self.snapshot.source_branch.clone()).child("→");
        if self.editing {
            let owner = cx.entity().downgrade(); let remotes = self.snapshot.targets.remotes.clone();
            header = header.child(Button::new("push-remote").disabled(self.busy).label(self.remote.clone()).dropdown_menu(move |mut menu, _, _| {
                for remote in &remotes { let owner = owner.clone(); let remote = remote.clone();
                    menu = menu.item(PopupMenuItem::new(remote.clone()).on_click(move |_, _, cx| { let _ = owner.update(cx, |s, cx| { s.remote = remote.clone(); cx.notify(); }); }));
                } menu
            })).child(div().flex_1().child(Input::new(&self.branch).disabled(self.busy)))
                .child(Button::new("apply-target").label(t("Apply")).disabled(self.busy).on_click(cx.listener(|s, _, _, cx| s.load(0, true, cx))))
                .child(Button::new("cancel-target").label(t("Cancel")).on_click(cx.listener(|s, _, w, cx| s.cancel_edit(w, cx))));
        } else {
            let label = self.target.as_ref().map(|t| format!("{} : {}", t.remote, t.branch)).unwrap_or_else(|| t("Select remote").into());
            header = header.child(Button::new("edit-target").link().h(px(24.)).px_0().disabled(self.busy).label(label).on_click(cx.listener(|s, _, w, cx| {
                s.editing = true; s.commits.clear(); s.files.clear(); s.aggregate = None; s.details = None; s.selected = None; s.branch.read(cx).focus_handle(cx).focus(w, cx); cx.notify();
            })));
        }
        let mut left = v_flex().w_1_2().h_full().min_w_0().child(
            div().id("push-repository").child(header)
                .on_mouse_down(MouseButton::Left, cx.listener(|s, _, _, cx| { if !s.editing { s.select_repository(cx); } })));
        let show_upstream = self.editing && plico::push_target::show_upstream_checkbox(
            self.snapshot.default.as_ref(), &self.remote, self.branch.read(cx).value().as_ref());
        if show_upstream {
            left = left.child(Checkbox::new("push-set-upstream").label(t("Set upstream")).checked(self.explicit_upstream).disabled(self.busy)
                .on_click(cx.listener(|s, value, _, cx| { s.explicit_upstream = *value; cx.notify(); })));
        }
        if !self.editing && self.applied_upstream { left = left.child(t("New upstream")); }
        if self.target.as_ref().is_some_and(|target| target.new_branch) { left = left.child(t("New branch")); }
        let rows = self.commits.iter().enumerate().map(|(i, commit)| {
            div().id(("outgoing", i)).h(px(24.)).pl(px(24.)).pr_2().truncate()
                .when(self.selected == Some(i), |v| v.bg(rgb(0x2a4371)))
                .child(commit.subject.clone()).on_mouse_down(MouseButton::Left, cx.listener(move |s, _, _, cx| s.select(i, cx)))
        }).collect::<Vec<_>>();
        left = left.child(div().id("outgoing-scroll").flex_1().min_h_0().overflow_y_scroll().children(rows));
        left = left.when(self.offset > 0 || self.more, |left| left.child(h_flex().flex_shrink_0().gap_2()
            .child(Button::new("outgoing-prev").label(t("Previous")).disabled(self.busy || self.editing || self.offset == 0).on_click(cx.listener(|s, _, _, cx| s.load(s.offset.saturating_sub(100), false, cx))))
            .child(Button::new("outgoing-next").label(t("Next")).disabled(self.busy || self.editing || !self.more).on_click(cx.listener(|s, _, _, cx| s.load(s.offset+100, false, cx))))));
        let mut right = v_flex().flex_1().min_w_0().h_full().border_l_1().border_color(rgb(0x393b40));
        if !self.busy && !self.editing && self.aggregate.is_some() {
            let file_rows = plico::file_tree::rows(self.files.iter().enumerate().filter_map(|(i, f)| f.after.as_ref().or(f.before.as_ref()).map(|r| (i, r.path.as_path()))),
                |path| self.collapsed_directories.contains(path));
            right = right.child(h_flex().h(px(28.)).items_center().px_1().gap_1().flex_shrink_0()
                .child(Button::new("push-show-diff").ghost().size(px(24.)).icon(gpui_kit::component::Icon::default().path("jb/diff_dark.svg").size(px(16.)))
                    .tooltip(t("Show Diff")).disabled(self.files.is_empty()).on_click(cx.listener(|s, _, w, cx| s.show_file_diff(w, cx))))
                .child(Button::new("push-source").ghost().size(px(24.)).icon(gpui_kit::component::Icon::default().path("jb/edit_dark.svg").size(px(16.)))
                    .tooltip(t("Jump to Source · F4")).disabled(self.files.is_empty() || self.source_callback.is_none())
                    .on_click(cx.listener(|s,_,w,cx|s.jump_to_source(w,cx))))
                .child(Button::new("push-show-details").ghost().size(px(24.)).selected(self.show_details)
                    .icon(gpui_kit::component::Icon::default().path("jb/history-show_dark.svg").size(px(16.)))
                    .tooltip(t("Commit details")).on_click(cx.listener(|s, _, _, cx| { s.show_details = !s.show_details; cx.notify(); })))
                .child(div().flex_1())
                .child(Button::new("push-expand-files").ghost().size(px(24.)).icon(gpui_kit::component::Icon::default().path("jb/expandAll_dark.svg").size(px(16.)))
                    .tooltip(t("Expand All")).on_click(cx.listener(|s, _, _, cx| { s.collapsed_directories.clear(); cx.notify(); })))
                .child(Button::new("push-collapse-files").ghost().size(px(24.)).icon(gpui_kit::component::Icon::default().path("jb/collapseAll_dark.svg").size(px(16.)))
                    .tooltip(t("Collapse All")).on_click(cx.listener(|s, _, _, cx| {
                        s.collapsed_directories = plico::file_tree::rows(s.files.iter().enumerate().filter_map(|(i,f)|f.after.as_ref().or(f.before.as_ref()).map(|r|(i,r.path.as_path()))), |_| false)
                            .into_iter().filter_map(|row|if let plico::file_tree::Row::Folder{path,..}=row{Some(path)}else{None}).collect();cx.notify();
                    }))))
                .child(div().id("push-files").key_context("PushFiles").track_focus(&self.file_focus)
                    .on_action(cx.listener(|s, _: &ShowFileDiff, w, cx| s.show_file_diff(w, cx)))
                    .flex_1().min_h_0().overflow_y_scroll()
                    .when(self.files.is_empty(),|v|v.child(div().size_full().flex().items_center().justify_center().text_color(rgb(0x6f737a))
                        .child(t(if self.commits.is_empty(){"No commits selected"}else{"No changes"}))))
                    .children(file_rows.into_iter().map(|row| match row {
                        plico::file_tree::Row::Folder { path, depth, count, expanded } => {
                            let name=path.file_name().unwrap_or_default().to_string_lossy().to_string();
                            h_flex().id(SharedString::from(format!("push-folder:{}",path.display()))).h(px(24.)).items_center().gap_1().pl(px(6.+depth as f32*16.))
                                .child(gpui_kit::component::Icon::default().path(if expanded{"jb/chevronDown_dark.svg"}else{"jb/chevronRight_dark.svg"}).size(px(12.)))
                                .child(gpui_kit::component::Icon::default().path("jb/folder_dark_4x.svg").size(px(16.)))
                                .child(name).child(div().text_color(rgb(0x6f737a)).child(format!("{count} {}",t("files"))))
                                .on_mouse_down(MouseButton::Left,cx.listener(move|s,_,_,cx|{if !s.collapsed_directories.remove(&path){s.collapsed_directories.insert(path.clone());}cx.notify();})).into_any_element()
                        }
                        plico::file_tree::Row::File { index:i, depth } => {
                            let file=&self.files[i];let path=&file.after.as_ref().or(file.before.as_ref()).unwrap().path;
                            let color=if file.before.is_none(){0x6aab73}else if file.after.is_none(){0xcf8e6d}else{0x6897bb};
                            h_flex().id(("push-file",i)).h(px(24.)).items_center().gap_1().pl(px(22.+depth as f32*16.)).pr_2()
                                .when(self.selected_file==Some(i),|v|v.bg(rgb(0x33353b)))
                                .child(gpui_kit::component::Icon::default().path(crate::ui::file_icon(path)).size(px(16.)))
                                .child(div().truncate().text_color(rgb(color)).child(path.file_name().unwrap_or_default().to_string_lossy().to_string()))
                                .on_mouse_down(MouseButton::Left,cx.listener(move|s,event:&MouseDownEvent,w,cx|{s.selected_file=Some(i);s.file_focus.focus(w,cx);if event.click_count==2{s.show_file_diff(w,cx);}cx.notify();})).into_any_element()
                        }
                    })));
            if let Some(details) = self.details.as_ref().filter(|_| self.show_details) { right = right.child(div().id("push-details").h(px(130.)).overflow_y_scroll().border_t_1().border_color(rgb(0x393b40))
                    .child(details.message.clone()).child(div().child(details.author.clone()))); }
        }
        v_flex().h_full().min_h_0().child(h_flex().flex_1().min_h_0().child(left).child(right))
            .when(self.busy, |v| v.child(t("Loading outgoing commits…")))
            .when(!self.busy && !self.editing && self.commits.is_empty() && self.error.is_none(), |v| v.child(div().px_2().py_1().text_color(rgb(0x6f737a)).child(t("No outgoing commits"))))
            .when(self.capped, |v| v.child(t("New branch preview is limited to 1,000 commits")))
            .when_some(self.error, |v, error| v.child(div().text_color(rgb(0xf2a6a6)).child(t(error))))
    }
}
