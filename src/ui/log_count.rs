//! Independent count task: selecting details must never cancel it or delay first paint.
use super::*;

#[derive(Clone, PartialEq, Eq)]
struct Key {
    path: std::path::PathBuf,
    roots: Vec<String>,
    filter: Option<git::LogFilter>,
}
#[derive(Default)]
pub(super) struct Count {
    key: Option<Key>,
    pub total: Option<usize>,
    pub error: Option<String>,
    job: super::log_query::Query,
}
impl Count {
    pub fn loading(&self) -> bool {
        self.job.history_loading()
    }
    pub fn invalidate(&mut self) {
        self.job.cancel();
        self.key = None;
        self.total = None;
        self.error = None;
    }
}
impl Review {
    pub(super) fn update_history_count(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = &self.snapshot else {
            self.history_count.invalidate();
            return;
        };
        let mut roots = snapshot.tips.clone();
        roots.sort();
        roots.dedup();
        let key = Key {
            path: self.repo.path.clone(),
            roots,
            filter: self.text_filter.clone(),
        };
        if !snapshot.more && (self.history_offset == 0 || !self.commits.is_empty()) {
            self.history_count.invalidate();
            self.history_count.key = Some(key);
            self.history_count.total = Some(self.history_offset + self.commits.len());
            return;
        }
        if self.history_count.key.as_ref() == Some(&key) {
            return;
        }
        self.history_count.invalidate();
        self.history_count.key = Some(key.clone());
        let repo = self.repo.clone();
        let (stamp, token) = self
            .history_count
            .job
            .begin(super::log_query::Kind::History);
        let task = cx.spawn(async move |this, cx| {
            // Allow the accepted first window to paint before starting the count.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let result = cx
                .background_spawn(async move {
                    history::count(&repo, &key.roots, key.filter.as_ref(), &token)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if !s.history_count.job.finish(stamp) {
                    return;
                }
                match result {
                    Ok(total) => s.history_count.total = Some(total),
                    Err(error) => s.history_count.error = Some(error),
                }
                cx.notify();
            });
        });
        self.history_count.job.track(stamp, task);
    }

    pub(super) fn history_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.log_query.history_loading();
        let counting = self.history_count.loading();
        let loaded = self.commits.len();
        let label =
            plico::i18n::history_count(loaded, self.history_offset, self.history_count.total);
        h_flex()
            .h(px(32.))
            .flex_shrink_0()
            .min_w_0()
            .px_3()
            .gap_2()
            .border_t_1()
            .border_color(rgb(BORDER))
            .text_size(px(12.))
            .text_color(rgb(MUTED))
            .child(div().min_w_0().truncate().child(label))
            .when(counting, |v| {
                v.child(gpui_kit::component::spinner::Spinner::new().small())
                    .child(div().min_w_0().truncate().child(t("Counting commits…")))
            })
            .when(self.history_count.error.is_some(), |v| {
                v.child(
                    Button::new("retry-history-count")
                        .link()
                        .text_color(cx.theme().link)
                        .small()
                        .label(t("Retry count"))
                        .tooltip(self.history_count.error.clone().unwrap_or_default())
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.history_count.invalidate();
                            s.update_history_count(cx);
                        })),
                )
            })
            .child(div().flex_1())
            .when(self.history_offset > 0, |v| {
                v.child(
                    Button::new("previous-window")
                        .link()
                        .text_color(cx.theme().link)
                        .small()
                        .label(t("Newer window"))
                        .disabled(busy)
                        .on_click(cx.listener(|s, _, _, cx| s.previous_window(cx))),
                )
                .child(
                    Button::new("latest")
                        .link()
                        .text_color(cx.theme().link)
                        .small()
                        .label(t("Back to latest"))
                        .disabled(busy)
                        .on_click(cx.listener(|s, _, _, cx| s.refresh(false, cx))),
                )
            })
            .when(self.snapshot.as_ref().is_some_and(|s| s.more), |v| {
                v.child(
                    Button::new("more")
                        .link()
                        .text_color(cx.theme().link)
                        .small()
                        .loading(busy)
                        .disabled(busy)
                        .label(t(if self.limit >= git::HISTORY_MAX {
                            "Older window"
                        } else {
                            "Load more"
                        }))
                        .on_click(cx.listener(|s, _, _, cx| {
                            if !s.log_query.history_loading() {
                                s.refresh(true, cx);
                            }
                        })),
                )
            })
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use core::prelude::v1::test;
    extern crate gpui_kit as gpui;
    use super::*;
    use gpui::component::Root;
    use std::{
        path::PathBuf,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    #[gpui::test]
    fn first_page_precedes_count_and_filter_replacement_rejects_old_total(
        cx: &mut gpui::TestAppContext,
    ) {
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
                "plico-count-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir(&fixture.0).unwrap();
        let git = |args: &[&str]| {
            let result = std::process::Command::new("git")
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        };
        git(&["init", "-q"]);
        for message in ["one", "two", "three"] {
            git(&[
                "-c",
                "user.name=Count",
                "-c",
                "user.email=count@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-qm",
                message,
            ]);
        }
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let entity = cx.new(|cx| Review::new(fixture.0.clone(), 1, None, window, cx));
            review = Some(entity.clone());
            Root::new(entity, window, cx)
        });
        let review = review.unwrap();
        review.update(cx, |s, cx| s.refresh(false, cx));
        cx.run_until_parked();
        review.update(cx, |s, cx| {
            assert_eq!(s.commits.len(), 1);
            assert!(s.snapshot.as_ref().unwrap().more);
            assert!(
                s.history_count.loading(),
                "first page must not wait for total"
            );
            assert_eq!(s.history_count.total, None);
            s.select_commit(0, cx);
        });
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        review.update(cx, |s, cx| {
            assert_eq!(
                s.history_count.total,
                Some(3),
                "detail selection must not cancel count"
            );
            s.history_count.invalidate();
            s.update_history_count(cx);
            assert!(s.history_count.loading());
            s.text_filter = Some(git::LogFilter {
                text: "missing".into(),
                ..Default::default()
            });
            s.refresh(false, cx);
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        review.read_with(cx, |s, _| {
            assert!(s.commits.is_empty());
            assert_eq!(s.history_count.total, Some(0));
            assert!(!s.history_count.loading());
        });
    }
}
