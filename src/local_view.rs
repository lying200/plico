//! Independent workspace tool window; opening a comparison does not select a Log row.
use plico::{git::{Cancel, Repository}, local_changes::{Model, Row}, file_tree, diff_target::{Target, Chain}, i18n::t, commit_inclusion::{Inclusion as Selection, Identity as InclusionIdentity}, commit_changes::{Snapshot, File as CommitFile}, commit_draft::{Draft, Amend}, commit_operation, commit_recovery};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{button::{Button, ButtonVariants as _}, checkbox::Checkbox, input::{Textarea, TextareaState, InputEvent}, Disableable as _, Sizable as _, Icon, h_flex, v_flex};
actions!(local_review, [ShowDiff, NextFile, PreviousFile, Refresh]);
pub struct OpenDiff(pub Target, pub Chain);
pub struct Hide;
pub struct Submit(pub bool, pub bool);
pub struct Changed { pub push: bool }
struct WriteFailure { summary: &'static str, details: String }
pub struct LocalView {
    ui_chinese: bool,
    repo: Repository, model: Model, focus: FocusHandle, scroll: UniformListScrollHandle,
    selection: Selection, draft: Draft, input: Entity<TextareaState>, head: Option<String>, recovery: bool,
    preparing: bool,
    write_token: Option<Cancel>, write_task: Task<()>, amend_token: Cancel, amend_task: Task<()>, notice: Option<String>, write_error: Option<WriteFailure>,
    token: Cancel, generation: u64, task: Task<()>, busy: bool, refresh_pending: bool, error: Option<String>,
}
impl EventEmitter<OpenDiff> for LocalView {}
impl EventEmitter<Hide> for LocalView {}
impl EventEmitter<Submit> for LocalView {}
impl EventEmitter<Changed> for LocalView {}
impl Drop for LocalView { fn drop(&mut self) { self.token.cancel(); self.amend_token.cancel(); if let Some(t) = &self.write_token { t.cancel(); } } }
impl LocalView {
    pub fn new(repo: Repository, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new("enter",ShowDiff,Some("LocalChanges")), KeyBinding::new("ctrl-d",ShowDiff,Some("LocalChanges")),
            KeyBinding::new("down",NextFile,Some("LocalChanges")),KeyBinding::new("up",PreviousFile,Some("LocalChanges")),
            KeyBinding::new("ctrl-f5",Refresh,Some("LocalChanges"))]);
        let input = cx.new(|cx| TextareaState::new(window, cx).placeholder(t("Commit message")));
        cx.subscribe_in(&input, window, |s, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                if let Err(error) = s.draft.edit(s.input.read(cx).value().to_string()) { s.notice = Some(error); }
                cx.notify();
            }
        }).detach();
        let mut view = Self { ui_chinese: plico::i18n::is_chinese(), selection: Selection::default(), draft: Draft::default(), input, head: None, recovery: false,
            preparing: false, write_token: None, write_task: Task::ready(()), amend_token: Cancel::default(), amend_task: Task::ready(()), notice: None, write_error: None, repo, model: Model::default(), focus: cx.focus_handle(), scroll: UniformListScrollHandle::new(),
            token: Cancel::default(), generation: 0, task: Task::ready(()), busy: false, refresh_pending: false, error: None };
        view.refresh(cx); view
    }
    pub fn focus_message(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input,cx| input.focus(window,cx));
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.write_token.is_some() { return; }
        if self.busy { self.refresh_pending = true; return; }
        self.refresh_pending = false;
        self.token.cancel(); self.generation+=1; self.token=Cancel::default(); self.busy=true; self.error=None;
        let repo=self.repo.clone(); let token=self.token.clone(); let generation=self.generation;
        self.task=cx.spawn(async move |this,cx| {
            let result=cx.background_spawn(async move {
                let snapshot=Snapshot::load(&repo,&token)?;
                let recovery=commit_recovery::pending(&repo.editor_directory(&token)?)?.is_some();
                Ok::<_,String>((snapshot,recovery))
            }).await;
            let _=this.update(cx,|s,cx| {
                if generation!=s.generation {return;}
                s.busy=false;
                match result { Ok((snapshot,recovery))=>{s.selection.replace(&snapshot);s.model.replace(snapshot.files.iter().map(CommitFile::change).collect());s.head=snapshot.head;s.recovery=recovery;}, Err(e)=>s.error=Some(e) }
                if std::mem::take(&mut s.refresh_pending) { s.refresh(cx); }
                cx.notify();
            });
        }); cx.notify();
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
                    if s.busy {return false;}
                    s.preparing = false;
                    s.execute_submit(push,recover,w,cx); cx.notify(); true
                }).unwrap_or(true);
                if done {break;}
                wx.background_executor().timer(std::time::Duration::from_millis(25)).await;
            }
        }).detach();
    }
    fn execute_submit(&mut self, push: bool, recover: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.write_token.is_some() || self.error.is_some() {return;}
        if self.recovery != recover {return;}
        let request=if recover {None} else {
            if let Err(error)=self.draft.edit(self.input.read(cx).value().to_string()){self.notice=Some(error);cx.notify();return;}
            if !self.draft.can_submit(self.selection.count()){return;}
            let amend=matches!(self.draft.amend(),Amend::Ready(_));
            if let Amend::Ready(oid)=self.draft.amend() {if self.head.as_ref()!=Some(oid){self.notice=Some(t("HEAD changed; reload Amend").into());cx.notify();return;}}
            match self.selection.request(self.draft.message().to_owned(),amend) {
                Ok(r)=>Some(r),Err(e)=>{self.notice=Some(e);cx.notify();return;}
            }
        };
        let repo=self.repo.clone();let token=Cancel::default();self.write_token=Some(token.clone());
        self.write_error=None;
        self.notice=Some(t(if recover{"Restoring staged changes…"}else{"Committing…"}).into());
        self.write_task=cx.spawn_in(window,async move|this,wx|{
            let result=wx.background_spawn(async move{
                let progress=plico::operation_process::Progress::default();
                if recover {return commit_recovery::restore(&repo,&token,&progress).map_err(|details|WriteFailure{summary:"Could not restore staged changes",details});}
                let report=commit_operation::execute(&repo,&request.unwrap(),&token,&progress).map_err(|details|WriteFailure{summary:"Commit failed",details})?;
                if report.succeeded(){return Ok(());}
                let label=match report.status(){
                    commit_operation::Status::RestoreFailed=>"Commit index recovery is required",
                    commit_operation::Status::RecoveryRecordFailed=>"Could not remove commit recovery record",
                    commit_operation::Status::Cancelled=>"Commit cancelled",
                    commit_operation::Status::TimedOut=>"Commit timed out",
                    commit_operation::Status::SigningFailed=>"Commit signing failed",
                    commit_operation::Status::Empty=>"Nothing to commit",
                    _=>"Commit failed",
                };
                let mut details=String::new();
                if let Some(step)=report.steps.last() {
                    match &step.result {
                        Err(e)=>details.push_str(e),
                        Ok(out)=>{
                            details.push_str(&String::from_utf8_lossy(&out.stderr.bytes()));
                            details.push_str(&String::from_utf8_lossy(&out.stdout.bytes()));
                            if let Some(e)=&out.input_error{details.push_str(&e.to_string());}
                        }
                    }
                }
                for e in [report.restore.as_ref().err(),report.recovery_record_error.as_ref(),report.head.as_ref().err(),report.state.as_ref().err()].into_iter().flatten(){
                    if !details.is_empty(){details.push('\n');}details.push_str(e);
                }
                // Both process streams are already bounded tails. Keep a bounded
                // presentation copy as well, with an explicit truncation marker.
                if details.chars().count()>32000{details=details.chars().take(32000).collect();details.push_str("\n[Diagnostic truncated]");}
                Err(WriteFailure{summary:label,details})
            }).await;
            let _=wx.update(|window,cx|{let _=this.update(cx,|s,cx|{
                s.write_token=None;let success=result.is_ok();
                match result {Ok(())=>{s.notice=Some(t(if recover{"Staged changes restored"}else{"Committed"}).into());if !recover{s.draft.committed();s.selection.clear();s.sync_input(window,cx);}},Err(e)=>{s.notice=None;s.write_error=Some(e);}}
                cx.emit(Changed{push:success && push && !recover});cx.notify();
            });});
        });cx.notify();
    }
    pub fn preview_loading(&self) -> bool { self.busy }
    pub fn preview_revision(&self) -> u64 { self.generation }
    pub fn preview_change(&self) -> Option<&plico::git::Change> {
        if self.busy || self.error.is_some() { return None; }
        self.model.files().get(self.model.selected().unwrap_or(0))
    }
    pub fn preview_target(&self) -> Option<(Target, Chain)> {
        let change = self.preview_change()?.clone();
        let target = Target::new(self.repo.clone(), None, change).ok()?;
        let chain = Chain::new(&target, self.model.files()).ok()?;
        Some((target, chain))
    }
    pub fn select_target(&mut self, target: &Target, cx: &mut Context<Self>) {
        if target.identity().repository != self.repo.path || target.identity().revision.is_some() { return; }
        if let Some(row) = self.model.reveal(target.change()) {
            self.scroll.scroll_to_item(row, ScrollStrategy::Top);
            cx.notify();
        }
    }
    fn show_diff(&mut self,cx:&mut Context<Self>) {
        if self.busy || self.error.is_some() {return;}
        let Some(change)=self.model.selected().and_then(|i|self.model.files().get(i)).cloned() else {return;};
        match Target::new(self.repo.clone(),None,change).and_then(|target| Chain::new(&target,self.model.files()).map(|chain|OpenDiff(target,chain))) {
            Ok(event)=>cx.emit(event), Err(e)=>{self.error=Some(e);cx.notify();}
        }
    }
    fn step(&mut self,forward:bool,cx:&mut Context<Self>) {
        self.model.step(forward);
        if let Some(index)=self.model.selected() {
            if let Some(row)=self.model.rows().iter().position(|r|matches!(r,Row::Entry{row:file_tree::Row::File{index:i,..},..} if *i==index)) {
                self.scroll.scroll_to_item(row,ScrollStrategy::Top);
            }
        } cx.notify();
    }
}
fn icon(path:&'static str)->Icon {Icon::default().path(path).size(px(16.))}
impl Render for LocalView {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
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
            .child(h_flex().h(px(36.)).px_3().child(t("Commit")).child(div().flex_1())
                .child(Button::new("local-hide").ghost().small().icon(icon("jb/closeSmall.svg")).tooltip(t("Hide"))
                    .on_click(cx.listener(|_,_,_,cx|cx.emit(Hide)))))
            .child(h_flex().h(px(32.)).px_2().gap_1()
                .child(Button::new("local-refresh").ghost().small().icon(icon("jb/refresh_dark.svg")).tooltip(t("Refresh"))
                    .disabled(self.write_token.is_some()).on_click(cx.listener(|s,_,_,cx|s.refresh(cx))))
                .child(Button::new("local-diff").ghost().small().icon(icon("jb/diff_dark.svg")).tooltip(t("Show Diff · Ctrl+D"))
                    .disabled(self.busy || self.error.is_some() || self.model.selected().is_none()).on_click(cx.listener(|s,_,_,cx|s.show_diff(cx))))
                .child(div().flex_1())
                .child(Button::new("local-expand").ghost().small().icon(icon("jb/expandAll_dark.svg")).tooltip(t("Expand All"))
                    .on_click(cx.listener(|s,_,_,cx|{s.model.expand_all();cx.notify();})))
                .child(Button::new("local-collapse").ghost().small().icon(icon("jb/collapseAll_dark.svg")).tooltip(t("Collapse All"))
                    .on_click(cx.listener(|s,_,_,cx|{s.model.collapse_all();cx.notify();}))))
            .when(self.busy,|v|v.child(div().px_3().child(t("Loading local changes…"))))
            .when_some(self.error.clone(),|v,e|v.child({
                let owner=cx.entity().downgrade();
                crate::error_ui::notice("commit-read-error","Could not load local changes",e,self.busy,move|_,_,cx|{let _=owner.update(cx,|s,cx|s.refresh(cx));})
            }))
            .when(!self.busy && self.error.is_none() && self.model.files().is_empty(),|v|v.child(div().p_3().child(t("No changes to commit."))))
            .child(uniform_list("local-tree",self.model.rows().len(),cx.processor(|s,range:std::ops::Range<usize>,_,cx| {
                range.map(|row| {
                    let item=s.model.rows()[row].clone();
                    let paths=s.model.inclusion_paths(row);
                    let area=match &item {Row::Group{area,..}|Row::Entry{area,..}=>*area};
                    let ids=s.model.files().iter().filter(|f|f.area==area && paths.contains(&f.path))
                        .filter_map(CommitFile::from_change).map(|f|InclusionIdentity::of(&f)).collect();
                    let (included,total)=s.selection.scope_counts(&ids);
                    let branch=!matches!(&item, Row::Entry{row:file_tree::Row::File{..},..});
                    let (label,depth,asset,selected,color)=match item {
                        Row::Group{area,count,expanded}=>(format!("{}  {count}",t(if area==plico::git::Area::Untracked {"Unversioned Files"} else {area.label()})),0,if expanded{"jb/chevronDown_dark.svg"}else{"jb/chevronRight_dark.svg"},false,0xbcbec4),
                        Row::Entry{row:file_tree::Row::Folder{path,depth,expanded,..},..}=>(path.file_name().unwrap_or_default().to_string_lossy().into_owned(),depth+1,if expanded{"jb/chevronDown_dark.svg"}else{"jb/chevronRight_dark.svg"},false,0xbcbec4),
                        Row::Entry{row:file_tree::Row::File{index,depth},..}=>{
                            let f=&s.model.files()[index];
                            (f.path.file_name().unwrap_or_default().to_string_lossy().into_owned(),depth+2,crate::ui_style::file_icon(&f.path),s.model.selected()==Some(index),crate::ui_style::change_color(f))
                        }
                    };
                    h_flex().id(("local-row",row)).w_full().h(px(28.)).pl(px(8.+depth as f32*14.)).pr_2().gap_2()
                        .when(selected,|v|v.bg(rgb(0x2e436e)))
                        .when(branch, |v|v.child(icon(asset)))
                        .child(Checkbox::new(("commit-file",row)).checked(total>0 && included==total)
                            .indeterminate(included>0 && included<total)
                            .accessibility_label(label.clone())
                            .tooltip(t("Commit entire file, including unstaged changes"))
                            .disabled(s.busy || s.error.is_some() || s.write_token.is_some() || total==0)
                            .on_click(cx.listener(move|s,checked:&bool,_,cx|{s.selection.set_scope(&ids,*checked);cx.stop_propagation();cx.notify();})))
                        .when(!branch, |v|v.child(img(asset).size(px(16.)).flex_shrink_0()))
                        .child(div().truncate().text_color(rgb(color)).child(label))
                        .on_click(cx.listener(move |s,e:&ClickEvent,w,cx|{s.focus.focus(w,cx);
                            match s.model.rows().get(row).cloned() {
                                Some(Row::Entry{row:file_tree::Row::File{index,..},..})=>{s.model.select(index);if e.click_count()==2{s.show_diff(cx);}},
                                _=>s.model.toggle(row),
                            } cx.notify();
                        }))
                }).collect::<Vec<_>>()
            })).track_scroll(&self.scroll).flex_1().min_h_0())
            .child(v_flex().flex_shrink_0().p_3().gap_2().border_t_1().border_color(rgb(0x2c2d30))
                .child(Checkbox::new("commit-amend").label(t("Amend last commit")).checked(!matches!(self.draft.amend(),Amend::Off))
                    .disabled(self.write_token.is_some() || self.busy || self.head.is_none() || self.recovery)
                    .on_click(cx.listener(|s,checked:&bool,w,cx|s.toggle_amend(*checked,w,cx))))
                .when(matches!(self.draft.amend(),Amend::Loading(_)),|v|v.child(t("Loading commit message…")))
                .when_some(match self.draft.amend(){Amend::Failed(e)=>Some(e.clone()),_=>None},|v,e|v.child(v_flex().gap_1()
                    .child(div().text_size(px(13.)).child(t("Could not load commit message")))
                    .child(h_flex().gap_2()
                        .child(Button::new("amend-message-retry").link().small().label(t("Retry"))
                            .on_click(cx.listener(|s,_,w,cx|s.toggle_amend(true,w,cx))))
                        .child(crate::error_ui::details_button("amend-message-error",e)))))
                .child(Textarea::new(&self.input).h(px(120.)).external_focus_ring(false).disabled(self.write_token.is_some()).aria_label(t("Commit message")))
                .when_some(self.notice.clone(),|v,text|v.child(div().text_size(px(12.)).max_h(px(72.)).overflow_hidden().child(text)))
                .when_some(self.write_error.as_ref(),|v,e|v.child(v_flex().gap_1().min_w_0()
                    .child(div().text_size(px(13.)).child(t(e.summary)))
                    .child(h_flex().gap_2()
                        .child(crate::error_ui::details_button("commit-write-error",format!("{}\n{}",t(e.summary),e.details)))
                        .child(Button::new("commit-error-dismiss").link().small().label(t("Dismiss"))
                            .on_click(cx.listener(|s,_,_,cx|{s.write_error=None;cx.notify();}))))))
                .when(self.recovery,|v|v.child(Button::new("commit-recover").label(t("Restore staged changes")).disabled(self.write_token.is_some() || self.busy)
                    .on_click(cx.listener(|s,_,w,cx|s.submit(false,true,w,cx)))))
                .child(h_flex().flex_wrap().gap_2()
                    .child(Button::new("commit-submit").primary().label(t(if !matches!(self.draft.amend(),Amend::Off){"Amend Commit"}else{"Commit"}))
                        .disabled(self.busy || self.error.is_some() || self.recovery || self.write_token.is_some() || !self.draft.can_submit(self.selection.count()))
                        .on_click(cx.listener(|s,_,w,cx|s.submit(false,false,w,cx))))
                    .child(Button::new("commit-push").label(t(if !matches!(self.draft.amend(),Amend::Off){"Amend Commit and Push…"}else{"Commit and Push…"}))
                        .disabled(self.busy || self.error.is_some() || self.recovery || self.write_token.is_some() || !self.draft.can_submit(self.selection.count()))
                        .on_click(cx.listener(|s,_,w,cx|s.submit(true,false,w,cx))))
                    .when(self.write_token.is_some(),|v|v.child(Button::new("commit-cancel").label(t("Cancel")).on_click(cx.listener(|s,_,_,cx|{if let Some(t)=&s.write_token{t.cancel();}cx.notify();}))))))
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod refresh_tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    #[gpui::test]
    fn repeated_refresh_does_not_cancel_inflight_query(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut local = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| LocalView::new(Repository::new("missing-fixture"), window, cx));
            local = Some(view.clone());
            gpui::component::Root::new(view, window, cx)
        });
        cx.update(|_, cx| local.as_ref().unwrap().update(cx, |s, cx| {
            s.task = Task::ready(());
            s.busy = true;
            let generation = s.generation;
            let token = s.token.clone();
            for _ in 0..10 { s.refresh(cx); }
            assert_eq!(s.generation, generation);
            assert!(!token.cancelled());
            assert!(s.refresh_pending);
        }));
    }
}
