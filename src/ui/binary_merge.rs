//! BinaryMergeTool-style side selection, sharing the conflict mutation boundary.
use super::*;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use plico::{
    conflicts::{Resolution, Side},
    merge_result::Prepared,
};

pub(super) fn show(
    owner: WeakEntity<Review>,
    repository: PathBuf,
    prepared: Arc<Prepared>,
    window: &mut Window,
    cx: &mut App,
) {
    window.open_dialog(cx, move |d, w, _| {
        let actions = [
            ("binary-accept-left", "Accept Left", Side::Yours),
            ("binary-accept-right", "Accept Right", Side::Theirs),
        ]
        .map(|(id, label, side)| {
            let owner = owner.clone();
            let repository = repository.clone();
            let snapshot = Arc::new(prepared.snapshot().clone());
            let file = prepared.file().clone();
            Button::new(id).label(t(label)).on_click(move |_, w, cx| {
                let snapshot = snapshot.clone();
                let file = file.clone();
                let _ = owner.update(cx, |s, cx| {
                    if s.repo.path != repository {
                        return;
                    }
                    w.close_dialog(cx);
                    w.close_dialog(cx);
                    s.with_saved_documents(w, cx, move |s, w, cx| {
                        s.resolve_conflict(snapshot, file, Resolution::Accept(side), w, cx);
                    });
                });
            })
        });
        d.title(t("Merge Conflicts"))
            .w((w.viewport_size().width - px(40.)).max(px(600.)))
            .centered(true)
            .overlay_closable(false)
            .child(
                div()
                    .truncate()
                    .child(prepared.file().path.to_string_lossy().into_owned()),
            )
            .child(
                h_flex()
                    .w_full()
                    .h((w.viewport_size().height - px(210.)).max(px(160.)))
                    .children(
                        ["Yours", "Merge Result", "Theirs"]
                            .into_iter()
                            .map(|label| {
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(div().px_2().py_2().child(t(label)))
                                    .child(
                                        div()
                                            .flex_1()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .px_2()
                                            .child(
                                                div()
                                                    .whitespace_normal()
                                                    .child(t("Cannot show diff for a binary file")),
                                            ),
                                    )
                            }),
                    ),
            )
            .footer(
                DialogFooter::new().child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .children(actions)
                        .child(div().flex_1())
                        .child(DialogClose::new().trigger(|b| b.label(t("Cancel")))),
                ),
            )
    });
}
