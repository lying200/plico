//! Main-window log selection coordination.
//! Private implementation; shared lifecycle state remains owned by Review.
use super::*;

impl Review {
    pub(super) fn select_commit(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.history_loading {
            return;
        }
        let ids = self.commit_ids();
        self.log_selection.select(&ids, index, false, false);
        self.load_log_selection(cx);
    }
    pub(super) fn commit_ids(&self) -> Vec<String> {
        self.commits.iter().map(|c| c.id.clone()).collect()
    }
    pub(super) fn select_log_row(
        &mut self,
        index: usize,
        toggle: bool,
        extend: bool,
        context: bool,
        cx: &mut Context<Self>,
    ) {
        if self.history_loading {
            return;
        }
        let ids = self.commit_ids();
        let before = self
            .log_selection
            .ordered(&ids)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if context {
            self.log_selection.context(&ids, index);
        } else {
            self.log_selection.select(&ids, index, toggle, extend);
        }
        if before.iter().map(String::as_str).collect::<Vec<_>>() != self.log_selection.ordered(&ids)
        {
            self.load_log_selection(cx);
        }
        cx.notify();
    }
    pub(super) fn load_log_selection(&mut self, cx: &mut Context<Self>) {
        self.reword_gate = None;
        let rows = self.commit_ids();
        let ids = self
            .log_selection
            .ordered(&rows)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let preferred = self
            .selected_file
            .and_then(|i| self.files.get(i))
            .cloned()
            .or_else(|| self.pending_preview_file.clone());
        self.aggregate_chain = None;
        self.selection_details.clear();
        self.selection_branches = None;
        self.local_preview = None;
        if ids.len() == 1 {
            if let Some(index) = rows.iter().position(|id| id == &ids[0]) {
                self.select_commit_with_file(index, preferred, cx);
            }
            return;
        }
        let (generation, token) = self.reset_job(cx);
        self.selected = None;
        self.selected_file = None;
        self.files = Arc::default();
        self.rebuild_files();
        self.message.clear();
        self.commit_metadata = Default::default();
        self.containing_branches = None;
        self.base = None;
        self.pending_preview_file = preferred.clone();
        self.busy = !ids.is_empty();
        if ids.is_empty() {
            self.pending_preview_file = None;
            cx.notify();
            return;
        }
        let repo = self.repo.clone();
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let snapshot = plico::log_changes::load(&repo, &ids, &token)?;
                    let mut presentations = Vec::new();
                    let mut presentation_bytes = 0usize;
                    for commit in snapshot.commits.iter().take(50) {
                        let details = repo.commit_metadata(commit, &token)?;
                        let presentation = commit_presentation::build(&details, Some(commit));
                        presentation_bytes = presentation_bytes
                            .saturating_add(presentation.message.len())
                            .saturating_add(
                                presentation.metadata.iter().map(String::len).sum::<usize>(),
                            );
                        if presentation_bytes > git::OUTPUT_LIMIT {
                            return Err("Log details exceed the preview output limit".into());
                        }
                        presentations.push((commit.refs.clone(), presentation, commit.id.clone()));
                    }
                    let chain = if snapshot.files.is_empty() {
                        None
                    } else {
                        Some(diff_target::Chain::combined(repo, snapshot.files, 0)?.1)
                    };
                    Ok::<_, String>((chain, presentations))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                this.pending_preview_file = None;
                match result {
                    Ok((chain, presentations)) => {
                        this.files = Arc::new(
                            chain
                                .as_ref()
                                .map(|c| c.changes().to_vec())
                                .unwrap_or_default(),
                        );
                        this.aggregate_chain = chain;
                        let mut cards = Vec::new();
                        this.selection_details = presentations
                            .into_iter()
                            .map(|(refs, presentation, id)| {
                                let view =
                                    cx.new(crate::containing_branches::ContainingBranches::pending);
                                cards.push((id, view.downgrade()));
                                (refs, presentation, view)
                            })
                            .collect();
                        let repo = this.repo.clone();
                        this.selection_branches = Some(
                            cx.new(|cx| crate::containing_branches::Batch::new(repo, cards, cx)),
                        );
                        this.prune_directory_state();
                        this.rebuild_files();
                        if let Some(index) =
                            plico::preview_selection::index(&this.files, preferred.as_ref())
                        {
                            this.load_file(index, cx);
                        }
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    pub(super) fn select_commit_with_file(
        &mut self,
        index: usize,
        preferred: Option<Change>,
        cx: &mut Context<Self>,
    ) {
        if self.history_loading {
            return;
        }
        let Some(commit) = self.commits.get(index).cloned() else {
            return;
        };
        self.reword_gate = Some(cx.new(|cx| super::reword::Gate::new(self.repo.clone(), commit.id.clone(), cx)));
        let (generation, token) = self.reset_job(cx);
        self.aggregate_chain = None;
        self.selection_details.clear();
        self.selected = Some(index);
        self.pending_preview_file = preferred.clone();
        self.selected_file = None;
        self.files = Arc::default();
        self.rebuild_files();
        self.busy = true;
        self.message.clear();
        self.commit_metadata = Default::default();
        self.containing_branches = None;
        self.base = None;
        self.containing_branches = Some(cx.new(|cx| {
            crate::containing_branches::ContainingBranches::new(
                self.repo.clone(),
                commit.id.clone(),
                cx,
            )
        }));
        let repo = self.repo.clone();
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { repo.details(&commit, &token) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                match result {
                    Ok(d) => {
                        let commit = this.selected.and_then(|i| this.commits.get(i));
                        let presentation = commit_presentation::build(&d, commit);
                        this.message = presentation.message;
                        this.commit_metadata = presentation.metadata;
                        this.base = Some(if commit.is_some_and(|commit| commit.shallow_boundary) {
                            i18n::ComparisonTitle::Label(
                                "Shallow boundary · Parent comparison unavailable",
                            )
                        } else {
                            i18n::ComparisonTitle::Parent {
                                revision: d.base,
                                merge: commit.is_some_and(|commit| commit.parents.len() > 1),
                            }
                        });
                        this.files = Arc::new(d.files);
                        this.prune_directory_state();
                        this.rebuild_files();
                        this.pending_preview_file = None;
                        if let Some(index) =
                            plico::preview_selection::index(&this.files, preferred.as_ref())
                        {
                            this.load_file(index, cx);
                        }
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
}
