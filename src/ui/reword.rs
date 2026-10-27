//! Selection-scoped availability and explicit message rewrite coordination.
use super::*;
use plico::{
    active_operation::{Kind, Phase},
    reword::Prepared,
};

pub(super) struct Gate {
    pub target: String,
    repository: Repository,
    pub result: Option<Result<Arc<Prepared>, String>>,
    token: Cancel,
    _task: Task<()>,
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Gate {
    pub fn new(repo: Repository, target: String, cx: &mut Context<Self>) -> Self {
        let token = Cancel::default();
        let query = token.clone();
        let id = target.clone();
        let repository = repo.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { Prepared::prepare(&repo, id, &query).map(Arc::new) })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.result = Some(result);
                cx.notify();
            });
        });
        Self {
            target,
            repository,
            token,
            _task: task,
            result: None,
        }
    }
}
impl Review {
    pub(super) fn refresh_reword_gate(&mut self, cx: &mut Context<Self>) {
        let ids = self.commit_ids();
        let selected = self.log_selection.ordered(&ids);
        self.reword_gate = if selected.len() == 1 {
            Some(cx.new(|cx| {
                Gate::new(
                    Repository::new(selected[0].root.as_ref().clone()),
                    selected[0].hash.clone(),
                    cx,
                )
            }))
        } else {
            None
        };
    }
    pub(super) fn can_reword(&self, target: &str, cx: &App) -> bool {
        if self.operation.busy()
            || self.operation.saving_documents()
            || self.log_query.history_loading()
        {
            return false;
        }
        let ids = self.commit_ids();
        let selected = self.log_selection.ordered(&ids);
        let [identity] = selected.as_slice() else {
            return false;
        };
        let Some(gate) = &self.reword_gate else {
            return false;
        };
        let gate = gate.read(cx);
        if identity.hash != target
            || gate.target != target
            || gate.repository.path != *identity.root
        {
            return false;
        }
        let Some(index) = ids.iter().position(|id| id == *identity) else {
            return false;
        };
        let Some(commit) = self.commits.get(index) else {
            return false;
        };
        let is_head = gate.result.as_ref().is_some_and(|result| {
            result
                .as_ref()
                .is_ok_and(|prepared| prepared.head == target)
        }) || (gate.repository.path == self.repo.path
            && self.snapshot.as_ref().and_then(|s| s.head_id.as_deref()) == Some(target));
        (commit.parents.len() <= 1 || is_head) && gate.result.as_ref().is_none_or(Result::is_ok)
    }
    pub(super) fn edit_commit_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(gate) = &self.reword_gate else {
            return;
        };
        if !self.can_reword(&gate.read(cx).target, cx) {
            return;
        }
        if let Some(Ok(prepared)) = &gate.read(cx).result {
            self.show_reword(prepared.clone(), window, cx);
            return;
        }
        // Like Rebased, unknown containing-branch information does not make the
        // first context menu inert. Resolve it before showing the editor.
        let target = gate.read(cx).target.clone();
        let repo = gate.read(cx).repository.clone();
        let Some((operation, token)) = self.begin_operation(Kind::Reword, Phase::Preparing, cx)
        else {
            return;
        };
        self.status_message = Some(t("Checking…").into());
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move { Prepared::prepare(&repo, target, &token) })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                let Some(cancelled) = s.finish_operation(operation, cx) else {
                    return;
                };
                s.status_message = None;
                if cancelled {
                    return;
                }
                match result {
                    Ok(p) => s.show_reword(Arc::new(p), w, cx),
                    Err(e) => {
                        s.rebase_diagnostic = Some(e);
                        s.status_message = Some(t("Could not edit commit message").into());
                    }
                }
                cx.notify();
            });
        });
    }
    fn show_reword(
        &mut self,
        prepared: Arc<Prepared>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let initial = prepared.initial;
        let owner = cx.entity().downgrade();
        let repo = prepared.repository();
        let show = move |w: &mut Window, cx: &mut App| {
            let owner = owner.clone();
            let repo = repo.clone();
            let expected = prepared.clone();
            crate::reword_dialog::show(repo, prepared.clone(), w, cx, move |message, w, cx| {
                let _ = owner.update(cx, |s, cx| s.start_reword(expected.clone(), message, w, cx));
            });
        };
        if initial {
            window.open_dialog(cx, move |d, _, _| {
                let show = show.clone();
                d.title(t("Editing Initial Commit")).w(px(470.)).centered(true).overlay_closable(false)
                    .child(t("Editing the initial commit rewrites the history of the current branch. Continue?"))
                    .footer(crate::dialog_style::confirmation(Button::new("reword-root").primary().label(t("Continue"))))
                    .on_ok(move |_, w, cx| { w.close_dialog(cx); show(w, cx); false })
            });
        } else {
            show(window, cx);
        }
    }
    fn start_reword(
        &mut self,
        prepared: Arc<Prepared>,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.welcome || self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        let Some((operation, token)) = self.begin_operation(Kind::Reword, Phase::Executing, cx)
        else {
            return;
        };
        let repo = prepared.repository();
        self.rebase_diagnostic = None;
        self.status_message = Some(t("Editing commit message…").into());
        self.operation_task = cx.spawn_in(window, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    prepared.execute(&repo, &message, &token, &Default::default())
                })
                .await;
            let _ = this.update_in(wx, |s, _, cx| {
                if s.finish_operation(operation, cx).is_none() {
                    return;
                }
                match result {
                    Ok(done) => {
                        s.status_message = Some(
                            t(if done.warning.is_some() {
                                "Commit message edited; follow-up failed"
                            } else {
                                "Commit message edited"
                            })
                            .into(),
                        );
                        s.rebase_diagnostic = done.warning;
                        // SelectionSnapshot restores only original identities
                        // still present in the refreshed graph. Never relabel an
                        // existing row as the newly written Git object.
                    }
                    Err(e) => {
                        s.rebase_diagnostic = Some(e);
                        s.status_message = Some(t("Could not edit commit message").into());
                    }
                }
                s.squash_undo = None;
                s.operation_state = None;
                s.refresh_impl(false, true, None, cx);
                cx.notify();
            });
        });
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;

    #[gpui::test]
    fn reword_refresh_restores_only_original_commits_still_in_the_graph(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn git(path: &std::path::Path, args: &[&str]) -> String {
            let result = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            String::from_utf8(result.stdout).unwrap().trim().to_owned()
        }
        cx.update(gpui::component::init);
        for retain_old in [false, true] {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = format!(
                "plico-reword-selection-{}-{nonce}-{retain_old}",
                std::process::id()
            );
            let fixture = Fixture(std::env::temp_dir().join(directory));
            std::fs::create_dir_all(&fixture.0).unwrap();
            for args in [
                vec!["init", "-q", "-b", "main"],
                vec!["config", "user.name", "Test"],
                vec!["config", "user.email", "test@example.invalid"],
                vec!["config", "commit.gpgsign", "false"],
                vec!["config", "core.hooksPath", "disabled-hooks"],
                vec!["commit", "--allow-empty", "-qm", "base"],
                vec!["commit", "--allow-empty", "-qm", "old message"],
            ] {
                git(&fixture.0, &args);
            }
            let original = git(&fixture.0, &["rev-parse", "HEAD"]);
            if retain_old {
                git(&fixture.0, &["tag", "keep-original"]);
            }
            let prepared = Arc::new(
                Prepared::prepare(
                    &Repository::new(&fixture.0),
                    original.clone(),
                    &Cancel::default(),
                )
                .unwrap(),
            );
            let mut review = None;
            let (_, window_cx) = cx.add_window_view(|window, cx| {
                let view = cx.new(|cx| Review::new(fixture.0.clone(), 100, None, window, cx));
                review = Some(view.clone());
                Root::new(view, window, cx)
            });
            let review = review.unwrap();
            window_cx.run_until_parked();
            window_cx.update(|window, cx| {
                review.update(cx, |s, cx| {
                    assert!(s.error.is_none(), "{:?}", s.error);
                    let row = s.commits.iter().position(|c| c.id == original).unwrap();
                    s.select_commit(row, cx);
                    s.start_reword(prepared, "new message".into(), window, cx);
                })
            });
            window_cx.run_until_parked();
            review.read_with(window_cx, |s, _| {
                assert!(s.error.is_none(), "{:?}", s.error);
                assert!(s.rebase_diagnostic.is_none(), "{:?}", s.rebase_diagnostic);
                assert!(!s.operation.busy());
                let ids = s.commit_ids();
                let selected = s.log_selection.ordered(&ids);
                if retain_old {
                    assert_eq!(selected.len(), 1);
                    assert_eq!(selected[0].hash, original);
                } else {
                    assert!(selected.is_empty());
                    assert!(s.selected.is_none());
                }
            });
            assert_ne!(git(&fixture.0, &["rev-parse", "HEAD"]), original);
        }
    }
}
