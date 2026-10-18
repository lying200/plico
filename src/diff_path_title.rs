// Separator selection adapted from JetBrains SplitBySeparatorPolicy.java (Apache-2.0).
// Copyright 2000-2023 JetBrains s.r.o. and contributors.
// Reference: detachhead/rebased 4aec35256d765c217aa0d662b82ac4e860c396fd,
// platform/util/ui/src/com/intellij/util/ui/SplitBySeparatorPolicy.java.
//! Rebased's separator-based middle shortening, measured with the active UI font.
use gpui_kit::*;
use gpui_kit::component::tooltip::Tooltip;

pub fn path_title(path: String, full_path: String) -> impl IntoElement {
    div().id(SharedString::from(format!("diff-path-{path}"))).flex_1().min_w_0()
        .h_full().overflow_hidden()
        .tooltip(move |window, cx| {
            let text = full_path.clone();
            let width = (window.viewport_size().width - px(48.)).min(px(600.)).max(px(120.));
            Tooltip::element(move |_, _| div().w(width).whitespace_normal().child(text.clone()))
                .build(window, cx)
        })
        .child(canvas(move |bounds, window, _| {
            let style = window.text_style();
            let size = style.font_size.to_pixels(window.rem_size());
            let shape = |text: String| {
                let run = TextRun { len: text.len(), font: style.font(), color: style.color,
                    background_color: None, underline: None, strikethrough: None };
                window.text_system().shape_line(text.into(), size, &[run], None)
            };
            let full = shape(path.clone());
            if full.width <= bounds.size.width { return full; }
            let candidates = shortened_paths(&path);
            let mut chosen = shape(candidates[0].clone());
            for candidate in candidates.into_iter().skip(1) {
                let line = shape(candidate);
                if line.width > bounds.size.width { break; }
                chosen = line;
            }
            chosen
        }, |bounds, line, window, cx| {
            let _ = line.paint(bounds.origin, bounds.size.height, TextAlign::Left, None, window, cx);
        }).size_full())
}

// Add whole components alternately from the filename and path start, including
// separators. Keep the filename even when it alone exceeds the available width.
fn shortened_paths(path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    for (offset, ch) in path.char_indices() {
        if ch == '/' || ch == '\\' {
            if offset > start { parts.push(&path[start..offset]); }
            parts.push(&path[offset..offset + ch.len_utf8()]);
            start = offset + ch.len_utf8();
        }
    }
    if start < path.len() { parts.push(&path[start..]); }
    let filename = parts.last().copied().unwrap_or(path);
    let mut result = vec![filename.to_owned()];
    let (mut left, mut right) = (0, parts.len());
    let (mut prefix, mut suffix) = (String::new(), String::new());
    let mut take_end = true;
    while left < right {
        if take_end { right -= 1; suffix.insert_str(0, parts[right]); }
        else { prefix.push_str(parts[left]); left += 1; }
        if left < right { result.push(format!("{prefix}...{suffix}")); }
        take_end = !take_end;
    }
    result
}
