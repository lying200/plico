# Local source patch

Source: gpui-base 0.7.3 from crates.io, original Cargo metadata and licenses retained.

Popup and Popover add opt-in `side_start` positioning: lateral popup corners align to the measured trigger top edge. Default anchor placement and viewport clamping remain unchanged. Used by the Rebased-style branch submenu through gpui-component. No added retained geometry cache.

DialogClose handles a focused descendant's Confirm (Enter) by dispatching Cancel from its own anchor and stopping propagation. Otherwise the dialog host's Enter binding intercepts native button activation and incorrectly invokes OK. Pointer/Space activation and other dialog default actions are unchanged.
