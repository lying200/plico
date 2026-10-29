use gpui_kit::component::{checkbox::Checkbox, h_flex, radio::Radio, v_flex};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    i18n::t,
    update_preferences::{self, Preferences, Strategy},
};
pub struct Settings {
    path: std::path::PathBuf,
    draft: Preferences,
    saved: Preferences,
    error: Option<String>,
    show_options: bool,
    saved_show_options: bool,
    helper: bool,
    saved_helper: bool,
}
impl Settings {
    pub fn modified(&self) -> bool {
        self.draft != self.saved
            || self.show_options != self.saved_show_options
            || self.helper != self.saved_helper
    }
    pub fn save(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.modified() {
            self.error = None;
            cx.notify();
            return true;
        }
        let result = update_preferences::save(&self.path, self.draft)
            .and_then(|_| {
                plico::project_update_preferences::save_show_options(
                    &self.path.with_file_name("project-update-options"),
                    self.show_options,
                )
            })
            .and_then(|_| {
                if self.helper != self.saved_helper {
                    plico::http_auth::set_use_credential_helper(self.helper)
                        .map_err(std::io::Error::other)
                } else {
                    Ok(())
                }
            });
        match result {
            Ok(()) => {
                self.saved = self.draft;
                self.saved_show_options = self.show_options;
                self.saved_helper = self.helper;
                self.error = None;
                cx.notify();
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
impl Render for Settings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut methods = h_flex().gap_4();
        for (id, strategy, label) in [
            ("merge", Strategy::Merge, "Merge"),
            ("rebase", Strategy::Rebase, "Rebase"),
            (
                "branch-default",
                Strategy::BranchDefault,
                "Follow branch configuration",
            ),
        ] {
            methods = methods.child(
                Radio::new(id)
                    .label(t(label))
                    .checked(self.draft.strategy == strategy)
                    .on_change(cx.listener(move |s, _, _, cx| {
                        s.draft.strategy = strategy;
                        cx.notify();
                    })),
            );
        }
        v_flex()
            .gap_3()
            .child(t("Applies to the current worktree"))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t("Push")))
            .child(
                Checkbox::new("auto-update")
                    .label(t("Auto-update if push is rejected"))
                    .checked(self.draft.auto_update)
                    .on_click(cx.listener(|s, value, _, cx| {
                        s.draft.auto_update = *value;
                        cx.notify();
                    })),
            )
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t("Update")))
            .child(t("Update method"))
            .child(methods)
            .child(
                Checkbox::new("show-project-update-options")
                    .label(t("Show Update Project options"))
                    .checked(self.show_options)
                    .on_click(cx.listener(|s, value, _, cx| {
                        s.show_options = *value;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t("Authentication")),
            )
            .child(
                Checkbox::new("git-credential-helper")
                    .label(t("Use credential helper"))
                    .checked(self.helper)
                    .on_click(cx.listener(|s, value, _, cx| {
                        s.helper = *value;
                        cx.notify();
                    })),
            )
            .child(t("Applies to all repositories"))
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    div()
                        .text_color(rgb(crate::ui_theme::color(cx, 0xf2a6a6, 0xc54e58)))
                        .child(error),
                )
            })
    }
}
pub fn new(
    path: std::path::PathBuf,
    draft: Preferences,
    show_options: bool,
    cx: &mut App,
) -> Entity<Settings> {
    let helper = plico::http_auth::use_credential_helper();
    cx.new(|_| Settings {
        path,
        draft,
        saved: draft,
        error: None,
        show_options,
        saved_show_options: show_options,
        helper,
        saved_helper: helper,
    })
}
