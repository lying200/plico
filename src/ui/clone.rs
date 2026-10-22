use super::*;
impl Review {
    pub(super) fn open_clone(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.settings_open {
            return;
        }
        let owner = cx.entity().downgrade();
        crate::clone_dialog::show(w, cx, move |request, form, w, cx| {
            let _ = owner.update(cx, |s, cx| s.start_clone(request, form, w, cx));
        });
    }
    fn start_clone(
        &mut self,
        request: plico::clone_repository::Request,
        form: Entity<crate::clone_dialog::Form>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Clone,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        form.update(cx, |s, cx| s.begin(token.clone(), cx));
        let requests = plico::auth_request::Requests::new(token.clone());
        let worker_requests = requests.clone();
        let progress = plico::operation_process::Progress::default();
        let worker_progress = progress.clone();
        let destination = request.destination.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        cx.background_spawn(async move {
            let result = std::env::current_exe()
                .map_err(|e| e.to_string())
                .and_then(|exe| {
                    let helper = exe.with_file_name(if cfg!(windows) {
                        "plico-askpass.exe"
                    } else {
                        "plico-askpass"
                    });
                    plico::clone_repository::validate(&request)?;
                    if !token.cancelled() {
                        let _ = plico::clone_destination::remember(&request.destination);
                    }
                    plico::clone_repository::execute(
                        &request,
                        &token,
                        &worker_progress,
                        Some((&worker_requests, &helper)),
                    )
                });
            // Network/WSL filesystem metadata can block: keep it off the UI thread,
            // including validation failures which do not produce a Report.
            let destination_exists = result.as_ref().map_or_else(
                |_| request.destination.exists(),
                |report| report.destination_exists,
            );
            let _ = tx.send((result, destination_exists));
        })
        .detach();
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let mut login: Option<Entity<crate::auth_dialog::Login>> = None;
            loop {
                if let Some(pending) = requests.take_pending() {
                    if let Ok(d) = wx.update(|w, cx| crate::auth_dialog::show(pending, w, cx)) {
                        login = Some(d);
                    }
                }
                let result = match rx.try_recv() {
                    Ok(r) => Some(r),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some((Err("Clone worker ended".into()), false))
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                };
                if let Some((result, destination_exists)) = result {
                    let _ = this.update_in(wx, |s, w, cx| {
                        if login.as_ref().is_some_and(|d| d.read(cx).active()) {
                            w.close_dialog(cx);
                        }
                        let Some(cancelled) = s.finish_operation(id, cx) else {
                            return;
                        };
                        let success = !cancelled && result.as_ref().is_ok_and(|r| r.succeeded());
                        let error = if success {
                            None
                        } else {
                            let mut text = t(
                                if cancelled
                                    || result.as_ref().is_ok_and(|r| {
                                        r.outcome.stopped
                                            == Some(plico::operation_process::StopReason::Cancelled)
                                    })
                                {
                                    "Clone cancelled"
                                } else {
                                    "Clone failed"
                                },
                            )
                            .to_owned();
                            let details = match &result {
                                Ok(r) => r.diagnostic(),
                                Err(e) => match e.as_str() {
                                    "Enter a repository URL or directory" => {
                                        t("Enter a repository URL or directory").into()
                                    }
                                    "Source is not a directory" => {
                                        t("Source is not a directory").into()
                                    }
                                    "Choose an absolute destination directory" => {
                                        t("Choose an absolute destination directory").into()
                                    }
                                    "Destination is not a directory" => {
                                        t("Destination is not a directory").into()
                                    }
                                    "Destination directory is not empty" => {
                                        t("Destination directory is not empty").into()
                                    }
                                    "Clone depth must be greater than zero" => {
                                        t("Clone depth must be greater than zero").into()
                                    }
                                    "Clone worker ended" => t("Clone worker ended").into(),
                                    "Clone cancelled" => t("Clone cancelled").into(),
                                    "Authentication helper is unavailable" => {
                                        t("Authentication helper is unavailable").into()
                                    }
                                    _ => plico::remote_management::display_url(e),
                                },
                            };
                            if !details.is_empty() {
                                text.push('\n');
                                text.push_str(&details);
                            }
                            if destination_exists {
                                text.push('\n');
                                text.push_str(t(
                                    "The destination remains on disk. Inspect it before retrying.",
                                ));
                            }
                            Some(text)
                        };
                        let visible = form.update(cx, |s, cx| s.complete(error, cx));
                        if success && visible {
                            w.close_dialog(cx);
                            s.choose_project_window(destination, w, cx);
                        }
                    });
                    break;
                }
                let snapshot = progress.snapshot();
                let text = plico::remote_management::display_url(&String::from_utf8_lossy(
                    &snapshot.bytes(),
                ));
                let text = text
                    .lines()
                    .rev()
                    .take(3)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                let _ = wx.update(|_, cx| form.update(cx, |s, cx| s.progress(text, cx)));
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            }
        });
    }
}
