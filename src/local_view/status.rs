//! Rebased CommitStatusPanel and CommitLegendComponent rules, translated to
//! the shared inclusion model; no Git query or cached layout in this renderer.
use gpui_kit::component::{ActiveTheme, tooltip::Tooltip};
use gpui_kit::*;
use plico::{commit_inclusion::Summary, i18n::t};

fn chunks(summary: Summary, compact: bool) -> Vec<(String, u32)> {
    let mut chunks = Vec::new();
    let added = match (summary.added, summary.unversioned) {
        (0, 0) => String::new(),
        (a, 0) | (0, a) => a.to_string(),
        (a, b) => format!("{a}+{b}"),
    };
    for (value, label, symbol, color) in [
        (added, t("new"), "+", 0x73bd79),
        (
            if summary.modified == 0 {
                String::new()
            } else {
                summary.modified.to_string()
            },
            t("modified"),
            "*",
            0x70aeff,
        ),
        (
            if summary.deleted == 0 {
                String::new()
            } else {
                summary.deleted.to_string()
            },
            t("deleted"),
            "-",
            0x6f737a,
        ),
    ] {
        if !value.is_empty() {
            chunks.push((
                if compact {
                    format!("{symbol}{value}")
                } else {
                    format!("{value} {label}")
                },
                color,
            ));
        }
    }
    chunks
}
fn text_and_runs(chunks: &[(String, u32)], font: &Font, cx: &App) -> (String, Vec<TextRun>) {
    let mut text = String::new();
    let mut runs = Vec::new();
    for (label, color) in chunks {
        let part = if text.is_empty() {
            label.clone()
        } else {
            format!("   {label}")
        };
        runs.push(TextRun {
            len: part.len(),
            font: font.clone(),
            color: rgb(crate::ui_theme::color(
                cx,
                *color,
                match *color {
                    0x73bd79 => 0x0a7700,
                    0x70aeff => 0x0032a0,
                    _ => 0x616161,
                },
            ))
            .into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        });
        text.push_str(&part);
    }
    (text, runs)
}
pub(super) fn selection_status(summary: Summary, window: &Window, cx: &App) -> impl IntoElement {
    let full = chunks(summary, false);
    let compact = chunks(summary, true);
    let font = font(cx.theme().font_family.clone());
    let (compact_text, compact_runs) = text_and_runs(&compact, &font, cx);
    let min_width = if compact_text.is_empty() {
        px(0.)
    } else {
        window
            .text_system()
            .shape_line(
                compact_text.into(),
                cx.theme().font_size,
                &compact_runs,
                None,
            )
            .width
    };
    let tooltip = full
        .iter()
        .map(|(text, _)| text.as_str())
        .collect::<Vec<_>>()
        .join("   ");
    div()
        .id("commit-selection-status")
        .flex_1()
        .min_w(min_width)
        .h(cx.theme().font_size * 1.5)
        .overflow_hidden()
        .aria_label(tooltip.clone())
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .child(
            canvas(
                move |bounds, window, cx| {
                    if full.is_empty() {
                        return None;
                    }
                    let style = window.text_style();
                    let size = style.font_size.to_pixels(window.rem_size());
                    let shape = |chunks: &[(String, u32)]| {
                        let (text, runs) = text_and_runs(chunks, &style.font(), cx);
                        window
                            .text_system()
                            .shape_line(text.into(), size, &runs, None)
                    };
                    let full_line = shape(&full);
                    Some(if full_line.width <= bounds.size.width {
                        full_line
                    } else {
                        shape(&compact)
                    })
                },
                |bounds, line, window, cx| {
                    if let Some(line) = line {
                        let origin = point(bounds.right() - line.width, bounds.top());
                        let _ = line.paint(
                            origin,
                            bounds.size.height,
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                },
            )
            .size_full(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn compact_legend_keeps_categories_and_new_file_origins() {
        let summary = Summary {
            added: 1,
            unversioned: 2,
            modified: 3,
            deleted: 4,
        };
        let labels: Vec<_> = chunks(summary, true)
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        assert_eq!(labels, ["+1+2", "*3", "-4"]);
        assert!(chunks(Summary::default(), false).is_empty());
        assert_eq!(
            chunks(
                Summary {
                    unversioned: 2,
                    ..Default::default()
                },
                true
            )[0]
            .0,
            "+2"
        );
    }
}
