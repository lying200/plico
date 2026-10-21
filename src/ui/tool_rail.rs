//! Rebased stripe semantics: preview a slot while dragging, commit only on drop.
use super::*;
use plico::tool_window_order::Tool;
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub(super) struct DragSession {
    tool: Tool,
    owner: EntityId,
    active: Rc<Cell<bool>>,
    gap: Rc<Cell<Option<usize>>>,
}
struct DragPreview(DragSession);
impl Drop for DragPreview {
    fn drop(&mut self) {
        self.0.active.set(false);
        self.0.gap.set(None);
    }
}
fn icon(tool: Tool) -> &'static str {
    match tool {
        Tool::Commit => "tool-commit_dark.svg",
        Tool::Comparison => "tool-changes_dark.svg",
        Tool::Project => "tool-project_dark.svg",
    }
}
fn label(tool: Tool) -> &'static str {
    match tool {
        Tool::Commit => "Commit",
        Tool::Comparison => "Compare Versions",
        Tool::Project => "Project",
    }
}
impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .bg(rgb(SELECT))
            .child(jb_icon(icon(self.0.tool)).size(px(20.)))
    }
}
impl Review {
    pub(super) fn cancel_tool_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(drag) = &self.tool_drag {
            if drag.active.replace(false) {
                drag.gap.set(None);
                cx.stop_active_drag(window);
                cx.notify();
                return true;
            }
        }
        false
    }
    pub(super) fn tool_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tools = self.tool_order.visible(self.comparison.is_some());
        let dragging = self.tool_drag.as_ref().filter(|d| d.active.get());
        let mut children = Vec::new();
        let visible: Vec<_> = tools
            .iter()
            .copied()
            .filter(|t| dragging.is_none_or(|d| d.tool != *t))
            .collect();
        for index in 0..=visible.len() {
            if dragging.is_some_and(|d| d.gap.get() == Some(index)) {
                children.push(
                    div()
                        .w(px(32.))
                        .h(px(32.))
                        .flex_shrink_0()
                        .rounded(px(6.))
                        .bg(rgb(SELECT))
                        .border_1()
                        .border_color(rgb(TOOL_DROP_BORDER))
                        .into_any_element(),
                );
            }
            let Some(&tool) = visible.get(index) else {
                continue;
            };
            let selected = match tool {
                Tool::Commit => self.local_visible || self.stash_open,
                Tool::Comparison => self.comparison_visible,
                Tool::Project => self.project_visible,
            };
            let owner = cx.entity().downgrade();
            let drag = DragSession {
                tool,
                owner: cx.entity_id(),
                active: Rc::new(Cell::new(false)),
                gap: Rc::new(Cell::new(None)),
            };
            let original_index = tools.iter().position(|t| *t == tool).unwrap();
            children.push(
                div()
                    .id(tool.key())
                    .w(px(32.))
                    .h(px(32.))
                    .flex_shrink_0()
                    .child(
                        Button::new(tool.key())
                            .ghost()
                            .selected(selected)
                            .w(px(32.))
                            .h(px(32.))
                            .flex_shrink_0()
                            .icon_size(px(20.))
                            .icon(jb_icon(icon(tool)))
                            .tooltip(t(label(tool)))
                            .accessibility_label(t(label(tool)))
                            .on_click(cx.listener(move |s, _, w, cx| match tool {
                                Tool::Commit => s.toggle_commit_window(w, cx),
                                Tool::Project => s.toggle_project(w, cx),
                                Tool::Comparison => {
                                    s.comparison_visible = !s.comparison_visible;
                                    if s.comparison_visible {
                                        s.cancel_project_preview(cx);
                                        s.local_visible = false;
                                        s.project_visible = false;
                                        s.stash_open = false;
                                    }
                                    cx.notify();
                                }
                            })),
                    )
                    .on_drag(drag, move |drag, _, _, cx| {
                        drag.active.set(true);
                        drag.gap.set(Some(original_index));
                        let _ = owner.update(cx, |s, cx| {
                            s.tool_drag = Some(drag.clone());
                            cx.notify();
                        });
                        cx.stop_propagation();
                        cx.new(|_| DragPreview(drag.clone()))
                    })
                    .into_any_element(),
            );
        }
        v_flex()
            .id("tool-rail")
            .w(px(36.))
            .h_full()
            .gap(px(4.))
            .children(children)
            .on_drag_move(cx.listener(|s, event: &DragMoveEvent<DragSession>, _, cx| {
                let drag = event.drag(cx);
                if drag.owner != cx.entity_id() {
                    return;
                }
                let gap = event.bounds.contains(&event.event.position).then(|| {
                    let count = s.tool_order.visible(s.comparison.is_some()).len();
                    (((event.event.position.y - event.bounds.origin.y).as_f32() / 36.).floor()
                        as usize)
                        .min(count - 1)
                });
                if drag.gap.replace(gap) != gap {
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|s, drag: &DragSession, _, cx| {
                if drag.owner != cx.entity_id() {
                    return;
                }
                if let Some(gap) = drag.gap.get() {
                    let order = s.tool_order.moved(drag.tool, gap, s.comparison.is_some());
                    if order != s.tool_order {
                        match layout_preferences::save_tool_window_order(&order) {
                            Ok(()) => s.tool_order = order,
                            Err(error) => s.error = Some(error.to_string()),
                        }
                    }
                }
                drag.active.set(false);
                drag.gap.set(None);
                cx.notify();
            }))
    }
}
