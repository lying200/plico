use plico::{editor::Document, i18n::t};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::component::{WindowExt as _, button::{Button, ButtonVariants as _},
    input::{Textarea, TextareaState}, v_flex};

pub struct Editor {
    document: Option<Document>,
    input: Entity<TextareaState>,
    error: bool,
    sequence: bool,
}
impl Editor {
    pub fn active(&self) -> bool { self.document.is_some() }
    fn new(document: Document, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // GitImplBase.loadFileAndShowInSimpleEditor ignores U+0001 comment lines
        // and trims leading whitespace. Keep raw original in Document for checks.
        let text = if document.sequence { document.text().to_owned() } else {
            document.text().split_inclusive('\n').filter(|line| !line.starts_with('\u{1}'))
                .collect::<String>().trim_start().to_owned()
        };
        let input = cx.new(|cx| {
            let mut state = TextareaState::new(window, cx);
            state.set_value(text, window, cx); state
        });
        let sequence = document.sequence;
        Self { document: Some(document), input, error: false, sequence }
    }
    fn confirm(&mut self, cx: &mut Context<Self>) -> bool {
        let text = self.input.read(cx).value();
        let Some(document) = self.document.as_mut() else { return true; };
        if document.save(&text).is_err() { self.error = true; cx.notify(); return false; }
        self.document.take();
        true
    }
}
impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3()
            .child(Textarea::new(&self.input).external_focus_ring(false).h(px(300.)).aria_label(t(if self.sequence { "Edit rebase todo" } else { "Commit message" })))
            .when(self.error, |v| v.child(div().text_color(rgb(0xf2a6a6))
                .child(t("Could not save. The file may have changed or the operation has ended."))))
            .capture_key_down(cx.listener(|s, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "enter" && event.keystroke.modifiers.control {
                    if s.confirm(cx) { window.close_dialog(cx); }
                    cx.stop_propagation();
                }
            }))
    }
}
pub fn show(document: Document, window: &mut Window, cx: &mut App) -> Entity<Editor> {
    let title = if document.sequence { "Edit rebase todo" } else { "Edit commit message" };
    let editor = cx.new(|cx| Editor::new(document, window, cx));
    let retained = editor.clone();
    window.open_dialog(cx, move |dialog, _, _| {
        let confirm = editor.clone(); let close = editor.clone();
        dialog.title(t(title)).w(px(640.)).centered(true).overlay_closable(false)
            .child(editor.clone())
            .footer(crate::dialog_style::confirmation(Button::new("editor-ok").primary().label(t("OK"))))
            .on_ok(move |_, _, cx| confirm.update(cx, |s, cx| s.confirm(cx)))
            .on_close(move |_, _, cx| { close.update(cx, |s, _| { s.document.take(); }); })
    });
    retained.read(cx).input.read(cx).focus_handle(cx).focus(window, cx);
    retained
}
