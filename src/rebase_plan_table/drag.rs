//! Single-row DnD follows RowsDnDSupport; group movement belongs to Model::exchange.
use super::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub(super) struct RowDrag {
    owner: EntityId,
    row: RowId,
    from: usize,
    target: Rc<Cell<Option<usize>>>,
    active: Rc<Cell<bool>>,
}
struct Preview(RowDrag);
impl Drop for Preview {
    fn drop(&mut self) {
        self.0.active.set(false);
        self.0.target.set(None);
    }
}
impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
impl PlanTable {
    pub(super) fn cancel_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self
            .drag
            .as_ref()
            .is_some_and(|drag| drag.active.replace(false))
        {
            self.drag = None;
            cx.stop_active_drag(window);
            cx.notify();
            return true;
        }
        false
    }

    pub(super) fn drag_line(&self, index: usize) -> Option<bool> {
        self.drag
            .as_ref()
            .filter(|drag| {
                drag.active.get() && drag.target.get() == Some(index) && drag.from != index
            })
            .map(|drag| drag.from < index)
    }
    pub(super) fn drag_row(
        &self,
        index: usize,
        row: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let owner = cx.entity().downgrade();
        let drag = RowDrag {
            owner: cx.entity_id(),
            row: self.model().rows()[index].id,
            from: index,
            target: Rc::new(Cell::new(None)),
            active: Rc::new(Cell::new(false)),
        };
        row.when(!self.editing(), |row| {
            row.on_drag(drag, move |drag, _, _, cx| {
                drag.active.set(true);
                let _ = owner.update(cx, |s, cx| {
                    s.drag = Some(drag.clone());
                    cx.notify();
                });
                cx.stop_propagation();
                cx.new(|_| Preview(drag.clone()))
            })
        })
        .on_drag_move(cx.listener(move |s, e: &DragMoveEvent<RowDrag>, _, cx| {
            let drag = e.drag(cx);
            if drag.owner != cx.entity_id() || s.editing() {
                return;
            }
            if e.bounds.contains(&e.event.position) {
                if drag.target.replace(Some(index)) != Some(index) {
                    cx.notify();
                }
            } else if drag.target.get() == Some(index) {
                drag.target.set(None);
                cx.notify();
            }
        }))
        .on_drop(cx.listener(move |s, drag: &RowDrag, w, cx| {
            s.drop_row(drag, index, w, cx);
        }))
    }
    fn drop_row(
        &mut self,
        drag: &RowDrag,
        target: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if drag.owner != cx.entity_id() || !drag.active.get() || self.editing() {
            return;
        }
        drag.active.set(false);
        drag.target.set(None);
        let Some(from) = self.model().index(drag.row) else {
            return;
        };
        if self.draft.reorder(from, target) {
            // RowsDnDSupport selects the target index, including a group's child.
            let id = self.model().rows()[target].id;
            self.selected = vec![id];
            self.anchor = Some(id);
            self.reset_visible = true;
            self.focus.focus(window, cx);
            self.changed(cx);
        }
    }
}
