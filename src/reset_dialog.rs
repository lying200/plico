//! Rebased GitNewResetDialog: target description, four radio choices, Reset/Cancel.
use plico::{i18n::t, reset_operation::{Mode,Prepared},layout_preferences};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{WindowExt as _, radio::Radio,v_flex,button::{Button,ButtonVariants as _}};
use std::sync::Arc;
struct Options { mode:Mode, error:Option<String> }
impl Render for Options {
    fn render(&mut self,_:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let mut choices=v_flex().gap_3();
        for mode in Mode::ALL {
            choices=choices.child(v_flex().gap_1().child(Radio::new(mode.name()).label(mode.name()).checked(self.mode==mode)
                .on_change(cx.listener(move|s,_,_,cx|{s.mode=mode;cx.notify();})))
                .child(div().pl(px(28.)).text_color(rgb(0x92949a)).child(t(mode.description()))));
        }
        choices.when_some(self.error.clone(),|v,e|v.child(div().text_color(rgb(0xf2a6a6)).child(e)))
    }
}
pub fn show(prepared:Arc<Prepared>,window:&mut Window,cx:&mut App,submit:impl Fn(Mode,&mut Window,&mut App)+'static) {
    let options=cx.new(|_|Options{mode:layout_preferences::load_reset_mode(),error:None});

    let submit=std::rc::Rc::new(submit);
    window.open_dialog(cx,move|dialog,_,_| {
        let options=options.clone();let confirm=options.clone();let submit=submit.clone();
        dialog.title(t("Git Reset")).w(px(600.)).centered(true).overlay_closable(false)
            .child(v_flex().gap_4().child(format!("{} → {}",prepared.source(),&prepared.target[..8])).child(prepared.summary.clone())
                .child(t("Reset the current branch to the selected commit:" )).child(options))
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
