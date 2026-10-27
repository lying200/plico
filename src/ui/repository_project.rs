//! Repository entries in the branch popup. Actions capture their repository;
//! browsing a nested menu never replaces Review.repo or its Log snapshot.
use super::*;
use gpui_kit::component::menu::PopupMenu;

impl Review {
    pub(super) fn repository_roots_updating(&self, cx: &App) -> bool {
        self.repository_recent_token.is_some() || self.project_roots.read(cx).snapshot().is_none()
    }

    pub(super) fn repository_popup_roots(&self) -> Vec<&plico::branch_popup::project::Root> {
        let Some(project) = &self.repository_project else {
            return vec![];
        };
        if project.roots.len() < 2 {
            return vec![];
        }
        let mut roots: Vec<_> = project
            .roots
            .iter()
            .filter(|root| {
                self.repository_search.as_ref().is_none_or(|search| {
                    search
                        .find(
                            &root
                                .repository
                                .path
                                .file_name()
                                .unwrap_or(root.repository.path.as_os_str())
                                .to_string_lossy(),
                        )
                        .is_some()
                })
            })
            .collect();
        roots.sort_by(|a, b| {
            plico::natural_order::compare(
                &a.repository.path.to_string_lossy(),
                &b.repository.path.to_string_lossy(),
            )
        });
        roots
    }
    pub(super) fn project_repository_move(
        &self,
        direction: plico::branch_navigation::Move,
    ) -> Option<plico::branch_search::Selection> {
        use plico::{branch_navigation, branch_search::Selection};
        let groups = self.repository_groups();
        let rows = plico::branch_popup::visible_rows(&groups, &self.repository_expansion, true);
        let entries = self
            .repository_actions
            .iter()
            .map(|id| Selection::Action(id.clone()))
            .chain(
                self.repository_popup_roots()
                    .into_iter()
                    .filter(|root| root.references.is_ok())
                    .map(|root| Selection::Repository(root.repository.path.clone())),
            )
            .chain(rows.iter().map(branch_navigation::row_selection))
            .collect::<Vec<_>>();
        branch_navigation::navigate_selections(
            &entries,
            self.repository_selection.as_ref(),
            direction,
            self.repository_search.is_some(),
        )
    }
    pub(super) fn repository_project_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let roots = self.repository_popup_roots();
        if roots.is_empty() {
            return vec![];
        }
        let mut rows = vec![
            div()
                .h(px(1.))
                .my_1()
                .w_full()
                .bg(rgb(POPUP_BORDER))
                .into_any_element(),
        ];
        let refreshing = self.repository_roots_updating(cx);
        for root in roots {
            let repository = root.repository.clone();
            let color = self.project_roots.read(cx).color(&repository.path);
            let label = repository
                .path
                .file_name()
                .unwrap_or(repository.path.as_os_str())
                .to_string_lossy()
                .into_owned();
            let snapshot = root.references.as_ref().ok().cloned();
            let branch = snapshot
                .as_ref()
                .and_then(|s| s.current_branch.clone())
                .or_else(|| {
                    snapshot
                        .as_ref()
                        .and_then(|s| s.head_id.as_ref().map(|id| id.chars().take(8).collect()))
                })
                .unwrap_or_default();
            let identity = plico::branch_search::Selection::Repository(repository.path.clone());
            let hover = identity.clone();
            let opened = self.repository_child_selection.as_ref() == Some(&identity);
            let selected = self.repository_selection.as_ref() == Some(&identity);
            let menu_identity = identity.clone();
            let change_owner = cx.entity().downgrade();
            let owner = cx.entity().downgrade();
            let tooltip = root
                .references
                .as_ref()
                .err()
                .cloned()
                .unwrap_or_else(|| repository.path.display().to_string());
            rows.push(
                Button::new(SharedString::from(format!(
                    "repository-root:{}",
                    repository.path.display()
                )))
                .custom(ButtonCustomVariant::new(cx).active(rgb(0x2e4773).into()))
                .selected(selected)
                .on_mouse_move(cx.listener(move |s, e, w, cx| {
                    s.repository_pointer_selection(e, hover.clone(), w, cx)
                }))
                .h(px(28.))
                .w_full()
                .tooltip(tooltip)
                .disabled(snapshot.is_none() || refreshing)
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .child(match color {
                            Some(color) => div()
                                .size(px(14.))
                                .flex_shrink_0()
                                .rounded(px(2.))
                                .bg(rgb(color))
                                .into_any_element(),
                            None => jb_icon("folder_dark.svg").size(px(14.)).into_any_element(),
                        })
                        .child(div().flex_1().min_w_0().truncate().child(label))
                        .child(div().text_color(rgb(MUTED)).child(branch))
                        .child(jb_icon("chevronRight_dark.svg")),
                )
                .dropdown_menu_with_anchor(Anchor::LeftCenter, move |menu, window, cx| {
                    let child = cx.entity().downgrade();
                    let _ = owner.update(cx, |s, cx| {
                        s.dismiss_repository_child(cx);
                        s.repository_child_menu = Some(child);
                        s.repository_child_selection = Some(menu_identity.clone());
                        s.repository_selection = Some(menu_identity.clone());
                    });
                    match &snapshot {
                        Some(snapshot) => Self::repository_root_menu(
                            menu,
                            &owner,
                            repository.clone(),
                            snapshot.clone(),
                            window,
                            cx,
                        ),
                        None => menu,
                    }
                })
                .side_start(true)
                .open(opened)
                .on_open_change(move |open, _, cx| {
                    let _ = change_owner.update(cx, |s, cx| {
                        if *open {
                            s.repository_child_selection = Some(identity.clone());
                        } else if s.repository_child_selection.as_ref() == Some(&identity) {
                            s.repository_child_selection = None;
                            s.repository_child_menu = None;
                        }
                        cx.notify();
                    });
                })
                .into_any_element(),
            );
        }
        rows
    }
    fn repository_root_menu(
        menu: PopupMenu,
        owner: &WeakEntity<Self>,
        repository: Repository,
        snapshot: Arc<Snapshot>,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let mut expanded = plico::branch_popup::Expansion::default();
        if let Some(branch) = &snapshot.current_branch {
            expanded.reveal(plico::branch_popup::Section::Local, branch, true);
        }
        Self::repository_root_menu_expanded(menu, owner, repository, snapshot, expanded, window, cx)
    }
    fn repository_root_menu_expanded(
        mut menu: PopupMenu,
        owner: &WeakEntity<Self>,
        repository: Repository,
        snapshot: Arc<Snapshot>,
        expanded: plico::branch_popup::Expansion,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let disabled = owner
            .upgrade()
            .is_none_or(|view| view.read(cx).operation.busy());
        let create_owner = owner.clone();
        let create_root = repository.clone();
        menu = menu
            .min_w(px(375.))
            .min_h(px(312.))
            .icon_size(px(16.))
            .close_on_left(true)
            .item(
                PopupMenuItem::new(t("New Branch…"))
                    .disabled(disabled || snapshot.head_id.is_none())
                    .on_click(move |_, window, cx| {
                        let _ = create_owner.update(cx, |s, cx| {
                            s.dismiss_repository_menu(cx);
                            s.new_log_branch_dialog(create_root.clone(), "HEAD".into(), window, cx);
                        });
                    }),
            );
        let revision_owner = owner.clone();
        let revision_root = repository.clone();
        let revision_snapshot = snapshot.clone();
        menu = menu
            .item(
                PopupMenuItem::new(t("Checkout Tag or Revision…"))
                    .disabled(disabled || snapshot.head_id.is_none())
                    .on_click(move |_, window, cx| {
                        let _ = revision_owner.update(cx, |s, cx| {
                            s.dismiss_repository_menu(cx);
                            s.open_checkout_input_in(
                                revision_root.clone(),
                                &revision_snapshot,
                                window,
                                cx,
                            );
                        });
                    }),
            )
            .separator();
        // Use the same reference grouping and natural sorting as the main popup.
        let current = snapshot
            .current_branch
            .as_ref()
            .map(|name| format!("refs/heads/{name}"));
        let groups = plico::branch_popup::groups(
            &snapshot.branches,
            &snapshot.tags,
            current.as_deref(),
            &Default::default(),
            &[],
            true,
            None,
        );
        use plico::branch_popup::{FolderKey, Node, VisibleNode};
        let rows = plico::branch_popup::visible_rows(&groups, &expanded, true);
        for (row_index, row) in rows.into_iter().enumerate() {
            let folder = match &row.node {
                VisibleNode::Section(section) => Some((
                    FolderKey {
                        section: *section,
                        prefix: String::new(),
                    },
                    t(match section {
                        plico::branch_popup::Section::Local => "Local",
                        plico::branch_popup::Section::Remote => "Remote",
                        plico::branch_popup::Section::Tags => "Tags",
                        plico::branch_popup::Section::Recent => "Recent",
                    })
                    .to_owned(),
                )),
                VisibleNode::Entry(Node::Folder { key, label }) => {
                    Some((key.clone(), (*label).to_owned()))
                }
                _ => None,
            };
            let indent = px(row.depth as f32 * 16.);
            if let Some((key, label)) = folder {
                let open = row.expanded;
                let mut toggled = expanded.clone();
                let is_directory = !key.prefix.is_empty();
                toggled.toggle_tree(key, &groups, true);
                let menu_handle = cx.entity().downgrade();
                let group_owner = owner.clone();
                let group_root = repository.clone();
                let group_snapshot = snapshot.clone();
                menu = menu.item(
                    PopupMenuItem::new(label)
                        .icon(jb_icon(if open {
                            "chevronDown_dark.svg"
                        } else {
                            "chevronRight_dark.svg"
                        }))
                        .disclosure(open)
                        .when(is_directory, |item| {
                            item.content_icon(jb_icon("folder_dark.svg"))
                        })
                        .on_click(move |_, window, cx| {
                            let handle = menu_handle.clone();
                            let owner = group_owner.clone();
                            let root = group_root.clone();
                            let snapshot = group_snapshot.clone();
                            let expanded = toggled.clone();
                            window.defer(cx, move |window, cx| {
                                let _ = handle.update(cx, |menu, cx| {
                                    menu.rebuild(window, cx, |menu, window, cx| {
                                        Self::repository_root_menu_expanded(
                                            menu, &owner, root, snapshot, expanded, window, cx,
                                        )
                                        .select_item(row_index + 3)
                                    });
                                });
                            });
                        }),
                );
                menu = menu.indent_last(indent);
                continue;
            }
            let VisibleNode::Entry(Node::Ref { reference, label }) = row.node else {
                continue;
            };
            let name = reference.full_name.to_owned();
            let owner = owner.clone();
            let root = repository.clone();
            let refs = snapshot.clone();
            let icon = if reference.current {
                "currentBranchLabel_dark.svg"
            } else if row.section == plico::branch_popup::Section::Tags {
                "branchLabel_dark.svg"
            } else {
                "vcs_dark.svg"
            };
            menu = menu
                .submenu_with_secondary(
                    Some(jb_icon(icon).text_color(rgb(if reference.current {
                        0xf4af3d
                    } else {
                        FG
                    }))),
                    label.to_owned(),
                    snapshot
                        .tracked_branches
                        .get(reference.full_name)
                        .cloned()
                        .map(SharedString::from),
                    window,
                    cx,
                    move |mut menu, _, _| {
                        if let Some((target, head)) =
                            Self::reference_checkout_in_snapshot(&refs, &name)
                        {
                            let owner = owner.clone();
                            let root = root.clone();
                            let name = name.clone();
                            menu = menu.item(
                                PopupMenuItem::new(t("Checkout"))
                                    .disabled(disabled)
                                    .on_click(move |_, window, cx| {
                                        let _ = owner.update(cx, |s, cx| {
                                            s.dismiss_repository_menu(cx);
                                            if name.starts_with("refs/heads/") {
                                                s.start_local_checkout_in(
                                                    root.clone(),
                                                    name.clone(),
                                                    target.clone(),
                                                    head.clone(),
                                                    window,
                                                    cx,
                                                );
                                            } else {
                                                s.prepare_reference_checkout_in(
                                                    root.clone(),
                                                    name.clone(),
                                                    target.clone(),
                                                    head.clone(),
                                                    None,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        });
                                    }),
                            );
                        }
                        let create_owner = owner.clone();
                        let create_root = root.clone();
                        let source = name.clone();
                        menu = menu.item(
                            PopupMenuItem::new(t("New Branch…"))
                                .disabled(disabled)
                                .on_click(move |_, window, cx| {
                                    let _ = create_owner.update(cx, |s, cx| {
                                        s.dismiss_repository_menu(cx);
                                        s.new_log_branch_dialog(
                                            create_root.clone(),
                                            source.clone(),
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        );
                        if name.starts_with("refs/heads/") || name.starts_with("refs/remotes/") {
                            menu = menu.separator().item(Self::branch_compare_item_in(
                                &owner,
                                root.clone(),
                                Some(&refs),
                                &name,
                            ));
                        }
                        menu = menu.item(Self::branch_working_comparison_item_in(
                            &owner,
                            root.clone(),
                            Some(&refs),
                            &name,
                        ));
                        if name.starts_with("refs/heads/") {
                            menu = menu.separator();
                        }
                        if name.starts_with("refs/heads/") {
                            if let Some(branch) =
                                refs.branches.iter().find(|branch| branch.name == name)
                            {
                                let rename_owner = owner.clone();
                                let rename_root = root.clone();
                                let rename_ref = name.clone();
                                let tip = branch.id.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(t("Rename…"))
                                        .disabled(disabled)
                                        .on_click(move |_, window, cx| {
                                            let _ = rename_owner.update(cx, |s, cx| {
                                                s.prepare_rename_branch_in(
                                                    rename_root.clone(),
                                                    rename_ref.clone(),
                                                    tip.clone(),
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }),
                                );
                            }
                        }
                        if name.starts_with("refs/heads/") {
                            let delete_owner = owner.clone();
                            let delete_root = root.clone();
                            let delete_ref = name.clone();
                            let current = refs
                                .current_branch
                                .as_ref()
                                .is_some_and(|branch| name == format!("refs/heads/{branch}"));
                            menu = menu.item(
                                PopupMenuItem::new(t("Delete…"))
                                    .disabled(disabled || current)
                                    .on_click(move |_, w, cx| {
                                        let _ = delete_owner.update(cx, |s, cx| {
                                            s.prepare_delete_branch_in(
                                                delete_root.clone(),
                                                delete_ref.clone(),
                                                w,
                                                cx,
                                            )
                                        });
                                    }),
                            );
                        }
                        let copy = name.clone();
                        menu.item(PopupMenuItem::new(t("Copy Reference Name")).on_click(
                            move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                            },
                        ))
                    },
                )
                .indent_last(indent);
        }
        menu
    }
}
