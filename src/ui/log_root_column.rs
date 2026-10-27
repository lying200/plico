//! Repository identity column. No queries: rows use the accepted Log identities.
// Interaction and segment rules adapted from Rebased 4aec352 RootCellRenderer,
// NewUiRootCellRenderer and RepositoryColorStripe (Apache-2.0).
// Copyright 2000-2026 JetBrains s.r.o. and contributors.
use super::*;
use std::collections::HashMap;

struct RootStyle {
    name: String,
    color: u32,
}
pub(super) struct Column {
    roots: HashMap<String, RootStyle>,
    width: Pixels,
    expanded: bool,
}
impl Column {
    pub fn width(&self) -> Pixels {
        self.width
    }
    pub fn new(review: &Review, window: &Window, cx: &App) -> Option<Self> {
        let project = review.project_roots.read(cx);
        if !project.has_multiple_roots() {
            return None;
        }
        let expanded = review.show_log_root_names;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let mut width = px(10.);
        let roots = project
            .observation_roots()
            .iter()
            .map(|repo| {
                let name = repo
                    .path
                    .file_name()
                    .unwrap_or(repo.path.as_os_str())
                    .to_string_lossy()
                    .into_owned();
                if expanded {
                    let text: SharedString = format!("{name}  ").into();
                    let run = TextRun {
                        len: text.len(),
                        font: style.font(),
                        color: style.color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    width = width.max(
                        window
                            .text_system()
                            .shape_line(text, font_size, &[run], None)
                            .width
                            + px(8.),
                    );
                }
                let color = if expanded {
                    project.opened_color(&repo.path)
                } else {
                    project.color(&repo.path)
                }
                .unwrap_or(BG);
                (
                    plico::recent_projects::identity(&repo.path),
                    RootStyle { name, color },
                )
            })
            .collect();
        Some(Self {
            roots,
            width: width.min(px(300.)),
            expanded,
        })
    }
    pub fn cell(
        &self,
        index: usize,
        ids: &[plico::project_log::CommitId],
        first_readable: usize,
        owner: WeakEntity<Review>,
        row_background: u32,
    ) -> impl IntoElement {
        let root = &ids[index].root;
        let same_prev = index.checked_sub(1).is_some_and(|i| ids[i].root == *root);
        let same_next = ids.get(index + 1).is_some_and(|id| id.root == *root);
        let style = self.roots.get(&plico::recent_projects::identity(root));
        let name = style.map(|s| s.name.clone()).unwrap_or_default();
        let color = style.map(|s| s.color).unwrap_or(BG);
        let expanded = self.expanded;
        let tooltip = format!(
            "{}\n{}",
            root.display(),
            t(if expanded {
                "Click to collapse repository names"
            } else {
                "Click to expand repository names"
            })
        );
        div()
            .id((
                if expanded {
                    "log-root-expanded"
                } else {
                    "log-root"
                },
                index,
            ))
            .relative()
            .w(self.width)
            .h_full()
            .flex_shrink_0()
            .cursor_pointer()
            .overflow_hidden()
            .bg(rgb(if expanded { BG } else { row_background }))
            .tooltip(move |w, cx| Tooltip::new(tooltip.clone()).build(w, cx))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                // The enclosing row must not select/focus a commit for this click.
                cx.stop_propagation();
                let _ = owner.update(cx, |s, cx| {
                    let enabled = !s.show_log_root_names;
                    if let Err(error) =
                        layout_preferences::save_log_root_names(&s.repo.path, enabled)
                    {
                        s.error = Some(error.to_string());
                        cx.notify();
                        return;
                    }
                    s.show_log_root_names = enabled;
                    cx.notify();
                });
            })
            .child(
                div()
                    .absolute()
                    .left(px(2.))
                    .top_0()
                    .w(if expanded {
                        self.width - px(4.)
                    } else {
                        px(4.)
                    })
                    .h_full()
                    .when(!same_next, |d| d.bottom(px(2.)).h_auto())
                    .bg(rgb(color))
                    .when(!same_prev, |d| d.rounded_t(px(2.)))
                    .when(!same_next, |d| d.rounded_b(px(2.))),
            )
            .when(expanded && (!same_prev || index == first_readable), |d| {
                d.child(
                    h_flex()
                        .relative()
                        .size_full()
                        .px(px(4.))
                        .overflow_hidden()
                        .child(div().truncate().child(name)),
                )
            })
    }
}
