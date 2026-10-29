//! Selected-directory initialization. Filesystem discovery and Git run off-thread.
use super::*;
use plico::{
    active_operation::{Kind, Phase},
    init_repository::Prepared,
};

impl Review {
    pub(super) fn open_init(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.settings_open {
            return;
        }
        let request = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t("Create Git Repository").into()),
        });
        cx.spawn_in(w, async move |this, wx| {
            let Some(path) = request
                .await
                .ok()
                .and_then(|r| r.ok())
                .flatten()
                .and_then(|v| v.into_iter().next())
            else {
                return;
            };
            let _ = this.update_in(wx, |s, w, cx| s.prepare_init(path, w, cx));
        })
        .detach();
    }
    fn prepare_init(&mut self, path: PathBuf, w: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            return;
        }
        let Some((id, token)) = self.begin_operation(Kind::Init, Phase::Preparing, cx) else {
            return;
        };
        self.status_message = Some(t("Creating Git repository…").into());
        self.error = None;
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let result = wx
                .background_spawn(async move { Prepared::read(&path, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                s.status_message = None;
                if cancelled {
                    return;
                }
                match result {
                    Ok(prepared) if prepared.needs_confirmation() => {
                        let prepared = Arc::new(prepared);
                        let owner = cx.entity().downgrade();
                        w.open_dialog(cx, move |dialog, _, cx| {
                            let prepared = prepared.clone();
                            let owner = owner.clone();
                            dialog
                                .title(t("Git Init"))
                                .w(px(540.))
                                .centered(true)
                                .child(
                                    div().child(t("The selected directory is already under Git")),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(crate::ui_theme::palette(cx).muted))
                                        .child(project_display_path(prepared.directory())),
                                )
                                .child(
                                    div().child(t(
                                        "Are you sure you want to create a new VCS root?",
                                    )),
                                )
                                .footer(
                                    DialogFooter::new()
                                        .child(DialogAction::new().child(
                                            Button::new("init-yes").primary().label(t("Yes")),
                                        ))
                                        .child(
                                            gpui_kit::component::dialog::DialogClose::new()
                                                .trigger(|b| b.label(t("No"))),
                                        ),
                                )
                                .on_ok(move |_, w, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        s.execute_init(prepared.clone(), true, w, cx)
                                    });
                                    true
                                })
                        });
                    }
                    Ok(prepared) => s.execute_init(Arc::new(prepared), false, w, cx),
                    Err(error) => {
                        s.error = Some(format!("{}\n{}", t("Git init failed"), error));
                    }
                }
                cx.notify();
            });
        });
    }
    fn execute_init(
        &mut self,
        prepared: Arc<Prepared>,
        confirmed: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(Kind::Init, Phase::Executing, cx) else {
            return;
        };
        let path = prepared.directory().to_owned();
        self.status_message = Some(t("Creating Git repository…").into());
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let result = wx.background_spawn(async move {
                let outcome = plico::init_repository::execute(&prepared, confirmed, &token, &Default::default())?;
                if outcome.status.success() && outcome.stopped.is_none() && outcome.input_error.is_none() { Ok(()) }
                else { Err(String::from_utf8_lossy(&outcome.stderr.bytes()).into_owned()) }
            }).await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else { return; };
                s.status_message = None;
                if cancelled {
                    s.error = Some(t("Initialization cancelled. The directory remains on disk; inspect it before retrying.").into());
                } else if let Err(error) = result {
                    s.error = Some(format!("{}\n{}", t("Git init failed"), error));
                } else {
                    s.error = None;
                    s.choose_project_window(path, w, cx);
                }
                cx.notify();
            });
        });
    }
}
