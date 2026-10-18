//! A visual row whose offsets remain in source UTF-8 bytes.
use gpui::{App, Pixels, Point, ShapedLine, SharedString, TextAlign, Window, point, px};
use std::ops::Range;

pub(crate) struct InlineFragment {
    pub(crate) range: Range<usize>,
    pub(crate) x: Pixels,
    pub(crate) width: Pixels,
    pub(crate) text: Option<ShapedLine>,
}

pub(crate) struct InputLine {
    pub(crate) len: usize,
    pub(crate) width: Pixels,
    pub(crate) text: SharedString,
    content: Content,
}
// Keep the ordinary shaped row inline: boxing it would add an allocation to
// every existing plain-text row. Only token rows allocate fragment storage.
#[allow(clippy::large_enum_variant)]
enum Content {
    Text(ShapedLine),
    Inline(Vec<InlineFragment>),
}
impl From<ShapedLine> for InputLine {
    fn from(line: ShapedLine) -> Self {
        Self {
            len: line.len,
            width: line.width,
            text: line.text.clone(),
            content: Content::Text(line),
        }
    }
}
impl InputLine {
    /// Shape tabs as source-byte-preserving gaps, never as font glyphs.
    pub(crate) fn with_tabs(text: SharedString, font_size: Pixels, runs: &[gpui::TextRun], tab_size: usize, text_system: &gpui::WindowTextSystem) -> Self {
        if !text.contains('\t') {
            return text_system.shape_line(text, font_size, runs, None).into();
        }
        let mut space_run = runs[0].clone();
        space_run.len = 1;
        let stop = text_system.shape_line(" ".into(), font_size, &[space_run], None).width * tab_size.max(1) as f32;
        let mut fragments = Vec::new();
        let mut start = 0;
        let mut x = px(0.);
        for end in text.match_indices('\t').map(|(i,_)|i).chain(std::iter::once(text.len())) {
            if end > start {
                let mut offset = 0;
                let part_runs: Vec<_> = runs.iter().filter_map(|run| {
                    let a = offset.max(start); let b = (offset + run.len).min(end);
                    offset += run.len;
                    (a < b).then(|| {let mut r=run.clone();r.len=b-a;r})
                }).collect();
                let line=text_system.shape_line(text[start..end].to_owned().into(),font_size,&part_runs,None);
                let width=line.width;
                fragments.push(InlineFragment {range:start..end,x,width,text:Some(line)});
                x += width;
            }
            if end < text.len() {
                let width=stop * ((x / stop).floor() + 1.) - x;
                fragments.push(InlineFragment {range:end..end+1,x,width,text:None});
                x += width;
            }
            start=end+1;
        }
        Self::inline(text,fragments)
    }
    pub(crate) fn inline(text: SharedString, fragments: Vec<InlineFragment>) -> Self {
        let width = fragments.last().map_or(px(0.), |f| f.x + f.width);
        Self {
            len: text.len(),
            text,
            width,
            content: Content::Inline(fragments),
        }
    }
    pub(crate) fn x_for_index(&self, ix: usize) -> Pixels {
        match &self.content {
            Content::Text(line) => line.x_for_index(ix),
            Content::Inline(fragments) => {
                for f in fragments {
                    if ix < f.range.end {
                        return f.x
                            + f.text.as_ref().map_or(
                                if ix <= f.range.start { px(0.) } else { f.width },
                                |line| line.x_for_index(ix.saturating_sub(f.range.start)),
                            );
                    }
                }
                self.width
            }
        }
    }
    pub(crate) fn closest_index_for_x(&self, x: Pixels) -> usize {
        match &self.content {
            Content::Text(line) => line.closest_index_for_x(x),
            Content::Inline(fragments) => {
                for f in fragments {
                    if x <= f.x + f.width {
                        return f.text.as_ref().map_or(
                            if x - f.x < f.width / 2. {
                                f.range.start
                            } else {
                                f.range.end
                            },
                            |line| f.range.start + line.closest_index_for_x(x - f.x),
                        );
                    }
                }
                self.len
            }
        }
    }
    pub(crate) fn index_for_x(&self, x: Pixels) -> Option<usize> {
        match &self.content {
            Content::Text(line) => line.index_for_x(x),
            Content::Inline(_) => {
                (x >= px(0.) && x <= self.width).then(|| self.closest_index_for_x(x))
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint(
        &self,
        pos: Point<Pixels>,
        height: Pixels,
        align: TextAlign,
        width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.paint_pass(pos, height, align, width, false, window, cx);
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_background(
        &self,
        pos: Point<Pixels>,
        height: Pixels,
        align: TextAlign,
        width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.paint_pass(pos, height, align, width, true, window, cx);
    }
    #[allow(clippy::too_many_arguments)]
    fn paint_pass(
        &self,
        pos: Point<Pixels>,
        height: Pixels,
        align: TextAlign,
        width: Option<Pixels>,
        background: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let paint = |line: &ShapedLine, pos, align, width, window: &mut Window, cx: &mut App| {
            if background {
                let _ = line.paint_background(pos, height, align, width, window, cx);
            } else {
                let _ = line.paint(pos, height, align, width, window, cx);
            }
        };
        match &self.content {
            Content::Text(line) => paint(line, pos, align, width, window, cx),
            Content::Inline(fragments) => {
                let remaining = (width.unwrap_or(self.width) - self.width).max(px(0.));
                let offset = match align {
                    TextAlign::Right => remaining,
                    TextAlign::Center => remaining / 2.,
                    _ => px(0.),
                };
                for f in fragments {
                    if let Some(line) = &f.text {
                        paint(
                            line,
                            pos + point(offset + f.x, px(0.)),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(all(test,target_os="linux"))]
mod tab_tests {
    use super::*;
    #[test]
    fn tab_shaping_preserves_source_offsets_and_tab_stops() {
        let platform=gpui_platform::current_platform(true);
        let cx=gpui::TestAppContext::build_with_text_system(gpui::TestDispatcher::new(0),None,platform.text_system());
        cx.update(|cx| {
            let system=gpui::WindowTextSystem::new(cx.text_system().clone());
            let run=gpui::TextRun {len:1,font:gpui::font("monospace"),color:gpui::black(),background_color:None,underline:None,strikethrough:None};
            let space=system.shape_line(" ".into(),px(14.),&[run.clone()],None).width;
            assert!(space>px(0.));
            for text in ["\t\ttry {", "a\tb", "中文\tvalue", "\t"] {
                let mut r=run.clone();r.len=text.len();
                let line=InputLine::with_tabs(text.to_owned().into(),px(14.),&[r],4,&system);
                assert_eq!(line.text.as_ref(),text);
                assert_eq!(line.len,text.len());
                for (i,_) in text.match_indices('\t') {
                    let left=line.x_for_index(i);let right=line.x_for_index(i+1);
                    assert!(right>left);
                    assert!((right/(space*4.)-(right/(space*4.)).round()).abs()<0.001);
                    assert_eq!(line.closest_index_for_x(left+(right-left)*0.25),i);
                    assert_eq!(line.closest_index_for_x(left+(right-left)*0.75),i+1);
                }
            }
        });
    }
}
