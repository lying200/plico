//! Multi-root worker and editor pump; result presentation stays shared with a
//! single-root sequence so recovery actions retain the stopped repository.
use super::*;
use plico::{
    project_log::CommitId,
    project_sequence::{Kind, Plan, Report, Results, SavedResults},
};

/// The original conflict interaction owns the entire workflow, including drafts.
/// Closing it drops continuation authority, but never removes retained Git stashes.
pub(in crate::ui) enum Session {
    Ordinary(Results),
    Saved(SavedResults),
}
impl Session {
    fn operation(&self) -> Option<Operation> {
        let stopped = self.results().roots.iter().find(|root| !root.complete())?;
        match stopped.result.as_ref()?.as_ref().ok()? {
            Report::Revert(_) => Some(Operation::Revert),
            Report::CherryPick(_) => Some(Operation::CherryPick),
        }
    }
    fn results(&self) -> &Results {
        match self {
            Self::Ordinary(results) => results,
            Self::Saved(saved) => &saved.results,
        }
    }
    fn results_mut(&mut self) -> &mut Results {
        match self {
            Self::Ordinary(results) => results,
            Self::Saved(saved) => &mut saved.results,
        }
    }
    fn resumable(&self) -> bool {
        match self {
            Self::Ordinary(_) => true,
            Self::Saved(saved) => {
                saved.error.is_none() && saved.saved.iter().all(|root| root.restore.is_none())
            }
        }
    }
    fn complete(&self) -> bool {
        match self {
            Self::Ordinary(results) => results.complete(),
            Self::Saved(saved) => saved.complete(),
        }
    }
    fn resume(
        &mut self,
        editor: &std::ffi::OsStr,
        requests: &plico::auth_request::Requests,
        token: &plico::git::Cancel,
    ) -> Result<(), String> {
        let operation = self.operation().ok_or("No stopped operation to continue")?;
        match (operation, self) {
            (Operation::Revert, Self::Ordinary(results)) => {
                results.resume_revert(editor, Some(requests), token, &Default::default())
            }
            (Operation::Revert, Self::Saved(saved)) => {
                saved.resume_revert(editor, Some(requests), token, &Default::default())
            }
            (Operation::CherryPick, Self::Ordinary(results)) => {
                results.resume_cherry_pick(editor, Some(requests), token, &Default::default())
            }
            (Operation::CherryPick, Self::Saved(saved)) => {
                saved.resume_cherry_pick(editor, Some(requests), token, &Default::default())
            }
            _ => Err("Unexpected stopped operation".into()),
        }
    }
    fn resolve_empty(
        &mut self,
        empty: &EmptyCommit,
        choice: EmptyChoice,
        editor: &std::ffi::OsStr,
        requests: &plico::auth_request::Requests,
        token: &plico::git::Cancel,
    ) -> Result<(), String> {
        match self {
            Self::Ordinary(results) => results.resolve_empty(
                (empty, choice),
                editor,
                Some(requests),
                token,
                &Default::default(),
            ),
            Self::Saved(saved) => saved.resolve_empty(
                (empty, choice),
                editor,
                Some(requests),
                token,
                &Default::default(),
            ),
        }
    }
    /// Read reports without destroying the saved workflow needed by continuation.
    fn saved_details(&self) -> (Option<String>, Option<Repository>) {
        let Self::Saved(saved) = self else {
            return (None, None);
        };
        let mut details = Vec::new();
        let mut conflict = None;
        if let Some(error) = &saved.error {
            details.push(error.clone());
        }
        for root in &saved.saved {
            if root.restore.is_some() {
                let repository = Repository::new(&root.path);
                if repository
                    .operation_state(&Default::default())
                    .is_ok_and(|state| !state.conflicts.is_empty())
                {
                    conflict.get_or_insert(repository);
                }
            }
            if let Some(oid) = &root.saved_oid {
                details.push(format!(
                    "{}: {}: {}",
                    root.path.display(),
                    t("Saved changes retained"),
                    oid
                ));
            }
            if let Some(Err(error)) = &root.restore {
                details.push(format!("{}: {}", root.path.display(), error));
            }
        }
        ((!saved.complete()).then(|| details.join("\n")), conflict)
    }
}

enum Work {
    Selection(Vec<CommitId>),
    Saved(Plan),
    Prepared(Plan),
    Resume(Session),
    Empty(Session, EmptyCommit, EmptyChoice),
}
impl Work {
    fn selection(&self) -> Vec<CommitId> {
        match self {
            Self::Selection(ids) => ids.clone(),
            Self::Saved(plan) | Self::Prepared(plan) => plan.selection(),
            Self::Resume(session) | Self::Empty(session, _, _) => session
                .results()
                .roots
                .iter()
                .flat_map(|root| {
                    let path = Arc::new(root.repository.path.clone());
                    root.targets.iter().map(move |hash| CommitId {
                        root: path.clone(),
                        hash: hash.clone(),
                    })
                })
                .collect(),
        }
    }
}

impl Review {
    pub(in crate::ui) fn can_sequence_selection(&self, targets: &[CommitId]) -> bool {
        if targets.is_empty() || targets.len() > 1000 {
            return false;
        }
        let selected: std::collections::HashSet<_> = targets.iter().collect();
        if selected.len() != targets.len() {
            return false;
        }
        let mut matched = 0;
        for (index, id) in self.commit_ids().iter().enumerate() {
            if selected.contains(id) {
                if self.log_repository(index).is_none()
                    || self.log.commits()[index].parents.len() > 1
                {
                    return false;
                }
                matched += 1;
            }
        }
        matched == selected.len()
    }

    pub(in crate::ui) fn start_log_sequence(
        &mut self,
        kind: Operation,
        ids: Vec<CommitId>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_log_sequence(kind, ids, None, w, cx);
    }
    fn run_log_sequence(
        &mut self,
        kind: Operation,
        ids: Vec<CommitId>,
        editor_override: Option<std::ffi::OsString>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_project_sequence(kind, Work::Selection(ids), editor_override, w, cx);
    }

    pub(super) fn retry_project_sequence(
        &mut self,
        repository: &Repository,
        kind: Operation,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.retry_project_sequence_with_editor(repository, kind, None, w, cx)
    }

    fn retry_project_sequence_with_editor(
        &mut self,
        repository: &Repository,
        kind: Operation,
        editor_override: Option<std::ffi::OsString>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .sequence_retry
            .as_ref()
            .is_some_and(|(operation, root, _)| *operation == kind && root == &repository.path)
        {
            return false;
        }
        if self.operation.busy() || self.operation.saving_documents() {
            return true;
        }
        let (_, _, plan) = self.sequence_retry.take().unwrap();
        self.run_project_sequence(kind, Work::Saved(plan), editor_override, w, cx);
        true
    }

    pub(in crate::ui) fn resume_project_sequence(
        &mut self,
        repository: &Repository,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .sequence_conflict
            .as_ref()
            .is_some_and(|(_, root, _, _)| root == &repository.path)
        {
            return false;
        }
        if self.operation.busy() || self.operation.saving_documents() {
            return true;
        }
        let Some(kind) = self
            .sequence_conflict
            .as_ref()
            .and_then(|(_, _, session, _)| session.operation())
        else {
            self.sequence_conflict = None;
            return false;
        };
        let (_, _, results, editor) = self.sequence_conflict.take().unwrap();
        self.run_project_sequence(kind, Work::Resume(results), Some(editor), w, cx);
        true
    }

    pub(super) fn resolve_project_empty(
        &mut self,
        repository: &Repository,
        empty: &EmptyCommit,
        choice: EmptyChoice,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_, root, session, _)) = &self.sequence_conflict else {
            return false;
        };
        if root != &repository.path {
            return false;
        }
        let matching = session
            .results()
            .roots
            .iter()
            .find(|root| !root.complete())
            .and_then(|root| root.result.as_ref())
            .is_some_and(|report| {
                matches!(report,
                Ok(Report::CherryPick(report)) if report.empty.as_ref() == Some(empty))
            });
        if !matching {
            return true;
        }
        if self.operation.busy() || self.operation.saving_documents() {
            return true;
        }
        let (_, _, session, editor) = self.sequence_conflict.take().unwrap();
        self.run_project_sequence(
            Operation::CherryPick,
            Work::Empty(session, empty.clone(), choice),
            Some(editor),
            w,
            cx,
        );
        true
    }

    fn run_project_sequence(
        &mut self,
        kind: Operation,
        work: Work,
        editor_override: Option<std::ffi::OsString>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.welcome || self.operation.busy() || self.operation.saving_documents() {
            return;
        }
        if work.selection().is_empty() {
            return;
        }
        if self.documents.pending(cx) {
            self.with_saved_documents(w, cx, move |s, w, cx| {
                s.run_project_sequence(kind, work, editor_override, w, cx)
            });
            return;
        }
        let editor = match editor_override.map(Ok).unwrap_or_else(|| {
            std::env::current_exe().and_then(|exe| {
                plico::editor::command(
                    &exe.with_file_name(if cfg!(windows) {
                        "plico-editor.exe"
                    } else {
                        "plico-editor"
                    }),
                    false,
                )
            })
        }) {
            Ok(editor) => editor,
            Err(error) => {
                crate::error_ui::show_details(
                    error.to_string(),
                    "Git editor helper is unavailable",
                    w,
                    cx,
                );
                return;
            }
        };
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Recovery,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        self.sequence_diagnostic = None;
        self.sequence_retry = None;
        self.sequence_conflict = None;
        let saving = matches!(
            &work,
            Work::Saved(_) | Work::Resume(Session::Saved(_)) | Work::Empty(Session::Saved(_), _, _)
        );
        self.status_message = Some(
            t(if kind == Operation::CherryPick {
                "Cherry-picking commits…"
            } else {
                "Updating operation state…"
            })
            .into(),
        );
        let project_path = self.repo.path.clone();
        let requests = plico::auth_request::Requests::new(token.clone());
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let preparation = wx
                .background_spawn({
                    let token = token.clone();
                    async move {
                        let ids = work.selection();
                        let groups = plico::project_log::selection::group(&ids, 1000)?;
                        let directories = groups
                            .iter()
                            .map(|group| {
                                Repository::new(group.root.as_ref()).editor_directory(&token)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let work = match work {
                            Work::Selection(ids) => Work::Prepared(Plan::read(
                                &ids,
                                if kind == Operation::CherryPick {
                                    Kind::CherryPick
                                } else {
                                    Kind::Revert
                                },
                                &token,
                            )?),
                            work => work,
                        };
                        Ok::<_, String>((work, directories))
                    }
                })
                .await;
            let (work, directories) = match preparation {
                Ok(prepared) => prepared,
                Err(error) => {
                    let _ = this.update_in(wx, |s, w, cx| {
                        let Some(cancelled) = s.finish_operation(id, cx) else {
                            return;
                        };
                        if s.repo.path != project_path {
                            return;
                        }
                        s.sequence_diagnostic = Some(error.clone());
                        s.present_sequence_result(
                            s.repo.clone(),
                            kind,
                            false,
                            cancelled,
                            Err(error),
                            w,
                            cx,
                        );
                    });
                    return;
                }
            };
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            let worker_requests = requests.clone();
            let continuation_editor = editor.clone();
            wx.background_spawn(async move {
                let result = match work {
                    Work::Empty(mut session, empty, choice) => {
                        let error = session
                            .resolve_empty(&empty, choice, &editor, &worker_requests, &token)
                            .err();
                        Ok((session, error))
                    }
                    Work::Resume(mut session) => {
                        let error = session.resume(&editor, &worker_requests, &token).err();
                        Ok((session, error))
                    }
                    Work::Saved(plan) => plan
                        .save_and_execute(
                            &editor,
                            Some(&worker_requests),
                            &token,
                            &Default::default(),
                        )
                        .map(|saved| (Session::Saved(saved), None)),
                    Work::Prepared(plan) => Ok((
                        Session::Ordinary(plan.execute(
                            &editor,
                            Some(&worker_requests),
                            &token,
                            &Default::default(),
                        )),
                        None,
                    )),
                    Work::Selection(_) => unreachable!("selection prepared before execution"),
                };
                let results = result.map(|(session, error)| {
                    let (saved_details, restore_conflict) = session.saved_details();
                    let diagnostic = match (&error, saved_details) {
                        (Some(error), Some(saved)) => Some(format!("{error}\n{saved}")),
                        (Some(error), None) => Some(error.clone()),
                        (None, saved) => saved,
                    };
                    (session, diagnostic, restore_conflict, error.is_none())
                });
                let _ = tx.send(results);
            })
            .detach();
            let mut interaction = crate::ui::git_interaction::Session::new(
                id,
                this.clone(),
                requests,
                directories.clone(),
            );
            loop {
                interaction.poll(wx).await;
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Operation worker ended".to_owned()))
                    }
                };
                if let Some(result) = result {
                    let _ = this.update_in(wx, |s, w, cx| {
                        let Some(cancelled) = interaction.finish(s, w, cx) else {
                            return;
                        };
                        if s.repo.path != project_path {
                            return;
                        }
                        let (mut session, saved_diagnostic, restore_conflict, can_resume) =
                            match result {
                                Ok(result) => result,
                                Err(error) => {
                                    s.sequence_diagnostic = Some(error.clone());
                                    s.present_sequence_result(
                                        s.repo.clone(),
                                        kind,
                                        false,
                                        cancelled,
                                        Err(error),
                                        w,
                                        cx,
                                    );
                                    return;
                                }
                            };
                        let complete = session.complete() && saved_diagnostic.is_none();
                        let results = session.results();
                        if !cancelled && !saving {
                            if let Ok(plan) = results.retry_plan() {
                                if let Some(root) =
                                    results.roots.iter().find(|root| !root.complete())
                                {
                                    s.sequence_retry =
                                        Some((kind, root.repository.path.clone(), plan));
                                }
                            }
                        }
                        if kind == Operation::Revert {
                            for root in &results.roots {
                                if root.complete() {
                                    s.revert_recoveries.remove(&root.repository.path);
                                }
                            }
                        }
                        let details = results
                            .roots
                            .iter()
                            .map(|root| {
                                let status = if root.complete() {
                                    "Operation completed"
                                } else if root.result.is_none() {
                                    "Not run"
                                } else {
                                    "Operation requires attention"
                                };
                                format!(
                                    "{}: {}\n{}",
                                    root.repository.path.display(),
                                    t(status),
                                    root.targets.join("\n")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        // Keep the whole selection and untouched roots in diagnostics, not just
                        // the last command. Never automatically replay them after Continue/Abort.
                        s.sequence_diagnostic = (!complete).then(|| match &saved_diagnostic {
                            Some(saved) => format!("{details}\n\n{saved}"),
                            None => details,
                        });
                        if can_resume && !cancelled && !results.cancelled && session.resumable() {
                            let stopped = results.roots.iter().find(|root| !root.complete());
                            let empty = stopped.and_then(|root| match root.result.as_ref()? {
                                Ok(Report::CherryPick(report))
                                    if report.status == plico::cherry_pick::Status::Empty
                                        && report.recovery.is_some() =>
                                {
                                    report
                                        .empty
                                        .clone()
                                        .map(|empty| (root.repository.clone(), empty))
                                }
                                _ => None,
                            });
                            if let Some((repository, empty)) = empty {
                                s.sequence_conflict = Some((
                                    Arc::new(()),
                                    repository.path.clone(),
                                    session,
                                    continuation_editor,
                                ));
                                s.refresh_impl(false, true, None, cx);
                                s.choose_empty_commit(repository, empty, w, cx);
                                return;
                            }
                            let recovery = stopped.and_then(|root| match root.result.as_ref()? {
                                Ok(Report::Revert(report))
                                    if report.error.is_none()
                                        && report.last.as_ref().is_some_and(|last| {
                                            last.state
                                                .as_ref()
                                                .is_ok_and(|state| !state.conflicts.is_empty())
                                        }) =>
                                {
                                    report
                                        .recovery
                                        .clone()
                                        .map(|recovery| (root.repository.clone(), Some(recovery)))
                                }
                                Ok(Report::CherryPick(report))
                                    if report.status == plico::cherry_pick::Status::Conflicts
                                        && report.recovery.is_some() =>
                                {
                                    Some((root.repository.clone(), None))
                                }
                                _ => None,
                            });
                            if let Some((repository, recovery)) = recovery {
                                if let Some(recovery) = recovery {
                                    s.revert_recoveries
                                        .insert(repository.path.clone(), recovery);
                                }
                                s.sequence_conflict = Some((
                                    Arc::new(()),
                                    repository.path.clone(),
                                    session,
                                    continuation_editor,
                                ));
                                s.status_message = Some(t("Resolve conflicts to continue").into());
                                s.refresh_impl(false, true, None, cx);
                                s.open_conflicts_in(repository, w, cx);
                                return;
                            }
                        }
                        let results = session.results_mut();
                        if let Some(index) =
                            results.roots.iter().rposition(|root| root.result.is_some())
                        {
                            let root = &mut results.roots[index];
                            let repository = root.repository.clone();
                            let mut completion =
                                root.result.take().unwrap().map(|report| match report {
                                    Report::CherryPick(report) => {
                                        SequenceCompletion::cherry(report)
                                    }
                                    Report::Revert(report) => batch_completion(&repository, report),
                                });
                            if saving {
                                if let Ok(completion) = &mut completion {
                                    completion.retry = None;
                                    completion.cherry_retry = None;
                                }
                            }
                            s.present_sequence_result(
                                repository,
                                kind,
                                false,
                                cancelled || results.cancelled,
                                completion,
                                w,
                                cx,
                            );
                        } else {
                            s.present_sequence_result(
                                s.repo.clone(),
                                kind,
                                false,
                                cancelled || results.cancelled,
                                Err(saved_diagnostic
                                    .clone()
                                    .unwrap_or_else(|| "Cancelled".into())),
                                w,
                                cx,
                            );
                        }
                        if let Some(repository) = restore_conflict {
                            s.open_conflicts_in(repository, w, cx);
                        }
                        if saved_diagnostic.is_some() {
                            s.status_message = Some(t("Operation requires attention").into());
                        }
                    });
                    break;
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(25))
                    .await;
            }
        });
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    #[gpui::test]
    fn mixed_root_log_action_reports_partial_success_and_opens_the_stopped_roots_retry(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::component::Root;
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn git(path: &std::path::Path, args: &[&str]) -> String {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "{args:?}: {out:?}");
            String::from_utf8(out.stdout).unwrap().trim().into()
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-mixed-revert-ui-{}-{nonce}",
            std::process::id()
        )));
        let mut ids = Vec::new();
        for name in ["first", "stopped", "untouched"] {
            let path = fixture.0.join(name);
            std::fs::create_dir_all(&path).unwrap();
            for args in [
                vec!["init", "-q", "-b", "main"],
                vec!["config", "user.name", "Test"],
                vec!["config", "user.email", "test@example.invalid"],
                vec!["config", "commit.gpgsign", "false"],
                vec!["commit", "--allow-empty", "-qm", "base"],
            ] {
                git(&path, &args);
            }
            std::fs::write(path.join("a"), name).unwrap();
            git(&path, &["add", "."]);
            git(&path, &["commit", "-qm", "add a"]);
            ids.push(CommitId {
                hash: git(&path, &["rev-parse", "HEAD"]),
                root: Arc::new(path),
            });
        }
        std::fs::write(ids[1].root.join("a"), "unsaved disk draft").unwrap();
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(ids[0].root.as_ref().clone(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.run_log_sequence(Operation::Revert, ids.clone(), Some("true".into()), w, cx)
            })
        });
        cx.run_until_parked();
        // The editor pump polls a channel on a 25ms timer; advance the test
        // clock after the Git worker has completed, rather than sleeping.
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(25));
        cx.run_until_parked();
        assert!(!ids[0].root.join("a").exists());
        assert_eq!(
            std::fs::read_to_string(ids[1].root.join("a")).unwrap(),
            "unsaved disk draft"
        );
        assert_eq!(git(&ids[2].root, &["rev-parse", "HEAD"]), ids[2].hash);
        review.read_with(cx, |s, _| {
            assert_eq!(s.repo.path, *ids[0].root);
            assert!(!s.operation.busy());
            assert!(!s.documents.locked());
            let diagnostic = s.sequence_diagnostic.as_ref().unwrap();
            for id in &ids {
                assert!(diagnostic.contains(&id.hash));
            }
            assert!(diagnostic.contains(t("Not run")));
        });
        cx.update(|w, cx| {
            assert!(w.has_active_dialog(cx));
            w.close_dialog(cx);
        });
        // Completion refresh must not erase the multi-root result diagnostic.
        review.update(cx, |s, cx| s.refresh_impl(false, true, None, cx));
        cx.run_until_parked();
        review.read_with(cx, |s, _| assert!(s.sequence_diagnostic.is_some()));
        let completed_head = git(&ids[0].root, &["rev-parse", "HEAD"]);
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                let plan = &s.sequence_retry.as_ref().unwrap().2;
                assert_eq!(plan.selection(), ids[1..]);
                assert!(s.retry_project_sequence_with_editor(
                    &Repository::new(ids[1].root.as_ref()),
                    Operation::Revert,
                    Some("true".into()),
                    w,
                    cx,
                ));
            })
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(25));
        cx.run_until_parked();
        assert_eq!(git(&ids[0].root, &["rev-parse", "HEAD"]), completed_head);
        assert!(
            !ids[2].root.join("a").exists(),
            "retry must include the previously untouched root"
        );
        assert_eq!(
            git(&ids[1].root, &["show", "stash:a"]),
            "unsaved disk draft"
        );
        review.read_with(cx, |s, _| {
            assert!(!s.operation.busy());
            assert!(!s.documents.locked());
            assert!(s.sequence_retry.is_none());
            assert!(
                s.sequence_diagnostic
                    .as_ref()
                    .unwrap()
                    .contains(t("Saved changes retained"))
            );
            assert_eq!(
                s.status_message.as_deref(),
                Some(t("Operation requires attention"))
            );
        });
    }
    #[gpui::test]
    fn resolving_original_conflict_continues_other_roots(cx: &mut gpui::TestAppContext) {
        original_conflict_flow(cx, false, false, false, None);
    }

    #[gpui::test]
    fn cancelling_original_conflict_does_not_revive_other_roots(cx: &mut gpui::TestAppContext) {
        original_conflict_flow(cx, true, false, false, None);
    }

    #[gpui::test]
    fn saved_original_conflict_restores_all_drafts_after_continuation(
        cx: &mut gpui::TestAppContext,
    ) {
        original_conflict_flow(cx, false, true, false, None);
    }

    #[gpui::test]
    fn cancelling_saved_conflict_keeps_drafts_and_does_not_revive_other_roots(
        cx: &mut gpui::TestAppContext,
    ) {
        original_conflict_flow(cx, true, true, false, None);
    }

    #[gpui::test]
    fn cherry_conflict_continues_original_other_roots(cx: &mut gpui::TestAppContext) {
        original_conflict_flow(cx, false, false, true, None);
    }
    #[gpui::test]
    fn saved_cherry_conflict_restores_all_root_drafts(cx: &mut gpui::TestAppContext) {
        original_conflict_flow(cx, false, true, true, None);
    }
    #[gpui::test]
    fn cancelled_saved_cherry_does_not_revive_other_roots(cx: &mut gpui::TestAppContext) {
        original_conflict_flow(cx, true, true, true, None);
    }

    #[gpui::test]
    fn empty_skip_continues_saved_roots_and_restores_drafts(cx: &mut gpui::TestAppContext) {
        original_conflict_flow(cx, false, true, true, Some(EmptyChoice::Skip));
    }
    #[gpui::test]
    fn empty_keep_continues_saved_roots_and_restores_drafts(cx: &mut gpui::TestAppContext) {
        original_conflict_flow(cx, false, true, true, Some(EmptyChoice::Keep));
    }
    #[gpui::test]
    fn cancelling_empty_dialog_retains_stashes_and_pending_roots(cx: &mut gpui::TestAppContext) {
        original_conflict_flow(cx, true, true, true, Some(EmptyChoice::Skip));
    }

    fn original_conflict_flow(
        cx: &mut gpui::TestAppContext,
        cancel: bool,
        saved: bool,
        cherry: bool,
        decision: Option<EmptyChoice>,
    ) {
        use gpui::component::Root;
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn git(path: &std::path::Path, args: &[&str]) -> String {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "{args:?}: {out:?}");
            String::from_utf8(out.stdout).unwrap().trim().into()
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "plico-original-conflict-{}-{nonce}",
            std::process::id()
        )));
        let mut ids = Vec::new();
        for name in ["conflict", "later"] {
            let path = fixture.0.join(name);
            std::fs::create_dir_all(&path).unwrap();
            for args in [
                vec!["init", "-q", "-b", "main"],
                vec!["config", "user.name", "Test"],
                vec!["config", "user.email", "test@example.invalid"],
                vec!["config", "commit.gpgsign", "false"],
                vec!["commit", "--allow-empty", "-qm", "base"],
            ] {
                git(&path, &args);
            }
            if saved {
                std::fs::write(path.join("draft"), "base draft").unwrap();
                git(&path, &["add", "draft"]);
                git(&path, &["commit", "-qm", "base draft"]);
            }
            std::fs::write(path.join("a"), "original").unwrap();
            git(&path, &["add", "a"]);
            git(&path, &["commit", "-qm", "add a"]);
            ids.push(CommitId {
                root: Arc::new(path),
                hash: String::new(),
            });
            let id = ids.last_mut().unwrap();
            id.hash = git(&id.root, &["rev-parse", "HEAD"]);
        }
        if cherry {
            for id in &ids {
                git(&id.root, &["checkout", "-qb", "destination", "HEAD~"]);
            }
        }
        std::fs::write(ids[0].root.join("a"), "later modification").unwrap();
        git(&ids[0].root, &["add", "a"]);
        git(&ids[0].root, &["commit", "-qam", "later change"]);
        if saved {
            for id in &ids {
                std::fs::write(id.root.join("draft"), "staged draft").unwrap();
                git(&id.root, &["add", "draft"]);
                std::fs::write(id.root.join("draft"), "unstaged draft").unwrap();
            }
        }
        let later_head = git(&ids[1].root, &["rev-parse", "HEAD"]);
        let operation = if cherry {
            Operation::CherryPick
        } else {
            Operation::Revert
        };
        let kind = if cherry {
            Kind::CherryPick
        } else {
            Kind::Revert
        };
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(ids[0].root.as_ref().clone(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.run_until_parked();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                if saved {
                    let plan = Plan::read(&ids, kind, &Default::default()).unwrap();
                    s.run_project_sequence(
                        operation,
                        Work::Saved(plan),
                        Some("true".into()),
                        w,
                        cx,
                    );
                } else {
                    s.run_log_sequence(operation, ids.clone(), Some("true".into()), w, cx);
                }
            })
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(25));
        cx.run_until_parked();
        review.read_with(cx, |s, _| assert!(s.sequence_conflict.is_some()));
        cx.update(|w, cx| assert!(w.has_active_dialog(cx)));
        if saved {
            review.read_with(cx, |s, _| {
                assert!(matches!(
                    s.sequence_conflict.as_ref().unwrap().2,
                    Session::Saved(_)
                ))
            });
            for id in &ids {
                assert_eq!(
                    std::fs::read_to_string(id.root.join("draft")).unwrap(),
                    "base draft"
                );
                assert_eq!(git(&id.root, &["show", "stash:draft"]), "unstaged draft");
            }
        }
        if cancel && decision.is_none() {
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
            review.read_with(cx, |s, _| assert!(s.sequence_conflict.is_none()));
        }
        if cancel && decision.is_none() {
            git(&ids[0].root, &["rm", "-q", "a"]);
            cx.update(|w, cx| {
                review.update(cx, |s, cx| {
                    s.open_conflicts_in(Repository::new(ids[0].root.as_ref()), w, cx);
                })
            });
        } else {
            let snapshot = Arc::new(
                plico::conflicts::Snapshot::read(
                    &Repository::new(ids[0].root.as_ref()),
                    &Default::default(),
                )
                .unwrap(),
            );
            let file = snapshot.files[0].clone();
            cx.update(|w, cx| {
                w.close_dialog(cx);
                review.update(cx, |s, cx| {
                    s.resolve_conflict(
                        snapshot,
                        file,
                        plico::conflicts::Resolution::Accept(if decision.is_some() {
                            plico::conflicts::Side::Yours
                        } else {
                            plico::conflicts::Side::Theirs
                        }),
                        w,
                        cx,
                    );
                });
            });
        }
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(25));
        cx.run_until_parked();
        if let Some(choice) = decision {
            let empty = review.read_with(cx, |s, _| {
                let session = &s.sequence_conflict.as_ref().unwrap().2;
                let Report::CherryPick(report) = session.results().roots[0]
                    .result
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                else {
                    panic!("expected cherry report");
                };
                report.empty.clone().unwrap()
            });
            cx.update(|w, cx| assert!(w.has_active_dialog(cx)));
            if cancel {
                cx.simulate_keystrokes("escape");
                cx.run_until_parked();
                review.read_with(cx, |s, _| assert!(s.sequence_conflict.is_none()));
            } else {
                cx.update(|w, cx| {
                    w.close_dialog(cx);
                    review.update(cx, |s, cx| {
                        assert!(s.resolve_project_empty(
                            &Repository::new(ids[0].root.as_ref()),
                            &empty,
                            choice,
                            w,
                            cx
                        ))
                    });
                });
                cx.run_until_parked();
                cx.executor()
                    .advance_clock(std::time::Duration::from_millis(25));
                cx.run_until_parked();
            }
        }
        if saved {
            for id in &ids {
                if cancel {
                    assert_eq!(git(&id.root, &["show", "stash:draft"]), "unstaged draft");
                    assert_eq!(
                        std::fs::read_to_string(id.root.join("draft")).unwrap(),
                        "base draft"
                    );
                } else {
                    assert!(git(&id.root, &["stash", "list"]).is_empty());
                    assert_eq!(git(&id.root, &["show", ":draft"]), "staged draft");
                    assert_eq!(
                        std::fs::read_to_string(id.root.join("draft")).unwrap(),
                        "unstaged draft"
                    );
                }
            }
        }
        if cancel {
            assert_eq!(git(&ids[1].root, &["rev-parse", "HEAD"]), later_head);
            assert_eq!(ids[1].root.join("a").exists(), !cherry);
            assert!(
                !git(
                    &ids[0].root,
                    &[
                        "rev-parse",
                        "--verify",
                        if cherry {
                            "CHERRY_PICK_HEAD"
                        } else {
                            "REVERT_HEAD"
                        }
                    ]
                )
                .is_empty()
            );
        } else {
            assert_eq!(ids[1].root.join("a").exists(), cherry);
            review.read_with(cx, |s, _| {
                assert!(s.sequence_conflict.is_none());
                assert!(!s.operation.busy());
                assert!(!s.documents.locked());
                assert!(s.sequence_diagnostic.is_none());
            });
        }
    }
}
