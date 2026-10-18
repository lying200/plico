//! Native validation harness for the production editor dialog; not packaged.
#[path = "../src/assets.rs"] mod assets;
#[path = "../src/editor_dialog.rs"] mod editor_dialog;
#[path = "../src/dialog_style.rs"] mod dialog_style;
use gpui_kit::*;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use plico::{auth_request::{Kind, Requests}, editor::Document, git::Cancel};
struct Preview { document: Option<Document> }
impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_8().bg(rgb(0x191a1c)).child(Button::new("open-editor").primary().label("Open editor")
            .on_click(cx.listener(|s, _, w, cx| {
                if let Some(document) = s.document.take() { editor_dialog::show(document, w, cx); }
            })))
    }
}
fn main() {
    let mut args = std::env::args_os().skip(1);
    let root = std::path::PathBuf::from(args.next().expect("git directory"));
    let chinese = args.next().is_some_and(|s| s == "zh");
    plico::i18n::apply(if chinese { plico::i18n::Language::Chinese } else { plico::i18n::Language::English });
    let requests = Requests::new(Cancel::default());
    let worker_requests = requests.clone(); let worker_root = root.clone();
    std::thread::spawn(move || {
        let result = worker_requests.ask(Kind::CommitEditor, worker_root.join("COMMIT_EDITMSG").to_str().unwrap().into());
        std::fs::write(worker_root.join("editor-result.txt"), result.unwrap_or_else(|_| "cancelled".into())).unwrap();
    });
    let pending = loop {
        if let Some(pending) = requests.take_pending() { break pending; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let document = Document::open(pending, &root).unwrap();
    gpui_kit::application().with_assets(assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        cx.text_system().add_fonts(vec![std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Regular.otf").as_slice())]).unwrap();
        gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::centered(size(px(900.), px(650.)), cx)), ..Default::default() },
            cx, |window, cx| {
                use gpui_kit::component::{Theme, ThemeMode};
                Theme::change(ThemeMode::Dark, Some(window), cx);
                Theme::update(cx, |t| {
                    t.background = rgb(0x191a1c).into(); t.foreground = rgb(0xd1d3d9).into();
                    t.border = rgb(0x26282c).into(); t.popover = rgb(0x26282c).into();
                    t.popover_foreground = rgb(0xd1d3d9).into(); t.primary = rgb(0x3574f0).into();
                    t.button_primary = rgb(0x3574f0).into(); t.button_primary_foreground = rgb(0xffffff).into();
                    t.ring = rgb(0x3574f0).into(); t.font_family = "Inter".into(); t.font_size = px(13.);
                });
                cx.new(|_| Preview { document: Some(document) })
            }).unwrap();
        cx.activate(true);
    });
}
