use gpui_kit::component::{
    IconName,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use plico::{
    i18n::t,
    pull::{OptionFlag, Options, Target, Targets},
};
use std::collections::BTreeMap;

const FLAGS: [OptionFlag; 6] = [
    OptionFlag::Rebase,
    OptionFlag::FfOnly,
    OptionFlag::NoFf,
    OptionFlag::Squash,
    OptionFlag::NoCommit,
    OptionFlag::NoVerify,
];
fn description(flag: OptionFlag) -> &'static str {
    t(match flag {
        OptionFlag::Rebase => "Rebase the current branch on top of incoming changes",
        OptionFlag::FfOnly => "Merge only if it can be fast-forwarded",
        OptionFlag::NoFf => "Create a merge commit even if it can be fast-forwarded",
        OptionFlag::Squash => "Create a single commit for all pulled changes",
        OptionFlag::NoCommit => "Merge, but do not commit the result",
        OptionFlag::NoVerify => "Bypass the pre-merge and commit message hooks",
    })
}

pub struct PullDialog {
    targets: Targets,
    rebase_available: bool,
    remote: String,
    branch: Entity<InputState>,
    flags: Vec<OptionFlag>,
    option_focus: BTreeMap<OptionFlag, FocusHandle>,
    _input_watch: Subscription,
}
impl PullDialog {
    pub fn new(
        targets: Targets,
        rebase_available: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let remote = targets.default_remote().unwrap_or("").to_owned();
        let value = targets
            .default_branch(&remote, None)
            .unwrap_or("")
            .to_owned();
        let branch = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(value, window, cx);
            s
        });
        let watch = cx.observe(&branch, |_, _, cx| cx.notify());
        Self {
            targets,
            rebase_available,
            remote,
            branch,
            flags: Vec::new(),
            option_focus: [
                OptionFlag::Rebase,
                OptionFlag::FfOnly,
                OptionFlag::NoFf,
                OptionFlag::Squash,
                OptionFlag::NoCommit,
                OptionFlag::NoVerify,
            ]
            .into_iter()
            .map(|flag| (flag, cx.focus_handle()))
            .collect(),
            _input_watch: watch,
        }
    }
    pub fn selection(&self, cx: &App) -> Option<(Target, Options)> {
        let branch = self.branch.read(cx).value().to_string();
        (self.targets.contains(&self.remote, &branch)
            && (!self.flags.contains(&OptionFlag::Rebase) || self.rebase_available))
            .then(|| {
                (
                    Target {
                        remote: self.remote.clone(),
                        branch,
                    },
                    Options::new(self.flags.iter().copied()).unwrap(),
                )
            })
    }
    pub fn options_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let options = Options::new(self.flags.iter().copied()).unwrap();
        let attached = self.rebase_available;
        Button::new("pull-options")
            .link()
            .dropdown_caret(true)
            .label(t("Modify options"))
            .dropdown_menu(move |mut menu, _, mcx| {
                menu = menu.close_on_confirm(false).min_w(px(550.)).max_w(px(680.));
                let menu_owner = mcx.entity().downgrade();
                for flag in FLAGS {
                    let owner = owner.clone();
                    let menu_owner = menu_owner.clone();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, cx| {
                            h_flex()
                                .w_full()
                                .gap(px(50.))
                                .child(div().flex_1().child(description(flag)))
                                .child(
                                    div()
                                        .w(px(100.))
                                        .flex_shrink_0()
                                        .text_color(rgb(crate::ui_theme::color(
                                            cx, 0x9fa2a8, 0x5f6269,
                                        )))
                                        .child(flag.argument()),
                                )
                        })
                        .checked(options.contains(flag))
                        .disabled(
                            !options.enabled(flag) || (flag == OptionFlag::Rebase && !attached),
                        )
                        .on_click(move |_, _, cx| {
                            let updated = owner.update(cx, |s, cx| {
                                let options = Options::new(s.flags.iter().copied()).unwrap();
                                if !options.enabled(flag)
                                    || (flag == OptionFlag::Rebase && !s.rebase_available)
                                {
                                    return options;
                                }
                                if s.flags.contains(&flag) {
                                    s.flags.retain(|value| *value != flag);
                                } else {
                                    s.flags.push(flag);
                                }
                                cx.notify();
                                Options::new(s.flags.iter().copied()).unwrap()
                            });
                            if let Ok(options) = updated {
                                let menu_owner = menu_owner.clone();
                                cx.defer(move |cx| {
                                    let _ = menu_owner.update(cx, |menu, cx| {
                                        for (index, flag) in FLAGS.into_iter().enumerate() {
                                            menu.set_item_state(
                                                index,
                                                options.contains(flag),
                                                options.enabled(flag)
                                                    && (flag != OptionFlag::Rebase || attached),
                                                cx,
                                            );
                                        }
                                    });
                                });
                            }
                        }),
                    );
                }
                menu
            })
            .into_any_element()
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.branch.read(cx).focus_handle(cx).focus(window, cx);
    }
}
impl Render for PullDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let remotes = self.targets.remotes.clone();
        let remote = Button::new("pull-remote")
            .label(if self.remote.is_empty() {
                t("Select remote").into()
            } else {
                self.remote.clone()
            })
            .dropdown_menu(move |mut menu, _, _| {
                for remote in &remotes {
                    let owner = owner.clone();
                    let remote = remote.clone();
                    menu = menu.item(PopupMenuItem::new(remote.clone()).on_click(
                        move |_, window, cx| {
                            let _ = owner.update(cx, |s, cx| {
                                let previous = s.branch.read(cx).value().to_string();
                                let next = s
                                    .targets
                                    .default_branch(&remote, Some(&previous))
                                    .unwrap_or("")
                                    .to_owned();
                                s.remote = remote.clone();
                                s.branch.update(cx, |s, cx| s.set_value(next, window, cx));
                                cx.notify();
                            });
                        },
                    ));
                }
                menu
            });
        let owner = cx.entity().downgrade();
        let mut branches: Vec<_> = self
            .targets
            .branches
            .iter()
            .filter(|b| b.remote == self.remote)
            .map(|b| b.branch.clone())
            .collect();
        branches.sort();
        let branch =
            Input::new(&self.branch).suffix(Button::new("pull-branches").label("▾").dropdown_menu(
                move |mut menu, _, _| {
                    for branch in branches.iter().take(200) {
                        let owner = owner.clone();
                        let branch = branch.clone();
                        menu = menu.item(PopupMenuItem::new(branch.clone()).on_click(
                            move |_, window, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    s.branch
                                        .update(cx, |s, cx| s.set_value(branch.clone(), window, cx))
                                });
                            },
                        ));
                    }
                    menu
                },
            ));
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child("git pull")
                    .child(remote)
                    .child(div().flex_1().child(branch)),
            )
            .when(!self.flags.is_empty(), |v| {
                v.child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .children(self.flags.iter().copied().map(|flag| {
                            let focus = self.option_focus[&flag].clone();
                            let focused = focus.is_focused(window);
                            let click_focus = focus.clone();
                            h_flex()
                                .id(("pull-option", flag as usize))
                                .track_focus(&focus)
                                .tab_stop(true)
                                .gap_1()
                                .pl_2()
                                .rounded(px(4.))
                                .bg(rgb(crate::ui_theme::color(cx, 0x35363a, 0xf7f8f9)))
                                .border_1()
                                .border_color(rgb(if focused {
                                    crate::ui_theme::color(cx, 0x6b9bfa, 0x3871e1)
                                } else {
                                    crate::ui_theme::color(cx, 0x35363a, 0xdddfe4)
                                }))
                                .on_mouse_down(MouseButton::Left, move |_, w, cx| {
                                    click_focus.focus(w, cx)
                                })
                                .on_key_down(cx.listener(move |s, event: &KeyDownEvent, w, cx| {
                                    if matches!(
                                        event.keystroke.key.as_str(),
                                        "backspace" | "delete"
                                    ) {
                                        s.flags.retain(|value| *value != flag);
                                        s.focus(w, cx);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                }))
                                .child(flag.argument())
                                .child(
                                    Button::new(("remove-pull-option", flag as usize))
                                        .ghost()
                                        .tab_stop(false)
                                        .icon(IconName::Close)
                                        .tooltip(t("Remove option (Backspace/Delete)"))
                                        .on_click(cx.listener(move |s, _, w, cx| {
                                            s.flags.retain(|value| *value != flag);
                                            s.focus(w, cx);
                                            cx.notify();
                                        })),
                                )
                        })),
                )
            })
            .when(self.selection(cx).is_none(), |v| {
                v.child(
                    div()
                        .text_color(rgb(crate::ui_theme::color(cx, 0xf2a6a6, 0xc54e58)))
                        .child(t("Select an existing remote branch")),
                )
            })
    }
}
