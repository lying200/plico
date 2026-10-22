//! Two-column Diff divider. The narrow hit area overlays the existing hairline.
use gpui_kit::*;
use std::{cell::Cell, rc::Rc};

pub fn ratio_at(x: Pixels, width: Pixels) -> f32 {
    if width <= px(0.) {
        return 0.5;
    }
    (f32::from(x) / f32::from(width)).clamp(0.1, 0.9)
}

pub fn handle(
    ratio: f32,
    dragging: Rc<Cell<bool>>,
    changed: Rc<dyn Fn(f32, &mut App)>,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let band = Bounds::new(
                point(
                    bounds.left() + bounds.size.width * ratio - px(4.),
                    bounds.top(),
                ),
                size(px(8.), bounds.size.height),
            );
            (bounds, window.insert_hitbox(band, HitboxBehavior::Normal))
        },
        move |_, (bounds, hitbox), window, _| {
            window.set_cursor_style(CursorStyle::ResizeLeftRight, &hitbox);
            let down = dragging.clone();
            let change = changed.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase.capture() && event.button == MouseButton::Left && hitbox.is_hovered(window)
                {
                    down.set(event.click_count != 2);
                    if event.click_count == 2 {
                        change(0.5, cx);
                    }
                    cx.stop_propagation();
                }
            });
            let moving = dragging.clone();
            let change = changed.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if !phase.capture() || !moving.get() {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    moving.set(false);
                    return;
                }
                change(
                    ratio_at(event.position.x - bounds.left(), bounds.size.width),
                    cx,
                );
                cx.stop_propagation();
            });
            let released = dragging.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase.capture() && event.button == MouseButton::Left && released.replace(false) {
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::ratio_at;
    use gpui_kit::px;
    #[::core::prelude::v1::test]
    fn divider_ratio_is_bounded_in_narrow_and_regular_views() {
        assert_eq!(ratio_at(px(300.), px(1000.)), 0.3);
        assert_eq!(ratio_at(px(-20.), px(1000.)), 0.1);
        assert_eq!(ratio_at(px(2000.), px(1000.)), 0.9);
        assert_eq!(ratio_at(px(10.), px(0.)), 0.5);
        assert_eq!(ratio_at(px(20.), px(40.)), 0.5);
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod interaction_tests {
    extern crate gpui_kit as gpui;
    use super::{Cell, Rc, handle};
    use core::prelude::v1::test;
    use gpui::*;
    struct Host {
        ratio: f32,
        dragging: Rc<Cell<bool>>,
    }
    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let owner = cx.entity().downgrade();
            div().relative().w(px(1000.)).h(px(500.)).child(handle(
                self.ratio,
                self.dragging.clone(),
                Rc::new(move |ratio, cx| {
                    owner
                        .update(cx, |s, cx| {
                            s.ratio = ratio;
                            cx.notify();
                        })
                        .unwrap();
                }),
            ))
        }
    }
    #[gpui::test]
    fn divider_drag_release_and_double_click(cx: &mut TestAppContext) {
        let (host, cx) = cx.add_window_view(|_, _| Host {
            ratio: 0.5,
            dragging: Default::default(),
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.simulate_mouse_move(point(px(500.), px(100.)), None, Modifiers::default());
        cx.simulate_mouse_down(
            point(px(500.), px(100.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(
            point(px(300.), px(100.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        assert_eq!(host.read_with(cx, |s, _| s.ratio), 0.3);
        cx.simulate_mouse_up(
            point(px(300.), px(100.)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(point(px(700.), px(100.)), None, Modifiers::default());
        assert_eq!(host.read_with(cx, |s, _| s.ratio), 0.3);
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.simulate_mouse_move(point(px(300.), px(100.)), None, Modifiers::default());
        cx.simulate_event(MouseDownEvent {
            position: point(px(300.), px(100.)),
            button: MouseButton::Left,
            click_count: 2,
            ..Default::default()
        });
        assert_eq!(host.read_with(cx, |s, _| s.ratio), 0.5);
        assert!(!host.read_with(cx, |s, _| s.dragging.get()));
    }
}
