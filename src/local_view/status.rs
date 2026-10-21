//! Width-adaptive selection status, following Rebased CommitStatusPanel's
//! full-label measurement and compact fallback. No cached layout or Git queries.
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme, tooltip::Tooltip};
use plico::i18n::t;

pub(super) fn selection_status(count: usize, cx: &App) -> impl IntoElement {
    let full = format!("{count} {}", t("selected"));
    let tooltip = full.clone();
    div().id("commit-selection-status").flex_1().min_w_0()
        .h(cx.theme().font_size * 1.5).overflow_hidden()
        .aria_label(full.clone())
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .child(canvas(move |bounds, window, _| {
            let style = window.text_style();
            let size = style.font_size.to_pixels(window.rem_size());
            let shape = |text: String| {
                let run = TextRun { len: text.len(), font: style.font(), color: rgb(0x589df6).into(),
                    background_color: None, underline: None, strikethrough: None };
                window.text_system().shape_line(text.into(), size, &[run], None)
            };
            let full_line = shape(full.clone());
            if full_line.width <= bounds.size.width { full_line } else { shape(count.to_string()) }
        }, |bounds, line, window, cx| {
            let origin = point(bounds.right() - line.width, bounds.top());
            let _ = line.paint(origin, bounds.size.height, TextAlign::Left, None, window, cx);
        }).size_full())
}
