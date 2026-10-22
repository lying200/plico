//! Main-window settings coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Language = 0,
    Git = 1,
    EditorTabs = 3,
    Appearance = 4,
    EditorFont = 5,
    GitHub = 6,
    GitLab = 7,
}

pub(super) struct SettingsDraft {
    fonts: crate::ui_font::Preferences,
    section: Page,
    language: Language,
    file_preview: bool,
    git: Option<Entity<crate::update_settings::Settings>>,
    github: Entity<crate::hosting_settings::Settings>,
    gitlab: Entity<crate::hosting_settings::Settings>,
    loading: bool,
    error: Option<String>,
}

impl Review {
    pub(super) fn set_language(
        &mut self,
        language: Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Err(error) = layout_preferences::save_language(language) {
            if let Some(draft) = &mut self.settings_draft {
                draft.section = Page::Language;
                draft.error = Some(format!("{}: {error}", t("Settings could not be saved")));
            }
            cx.notify();
            return false;
        }
        i18n::apply(language);
        gpui_kit::component::set_locale(if i18n::is_chinese() { "zh-CN" } else { "en" });
        self.sync_language(window, cx);
        cx.refresh_windows();
        cx.notify();
        true
    }
    // Language is application-wide. Each retained window synchronizes its
    // inputs on redraw, without rebuilding them or changing their text/focus.
    pub(super) fn sync_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chinese = i18n::is_chinese();
        if self.ui_chinese == chinese {
            return;
        }
        self.ui_chinese = chinese;
        self.filter_search.update(cx, |input, cx| {
            input.set_placeholder(t("Text or hash"), window, cx)
        });
        self.branch_search.update(cx, |input, cx| {
            input.set_placeholder(t("Branches and Tags"), window, cx)
        });
        self.repository_search_input.update(cx, |input, cx| {
            input.set_placeholder(t("Search for branches and actions"), window, cx)
        });
        if self.repository_menu_open {
            self.apply_repository_search(cx);
        }
    }
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_project_preview(cx);
        if self.settings_open {
            return;
        }
        self.settings_session = self.settings_session.wrapping_add(1);
        let github = crate::hosting_settings::Settings::new(
            plico::hosting::Provider::GitHub,
            self.repo.path.clone(),
            window,
            cx,
        );
        let gitlab = crate::hosting_settings::Settings::new(
            plico::hosting::Provider::GitLab,
            self.repo.path.clone(),
            window,
            cx,
        );
        cx.observe(&github, |_, _, cx| cx.notify()).detach();
        cx.observe(&gitlab, |_, _, cx| cx.notify()).detach();
        self.settings_draft = Some(SettingsDraft {
            fonts: crate::ui_font::Preferences::current(cx),
            section: Page::Language,
            language: i18n::preference(),
            file_preview: self.file_preview,
            git: None,
            github,
            gitlab,
            loading: false,
            error: None,
        });
        self.settings_open = true;
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, cx| {
            let body_owner = owner.clone();
            let confirm_owner = owner.clone();
            let close_owner = owner.clone();
            let title = owner
                .update(cx, |s, _| {
                    if s.welcome {
                        t("Settings").to_owned()
                    } else {
                        format!(
                            "{} — {}",
                            t("Settings"),
                            s.repo
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                        )
                    }
                })
                .unwrap_or_else(|_| t("Settings").to_owned());
            dialog
                .title(
                    div()
                        .text_size(cx.theme().font_size)
                        .font_weight(FontWeight::NORMAL)
                        .child(title),
                )
                .pb_0()
                .w(px(900.).min(window.viewport_size().width - px(32.)))
                .h(px(650.).min(window.viewport_size().height - px(48.)))
                .centered(true)
                .overlay_closable(false)
                .content(move |body, _, cx| {
                    let panel = body_owner
                        .update(cx, |s, cx| {
                            if s.settings_draft.is_some() {
                                s.settings_panel(cx).into_any_element()
                            } else {
                                div().into_any_element()
                            }
                        })
                        .unwrap_or_else(|_| div().into_any_element());
                    body.p_0().min_h_0().overflow_hidden().child(panel)
                })
                .on_ok(move |_, window, cx| {
                    confirm_owner
                        .update(cx, |s, cx| {
                            s.apply_settings(true, window, cx);
                            !s.settings_open
                        })
                        .unwrap_or(true)
                })
                .on_close(move |_, _, cx| {
                    let _ = close_owner.update(cx, |s, cx| s.close_settings(cx));
                })
        });
        cx.notify();
    }
    pub(super) fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = false;
        self.settings_draft = None;
        cx.notify();
    }
    pub(super) fn apply_settings(
        &mut self,
        close: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(draft) = &self.settings_draft else {
            return;
        };
        if draft.loading || self.operation.busy() {
            return;
        }
        let fonts = draft.fonts.clone();
        for (page, accounts) in [
            (Page::GitHub, draft.github.clone()),
            (Page::GitLab, draft.gitlab.clone()),
        ] {
            if !accounts.update(cx, |s, cx| s.save(cx)) {
                if let Some(d) = &mut self.settings_draft {
                    d.section = page;
                }
                cx.notify();
                return;
            }
        }
        let draft = self.settings_draft.as_ref().unwrap();
        let (language, file_preview, git) = (draft.language, draft.file_preview, draft.git.clone());
        if let Some(draft) = &mut self.settings_draft {
            draft.error = None;
        }
        if let Some(git) = git {
            if !git.update(cx, |s, cx| s.save(cx)) {
                if let Some(d) = &mut self.settings_draft {
                    d.section = Page::Git;
                }
                cx.notify();
                return;
            }
        }
        if language != i18n::preference() && !self.set_language(language, window, cx) {
            return;
        }
        if file_preview != self.file_preview {
            if let Err(error) = layout_preferences::save_file_preview(file_preview) {
                if let Some(d) = &mut self.settings_draft {
                    d.section = Page::EditorTabs;
                    d.error = Some(format!("{}: {error}", t("Settings could not be saved")));
                }
                cx.notify();
                return;
            }
            self.file_preview = file_preview;
            if let Some(view) = &self.project_view {
                view.update(cx, |v, cx| v.set_preview_enabled(file_preview, cx));
            }
            if !file_preview {
                for tab in &self.file_tabs {
                    tab.update(cx, |t, _| t.preview = false);
                }
            }
        }
        if fonts != crate::ui_font::Preferences::current(cx) {
            if let Err(error) = fonts.save() {
                if let Some(d) = &mut self.settings_draft {
                    d.error = Some(format!("{}: {error}", t("Settings could not be saved")));
                }
                cx.notify();
                return;
            }
            fonts.apply(cx);
        }
        if close {
            self.close_settings(cx);
        }
        cx.notify();
    }
    pub(super) fn open_update_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.welcome || self.operation.busy() {
            return;
        }
        let Some(draft) = &mut self.settings_draft else {
            return;
        };
        if draft.git.is_some() || draft.loading {
            return;
        }
        draft.loading = true;
        draft.error = None;
        let session = self.settings_session;
        let repo = self.repo.clone();
        let path = repo.path.clone();
        cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::update_preferences::for_repository(&repo, &Cancel::default()).map(
                        |(path, preferences)| {
                            let show = plico::project_update_preferences::load_show_options(
                                &path.with_file_name("project-update-options"),
                            );
                            (path, preferences, show)
                        },
                    )
                })
                .await;
            let _ = wx.update(|_, cx| {
                let _ = this.update(cx, |s, cx| {
                    if s.repo.path != path || !s.settings_open || s.settings_session != session {
                        return;
                    }
                    let Some(draft) = &mut s.settings_draft else {
                        return;
                    };
                    draft.loading = false;
                    match result {
                        Ok((path, preferences, show)) => {
                            let git = crate::update_settings::new(path, preferences, show, cx);
                            cx.observe(&git, |_, _, cx| cx.notify()).detach();
                            draft.git = Some(git);
                        }
                        Err(error) => draft.error = Some(error),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let draft = self
            .settings_draft
            .as_ref()
            .expect("Open settings have a draft");
        let modified = draft.fonts != crate::ui_font::Preferences::current(cx)
            || draft.github.read(cx).modified(cx)
            || draft.gitlab.read(cx).modified(cx)
            || draft.language != i18n::preference()
            || draft.file_preview != self.file_preview
            || draft
                .git
                .as_ref()
                .is_some_and(|git| git.read(cx).modified());
        let mut navigation = v_flex().w(px(240.)).flex_shrink_0().py_3();
        for (section, label) in [
            (Page::Language, "Language"),
            (Page::Appearance, "Appearance"),
            (Page::EditorFont, "Editor Font"),
            (Page::EditorTabs, "Editor Tabs"),
            (Page::Git, "Git"),
            (Page::GitHub, "GitHub"),
            (Page::GitLab, "GitLab"),
        ] {
            if section == Page::Git {
                navigation = navigation.child(
                    div()
                        .mt_2()
                        .px_4()
                        .py_2()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(t("Version Control")),
                );
            }
            navigation = navigation.child(
                Button::new(("settings-section", section as usize))
                    .ghost()
                    .h(text_row_height(26., 7., cx))
                    .rounded(px(0.))
                    .accessibility_label(t(label))
                    .child(
                        div()
                            .w_full()
                            .pl(if section == Page::Language {
                                px(4.)
                            } else {
                                px(28.)
                            })
                            .child(t(label)),
                    )
                    .selected(draft.section == section)
                    .when(draft.section == section, |b| b.bg(rgb(0x2d466f)))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if let Some(draft) = &mut s.settings_draft {
                            draft.section = section;
                        }
                        if section == Page::Git {
                            s.open_update_settings(w, cx);
                        }
                        cx.notify();
                    })),
            );
        }
        let title = match draft.section {
            Page::Appearance => t("Appearance").to_owned(),
            Page::EditorFont => t("Editor Font").to_owned(),
            Page::EditorTabs => t("Editor Tabs").to_owned(),
            Page::Git => format!("{}  ›  Git", t("Version Control")),
            Page::GitHub => format!("{}  ›  GitHub", t("Version Control")),
            Page::GitLab => format!("{}  ›  GitLab", t("Version Control")),
            Page::Language => t("Language").to_owned(),
        };
        let mut content = v_flex()
            .flex_1()
            .min_w_0()
            .px_5()
            .pt_3()
            .pb_4()
            .gap_5()
            .child(
                div()
                    .text_size(cx.theme().font_size)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            );
        content = match draft.section {
            Page::Appearance | Page::EditorFont => {
                let editor = draft.section == Page::EditorFont;
                let family = if editor {
                    &draft.fonts.editor_family
                } else {
                    &draft.fonts.ui_family
                };
                let size = if editor {
                    draft.fonts.editor_size
                } else {
                    draft.fonts.ui_size
                };
                let owner = cx.entity().downgrade();
                let sizes_owner = owner.clone();
                content
                    .child(
                        h_flex().gap_3().items_center().child(t("Font:")).child(
                            Button::new("font-family")
                                .w(px(260.))
                                .label(family.clone())
                                .dropdown_caret(true)
                                .dropdown_menu(move |mut menu, _, cx| {
                                    let mut names = cx.text_system().all_font_names();
                                    names.push(crate::ui_font::family().into());
                                    names.push("JetBrains Mono".into());
                                    names.sort();
                                    names.dedup();
                                    menu = menu.scrollable(true).max_h(px(320.));
                                    for name in names {
                                        let owner = owner.clone();
                                        menu =
                                            menu.item(PopupMenuItem::new(name.clone()).on_click(
                                                move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        if let Some(d) = &mut s.settings_draft {
                                                            if editor {
                                                                d.fonts.editor_family =
                                                                    name.clone();
                                                            } else {
                                                                d.fonts.ui_family = name.clone();
                                                            }
                                                        }
                                                        cx.notify();
                                                    });
                                                },
                                            ));
                                    }
                                    menu
                                }),
                        ),
                    )
                    .child(
                        h_flex().gap_3().items_center().child(t("Size:")).child(
                            Button::new("font-size")
                                .w(px(90.))
                                .label(size.to_string())
                                .dropdown_caret(true)
                                .dropdown_menu(move |mut menu, _, _| {
                                    menu = menu.scrollable(true).max_h(px(280.));
                                    for size in 9..=if editor { 40 } else { 32 } {
                                        let owner = sizes_owner.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(size.to_string()).on_click(
                                                move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        if let Some(d) = &mut s.settings_draft {
                                                            if editor {
                                                                d.fonts.editor_size = size;
                                                            } else {
                                                                d.fonts.ui_size = size;
                                                            }
                                                        }
                                                        cx.notify();
                                                    });
                                                },
                                            ),
                                        );
                                    }
                                    menu
                                }),
                        ),
                    )
                    .child(div().text_color(rgb(MUTED)).child(t("Preview")))
                    .child(
                        div()
                            .p_3()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .rounded(px(4.))
                            .font_family(family.clone())
                            .text_size(px(size as f32))
                            .child(if editor {
                                "fn main() { println!(\"Hello, Plico!\"); }  // 代码预览"
                            } else {
                                "Plico · Projects · Settings · 项目与设置"
                            }),
                    )
                    .child(
                        Button::new("restore-font")
                            .self_start()
                            .label(t("Restore Defaults"))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                if let Some(d) = &mut s.settings_draft {
                                    let defaults = crate::ui_font::Preferences::default();
                                    if editor {
                                        d.fonts.editor_family = defaults.editor_family;
                                        d.fonts.editor_size = defaults.editor_size;
                                    } else {
                                        d.fonts.ui_family = defaults.ui_family;
                                        d.fonts.ui_size = defaults.ui_size;
                                    }
                                }
                                cx.notify();
                            })),
                    )
            }
            Page::EditorTabs => content
                .child(
                    gpui_kit::component::checkbox::Checkbox::new("file-preview-setting")
                        .label(t("Enable preview tab"))
                        .checked(draft.file_preview)
                        .on_click(cx.listener(|s, checked, _, cx| {
                            if let Some(d) = &mut s.settings_draft {
                                d.file_preview = *checked;
                            }
                            cx.notify();
                        })),
                )
                .child(div().text_color(rgb(MUTED)).child(t(
                    "Select a file to preview it. Double-click or press Enter to keep it open.",
                ))),
            Page::Git => {
                if let Some(git) = &draft.git {
                    content.child(git.clone())
                } else if draft.loading {
                    content.child(t("Loading settings…"))
                } else {
                    content.child(t(if self.welcome {
                        "Open a repository to configure Git"
                    } else {
                        "Git settings are unavailable during an operation"
                    }))
                }
            }
            Page::GitHub => content.child(draft.github.clone()),
            Page::GitLab => content.child(draft.gitlab.clone()),
            Page::Language => content.child(
                v_flex().gap_3().children(
                    [
                        ("language-system", Language::System, t("Follow system")),
                        ("language-chinese", Language::Chinese, "中文"),
                        ("language-english", Language::English, "English"),
                    ]
                    .into_iter()
                    .map(|(id, language, label)| {
                        gpui_kit::component::radio::Radio::new(id)
                            .label(label)
                            .checked(draft.language == language)
                            .on_change(cx.listener(move |s, _, _, cx| {
                                if let Some(d) = &mut s.settings_draft {
                                    d.language = language;
                                }
                                cx.notify();
                            }))
                    }),
                ),
            ),
        };
        v_flex()
            .size_full()
            .bg(rgb(PANEL))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(navigation)
                    .child(div().w(px(1.)).bg(rgb(BORDER)))
                    .child(content),
            )
            .when_some(draft.error.clone(), |v, error| {
                v.child(div().px_4().text_color(rgb(0xf2a6a6)).child(error))
            })
            .child(
                h_flex()
                    .h(px(50.))
                    .px_4()
                    .gap_3()
                    .items_center()
                    .justify_end()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .child(
                        Button::new("settings-ok")
                            .primary()
                            .min_w(px(72.))
                            .label(t("OK"))
                            .disabled(draft.loading || self.operation.busy())
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.apply_settings(true, w, cx);
                                if !s.settings_open {
                                    w.close_dialog(cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("close-settings")
                            .min_w(px(72.))
                            .label(t("Cancel"))
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.close_settings(cx);
                                w.close_dialog(cx);
                            })),
                    )
                    .child(
                        Button::new("settings-apply")
                            .min_w(px(72.))
                            .label(t("Apply"))
                            .disabled(!modified || draft.loading || self.operation.busy())
                            .on_click(cx.listener(|s, _, w, cx| s.apply_settings(false, w, cx))),
                    ),
            )
    }
}
