//! Rebased GitUpdateOptionsPanel: vertical Merge/Rebase choice.
use gpui_kit::component::{radio::Radio, v_flex};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    i18n::t, project_update::Prepared, project_update_preferences::Preferences,
    update_method::Method,
};

pub struct Options {
    pub request: Option<Prepared>,
    pub method: Method,
    pub show_options: bool,
    preferences: Preferences,
    error: Option<String>,
}
impl Options {
    pub fn new(request: Prepared, preferences: Preferences) -> Self {
        Self {
            request: Some(request),
            method: preferences.method,
            show_options: preferences.show_options,
            preferences,
            error: None,
        }
    }
    pub fn save(&mut self, cx: &mut Context<Self>) -> bool {
        match self.preferences.accept(self.method, self.show_options) {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(format!("{}: {error}", t("Settings could not be saved")));
                cx.notify();
                false
            }
        }
    }
}
impl Render for Options {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut choices = v_flex().gap_3().px(px(8.)).pt(px(8.)).pb(px(12.));
        for (id, method, label) in [
            (
                "update-merge",
                Method::Merge,
                "Merge incoming changes into the current branch",
            ),
            (
                "update-rebase",
                Method::Rebase,
                "Rebase the current branch on top of incoming changes",
            ),
        ] {
            choices = choices.child(
                Radio::new(id)
                    .label(t(label))
                    .checked(self.method == method)
                    .on_change(cx.listener(move |s, _, _, cx| {
                        s.method = method;
                        cx.notify();
                    })),
            );
        }
        choices.when_some(self.error.clone(), |v, error| {
            v.child(div().text_color(rgb(0xf2a6a6)).child(error))
        })
    }
}
