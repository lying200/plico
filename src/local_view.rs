//! Independent workspace tool window; opening a comparison does not select a Log row.
use plico::{git::{Cancel, Repository}, local_changes::{Model, Row}, file_tree, diff_target::{Target, Chain}, i18n::t, commit_inclusion::{Identity as InclusionIdentity}, commit_changes::File as CommitFile, commit_draft::{Draft, Amend}, commit_operation, commit_recovery};
use plico::commit_feedback::{self, Failure as WriteFailure};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{button::{Button, ButtonVariants as _}, checkbox::Checkbox, input::{Textarea, TextareaState, InputEvent}, Disableable as _, Sizable as _, menu::{ContextMenuExt as _, PopupMenuItem}, Icon, h_flex, v_flex};
mod rollback;
mod options;
mod status;
pub use rollback::{RequestRollback, ExecuteRollback};
actions!(local_review, [ShowDiff, NextFile, PreviousFile, Refresh, Rollback, ToggleDirectory]);
pub struct OpenDiff(pub Target, pub Chain);
/// Explicit tree navigation, separate from refresh/draft notifications.
pub struct PreviewSelected;
pub struct Submit(pub bool, pub bool);
pub struct Changed { pub push: bool }
pub struct LocalView {
    ui_chinese: bool,
    repo: Repository, model: Model, focus: FocusHandle, scroll: UniformListScrollHandle,
    pub scope: Entity<crate::commit_scope::Scope>, draft: Draft, input: Entity<TextareaState>, head: Option<String>, recovery: bool,
    show_ignored: bool,
    preparing: bool, rolling_back: bool, rollback_scope: Option<(plico::git::Area, std::collections::HashSet<std::path::PathBuf>)>,
    write_token: Option<Cancel>, write_task: Task<()>, amend_token: Cancel, amend_task: Task<()>, notice: Option<String>, write_error: Option<WriteFailure>,
    refresh: Entity<crate::changes_refresh::ChangesRefresh>, error: Option<String>,
}
impl EventEmitter<OpenDiff> for LocalView {}
impl EventEmitter<PreviewSelected> for LocalView {}
impl EventEmitter<Submit> for LocalView {}
impl EventEmitter<Changed> for LocalView {}
impl Drop for LocalView { fn drop(&mut self) { self.amend_token.cancel(); if let Some(t) = &self.write_token { t.cancel(); } } }
impl LocalView {
    pub fn new(repo: Repository, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let refresh = cx.new(|_| crate::changes_refresh::ChangesRefresh::new(repo.clone()));
        Self::with_refresh(repo, refresh, window, cx)
    }
    fn with_refresh(repo: Repository, refresh: Entity<crate::changes_refresh::ChangesRefresh>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new("enter",ShowDiff,Some("LocalChanges")), KeyBinding::new("ctrl-d",ShowDiff,Some("LocalChanges")),
            KeyBinding::new("down",NextFile,Some("LocalChanges")),KeyBinding::new("up",PreviousFile,Some("LocalChanges")),
            KeyBinding::new("ctrl-f5",Refresh,Some("LocalChanges")),KeyBinding::new("ctrl-alt-z",Rollback,Some("LocalChanges")),KeyBinding::new("ctrl-alt-p",ToggleDirectory,Some("LocalChanges"))]);
        let input = cx.new(|cx| TextareaState::new(window, cx).placeholder(t("Commit message")));
        cx.subscribe_in(&input, window, |s, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                if let Err(error) = s.draft.edit(s.input.read(cx).value().to_string()) { s.notice = Some(error); }
                cx.notify();
            }
        }).detach();
        cx.observe(&refresh, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&refresh, |s, _, event, cx| {
            use crate::changes_refresh::Update;
            match event {
                Update::Started => s.error = None,
                Update::Loaded(snapshot, recovery, ignored) => {
                    s.scope.update(cx,|scope,cx|scope.replace(snapshot,cx));
                    s.model.replace(snapshot.files.iter().map(CommitFile::change).chain(ignored.iter().filter(|_|s.show_ignored).cloned()).collect());
                    s.head = snapshot.head.clone();
                    s.recovery = *recovery;
                }
                Update::Failed(error) => s.error = Some(error.clone()),
            }
            cx.notify();
        }).detach();
        let scope=cx.new(|_|crate::commit_scope::Scope::default());
        cx.observe(&scope,|_,_,cx|cx.notify()).detach();
        let mut view = Self { ui_chinese: plico::i18n::is_chinese(), scope, draft: Draft::default(), input, head: None, recovery: false,
            show_ignored: false, preparing: false, rolling_back: false, rollback_scope: None, write_token: None, write_task: Task::ready(()), amend_token: Cancel::default(), amend_task: Task::ready(()), notice: None, write_error: None, repo, model: Model::default(), focus: cx.focus_handle(), scroll: UniformListScrollHandle::new(),
            refresh, error: None };
        view.model.set_grouped(plico::layout_preferences::load_changes_option(&view.repo.path,plico::layout_preferences::ChangesOption::Directory));
        view.show_ignored=plico::layout_preferences::load_changes_option(&view.repo.path,plico::layout_preferences::ChangesOption::Ignored);
        let ignored=view.show_ignored;view.refresh.update(cx,|q,cx|q.set_show_ignored(ignored,cx)); view
    }
    pub fn focus_message(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input,cx| input.focus(window,cx));
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.write_token.is_none() {
            self.refresh.update(cx, |query, cx| query.request(cx));
        }
    }
    fn sync_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text=self.draft.message().to_owned(); self.input.update(cx,|s,cx|s.set_value(text,window,cx));
    }
    fn sync_amend_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Rebased focuses the message only when Amend actually replaces it.
        // In particular, an edit made while loading must keep its caret/selection.
        if self.input.read(cx).value().as_ref() == self.draft.message() { return; }
        let text=self.draft.message().to_owned();
        self.input.update(cx,|s,cx| { s.set_value(text,window,cx); s.focus(window,cx); s.select_all(window,cx); });
    }
    fn toggle_amend(&mut self, checked: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.write_token.is_some() {return;}
        self.amend_token.cancel(); self.amend_token=Cancel::default();
        if !checked {self.draft.disable_amend();self.sync_amend_input(window,cx);cx.notify();return;}
        let Some(oid)=self.head.clone() else{return;};
        let Ok(ticket)=self.draft.enable_amend(&oid) else{return;};
        let repo=self.repo.clone();let token=self.amend_token.clone();
        self.amend_task=cx.spawn_in(window,async move|this,wx|{
            let worker_oid=oid.clone();
            let result=wx.background_spawn(async move{plico::commit_draft::load_amend_message(&repo,&worker_oid,&token)}).await;
            let _=wx.update(|window,cx|{let _=this.update(cx,|s,cx|{
                let result=if s.head.as_deref()==Some(oid.as_str()){result}else{Err("HEAD changed; reload Amend".into())};
                if s.draft.loaded(ticket,&oid,result){s.sync_amend_input(window,cx);cx.notify();}
            });});
        });cx.notify();
    }
    pub fn writing(&self) -> bool { self.preparing || self.write_token.is_some() }
    fn submit(&mut self, push: bool, recover: bool, _: &mut Window, cx: &mut Context<Self>) { cx.emit(Submit(push,recover)); }
    pub fn submit_saved(&mut self, push: bool, recover: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.preparing || self.write_token.is_some() { return; }
        self.preparing = true; cx.notify();
        cx.spawn_in(window, async move |this, wx| {
            loop {
                let done=this.update_in(wx,|s,w,cx| {
                    if s.refresh.read(cx).busy() {return false;}
                    s.preparing = false;
                    s.execute_submit(push,recover,w,cx); cx.notify(); true
                }).unwrap_or(true);
                if done {break;}
                wx.background_executor().timer(std::time::Duration::from_millis(25)).await;
            }
        }).detach();
    }
    fn execute_submit(&mut self, push: bool, recover: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.refresh.read(cx).busy() || self.write_token.is_some() || self.error.is_some() {return;}
        if self.recovery != recover {return;}
        let request=if recover {None} else {
            if let Err(error)=self.draft.edit(self.input.read(cx).value().to_string()){self.notice=Some(error);cx.notify();return;}
            if !self.draft.can_submit(self.scope.read(cx).inclusion.count()){return;}
            let amend=matches!(self.draft.amend(),Amend::Ready(_));
            if let Amend::Ready(oid)=self.draft.amend() {if self.head.as_ref()!=Some(oid){self.notice=Some(t("HEAD changed; reload Amend").into());cx.notify();return;}}
            match self.scope.read(cx).inclusion.request(self.draft.message().to_owned(),amend) {
                Ok(r)=>Some(r),Err(e)=>{self.notice=Some(e);cx.notify();return;}
            }
        };
        let partial=self.scope.read(cx).partial.values().filter(|f|f.selection.counts().0>0).cloned().collect::<Vec<_>>();
        let repo=self.repo.clone();let token=Cancel::default();self.write_token=Some(token.clone());
        self.write_error=None;
        self.notice=Some(t(if recover{"Restoring staged changes…"}else{"Committing…"}).into());
        self.write_task=cx.spawn_in(window,async move|this,wx|{
            let result=wx.background_spawn(async move{
                let progress=plico::operation_process::Progress::default();
                if recover {return commit_recovery::restore(&repo,&token,&progress).map_err(WriteFailure::restore);}
                let report=commit_operation::execute_partial(&repo,&request.unwrap(),&partial,&token,&progress).map_err(WriteFailure::commit)?;
                commit_feedback::from_report(&report)
            }).await;
            let _=wx.update(|window,cx|{let _=this.update(cx,|s,cx|{
                s.write_token=None;let success=result.is_ok();
                match result {Ok(())=>{s.notice=Some(t(if recover{"Staged changes restored"}else{"Committed"}).into());if !recover{s.draft.committed();s.scope.update(cx,|q,cx|{q.inclusion.clear();q.partial.clear();cx.notify();});s.sync_input(window,cx);}},Err(e)=>{s.notice=None;s.write_error=Some(e);}}
                cx.emit(Changed{push:success && push && !recover});cx.notify();
            });});
        });cx.notify();
    }
    pub fn preview_loading(&self, cx: &App) -> bool { self.refresh.read(cx).busy() }
    pub fn preview_revision(&self, cx: &App) -> u64 { self.refresh.read(cx).revision() }
    pub fn preview_change(&self, cx: &App) -> Option<&plico::git::Change> {
        if self.refresh.read(cx).busy() || self.error.is_some() { return None; }
        self.model.files().get(self.model.selected().unwrap_or(0)).filter(|f|f.area!=plico::git::Area::Ignored)
    }
    pub fn preview_target(&self, cx: &App) -> Option<(Target, Chain)> {
        let change = self.preview_change(cx)?.clone();
        let target = Target::new(self.repo.clone(), None, change).ok()?;
        let chain = Chain::new(&target, self.model.files()).ok()?;
        Some((target, chain))
    }
    pub fn select_target(&mut self, target: &Target, cx: &mut Context<Self>) {
        if target.identity().repository != self.repo.path || target.identity().revision.is_some() { return; }
        self.rollback_scope=Some((target.change().area,std::iter::once(target.change().path.clone()).collect()));
        if let Some(row) = self.model.reveal(target.change()) {
            self.scroll.scroll_to_item(row, ScrollStrategy::Top);
            cx.emit(PreviewSelected);
            cx.notify();
        }
    }
    fn show_diff(&mut self,cx:&mut Context<Self>) {
        if self.refresh.read(cx).busy() || self.error.is_some() {return;}
        let Some(change)=self.model.selected().and_then(|i|self.model.files().get(i)).filter(|f|f.area!=plico::git::Area::Ignored).cloned() else {return;};
        match Target::new(self.repo.clone(),None,change).and_then(|target| Chain::new(&target,self.model.files()).map(|chain|OpenDiff(target,chain))) {
            Ok(event)=>cx.emit(event), Err(e)=>{self.error=Some(e);cx.notify();}
        }
    }
    fn step(&mut self,forward:bool,cx:&mut Context<Self>) {
        self.model.step(forward);
        cx.emit(PreviewSelected);
        if let Some(index)=self.model.selected() {
            let change=&self.model.files()[index];self.rollback_scope=Some((change.area,std::iter::once(change.path.clone()).collect()));
            if let Some(row)=self.model.rows().iter().position(|r|matches!(r,Row::Entry{row:file_tree::Row::File{index:i,..},..} if *i==index)) {
                self.scroll.scroll_to_item(row,ScrollStrategy::Top);
            }
        } cx.notify();
    }
}
fn icon(path:&'static str)->Icon {Icon::default().path(path).size(px(16.))}
impl Render for LocalView {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let selection_color = crate::ui_style::row_selection(&self.focus, window);
        let row_height = crate::ui_style::tree_row_height(cx);
        let chinese = plico::i18n::is_chinese();
        if self.ui_chinese != chinese {
            self.ui_chinese = chinese;
            self.input.update(cx, |input,cx| input.set_placeholder(t("Commit message"),window,cx));
        }
        v_flex().size_full().rounded(px(10.)).overflow_hidden().bg(rgb(0x18191a)).key_context("LocalChanges").track_focus(&self.focus)

            .on_action(cx.listener(|s,_:&ShowDiff,_,cx|s.show_diff(cx)))
            .on_action(cx.listener(|s,_:&NextFile,_,cx|s.step(true,cx)))
            .on_action(cx.listener(|s,_:&PreviousFile,_,cx|s.step(false,cx)))
            .on_action(cx.listener(|s,_:&Refresh,_,cx|s.refresh(cx)))
            .on_action(cx.listener(|s,_:&Rollback,_,cx|s.request_rollback(cx)))
            .on_action(cx.listener(|s,_:&ToggleDirectory,_,cx|s.toggle_directory(cx)))
            .child(h_flex().h(px(32.)).flex_shrink_0().px_2().gap_1()
                .child(Button::new("local-refresh").ghost().small().icon(icon("jb/refresh_dark.svg")).tooltip(t("Refresh"))
                    .loading(self.refresh.read(cx).feedback())
                    .disabled(self.write_token.is_some()).on_click(cx.listener(|s,_,_,cx|s.refresh(cx))))
                .child(Button::new("local-rollback").ghost().small().icon(icon("jb/rollback_dark.svg")).tooltip(t("Rollback… · Ctrl+Alt+Z"))
                    .disabled(!self.can_rollback(cx)).on_click(cx.listener(|s,_,_,cx|s.request_rollback(cx))))
                .child(Button::new("local-diff").ghost().small().icon(icon("jb/diff_dark.svg")).tooltip(t("Show Diff · Ctrl+D"))
                    .disabled(self.refresh.read(cx).busy() || self.error.is_some() || self.model.selected().is_none() || self.model.selected().is_some_and(|i|self.model.files()[i].area==plico::git::Area::Ignored)).on_click(cx.listener(|s,_,_,cx|s.show_diff(cx))))
                .child(self.view_options(cx))
                .child(div().w(px(1.)).h(px(20.)).bg(rgb(0x393b40)))
                .child(Button::new("local-locate").ghost().small().icon(icon("jb/locate_dark.svg")).tooltip(t("Show selected change")).disabled(self.model.selected().is_none()).on_click(cx.listener(|s,_,_,cx|{if let Some(c)=s.model.selected().and_then(|i|s.model.files().get(i)).cloned(){if let Some(row)=s.model.reveal(&c){s.scroll.scroll_to_item(row,ScrollStrategy::Top);cx.notify();}} })))
                .child(Button::new("local-expand").ghost().small().icon(icon("jb/expandAll_dark.svg")).tooltip(t("Expand All"))
                    .on_click(cx.listener(|s,_,_,cx|{s.model.expand_all();cx.notify();})))
                .child(Button::new("local-collapse").ghost().small().icon(icon("jb/collapseAll_dark.svg")).tooltip(t("Collapse All"))
                    .on_click(cx.listener(|s,_,_,cx|{s.model.collapse_all();cx.notify();})))
                .when(self.refresh.read(cx).feedback() && self.model.files().is_empty(), |v| v.child(
                    div().truncate().text_color(rgb(crate::ui_style::MUTED)).child(t("Loading local changes…")))))
            .when_some(self.error.clone(),|v,e|v.child({
                let owner=cx.entity().downgrade();
                crate::error_ui::notice("commit-read-error","Could not load local changes",e,self.refresh.read(cx).busy(),move|_,_,cx|{let _=owner.update(cx,|s,cx|s.refresh(cx));})
            }))
            .child(div().relative().flex_1().min_h_0().child(uniform_list("local-tree",self.model.rows().len(),cx.processor(move |s,range:std::ops::Range<usize>,_,cx| {
                range.map(|row| {
                    let item=s.model.rows()[row].clone();
                    let updating = s.refresh.read(cx).feedback() && matches!(item, Row::Group { .. });
                    let paths=s.model.inclusion_paths(row);
                    let area=match &item {Row::Group{area,..}|Row::Entry{area,..}=>*area};
                    let ids=s.model.files().iter().filter(|f|f.area==area && paths.contains(&f.path))
                        .filter_map(CommitFile::from_change).map(|f|InclusionIdentity::of(&f)).collect();
                    let (included,total)=s.scope.read(cx).counts(&ids);
                    let branch=!matches!(&item, Row::Entry{row:file_tree::Row::File{..},..});
                    let (label,depth,asset,selected,color)=match item {
                        Row::Group{area,count,expanded}=>(format!("{}  {count}",t(if area==plico::git::Area::Untracked {"Unversioned Files"} else {area.label()})),0,if expanded{"jb/chevronDown_dark.svg"}else{"jb/chevronRight_dark.svg"},false,0xbcbec4),
                        Row::Entry{row:file_tree::Row::Folder{path,depth,expanded,..},..}=>(path.file_name().unwrap_or_default().to_string_lossy().into_owned(),depth+1,if expanded{"jb/chevronDown_dark.svg"}else{"jb/chevronRight_dark.svg"},false,0xbcbec4),
                        Row::Entry{row:file_tree::Row::File{index,depth},..}=>{
                            let f=&s.model.files()[index];
                            (f.path.file_name().unwrap_or_default().to_string_lossy().into_owned(),depth+2,crate::ui_style::file_icon(&f.path),s.model.selected()==Some(index),if f.area==plico::git::Area::Ignored{0x72757a}else{crate::ui_style::change_color(f)})
                        }
                    };
                    let rollback_paths=paths.clone();let context_owner=cx.entity().downgrade();let rollback_enabled=area==plico::git::Area::WorkingTree && s.can_rollback(cx);
                    let parent_label=if !s.model.grouped(){match &s.model.rows()[row]{Row::Entry{row:file_tree::Row::File{index,..},..}=>{let p=s.model.files()[*index].path.parent().unwrap_or(std::path::Path::new(""));Some(if p.as_os_str().is_empty(){s.repo.path.file_name().unwrap_or_default().to_string_lossy().into_owned()}else{p.display().to_string()})},_=>None}}else{None};
                    // One ordered text run: the secondary path must not shrink a short filename.
                    let mut row_text = label.clone();
                    let name_end = row_text.len();
                    if let Some(parent) = parent_label { row_text.push_str("  "); row_text.push_str(&parent); }
                    let text_end = row_text.len();
                    let row_text = StyledText::new(row_text).with_highlights([(name_end..text_end, HighlightStyle {
                        color: Some(rgb(crate::ui_style::MUTED).into()), ..Default::default()
                    })]);
                    h_flex().id(("local-row",row)).relative().w_full().h(row_height).pl(px(crate::ui_style::TREE_INSET+depth as f32*crate::ui_style::TREE_INDENT)).pr(px(crate::ui_style::TREE_INSET)).gap(px(4.))
                        .when(selected,|v|v.child(crate::ui_style::tree_selection(selection_color)))
                        .when(branch, |v|v.child(icon(asset)))
                        .when(area!=plico::git::Area::Ignored,|v|v.child(Checkbox::new(("commit-file",row)).checked(total>0 && included==total)
                            .indeterminate(included>0 && included<total)
                            .accessibility_label(label.clone())
                            .tooltip(t("Commit entire file, including unstaged changes"))
                            .disabled(s.refresh.read(cx).busy() || s.error.is_some() || s.write_token.is_some() || total==0)
                            .on_click(cx.listener(move|s,checked:&bool,_,cx|{s.scope.update(cx,|q,cx|q.set_scope(&ids,*checked,cx));cx.stop_propagation();cx.notify();}))))
                        .when(!branch, |v|v.child(img(asset).size(px(16.)).flex_shrink_0()))
                        .child(div().truncate().text_color(rgb(color)).when(branch && area!=plico::git::Area::Untracked,|v|v.font_weight(FontWeight::SEMIBOLD)).child(row_text))
                        .when(updating, |v| v.child(div().truncate().text_color(rgb(crate::ui_style::MUTED)).child(t("updating…"))))
                        .on_mouse_down(MouseButton::Right,cx.listener(move|s,_,w,cx|{s.rollback_scope=Some((area,rollback_paths.clone()));s.focus.focus(w,cx);cx.notify();}))
                        .on_click(cx.listener(move |s,e:&ClickEvent,w,cx|{s.focus.focus(w,cx);
                            s.rollback_scope=s.model.rows().get(row).map(|r|(match r{Row::Group{area,..}|Row::Entry{area,..}=>*area},s.model.inclusion_paths(row)));
                            match s.model.rows().get(row).cloned() {
                                Some(Row::Entry{row:file_tree::Row::File{index,..},..})=>{s.model.select(index);cx.emit(PreviewSelected);if e.click_count()==2{s.show_diff(cx);}},
                                _=>s.model.toggle(row),
                            } cx.notify();
                        }))
                        .context_menu(move|menu,_,_|{let owner=context_owner.clone();menu.item(PopupMenuItem::new(t("Rollback…")).disabled(!rollback_enabled).on_click(move|_,_,cx|{let _=owner.update(cx,|s,cx|s.request_rollback(cx));}))})
                }).collect::<Vec<_>>()
            })).track_scroll(&self.scroll).size_full().map(|tree| {
                #[cfg(all(test, feature = "desktop-tests"))]
                let tree = tree.debug_selector(|| "local-tree".into());
                tree
            })).when(self.refresh.read(cx).loaded() && self.error.is_none() && self.model.files().is_empty(), |v| v.child(
                div().absolute().top_0().left_0().p_3().child(t("No changes to commit.")))))
            .child(v_flex().flex_shrink_0().p_3().gap_2().border_t_1().border_color(rgb(0x2c2d30))
                .child(h_flex().min_w_0().gap_2().child(Checkbox::new("commit-amend").max_w(relative(0.8)).flex_shrink_0().label(t("Amend last commit")).checked(!matches!(self.draft.amend(),Amend::Off))
                    .disabled(self.write_token.is_some() || self.refresh.read(cx).busy() || self.head.is_none() || self.recovery)
                    .on_click(cx.listener(|s,checked:&bool,w,cx|s.toggle_amend(*checked,w,cx)))).child(status::selection_status(self.scope.read(cx).inclusion.count(), cx)))
                .when(matches!(self.draft.amend(),Amend::Loading(_)),|v|v.child(t("Loading commit message…")))
                .when_some(match self.draft.amend(){Amend::Failed(e)=>Some(e.clone()),_=>None},|v,e|v.child(v_flex().gap_1()
                    .child(div().text_size(rems(1.)).child(t("Could not load commit message")))
                    .child(h_flex().gap_2()
                        .child(Button::new("amend-message-retry").link().small().label(t("Retry"))
                            .on_click(cx.listener(|s,_,w,cx|s.toggle_amend(true,w,cx))))
                        .child(crate::error_ui::details_button("amend-message-error",e)))))
                .child(Textarea::new(&self.input).h(px(200.)).external_focus_ring(false).disabled(self.write_token.is_some()).aria_label(t("Commit message")))
                .when_some(self.notice.clone(),|v,text|v.child(div().text_size(px(12.)).max_h(px(72.)).overflow_hidden().child(text)))
                .when_some(self.write_error.as_ref(),|v,e|v.child(v_flex().gap_1().min_w_0()
                    .child(div().text_size(rems(1.)).child(t(e.summary)))
                    .child(h_flex().gap_2()
                        .child(crate::error_ui::details_button("commit-write-error",format!("{}\n{}",t(e.summary),e.details)))
                        .child(Button::new("commit-error-dismiss").link().small().label(t("Dismiss"))
                            .on_click(cx.listener(|s,_,_,cx|{s.write_error=None;cx.notify();}))))))
                .when(self.recovery,|v|v.child(Button::new("commit-recover").label(t("Restore staged changes")).disabled(self.write_token.is_some() || self.refresh.read(cx).busy())
                    .on_click(cx.listener(|s,_,w,cx|s.submit(false,true,w,cx)))))
                .child(h_flex().flex_wrap().gap_2()
                    .child(Button::new("commit-submit").primary().label(t(if !matches!(self.draft.amend(),Amend::Off){"Amend Commit"}else{"Commit"}))
                        .disabled(self.refresh.read(cx).busy() || self.error.is_some() || self.recovery || self.write_token.is_some() || !self.draft.can_submit(self.scope.read(cx).inclusion.count()))
                        .on_click(cx.listener(|s,_,w,cx|s.submit(false,false,w,cx))))
                    .child(Button::new("commit-push").label(t(if !matches!(self.draft.amend(),Amend::Off){"Amend Commit and Push…"}else{"Commit and Push…"}))
                        .disabled(self.refresh.read(cx).busy() || self.error.is_some() || self.recovery || self.write_token.is_some() || !self.draft.can_submit(self.scope.read(cx).inclusion.count()))
                        .on_click(cx.listener(|s,_,w,cx|s.submit(true,false,w,cx))))
                    .when(self.write_token.is_some() && !self.rolling_back,|v|v.child(Button::new("commit-cancel").label(t("Cancel")).on_click(cx.listener(|s,_,_,cx|{if let Some(t)=&s.write_token{t.cancel();}cx.notify();}))))))
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod refresh_tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    #[gpui::test]
    fn refreshing_retains_tree_geometry_and_delivers_results(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut local = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let count = std::rc::Rc::new(std::cell::Cell::new(0));
            let query = cx.new(|_| crate::changes_refresh::ChangesRefresh::with_loader(
                Repository::new("unused"), std::rc::Rc::new(move |_, _, _, cx| {
                    let n = count.get() + 1; count.set(n);
                    let timer = cx.background_executor().timer(std::time::Duration::from_millis(if n == 1 { 100 } else { 1000 }));
                    cx.background_spawn(async move {
                        timer.await;
                        Ok((plico::commit_changes::Snapshot {
                            head: Some(format!("head-{n}")),
                            files: vec![CommitFile::Unversioned(plico::git::Change {
                                path: format!("review-{n}.txt").into(), old_path: None,
                                status: "?".into(), area: plico::git::Area::Untracked,
                            })], review_versions: vec![],
                        }, false, vec![]))
                    })
                })));
            let view = cx.new(|cx| LocalView::with_refresh(Repository::new("unused"), query, window, cx));
            local = Some(view.clone());
            gpui::component::Root::new(view, window, cx)
        });
        let local = local.unwrap();
        cx.run_until_parked();
        cx.executor().advance_clock(std::time::Duration::from_millis(100)); cx.run_until_parked();
        cx.update(|w, cx| w.draw(cx).clear(cx));
        let before = cx.debug_bounds("local-tree").unwrap();
        assert_eq!(local.read_with(cx, |s, _| s.head.clone()).as_deref(), Some("head-1"));
        cx.update(|_, cx| local.update(cx, |s, cx| s.refresh(cx)));
        cx.run_until_parked();
        cx.executor().advance_clock(std::time::Duration::from_millis(350)); cx.run_until_parked();
        cx.update(|w, cx| w.draw(cx).clear(cx));
        assert_eq!(cx.debug_bounds("local-tree").unwrap(), before);
        assert!(local.read_with(cx, |s, cx| s.refresh.read(cx).feedback()));
        assert_eq!(local.read_with(cx, |s, _| s.head.clone()).as_deref(), Some("head-1"));
        cx.executor().advance_clock(std::time::Duration::from_millis(650)); cx.run_until_parked();
        assert_eq!(local.read_with(cx, |s, _| s.head.clone()).as_deref(), Some("head-2"));
        assert_eq!(local.read_with(cx, |s, _| s.model.files()[0].path.clone()), std::path::PathBuf::from("review-2.txt"));
        assert!(!local.read_with(cx, |s, cx| s.refresh.read(cx).feedback()));
    }
}
