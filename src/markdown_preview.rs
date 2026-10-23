//! Markdown presentation of the existing editor document; never a second editable buffer.
use gpui_kit::{
    base::{TextView, TextViewState, TextViewStyle},
    component::{
        ActiveTheme as _,
        input::{EditorState, InputEvent, Position, RopeExt as _},
    },
    *,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    Source,
    #[default]
    Split,
    Preview,
}
impl Mode {
    pub fn load() -> Self {
        match plico::layout_preferences::load_markdown_layout().as_str() {
            "source" => Self::Source,
            "preview" => Self::Preview,
            _ => Self::Split,
        }
    }
    pub fn save(self) {
        let value = match self {
            Self::Source => "source",
            Self::Split => "split",
            Self::Preview => "preview",
        };
        if let Err(error) = plico::layout_preferences::save_markdown_layout(value) {
            eprintln!("Could not save Markdown layout: {error}");
        }
    }
    pub const ALL: [Self; 3] = [Self::Source, Self::Split, Self::Preview];
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Editor only",
            Self::Split => "Editor and Preview",
            Self::Preview => "Preview only",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Source => "jb/layoutEditorOnly_dark.svg",
            Self::Split => "jb/layoutEditorPreview_dark.svg",
            Self::Preview => "jb/layoutPreviewOnly_dark.svg",
        }
    }
}

pub fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("md") || s.eq_ignore_ascii_case("markdown"))
}

pub struct OpenLink(pub PathBuf);
impl EventEmitter<OpenLink> for MarkdownPreview {}

pub struct MarkdownPreview {
    state: Entity<TextViewState>,
    directory: PathBuf,
    _subscription: Subscription,
    top_line: usize,
    source: String,
    validated_source: Option<(usize, bool)>,
}
impl MarkdownPreview {
    pub fn new(editor: &Entity<EditorState>, path: PathBuf, cx: &mut Context<Self>) -> Self {
        let text = editor.read(cx).text().to_string();
        let state = cx.new(|cx| TextViewState::markdown(&text, cx).scrollable(true));
        let subscription = cx.subscribe(editor, |s, editor, event, cx| {
            if matches!(event, InputEvent::Change) {
                let text = editor.read(cx).text().to_string();
                s.refresh(&text, cx);
            } else if matches!(event, InputEvent::ScrollChanged { .. }) {
                let editor = editor.read(cx);
                if let Some(height) = editor.line_height().filter(|h| *h > px(0.)) {
                    let row = ((-editor.scroll_offset().y).max(px(0.)) / height) as usize;
                    let line = editor.source_line_at_row(row);
                    if s.top_line != line {
                        let rendered = s.state.read(cx).rendered_text();
                        let identity = rendered.source().as_ptr() as usize;
                        let valid = match s.validated_source {
                            Some((previous, valid)) if previous == identity => valid,
                            _ => {
                                let valid = rendered.source() == s.source;
                                s.validated_source = Some((identity, valid));
                                valid
                            }
                        };
                        // Background parsing may still describe the previous document.
                        if !valid {
                            return;
                        }
                        s.top_line = line;
                        let start = editor
                            .text()
                            .position_to_offset(&Position::new(line as u32, 0));
                        let end = editor
                            .text()
                            .position_to_offset(&Position::new(line.saturating_add(1) as u32, 0));
                        s.state.update(cx, |state, cx| {
                            if let Some(range) = state.rendered_text().range_for_source(start..end)
                            {
                                let _ = state.reveal_range(range.start..range.start, cx);
                            }
                        });
                    }
                }
            }
        });
        Self {
            state,
            top_line: 0,
            source: text,
            validated_source: None,
            directory: path.parent().unwrap_or(Path::new("")).to_path_buf(),
            _subscription: subscription,
        }
    }
    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).focus_handle().clone()
    }
    pub fn refresh(&mut self, text: &str, cx: &mut App) {
        self.source = text.to_owned();
        self.validated_source = None;
        self.state.update(cx, |state, cx| state.set_text(text, cx));
    }
}
impl Render for MarkdownPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let directory = self.directory.clone();
        let link_directory = directory.clone();
        let owner = cx.entity().downgrade();
        div()
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .px(px(24.))
            .py(px(24.))
            .font_family(cx.theme().font_family.clone())
            .text_size(px(14.))
            .child(
                TextView::new(&self.state)
                    .selectable(true)
                    .scrollable(true)
                    .style(
                        TextViewStyle::default()
                            .with_dark(true)
                            .with_foreground(rgb(0xbcbec4).into())
                            .with_muted_foreground(rgb(0x868a91).into())
                            .with_link(rgb(0x6daaf5).into())
                            .with_code_background(rgb(0x26282b).into())
                            .with_border(rgb(0x393b40).into())
                            .with_selection(rgb(0x2f4d7b).into()),
                    )
                    .image_source(move |uri| {
                        let value = uri.to_string();
                        match image_path(&directory, &value) {
                            Some(path) => ImageSource::from(path),
                            None if value.starts_with("https://")
                                || value.starts_with("http://") =>
                            {
                                ImageSource::from(uri.clone())
                            }
                            _ => ImageSource::from(directory.join(".plico-unavailable-image")),
                        }
                    })
                    .on_link_click(move |url, _, _, cx| {
                        // Never hand arbitrary executable/custom protocols from a repository to the OS.
                        if url.starts_with("https://")
                            || url.starts_with("http://")
                            || url.starts_with("mailto:")
                        {
                            cx.open_url(url);
                        } else if !url.starts_with('#') {
                            if let Some(path) = image_path(&link_directory, url) {
                                let _ = owner.update(cx, |_, cx| cx.emit(OpenLink(path)));
                            }
                        }
                    }),
            )
    }
}

fn image_path(directory: &Path, source: &str) -> Option<PathBuf> {
    // URL parsing decodes escaped spaces/non-ASCII paths and handles Windows drives.
    let base = url::Url::from_directory_path(directory).ok()?;
    let resolved = base.join(source).ok()?;
    (resolved.scheme() == "file")
        .then(|| resolved.to_file_path().ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn markdown_extensions_are_case_insensitive() {
        assert!(supported(Path::new("README.MD")));
        assert!(supported(Path::new("notes.markdown")));
        assert!(!supported(Path::new("notes.md.rs")));
    }
    #[::core::prelude::v1::test]
    fn relative_images_use_document_directory_and_decode_spaces() {
        let directory = std::env::temp_dir().join("plico-markdown").join("docs");
        assert_eq!(
            image_path(&directory, "images/hello%20world.png"),
            Some(directory.join("images/hello world.png"))
        );
        assert!(image_path(&directory, "https://example.com/a.png").is_none());
        assert!(image_path(&directory, "javascript:alert(1)").is_none());
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod interaction_tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;

    #[gpui::test]
    fn preview_tracks_draft_changes_and_programmatic_reload(cx: &mut TestAppContext) {
        cx.update(gpui::component::init);
        let mut handles = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx).default_value("# Original"));
            let preview = cx.new(|cx| {
                MarkdownPreview::new(&editor, std::env::temp_dir().join("README.md"), cx)
            });
            handles = Some((editor, preview.clone()));
            Root::new(preview, window, cx)
        });
        let (editor, preview) = handles.unwrap();
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_value("# Draft", window, cx);
                cx.emit(InputEvent::Change);
            });
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(
                preview
                    .read(cx)
                    .state
                    .read(cx)
                    .rendered_text()
                    .as_str()
                    .contains("Draft")
            );
            preview.update(cx, |s, cx| s.refresh("# Reloaded", cx));
            preview.read(cx).focus_handle(cx).focus(window, cx);
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(
                preview
                    .read(cx)
                    .state
                    .read(cx)
                    .rendered_text()
                    .as_str()
                    .contains("Reloaded")
            );
            assert_eq!(
                editor.read(cx).text().to_string(),
                "# Draft",
                "preview must never write into the source editor"
            );
        });
    }
}
