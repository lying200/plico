//! Merge block controls use editor geometry from this frame, never a timer.
use super::*;

impl MergeEditor {
    pub(super) fn gutters(&self, cx: &Context<Self>) -> impl IntoElement {
        let editors = self.inputs.clone();
        let ranges = self.line_ranges(cx);
        let states = self
            .model
            .borrow()
            .changes
            .iter()
            .map(|c| {
                (
                    [c.available(Side::Yours), c.available(Side::Theirs)],
                    c.appended,
                    c.sides,
                    c.kind(),
                    [c.changed(Side::Yours), c.changed(Side::Theirs)],
                    c.resolved,
                )
            })
            .collect::<Vec<_>>();
        let busy = self.applying;
        let owner = cx.entity().downgrade();
        let font_size = crate::ui_font::editor_size(cx);
        let font = gpui_kit::font(crate::ui_font::editor_family(cx));
        canvas(
            move |bounds, window, cx| {
                let width = bounds.size.width / 3.;
                let edges = [bounds.left() + width, bounds.left() + width * 2.];
                let mut bands = Vec::new();
                let mut buttons = Vec::new();
                let mut borders = Vec::new();
                for (index, range) in ranges.iter().enumerate() {
                    let extents: Vec<_> = editors
                        .iter()
                        .zip(range)
                        .map(|(e, r)| e.read(cx).source_line_extent(r.clone()))
                        .collect();
                    let (available, append, resolved, kind, changed, complete) = states[index];
                    for pane in 0..3 {
                        if (pane == 0 && !changed[0]) || (pane == 2 && !changed[1]) {
                            continue;
                        }
                        let done = match pane {
                            0 => resolved[0],
                            2 => resolved[1],
                            _ => complete,
                        };
                        if let Some(extent) = extents[pane]
                            && (done || range[pane].is_empty())
                        {
                            borders.push((pane, extent, done, kind));
                        }
                    }
                    for (side, source) in [(0, 0), (1, 2)] {
                        if !changed[side] {
                            continue;
                        }
                        let (Some(a), Some(b)) = (extents[source], extents[1]) else {
                            continue;
                        };
                        if a.1.max(b.1) < bounds.top() || a.0.min(b.0) > bounds.bottom() {
                            continue;
                        }
                        bands.push((side, a, b, resolved[side], kind));
                        if busy
                            || !available[side]
                            || a.0 < bounds.top()
                            || a.0 + px(18.) > bounds.bottom()
                        {
                            continue;
                        }
                        // Controls mirror each other: x >> | Result | << x.
                        let accept_x = if side == 0 {
                            edges[0] - px(74.)
                        } else {
                            edges[1] + px(28.)
                        };
                        for ignore in [false, true] {
                            let x = accept_x
                                + if ignore {
                                    if side == 0 { -px(18.) } else { px(18.) }
                                } else {
                                    px(0.)
                                };
                            let rect = Bounds::new(point(x, a.0), size(px(18.), px(18.)));
                            let icon = if ignore {
                                "jb/merge-ignore.svg"
                            } else {
                                match (side, append) {
                                    (0, false) => "jb/diff-apply-right_dark.svg",
                                    (1, false) => "jb/merge-accept-left.svg",
                                    (0, true) => "jb/merge-append-right.svg",
                                    _ => "jb/merge-append-left.svg",
                                }
                            };
                            buttons.push((
                                index,
                                side,
                                ignore,
                                rect,
                                icon,
                                window.insert_hitbox(rect, HitboxBehavior::Normal),
                            ));
                        }
                    }
                }
                let lines = [0, 2].map(|i| {
                    (
                        editors[i].read(cx).visible_source_line_extents(),
                        editors[i].read(cx).line_height().unwrap_or(font_size * 1.5),
                    )
                });
                (bounds, edges, bands, buttons, lines, borders)
            },
            move |_, (bounds, edges, bands, buttons, lines, borders), window, cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    for (side, a, b, resolved, kind) in bands {
                        let (a, b) = if side == 0 { (a, b) } else { (b, a) };
                        let left = edges[side] - px(32.);
                        let right = edges[side];
                        let color = color(kind, cx);
                        if resolved {
                            dotted_curve(left, right, a.0, b.0, color, bounds, window);
                            dotted_curve(left, right, a.1, b.1, color, bounds, window);
                            continue;
                        }
                        let mut path = PathBuilder::fill();
                        let top_a = if a.0 == a.1 { a.0 - px(1.) } else { a.0 };
                        let top_b = if b.0 == b.1 { b.0 - px(1.) } else { b.0 };
                        path.move_to(point(left, top_a));
                        path.cubic_bezier_to(
                            point(right, top_b),
                            point(left + px(10.), top_a),
                            point(right - px(10.), top_b),
                        );
                        path.line_to(point(right, b.1));
                        path.cubic_bezier_to(
                            point(left, a.1),
                            point(right - px(10.), b.1),
                            point(left + px(10.), a.1),
                        );
                        path.close();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                        // The source-side gutter is part of the block highlight.
                        let (start, end, y, z) = if side == 0 {
                            (left - px(70.), left, a.0, a.1)
                        } else {
                            (right, right + px(70.), b.0, b.1)
                        };
                        if !resolved {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(start, y),
                                    size(end - start, (z - y).max(px(1.))),
                                ),
                                color,
                            ));
                        }
                    }
                    for (pane, (top, bottom), resolved, kind) in borders {
                        let (left, right) = match pane {
                            0 => (bounds.left(), edges[0] - px(32.)),
                            1 => (edges[0], edges[1] - px(32.)),
                            _ => (edges[1], bounds.right()),
                        };
                        for y in if top == bottom {
                            vec![top - px(1.), top]
                        } else {
                            vec![top, bottom]
                        } {
                            if y < bounds.top() || y > bounds.bottom() {
                                continue;
                            }
                            if resolved {
                                dotted_curve(left, right, y, y, color(kind, cx), bounds, window);
                            } else {
                                window.paint_quad(fill(
                                    Bounds::new(point(left, y), size(right - left, px(1.))),
                                    color(kind, cx),
                                ));
                            }
                        }
                    }
                    for (side, (rows, height)) in lines.into_iter().enumerate() {
                        for (line, top, _) in rows {
                            let text: SharedString = (line + 1).to_string().into();
                            let shaped = window.text_system().shape_line(
                                text.clone(),
                                font_size,
                                &[TextRun {
                                    len: text.len(),
                                    font: font.clone(),
                                    color: rgb(crate::ui_theme::color(cx, 0x6f737a, 0x73767c))
                                        .into(),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                }],
                                None,
                            );
                            let x = if side == 0 {
                                edges[0] - px(36.) - shaped.width
                            } else {
                                edges[1] + px(24.) - shaped.width
                            };
                            let _ = shaped.paint(
                                point(x, top),
                                height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    }
                    for (_, _, _, rect, icon, hit) in &buttons {
                        window.set_cursor_style(CursorStyle::PointingHand, hit);
                        let _ = window.paint_svg(
                            Bounds::new(
                                rect.origin + point(px(2.), px(2.)),
                                size(px(14.), px(14.)),
                            ),
                            (*icon).into(),
                            None,
                            TransformationMatrix::unit(),
                            rgb(crate::ui_theme::color(cx, 0xced0d6, 0x000000)).into(),
                            cx,
                        );
                    }
                });
                let owner = owner.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if !phase.bubble() || event.button != MouseButton::Left {
                        return;
                    }
                    for (index, side, ignore, _, _, hit) in &buttons {
                        if hit.is_hovered(window) {
                            let _ = owner.update(cx, |s, cx| {
                                s.decide(
                                    *index,
                                    if *side == 0 {
                                        Side::Yours
                                    } else {
                                        Side::Theirs
                                    },
                                    *ignore,
                                    event.modifiers.control,
                                    window,
                                    cx,
                                )
                            });
                            cx.stop_propagation();
                            break;
                        }
                    }
                });
            },
        )
        .absolute()
        .inset_0()
        .size_full()
    }
}

// Same semantic colors in the editors, gutters and connecting bands.
pub(super) fn color(kind: plico::text_merge::Kind, cx: &App) -> Rgba {
    use plico::text_merge::Kind;
    rgb(match kind {
        Kind::Added => crate::ui_theme::color(cx, 0x294436, 0xbee6be),
        Kind::Deleted => crate::ui_theme::color(cx, 0x484a4a, 0xd6d6d6),
        Kind::Modified => crate::ui_theme::color(cx, 0x385570, 0xc2d8f2),
        Kind::Conflict => crate::ui_theme::color(cx, 0x49332e, 0xffd5cc),
    })
}

/// Dot spacing follows arc length, including steep connecting curves.
/// Clip before painting so offscreen blocks do not create offscreen quads.
fn dotted_curve(
    left: Pixels,
    right: Pixels,
    y1: Pixels,
    y2: Pixels,
    color: Rgba,
    clip: Bounds<Pixels>,
    window: &mut Window,
) {
    let width = f32::from(right - left);
    let delta = f32::from(y2 - y1);
    let steps = (width.abs() + delta.abs()).ceil().max(1.) as usize;
    let bend = width.min(32.) / 3.;
    let mut previous = (0., 0.);
    let mut distance = 3.;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let u = 1. - t;
        let x = 3. * u * u * t * bend + 3. * u * t * t * (width - bend) + t * t * t * width;
        let y = (3. * u * t * t + t * t * t) * delta;
        distance += ((x - previous.0).powi(2) + (y - previous.1).powi(2)).sqrt();
        previous = (x, y);
        if distance >= 3. {
            distance %= 3.;
            let top = y1 + px(y);
            if top >= clip.top() && top <= clip.bottom() {
                window.paint_quad(fill(
                    Bounds::new(point(left + px(x), top - px(1.)), size(px(1.), px(1.))),
                    color,
                ));
            }
        }
    }
}
