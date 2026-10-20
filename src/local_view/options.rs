use super::*;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use plico::layout_preferences::{self, ChangesOption};
impl LocalView {
    pub(super) fn toggle_directory(&mut self, cx: &mut Context<Self>) {
        let value = !self.model.grouped();
        if let Err(e) = layout_preferences::save_changes_option(
            &self.repo.path,
            ChangesOption::Directory,
            value,
        ) {
            self.notice = Some(e.to_string());
        } else {
            self.model.set_grouped(value);
        }
        cx.notify();
    }
    fn toggle_ignored(&mut self, cx: &mut Context<Self>) {
        if self.writing() {
            return;
        }
        let value = !self.show_ignored;
        if let Err(e) =
            layout_preferences::save_changes_option(&self.repo.path, ChangesOption::Ignored, value)
        {
            self.notice = Some(e.to_string());
        } else {
            self.show_ignored = value;
            if !value {
                self.model.replace(
                    self.model
                        .files()
                        .iter()
                        .filter(|f| f.area != plico::git::Area::Ignored)
                        .cloned()
                        .collect(),
                );
            }
            self.refresh
                .update(cx, |q, cx| q.set_show_ignored(value, cx));
        }
        cx.notify();
    }
    pub(super) fn view_options(&self, cx: &Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let grouped = self.model.grouped();
        let ignored = self.show_ignored;
        let writing = self.writing();
        Button::new("local-view-options")
            .ghost()
            .small()
            .icon(
                Icon::default()
                    .path("jb/history-show_dark.svg")
                    .size(px(16.)),
            )
            .tooltip(t("View Options"))
            .dropdown_menu(move |menu, _, _| {
                let directory = owner.clone();
                let ignored_owner = owner.clone();
                menu.label(t("Group By"))
                    .item(
                        PopupMenuItem::new(t("Directory · Ctrl+Alt+P"))
                            .checked(grouped)
                            .on_click(move |_, _, cx| {
                                let _ = directory.update(cx, |s, cx| s.toggle_directory(cx));
                            }),
                    )
                    .separator()
                    .label(t("Show"))
                    .item(
                        PopupMenuItem::new(t("Ignored Files"))
                            .checked(ignored)
                            .disabled(writing)
                            .on_click(move |_, _, cx| {
                                let _ = ignored_owner.update(cx, |s, cx| s.toggle_ignored(cx));
                            }),
                    )
            })
    }
}
