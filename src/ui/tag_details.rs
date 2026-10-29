//! Rebased GitReferenceValidator → ChangeListViewerDialog: files above message.
use super::*;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use std::collections::HashSet;
use std::path::PathBuf;
struct Files {
    repo: Repository,
    preview: plico::tag_create::Preview,
    collapsed: HashSet<PathBuf>,
    selected: Option<usize>,
    focus: FocusHandle,
}
impl Files {
    fn diff(&self, w: &mut Window, cx: &mut Context<Self>) {
        let Ok((target, chain)) = plico::diff_target::Chain::combined(
            self.repo.clone(),
            self.preview.files.clone(),
            self.selected.unwrap_or(0),
        ) else {
            return;
        };
        let viewer = cx.new(|_| crate::diff_tab::DiffTab::new(target, chain));
        let activate = viewer.clone();
        w.open_dialog(cx, move |d, w, _| {
            let next = viewer.clone();
            let previous = viewer.clone();
            d.title(t("Diff"))
                .w(px(1200.))
                .overlay_closable(false)
                .child(
                    div()
                        .key_context("PushDiff")
                        .h((w.viewport_size().height - px(170.))
                            .max(px(240.))
                            .min(px(700.)))
                        .child(viewer.clone())
                        .on_action(move |_: &crate::push_dialog::NextDifference, w, cx| {
                            next.update(cx, |s, cx| s.navigate(true, w, cx))
                        })
                        .on_action(move |_: &crate::push_dialog::PreviousDifference, w, cx| {
                            previous.update(cx, |s, cx| s.navigate(false, w, cx))
                        }),
                )
        });
        activate.update(cx, |s, cx| s.activate(w, cx));
    }
}
impl Render for Files {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = plico::file_tree::compact_rows(
            self.preview.files.iter().enumerate().filter_map(|(i, f)| {
                f.after
                    .as_ref()
                    .or(f.before.as_ref())
                    .map(|r| (i, r.path.as_path()))
            }),
            |p| self.collapsed.contains(p),
        );
        v_flex()
            .h(px(460.))
            .gap_2()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("tag-details-diff")
                            .ghost()
                            .icon(jb_icon("diff_dark.svg"))
                            .tooltip(t("Show Diff"))
                            .disabled(self.preview.files.is_empty())
                            .on_click(cx.listener(|s, _, w, cx| s.diff(w, cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("tag-details-expand")
                            .ghost()
                            .label(t("Expand All"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.collapsed.clear();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("tag-details-collapse")
                            .ghost()
                            .label(t("Collapse All"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.collapsed = s
                                    .preview
                                    .files
                                    .iter()
                                    .filter_map(|f| f.after.as_ref().or(f.before.as_ref()))
                                    .flat_map(|r| r.path.ancestors().skip(1))
                                    .filter(|p| !p.as_os_str().is_empty())
                                    .map(|p| p.to_owned())
                                    .collect();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("tag-details-files")
                    .key_context("PushFiles")
                    .track_focus(&self.focus)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .on_action(
                        cx.listener(|s, _: &crate::push_dialog::ShowFileDiff, w, cx| s.diff(w, cx)),
                    )
                    .when(rows.is_empty(), |v| v.child(t("No changes")))
                    .children(rows.into_iter().map(|row| {
                        match row {
                            plico::file_tree::Row::Folder {
                                path,
                                label,
                                depth,
                                count,
                                expanded,
                            } => h_flex()
                                .id(SharedString::from(format!(
                                    "tag-directory:{}",
                                    path.display()
                                )))
                                .h(px(28.))
                                .gap_1()
                                .pl(px(depth as f32 * 16.))
                                .child(jb_icon(if expanded {
                                    "chevronDown_dark.svg"
                                } else {
                                    "chevronRight_dark.svg"
                                }))
                                .child(jb_icon("folder_dark_4x.svg"))
                                .child(label.display().to_string())
                                .child(
                                    div()
                                        .text_color(rgb(crate::ui_theme::palette(cx).muted))
                                        .child(crate::ui_style::file_count(count)),
                                )
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    if !s.collapsed.remove(&path) {
                                        s.collapsed.insert(path.clone());
                                    }
                                    cx.notify();
                                }))
                                .into_any_element(),
                            plico::file_tree::Row::File { index, depth } => {
                                let f = &self.preview.files[index];
                                let path = &f.after.as_ref().or(f.before.as_ref()).unwrap().path;
                                h_flex()
                                    .id(("tag-details-file", index))
                                    .h(px(28.))
                                    .gap_1()
                                    .pl(px(16. + depth as f32 * 16.))
                                    .when(self.selected == Some(index), |v| {
                                        v.bg(rgb(crate::ui_theme::color(cx, 0x33353b, 0xe9eaee)))
                                    })
                                    .child(
                                        gpui_kit::component::Icon::default()
                                            .path(crate::ui_style::file_icon(path, cx))
                                            .size(px(16.)),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_color(rgb(if f.before.is_none() {
                                                0x6aab73
                                            } else if f.after.is_none() {
                                                0xcf8e6d
                                            } else {
                                                0x6897bb
                                            }))
                                            .child(
                                                path.file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy()
                                                    .to_string(),
                                            ),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |s, e: &MouseDownEvent, w, cx| {
                                            s.selected = Some(index);
                                            s.focus.focus(w, cx);
                                            if e.click_count == 2 {
                                                s.diff(w, cx);
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .into_any_element()
                            }
                        }
                    })),
            )
            .child(
                div()
                    .id("tag-details-message")
                    .h(px(100.))
                    .border_t_1()
                    .border_color(rgb(crate::ui_theme::color(cx, 0x393b40, 0xdddfe4)))
                    .overflow_y_scroll()
                    .whitespace_normal()
                    .child(self.preview.message.clone()),
            )
    }
}
pub(super) fn show(
    repo: Repository,
    preview: plico::tag_create::Preview,
    w: &mut Window,
    cx: &mut App,
) {
    let title = format!("{} {}", t("Paths affected in commit"), &preview.id[..8]);
    let view = cx.new(|cx| Files {
        repo,
        preview,
        collapsed: HashSet::new(),
        selected: None,
        focus: cx.focus_handle(),
    });
    let focus = view.read(cx).focus.clone();
    w.open_dialog(cx, move |d, _, _| {
        d.title(title.clone())
            .w(px(600.))
            .centered(true)
            .overlay_closable(false)
            .child(view.clone())
            .footer(
                DialogFooter::new()
                    .child(DialogClose::new().trigger(|b| b.primary().label(t("Close")))),
            )
    });
    focus.focus(w, cx);
}
