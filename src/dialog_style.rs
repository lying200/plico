//! Windows DialogWrapper default action order and content-sized button group.
//! Reference: Rebased 4aec352, DialogWrapper.createActions and JBUI.Button.minimumSize.
use gpui_kit::component::{
    button::Button,
    dialog::{DialogAction, DialogClose, DialogFooter},
};
use gpui_kit::{ParentElement as _, Styled as _, px};
use plico::i18n::t;

/// For standard OK/Cancel dialogs only; custom risk decisions keep their own order.
/// 72 is JBUI's logical minimum fallback, not a claim about installed theme pixels.
pub fn confirmation(action: Button) -> DialogFooter {
    DialogFooter::new()
        .child(DialogAction::new().child(action.min_w(px(72.))))
        .child(DialogClose::new().trigger(|button| button.label(t("Cancel")).min_w(px(72.))))
}
