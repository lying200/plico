//! Selected-commit ancestry query, independent of file/Diff loading jobs.
use plico::i18n::{self, t};
use plico::git::{Cancel, Repository};
use gpui_kit::{component::{ActiveTheme as _, h_flex, v_flex, text::TextView}, prelude::FluentBuilder as _, *};

pub struct ContainingBranches {
    token: Cancel,
    _task: Task<()>,
    result: Option<Result<Vec<String>, String>>,
    expanded: bool,
    width: Pixels,
    link_focus: FocusHandle,
}
impl Drop for ContainingBranches {
    fn drop(&mut self) { self.token.cancel(); }
}
impl ContainingBranches {
    pub fn pending(cx: &mut Context<Self>) -> Self {
        Self { token: Cancel::default(), _task: Task::ready(()), result: None,
            expanded: false, width: px(0.), link_focus: cx.focus_handle().tab_stop(true) }
    }
    // One entity per selection. Its weak task cannot update a later selection,
    // even if the cancelled Git process finishes after the replacement query.
    pub fn new(repo: Repository, id: String, cx: &mut Context<Self>) -> Self {
        let token = Cancel::default();
        let query_token = token.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { repo.containing_branches(&id, &query_token) }).await;
            let _ = this.update(cx, |this, cx| { this.result = Some(result); cx.notify(); });
        });
        Self { token, _task: task, result: None, expanded: false, width: px(0.), link_focus: cx.focus_handle().tab_stop(true) }
    }
}
/// One sequential worker for the current multi-selection, not one worker per card.
/// Results belong to weak card entities; switching selection cancels the worker.
pub struct Batch { token: Cancel, _task: Task<()> }
impl Drop for Batch { fn drop(&mut self) { self.token.cancel(); } }
impl Batch {
    pub fn new(repo: Repository, cards: Vec<(String, WeakEntity<ContainingBranches>)>, cx: &mut Context<Self>) -> Self {
        let token = Cancel::default();
        let query_token = token.clone();
        let task = cx.spawn(async move |this, cx| {
            let mut remaining = plico::git::OUTPUT_LIMIT;
            for (id, card) in cards {
                if query_token.cancelled() { break; }
                let result = if remaining == 0 {
                    Err("Containing branches exceed the selection output limit".to_owned())
                } else {
                    let repo = repo.clone(); let token = query_token.clone();
                    cx.background_spawn(async move { repo.containing_branches(&id, &token) }).await
                };
                if query_token.cancelled() { break; }
                let result = result.and_then(|names| {
                    let size = names.capacity().saturating_mul(std::mem::size_of::<String>())
                        .saturating_add(names.iter().map(String::capacity).sum::<usize>());
                    if size > remaining { remaining = 0; Err("Containing branches exceed the selection output limit".into()) }
                    else { remaining -= size; Ok(names) }
                });
                if this.update(cx, |_, cx| {
                    let _ = card.update(cx, |view, cx| { view.result = Some(result); cx.notify(); });
                }).is_err() { break; }
            }
        });
        Self { token, _task: task }
    }
}
impl Render for ContainingBranches {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = v_flex().relative().w_full().min_w_0().flex_shrink_0()
            .font_family(cx.theme().font_family.clone()).text_size(cx.theme().font_size).line_height(relative(1.3)).text_color(rgb(0xd1d3d9));
        let owner = cx.entity().downgrade();
        let width = self.width;
        panel = panel.child(canvas(move |bounds, window, _| {
            if (bounds.size.width - width).abs() > px(0.5) {
                window.on_next_frame(move |_, cx| {
                    let _ = owner.update(cx, |s, cx| { s.width = bounds.size.width; cx.notify(); });
                });
            }
        }, |_, _, _, _| {}).absolute().size_full());
        match &self.result {
            None => panel.child(t("In branches: loading…")),
            Some(Err(error)) => panel.child(div().whitespace_normal().child(format!("Could not load containing branches: {error}"))),
            Some(Ok(branches)) if branches.is_empty() => panel.child(t("Not in any branch")),
            Some(Ok(branches)) => {
                let head = i18n::message(i18n::Message::ContainingBranches(branches.len()));
                let measure = |text: &str| {
                    let run = TextRun { len: text.len(), font: Font { family: cx.theme().font_family.clone(), ..Default::default() }, color: rgb(0xd1d3d9).into(), background_color: None, underline: None, strikethrough: None };
                    window.text_system().shape_line(text.to_owned().into(), cx.theme().font_size, &[run], None).width
                };
                // Fit incrementally: avoid shaping every name in a large result.
                // Rebased reserves the ellipsis/link width before fitting names.
                let mut summary = head.clone();
                let mut remaining = (self.width - measure(&head) - measure(&format!("… {}", t("Show all")))).max(px(0.));
                let mut shortened = false;
                if !self.expanded {
                    for (index, branch) in branches.iter().enumerate() {
                        let piece = format!("{branch}{}", if index + 1 < branches.len() { ", " } else { "" });
                        let piece_width = measure(&piece);
                        if piece_width < remaining {
                            summary.push_str(&piece);
                            remaining -= piece_width;
                        } else {
                            // Binary search only UTF-8 boundaries; never split a character.
                            let offsets: Vec<_> = piece.char_indices().map(|(i, _)| i).chain(std::iter::once(piece.len())).collect();
                            let mut low = 0;
                            let mut high = offsets.len();
                            while low + 1 < high {
                                let mid = (low + high) / 2;
                                if measure(&piece[..offsets[mid]]) < remaining { low = mid; } else { high = mid; }
                            }
                            summary.push_str(&piece[..offsets[low]]);
                            summary.push('…');
                            shortened = true;
                            break;
                        }
                    }
                }
                let show_link = self.expanded || shortened;
                let line = h_flex().w_full().min_w_0()
                    .child(div().min_w_0().when(!self.expanded, |d| d.truncate())
                        .child(if self.expanded { head } else { summary }))
                    .when(show_link, |d| d.child(div().id("show-containing-branches")
                        .track_focus(&self.link_focus)
                        .on_mouse_down(MouseButton::Left, cx.listener(|s, _, w, cx| s.link_focus.focus(w, cx)))
                        .on_key_down(cx.listener(|s, e: &KeyDownEvent, _, cx| {
                            if matches!(e.keystroke.key.as_str(), "enter" | "space") { s.expanded = !s.expanded; cx.stop_propagation(); cx.notify(); }
                        }))
                        .flex_shrink_0().ml(px(4.)).text_color(rgb(0x71a1fe)).cursor_pointer()
                        .hover(|d| d.underline())
                        .child(t(if self.expanded { "Hide" } else { "Show all" }))
                        .on_click(cx.listener(|s, _, _, cx| { s.expanded = !s.expanded; cx.notify(); }))));
                panel = panel.child(line);
                if self.expanded {
                    let html = branches.iter().map(|name| format!("<p>{}</p>", name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"))).collect::<String>();
                    panel = panel.child(TextView::html("containing-branches-list", html).w_full().selectable(true)
                        .style(gpui_kit::component::text::TextViewStyle::default().paragraph_gap(rems(0.))));
                }
                panel
            }
        }
    }
}
