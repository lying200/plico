//! Bridge the virtualized code surface to Base's window text selection.
//! Content keys are exact source offsets, so copying does not depend on which
//! rows are still painted when the gesture ends.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    ops::Range,
    rc::Rc,
};

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, HighlightStyle, Hitbox,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, ListState, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, StyledText,
    TextLayout, Window, px,
};
use gpui_base::{
    ScrollbarHandle, TextSelectionHandle, TextSelectionRegistration, TextSelectionRun,
};

use super::{
    DiffFile, DiffSide,
    horizontal::HorizontalScroll,
    state::{decode_key, encode_key},
};

pub(crate) struct CodeRun {
    file: usize,
    side: DiffSide,
    line_ix: usize,
    display_range: Range<usize>,
    layout: TextLayout,
    bounds: Bounds<Pixels>,
    document: DiffFile,
    visible_bounds: Bounds<Pixels>,
}

#[derive(Default)]
pub(crate) struct SelectionGeometry {
    bounds: Bounds<Pixels>,
    scroll_offset: Point<Pixels>,
    multi_click: Cell<Option<(usize, Point<Pixels>, bool)>>,
    runs: Vec<CodeRun>,
}

impl SelectionGeometry {
    pub fn clear(&mut self) {
        self.runs.clear();
    }

    /// The widest painted source line, measured from its first text run.
    pub fn painted_width(&self) -> Pixels {
        self.runs
            .iter()
            .fold((None, px(0.), px(0.)), |(previous, left, widest), run| {
                let key = (run.file, run.side, run.line_ix);
                let left = if previous == Some(key) {
                    left
                } else {
                    run.bounds.left()
                };
                (Some(key), left, widest.max(run.bounds.right() - left))
            })
            .2
    }
    #[cfg(test)]
    pub fn key_at(&self, point: Point<Pixels>, side: Option<DiffSide>) -> Option<u64> {
        self.key_at_file(point, side, None)
    }
    pub fn key_at_file(&self, point: Point<Pixels>, side: Option<DiffSide>, file: Option<usize>) -> Option<u64> {
        if let Some((count, position, cursor)) = self.multi_click.get() {
            let run = self.run_at(position, None, file)?;
            let line = &run.document.lines(run.side)[run.line_ix];
            let ix = run.display_range.start
                + run
                    .layout
                    .index_for_position(position)
                    .unwrap_or_else(|ix| ix);
            let range = if count >= 3 {
                0..line.display().len()
            } else {
                word_range(line.display(), ix)?
            };
            self.multi_click
                .set((!cursor).then_some((count, position, true)));
            return Some(encode_key(
                run.file,
                run.side,
                line.source().start
                    + line.source_offset(if cursor { range.end } else { range.start }),
            ));
        }
        let point = point + self.bounds.origin + self.scroll_offset;
        let run = self.run_at(point, side, file)?;
        let ix =
            run.display_range.start + run.layout.index_for_position(point).unwrap_or_else(|ix| ix);
        let line = &run.document.lines(run.side)[run.line_ix];
        Some(encode_key(
            run.file,
            run.side,
            line.source().start + line.source_offset(ix),
        ))
    }

    fn run_at(&self, point: Point<Pixels>, side: Option<DiffSide>, file: Option<usize>) -> Option<&CodeRun> {
        let side = side.or_else(|| {
            self.runs
                .iter()
                .filter(|run| !run.visible_bounds.is_empty() && file.is_none_or(|f| f == run.file))
                .min_by(|a, b| {
                    distance_to_bounds(point, a.visible_bounds)
                        .total_cmp(&distance_to_bounds(point, b.visible_bounds))
                })
                .map(|run| run.side)
        })?;
        self.runs
            .iter()
            .filter(|run| run.side == side && file.is_none_or(|f| f == run.file))
            .min_by(|a, b| {
                line_distance(point, a.bounds)
                    .cmp(&line_distance(point, b.bounds))
                    .then_with(|| {
                        distance_to_bounds(point, a.bounds)
                            .total_cmp(&distance_to_bounds(point, b.bounds))
                    })
            })
    }
}

/// Prefer the half-open line band before proximity. Multi-click endpoints are
/// positioned at the line top, shared with the preceding line's bottom.
pub(super) fn line_distance(point: Point<Pixels>, bounds: Bounds<Pixels>) -> (bool, u32) {
    let outside = point.y < bounds.top() || point.y >= bounds.bottom();
    let distance: f32 = (point.y - bounds.center().y).abs().into();
    (outside, distance.to_bits())
}

pub(super) fn clip_highlight(range: Range<usize>, chunk: &Range<usize>) -> Option<Range<usize>> {
    let start = range.start.max(chunk.start);
    let end = range.end.min(chunk.end);
    (start < end).then(|| start - chunk.start..end - chunk.start)
}

/// Match Base's bounded native word selection across rendering chunks.
/// Classification and the 128-character flank bound follow text_boundary.rs.
pub(super) fn word_range(text: &str, mut offset: usize) -> Option<Range<usize>> {
    offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let character = text[offset..].chars().next()?;
    let class = |character: char| {
        if character == '_'
            || character.is_ascii_alphanumeric()
            || matches!(character, '\u{00C0}'..='\u{024F}' | '\u{0400}'..='\u{04FF}' | '\u{1E00}'..='\u{1EFF}' | '\u{0300}'..='\u{036F}')
        {
            1
        } else if matches!(character, '\n' | '\r') {
            0
        } else if character.is_whitespace() {
            2
        } else {
            0
        }
    };
    let kind = class(character);
    let connects = |character: &char| kind != 0 && class(*character) == kind;
    let start = text[..offset]
        .chars()
        .rev()
        .take(128)
        .take_while(connects)
        .fold(offset, |offset, character| offset - character.len_utf8());
    let end = offset + character.len_utf8();
    let end = text[end..]
        .chars()
        .take(128)
        .take_while(connects)
        .fold(end, |offset, character| offset + character.len_utf8());
    Some(start..end)
}

fn distance_to_bounds(point: Point<Pixels>, bounds: Bounds<Pixels>) -> f32 {
    let dx: f32 = (bounds.left() - point.x)
        .max(point.x - bounds.right())
        .max(gpui::px(0.))
        .into();
    let dy: f32 = (bounds.top() - point.y)
        .max(point.y - bounds.bottom())
        .max(gpui::px(0.))
        .into();
    dx * dx + dy * dy
}

/// The native text selection as a source range within one side of one file.
pub(crate) fn selected_source_range(
    selection: &TextSelectionHandle,
    cx: &App,
) -> Option<(usize, DiffSide, Range<usize>)> {
    let snapshot = selection.snapshot(cx)?;
    let entity = Some(selection.entity_id());
    if snapshot.anchor().entity_id() != entity || snapshot.cursor().entity_id() != entity {
        return None;
    }
    let (file, side, anchor) = decode_key(snapshot.anchor().content_key()?.value());
    let (other_file, other_side, cursor) = decode_key(snapshot.cursor().content_key()?.value());
    (file == other_file && side == other_side && anchor != cursor).then_some((
        file,
        side,
        anchor.min(cursor)..anchor.max(cursor),
    ))
}

pub(crate) struct CodeText {
    id: ElementId,
    document: DiffFile,
    file: usize,
    side: DiffSide,
    line_ix: usize,
    range: Range<usize>,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    selection: TextSelectionHandle,
    geometry: Rc<RefCell<SelectionGeometry>>,
    styled_text: Option<StyledText>,
}

impl CodeText {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ElementId,
        document: DiffFile,
        file: usize,
        side: DiffSide,
        line_ix: usize,
        range: Range<usize>,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
        selection: TextSelectionHandle,
        geometry: Rc<RefCell<SelectionGeometry>>,
    ) -> Self {
        Self {
            id,
            document,
            file,
            side,
            line_ix,
            range,
            highlights,
            selection,
            geometry,
            styled_text: None,
        }
    }
}

impl IntoElement for CodeText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for CodeText {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let line = &self.document.lines(self.side)[self.line_ix];
        let mut layers = self.highlights.clone();
        if let Some((file, side, range)) = selected_source_range(&self.selection, cx)
            && file == self.file
            && side == self.side
        {
            let start = range.start.max(line.source().start);
            let end = range.end.min(line.content_end());
            if start < end {
                layers.push((
                    line.display_range(start - line.source().start..end - line.source().start),
                    HighlightStyle {
                        background_color: Some(crate::ActiveTheme::theme(cx).selection),
                        ..Default::default()
                    },
                ));
            }
        }
        let layers = layers
            .into_iter()
            .filter_map(|(range, style)| {
                clip_highlight(range, &self.range).map(|range| (range, style))
            })
            .collect();
        let text = line.chunk_text(&self.range);
        self.styled_text =
            Some(StyledText::new(text).with_highlights(merge_highlights(self.range.len(), layers)));
        self.styled_text
            .as_mut()
            .unwrap()
            .request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let text = self.styled_text.as_mut().unwrap();
        text.prepaint(id, inspector, bounds, &mut (), window, cx);
        self.geometry.borrow_mut().runs.push(CodeRun {
            file: self.file,
            side: self.side,
            line_ix: self.line_ix,
            display_range: self.range.clone(),
            layout: text.layout().clone(),
            bounds,
            document: self.document.clone(),
            visible_bounds: bounds.intersect(&window.content_mask().bounds),
        });
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.styled_text.as_mut().unwrap().paint(
            id,
            inspector,
            bounds,
            &mut (),
            &mut (),
            window,
            cx,
        );
    }
}

/// Later layers override only their specified fields, preserving syntax color
/// when inline and selection backgrounds overlap it.
pub(crate) fn merge_highlights(
    len: usize,
    layers: Vec<(Range<usize>, HighlightStyle)>,
) -> Vec<(Range<usize>, HighlightStyle)> {
    // Sweep range edges while retaining precedence separately for each field.
    // Syntax token counts can grow with a line; never rescan all layers per edge.
    let mut events = Vec::with_capacity(layers.len() * 2 + 2);
    for (ix, (range, _)) in layers.iter().enumerate() {
        let start = range.start.min(len);
        let end = range.end.min(len);
        if start < end {
            events.push((start, ix, true));
            events.push((end, ix, false));
        }
    }
    events.sort_unstable();
    let mut active: [BTreeSet<usize>; 7] = Default::default();
    let mut output = Vec::with_capacity(events.len() + 1);
    let mut position = 0;
    let mut event_ix = 0;
    while position < len {
        while event_ix < events.len() && events[event_ix].0 == position {
            let (_, ix, entering) = events[event_ix];
            let style = &layers[ix].1;
            let specified = [
                style.color.is_some(),
                style.background_color.is_some(),
                style.font_weight.is_some(),
                style.font_style.is_some(),
                style.underline.is_some(),
                style.strikethrough.is_some(),
                style.fade_out.is_some(),
            ];
            for (field, specified) in specified.into_iter().enumerate() {
                if specified {
                    if entering {
                        active[field].insert(ix);
                    } else {
                        active[field].remove(&ix);
                    }
                }
            }
            event_ix += 1;
        }
        let end = events.get(event_ix).map_or(len, |event| event.0);
        let mut style = HighlightStyle::default();
        macro_rules! resolve {
            ($field:ident, $ix:expr) => {
                if let Some(ix) = active[$ix].last() {
                    style.$field = layers[*ix].1.$field;
                }
            };
        }
        resolve!(color, 0);
        resolve!(background_color, 1);
        resolve!(font_weight, 2);
        resolve!(font_style, 3);
        resolve!(underline, 4);
        resolve!(strikethrough, 5);
        resolve!(fade_out, 6);
        output.push((position..end, style));
        position = end;
    }
    output
}

pub(crate) struct SelectionLayer {
    child: AnyElement,
    scroll: ListState,
    horizontal: HorizontalScroll,
    selection: TextSelectionHandle,
    geometry: Rc<RefCell<SelectionGeometry>>,
}

impl SelectionLayer {
    pub fn new(
        child: AnyElement,
        scroll: ListState,
        horizontal: HorizontalScroll,
        selection: TextSelectionHandle,
        geometry: Rc<RefCell<SelectionGeometry>>,
    ) -> Self {
        Self {
            child,
            scroll,
            horizontal,
            selection,
            geometry,
        }
    }
}

impl IntoElement for SelectionLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for SelectionLayer {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;
    fn id(&self) -> Option<ElementId> {
        Some(ElementId::NamedInteger(
            "diff-selection".into(),
            self.selection.entity_id().as_u64(),
        ))
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        self.horizontal.set_viewport(bounds);
        {
            let mut geometry = self.geometry.borrow_mut();
            geometry.clear();
            geometry.bounds = bounds;
        }
        // This participant's hitbox is behind its child disclosures and gutters.
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        self.child.prepaint(window, cx);
        let mut offset = ScrollbarHandle::offset(&self.scroll);
        offset.x = self.horizontal.offset().x;
        self.geometry.borrow_mut().scroll_offset = offset;
        let text_bounds = self
            .geometry
            .borrow()
            .runs
            .iter()
            .filter(|run| !run.visible_bounds.is_empty())
            .map(|run| run.visible_bounds)
            .collect();
        self.selection.register(
            TextSelectionRegistration::new(hitbox.clone(), bounds)
                .with_scroll_offset(offset)
                .with_text_bounds(text_bounds)
                .with_rendered_element(&self.selection, window, cx),
            window,
            cx,
        );
        hitbox
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let runs = self
            .geometry
            .borrow()
            .runs
            .iter()
            .map(|run| {
                let text = run.document.lines(run.side)[run.line_ix].chunk_text(&run.display_range);
                TextSelectionRun::new(text, run.layout.clone(), run.bounds).with_document_order(
                    encode_key(
                        run.file,
                        run.side,
                        run.document.lines(run.side)[run.line_ix].source().start
                            + run.document.lines(run.side)[run.line_ix]
                                .source_offset(run.display_range.start),
                    ),
                )
            })
            .collect::<Vec<_>>();
        self.selection.update_runs(&runs, cx);
        window.on_mouse_event({
            let geometry = self.geometry.clone();
            move |event: &MouseDownEvent, phase, _, _| {
                if phase.capture() && event.button == MouseButton::Left {
                    geometry
                        .borrow()
                        .multi_click
                        .set((event.click_count >= 2).then_some((
                            event.click_count,
                            event.position,
                            false,
                        )));
                }
            }
        });
        window.on_mouse_event({
            let geometry = self.geometry.clone();
            move |_: &MouseMoveEvent, phase, _, _| {
                if phase.capture() {
                    geometry.borrow().multi_click.set(None);
                }
            }
        });
        window.on_mouse_event({
            let geometry = self.geometry.clone();
            move |_: &MouseUpEvent, phase, _, _| {
                if phase.capture() {
                    geometry.borrow().multi_click.set(None);
                }
            }
        });
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    use gpui::{AppContext as _, Context, Entity, ParentElement as _, Render,
        Styled as _, TestAppContext, div, point, size};
    use crate::diff::{Diff, DiffMode, DiffState, DiffChangeIndicator, document::fixture};

    struct Probe { state: Entity<DiffState> }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(gpui_base::TextSelectionLayer).child(Diff::new(&self.state).size_full()
                .header_visible(false).soft_wrap(false)
                .change_indicator(DiffChangeIndicator::None))
        }
    }

    #[gpui::test]
    fn split_long_lines_keep_separate_viewports_and_selection_sides(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let (view, cx) = cx.add_window_view(|_, cx| {
            let old = format!("{}\nold\n", "left ".repeat(100));
            let new = format!("{}\nnew\n", "right ".repeat(100));
            Probe { state: cx.new(|cx| DiffState::new(
                [fixture::modified("review.txt", &old, &new)], cx).with_mode(DiffMode::Split)) }
        });
        cx.simulate_resize(size(px(800.), px(400.)));
        // First paint measures the viewport; the following frame lays out rows.
        for _ in 0..3 {
            cx.update(|window, cx| {
                view.update(cx, |_, cx| cx.notify());
                window.draw(cx).clear(cx);
            });
        }
        let (start, end) = cx.update(|_, cx| {
            let state = view.read(cx).state.read(cx);
            let geometry = state.geometry().borrow();
            let middle = geometry.bounds.center().x;
            let left = geometry.runs.iter().find(|run|
                run.side == DiffSide::Original && !run.visible_bounds.is_empty()).unwrap();
            let right = geometry.runs.iter().find(|run|
                run.side == DiffSide::Modified && !run.visible_bounds.is_empty()).unwrap();
            assert!(left.bounds.right() > middle, "fixture must overflow the left viewport");
            assert!(left.visible_bounds.right() <= middle);
            assert!(right.visible_bounds.left() >= middle);
            assert!(right.visible_bounds.right() <= geometry.bounds.right());
            let hit = right.visible_bounds.origin + point(px(2.), px(2.));
            let key = geometry.key_at(hit - geometry.bounds.origin - geometry.scroll_offset, None).unwrap();
            assert_eq!(decode_key(key).1, DiffSide::Modified,
                "a clipped left text run must not steal a right-column selection");
            (right.layout.position_for_index(0).unwrap() + point(px(0.1), px(2.)),
             right.layout.position_for_index(5).unwrap() + point(px(0.1), px(2.)))
        });
        cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert_eq!(view.read(cx).state.read(cx).selected_text(cx), "right");
        });
    }
}
