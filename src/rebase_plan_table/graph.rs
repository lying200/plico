//! Rebased 4aec352 GitRebaseCommitsTableView renderers (Apache-2.0).
//! Geometry is row-local, so virtualized rows paint their own connecting segments.
use super::*;

impl PlanTable {
    pub(super) fn graph(&self, index: usize, selected: bool, cx: &App) -> AnyElement {
        let rows = self.model().rows();
        let row = &rows[index];
        let edit = matches!(row.action(), Action::Edit);
        let node = matches!(row.action(), Action::Pick | Action::Reword(_));
        let double = rows
            .get(index + 1)
            .is_some_and(|next| next.parent() == Some(row.id));
        let parent = row.parent();
        let first = parent.is_some()
            && index
                .checked_sub(1)
                .and_then(|i| rows.get(i))
                .is_none_or(|prev| prev.parent() != parent);
        let last = rows
            .get(index + 1)
            .is_none_or(|next| next.parent() != parent);
        let head = index + 1 == rows.len();
        let width = if parent.is_some() { 44. } else { 24. };
        let background = if selected {
            crate::ui_theme::palette(cx).selection
        } else {
            crate::ui_theme::palette(cx).background
        };
        div()
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(px(width))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        let ink = rgb(crate::ui_theme::color(cx, 0x87929a, 0x6c707e));
                        let p = |x: f32, y: f32| point(bounds.left() + px(x), bounds.top() + px(y));
                        let height = bounds.size.height.as_f32();
                        let line = |a, b, window: &mut Window| {
                            let mut path = PathBuilder::stroke(px(1.5));
                            path.move_to(a);
                            path.line_to(b);
                            if let Ok(path) = path.build() {
                                window.paint_path(path, ink);
                            }
                        };
                        let circle = |x: f32, radius: f32, color, window: &mut Window| {
                            window.paint_quad(quad(
                                Bounds::new(
                                    p(x - radius + 0.5, 11. - radius + 0.5),
                                    size(px(radius * 2.), px(radius * 2.)),
                                ),
                                px(radius),
                                color,
                                px(0.),
                                color,
                                BorderStyle::Solid,
                            ));
                        };
                        if !edit {
                            line(p(8., 0.), p(8., 11.), window);
                            if !head {
                                line(p(8., 11.), p(8., height), window);
                            }
                            if node {
                                if double {
                                    circle(12., 4., ink, window);
                                    circle(8., 5., rgb(background), window);
                                }
                                circle(8., 4., ink, window);
                            }
                        }
                        if parent.is_some() {
                            let middle = height / 2.;
                            line(p(29., middle), p(40., middle), window);
                            line(p(29., middle), p(29., if first { 4. } else { 0. }), window);
                            if !last {
                                line(p(29., middle), p(29., height), window);
                            }
                            if first {
                                let mut arrow = PathBuilder::fill();
                                arrow.move_to(p(29., 0.));
                                arrow.line_to(p(25., 4.));
                                arrow.line_to(p(33., 4.));
                                arrow.close();
                                if let Ok(path) = arrow.build() {
                                    window.paint_path(path, ink);
                                }
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .when(edit, |v| {
                v.child(
                    Icon::default()
                        .path("jb/pause_dark.svg")
                        .size(px(16.))
                        .absolute()
                        .left_0()
                        .top(px(3.)),
                )
            })
            .into_any_element()
    }
}
