//! Shared settings menu for editor tabs and embedded previews.
use gpui_kit::{
    component::{
        Icon, Selectable as _, Sizable as _,
        button::{Button, ButtonCustomVariant, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
    },
    *,
};
use plico::i18n::t;
use plico::layout_preferences::{DiffAppearance, DiffEmphasis};
use std::rc::Rc;

pub fn inline_unit(emphasis: DiffEmphasis) -> Option<gpui_kit::component::diff::DiffInlineUnit> {
    use gpui_kit::component::diff::DiffInlineUnit;
    match emphasis {
        DiffEmphasis::Lines => None,
        DiffEmphasis::Words => Some(DiffInlineUnit::Word),
        DiffEmphasis::Characters => Some(DiffInlineUnit::Character),
    }
}

pub fn appearance_button(
    value: DiffAppearance,
    on_change: impl Fn(DiffAppearance, &mut App) + 'static,
) -> impl IntoElement {
    let on_change = Rc::new(on_change);
    Button::new("diff-settings")
        .ghost()
        .small()
        .icon(Icon::default().path("jb/settings_dark.svg").size(px(16.)))
        .tooltip(t("Settings"))
        .dropdown_menu(move |mut menu, _, _| {
            menu = menu.min_w(px(230.)).label(t("Highlighting Differences"));
            for (label, emphasis) in [
                ("Lines", DiffEmphasis::Lines),
                ("Words", DiffEmphasis::Words),
                ("Characters", DiffEmphasis::Characters),
            ] {
                let on_change = on_change.clone();
                menu = menu.item(
                    PopupMenuItem::new(t(label))
                        .checked(value.emphasis == emphasis)
                        .on_click(move |_, _, cx| {
                            on_change(DiffAppearance { emphasis, ..value }, cx)
                        }),
                );
            }
            menu = menu.separator().label(t("Appearance"));
            for (label, line_numbers, checked) in [
                ("Show Line Numbers", true, value.line_numbers),
                ("Soft-Wrap", false, value.soft_wrap),
            ] {
                let on_change = on_change.clone();
                menu = menu.item(PopupMenuItem::new(t(label)).checked(checked).on_click(
                    move |_, _, cx| {
                        let mut next = value;
                        if line_numbers {
                            next.line_numbers = !next.line_numbers;
                        } else {
                            next.soft_wrap = !next.soft_wrap;
                        }
                        on_change(next, cx);
                    },
                ));
            }
            menu
        })
}

/// The tool preference is retained even when a one-sided file uses a single pane.
pub fn mode_selector(
    value: gpui_kit::component::diff::DiffMode,
    on_change: impl Fn(gpui_kit::component::diff::DiffMode, &mut App) + 'static,
) -> impl IntoElement {
    ModeSelector {
        value,
        on_change: Rc::new(on_change),
    }
}

#[derive(IntoElement)]
struct ModeSelector {
    value: gpui_kit::component::diff::DiffMode,
    on_change: Rc<dyn Fn(gpui_kit::component::diff::DiffMode, &mut App)>,
}

impl RenderOnce for ModeSelector {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::component::diff::DiffMode;
        let value = self.value;
        let on_change = self.on_change;
        let focus = window
            .use_keyed_state("diff-view-mode-focus", cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let keyboard_change = on_change.clone();
        let mut group = div()
            .id("diff-view-mode")
            .relative()
            .flex()
            .h(px(28.))
            .flex_shrink_0()
            .track_focus(&focus)
            .tab_stop(true)
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                let modifiers = &event.keystroke.modifiers;
                if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
                    return;
                }
                let next = match event.keystroke.key.as_str() {
                    "left" => DiffMode::Split,
                    "right" => DiffMode::Unified,
                    _ => return,
                };
                cx.stop_propagation();
                if next != value {
                    keyboard_change(next, cx);
                }
            })
            .child(
                div()
                    .absolute()
                    .size_full()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(if focused { 0x3871e1 } else { 0x40434a })),
            );
        for (id, mode, icon, tooltip) in [
            (
                "side-by-side",
                DiffMode::Split,
                "jb/sideBySide_dark.svg",
                "Side-by-side viewer",
            ),
            (
                "unified",
                DiffMode::Unified,
                "jb/unified_dark.svg",
                "Unified viewer",
            ),
        ] {
            let on_change = on_change.clone();
            let selected = value == mode;
            let focus = focus.clone();
            // Ghost installs its own accent hover during rendering, after caller
            // hover styles. Use the variant's state colors for these segments.
            let variant = ButtonCustomVariant::new(cx)
                .foreground(rgb(0xced0d6).into())
                .hover(rgba(0xffffff17).into())
                .active(rgb(if focused { 0x2e4d89 } else { 0x26282c }).into());
            group = group.child(
                Button::new(id)
                    .custom(variant)
                    .small()
                    .w(px(49.))
                    .h(px(28.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(if selected {
                        rgb(if focused { 0x191a1c } else { 0x5f6269 }).into()
                    } else {
                        transparent_black()
                    })
                    .bg(if selected {
                        rgb(if focused { 0x2e4d89 } else { 0x26282c }).into()
                    } else {
                        transparent_black()
                    })
                    .text_color(rgb(0xced0d6))
                    .child(Icon::default().path(icon).size(px(16.)))
                    .tooltip(t(tooltip))
                    .selected(selected)
                    .toggled(selected)
                    .tab_stop(false)
                    .on_click(move |_, window, cx| {
                        focus.focus(window, cx);
                        if mode != value {
                            on_change(mode, cx);
                        }
                    }),
            );
        }
        group
    }
}
