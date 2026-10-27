use super::*;

impl PlanTable {
    pub(super) fn select_with_mouse(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing() {
            if self.apply_message(window, cx) {
                self.mouse_selection_serial = self.mouse_selection_serial.wrapping_add(1);
                self.pending_mouse_selection = Some((self.mouse_selection_serial, event.clone()));
                cx.notify();
            }
            return;
        }
        self.select(index, event.modifiers, window, cx);
        if event.click_count == 2 && self.selected.len() == 1 {
            self.begin_message(self.selected[0], window, cx);
        }
    }

    pub(super) fn mouse_selection_after_layout(&self, cx: &Context<Self>) -> impl IntoElement {
        let pending = self.pending_mouse_selection.clone();
        let owner = cx.entity().downgrade();
        let list = self.list.clone();
        let count = self.model().rows().len();
        // Swing stops editing before hit testing. GPUI's row listener belongs to
        // the old layout, so resolve the same point after the collapsed list paints.
        canvas(
            |_, _, _| (),
            move |_, _, window, cx| {
                let Some((serial, event)) = pending else {
                    return;
                };
                let mut target = None;
                if list.viewport_bounds().contains(&event.position) {
                    for index in list.logical_scroll_top().item_ix..count {
                        let Some(bounds) = list.bounds_for_item(index) else {
                            break;
                        };
                        if bounds.contains(&event.position) {
                            target = Some(index);
                            break;
                        }
                        if bounds.top() > event.position.y {
                            break;
                        }
                    }
                }
                window.defer(cx, move |window, cx| {
                    let _ = owner.update(cx, |table, cx| {
                        if table.pending_mouse_selection.as_ref().map(|p| p.0) != Some(serial) {
                            return;
                        }
                        table.pending_mouse_selection = None;
                        if let Some(index) = target {
                            table.select_with_mouse(index, &event, window, cx);
                        }
                    });
                });
            },
        )
        .absolute()
        .size_full()
    }
}
