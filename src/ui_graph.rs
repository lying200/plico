//! Log graph drawing and hit testing. Callers own navigation and async state.

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    graph::{self, Row},
    i18n::t,
};

const COLORS: [u32; 8] = [
    0x6aab73, 0x548af7, 0xc77dbb, 0xe5b567, 0x56b6c2, 0xcc7832, 0x9876aa, 0xe06c75,
];

pub(crate) fn graph_arrow_tooltip(id: &str, subject: &str, author: &str, date: &str) -> String {
    let chars: Vec<_> = subject.chars().take(51).collect();
    let title: String = if chars.len() > 50 {
        chars[..47].iter().collect::<String>() + "..."
    } else {
        chars.iter().collect()
    };
    format!(
        "{} {}\n{title}\n{author} · {date}",
        t("Jump to Commit"),
        &id[..id.len().min(8)]
    )
}

/// Shared hit regions, cursor and tooltips; owners supply their own guarded
/// selection/preview callbacks. No history queries or owner lifetimes live here.
#[derive(Clone, Copy)]
pub(crate) struct GraphNodeStyle {
    pub head: bool,
    pub background: u32,
}

pub(crate) fn graph_arrow_cell(
    row: Row,
    width: f32,
    row_height: f32,
    index: usize,
    node_style: GraphNodeStyle,
    hovered: Option<usize>,
    tooltip: Option<String>,
    on_hover: impl Fn(Option<usize>, &mut App) + 'static,
    on_jump: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> AnyElement {
    if !row.segments.iter().any(|edge| edge.arrow()) {
        return graph_cell(row, width, row_height, node_style, None).into_any_element();
    }
    let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::default()));
    let actionable = hovered.is_some_and(|segment| row.arrow_target(segment).is_some());
    let hit = {
        let bounds = bounds.clone();
        let row = row.clone();
        move |position: Point<Pixels>| {
            let bounds = bounds.get();
            if !bounds.contains(&position) {
                return None;
            }
            let point = position - bounds.origin;
            graph::arrow_at_height(&row, point.x.as_f32(), point.y.as_f32(), row_height)
        }
    };
    let hover_hit = hit.clone();
    let down_hit = hit.clone();
    let on_hover = std::rc::Rc::new(on_hover);
    let leave = on_hover.clone();
    graph_cell(row, width, row_height, node_style, hovered)
        .id(("graph-arrow", index))
        // Explicit anchoring avoids the default static position below the
        // drawing canvas, which would offset the hit region by a whole row.
        .child(
            canvas(move |value, _, _| bounds.set(value), |_, _, _, _| {})
                .absolute()
                .inset_0(),
        )
        .when(actionable, |v| v.cursor_pointer())
        .when_some(tooltip, |v, text| {
            v.tooltip(move |w, cx| Tooltip::new(text.clone()).build(w, cx))
        })
        .on_mouse_move(move |event, _, cx| {
            on_hover(hover_hit(event.position).map(|hit| hit.segment), cx)
        })
        .on_hover(move |hovered, _, cx| {
            if !hovered {
                leave(None, cx);
            }
        })
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            if down_hit(event.position).is_some() {
                cx.stop_propagation();
            }
        })
        .on_click(move |event, window, cx| {
            let Some(hit) = hit(event.position()) else {
                return;
            };
            cx.stop_propagation();
            if let Some(target) = hit.target {
                on_jump(target, window, cx);
            }
        })
        .into_any_element()
}

pub(crate) fn graph_cell(
    row: Row,
    width: f32,
    row_height: f32,
    node_style: GraphNodeStyle,
    hovered: Option<usize>,
) -> Div {
    let node = row.node;
    let color = row.color;
    div()
        .relative()
        .overflow_hidden()
        .w(px(width))
        .h(px(row_height))
        .flex_none()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let visible = bounds.intersect(&window.content_mask().bounds);
                    // FromPrintElement selects this exact row/segment. Like
                    // SimpleGraphCellPainter, paint it last: themed outline,
                    // then the ordinary branch color (including arrow wings).
                    let selected = hovered
                        .filter(|&index| row.segments.get(index).is_some_and(|edge| edge.arrow()));
                    let selected_edge = selected.and_then(|index| row.segments.get(index));
                    let passes = row
                        .segments
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| Some(*index) != selected)
                        .map(|(_, edge)| (edge, false))
                        .chain(selected_edge.map(|edge| (edge, true)))
                        .chain(selected_edge.map(|edge| (edge, false)));
                    for (edge, outline) in passes {
                        // Preserve the existing 1.5 logical pixel ordinary
                        // geometry; guarantee one physical pixel on each side.
                        let thickness = if outline {
                            2.5_f32.max(1.5 + 2. / window.scale_factor())
                        } else {
                            1.5
                        };
                        let ink = if outline {
                            crate::ui_theme::palette(cx).foreground
                        } else {
                            COLORS[edge.color % COLORS.len()]
                        };
                        let x = |lane: f32| bounds.origin.x + px(8. + lane * 16.);
                        let middle = (edge.from() as f32 + edge.to() as f32) / 2.;
                        let from = point(
                            x(if edge.upper() && edge.adjacent() {
                                middle
                            } else {
                                edge.from() as f32
                            }),
                            bounds.origin.y + px(if edge.upper() { 0. } else { row_height / 2. }),
                        );
                        let to = point(
                            x(if !edge.upper() && edge.adjacent() {
                                middle
                            } else {
                                edge.to() as f32
                            }),
                            bounds.origin.y
                                + px(if edge.full() || !edge.upper() {
                                    row_height
                                } else {
                                    row_height / 2.
                                }),
                        );
                        // The Commit cell clips wide graphs before the author
                        // column. Avoid creating GPU paths wholly outside that
                        // clip; keep crossing edges and the arrow's stroke halo.
                        let halo = px(if edge.arrow() { 9. } else { 2. });
                        if from.x.min(to.x) > visible.right() + halo
                            || from.x.max(to.x) < visible.left() - halo
                            || from.y.min(to.y) > visible.bottom() + halo
                            || from.y.max(to.y) < visible.top() - halo
                        {
                            continue;
                        }
                        let mut p = PathBuilder::stroke(px(thickness));
                        if edge.dotted() {
                            // Rebased uses one dash/space per vertical row,
                            // centered on the node; our paths are half-row segments.
                            let dx = (to.x - from.x).as_f32();
                            let dy = (to.y - from.y).as_f32();
                            let length = dx.hypot(dy);
                            let period = length * 2. / (length * 2. / row_height).floor().max(1.);
                            let dash = period - 12.;
                            let phase = dash / 2. + if edge.upper() { length } else { 0. };
                            let mut at = 0.;
                            while at < length {
                                let position = (at + phase) % period;
                                let on = position < dash;
                                let step = (if on {
                                    dash - position
                                } else {
                                    period - position
                                })
                                .max(0.01);
                                let end = (at + step).min(length);
                                if on {
                                    p.move_to(point(
                                        from.x + px(dx * at / length),
                                        from.y + px(dy * at / length),
                                    ));
                                    p.line_to(point(
                                        from.x + px(dx * end / length),
                                        from.y + px(dy * end / length),
                                    ));
                                }
                                at = end;
                            }
                        } else {
                            p.move_to(from);
                            p.line_to(to);
                        }
                        if let Ok(path) = p.build() {
                            window.paint_path(path, rgb(ink));
                        }
                        if edge.arrow() {
                            let tip = if edge.upper() { from } else { to };
                            let center = point(
                                x(if edge.upper() { edge.to() } else { edge.from() } as f32),
                                bounds.origin.y + px(row_height / 2.),
                            );
                            let dx = (tip.x - center.x).as_f32();
                            let dy = (tip.y - center.y).as_f32();
                            let length = dx.hypot(dy).max(0.01);
                            // SimpleGraphCellPainter: length=.3*rowHeight,
                            // cos²(angle)=.7, oriented along the actual edge.
                            let back = 0.7_f32.sqrt() * 8.4;
                            let side = 0.3_f32.sqrt() * 8.4;
                            let mut arrow = PathBuilder::stroke(px(thickness));
                            for sign in [-1., 1.] {
                                arrow.move_to(point(
                                    tip.x - px(dx / length * back) + px(-dy / length * side * sign),
                                    tip.y - px(dy / length * back) + px(dx / length * side * sign),
                                ));
                                arrow.line_to(tip);
                            }
                            if let Ok(path) = arrow.build() {
                                window.paint_path(path, rgb(ink));
                            }
                        }
                    }
                },
            )
            .size_full(),
        )
        .child(if node_style.head {
            // HeadNodePainter: colored outer ring, commit-row background,
            // colored center. Keep this presentation out of shared geometry.
            div()
                .absolute()
                .left(px(2. + node as f32 * 16.))
                .top(px(row_height / 2. - 6.))
                .size(px(12.))
                .rounded_full()
                .bg(rgb(COLORS[color % COLORS.len()]))
                .child(
                    div()
                        .absolute()
                        .left(px(2.))
                        .top(px(2.))
                        .size(px(8.))
                        .rounded_full()
                        .bg(rgb(node_style.background))
                        .child(
                            div()
                                .absolute()
                                .left(px(2.))
                                .top(px(2.))
                                .size(px(4.))
                                .rounded_full()
                                .bg(rgb(COLORS[color % COLORS.len()])),
                        ),
                )
        } else {
            div()
                .absolute()
                .left(px(4. + node as f32 * 16.))
                .top(px(row_height / 2. - 4.))
                .size(px(8.))
                .rounded_full()
                .bg(rgb(COLORS[color % COLORS.len()]))
        })
}
