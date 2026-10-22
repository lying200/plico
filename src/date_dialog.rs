use chrono::{Local, NaiveDate, TimeZone};
use gpui_kit::component::{
    Disableable,
    checkbox::Checkbox,
    date_picker::{DatePicker, DatePickerEvent, DatePickerState},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use plico::date_filter::Bounds;
use plico::i18n::t;

/// Draft values live only as long as this dialog; cancel never mutates Log.
pub struct DateDialog {
    after: Entity<DatePickerState>,
    before: Entity<DatePickerState>,
    after_input: Entity<InputState>,
    before_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
    use_after: bool,
    use_before: bool,
    error: Option<String>,
}

impl DateDialog {
    pub fn new(bounds: Bounds, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let now = Local::now().naive_local();
        let mut picker = |timestamp: Option<i64>| {
            let value = timestamp
                .and_then(|t| Local.timestamp_opt(t, 0).single())
                .map(|d| d.naive_local())
                .unwrap_or(now);
            cx.new(|cx| {
                let mut state = DatePickerState::new(window, cx).date_format("%Y-%m-%d");
                state.set_date_time(value, window, cx);
                state
            })
        };
        let after = picker(bounds.after);
        let before = picker(bounds.before);
        let mut input = |picker: &Entity<DatePickerState>| {
            let value = picker
                .read(cx)
                .date_time()
                .start()
                .unwrap()
                .format("%Y/%m/%d")
                .to_string();
            cx.new(|cx| {
                let mut state = InputState::new(window, cx);
                state.set_value(value, window, cx);
                state
            })
        };
        let after_input = input(&after);
        let before_input = input(&before);
        let mut subscriptions = Vec::new();
        for (picker, input) in [(&after, &after_input), (&before, &before_input)] {
            let text = input.clone();
            subscriptions.push(
                cx.subscribe_in(picker, window, move |s, _, event, window, cx| {
                    let DatePickerEvent::Change(value) = event;
                    if let Some(date) = value.start() {
                        text.update(cx, |input, cx| {
                            input.set_value(date.format("%Y/%m/%d").to_string(), window, cx)
                        });
                        s.error = None;
                        cx.notify();
                    }
                }),
            );
            let calendar = picker.clone();
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |s, input, event, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        if let Ok(date) =
                            NaiveDate::parse_from_str(input.read(cx).value().trim(), "%Y/%m/%d")
                        {
                            calendar.update(cx, |picker, cx| picker.set_date(date, window, cx));
                        }
                        s.error = None;
                        cx.notify();
                    }
                },
            ));
        }
        Self {
            after,
            before,
            after_input,
            before_input,
            _subscriptions: subscriptions,
            use_after: bounds.after.is_some(),
            use_before: bounds.before.is_some(),
            error: None,
        }
    }

    pub fn confirm(&mut self, cx: &mut Context<Self>) -> Result<Bounds, ()> {
        let read = |enabled: bool,
                    picker: &Entity<DatePickerState>,
                    input: &Entity<InputState>|
         -> Result<Option<i64>, String> {
            if !enabled {
                return Ok(None);
            }
            let date = NaiveDate::parse_from_str(input.read(cx).value().trim(), "%Y/%m/%d")
                .map_err(|_| "Enter a valid date (YYYY/MM/DD)")?;
            let time = picker
                .read(cx)
                .date_time()
                .start()
                .ok_or("Select a valid date")?
                .time();
            let date = date.and_time(time);
            Local
                .from_local_datetime(&date)
                .single()
                .map(|d| Some(d.timestamp()))
                .ok_or_else(|| "This local time is ambiguous or unavailable".into())
        };
        let result = read(self.use_after, &self.after, &self.after_input).and_then(|after| {
            read(self.use_before, &self.before, &self.before_input)
                .map(|before| Bounds { after, before })
        });
        match result {
            Ok(bounds) => Ok(bounds),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                Err(())
            }
        }
    }
}

impl Render for DateDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        Checkbox::new("after-enabled")
                            .label(t("After"))
                            .checked(self.use_after)
                            .on_click(cx.listener(|s, value, _, cx| {
                                s.use_after = *value;
                                s.error = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .w(px(88.))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|s, _, window, cx| {
                                    if s.use_after {
                                        s.after_input.read(cx).focus_handle(cx).focus(window, cx);
                                    }
                                }),
                            )
                            .child(Input::new(&self.after_input).disabled(!self.use_after)),
                    )
                    .child(
                        div().w(px(72.)).mr(px(10.)).child(
                            DatePicker::new(&self.after)
                                .icon_only(true)
                                .disabled(!self.use_after),
                        ),
                    )
                    .child(
                        Checkbox::new("before-enabled")
                            .label(t("Before"))
                            .checked(self.use_before)
                            .on_click(cx.listener(|s, value, _, cx| {
                                s.use_before = *value;
                                s.error = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .w(px(88.))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|s, _, window, cx| {
                                    if s.use_before {
                                        s.before_input.read(cx).focus_handle(cx).focus(window, cx);
                                    }
                                }),
                            )
                            .child(Input::new(&self.before_input).disabled(!self.use_before)),
                    )
                    .child(
                        div().w(px(72.)).child(
                            DatePicker::new(&self.before)
                                .icon_only(true)
                                .disabled(!self.use_before),
                        ),
                    ),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_color(rgb(0xf2a6a6)).child(error))
            })
    }
}
