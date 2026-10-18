use plico::{i18n::t, update_preferences::{self, Preferences, Strategy}};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{checkbox::Checkbox, radio::Radio,
    h_flex, v_flex};
pub struct Settings { path: std::path::PathBuf, draft: Preferences, saved: Preferences, error: Option<String>, show_options: bool, saved_show_options: bool }
impl Settings {
    pub fn modified(&self) -> bool { self.draft != self.saved || self.show_options != self.saved_show_options }
    pub fn save(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.modified() { self.error = None; cx.notify(); return true; }
        let result = update_preferences::save(&self.path, self.draft).and_then(|_|
            plico::project_update_preferences::save_show_options(&self.path.with_file_name("project-update-options"), self.show_options));
        match result {
            Ok(()) => { self.saved = self.draft; self.saved_show_options = self.show_options; self.error = None; cx.notify(); true },
            Err(error) => { self.error = Some(format!("{}: {error}", t("Settings could not be saved"))); cx.notify(); false }
        }
    }
}
impl Render for Settings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut methods = h_flex().gap_4();
        for (id, strategy, label) in [("merge", Strategy::Merge, "Merge"), ("rebase", Strategy::Rebase, "Rebase"),
            ("branch-default", Strategy::BranchDefault, "Follow branch configuration")] {
            methods = methods.child(Radio::new(id).label(t(label)).checked(self.draft.strategy == strategy)
                .on_change(cx.listener(move |s, _, _, cx| { s.draft.strategy = strategy; cx.notify(); })));
        }
        v_flex().gap_3().child(t("Applies to the current worktree"))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t("Push")))
            .child(Checkbox::new("auto-update").label(t("Auto-update if push is rejected")).checked(self.draft.auto_update)
                .on_click(cx.listener(|s, value, _, cx| { s.draft.auto_update = *value; cx.notify(); })))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t("Update")))
            .child(t("Update method")).child(methods)
            .child(Checkbox::new("show-project-update-options").label(t("Show Update Project options")).checked(self.show_options)
                .on_click(cx.listener(|s, value, _, cx| { s.show_options = *value; cx.notify(); })))
            .when_some(self.error.clone(), |v, error| v.child(div().text_color(rgb(0xf2a6a6)).child(error)))
    }
}
pub fn new(path: std::path::PathBuf, draft: Preferences, show_options: bool, cx: &mut App) -> Entity<Settings> {
    cx.new(|_| Settings { path, draft, saved: draft, error: None, show_options, saved_show_options: show_options })
}
