//! Application-wide Islands theme ownership and semantic palette.
use gpui_kit::{
    component::{ActiveTheme, Theme, ThemeMode},
    *,
};
use plico::layout_preferences::IslandsTheme;
use std::sync::Arc;

struct Initialized;
impl Global for Initialized {}

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub background: u32,
    pub shell: u32,
    pub panel: u32,
    pub border: u32,
    pub popup_border: u32,
    pub foreground: u32,
    pub muted: u32,
    pub selection: u32,
    pub inactive_selection: u32,
    pub drop_border: u32,
    pub error: u32,
    pub warning: u32,
    pub hover: u32,
}

pub(crate) fn current(cx: &App) -> IslandsTheme {
    if cx.theme().is_dark() {
        IslandsTheme::Dark
    } else {
        IslandsTheme::Light
    }
}

pub(crate) fn palette(cx: &App) -> Palette {
    colors(current(cx))
}

/// Explicit paired colors for surfaces whose existing dark variant differs
/// slightly from the shared palette. Call at render time, not when caching data.
pub(crate) fn color(cx: &App, dark: u32, light: u32) -> u32 {
    if cx.theme().is_dark() { dark } else { light }
}

fn colors(theme: IslandsTheme) -> Palette {
    match theme {
        IslandsTheme::Dark => Palette {
            background: 0x191a1c,
            shell: 0x26282c,
            panel: 0x191a1c,
            border: 0x26282c,
            popup_border: 0x33353b,
            foreground: 0xd1d3d9,
            muted: 0x9fa2a8,
            selection: 0x2e4d89,
            inactive_selection: 0x33353b,
            drop_border: 0x5374a6,
            error: 0xf2a6a6,
            warning: 0xd5b778,
            hover: 0x2b2d30,
        },
        IslandsTheme::Light => Palette {
            background: 0xffffff,
            shell: 0xe9eaee,
            panel: 0xffffff,
            border: 0xe9eaee,
            popup_border: 0xdddfe4,
            foreground: 0x000000,
            muted: 0x5f6269,
            selection: 0xd0dffe,
            inactive_selection: 0xe9eaee,
            drop_border: 0x3871e1,
            error: 0xc54e58,
            warning: 0xa56906,
            hover: 0xf7f8f9,
        },
    }
}

pub(crate) fn ensure_initialized(cx: &mut App) {
    if cx.has_global::<Initialized>() {
        return;
    }
    apply(plico::layout_preferences::IslandsTheme::load(), cx);
    crate::ui_font::Preferences::load().apply(cx);
    cx.set_global(Initialized);
}

/// Persist before publishing the choice, so a failed save leaves the current UI intact.
pub(crate) fn select(theme: IslandsTheme, cx: &mut App) -> std::io::Result<()> {
    if current(cx) == theme {
        return Ok(());
    }
    theme.save()?;
    apply(theme, cx);
    Ok(())
}

fn apply(theme: IslandsTheme, cx: &mut App) {
    let fonts = crate::ui_font::Preferences::current(cx);
    let dark = theme == IslandsTheme::Dark;
    let p = colors(theme);
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    Theme::update(cx, |t| {
        t.background = rgb(p.background).into();
        t.foreground = rgb(p.foreground).into();
        t.border = rgb(p.popup_border).into();
        t.popover = rgb(if dark { p.shell } else { 0xf7f8f9 }).into();
        t.popover_foreground = rgb(p.foreground).into();
        t.accent = rgb(if dark { 0x2a4371 } else { 0xd0dffe }).into();
        t.accent_foreground = rgb(p.foreground).into();
        t.muted_foreground = rgb(p.muted).into();
        t.selection = rgb(p.selection).into();
        t.primary = rgb(if dark { 0x3574f0 } else { 0x3871e1 }).into();
        t.primary_foreground = rgb(0xffffff).into();
        t.button_primary = t.primary;
        t.button_primary_foreground = t.primary_foreground;
        t.button_primary_hover = rgb(if dark { 0x467ff2 } else { 0x2f5eb9 }).into();
        t.button_primary_active = rgb(if dark { 0x315fbd } else { 0x2e4d89 }).into();
        t.ring = t.primary;
        t.font_family = fonts.ui_family.into();
        t.font_size = px(fonts.ui_size as f32);
        t.mono_font_family = fonts.editor_family.into();
        t.mono_font_size = px(fonts.editor_size as f32);
        let highlight = Arc::make_mut(&mut t.highlight_theme);
        highlight.name = theme.label().into();
        highlight.style.editor_background = Some(rgb(p.background).into());
        highlight.style.editor_foreground =
            Some(rgb(if dark { 0xbcbec4 } else { 0x080808 }).into());
        highlight.style.editor_line_number =
            Some(rgb(if dark { 0x4b5059 } else { 0xadadad }).into());
        highlight.style.syntax = serde_json::from_str(if dark {
            include_str!("../assets/themes/islands-syntax.json")
        } else {
            include_str!("../assets/themes/islands-light-syntax.json")
        })
        .expect("Bundled Islands syntax palette");
    });
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use gpui::component::ActiveTheme;

    #[gpui::test]
    fn switching_updates_palette_and_syntax_without_resetting_fonts(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        cx.update(|cx| {
            ensure_initialized(cx);
            Theme::update(cx, |theme| {
                theme.font_size = px(17.);
                theme.mono_font_size = px(19.);
            });
            for choice in [IslandsTheme::Light, IslandsTheme::Dark, IslandsTheme::Light] {
                apply(choice, cx);
                assert_eq!(current(cx), choice);
                assert_eq!(cx.theme().highlight_theme.name.as_str(), choice.label());
                assert_eq!(
                    cx.theme().background,
                    Hsla::from(rgb(colors(choice).background))
                );
                assert_eq!(cx.theme().font_size, px(17.));
                assert_eq!(cx.theme().mono_font_size, px(19.));
                ensure_initialized(cx);
                assert_eq!(current(cx), choice);
            }
        });
    }

    #[gpui::test]
    fn subsequent_window_initialization_preserves_current_theme_and_fonts(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        cx.update(|cx| {
            ensure_initialized(cx);
            Theme::change(ThemeMode::Light, None, cx);
            Theme::update(cx, |theme| {
                theme.font_size = px(17.);
                theme.mono_font_size = px(19.);
            });
            ensure_initialized(cx);
            assert!(!cx.theme().is_dark());
            assert_eq!(cx.theme().font_size, px(17.));
            assert_eq!(cx.theme().mono_font_size, px(19.));
        });
    }
}
