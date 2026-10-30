//! Repository-scoped observation. Slow remote checks cannot block local refs.
use super::*;
use plico::branch_updates::{Counts, References, RemoteState};
use std::time::{Duration, Instant};

pub(super) struct AutoRefresh {
    local: Task<()>,
    pub(super) worktree: Task<()>,
    remote_task: Task<()>,
    pub(super) token: Cancel,
    remote_token: Cancel,
    pub references: Option<References>,
    pub history_observation: Option<plico::branch_updates::HistoryObservation>,
    pub counts: Counts,
    pub remote: RemoteState,
    remote_busy: bool,
    next_remote: Instant,
}
impl Default for AutoRefresh {
    fn default() -> Self {
        Self {
            local: Task::ready(()),
            worktree: Task::ready(()),
            remote_task: Task::ready(()),
            token: Cancel::default(),
            remote_token: Cancel::default(),
            references: None,
            history_observation: None,
            counts: Counts::default(),
            remote: RemoteState::Unknown,
            remote_busy: false,
            next_remote: Instant::now(),
        }
    }
}
impl Drop for AutoRefresh {
    fn drop(&mut self) {
        self.token.cancel();
        self.remote_token.cancel();
    }
}
impl Review {
    pub(super) fn start_auto_refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.watch_shallow_startup(window, cx);
        self.updates = AutoRefresh::default();
        if self.welcome {
            return;
        }
        self.start_worktree_observation(window, cx);
        let repo = self.repo.clone();
        let token = self.updates.token.clone();
        self.updates.local = cx.spawn(async move |this, cx| {
            loop {
                if token.cancelled() {
                    break;
                }
                let ready = this
                    .read_with(cx, |s, _| {
                        !s.welcome
                            && !s.operation.busy()
                            && !s.documents.locked()
                            && !s.log_query.busy()
                    })
                    .unwrap_or(false);
                if ready {
                    let repo = repo.clone();
                    let query_token = token.clone();
                    let previous = this
                        .read_with(cx, |s, _| {
                            (s.updates.references.clone(), s.log_query.stamp())
                        })
                        .ok();
                    let Some((previous, stamp)) = previous else {
                        break;
                    };
                    let result = cx
                        .background_spawn(async move {
                            let refs = References::read(&repo, &query_token)?;
                            let counts = if previous.as_ref() != Some(&refs) {
                                Some(refs.counts(&repo, &query_token)?)
                            } else {
                                None
                            };
                            Ok::<_, String>((refs, counts))
                        })
                        .await;
                    if this
                        .update(cx, |s, cx| {
                            if token.cancelled()
                                || s.operation.busy()
                                || s.documents.locked()
                                || s.log_query.busy()
                                || !s.log_query.is_current(stamp)
                            {
                                return;
                            }
                            let Ok((refs, counts)) = result else {
                                return;
                            };
                            let notify = counts.is_some();
                            s.apply_observed_references(refs, counts, cx);
                            s.check_remote_updates(cx);
                            if notify {
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                cx.background_executor().timer(Duration::from_secs(5)).await;
            }
        });
    }
    /// Called only after the polling request's cancellation/query guards pass.
    pub(super) fn apply_observed_references(
        &mut self,
        refs: References,
        counts: Option<Counts>,
        cx: &mut Context<Self>,
    ) {
        let changed = self.updates.references.as_ref().map_or_else(
            || {
                self.snapshot
                    .as_ref()
                    .is_some_and(|s| s.head_id != refs.head)
            },
            |old| old != &refs,
        );
        let references_only = !self.local
            && self.updates.references.as_ref().is_some_and(|old| {
                old.head == refs.head && old.branch == refs.branch && old.same_replacements(&refs)
            });
        if self
            .updates
            .references
            .as_ref()
            .is_some_and(|old| !old.same_replacements(&refs))
        {
            self.invalidate_loaded_details(&self.repo.path.clone());
        }
        self.accept_reference_observation(refs, counts);
        if changed {
            if references_only {
                self.refresh_reference_changes(cx);
            } else {
                self.refresh_impl(false, true, None, cx);
            }
        }
    }
    /// A successful history refresh publishes the observation made BEFORE its
    /// history read. Later external changes remain detectable by the next poll.
    pub(super) fn accept_reference_observation(
        &mut self,
        refs: References,
        counts: Option<Counts>,
    ) {
        let remote_changed =
            self.updates.references.as_ref().map(|r| &r.upstream) != Some(&refs.upstream);
        if let Some(counts) = counts {
            self.updates.counts = counts;
        }
        if remote_changed {
            self.updates.remote_token.cancel();
            self.updates.remote_task = Task::ready(());
            self.updates.remote_busy = false;
            self.updates.remote = RemoteState::Unknown;
            self.updates.next_remote = Instant::now();
        }
        self.updates.references = Some(refs);
    }
    fn check_remote_updates(&mut self, cx: &mut Context<Self>) {
        if self.updates.remote_busy || Instant::now() < self.updates.next_remote {
            return;
        }
        let Some(refs) = self
            .updates
            .references
            .clone()
            .filter(|r| r.upstream.is_some())
        else {
            return;
        };
        self.updates.remote_busy = true;
        self.updates.next_remote = Instant::now() + Duration::from_secs(300);
        self.updates.remote_token = Cancel::default();
        let token = self.updates.remote_token.clone();
        let repo = self.repo.clone();
        self.updates.remote_task = cx.spawn(async move |this, cx| {
            let expected = refs.upstream.clone();
            let path = repo.path.clone();
            let request_token = token.clone();
            let result = cx
                .background_spawn(async move { refs.check_remote(&repo, &token) })
                .await;
            let _ = this.update(cx, |s, cx| {
                if request_token.cancelled() || s.repo.path != path {
                    return;
                }
                if s.updates
                    .references
                    .as_ref()
                    .and_then(|r| r.upstream.as_ref())
                    != expected.as_ref()
                {
                    return;
                }
                s.updates.remote_busy = false;
                s.updates.remote = result.unwrap_or(RemoteState::Unknown);
                cx.notify();
            });
        });
    }
    pub(super) fn branch_update_counts(&self) -> (usize, usize, bool) {
        if self
            .updates
            .references
            .as_ref()
            .and_then(|r| r.branch.as_ref())
            != self
                .snapshot
                .as_ref()
                .and_then(|s| s.current_branch.as_ref())
        {
            return (0, 0, false);
        }
        (
            self.updates.counts.incoming,
            self.updates.counts.outgoing,
            self.updates.remote == RemoteState::Unfetched,
        )
    }
    pub(super) fn branch_update_tooltip(&self) -> String {
        let (incoming, outgoing, unfetched) = self.branch_update_counts();
        if i18n::is_chinese() {
            format!(
                "分支\n待拉取 {incoming} 个提交 · 待推送 {outgoing} 个提交{}",
                if unfetched {
                    "\n远端已有更新，获取后可查看提交详情"
                } else if self.updates.remote == RemoteState::Unknown {
                    "\n远端状态尚未确认；数量基于本地引用"
                } else {
                    ""
                }
            )
        } else {
            format!(
                "Branch\n{incoming} incoming · {outgoing} outgoing{}",
                if unfetched {
                    "\nRemote updates available; fetch to view commits"
                } else if self.updates.remote == RemoteState::Unknown {
                    "\nRemote state unknown; counts use local references"
                } else {
                    ""
                }
            )
        }
    }
}
