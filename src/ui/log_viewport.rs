//! Scroll restoration when replacing the loaded Log window.
use gpui_kit::{ScrollStrategy, UniformListScrollHandle, px};
use plico::git::Commit;

pub(super) fn restore(
    scroll: &UniformListScrollHandle,
    before: &[Commit],
    after: &[Commit],
    history_offset: usize,
    row_height: f32,
) {
    // Read at publication time, not when the asynchronous query started.
    let (top, pending, position) = {
        let state = scroll.0.borrow();
        let position = state.base_handle.offset();
        let pending = state.deferred_scroll_to_item.is_some();
        let top = state
            .deferred_scroll_to_item
            .as_ref()
            .map(|item| item.item_index)
            .unwrap_or_else(|| ((-position.y / px(row_height)).max(0.)).floor() as usize);
        (top, pending, position)
    };
    let changed = before.iter().map(|c| &c.id).ne(after.iter().map(|c| &c.id));
    if !changed {
        return;
    }
    // Rebased SelectionSnapshot: isOnTop && permanentGraphChanged.
    // A bounded history page's first row is not the repository's first row.
    if top == 0 && history_offset == 0 {
        scroll.scroll_to_item_strict(0, ScrollStrategy::Top);
    } else if let Some(index) = before
        .get(top)
        .and_then(|anchor| after.iter().position(|c| c.id == anchor.id))
    {
        if pending {
            scroll.scroll_to_item_strict(index, ScrollStrategy::Top);
        } else {
            // Keep the visible commit at the same pixel height, including partial rows.
            let mut restored = position;
            restored.y -= px((index as f32 - top as f32) * row_height);
            scroll.0.borrow().base_handle.set_offset(restored);
        }
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn commits(ids: &[&str]) -> Vec<Commit> {
        ids.iter()
            .map(|id| Commit {
                id: (*id).into(),
                parents: vec![],
                shallow_boundary: false,
                author: String::new(),
                date: String::new(),
                subject: String::new(),
                refs: vec![],
            })
            .collect()
    }
    fn target(scroll: &UniformListScrollHandle) -> usize {
        scroll
            .0
            .borrow()
            .deferred_scroll_to_item
            .as_ref()
            .unwrap()
            .item_index
    }
    #[test]
    fn new_remote_commit_remains_visible_when_log_was_at_top() {
        let scroll = UniformListScrollHandle::new();
        restore(
            &scroll,
            &commits(&["old", "base"]),
            &commits(&["remote", "old", "base"]),
            0,
            28.,
        );
        assert_eq!(
            target(&scroll),
            0,
            "refresh must not scroll past the newly fetched commit"
        );
    }
    #[test]
    fn reading_old_history_keeps_its_commit_visible() {
        let scroll = UniformListScrollHandle::new();
        scroll.scroll_to_item_strict(1, ScrollStrategy::Top);
        restore(
            &scroll,
            &commits(&["old", "base"]),
            &commits(&["remote", "old", "base"]),
            0,
            28.,
        );
        assert_eq!(target(&scroll), 2);
    }
    #[test]
    fn top_of_later_history_window_is_not_repository_top() {
        let scroll = UniformListScrollHandle::new();
        restore(
            &scroll,
            &commits(&["old", "base"]),
            &commits(&["remote", "old", "base"]),
            1000,
            28.,
        );
        assert_eq!(scroll.0.borrow().base_handle.offset().y, px(-28.));
    }
    #[test]
    fn partially_visible_row_keeps_its_pixel_position() {
        let scroll = UniformListScrollHandle::new();
        scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui_kit::point(px(-12.), px(-35.)));
        restore(
            &scroll,
            &commits(&["old", "base"]),
            &commits(&["remote", "old", "base"]),
            0,
            28.,
        );
        assert_eq!(
            scroll.0.borrow().base_handle.offset(),
            gpui_kit::point(px(-12.), px(-63.))
        );
    }
    #[test]
    fn large_font_refresh_preserves_the_visible_commit_and_partial_row() {
        let scroll = UniformListScrollHandle::new();
        scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui_kit::point(px(0.), px(-44.)));
        restore(
            &scroll,
            &commits(&["old", "base"]),
            &commits(&["remote", "old", "base"]),
            0,
            37.,
        );
        assert_eq!(scroll.0.borrow().base_handle.offset().y, px(-81.));
    }
    #[test]
    fn reference_only_refresh_does_not_snap_partial_first_row() {
        let scroll = UniformListScrollHandle::new();
        scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui_kit::point(px(0.), px(-7.)));
        restore(
            &scroll,
            &commits(&["old", "base"]),
            &commits(&["old", "base"]),
            0,
            28.,
        );
        assert!(scroll.0.borrow().deferred_scroll_to_item.is_none());
        assert_eq!(scroll.0.borrow().base_handle.offset().y, px(-7.));
    }
}
