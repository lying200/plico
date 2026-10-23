//! Plico recovery browser; delegates writes to the shared Rebased-aligned branch dialog.
use super::*;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
struct Browser {
    repo: Repository,
    owner: WeakEntity<Review>,
    entries: Vec<plico::reflog::Entry>,
    selected: Option<usize>,
    limit: usize,
    more: bool,
    loading: bool,
    error: Option<String>,
    token: Cancel,
    task: Task<()>,
    scroll: UniformListScrollHandle,
}
impl Drop for Browser {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
impl Browser {
    fn reload(&mut self, cx: &mut Context<Self>) {
        self.token.cancel();
        self.token = Cancel::default();
        let token = self.token.clone();
        let repo = self.repo.clone();
        let limit = self.limit;
        self.loading = true;
        self.error = None;
        self.task = cx.spawn(async move |this, cx| {
            let check = token.clone();
            let result = cx
                .background_spawn(async move { plico::reflog::read(&repo, limit, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if check.cancelled() {
                    return;
                }
                s.loading = false;
                match result {
                    Ok(page) => {
                        let selected = s.selected.and_then(|i| s.entries.get(i)).cloned();
                        s.entries = page.entries;
                        s.selected = selected.and_then(|e| s.entries.iter().position(|r| r == &e));
                        s.more = page.has_more;
                    }
                    Err(error) => s.error = Some(error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
}
impl Render for Browser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .w_full()
            .child(div().text_color(rgb(MUTED)).child(t(
                "Select a reflog entry to recover its commit as a branch.",
            )))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("refresh-reflog")
                            .label(t("Refresh"))
                            .disabled(self.loading)
                            .on_click(cx.listener(|s, _, _, cx| s.reload(cx))),
                    )
                    .child(
                        Button::new("recover-reflog")
                            .label(t("New Branch…"))
                            .disabled(self.loading || self.selected.is_none())
                            .on_click(cx.listener(|s, _, w, cx| {
                                let Some(entry) = s.selected.and_then(|i| s.entries.get(i)) else {
                                    return;
                                };
                                let id = entry.commit.clone();
                                let path = s.repo.path.clone();
                                s.token.cancel();
                                w.close_dialog(cx);
                                let _ = s.owner.update(cx, |owner, cx| {
                                    if owner.repo.path == path {
                                        owner.new_branch_dialog(id, w, cx);
                                    }
                                });
                            })),
                    ),
            )
            .child(
                uniform_list(
                    "reflog-entries",
                    self.entries.len(),
                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|i| {
                                let entry = &s.entries[i];
                                h_flex()
                                    .id(i)
                                    .h(px(30.))
                                    .gap_2()
                                    .px_2()
                                    .w_full()
                                    .overflow_hidden()
                                    .when(s.selected == Some(i), |row| row.bg(rgb(0x2e4773)))
                                    .child(
                                        div().flex_shrink_0().child(entry.commit[..8].to_owned()),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_color(rgb(MUTED))
                                            .child(entry.reference.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(entry.action.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_color(rgb(MUTED))
                                            .child(plico::commit_date::timestamp(entry.timestamp)),
                                    )
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        s.selected = Some(i);
                                        cx.notify();
                                    }))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .h(px(360.))
                .track_scroll(&self.scroll),
            )
            .when(self.loading, |v| v.child(t("Loading…")))
            .when(
                !self.loading && self.entries.is_empty() && self.error.is_none(),
                |v| v.child(t("No reflog entries")),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    div()
                        .whitespace_normal()
                        .text_color(rgb(0xf2a6a6))
                        .child(error),
                )
            })
            .child(
                h_flex()
                    .gap_2()
                    .child(format!("{} {}", self.entries.len(), t("entries loaded")))
                    .when(self.more, |v| {
                        v.child(
                            Button::new("more-reflog")
                                .label(t("Load More"))
                                .disabled(self.loading || self.limit >= plico::reflog::MAX_ENTRIES)
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.limit = (s.limit + plico::reflog::PAGE_SIZE)
                                        .min(plico::reflog::MAX_ENTRIES);
                                    s.reload(cx);
                                })),
                        )
                    }),
            )
    }
}
impl Review {
    pub(super) fn open_reflog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.welcome || self.operation.busy() {
            return;
        }
        let owner = cx.entity().downgrade();
        let repo = self.repo.clone();
        let browser = cx.new(|cx| {
            let mut view = Browser {
                repo,
                owner,
                entries: vec![],
                selected: None,
                limit: plico::reflog::PAGE_SIZE,
                more: false,
                loading: false,
                error: None,
                token: Cancel::default(),
                task: Task::ready(()),
                scroll: UniformListScrollHandle::new(),
            };
            view.reload(cx);
            view
        });
        w.open_dialog(cx, move |dialog, _, _| {
            let close = browser.clone();
            dialog
                .title(t("Reflog"))
                .w(px(800.))
                .centered(true)
                .child(browser.clone())
                .footer(
                    DialogFooter::new().child(DialogClose::new().trigger(|b| b.label(t("Close")))),
                )
                .on_close(move |_, _, cx| {
                    close.update(cx, |s, _| s.token.cancel());
                })
        });
    }
}
