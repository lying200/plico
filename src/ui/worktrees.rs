//! Rebased Worktrees tool window: repository-scoped list and explicit actions.
use super::*;
use plico::worktrees::{self, Tree};
mod form;

pub(super) struct Browser {
    repo: Repository,
    owner: WeakEntity<Review>,
    trees: Vec<Tree>,
    selected: Option<PathBuf>,
    loading: bool,
    error: Option<String>,
    token: Cancel,
    task: Task<()>,
    poll: Task<()>,
    scroll: UniformListScrollHandle,
}
impl Drop for Browser {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Browser {
    fn create(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let occupied = self.trees.clone();
        let _ = self
            .owner
            .update(cx, |s, cx| s.new_worktree(None, occupied, w, cx));
    }
    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.token.cancel();
        self.token = Cancel::default();
        let token = self.token.clone();
        let repo = self.repo.clone();
        self.loading = true;
        self.task = cx.spawn(async move |this, cx| {
            let check = token.clone();
            let result = cx
                .background_spawn(async move { worktrees::list(&repo, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if check.cancelled() {
                    return;
                }
                s.loading = false;
                match result {
                    Ok(trees) => {
                        s.trees = trees;
                        s.error = None;
                        if !s.trees.iter().any(|t| Some(&t.path) == s.selected.as_ref()) {
                            s.selected = s
                                .trees
                                .iter()
                                .find(|t| t.current)
                                .or_else(|| s.trees.first())
                                .map(|t| t.path.clone());
                        }
                    }
                    Err(e) => s.error = Some(e),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn selected(&self) -> Option<&Tree> {
        self.trees
            .iter()
            .find(|t| Some(&t.path) == self.selected.as_ref())
    }
    fn open(&self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(t) = self
            .selected()
            .filter(|t| !t.bare && !t.current && !t.prunable)
        {
            let path = t.path.clone();
            let _ = self
                .owner
                .update(cx, |s, cx| s.open_worktree_project(path, w, cx));
        }
    }
}
impl Render for Browser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self
            .owner
            .upgrade()
            .is_none_or(|s| s.read(cx).operation.busy());
        let removable = !busy
            && self
                .selected()
                .is_some_and(|t| !t.main && !t.current && !t.bare);
        v_flex()
            .size_full()
            .bg(rgb(BG))
            .rounded(px(10.))
            .overflow_hidden()
            .child(
                h_flex()
                    .h(px(32.))
                    .w_full()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .px_3()
                    .gap(px(16.))
                    .child(div().font_weight(FontWeight::BOLD).child("Git"))
                    .child(
                        div()
                            .px_2()
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_1()
                            .border_color(rgb(0x37598b))
                            .bg(rgb(0x2e4773))
                            .rounded(px(4.))
                            .child(t("Worktrees"))
                            .child(
                                Button::new("close-worktrees-tab")
                                    .ghost()
                                    .xsmall()
                                    .icon(jb_icon("close_dark.svg"))
                                    .tooltip(t("Close"))
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        let _ = s.owner.update(cx, |s, cx| {
                                            s.worktrees_visible = false;
                                            s.worktrees = None;
                                            cx.notify();
                                        });
                                    })),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("close-worktrees")
                            .ghost()
                            .xsmall()
                            .icon(jb_icon("hideToolWindow_dark.svg"))
                            .tooltip(t("Hide"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                let _ = s.owner.update(cx, |s, cx| {
                                    s.worktrees_visible = false;
                                    cx.notify();
                                });
                            })),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .items_stretch()
                    .child(
                        v_flex()
                            .w(px(36.))
                            .flex_shrink_0()
                            .items_center()
                            .pt_1()
                            .gap_1()
                            .child(
                                Button::new("add-worktree")
                                    .ghost()
                                    .small()
                                    .icon_size(px(16.))
                                    .icon(jb_icon("add_dark.svg"))
                                    .tooltip(t("New Worktree"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|s, _, w, cx| s.create(w, cx))),
                            )
                            .child(
                                Button::new("remove-worktree")
                                    .ghost()
                                    .small()
                                    .icon_size(px(16.))
                                    .icon(jb_icon("delete_dark.svg"))
                                    .tooltip(t("Delete Worktree"))
                                    .disabled(!removable)
                                    .on_click(cx.listener(|s, _, w, cx| {
                                        if let Some(t) = s.selected().cloned() {
                                            let _ = s.owner.update(cx, |s, cx| {
                                                s.confirm_remove_worktree(t, w, cx)
                                            });
                                        }
                                    })),
                            )
                            .child(
                                Button::new("refresh-worktrees")
                                    .ghost()
                                    .small()
                                    .icon_size(px(16.))
                                    .icon(jb_icon("refresh_dark.svg"))
                                    .tooltip(t("Refresh"))
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|s, _, _, cx| s.reload(cx))),
                            )
                            .child(
                                Button::new("prune-worktrees")
                                    .ghost()
                                    .small()
                                    .icon_size(px(16.))
                                    .icon(jb_icon("clearCash_dark.svg"))
                                    .tooltip(t("Prune Worktrees"))
                                    .disabled(
                                        busy || self.loading
                                            || !self.trees.iter().any(|t| t.prunable),
                                    )
                                    .on_click(cx.listener(|s, _, w, cx| {
                                        let paths = s
                                            .trees
                                            .iter()
                                            .filter(|t| t.prunable)
                                            .map(|t| t.path.clone())
                                            .collect();
                                        let _ = s.owner.update(cx, |s, cx| {
                                            s.confirm_prune_worktrees(paths, w, cx)
                                        });
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(
                                uniform_list(
                                    "worktrees-list",
                                    self.trees.len(),
                                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                                        range
                                            .map(|i| {
                                                let tree = s.trees[i].clone();
                                                let path = tree.path.clone();
                                                h_flex()
                                                    .id(i)
                                                    .h(px(28.))
                                                    .gap_3()
                                                    .px_2()
                                                    .w_full()
                                                    .overflow_hidden()
                                                    .when(s.selected.as_ref() == Some(&path), |v| {
                                                        v.bg(rgb(0x2e4773))
                                                    })
                                                    .child(div().w(px(14.)).flex_shrink_0().child(
                                                        if tree.current { "✓" } else { "" },
                                                    ))
                                                    .child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .truncate()
                                                            .when(tree.main, |v| {
                                                                v.font_weight(FontWeight::BOLD)
                                                            })
                                                            .child(
                                                                path.file_name()
                                                                    .unwrap_or_default()
                                                                    .to_string_lossy()
                                                                    .into_owned(),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .truncate()
                                                            .text_color(rgb(MUTED))
                                                            .child(
                                                                tree.branch
                                                                    .as_deref()
                                                                    .and_then(|b| {
                                                                        b.strip_prefix(
                                                                            "refs/heads/",
                                                                        )
                                                                    })
                                                                    .map(str::to_owned)
                                                                    .unwrap_or_else(|| {
                                                                        tree.head
                                                                            .chars()
                                                                            .take(8)
                                                                            .collect()
                                                                    }),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .truncate()
                                                            .text_color(rgb(MUTED))
                                                            .child(project_display_path(&path)),
                                                    )
                                                    .when(tree.locked, |v| v.child(t("Locked")))
                                                    .when(tree.prunable, |v| v.child(t("Prunable")))
                                                    .on_click(cx.listener(
                                                        move |s, event: &ClickEvent, w, cx| {
                                                            s.selected = Some(path.clone());
                                                            if event.click_count() == 2 {
                                                                s.open(w, cx);
                                                            }
                                                            cx.notify();
                                                        },
                                                    ))
                                            })
                                            .collect::<Vec<_>>()
                                    }),
                                )
                                .flex_1()
                                .min_h_0()
                                .track_scroll(&self.scroll),
                            )
                            .when(self.loading && self.trees.is_empty(), |v| {
                                v.child(t("Loading…"))
                            })
                            .when_some(self.error.clone(), |v, e| {
                                v.child(div().px_2().text_color(rgb(0xf2a6a6)).child(e))
                            }),
                    ),
            )
    }
}
impl Review {
    fn focus_open_worktree(
        &self,
        path: &std::path::Path,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let identity = project_identity(path);
        if project_identity(&self.repo.path) == identity {
            w.activate_window();
            return true;
        }
        let own = w.window_handle().window_id();
        let existing = cx
            .windows()
            .into_iter()
            .filter(|h| h.window_id() != own)
            .find(|h| {
                h.downcast::<gpui_kit::base::Root>()
                    .and_then(|root| root.read(cx).ok()?.view().clone().downcast::<Review>().ok())
                    .is_some_and(|r| project_identity(&r.read(cx).repo.path) == identity)
            });
        existing.is_some_and(|handle| handle.update(cx, |_, w, _| w.activate_window()).is_ok())
    }
    fn open_worktree_project(&mut self, path: PathBuf, w: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || self.settings_open {
            return;
        }
        self.worktree_open_task = Task::ready(());
        if self.focus_open_worktree(&path, w, cx) {
            return;
        }
        let root = self.repo.path.clone();
        self.worktree_open_task = cx.spawn_in(w, async move |this, wx| {
            let (path, exists) = wx
                .background_spawn(async move {
                    let exists = path.is_dir();
                    (path, exists)
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.repo.path != root || s.operation.busy() || s.settings_open {
                    return;
                }
                if !exists {
                    s.reload_worktrees(cx);
                    s.error = Some(format!(
                        "{}: {}",
                        t("Project folder is unavailable"),
                        project_display_path(&path)
                    ));
                    cx.notify();
                } else if !s.focus_open_worktree(&path, w, cx) {
                    s.choose_project_window(path, w, cx);
                }
            });
        });
    }
    pub(super) fn open_worktrees(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.welcome {
            return;
        }
        self.worktrees_visible = true;
        if self.worktrees.is_some() {
            self.reload_worktrees(cx);
            cx.notify();
            return;
        }
        let repo = self.repo.clone();
        let owner = cx.entity().downgrade();
        self.worktrees = Some(cx.new(|cx| {
            let mut browser = Browser {
                repo,
                owner,
                trees: vec![],
                selected: None,
                loading: false,
                error: None,
                token: Cancel::default(),
                task: Task::ready(()),
                poll: Task::ready(()),
                scroll: UniformListScrollHandle::new(),
            };
            browser.reload(cx);
            browser.poll = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(5))
                        .await;
                    if this
                        .update(cx, |s, cx| {
                            if s.owner
                                .upgrade()
                                .is_some_and(|o| o.read(cx).worktrees_visible)
                            {
                                s.reload(cx);
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            browser
        }));
        cx.notify();
    }
    fn reload_worktrees(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = &self.worktrees {
            view.update(cx, |s, cx| s.reload(cx));
        }
    }
    fn confirm_remove_worktree(&mut self, tree: Tree, w: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() || tree.current || tree.main || tree.bare {
            return;
        }
        let owner = cx.entity().downgrade();
        w.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let tree = tree.clone();
            dialog
                .title(t("Delete Worktree"))
                .w(px(540.))
                .centered(true)
                .child(div().child(t("Delete worktree at")))
                .child(
                    div()
                        .whitespace_normal()
                        .child(project_display_path(&tree.path)),
                )
                .footer(crate::dialog_style::confirmation(
                    Button::new("delete-worktree-confirm")
                        .primary()
                        .label(t("Delete Worktree")),
                ))
                .on_ok(move |_, w, cx| {
                    let owner = owner.clone();
                    let tree = tree.clone();
                    let handle = w.window_handle();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, w, cx| {
                            let _ = owner.update(cx, |s, cx| s.remove_open_worktree(tree, w, cx));
                        });
                    });
                    true
                })
        });
    }
    fn remove_open_worktree(&mut self, tree: Tree, w: &mut Window, cx: &mut Context<Self>) {
        if self.operation.busy() {
            return;
        }
        let own_id = w.window_handle().window_id();
        let open = cx
            .windows()
            .into_iter()
            .filter(|h| h.window_id() != own_id)
            .find_map(|handle| {
                let root = handle.downcast::<gpui_kit::base::Root>()?;
                let review = root
                    .read(cx)
                    .ok()?
                    .view()
                    .clone()
                    .downcast::<Review>()
                    .ok()?;
                (project_identity(&review.read(cx).repo.path) == project_identity(&tree.path))
                    .then_some((handle, review))
            });
        let Some((handle, review)) = open else {
            self.remove_worktree(tree, w, cx);
            return;
        };
        let owner = cx.entity().downgrade();
        let original = w.window_handle();
        w.open_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            let tree = tree.clone();
            let review = review.clone();
            dialog
                .title(t("Delete Worktree"))
                .w(px(540.))
                .centered(true)
                .child(
                    div()
                        .whitespace_normal()
                        .child(project_display_path(&tree.path)),
                )
                .child(t(
                    "This worktree is open. Close its window before deleting it?",
                ))
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogAction::new().child(
                                Button::new("close-delete-worktree")
                                    .primary()
                                    .label(t("Close and Delete")),
                            ),
                        )
                        .child(
                            gpui_kit::component::dialog::DialogClose::new()
                                .trigger(|b| b.label(t("Do Not Delete"))),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    let review = review.clone();
                    let tree = tree.clone();
                    let owner = owner.clone();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, other, cx| {
                            review.update(cx, |s, cx| {
                                if project_identity(&s.repo.path) != project_identity(&tree.path)
                                    || s.operation.busy()
                                {
                                    return;
                                }
                                s.with_saved_documents(other, cx, move |_, other, cx| {
                                    other.remove_window();
                                    cx.defer(move |cx| {
                                        let _ = original.update(cx, |_, w, cx| {
                                            let _ = owner.update(cx, |s, cx| {
                                                s.remove_open_worktree(tree, w, cx)
                                            });
                                        });
                                    });
                                });
                            });
                        });
                    });
                    true
                })
        });
    }
    fn confirm_prune_worktrees(
        &mut self,
        paths: Vec<PathBuf>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation.busy() || paths.is_empty() {
            return;
        }
        let owner = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let paths = paths.clone();
            let owner = owner.clone();
            d.title(t("Prune Worktrees"))
                .w(px(500.))
                .centered(true)
                .child(t("Remove Git records for missing worktrees:"))
                .child(
                    div().whitespace_normal().child(
                        paths
                            .iter()
                            .map(|p| p.file_name().unwrap_or_default().to_string_lossy())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogAction::new().child(
                                Button::new("prune-worktrees-confirm")
                                    .primary()
                                    .label(t("Prune Worktrees")),
                            ),
                        )
                        .child(
                            gpui_kit::component::dialog::DialogClose::new()
                                .trigger(|b| b.label(t("Cancel"))),
                        ),
                )
                .on_ok(move |_, w, cx| {
                    let _ = owner.update(cx, |s, cx| s.prune_worktrees(paths.clone(), w, cx));
                    true
                })
        });
    }
    fn prune_worktrees(&mut self, paths: Vec<PathBuf>, w: &mut Window, cx: &mut Context<Self>) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Worktree,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.status_message = Some(t("Pruning worktrees…").into());
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let result = wx
                .background_spawn(async move {
                    worktrees::prune(&repo, &paths, &token, &Default::default())
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_operation(id, cx).is_some() {
                    s.finish_worktree(result, None, w, cx);
                }
            });
        });
    }
    fn remove_worktree(&mut self, tree: Tree, w: &mut Window, cx: &mut Context<Self>) {
        let Some((id, token)) = self.begin_operation(
            plico::active_operation::Kind::Worktree,
            plico::active_operation::Phase::Executing,
            cx,
        ) else {
            return;
        };
        let repo = self.repo.clone();
        self.status_message = Some(t("Deleting worktree…").into());
        self.operation_task = cx.spawn_in(w, async move |this, wx| {
            let (result, recent) = wx
                .background_spawn(async move {
                    let result = worktrees::remove(&repo, &tree, &token, &Default::default());
                    // Match notifyWorkingTreeDeletedSuccess: do not drop a recent
                    // project after a partial deletion or a cancelled operation.
                    let recent = result
                        .as_ref()
                        .is_ok_and(|r| r.succeeded())
                        .then(|| plico::recent_projects::remove(&tree.path));
                    (result, recent)
                })
                .await;
            let _ = this.update_in(wx, |s, w, cx| {
                if s.finish_operation(id, cx).is_none() {
                    return;
                }
                s.finish_worktree(result, None, w, cx);
                if let Some(recent) = recent {
                    match recent {
                        Ok(paths) => s.recent_projects = paths,
                        Err(error) => s.error = Some(error.to_string()),
                    }
                    cx.notify();
                }
            });
        });
    }
    fn finish_worktree(
        &mut self,
        result: Result<worktrees::Report, String>,
        open: Option<PathBuf>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let error = match result {
            Ok(r) if r.succeeded() => None,
            Ok(r) => Some(r.diagnostic()),
            Err(e) => Some(e),
        };
        self.status_message = Some(
            t(if error.is_some() {
                "Worktree operation needs attention"
            } else {
                "Worktrees updated"
            })
            .into(),
        );
        self.reload_worktrees(cx);
        self.refresh_impl(false, true, None, cx);
        if let Some(e) = error {
            crate::error_ui::show_details(e, "Worktree operation needs attention", w, cx);
        } else if let Some(path) = open {
            self.open_worktree_project(path, w, cx);
        }
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    #[gpui::test]
    fn opening_existing_worktree_reuses_window_without_open_dialog(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let path = std::env::temp_dir().join("plico-open-worktree-window-fixture");
        cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, w, cx));
            view.update(cx, |s, _| s.repo = Repository::new(&path));
            gpui::component::Root::new(view, w, cx)
        });
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, w, cx));
            review = Some(view.clone());
            gpui::component::Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.update(|w, cx| {
            let count = cx.windows().len();
            review.update(cx, |s, cx| {
                assert!(s.focus_open_worktree(&path, w, cx));
                assert!(!s.focus_open_worktree(&path.with_file_name("other-worktree"), w, cx));
                s.open_worktree_project(path, w, cx);
            });
            assert_eq!(cx.windows().len(), count);
            assert!(!w.has_active_dialog(cx));
        });
    }
    #[gpui::test]
    fn create_from_list_opens_dialog_without_reborrowing_list(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, w, cx));
            review = Some(view.clone());
            gpui::component::Root::new(view, w, cx)
        });
        let review = review.unwrap();
        cx.update(|w, cx| {
            review.update(cx, |s, cx| {
                s.welcome = false;
                s.open_worktrees(w, cx);
            });
            let browser = review.read(cx).worktrees.clone().unwrap();
            browser.update(cx, |s, cx| s.create(w, cx));
            assert!(
                w.has_active_dialog(cx),
                "Add must open the New Worktree dialog"
            );
        });
    }
}
