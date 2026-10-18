//! Horizontal content travel is independent of the virtual list's column layout.
//! Both split viewports share an offset, but retain their own clipping bounds.
use std::{cell::RefCell, rc::Rc};

use gpui::{AnyElement, App, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, Size, Window, point, px};
use gpui_base::ScrollbarHandle;

#[derive(Clone, Default)]
pub(crate) struct HorizontalScroll(Rc<RefCell<Metrics>>);

#[derive(Default)]
struct Metrics {
    viewport: Bounds<Pixels>,
    content_width: Pixels,
    columns: usize,
    wrapped: bool,
    x: Pixels,
}

impl HorizontalScroll {
    pub fn configure(&self, content_width: Pixels, columns: usize, wrapped: bool) {
        let mut metrics = self.0.borrow_mut();
        metrics.content_width = content_width;
        metrics.columns = columns.max(1);
        metrics.wrapped = wrapped;
    }

    pub fn set_viewport(&self, bounds: Bounds<Pixels>) {
        self.0.borrow_mut().viewport = bounds;
        // Resize/wrap changes clamp travel before any child paints.
        self.set_offset(self.offset());
    }
}

impl ScrollbarHandle for HorizontalScroll {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        let metrics = self.0.borrow();
        let mut bounds = metrics.viewport;
        bounds.size.width /= metrics.columns.max(1) as f32;
        bounds
    }

    fn content_size(&self) -> Size<Pixels> {
        let viewport = self.viewport_bounds().size;
        let metrics = self.0.borrow();
        Size {
            width: if metrics.wrapped { viewport.width } else {
                (metrics.content_width / metrics.columns.max(1) as f32).max(viewport.width)
            },
            height: viewport.height,
        }
    }

    fn offset(&self) -> Point<Pixels> { point(self.0.borrow().x, px(0.)) }

    fn set_offset(&self, offset: Point<Pixels>) {
        let max = (self.content_size().width - self.viewport_bounds().size.width).max(px(0.));
        self.0.borrow_mut().x = offset.x.clamp(-max, px(0.));
    }
}

/// Move content within an enclosing clipped viewport. Read the current offset
/// during prepaint, including when virtualized rows reuse their render result.
pub(crate) struct HorizontalOffset {
    child: AnyElement,
    scroll: HorizontalScroll,
}

impl HorizontalOffset {
    pub fn new(child: AnyElement, scroll: HorizontalScroll) -> Self { Self { child, scroll } }
}

impl IntoElement for HorizontalOffset {
    type Element = Self;
    fn into_element(self) -> Self { self }
}

impl Element for HorizontalOffset {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> { None }
    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>,
        window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>,
        _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        window.with_element_offset(self.scroll.offset(), |window| self.child.prepaint(window, cx));
    }
    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>,
        _: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        self.child.paint(window, cx);
    }
}
