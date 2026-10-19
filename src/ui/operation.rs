//! Window adapter for operation ownership and document write exclusion.
use super::*;
use plico::active_operation::{Id, Kind, Phase};
impl Review {
    pub(super) fn begin_operation(
        &mut self,
        kind: Kind,
        phase: Phase,
        cx: &mut Context<Self>,
    ) -> Option<(Id, Cancel)> {
        if self
            .local_view
            .as_ref()
            .is_some_and(|v| v.read(cx).writing())
        {
            return None;
        }
        let operation = self.operation.begin(kind, phase)?;
        self.sync_document_lock(cx);
        cx.notify();
        Some(operation)
    }
    pub(super) fn finish_operation(&mut self, id: Id, cx: &mut Context<Self>) -> Option<bool> {
        let cancelled = self.operation.finish(id)?;
        self.sync_document_lock(cx);
        cx.notify();
        Some(cancelled)
    }
    pub(super) fn sync_document_lock(&self, cx: &mut Context<Self>) {
        let local_write = self
            .local_view
            .as_ref()
            .is_some_and(|v| v.read(cx).writing());
        self.documents
            .set_locked(self.operation.documents_locked(local_write), cx);
    }
    pub(super) fn with_saved_documents(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        action: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if self
            .local_view
            .as_ref()
            .is_some_and(|v| v.read(cx).writing())
            || !self.operation.begin_save()
        {
            return;
        }
        self.sync_document_lock(cx);
        self.documents.save_all(cx);
        let documents = self.documents.all(); // retain drafts until writes finish
        cx.spawn_in(window, async move |this, wx| {
            let mut action = Some(action);
            loop {
                let done = this
                    .update_in(wx, |s, window, cx| {
                        if documents.iter().any(|d| d.read(cx).saving()) {
                            return false;
                        }
                        s.operation.finish_save();
                        if let Some(error) = documents.iter().find_map(|d| d.read(cx).error.clone())
                        {
                            s.error = Some(error);
                            s.status_message = Some(t("Could not save file").into());
                        } else if !documents.iter().any(|d| d.read(cx).pending(cx)) {
                            if let Some(action) = action.take() {
                                action(s, window, cx);
                            }
                        }
                        s.sync_document_lock(cx);
                        cx.notify();
                        true
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
                wx.background_executor()
                    .timer(std::time::Duration::from_millis(25))
                    .await;
            }
        })
        .detach();
        cx.notify();
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::{Root, input::EditorState};
    use gpui::{AppContext as _, TestAppContext};

    #[gpui::test]
    fn operation_locks_existing_and_new_documents_without_render(cx: &mut TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = review.unwrap();
        cx.update(|window, cx| {
            view.update(cx, |s, cx| {
                let doc = s.documents.get_or_create(
                    "fixture".into(),
                    "a.txt".into(),
                    plico::file_document::Document::new(b"old".to_vec()).unwrap(),
                    |text, cx| {
                        cx.new(|cx| EditorState::new(window, cx).default_value(text.to_owned()))
                    },
                    cx,
                );
                let (operation, token) = s
                    .begin_operation(Kind::Checkout, Phase::Executing, cx)
                    .unwrap();
                assert!(!doc.read(cx).editor.read(cx).is_editable());
                let other = s.documents.get_or_create(
                    "fixture".into(),
                    "b.txt".into(),
                    plico::file_document::Document::new(b"new".to_vec()).unwrap(),
                    |text, cx| {
                        cx.new(|cx| EditorState::new(window, cx).default_value(text.to_owned()))
                    },
                    cx,
                );
                assert!(!other.read(cx).editor.read(cx).is_editable());
                s.operation.request_cancel();
                assert!(token.cancelled());
                assert!(!doc.read(cx).editor.read(cx).is_editable());
                assert_eq!(s.finish_operation(operation, cx), Some(true));
                assert!(doc.read(cx).editor.read(cx).is_editable());
                assert!(other.read(cx).editor.read(cx).is_editable());
            })
        });
    }

    #[gpui::test]
    fn failed_save_preserves_draft_and_does_not_run_deferred_action(cx: &mut TestAppContext) {
        use std::{cell::Cell, rc::Rc};
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, window, cx));
            review = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = review.unwrap();
        let called = Rc::new(Cell::new(false));
        let result = called.clone();
        cx.update(|window, cx| {
            view.update(cx, |s, cx| {
                let doc = s.documents.get_or_create(
                    "fixture".into(),
                    "bad.txt".into(),
                    plico::file_document::Document::new(b"old".to_vec()).unwrap(),
                    |_, cx| {
                        cx.new(|cx| EditorState::new(window, cx).default_value("invalid\0draft"))
                    },
                    cx,
                );
                // Keep the failed draft retained even though there is no open source tab.
                s.documents.retain_pending("bad.txt".into(), &doc, cx);
                s.with_saved_documents(window, cx, move |_, _, _| result.set(true));
                assert!(s.documents.locked());
            })
        });
        cx.run_until_parked();
        assert!(!called.get());
        view.read_with(cx, |s, cx| {
            assert!(!s.operation.saving_documents());
            assert!(!s.documents.locked());
            assert!(s.error.is_some());
            let doc = s.documents.get(&PathBuf::from("bad.txt")).unwrap();
            assert_eq!(
                doc.read(cx).editor.read(cx).text().to_string(),
                "invalid\0draft"
            );
        });
    }
}
