//! Compact read-failure feedback; full diagnostics live in a bounded dialog.
//! Rebased source rules and local adaptations: docs/REBASED-REFERENCE.md.
use plico::{git_error, i18n::t};
use gpui_kit::component::{
    Disableable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogClose, DialogFooter},
    h_flex,
    scroll::{Scrollbar, ScrollbarMode},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

pub fn details_button(id: &'static str, error: String) -> Button {
    Button::new(SharedString::from(format!("{id}-details")))
        .link()
        .small()
        .label(t("Details"))
        .on_click(move |_, window, cx| show_details(error.clone(), "Error details", window, cx))
}

pub fn notice<F>(
    id: &'static str,
    fallback: &'static str,
    error: String,
    busy: bool,
    retry: F,
) -> impl IntoElement
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let summary = if git_error::is_ownership_error(&error) {
        git_error::summary(&error)
    } else {
        fallback
    };
    v_flex()
        .px_3()
        .py_2()
        .gap_2()
        .w_full()
        .min_w_0()
        .text_size(rems(1.))
        .child(div().child(t(summary)))
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new(SharedString::from(format!("{id}-retry")))
                        .link()
                        .small()
                        .label(t("Refresh"))
                        .disabled(busy)
                        .on_click(retry),
                )
                .child(details_button(id, error)),
        )
}

pub fn show_details(error: String, title: &'static str, window: &mut Window, cx: &mut App) {
    let scroll = ScrollHandle::new();
    window.open_dialog(cx, move |dialog, _, _| {
        let copied = error.clone();
        dialog.title(t(title)).w(px(680.)).centered(true)
            .when(git_error::is_ownership_error(&error), |dialog| dialog.child(
                div().text_size(rems(1.)).child(t("Git refused access because the repository owner differs. Review the path and Git's instructions before trusting this directory. No trust configuration has been changed."))))
            .child(div().relative().w_full().min_w_0().max_h(px(280.))
                .child(div().id("error-details-output").w_full().max_h(px(280.)).overflow_y_scroll()
                    .track_scroll(&scroll).pr_3().text_size(rems(1.)).child(error.clone()))
                .child(Scrollbar::vertical(&scroll).mode(ScrollbarMode::Always)))
            .footer(DialogFooter::new()
                .child(Button::new("copy-error-details").min_w(px(72.)).label(t("Copy error details"))
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))))
                .child(DialogClose::new().trigger(|button| button.primary().min_w(px(72.)).label(t("Close")))))
            .on_ok(|_, _, _| true)
    });
}
