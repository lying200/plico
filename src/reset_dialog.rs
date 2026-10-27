//! Rebased GitNewResetDialog: target description, four radio choices, Reset/Cancel.
use gpui_kit::component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    radio::Radio,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    i18n::t,
    layout_preferences,
    reset_operation::{Mode, Prepared},
};
use std::sync::Arc;
struct Options {
    mode: Mode,
    error: Option<String>,
}
impl Render for Options {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut choices = v_flex().gap(px(16.));
        for mode in Mode::ALL {
            choices = choices.child(
                v_flex()
                    .gap(px(6.))
                    .child(
                        Radio::new(mode.name())
                            .label(mode.name())
                            .checked(self.mode == mode)
                            .on_change(cx.listener(move |s, _, _, cx| {
                                s.mode = mode;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .pl(rems(2.))
                            .text_size(rems(1.))
                            .line_height(relative(1.5))
                            .text_color(rgb(0x92949a))
                            .whitespace_normal()
                            .child(t(mode.description())),
                    ),
            );
        }
        choices.when_some(self.error.clone(), |v, e| {
            v.child(div().text_color(rgb(0xf2a6a6)).child(e))
        })
    }
}
pub fn show(
    prepared: Arc<Prepared>,
    include_repository: bool,
    window: &mut Window,
    cx: &mut App,
    submit: impl Fn(Mode, &mut Window, &mut App) + 'static,
) {
    let options = cx.new(|_| Options {
        mode: layout_preferences::load_reset_mode(),
        error: None,
    });

    let source = if include_repository {
        let repository = prepared.repository();
        let name = repository
            .path
            .file_name()
            .unwrap_or(repository.path.as_os_str())
            .to_string_lossy();
        if plico::i18n::is_chinese() {
            format!("{}（{}）", prepared.source(), name)
        } else {
            format!("{} in {}", prepared.source(), name)
        }
    } else {
        prepared.source()
    };
    let submit = std::rc::Rc::new(submit);
    window.open_dialog(cx,move|dialog,_,_| {
        let options=options.clone();let confirm=options.clone();let submit=submit.clone();
        dialog.title(gpui_kit::component::h_flex().gap(px(12.)).child(img("branding/plico.svg").size(px(20.))).child(t("Git Reset"))).w(px(650.)).centered(true).overlay_closable(false)
            .child(v_flex().gap(px(16.)).child(div().font_weight(FontWeight::SEMIBOLD).child(format!("{} → {} \"{}\" {} {}",source,&prepared.target[..8],prepared.summary,t("by"),prepared.author)))
                .child(t("This will reset the current branch head to the selected commit,\nand update the working tree and the index according to the selected mode:")).child(options))
            .footer(crate::dialog_style::confirmation(Button::new("reset-ok").primary().label(t("Reset"))))
            .on_ok(move|_,w,cx| {
                let mode=confirm.read(cx).mode;
                if let Err(error)=layout_preferences::save_reset_mode(mode) {
                    confirm.update(cx,|s,cx|{s.error=Some(format!("{}: {error}",t("Settings could not be saved")));cx.notify();});return false;
                }
                submit(mode,w,cx);true
            })
    });
}
