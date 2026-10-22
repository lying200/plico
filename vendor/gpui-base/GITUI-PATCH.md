# Local source patch

Source: gpui-base 0.7.3 from crates.io, original Cargo metadata and licenses retained.

Popup and Popover add opt-in `side_start` positioning: lateral popup corners align to the measured trigger top edge. Default anchor placement and viewport clamping remain unchanged. Used by the Rebased-style branch submenu through gpui-component. No added retained geometry cache.

DialogClose handles a focused descendant's Confirm (Enter) by dispatching Cancel from its own anchor and stopping propagation. Otherwise the dialog host's Enter binding intercepts native button activation and incorrectly invokes OK. Pointer/Space activation and other dialog default actions are unchanged.

- Source gutter markers read the unscrolled gutter edge published alongside source geometry during prepaint. Reading paint-time input_bounds/last_layout made sidebar layout changes lag by one frame; marker drawing and hitboxes now use the same current-frame geometry. Regression: `source_gutter_moves_with_sidebar_in_same_frame`.

- Publish wheel and already-laid-out explicit viewport changes before paint so paired Diff editors update together. Clear obsolete deferred peer scrolls on new wheel input; retain paint-time clamping/publication for initial layout and geometry corrections. Same-frame paired-editor regression lives in Plico's `editable_diff` tests; wheel publication/deduplication is covered in input state tests.

- Optional owner metadata snapshots now share text UndoManager transactions, including coalesced typing and IME. Used for merge block ranges and accept/ignore state; metadata-only decisions create atomic undo entries. Replay restores metadata after text and publishes one Change event, so decorations never observe half-restored decisions. Ordinary editors do not install callbacks. No source-file copies are kept in metadata snapshots; the existing 1000-transaction cap remains. Integration coverage: `ui::text_merge::history::tests::decisions_typing_and_ime_share_undo_and_redo`.
