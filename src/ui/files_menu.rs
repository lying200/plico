//! Actions capture displayed file identities, never mutable row indices.
use super::*;
use gpui_kit::component::menu::PopupMenu;

impl Review {
    pub(super) fn file_row_targets(&self, row: &FileRow) -> Vec<diff_target::Target> {
        self.files
            .iter()
            .enumerate()
            .filter_map(|(i, change)| {
                let included = match row {
                    FileRow::File(index, _) => i == *index,
                    FileRow::Group(area, ..) => change.area == *area,
                    FileRow::Folder(root, area, path, ..) => {
                        change.area == *area
                            && self.file_root(i).as_ref() == Some(root)
                            && change.path.starts_with(path)
                    }
                };
                if !included {
                    return None;
                }
                if let Some(mut chain) = self.aggregate_chain.clone() {
                    return chain.select(i);
                }
                let commit = if self.local {
                    None
                } else {
                    Some(self.log.commits().get(self.selected?)?.clone())
                };
                diff_target::Target::new(self.log_files_repository()?, commit, change.clone()).ok()
            })
            .collect()
    }
    pub(super) fn files_context_menu(
        &self,
        mut menu: PopupMenu,
        targets: Vec<diff_target::Target>,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        let owner = cx.entity().downgrade();
        let diff_targets = targets.clone();
        menu = menu.min_w(px(230.)).item(
            PopupMenuItem::new(t("Show Diff"))
                .disabled(
                    targets.is_empty() || targets.iter().any(|t| t.committed_endpoints().is_none()),
                )
                .on_click(move |_, w, cx| {
                    let _ = owner.update(cx, |s, cx| {
                        if !diff_targets.is_empty() {
                            let endpoints = diff_targets
                                .iter()
                                .map(|t| {
                                    Some((
                                        Repository::new(&t.identity().repository),
                                        vec![t.committed_endpoints()?],
                                    ))
                                })
                                .collect::<Option<Vec<_>>>();
                            if let Some(endpoints) = endpoints {
                                if let Ok((target, chain)) =
                                    diff_target::Chain::combined_roots(endpoints, 0)
                                {
                                    s.open_files_menu_diff(target, chain, w, cx);
                                }
                            }
                        }
                    });
                }),
        );
        let owner = cx.entity().downgrade();
        let compare_targets = targets.clone();
        menu = menu.item(
            PopupMenuItem::new(t("Compare with Local"))
                .disabled(
                    targets.is_empty()
                        || targets.iter().any(|t| t.committed_endpoints().is_none())
                        || targets.windows(2).any(|pair| {
                            pair[0].identity().repository != pair[1].identity().repository
                        }),
                )
                .on_click(move |_, w, cx| {
                    let targets = compare_targets.clone();
                    let _ = owner.update(cx, |s, cx| {
                        let root = s.repo.path.clone();
                        cx.spawn_in(w, async move |this, wx| {
                            let result = wx
                                .background_spawn(async move {
                                    plico::change_comparison::with_local(
                                        &targets,
                                        &Cancel::default(),
                                    )
                                })
                                .await;
                            let _ = this.update_in(wx, |s, w, cx| {
                                if s.repo.path != root {
                                    return;
                                }
                                match result {
                                    Ok((target, chain)) => {
                                        s.open_files_menu_diff(target, chain, w, cx)
                                    }
                                    Err(error) => crate::error_ui::show_details(
                                        error,
                                        "Could not compare with local",
                                        w,
                                        cx,
                                    ),
                                }
                            });
                        })
                        .detach();
                    });
                }),
        );
        let requests = targets
            .iter()
            .map(|target| {
                let after = target.committed_endpoints()?.after?;
                Some(crate::history_view::GetRevision {
                    repository: target.identity().repository.clone(),
                    path: after.path.clone(),
                    revision: plico::file_history::Revision {
                        id: after.revision,
                        path: after.path,
                        parents: vec![],
                        timestamp: 0,
                        author: String::new(),
                        message: String::new(),
                        deleted: false,
                    },
                })
            })
            .collect::<Option<Vec<_>>>();
        let enabled = requests
            .as_ref()
            .is_some_and(|rs| !rs.is_empty() && rs.iter().all(|r| r.repository == self.repo.path))
            && !self.operation.busy();
        let owner = cx.entity().downgrade();
        menu.separator().item(
            PopupMenuItem::new(t("Get from Revision"))
                .disabled(!enabled)
                .on_click(move |_, w, cx| {
                    if let Some(requests) = requests.clone() {
                        let _ = owner.update(cx, |s, cx| s.get_file_revisions(requests, w, cx));
                    }
                }),
        )
    }
    fn open_files_menu_diff(
        &mut self,
        target: diff_target::Target,
        chain: diff_target::Chain,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Reopening a file via a different directory selection must replace
        // its navigation scope, rather than retain an older one-file chain.
        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.read(cx).target.identity() == target.identity())
        {
            self.tabs[index].update(cx, |tab, cx| tab.replace_target(target, chain, cx));
            self.activate_diff_tab(index, w, cx);
            return;
        }
        let tab = cx.new(|_| {
            let mut tab = DiffTab::new(target, chain);
            tab.documents = self.documents.clone();
            tab
        });
        self.subscribe_diff_navigation(&tab, cx);
        cx.observe(&tab, |_, _, cx| cx.notify()).detach();
        self.insert_editor_tab(tab.entity_id());
        self.tabs.push(tab);
        let index = self.tabs.len() - 1;
        self.activate_diff_tab(index, w, cx);
    }
}
