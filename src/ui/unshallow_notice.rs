use super::*;
use gpui_kit::component::notification::{Notification, NotificationDelivery};

#[derive(Default)]
pub(super) struct Startup {
    checked: bool,
    token: Cancel,
    task: Option<Task<()>>,
    subscription: Option<Subscription>,
}
impl Drop for Startup {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
pub(super) struct ShallowNotification;

impl Review {
    pub(super) fn watch_shallow_startup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let replacing_project = self.shallow_startup.subscription.is_some();
        self.shallow_startup = Startup::default();
        // Review is constructed before Root installs the notification host.
        // Only an existing project can have a notification to remove here.
        if replacing_project {
            window.remove_notification::<ShallowNotification>(cx);
        }
        self.shallow_startup.subscription = Some(cx.subscribe_in(
            &self.project_roots,
            window,
            |s, _, event: &project_roots::Changed, window, cx| {
                if matches!(event, project_roots::Changed::Ready { .. }) {
                    s.check_shallow_startup(window, cx);
                }
            },
        ));
        self.check_shallow_startup(window, cx);
    }

    fn check_shallow_startup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.shallow_startup.checked || self.welcome {
            return;
        }
        let Some(Ok(members)) = self.project_roots.read(cx).snapshot() else {
            return;
        };
        if !members.errors.is_empty() {
            return;
        }
        self.shallow_startup.checked = true;
        let [repository] = members.roots.as_slice() else {
            return;
        };
        #[cfg(not(test))]
        let store = plico::unshallow_notice::Store::for_project(&self.repo.path);
        #[cfg(test)]
        let store: Option<plico::unshallow_notice::Store> = None;
        let Some(store) = store else {
            return;
        };
        let repository = repository.clone();
        let token = self.shallow_startup.token.clone();
        let roots_id = self.project_roots.entity_id();
        self.shallow_startup.task = Some(cx.spawn_in(window, async move |this, wx| {
            let worker_repo = repository.clone();
            let worker_token = token.clone();
            let result = wx
                .background_spawn(async move { store.check(&worker_repo, &worker_token) })
                .await;
            let _ = this.update_in(wx, |s, window, cx| {
                if token.cancelled()
                    || s.project_roots.entity_id() != roots_id
                    || s.operation.busy()
                {
                    return;
                }
                if !matches!(result, Ok(true)) {
                    return;
                }
                let catalog = s.project_roots.read(cx);
                if !catalog.is_shallow(&repository.path)
                    || !catalog.snapshot().is_some_and(|r| {
                        r.is_ok_and(|r| r.roots.len() == 1 && r.roots[0].path == repository.path)
                    })
                {
                    return;
                }
                let owner = cx.entity().downgrade();
                window.push_notification(
                    Notification::info(t("This repository can be converted to a complete one"))
                        .id::<ShallowNotification>()
                        .title(t("Shallow repository detected"))
                        .delivery(NotificationDelivery::InApp)
                        .placement(Anchor::BottomRight)
                        .autohide(false)
                        .content(move |_, _, cx| {
                            let owner = owner.clone();
                            let repo = repository.clone();
                            h_flex()
                                .justify_start()
                                .child(
                                    Button::new("unshallow-startup-action")
                                        .ghost()
                                        .text_color(rgb(crate::ui_theme::color(
                                            cx, 0x589df6, 0x2f5eb9,
                                        )))
                                        .text_sm()
                                        .p_0()
                                        .h(px(24.))
                                        .label(t("Unshallow repository"))
                                        .on_click(move |_, window, cx| {
                                            let _ = owner.update(cx, |s, cx| {
                                                if !s.operation.busy()
                                                    && s.project_roots.entity_id() == roots_id
                                                {
                                                    s.start_unshallow(repo.clone(), window, cx);
                                                }
                                            });
                                        }),
                                )
                                .into_any_element()
                        }),
                    cx,
                );
            });
        }));
    }
}
