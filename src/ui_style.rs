//! Shared Islands presentation. No main-window state or Git operations.
use gpui_kit::component::{Icon, h_flex, text::TextView, v_flex};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::{self, Area, Change},
    i18n,
};

// Cover opaque child paint at island corners. GPUI overflow clips to a rectangle.
pub(crate) fn island_corners(top: bool, cx: &App) -> Vec<AnyElement> {
    [
        ("ui/corner-bl.svg", false, false),
        ("ui/corner-br.svg", false, true),
        ("ui/corner-tl.svg", true, false),
        ("ui/corner-tr.svg", true, true),
    ]
    .into_iter()
    .filter(|(_, upper, _)| top || !upper)
    .map(|(path, upper, right)| {
        svg()
            .path(path)
            .absolute()
            .size(px(10.))
            .text_color(rgb(crate::ui_theme::palette(cx).shell))
            .when(upper, |corner| corner.top_0())
            .when(!upper, |corner| corner.bottom_0())
            .when(right, |corner| corner.right_0())
            .when(!right, |corner| corner.left_0())
            .into_any_element()
    })
    .collect()
}
pub(crate) fn project_path_text(path: &std::path::Path) -> String {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", unc)
    } else {
        text.trim_start_matches(r"\\?\").to_owned()
    }
}
pub(crate) fn project_identity(path: &std::path::Path) -> String {
    plico::recent_projects::identity(path)
}
pub(crate) fn project_display_path(path: &std::path::Path) -> String {
    let text = project_path_text(path);
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = project_path_text(std::path::Path::new(&home));
        let home = home.trim_end_matches(['/', '\\']);
        if text
            .get(..home.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(home))
        {
            let rest = &text[home.len()..];
            if rest.is_empty() || rest.starts_with(['/', '\\']) {
                return format!("~{}", rest);
            }
        }
    }
    text
}

// Rebased AvatarUtils.initials / RecentProjectIconHelper: title and recent
// entries use the same name badge. Colors stay in app memory; never write .idea.
pub(crate) fn project_avatar(path: &std::path::Path, size: f32) -> impl IntoElement {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let filtered: String = name.chars().filter(|c| c.len_utf16() == 1).collect();
    let camel: String = name
        .chars()
        .skip_while(|c| !c.is_alphabetic())
        .take_while(|c| c.is_alphanumeric())
        .enumerate()
        .filter(|(i, c)| *i == 0 || c.is_uppercase())
        .map(|(_, c)| c)
        .take(2)
        .collect();
    let initials = if camel.chars().count() == 2 {
        camel.to_uppercase()
    } else {
        [' ', ',', '-', '_', '.', '`', '\'', '"']
            .into_iter()
            .find_map(|separator| {
                let words: Vec<String> = filtered
                    .split(separator)
                    .map(|word| {
                        word.chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect::<String>()
                    })
                    .filter(|word| !word.is_empty())
                    .collect();
                (words.len() >= 2).then(|| {
                    format!(
                        "{}{}",
                        words[0].chars().next().unwrap(),
                        words.last().unwrap().chars().next().unwrap()
                    )
                    .to_uppercase()
                })
            })
            .unwrap_or_else(|| camel.to_uppercase())
    };
    let palette = [
        (0xce443c, 0xe77e41),
        (0xe27237, 0xe8a83e),
        (0x2dbcad, 0x35d6a4),
        (0x3895e1, 0x51c5ea),
        (0x7b75e8, 0x7d99eb),
        (0x7854ad, 0x897ae6),
        (0x8f4593, 0xb572e3),
        (0xc840b9, 0xe074ae),
        (0xd75370, 0xe96fa3),
    ];
    let seed = project_identity(path)
        .encode_utf16()
        .fold(0u32, |hash, c| hash.wrapping_mul(31).wrapping_add(c as u32));
    let (start, end) = palette[seed as usize % palette.len()];
    // AvatarUtils.getNewUiFont uses its own Mono demi-bold face, independent
    // of the system UI font. Reserve a 10% inset and an explicit line box so
    // inherited UI line height cannot push the initials against the badge.
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.2))
        .p(px(size * 0.1))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        .font_family("JetBrains Mono")
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px((13. * size / 20.).floor()))
        .line_height(px(size * 0.8))
        .whitespace_nowrap()
        .text_color(rgb(0xffffff))
        .bg(linear_gradient(
            45.,
            linear_color_stop(rgb(end), 0.),
            linear_color_stop(rgb(start), 1.),
        ))
        .child(initials)
}

pub(crate) fn row_selection(focus: &FocusHandle, window: &Window, cx: &App) -> u32 {
    if window.is_window_active() && focus.is_focused(window) {
        if crate::ui_theme::current(cx) == plico::layout_preferences::IslandsTheme::Dark {
            0x2a4371
        } else {
            crate::ui_theme::palette(cx).selection
        }
    } else {
        crate::ui_theme::palette(cx).inactive_selection
    }
}
// Islands Tree.rowHeight and ClassicPainter's 16px control:
// max(leftChildIndent=7, controlWidth/2=8) + rightChildIndent=11.
// Rebased scales Tree.rowHeight with the UI font scale (12pt baseline).
// Keep the ordinary density as a minimum for smaller user-selected fonts.
// Monitor DPI is applied by GPUI, so it must not be multiplied here again.
pub(crate) fn tree_row_height(cx: &App) -> Pixels {
    use gpui_kit::component::ActiveTheme;
    px((f32::from(cx.theme().font_size) * 2.).round().max(24.))
}
// Rebased GraphCommitCellRenderer: content line box + 7px padding,
// with the modern Log theme's 26px minimum. GPUI's line box is 1.5em.
pub(crate) fn log_row_height(cx: &App) -> f32 {
    text_row_height(26., 7., cx).as_f32()
}
pub(crate) const TREE_INDENT: f32 = 19.;
pub(crate) const TREE_INSET: f32 = 12.;
pub(crate) fn tree_selection(color: u32) -> impl IntoElement {
    div()
        .absolute()
        .left(px(TREE_INSET))
        .right(px(TREE_INSET))
        .top_0()
        .h_full()
        .rounded(px(4.))
        .bg(rgb(color))
}

// Rebased MainFrame.setToolbarHeightReferent: all navigation panes start their
// rows below the same toolbar band. Keep this independent of the top tab strip.
pub(crate) const LOG_TOOLBAR_HEIGHT: f32 = 36.;

// IntelliJ IDEA Islands Dark: themes/islands/IslandSchemeDark.xml.
pub(crate) fn change_color(change: &Change, cx: &App) -> u32 {
    let dark = crate::ui_theme::current(cx) == plico::layout_preferences::IslandsTheme::Dark;
    if change.area == Area::Conflict {
        return if dark { 0xde6a66 } else { 0xff0000 };
    }
    if change.area == Area::Untracked {
        return if dark { 0xe88f89 } else { 0x993300 };
    }
    match change.status.as_bytes().first() {
        Some(b'A' | b'C') => {
            if dark {
                0x73bd79
            } else {
                0x0a7700
            }
        }
        Some(b'R') => {
            if dark {
                0x70aeff
            } else {
                0x007c7c
            }
        }
        Some(b'M' | b'T') => {
            if dark {
                0x70aeff
            } else {
                0x0032a0
            }
        }
        Some(b'D') => {
            if dark {
                0x6f737a
            } else {
                0x616161
            }
        }
        _ => crate::ui_theme::palette(cx).foreground,
    }
}
pub(crate) fn jb_icon(name: &'static str) -> Icon {
    Icon::default().path(format!("jb/{name}")).size(px(16.))
}
pub(crate) fn themed_asset(asset: &'static str, cx: &App) -> &'static str {
    if crate::ui_theme::current(cx) == plico::layout_preferences::IslandsTheme::Dark {
        return asset;
    }
    match asset {
        "jb/file-anyType_dark_4x.svg" => "jb/light/file-anyType_4x.svg",
        "jb/file-text_dark_4x.svg" => "jb/light/file-text_4x.svg",
        "jb/file-config_dark_4x.svg" => "jb/light/file-config_4x.svg",
        "jb/file-json_dark_4x.svg" => "jb/light/file-json_4x.svg",
        "jb/file-markdown_dark_4x.svg" => "jb/light/file-markdown_4x.svg",
        "jb/file-java_dark_4x.svg" => "jb/light/file-java_4x.svg",
        "jb/file-javaScript_dark_4x.svg" => "jb/light/file-javaScript_4x.svg",
        "jb/file-typeScript_dark_4x.svg" => "jb/light/file-typeScript_4x.svg",
        "jb/file-xml_dark_4x.svg" => "jb/light/file-xml_4x.svg",
        "jb/file-yaml_dark_4x.svg" => "jb/light/file-yaml_4x.svg",
        "jb/file-c_dark_4x.svg" => "jb/light/file-c_4x.svg",
        "jb/file-cpp_dark_4x.svg" => "jb/light/file-cpp_4x.svg",
        "jb/file-shell_dark_4x.svg" => "jb/light/file-shell_4x.svg",
        "jb/file-python_dark_4x.svg" => "jb/light/file-python_4x.svg",
        "jb/file-rust_dark_4x.svg" => "jb/light/file-rust_4x.svg",
        "jb/folder_dark.svg" | "jb/folder_dark_4x.svg" => "jb/light/folder_4x.svg",
        _ => asset,
    }
}
pub(crate) fn file_icon(path: &std::path::Path, cx: &App) -> &'static str {
    let asset = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" | "log" => "jb/file-text_dark_4x.svg",
        "rs" => "jb/file-rust_dark_4x.svg",
        "py" | "pyi" => "jb/file-python_dark_4x.svg",
        "c" | "h" => "jb/file-c_dark_4x.svg",
        "cpp" | "cc" | "cxx" | "hpp" => "jb/file-cpp_dark_4x.svg",
        "sh" | "bash" | "zsh" => "jb/file-shell_dark_4x.svg",
        "md" | "markdown" => "jb/file-markdown_dark_4x.svg",
        "json" | "jsonc" => "jb/file-json_dark_4x.svg",
        "yaml" | "yml" => "jb/file-yaml_dark_4x.svg",
        "toml" | "ini" | "properties" | "cfg" | "conf" => "jb/file-config_dark_4x.svg",
        "java" => "jb/file-java_dark_4x.svg",
        "js" | "jsx" | "mjs" | "cjs" => "jb/file-javaScript_dark_4x.svg",
        "ts" | "tsx" => "jb/file-typeScript_dark_4x.svg",
        "xml" | "svg" => "jb/file-xml_dark_4x.svg",
        _ => "jb/file-anyType_dark_4x.svg",
    };
    themed_asset(asset, cx)
}
// Copyright 2000-2023 JetBrains s.r.o. and contributors.
// Geometry and layering adapted from Rebased LabelIcon / TagPainter (Apache-2.0).
pub(crate) fn ref_color(kind: git::RefKind, cx: &App) -> u32 {
    match kind {
        git::RefKind::Head => crate::ui_theme::color(cx, 0xf5d273, 0xffaf0f),
        git::RefKind::Local => crate::ui_theme::color(cx, 0x5fad65, 0x369650),
        git::RefKind::Remote => crate::ui_theme::color(cx, 0xb589ec, 0x834df0),
        git::RefKind::Tag => crate::ui_theme::color(cx, 0x868a91, 0x6c707e),
    }
}
pub(crate) fn reference_icon(colors: Vec<u32>, background: u32) -> impl IntoElement {
    let width = 16. + 5.12 * colors.len().saturating_sub(1) as f32;
    let mut layers = Vec::new();
    for (i, color) in colors.iter().rev().enumerate() {
        let offset = 5.12 * i as f32;
        let last = i + 1 == colors.len();
        // Cached SVG masks retain TagPainter's curves and opaque layering,
        // without adding per-frame path-rendering resources on Windows.
        layers.push(
            jb_icon("reference-mask.svg")
                .absolute()
                .left(px(offset))
                .top_0()
                .text_color(rgb(background))
                .into_any_element(),
        );
        layers.push(
            jb_icon(if last {
                "reference-front.svg"
            } else {
                "reference-outline.svg"
            })
            .absolute()
            .left(px(offset))
            .top_0()
            .text_color(rgb(*color))
            .into_any_element(),
        );
        if !last {
            layers.push(
                jb_icon("reference-mask.svg")
                    .absolute()
                    .left(px(5.12 * (i + 1) as f32 - std::f32::consts::SQRT_2))
                    .top_0()
                    .text_color(rgb(background))
                    .into_any_element(),
            );
        }
    }
    div()
        .relative()
        .w(px(width))
        .h(px(16.))
        .flex_shrink_0()
        .children(layers)
}

pub(crate) fn commit_refs(references: &[git::RefLabel], cx: &App) -> AnyElement {
    v_flex()
        .flex_shrink_0()
        .gap_0()
        .when(!references.is_empty(), |v| v.mb(px(10.)))
        .children(
            [false, true]
                .into_iter()
                .filter_map(|tags| {
                    let refs: Vec<_> = references
                        .iter()
                        .filter(|r| (r.kind == git::RefKind::Tag) == tags)
                        .take(100)
                        .collect();
                    if refs.is_empty() {
                        return None;
                    }
                    let total = references
                        .iter()
                        .filter(|r| (r.kind == git::RefKind::Tag) == tags)
                        .count();
                    let mut items = Vec::new();
                    for kind in [
                        git::RefKind::Head,
                        git::RefKind::Local,
                        git::RefKind::Remote,
                        git::RefKind::Tag,
                    ] {
                        let group: Vec<_> = refs.iter().filter(|r| r.kind == kind).collect();
                        for (index, reference) in group.iter().enumerate() {
                            let text = format!(
                                "{}{}",
                                reference.name,
                                if index + 1 < group.len() { "," } else { "" }
                            );
                            items.push(
                                h_flex()
                                    .gap(px(2.))
                                    .max_w_full()
                                    .min_w_0()
                                    .when(index == 0, |d| {
                                        d.child(reference_icon(
                                            vec![ref_color(kind, cx); group.len().min(2)],
                                            crate::ui_theme::palette(cx).panel,
                                        ))
                                    })
                                    .child(
                                        TextView::html(
                                            format!("detail-ref-{kind:?}-{}", reference.name),
                                            text.replace('&', "&amp;")
                                                .replace('<', "&lt;")
                                                .replace('>', "&gt;"),
                                        )
                                        .selectable(true),
                                    )
                                    .into_any_element(),
                            );
                        }
                    }
                    if total > refs.len() {
                        items.push(
                            div()
                                .child(i18n::message(i18n::Message::MoreReferences(
                                    total - refs.len(),
                                )))
                                .into_any_element(),
                        );
                    }
                    Some(
                        h_flex()
                            .w_full()
                            .flex_shrink_0()
                            .flex_wrap()
                            .gap_x(px(4.))
                            .children(items),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}
// IslandsTabPainter: rounded selected fill/border, no underline, 28 px content.
pub(crate) fn text_row_height(minimum: f32, padding: f32, cx: &App) -> Pixels {
    use gpui_kit::component::ActiveTheme;
    px((f32::from(cx.theme().font_size) * 1.5 + padding)
        .ceil()
        .max(minimum))
}
pub(crate) fn tab_height(cx: &App) -> Pixels {
    text_row_height(28., 10., cx)
}
pub(crate) fn islands_tab(id: impl Into<ElementId>, selected: bool, cx: &App) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(tab_height(cx))
        .flex_shrink_0()
        .px(px(8.))
        .gap(px(6.))
        .rounded(px(5.))
        .border_1()
        .border_color(transparent_black())
        .cursor_pointer()
        .text_color(rgb(crate::ui_theme::palette(cx).foreground))
        .when(selected, |tab| {
            tab.bg(rgb(
                if crate::ui_theme::current(cx) == plico::layout_preferences::IslandsTheme::Dark {
                    0x233558
                } else {
                    0xe3ebfe
                },
            ))
            .border_color(rgb(crate::ui_theme::palette(cx).selection))
        })
        .when(!selected, |tab| {
            tab.hover(|style| style.bg(rgb(crate::ui_theme::palette(cx).shell)))
        })
}

/// Application presentation over the shared checkbox's controlled state and events.
pub(crate) fn checkbox(
    id: impl Into<ElementId>,
    cx: &App,
) -> gpui_kit::component::checkbox::Checkbox {
    let light = crate::ui_theme::current(cx) == plico::layout_preferences::IslandsTheme::Light;
    gpui_kit::component::checkbox::Checkbox::new(id).indicator(move |checked, mixed, disabled| {
        let asset = match (checked, mixed, disabled) {
            (_, true, true) => "jb/checkBoxIndeterminateSelectedDisabled.svg",
            (_, true, false) => "jb/checkBoxIndeterminateSelected.svg",
            (true, false, true) => "jb/checkBoxSelectedDisabled.svg",
            (true, false, false) => "jb/checkBoxSelected.svg",
            (false, false, true) => "jb/checkBoxDisabled.svg",
            (false, false, false) => "jb/checkBox.svg",
        };
        let asset = if light {
            asset.replacen("jb/", "jb/light/", 1)
        } else {
            asset.to_owned()
        };
        img(SharedString::from(asset))
            .size(px(16.))
            .into_any_element()
    })
}

pub(crate) fn file_count(count: usize) -> String {
    if i18n::is_chinese() {
        format!("{count} 个文件")
    } else {
        format!("{count} {}", if count == 1 { "file" } else { "files" })
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod popup_bounds_tests {
    use super::*;
    use core::prelude::v1::test;
    use gpui_kit::base::ElementExt as _;
    extern crate gpui_kit as gpui;
    use gpui::component::{
        Root,
        menu::{PopupMenu, PopupMenuItem},
    };
    use std::{cell::Cell, rc::Rc};

    struct MenuFixture {
        menu: Entity<PopupMenu>,
        bounds: Rc<Cell<Bounds<Pixels>>>,
    }
    impl Render for MenuFixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let bounds = self.bounds.clone();
            div().size_full().child(
                div()
                    .absolute()
                    .child(self.menu.clone())
                    .on_prepaint(move |rect, _, _| bounds.set(rect)),
            )
        }
    }
    #[gpui::test]
    fn long_default_popup_is_constrained_without_caller_opt_in(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let (_, cx) = cx.add_window_view(|w, cx| {
            let menu = PopupMenu::build(w, cx, |mut menu, _, _| {
                for i in 0..200 {
                    menu = menu.item(PopupMenuItem::new(format!("branch-{i}")));
                }
                menu
            });
            let view = cx.new(|_| MenuFixture {
                menu,
                bounds: bounds.clone(),
            });
            Root::new(view, w, cx)
        });
        cx.update(|w, cx| {
            w.draw(cx).clear(cx);
            let height = bounds.get().size.height;
            assert!(height > px(100.), "menu must actually be laid out");
            assert!(
                height <= (w.viewport_size().height * 0.5).min(px(450.)) + px(4.),
                "menu grew to {height:?}"
            );
        });
    }
}
