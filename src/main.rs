#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod assets;
mod auth_dialog;
mod changes_refresh;
mod clone_dialog;
mod commit_scope;
mod compare_view;
mod containing_branches;
mod date_dialog;
mod define_remote_dialog;
mod dialog_style;
mod diff_path_title;
mod diff_picker;
mod diff_revert;
mod diff_scroll;
mod diff_settings;
mod diff_splitter;
mod diff_tab;
mod editable_diff;
mod editing;
mod editor_dialog;
mod error_ui;
mod history_view;
mod identity_dialog;
mod local_view;
mod path_chooser;
mod paths_menu;
mod project_update_dialog;
mod project_view;
mod pull_dialog;
mod push_dialog;
mod remotes_dialog;
mod reset_dialog;
mod revision_input;
mod reword_dialog;
mod squash_dialog;
mod startup_report;
mod stash_view;
mod ui;
mod ui_actions;
mod ui_font;
mod ui_graph;
mod ui_style;
mod unified_editing;
mod update_dialog;
mod update_settings;
mod user_menu;
use gpui_kit::{AppContext as _, WindowBounds, WindowOptions, component::TitleBar, px, size};
fn main() {
    let started = std::time::Instant::now();
    let mut report_path = None;
    plico::i18n::apply(plico::layout_preferences::load_language());
    let mut args = std::env::args_os().skip(1);
    let mut path = std::path::PathBuf::new();
    let mut limit = plico::git::HISTORY_PAGE;
    while let Some(arg) = args.next() {
        if arg == "--startup-report" {
            report_path = Some(std::path::PathBuf::from(
                args.next()
                    .expect("--startup-report requires a new output file"),
            ));
        } else if arg == "--history-limit" {
            limit = args
                .next()
                .and_then(|s| s.to_str().and_then(|s| s.parse::<usize>().ok()))
                .filter(|n| *n > 0 && *n <= plico::git::HISTORY_MAX)
                .expect("--history-limit requires a number from 1 to 10000");
        } else {
            path = arg.into();
        }
    }
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            gpui_kit::component::set_locale(if plico::i18n::is_chinese() {
                "zh-CN"
            } else {
                "en"
            });
            cx.text_system()
                .add_fonts(vec![
                    std::borrow::Cow::Borrowed(
                        include_bytes!("../assets/fonts/Inter-Regular.otf").as_slice(),
                    ),
                    std::borrow::Cow::Borrowed(
                        include_bytes!("../assets/fonts/Inter-SemiBold.otf").as_slice(),
                    ),
                    std::borrow::Cow::Borrowed(
                        include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice(),
                    ),
                    std::borrow::Cow::Borrowed(
                        include_bytes!("../assets/fonts/JetBrainsMono-SemiBold.ttf").as_slice(),
                    ),
                ])
                .expect("Load bundled UI fonts");
            ui::init(cx);
            push_dialog::init(cx);
            stash_view::init(cx);
            update_dialog::init(cx);
            let report = report_path
                .map(|path| std::sync::Arc::new(startup_report::Probe::new(path, started)));
            open_project_window_with_report(path, limit, report, cx).expect("Open Plico window");
            cx.activate(true);
        });
}

fn open_project_window(
    path: std::path::PathBuf,
    limit: usize,
    cx: &mut gpui_kit::App,
) -> gpui_kit::Result<()> {
    open_project_window_with_report(path, limit, None, cx)
}
fn open_project_window_with_report(
    path: std::path::PathBuf,
    limit: usize,
    report: Option<std::sync::Arc<startup_report::Probe>>,
    cx: &mut gpui_kit::App,
) -> gpui_kit::Result<()> {
    open_project_window_at(path, limit, report, None, cx)
}
fn open_project_window_at(
    path: std::path::PathBuf,
    limit: usize,
    report: Option<std::sync::Arc<startup_report::Probe>>,
    bounds: Option<gpui_kit::Bounds<gpui_kit::Pixels>>,
    cx: &mut gpui_kit::App,
) -> gpui_kit::Result<()> {
    let welcome = path.as_os_str().is_empty();
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(bounds.map(WindowBounds::Windowed).unwrap_or_else(|| {
                WindowBounds::centered(
                    if welcome {
                        size(px(800.), px(520.))
                    } else {
                        size(px(1440.), px(900.))
                    },
                    cx,
                )
            })),
            window_min_size: Some(size(px(720.), px(480.))),
            app_id: Some("Plico".into()),
            ..TitleBar::window_options()
        },
        cx,
        |w, cx| cx.new(|cx| ui::Review::new(path, limit, report, w, cx)),
    )?;
    cx.activate(true);
    Ok(())
}

mod source_changes;

mod rollback_dialog;
