//! Capture date-menu bounds once, in the user's local calendar.
use chrono::{DateTime, Days, Local, TimeZone};

/// Like Rebased's menu: subtract one calendar day, then another six.
/// Do not silently substitute elapsed 24-hour periods across DST transitions.
pub fn presets<T: TimeZone>(now: DateTime<T>) -> Result<[i64; 2], String> {
    let day = now.checked_sub_days(Days::new(1))
        .ok_or("The previous day has an ambiguous or unavailable local time")?;
    let day_timestamp = day.timestamp();
    let week = day.checked_sub_days(Days::new(6))
        .ok_or("The previous week has an ambiguous or unavailable local time")?;
    Ok([day_timestamp, week.timestamp()])
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    pub after: Option<i64>,
    pub before: Option<i64>,
}

impl Bounds {
    pub fn is_empty(self) -> bool { self.after.is_none() && self.before.is_none() }

    pub fn caption(self) -> String {
        let format = |timestamp| Local.timestamp_opt(timestamp, 0).single()
            .map(|date| date.format("%Y-%m-%d").to_string()).unwrap_or_default();
        match (self.after, self.before) {
            (Some(a), Some(b)) => format!("{}–{}", format(a), format(b)),
            (Some(a), None) => format!("{} {}", crate::i18n::t("Since"), format(a)),
            (None, Some(b)) => format!("{} {}", crate::i18n::t("Until"), format(b)),
            (None, None) => crate::i18n::t("Date").into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::America::New_York;

    #[test]
    fn calendar_presets_preserve_wall_time_across_dst() {
        for (month, day, hours) in [(3, 8, 23), (11, 1, 25)] {
            let now = New_York.with_ymd_and_hms(2026, month, day, 12, 30, 0).unwrap();
            let [previous_day, previous_week] = presets(now).unwrap();
            assert_eq!(now.timestamp() - previous_day, hours * 3600);
            assert_eq!(New_York.timestamp_opt(previous_day, 0).unwrap().format("%H:%M").to_string(), "12:30");
            assert_eq!(New_York.timestamp_opt(previous_week, 0).unwrap().date_naive(), now.date_naive() - Days::new(7));
        }
    }

    #[test]
    fn invalid_wall_time_is_an_error_not_an_absent_filter() {
        let now = New_York.with_ymd_and_hms(2026, 3, 9, 2, 30, 0).unwrap();
        assert!(presets(now).is_err());
        let now = New_York.with_ymd_and_hms(2026, 11, 2, 1, 30, 0).unwrap();
        assert!(presets(now).is_err());
    }
}
