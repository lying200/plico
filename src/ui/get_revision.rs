//! Coordinate Get through the same document and repository write barrier as other operations.
use super::*;
use crate::history_view::GetRevision;
use plico::{
    active_operation::{Kind, Phase},
    get_revision::Prepared,
};
impl Review {
    pub(super) fn get_file_revision(
        &mut self,
        request: GetRevision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.get_file_revisions(vec![request], window, cx);
    }
    pub(super) fn get_file_revisions(
        &mut self,
        requests: Vec<GetRevision>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if requests.is_empty()
            || requests.iter().any(|r| r.repository != self.repo.path)
            || self.operation.busy()
            || self.operation.saving_documents()
        {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.get_file_revisions(requests, w, cx)
            });
            return;
        }
        let Some((id, token)) = self.begin_operation(Kind::Restore, Phase::Preparing, cx) else {
            return;
        };
        let repo = self.repo.clone();
        let root = repo.path.clone();
        self.status_message = Some(t("Loading file revision…").into());
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    requests
                        .iter()
                        .map(|request| {
                            Prepared::read(&repo, &request.path, &request.revision, &token)
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(id, cx) else {
                    return;
                };
                if cancelled || s.repo.path != root {
                    s.status_message = None;
                    return;
                }
                s.status_message = None;
                match result {
                    Err(error) => crate::error_ui::show_details(
                        error,
                        "Could not restore file revision",
                        w,
                        cx,
                    ),
                    Ok(prepared) => {
                        let prepared = Arc::new(prepared);
                        if !prepared.iter().any(|file| file.modified) {
                            s.apply_file_revision(root.clone(), prepared, w, cx);
                            return;
                        }
                        let owner = cx.entity().downgrade();
                        w.open_dialog(cx, move |dialog, _, _| {
                            let owner = owner.clone();
                            let root = root.clone();
                            let prepared = prepared.clone();
                            dialog
                                .title(
                                    h_flex()
                                        .gap_3()
                                        .items_center()
                                        .child(
                                            gpui_kit::img("jb/warningDialog_dark_4x.svg")
                                                .size(px(28.)),
                                        )
                                        .child(t("Get Revision")),
                                )
                                .w(px(520.))
                                .centered(true)
                                .overlay_closable(false)
                                .child(div().whitespace_normal().child(t(if prepared.len() == 1 {
                                    "This file has been locally modified."
                                } else {
                                    "Selected files have been locally modified."
                                })))
                                .child(
                                    div().truncate().child(
                                        prepared
                                            .iter()
                                            .filter(|p| p.modified)
                                            .map(|p| p.path.to_string_lossy().into_owned())
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                    ),
                                )
                                .footer(crate::dialog_style::confirmation(
                                    Button::new("get-revision-overwrite").primary().label(t(
                                        if prepared.len() == 1 {
                                            "Overwrite Modified File"
                                        } else {
                                            "Overwrite Modified Files"
                                        },
                                    )),
                                ))
                                .on_ok(move |_, w, cx| {
                                    let _ = owner.update(cx, |s, cx| {
                                        s.apply_file_revision(root.clone(), prepared.clone(), w, cx)
                                    });
                                    true
                                })
                        });
                    }
                }
                cx.notify();
            });
        });
    }
    fn apply_file_revision(
        &mut self,
        root: PathBuf,
        prepared: Arc<Vec<Prepared>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if root != self.repo.path || self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(window, cx, move |s, w, cx| {
                s.apply_file_revision(root, prepared, w, cx)
            });
            return;
        }
        let Some((id, token)) = self.begin_operation(Kind::Restore, Phase::Executing, cx) else {
            return;
        };
        let repo = self.repo.clone();
        let paths = prepared.iter().map(|p| p.path.clone()).collect();
        self.status_message = Some(t("Restoring file revision…").into());
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    plico::get_revision::execute_all(&prepared, &repo, &token)
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_operation(id, cx).is_none() || s.repo.path != root {
                    return;
                }
                match result {
                    Ok(()) => s.status_message = Some(t("File revision restored").into()),
                    Err(error) => {
                        s.status_message = Some(t("Could not restore file revision").into());
                        crate::error_ui::show_details(
                            error,
                            "Could not restore file revision",
                            w,
                            cx,
                        );
                    }
                }
                s.sync_worktree(
                    plico::worktree_events::Batch {
                        paths,
                        ..Default::default()
                    },
                    true,
                    w,
                    cx,
                );
                cx.notify();
            });
        });
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use gpui::component::Root;

    #[gpui::test]
    fn get_request_restores_clean_file_and_ignores_another_repository(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(
            std::env::temp_dir().join(format!("plico-get-ui-{}-{nonce}", std::process::id())),
        );
        std::fs::create_dir(&fixture.0).unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        };
        git(&["init", "-q"]);
        let mut old = String::new();
        for text in ["old", "new"] {
            std::fs::write(fixture.0.join("file.txt"), text).unwrap();
            git(&["add", "file.txt"]);
            git(&[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                text,
            ]);
            if old.is_empty() {
                old = git(&["rev-parse", "HEAD"]);
            }
        }
        let mut request = GetRevision {
            repository: fixture.0.join("another-repo"),
            path: "file.txt".into(),
            revision: plico::file_history::Revision {
                id: old,
                path: "file.txt".into(),
                parents: vec![],
                timestamp: 0,
                author: String::new(),
                message: String::new(),
                deleted: false,
            },
        };
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(fixture.0.clone(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.update(|w, cx| review.update(cx, |s, cx| s.get_file_revision(request.clone(), w, cx)));
        cx.run_until_parked();
        assert_eq!(std::fs::read(fixture.0.join("file.txt")).unwrap(), b"new");
        request.repository = fixture.0.clone();
        cx.update(|w, cx| review.update(cx, |s, cx| s.get_file_revision(request.clone(), w, cx)));
        cx.run_until_parked();
        assert_eq!(std::fs::read(fixture.0.join("file.txt")).unwrap(), b"old");
        assert!(git(&["diff", "--cached"]).is_empty());
        review.read_with(cx, |s, _| {
            assert!(!s.operation.busy());
            assert!(!s.documents.locked());
        });
    }
}
