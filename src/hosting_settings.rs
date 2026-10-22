//! Account settings follow Rebased AccountsPanelFactory's draft/apply lifecycle.
use crate::{
    hosting_panel::{LoggedIn, Panel},
    ui_style::jb_icon,
};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::{
    Disableable as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::Cancel,
    hosting::{self, Account, Credentials, Provider},
    i18n::t,
};

pub struct Settings {
    provider: Provider,
    project: std::path::PathBuf,
    default: Option<String>,
    saved_default: Option<String>,
    original: Vec<Account>,
    accounts: Vec<Account>,
    selected: Option<Account>,
    credentials: Vec<(Account, Credentials)>,
    login: Option<Entity<Panel>>,
    error: Option<String>,
    loaded: bool,
    oauth: Entity<TextareaState>,
    saved_oauth: String,
    show_oauth: bool,
}
impl Settings {
    pub fn new(
        provider: Provider,
        project: std::path::PathBuf,
        w: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let default = hosting::default_account(provider, &project).unwrap_or_default();
        let result = hosting::accounts();
        let loaded = result.is_ok();
        let error = result.as_ref().err().cloned();
        let accounts: Vec<_> = result
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.provider == provider)
            .collect();
        let saved_oauth = hosting::oauth::custom_config_text().unwrap_or_default();
        let oauth = cx.new(|cx| {
            let mut input = TextareaState::new(w, cx);
            input.set_value(saved_oauth.clone(), w, cx);
            input
        });
        cx.new(|cx| {
            cx.observe(&oauth, |_, _, cx| cx.notify()).detach();
            Self {
                provider,
                project,
                saved_default: default.clone(),
                default,
                original: accounts.clone(),
                accounts,
                selected: None,
                credentials: vec![],
                login: None,
                error,
                loaded,
                oauth,
                saved_oauth,
                show_oauth: false,
            }
        })
    }
    pub fn modified(&self, cx: &App) -> bool {
        self.default != self.saved_default
            || self.accounts != self.original
            || !self.credentials.is_empty()
            || (self.provider == Provider::GitLab
                && self.oauth.read(cx).value().as_ref() != self.saved_oauth)
    }
    pub fn save(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.modified(cx) {
            return true;
        }
        let text = self.oauth.read(cx).value().to_string();
        let result = (|| {
            if self.provider == Provider::GitLab && text != self.saved_oauth {
                hosting::oauth::save_custom_config(&text)?;
                self.saved_oauth = text;
            }
            for (account, credentials) in &self.credentials {
                hosting::save(account, credentials, &Cancel::default())?;
            }
            for account in &self.original {
                if !self.accounts.contains(account) {
                    hosting::remove(account)?;
                }
            }
            if self.default != self.saved_default {
                let account = self.accounts.iter().find(|a| Some(a.id()) == self.default);
                hosting::set_default(self.provider, &self.project, account)?;
                self.saved_default = self.default.clone();
            }
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => {
                self.original = self.accounts.clone();
                self.credentials.clear();
                self.error = None;
            }
            Err(error) => {
                self.error = Some(match error.as_str() {
                    "Use one server URL = client ID per line" => {
                        t("Use one server URL = client ID per line").into()
                    }
                    "Duplicate OAuth server" => t("Duplicate OAuth server").into(),
                    "OAuth configuration is too large" => {
                        t("OAuth configuration is too large").into()
                    }
                    "Use an HTTPS server URL without credentials" => {
                        t("Use an HTTPS server URL without credentials").into()
                    }
                    _ => error,
                });
                cx.notify();
                return false;
            }
        }
        cx.notify();
        true
    }
    fn begin_login(
        &mut self,
        token: bool,
        account: Option<Account>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel = Panel::account_login(self.provider, account, token, w, cx);
        cx.subscribe(&panel, |s, panel, _: &LoggedIn, cx| {
            if let Some((account, credentials)) = panel.update(cx, |p, _| p.take_login()) {
                if !s.accounts.contains(&account) {
                    s.accounts.push(account.clone());
                }
                s.credentials.retain(|(a, _)| a != &account);
                s.credentials.push((account.clone(), credentials));
                s.selected = Some(account);
                s.login = None;
                cx.notify();
            }
        })
        .detach();
        self.login = Some(panel);
        self.error = None;
        cx.notify();
    }
}
impl Render for Settings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(login) = &self.login {
            return v_flex()
                .gap_3()
                .w_full()
                .min_h(px(270.))
                .child(login.clone())
                .child(
                    Button::new("account-login-cancel")
                        .label(t("Cancel"))
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.login = None;
                            cx.notify();
                        })),
                )
                .into_any_element();
        }
        let owner = cx.entity().downgrade();
        let selected = self.selected.clone();
        v_flex().gap_3().w_full()
            .child(v_flex().border_1().border_color(rgb(0x34363a)).min_h(px(260.))
                .child(h_flex().gap_1().p_1().border_b_1().border_color(rgb(0x34363a))
                    .child(Button::new("accounts-add").ghost().small().icon(jb_icon("add_dark.svg")).disabled(!self.loaded).tooltip(t("Add account…"))
                        .dropdown_menu(move |menu, _, _| {
                            let browser = owner.clone(); let token = owner.clone();
                            menu.item(PopupMenuItem::new(t("Log in via browser…")).on_click(move |_, w, cx| { let _ = browser.update(cx, |s, cx| s.begin_login(false, None, w, cx)); }))
                                .item(PopupMenuItem::new(t("Log In with Token…")).on_click(move |_, w, cx| { let _ = token.update(cx, |s, cx| s.begin_login(true, None, w, cx)); }))
                        }))
                    .child(Button::new("accounts-remove").ghost().small().icon(jb_icon("remove_dark.svg")).disabled(selected.is_none()).tooltip(t("Remove account"))
                        .on_click(cx.listener(|s, _, _, cx| { if let Some(account) = s.selected.take() { if s.default.as_deref() == Some(account.id().as_str()) { s.default = None; } s.accounts.retain(|a| a != &account); s.credentials.retain(|(a, _)| a != &account); } cx.notify(); })))
                    .child(Button::new("accounts-default").ghost().small().icon(jb_icon("checked_dark.svg")).disabled(selected.is_none() || self.project.as_os_str().is_empty() || selected.as_ref().is_some_and(|a| Some(a.id()) == self.default)).tooltip(t("Set as default account for current project"))
                        .on_click(cx.listener(|s, _, _, cx| { s.default = s.selected.as_ref().map(Account::id); cx.notify(); })))
                    .child(Button::new("accounts-login-again").ghost().small().icon(jb_icon("edit_dark.svg")).disabled(selected.is_none()).tooltip(t("Log in again"))
                        .on_click(cx.listener(|s, _, w, cx| s.begin_login(false, s.selected.clone(), w, cx)))))
                .when(self.accounts.is_empty(), |v| v.child(v_flex().h(px(220.)).items_center().justify_center().gap_2()
                    .child(div().text_color(rgb(crate::ui_style::MUTED)).child(t("No accounts added")))
                    .child(Button::new("empty-add-account").ghost().label(t("Add account…")).disabled(!self.loaded).on_click(cx.listener(|s, _, w, cx| s.begin_login(false, None, w, cx))))))
                .child(v_flex().id("settings-accounts-list").max_h(px(220.)).overflow_y_scroll()
                .children(self.accounts.iter().map(|account| {
                    let value = account.clone();
                    Button::new(SharedString::from(format!("settings-account:{}", account.id()))).ghost().w_full().h(px(48.)).selected(selected.as_ref() == Some(account))
                        .child(v_flex().w_full().child(format!("{}{}", if self.default.as_deref() == Some(account.id().as_str()) { "✓ " } else { "" }, account.login)).child(div().text_color(rgb(crate::ui_style::MUTED)).child(account.server.trim_start_matches("https://").to_owned())))
                        .on_click(cx.listener(move |s, _, _, cx| { s.selected = Some(value.clone()); cx.notify(); }))
                }))))
            .when(self.provider == Provider::GitLab, |v| v
                .child(Button::new("custom-oauth-expand").ghost().label(t("Custom server OAuth applications")).on_click(cx.listener(|s, _, _, cx| { s.show_oauth = !s.show_oauth; cx.notify(); })))
                .when(self.show_oauth, |v| v.child(Textarea::new(&self.oauth).external_focus_ring(false).h(px(90.)))
                    .child(div().text_color(rgb(crate::ui_style::MUTED)).child(t("Use one server URL = client ID per line")))))
            .when(plico::http_auth::use_credential_helper(), |v| v.child(div().whitespace_normal().text_color(rgb(crate::ui_style::MUTED)).child(t("Git credential helper is enabled; configured accounts are not used for Git operations."))))
            .when_some(self.error.clone(), |v, e| v.child(div().whitespace_normal().text_color(rgb(0xf2a6a6)).child(e)))
            .into_any_element()
    }
}
