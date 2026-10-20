//! One repository-scoped event source; shared documents own their reload safety.
use super::*;
use plico::worktree_events::{Batch, Events};
use std::time::{Duration, Instant};

impl Review {
    pub(super) fn start_worktree_observation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let root = self.repo.path.clone();
        let token = self.updates.token.clone();
        self.updates.worktree = cx.spawn_in(window, async move |this, wx| {
            let watch_root = root.clone();
            let (events, fallback) = wx.background_spawn(async move {
                let events = Events::new(watch_root.clone()).ok();
                let fallback = events.is_none() || watch_root.to_string_lossy().starts_with("\\\\") || watch_root.join(".git").is_file();
                (events, fallback)
            }).await;
            // UNC notifications may be absent even when watch registration succeeds.
            // Deliver debounced filesystem events promptly. The two-second
            // timer is only a stamp check for open documents, not a Git refresh.
            let mut schedule = plico::worktree_events::Schedule::new(Instant::now(), fallback);
            loop {
                if token.cancelled() { break; }
                let ready = this.read_with(wx, |s, _| !s.welcome && !s.operation.busy() && !s.documents.locked()
                    && s.repo.path == root).unwrap_or(false);
                if ready {
                    let mut batch = events.as_ref().map(Events::take).unwrap_or_default();
                    let poll_open_documents = schedule.advance(&mut batch, Instant::now());
                    if batch.changed() || poll_open_documents {
                        if this.update_in(wx, |s, window, cx| {
                            if token.cancelled() || s.documents.locked() || s.operation.busy() { return; }
                            s.sync_worktree(batch, poll_open_documents, window, cx);
                        }).is_err() { break; }
                    }
                }
                wx.background_executor().timer(Duration::from_millis(100)).await;
            }
        });
    }
    fn sync_worktree(&mut self, batch: Batch, poll_open_documents: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.documents.observe_disk(&batch, poll_open_documents, window, cx);
        if !batch.changed() { return; }
        if let Some(view) = &self.local_view { view.update(cx, |v, cx| v.refresh(cx)); }
        if let Some(view) = &self.project_view { view.update(cx, |v, cx| v.refresh_external(cx)); }
        for tab in self.tabs.iter().chain(self.local_preview.iter()) {
            tab.update(cx, |tab, cx| tab.disk_changed(&batch, window, cx));
        }
        cx.notify();
    }
}
