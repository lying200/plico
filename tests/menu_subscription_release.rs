#![cfg(feature = "desktop-tests")]
extern crate gpui_kit as gpui;

use gpui::AppContext as _;
use std::{cell::Cell, rc::Rc};

#[gpui::test]
fn closed_menu_releases_observer_without_another_owner_notification(cx: &mut gpui::TestAppContext) {
    struct Lifetime(Rc<Cell<bool>>);
    impl Drop for Lifetime {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    cx.update(gpui::component::init);
    let released = Rc::new(Cell::new(false));
    let owner = cx.update(|cx| cx.new(|_| ()));
    let mut menu = None;
    cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let lifetime = Lifetime(released.clone());
            menu = Some(gpui::component::menu::PopupMenu::build(window, cx, |menu, _, cx| {
                let subscription = cx.observe(&owner, move |_, _, _| {
                    let _ = &lifetime;
                });
                menu.with_subscription(subscription)
            }));
            cx.new(|_| gpui::Empty)
        }).unwrap()
    });
    assert!(!released.get());
    cx.update(|_| drop(menu.take()));
    cx.run_until_parked();
    assert!(released.get(), "closed popup retained its observer callback");
    // Keep the observed owner alive: its next notification must not be needed
    // to unregister the closed menu's callback.
    cx.update(|cx| { owner.read(cx); });
}
