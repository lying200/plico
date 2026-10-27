use gpui_kit::component::{Disableable as _, Icon, checkbox::Checkbox, h_flex, v_flex};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    git::{Cancel, Repository},
    path_tree::{self, Node, Selection, State},
};
use std::collections::BTreeMap;

actions!(
    path_chooser,
    [
        PreviousPath,
        NextPath,
        TogglePath,
        ExpandPath,
        CollapsePath,
        ToggleExpandedPath
    ]
);
pub struct PathChooser {
    repos: Vec<Repository>,
    selection: Selection,
    directories: BTreeMap<String, Vec<Node>>,
    rows: Vec<(Node, usize)>,
    active: usize,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    loading: Option<String>,
    error: Option<String>,
    initializing: bool,
    generation: u64,
    token: Cancel,
    task: Task<()>,
}
impl Drop for PathChooser {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl PathChooser {
    pub fn new(
        repos: Vec<Repository>,
        initial: Vec<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.bind_keys([
            KeyBinding::new("up", PreviousPath, Some("PathChooser")),
            KeyBinding::new("down", NextPath, Some("PathChooser")),
            KeyBinding::new("space", TogglePath, Some("PathChooser")),
            KeyBinding::new("right", ExpandPath, Some("PathChooser")),
            KeyBinding::new("enter", ToggleExpandedPath, Some("PathChooser")),
            KeyBinding::new("left", CollapsePath, Some("PathChooser")),
        ]);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let rows = repos
            .iter()
            .map(|repo| {
                (
                    Node {
                        path: repo.path.to_string_lossy().replace('\\', "/"),
                        directory: true,
                    },
                    0,
                )
            })
            .collect();
        let mut this = Self {
            repos,
            selection: Selection::default(),
            directories: BTreeMap::new(),
            rows,
            active: 0,
            focus,
            scroll: UniformListScrollHandle::new(),
            loading: None,
            error: None,
            initializing: true,
            generation: 0,
            token: Cancel::default(),
            task: Task::ready(()),
        };
        let token = this.token.clone();
        this.task = cx.spawn(async move |this, cx| {
            let selected = cx
                .background_spawn(async move {
                    let mut selection = Selection::default();
                    for path in initial {
                        if token.cancelled() {
                            return Err("Cancelled".to_owned());
                        }
                        if std::fs::symlink_metadata(&path).is_ok() {
                            selection.toggle(&path.to_string_lossy().replace('\\', "/"))?;
                        }
                    }
                    Ok::<_, String>(selection)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.initializing = false;
                match selected {
                    Ok(selection) => s.selection = selection,
                    Err(error) => s.error = Some(error),
                }
                cx.notify();
            });
        });
        this
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }
    pub fn can_confirm(&self) -> bool {
        !self.initializing
            && self.loading.is_none()
            && !self.selection.is_empty()
            && self.error.is_none()
    }
    pub fn selected(&self) -> Option<Vec<std::path::PathBuf>> {
        self.can_confirm().then(|| {
            self.selection
                .paths()
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect()
        })
    }
    fn rebuild(&mut self) {
        fn walk(
            path: &str,
            depth: usize,
            dirs: &BTreeMap<String, Vec<Node>>,
            rows: &mut Vec<(Node, usize)>,
        ) {
            if let Some(children) = dirs.get(path) {
                for node in children {
                    rows.push((node.clone(), depth));
                    if node.directory {
                        walk(&node.path, depth + 1, dirs, rows);
                    }
                }
            }
        }
        let active = self.rows.get(self.active).map(|(n, _)| n.path.clone());
        self.rows.clear();
        for repo in &self.repos {
            let path = repo.path.to_string_lossy().replace('\\', "/");
            self.rows.push((
                Node {
                    path: path.clone(),
                    directory: true,
                },
                0,
            ));
            walk(&path, 1, &self.directories, &mut self.rows);
        }
        self.active = active
            .and_then(|p| self.rows.iter().position(|(n, _)| n.path == p))
            .unwrap_or(0);
    }
    fn expand(&mut self, path: String, cx: &mut Context<Self>) {
        if self.initializing {
            return;
        }
        if self.directories.contains_key(&path) {
            return;
        }
        self.token.cancel();
        self.generation += 1;
        self.token = Cancel::default();
        self.loading = Some(path.clone());
        self.error = None;
        let generation = self.generation;
        let token = self.token.clone();
        let absolute = std::path::PathBuf::from(&path);
        let Some(repo) = self
            .repos
            .iter()
            .filter(|repo| absolute.starts_with(&repo.path))
            .max_by_key(|repo| repo.path.components().count())
            .cloned()
        else {
            self.loading = None;
            self.error = Some("Path does not belong to a project repository".into());
            cx.notify();
            return;
        };
        self.task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let relative = absolute
                        .strip_prefix(&repo.path)
                        .map_err(|e| e.to_string())?;
                    let relative = if relative.as_os_str().is_empty() {
                        ".".into()
                    } else {
                        relative.to_string_lossy().replace('\\', "/")
                    };
                    let nodes = path_tree::children(&repo, &relative, &token)?
                        .into_iter()
                        .map(|node| Node {
                            path: repo
                                .path
                                .join(node.path)
                                .to_string_lossy()
                                .replace('\\', "/"),
                            directory: node.directory,
                        })
                        .collect::<Vec<_>>();
                    Ok::<_, String>((path, nodes))
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.generation != generation {
                    return;
                }
                s.loading = None;
                s.initializing = false;
                match result {
                    Ok((path, nodes)) => {
                        if s.directories.values().map(Vec::len).sum::<usize>() + nodes.len()
                            > 16_384
                        {
                            s.error = Some(
                                "Collapse folders before expanding more than 16,384 nodes".into(),
                            );
                        } else {
                            s.directories.insert(path, nodes);
                            s.rebuild();
                        }
                    }
                    Err(error) => s.error = Some(error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
    fn collapse(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.initializing {
            return;
        }
        if self
            .loading
            .as_deref()
            .is_some_and(|p| p == path || p.starts_with(&format!("{path}/")) || path == ".")
        {
            self.token.cancel();
            self.generation += 1;
            self.task = Task::ready(());
            self.loading = None;
        }
        self.directories
            .retain(|p, _| path != "." && p != path && !p.starts_with(&format!("{path}/")));
        self.error = None;
        self.rebuild();
        cx.notify();
    }
    fn toggle(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.initializing {
            return;
        }
        self.error = self.selection.toggle(path).err();
        cx.notify();
    }
    fn move_row(&mut self, down: bool, cx: &mut Context<Self>) {
        self.active = if down {
            (self.active + 1).min(self.rows.len().saturating_sub(1))
        } else {
            self.active.saturating_sub(1)
        };
        self.scroll.scroll_to_item(self.active, ScrollStrategy::Top);
        cx.notify();
    }
}
impl Render for PathChooser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .h(px(300.))
            .gap_1()
            .key_context("PathChooser")
            .track_focus(&self.focus)
            .on_action(cx.listener(|s, _: &PreviousPath, _, cx| s.move_row(false, cx)))
            .on_action(cx.listener(|s, _: &NextPath, _, cx| s.move_row(true, cx)))
            .on_action(cx.listener(|s, _: &TogglePath, _, cx| {
                if let Some((node, _)) = s.rows.get(s.active).cloned() {
                    s.toggle(&node.path, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ExpandPath, _, cx| {
                if let Some((node, _)) = s.rows.get(s.active).cloned().filter(|(n, _)| n.directory)
                {
                    s.expand(node.path, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &ToggleExpandedPath, _, cx| {
                if let Some((node, _)) = s
                    .rows
                    .get(s.active)
                    .cloned()
                    .filter(|(node, _)| node.directory)
                {
                    if s.directories.contains_key(&node.path) {
                        s.collapse(&node.path, cx);
                    } else {
                        s.expand(node.path, cx);
                    }
                }
            }))
            .on_action(cx.listener(|s, _: &CollapsePath, _, cx| {
                if let Some((node, _)) = s.rows.get(s.active).cloned() {
                    s.collapse(&node.path, cx);
                }
            }))
            .child(
                uniform_list(
                    "path-chooser-tree",
                    self.rows.len(),
                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|i| {
                                let (node, depth) = s.rows[i].clone();
                                let state = s.selection.state(&node.path);
                                let expanded = s.directories.contains_key(&node.path);
                                let checkbox_path = node.path.clone();
                                let expand_path = node.path.clone();
                                let name = std::path::Path::new(&node.path)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| node.path.clone());
                                let root_path = node.path.clone();
                                h_flex()
                                    .id(("path-row", i))
                                    .h(px(26.))
                                    .w_full()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .pl(px(depth as f32 * 16.))
                                    .gap_1()
                                    .when(i == s.active, |row| row.bg(rgb(0x2e4d89)))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |s, _, w, cx| {
                                            s.active = i;
                                            s.focus.focus(w, cx);
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .id(("path-expand", i))
                                            .w(px(16.))
                                            .flex_shrink_0()
                                            .when(node.directory, |d| {
                                                d.cursor_pointer()
                                                    .child(
                                                        Icon::default()
                                                            .path(if expanded {
                                                                "jb/chevronDown_dark.svg"
                                                            } else {
                                                                "jb/chevronRight_dark.svg"
                                                            })
                                                            .size(px(16.)),
                                                    )
                                                    .on_click(cx.listener(move |s, _, _, cx| {
                                                        cx.stop_propagation();
                                                        if expanded {
                                                            s.collapse(&expand_path, cx);
                                                        } else {
                                                            s.expand(expand_path.clone(), cx);
                                                        }
                                                    }))
                                            }),
                                    )
                                    .child(
                                        Checkbox::new(("path-check", i))
                                            .checked(state != State::Clear)
                                            .disabled(matches!(
                                                state,
                                                State::CoveredByAncestor | State::ContainsSelected
                                            ))
                                            .on_click(cx.listener(move |s, _, _, cx| {
                                                s.toggle(&checkbox_path, cx)
                                            })),
                                    )
                                    .child(
                                        img(if node.directory {
                                            "jb/folder_dark_4x.svg"
                                        } else {
                                            crate::ui_style::file_icon(std::path::Path::new(
                                                &node.path,
                                            ))
                                        })
                                        .size(px(16.))
                                        .flex_shrink_0(),
                                    )
                                    .child(div().flex_shrink_0().child(name))
                                    .when(depth == 0, |row| {
                                        row.child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_color(rgb(0x6f737a))
                                                .child(format!("({root_path})")),
                                        )
                                    })
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .min_h_0()
                .track_scroll(&self.scroll),
            )
            .when_some(self.loading.clone(), |view, path| {
                view.child(div().text_sm().child(format!("Loading {path}…")))
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_sm().text_color(rgb(0xf75464)).child(error))
            })
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;
    #[gpui::test]
    fn project_tree_expands_each_root_and_keeps_absolute_selection(cx: &mut gpui::TestAppContext) {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(
            format!("plico-path-tree-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
        ));
        let roots = [fixture.0.join("parent"), fixture.0.join("other")];
        for root in &roots {
            std::fs::create_dir_all(root).unwrap();
            std::fs::write(root.join("same.txt"), "tracked").unwrap();
            for args in [vec!["init", "-q"], vec!["add", "same.txt"]] {
                let output = std::process::Command::new("git")
                    .arg("-C")
                    .arg(root)
                    .args(args)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{output:?}");
            }
        }
        cx.update(gpui::component::init);
        let mut chooser = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| {
                PathChooser::new(
                    roots.iter().map(Repository::new).collect(),
                    vec![],
                    window,
                    cx,
                )
            });
            chooser = Some(view.clone());
            Root::new(view, window, cx)
        });
        let chooser = chooser.unwrap();
        cx.run_until_parked();
        chooser.read_with(cx, |s, _| {
            assert_eq!(s.rows.len(), 2);
            assert!(!s.can_confirm());
        });
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        chooser.read_with(cx, |s, _| {
            assert_eq!(s.rows.len(), 3, "Enter expands the focused root");
            assert!(!s.can_confirm(), "expanding must not select or confirm");
        });
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        chooser.read_with(cx, |s, _| assert_eq!(s.rows.len(), 2));
        for root in &roots {
            chooser.update(cx, |s, cx| {
                s.expand(root.to_string_lossy().replace('\\', "/"), cx)
            });
            cx.run_until_parked();
        }
        chooser.update(cx, |s, cx| {
            assert!(s.error.is_none(), "{:?}", s.error);
            assert_eq!(s.rows.len(), 4);
            s.toggle(
                &roots[1]
                    .join("same.txt")
                    .to_string_lossy()
                    .replace('\\', "/"),
                cx,
            );
            assert_eq!(s.selected().unwrap(), [roots[1].join("same.txt")]);
            s.collapse(&roots[0].to_string_lossy().replace('\\', "/"), cx);
            assert_eq!(s.rows.len(), 3);
            assert_eq!(s.selected().unwrap(), [roots[1].join("same.txt")]);
        });
    }
}
