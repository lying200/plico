//! Clone hosting page: login lifecycle and repository selection, not Git execution.
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::Cancel,
    hosting::{self, Account, Credentials, Provider, Repository, oauth},
    i18n::t,
};
use std::collections::HashMap;
use std::sync::{Arc, mpsc};
#[derive(Clone)]
pub struct Selection(pub Option<String>);
impl EventEmitter<Selection> for Panel {}
pub struct LoggedIn;
impl EventEmitter<LoggedIn> for Panel {}
enum ResultData {
    Login(Account, Credentials),
    Repositories(Vec<Repository>, u32),
}
pub struct Panel {
    provider: Provider,
    server: Entity<InputState>,
    secret: Entity<InputState>,
    search: Entity<InputState>,
    accounts: Vec<Account>,
    selected: Option<Account>,
    repositories: Vec<Repository>,
    selected_url: Option<String>,
    page: u32,
    more: bool,
    token_ui: bool,
    custom_server_ui: bool,
    login_ui: bool,
    busy: bool,
    error: Option<String>,
    cancel: Cancel,
    task: Task<()>,
    avatars: HashMap<String, Arc<Image>>,
    avatar_task: Task<()>,
    avatar_cancel: Cancel,
    login_only: bool,
    staged: Option<(Account, Credentials)>,
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.avatar_cancel.cancel();
    }
}
impl Panel {
    pub fn shows_repositories(&self) -> bool {
        !self.login_ui
    }

    pub fn account_label(&self, provider: Provider) -> String {
        let names: Vec<_> = self
            .accounts
            .iter()
            .filter(|a| a.provider == provider)
            .map(|a| a.login.clone())
            .collect();
        if names.is_empty() {
            t("No accounts").into()
        } else {
            names.join(", ")
        }
    }

    pub fn new(w: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let search = cx.new(|cx| InputState::new(w, cx).placeholder(t("Search repositories")));
            cx.subscribe(&search, |s: &mut Self, _, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    let filter = s.search.read(cx).value().to_lowercase();
                    if !s.repositories.iter().any(|r| {
                        Some(&r.url) == s.selected_url.as_ref()
                            && (r.name.to_lowercase().contains(&filter)
                                || r.url.to_lowercase().contains(&filter))
                    }) {
                        s.selected_url = None;
                        cx.emit(Selection(None));
                    }
                    cx.notify();
                }
            })
            .detach();
            Self {
                provider: Provider::GitHub,
                server: cx
                    .new(|cx| InputState::new(w, cx).default_value(Provider::GitHub.server())),
                secret: cx.new(|cx| {
                    let mut i = InputState::new(w, cx);
                    i.set_masked(true, w, cx);
                    i
                }),
                search,
                accounts: hosting::accounts().unwrap_or_default(),
                selected: None,
                repositories: vec![],
                selected_url: None,
                page: 0,
                more: false,
                token_ui: false,
                custom_server_ui: false,
                login_ui: true,
                busy: false,
                error: None,
                cancel: Cancel::default(),
                task: Task::ready(()),
                avatars: HashMap::new(),
                avatar_task: Task::ready(()),
                avatar_cancel: Cancel::default(),
                login_only: false,
                staged: None,
            }
        })
    }
    pub fn account_login(
        provider: Provider,
        account: Option<Account>,
        token: bool,
        w: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let panel = Self::new(w, cx);
        panel.update(cx, |s, cx| {
            s.login_only = true;
            s.provider = provider;
            s.server.update(cx, |v, cx| {
                v.set_value(
                    account
                        .as_ref()
                        .map(|a| a.server.as_str())
                        .unwrap_or(provider.server()),
                    w,
                    cx,
                )
            });
            s.selected = account;
            s.token_ui = token;
        });
        panel
    }
    pub fn take_login(&mut self) -> Option<(Account, Credentials)> {
        self.staged.take()
    }
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.task = Task::ready(());
        self.busy = false;
        cx.notify();
    }
    pub fn switch(&mut self, provider: Provider, w: &mut Window, cx: &mut Context<Self>) {
        self.cancel(cx);
        self.provider = provider;
        self.server
            .update(cx, |s, cx| s.set_value(provider.server(), w, cx));
        self.secret.update(cx, |s, cx| s.set_value("", w, cx));
        self.token_ui = false;
        self.custom_server_ui = false;
        self.error = None;
        self.repositories.clear();
        self.selected_url = None;
        cx.emit(Selection(None));
        self.selected = self
            .accounts
            .iter()
            .find(|a| a.provider == provider)
            .cloned();
        self.login_ui = self.selected.is_none();
        if self.selected.is_some() {
            self.load(false, w, cx)
        }
        cx.notify();
    }
    fn back(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.token_ui = false;
        self.custom_server_ui = false;
        if !self.login_only && self.accounts.iter().any(|a| a.provider == self.provider) {
            self.switch(self.provider, w, cx);
        } else {
            cx.notify();
        }
    }
    fn custom_server(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.selected = None;
        self.login_ui = true;
        self.custom_server_ui = true;
        self.token_ui = false;
        self.error = None;
        cx.emit(Selection(None));
        self.server.read(cx).focus_handle(cx).focus(w, cx);
        cx.notify();
    }
    fn work(
        &mut self,
        w: &mut Window,
        cx: &mut Context<Self>,
        job: impl FnOnce(Cancel, mpsc::Sender<String>) -> Result<ResultData, String> + Send + 'static,
    ) {
        self.cancel(cx);
        self.cancel = Cancel::default();
        self.busy = true;
        self.error = None;
        let token = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        let (urls, url_rx) = mpsc::channel();
        cx.background_spawn(async move {
            let result = job(token, urls);
            let _ = tx.send(result);
        })
        .detach();
        self.task = cx.spawn_in(w, async move |this, wx| {
            loop {
                while let Ok(url) = url_rx.try_recv() {
                    let _ = wx.update(|_, cx| cx.open_url(&url));
                }
                match rx.try_recv() {
                    Ok(result) => {
                        let _ = this.update_in(wx, |s, w, cx| {
                            s.busy = false;
                            if s.cancel.cancelled() {
                                return;
                            }
                            match result {
                                Ok(ResultData::Login(account, credentials)) => {
                                    if s.login_only {
                                        s.staged = Some((account, credentials));
                                        s.secret.update(cx, |v, cx| v.set_value("", w, cx));
                                        cx.emit(LoggedIn);
                                        return;
                                    }
                                    s.accounts = hosting::accounts().unwrap_or_default();
                                    s.selected = Some(account);
                                    s.login_ui = false;
                                    s.secret.update(cx, |v, cx| v.set_value("", w, cx));
                                    s.load(false, w, cx);
                                }
                                Ok(ResultData::Repositories(items, page)) => {
                                    s.more = items.len() == 100;
                                    s.page = page;
                                    if page == 1 {
                                        s.repositories.clear();
                                    }
                                    for item in items {
                                        if !s.repositories.iter().any(|r| r.url == item.url) {
                                            s.repositories.push(item)
                                        }
                                    }
                                }
                                Err(error) => s.error = Some(error_text(error)),
                            }
                            cx.notify();
                        });
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => break,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(40))
                    .await;
            }
        });
        cx.notify();
    }
    fn load_avatar(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(account) = self.selected.clone() else {
            return;
        };
        let id = account.id();
        if self.avatars.contains_key(&id) {
            return;
        }
        self.avatar_cancel.cancel();
        self.avatar_cancel = Cancel::default();
        let cancel = self.avatar_cancel.clone();
        let job = cx.background_spawn(async move { hosting::avatar(&account, &cancel) });
        self.avatar_task = cx.spawn_in(w, async move |this, wx| {
            if let Ok((mime, bytes)) = job.await {
                if let Some(format) = ImageFormat::from_mime_type(&mime) {
                    let _ = this.update(wx, |s, cx| {
                        if s.selected.as_ref().is_some_and(|a| a.id() == id) {
                            s.avatars
                                .insert(id, Arc::new(Image::from_bytes(format, bytes)));
                            cx.notify();
                        }
                    });
                }
            }
        });
    }
    fn load(&mut self, more: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = self.selected.clone() else {
            return;
        };
        if !more {
            self.load_avatar(w, cx);
        }
        let page = if more { self.page + 1 } else { 1 };
        self.work(w, cx, move |c, _| {
            hosting::repositories(&a, page, &c).map(|v| ResultData::Repositories(v, page))
        });
    }
    fn login(&mut self, browser: bool, w: &mut Window, cx: &mut Context<Self>) {
        let provider = self.provider;
        let server = self.server.read(cx).value().trim().to_string();
        let token = self.secret.read(cx).value().to_string();
        let expected = self.selected.clone().filter(|_| self.login_ui);
        let persist = !self.login_only;
        if !browser && token.trim().is_empty() {
            self.error = Some(t("Token must not be empty").into());
            cx.notify();
            return;
        }
        self.work(w, cx, move |cancel, urls| {
            let credentials = if browser {
                let config = oauth::config(provider, &server)?;
                oauth::authorize(provider, &server, &config, &cancel, |u| {
                    urls.send(u).map_err(|_| "Cancelled".into())
                })?
            } else {
                Credentials::token(token)
            };
            let account = hosting::validate_token(provider, &server, &credentials, &cancel)?;
            if expected.as_ref().is_some_and(|a| a != &account) {
                return Err("Token belongs to a different account".into());
            }
            if persist {
                hosting::save(&account, &credentials, &cancel)?;
            }
            Ok(ResultData::Login(account, credentials))
        });
    }
    fn field(label: &str, input: &Entity<InputState>, disabled: bool) -> impl IntoElement {
        h_flex()
            .gap_3()
            .items_center()
            .child(div().w(px(56.)).flex_shrink_0().child(label.to_string()))
            .child(
                Input::new(input)
                    .external_focus_ring(false)
                    .disabled(disabled)
                    .flex_1(),
            )
    }
}
impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = v_flex().size_full().gap_3().min_w_0();
        if self.login_ui {
            content = content.child(div().text_size(cx.theme().font_size * 1.5).child(format!(
                "{} {}",
                t("Log in to"),
                self.provider.name()
            )));
            if self.custom_server_ui || self.token_ui {
                content = content.child(Self::field(
                    t("Server"),
                    &self.server,
                    self.busy || self.provider == Provider::GitHub || self.selected.is_some(),
                ));
            }
            if self.token_ui {
                content = content
                    .child(h_flex().gap_3().items_center()
                        .child(div().w(px(56.)).flex_shrink_0().child("Token:"))
                        .child(Input::new(&self.secret).external_focus_ring(false).disabled(self.busy).flex_1())
                        .child(Button::new("generate-host-token").label(t("Generate…")).disabled(self.busy).on_click(cx.listener(|s, _, _, cx| {
                            if let Ok(host) = hosting::server(s.provider, s.server.read(cx).value().as_ref()) {
                                cx.open_url(&match s.provider {
                                    Provider::GitHub => format!("{host}/settings/tokens/new?scopes=repo,read:user,read:org,workflow&description=Plico"),
                                    Provider::GitLab => format!("{host}/-/user_settings/personal_access_tokens?name=Plico&scopes=read_api,read_user,write_repository"),
                                });
                            }
                        }))))
                    .child(div().ml(px(68.)).text_color(rgb(crate::ui_theme::palette(cx).muted)).whitespace_normal().child(t(match self.provider {
                        Provider::GitHub => "Token scopes requested by Generate: repo, read:user, read:org, workflow",
                        Provider::GitLab => "Token scopes requested by Generate: read_api, read_user, write_repository",
                    })))
                    .child(h_flex().ml(px(68.)).mt_2().gap_3()
                        .child(Button::new("host-login").label(t("Log In")).disabled(self.busy).on_click(cx.listener(|s, _, w, cx| s.login(false, w, cx))))
                        .child(Button::new("host-back").ghost().text_color(rgb(crate::ui_theme::color(cx, 0x6da1f7, 0x2f5eb9))).label(t("Back")).disabled(self.busy).on_click(cx.listener(|s, _, w, cx| s.back(w, cx)))));
            } else if !self.busy {
                content = content.child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Button::new("host-browser-login")
                                .label(if self.provider == Provider::GitLab {
                                    t(if self.custom_server_ui {
                                        "Log In"
                                    } else {
                                        "Log In to gitlab.com…"
                                    })
                                    .to_owned()
                                } else {
                                    format!("{} {}…", t("Log in via"), self.provider.name())
                                })
                                .disabled(self.busy)
                                .on_click(cx.listener(|s, _, w, cx| {
                                    if s.provider == Provider::GitLab
                                        && !s.custom_server_ui
                                        && s.selected.is_none()
                                    {
                                        s.server.update(cx, |v, cx| {
                                            v.set_value(Provider::GitLab.server(), w, cx)
                                        });
                                    }
                                    s.login(true, w, cx);
                                })),
                        )
                        .when(
                            self.provider == Provider::GitLab && !self.custom_server_ui,
                            |row| {
                                let entity = cx.entity().downgrade();
                                row.child(
                                    Button::new("gitlab-custom-server")
                                        .ghost()
                                        .label("⌄")
                                        .disabled(self.busy)
                                        .dropdown_menu(move |menu, _, _| {
                                            let entity = entity.clone();
                                            menu.item(
                                                PopupMenuItem::new(t("Log In to Custom Server…"))
                                                    .on_click(move |_, w, cx| {
                                                        let _ = entity.update(cx, |s, cx| {
                                                            s.custom_server(w, cx)
                                                        });
                                                    }),
                                            )
                                        }),
                                )
                            },
                        )
                        .child(t("or"))
                        .child(
                            Button::new("host-token-login")
                                .ghost()
                                .text_color(rgb(crate::ui_theme::color(cx, 0x6da1f7, 0x2f5eb9)))
                                .label(t("Use token…"))
                                .disabled(self.busy)
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.token_ui = true;
                                    s.error = None;
                                    s.secret.read(cx).focus_handle(cx).focus(w, cx);
                                    cx.notify();
                                })),
                        ),
                );
                if !self.login_only
                    && (self.selected.is_some()
                        || self.custom_server_ui
                        || self.accounts.iter().any(|a| a.provider == self.provider))
                {
                    content = content.child(
                        h_flex().child(
                            Button::new("host-back")
                                .ghost()
                                .text_color(rgb(crate::ui_theme::color(cx, 0x6da1f7, 0x2f5eb9)))
                                .label(t("Back"))
                                .disabled(self.busy)
                                .on_click(cx.listener(|s, _, w, cx| s.back(w, cx))),
                        ),
                    );
                }
            }
        } else {
            let entity = cx.entity().downgrade();
            let accounts = self
                .accounts
                .iter()
                .filter(|a| a.provider == self.provider)
                .cloned()
                .collect::<Vec<_>>();
            let avatar = self
                .selected
                .as_ref()
                .and_then(|a| self.avatars.get(&a.id()))
                .cloned();
            let menu_avatars = self.avatars.clone();
            let provider = self.provider;
            content = content.child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(Input::new(&self.search).external_focus_ring(false).flex_1())
                    .child(
                        div()
                            .w(px(1.))
                            .h(px(24.))
                            .bg(rgb(crate::ui_theme::palette(cx).border)),
                    )
                    .child(
                        Button::new("host-accounts")
                            .ghost()
                            .child(account_avatar(avatar.clone(), 24.))
                            .disabled(self.busy)
                            .dropdown_menu(move |mut menu, _, _| {
                                for account in &accounts {
                                    let account = account.clone();
                                    let selected_account = account.clone();
                                    let select = entity.clone();
                                    let image = menu_avatars.get(&account.id()).cloned();
                                    menu = menu.item(
                                        PopupMenuItem::element(move |_, cx| {
                                            h_flex()
                                                .font_family(cx.theme().font_family.clone())
                                                .text_size(cx.theme().font_size)
                                                .gap_3()
                                                .py_2()
                                                .items_center()
                                                .child(account_avatar(image.clone(), 40.))
                                                .child(
                                                    v_flex()
                                                        .gap_1()
                                                        .child(
                                                            div()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .child(account.login.clone()),
                                                        )
                                                        .child(
                                                            account
                                                                .server
                                                                .trim_start_matches("https://")
                                                                .to_owned(),
                                                        ),
                                                )
                                        })
                                        .on_click(
                                            move |_, w, cx| {
                                                let _ = select.update(cx, |s, cx| {
                                                    if s.selected.as_ref()
                                                        == Some(&selected_account)
                                                    {
                                                        return;
                                                    }
                                                    s.selected = Some(selected_account.clone());
                                                    s.repositories.clear();
                                                    s.selected_url = None;
                                                    cx.emit(Selection(None));
                                                    s.load(false, w, cx);
                                                });
                                            },
                                        ),
                                    );
                                }
                                for (label, token) in
                                    [("Log in via", false), ("Log In with Token…", true)]
                                {
                                    if token && provider == Provider::GitLab {
                                        let custom = entity.clone();
                                        menu = menu
                                            .item(
                                                PopupMenuItem::new(t("Log In to Custom Server…"))
                                                    .on_click(move |_, w, cx| {
                                                        let _ = custom.update(cx, |s, cx| {
                                                            s.custom_server(w, cx)
                                                        });
                                                    }),
                                            )
                                            .separator();
                                    }
                                    let entity = entity.clone();
                                    let label = if token {
                                        t(label).to_owned()
                                    } else if provider == Provider::GitLab {
                                        t("Log In to gitlab.com…").to_owned()
                                    } else {
                                        format!("{} {}…", t(label), provider.name())
                                    };
                                    menu =
                                        menu.item(
                                            PopupMenuItem::element(move |_, cx| {
                                                div()
                                                    .font_family(cx.theme().font_family.clone())
                                                    .text_size(cx.theme().font_size)
                                                    .child(label.clone())
                                            })
                                            .on_click(move |_, w, cx| {
                                                let _ = entity.update(cx, |s, cx| {
                                                    s.selected = None;
                                                    s.login_ui = true;
                                                    s.token_ui = token;
                                                    s.custom_server_ui = false;
                                                    s.server.update(cx, |v, cx| {
                                                        v.set_value(provider.server(), w, cx)
                                                    });
                                                    s.error = None;
                                                    cx.emit(Selection(None));
                                                    if token {
                                                        s.secret
                                                            .read(cx)
                                                            .focus_handle(cx)
                                                            .focus(w, cx);
                                                    }
                                                    cx.notify();
                                                });
                                            }),
                                        );
                                }
                                menu
                            }),
                    ),
            );
            let filter = self.search.read(cx).value().to_lowercase();
            let mut list = v_flex()
                .id("host-repositories")
                .border_1()
                .border_color(rgb(crate::ui_theme::palette(cx).border))
                .h(px(0.))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll();
            for (i, repo) in self
                .repositories
                .iter()
                .filter(|r| {
                    r.name.to_lowercase().contains(&filter)
                        || r.url.to_lowercase().contains(&filter)
                })
                .cloned()
                .enumerate()
            {
                let selected = self.selected_url.as_deref() == Some(&repo.url);
                list = list.child(
                    div()
                        .id(("host-repository", i))
                        .px_2()
                        .py_1()
                        .cursor_pointer()
                        .when(selected, |r| {
                            r.bg(rgb(crate::ui_theme::palette(cx).selection))
                        })
                        .child(repository_label(&repo.name, self.selected.as_ref()).to_owned())
                        .on_click(cx.listener(move |s, _, _, cx| {
                            if s.busy {
                                return;
                            }
                            if let Some(a) = &s.selected {
                                if let Err(e) = hosting::bind(a, &repo.url) {
                                    s.error = Some(error_text(e));
                                    cx.notify();
                                    return;
                                }
                            }
                            s.selected_url = Some(repo.url.clone());
                            cx.emit(Selection(Some(repo.url.clone())));
                            cx.notify();
                        })),
                );
            }
            content = content.child(list);
            if self.more {
                content = content.child(
                    Button::new("host-more")
                        .label(t("Load more"))
                        .disabled(self.busy)
                        .on_click(cx.listener(|s, _, w, cx| s.load(true, w, cx))),
                );
            }
        }
        if self.busy {
            content = content.child(
                h_flex()
                    .gap_2()
                    .child(gpui_kit::component::spinner::Spinner::new())
                    .child(t(if self.login_ui {
                        "Logging in…"
                    } else {
                        "Connecting…"
                    }))
                    .child(
                        Button::new("host-cancel")
                            .ghost()
                            .label(t("Cancel"))
                            .on_click(cx.listener(|s, _, _, cx| s.cancel(cx))),
                    ),
            )
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .whitespace_normal()
                    .text_color(rgb(crate::ui_theme::color(cx, 0xf2a6a6, 0xc54e58)))
                    .child(error.clone()),
            );
            if !self.login_ui && !self.busy {
                let relogin = self.error.as_deref() == Some(t("Account needs to log in again"));
                content = content.child(
                    Button::new("host-recover")
                        .ghost()
                        .label(t(if relogin { "Log In" } else { "Retry" }))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            if relogin {
                                s.login_ui = true;
                                s.error = None;
                                if let Some(a) = &s.selected {
                                    s.server
                                        .update(cx, |v, cx| v.set_value(a.server.clone(), w, cx));
                                    s.custom_server_ui = a.server != a.provider.server();
                                }
                                cx.emit(Selection(None));
                                cx.notify();
                            } else {
                                s.load(false, w, cx);
                            }
                        })),
                );
            }
        }
        content
    }
}

fn error_text(error: String) -> String {
    // Only the local API adapter's status-only diagnostic is a format template.
    // Preserve unknown diagnostics and external data verbatim.
    if let Some(code) = error
        .strip_prefix("HTTP request failed (")
        .and_then(|s| s.strip_suffix(')'))
        .filter(|s| s.len() == 3 && s.bytes().all(|b| b.is_ascii_digit()))
    {
        return format!("{} ({code})", t("HTTP request failed"));
    }
    match error.as_str() {
        "Browser login is unavailable in this build or for this server; use a token" => {
            t("Browser login is unavailable in this build or for this server; use a token").into()
        }
        "Cancelled" => t("Cancelled").into(),
        "Authorization timed out" => t("Authorization timed out").into(),
        "Account needs to log in again" => t("Account needs to log in again").into(),
        "Could not connect to hosting service" => t("Could not connect to hosting service").into(),
        "Invalid server URL" => t("Invalid server URL").into(),
        "Use an HTTPS server URL without credentials" => {
            t("Use an HTTPS server URL without credentials").into()
        }
        "GitHub Enterprise is not supported" => t("GitHub Enterprise is not supported").into(),
        "Account changed during authorization" => t("Account changed during authorization").into(),
        "Authorization was denied" => t("OAuth authorization returned an error").into(),
        "OAuth callback port is in use" => t("Could not bind the OAuth callback port").into(),
        "Could not start OAuth callback" => t("Could not start OAuth callback").into(),
        "OAuth callback failed" => t("OAuth callback failed").into(),
        "Invalid OAuth configuration" => t("Invalid OAuth configuration").into(),
        "Could not generate OAuth verifier" => t("Could not generate OAuth verifier").into(),
        "Invalid authorization callback" => t("Invalid authorization callback").into(),
        "Invalid authorization state" => t("Invalid authorization state").into(),
        "Missing authorization code" => t("Missing authorization code").into(),
        "Missing OAuth access token" => t("Missing OAuth access token").into(),
        "Access denied: check account permissions or organization policy" => {
            t("Access denied: check account permissions or organization policy").into()
        }
        "OAuth authorization failed; check application configuration or log in again" => {
            t("OAuth authorization failed; check application configuration or log in again").into()
        }
        "Account storage is full" => t("Account storage is full").into(),
        "Invalid repository list" => t("Invalid repository list").into(),
        "Missing account username" => t("Missing account username").into(),
        "Missing repository URL" => t("Missing repository URL").into(),
        "Invalid repository URL" => t("Invalid repository URL").into(),
        "Missing repository name" => t("Missing repository name").into(),
        "Unexpected repository host" => t("Unexpected repository host").into(),
        "Hosting response is too large" => t("Hosting response is too large").into(),
        "Could not read hosting response" => t("Could not read hosting response").into(),
        "Invalid hosting response" => t("Invalid hosting response").into(),
        _ => error,
    }
}

fn repository_label<'a>(name: &'a str, account: Option<&Account>) -> &'a str {
    if let Some((owner, short)) = name.split_once('/') {
        if account
            .is_some_and(|a| a.provider == Provider::GitHub && owner.eq_ignore_ascii_case(&a.login))
        {
            return short;
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn status_translation_preserves_codes_and_unknown_diagnostics() {
        for code in [302, 404, 429, 500, 503] {
            assert_eq!(
                error_text(format!("HTTP request failed ({code})")),
                format!("{} ({code})", t("HTTP request failed"))
            );
        }
        for raw in [
            "HTTP request failed (500): server detail",
            "HTTP request failed (５０３)",
            "HTTP request failed (50)",
            "HTTP request failed (5000)",
            "HTTP request failed (token)",
            "server returned {code}: 中文",
        ] {
            assert_eq!(error_text(raw.into()), raw);
        }
    }

    #[test]
    fn repository_labels_keep_external_owners_and_gitlab_namespaces() {
        let mut account = Account {
            provider: Provider::GitHub,
            server: "https://github.com".into(),
            login: "Alice".into(),
        };
        assert_eq!(repository_label("alice/project", Some(&account)), "project");
        assert_eq!(
            repository_label("bob/project", Some(&account)),
            "bob/project"
        );
        assert_eq!(repository_label("alice/project", None), "alice/project");
        account.provider = Provider::GitLab;
        assert_eq!(
            repository_label("Alice/group/project", Some(&account)),
            "Alice/group/project"
        );
    }
}

fn account_avatar(image: Option<Arc<Image>>, size: f32) -> AnyElement {
    let content = match image {
        Some(image) => img(image)
            .size(px(size))
            .rounded(px(size / 2.))
            .into_any_element(),
        None => img("jb/defaultAvatar_dark.svg")
            .size(px(size))
            .into_any_element(),
    };
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded_full()
        .overflow_hidden()
        .child(content)
        .into_any_element()
}
