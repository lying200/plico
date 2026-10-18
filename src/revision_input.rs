//! GitRefDialog / TwoStepCompletionProvider single-value completion adapter.
use plico::{git::Snapshot, i18n::t, user_filter::completion_matches};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{v_flex, input::{Input, InputState, InputEvent, Enter, Escape, MoveUp, MoveDown}};

pub struct RevisionInput {
    input: Entity<InputState>,
    candidates: Vec<String>,
    matches: Vec<usize>,
    selected: usize,
    accepted: Option<(SharedString, usize)>,
    scroll: UniformListScrollHandle,
    _subscription: Subscription,
}
impl RevisionInput {
    pub fn new(snapshot: &Snapshot, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let mut candidates: Vec<_> = snapshot.branches.iter().map(|b| b.name.as_str())
            .chain(snapshot.tags.iter().map(|t| t.name.as_str()))
            .filter_map(|name| ["refs/heads/", "refs/remotes/", "refs/tags/"].iter().find_map(|p| name.strip_prefix(p)))
            .map(str::to_owned).collect();
        candidates.sort_by(|a,b| plico::natural_order::compare(a,b));
        candidates.dedup();
        let subscription = cx.subscribe_in(&input, window, |s, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                let state = s.input.read(cx);
                let text = state.value();
                if s.accepted.as_ref() == Some(&(text.clone(), state.cursor())) { return; }
                s.accepted = None;
                let prefix = text.get(..state.cursor()).unwrap_or("");
                s.matches = if prefix.is_empty() { Vec::new() } else { s.candidates.iter().enumerate()
                    .filter(|(_, name)| completion_matches(name, prefix)).map(|(i,_)| i).collect() };
                s.selected = 0;
                s.scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            }
        });
        Self { input, candidates, matches: vec![], selected: 0, accepted: None, scroll: UniformListScrollHandle::new(), _subscription: subscription }
    }
    pub fn value(&self, cx: &App) -> String { self.input.read(cx).value().trim().to_owned() }
    pub fn focus(&self, window: &mut Window, cx: &mut App) { self.input.read(cx).focus_handle(cx).focus(window, cx); }
    fn accept(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(&candidate) = self.matches.get(index) else { return false; };
        let value = self.candidates[candidate].clone();
        let cursor = self.input.read(cx).cursor();
        self.input.update(cx, |s,cx| {
            s.set_selected_range(0..cursor, cx);
            s.replace_text_in_range(None, &value, window, cx);
        });
        let state = self.input.read(cx);
        self.accepted = Some((state.value(), state.cursor()));
        self.matches.clear(); cx.notify(); true
    }
    fn move_selection(&mut self, down: bool, cx: &mut Context<Self>) {
        if self.matches.is_empty() { cx.propagate(); return; }
        self.selected = if down { (self.selected+1)%self.matches.len() } else { (self.selected+self.matches.len()-1)%self.matches.len() };
        self.scroll.scroll_to_item(self.selected, ScrollStrategy::Nearest);
        cx.stop_propagation(); cx.notify();
    }
}
impl Render for RevisionInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_2()
            .child(t("Enter a branch, tag or commit revision"))
            .child(div().relative().child(Input::new(&self.input))
                .when(!self.matches.is_empty(), |v| v.child(deferred(div().absolute().top_full().left_0().w_full()
                    .occlude().bg(rgb(0x25262a)).border_1().border_color(rgb(0x393b40)).rounded(px(6.))
                    .child(uniform_list("revision-completion", self.matches.len(), cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                        range.map(|i| {
                            div().id(("revision-choice", i)).h(px(28.)).w_full().px_3().flex().items_center()
                                .when(i == s.selected, |v| v.bg(rgb(0x2d4771)))
                                .child(s.candidates[s.matches[i]].clone())
                                .on_click(cx.listener(move |s,_,w,cx| { s.accept(i,w,cx); }))
                        }).collect::<Vec<_>>()
                    })).track_scroll(&self.scroll).h(px(28. * self.matches.len().min(8) as f32)))).with_priority(101))))
            .child(div().text_color(rgb(0x9fa2a8)).child(t("Checkout opens the revision in detached HEAD mode.")))
            .capture_action(cx.listener(|s, _: &MoveDown, _, cx| s.move_selection(true,cx)))
            .capture_action(cx.listener(|s, _: &MoveUp, _, cx| s.move_selection(false,cx)))
            .capture_action(cx.listener(|s, _: &Enter, w, cx| {
                if s.accept(s.selected,w,cx) { cx.stop_propagation(); } else { cx.propagate(); }
            }))
            .capture_action(cx.listener(|s, _: &Escape, _, cx| {
                if s.matches.is_empty() { cx.propagate(); } else { s.matches.clear(); cx.stop_propagation(); cx.notify(); }
            }))
    }
}
