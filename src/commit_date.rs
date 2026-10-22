//! Log date presentation, following Rebased DateFormatUtil's pretty date rules.
use chrono::{DateTime, Local, TimeZone};

pub fn format(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|date| timestamp(date.timestamp()))
        .unwrap_or_else(|_| value.to_owned())
}
pub fn timestamp(seconds: i64) -> String {
    Local
        .timestamp_opt(seconds, 0)
        .single()
        .map(|date| pretty(date, Local::now(), crate::i18n::is_chinese()))
        .unwrap_or_default()
}
fn pretty<T: TimeZone>(date: DateTime<T>, now: DateTime<T>, chinese: bool) -> String {
    let delta = now.clone().signed_duration_since(&date).num_seconds();
    if (0..=3660).contains(&delta) {
        // Java Math.rint uses ties-to-even rounding.
        let minutes = (delta as f64 / 60.).round_ties_even() as i64;
        return match (chinese, minutes) {
            (true, 0) => "片刻前".into(),
            (true, 1) => "1 分钟前".into(),
            (true, 60..) => "1 小时前".into(),
            (true, _) => format!("{minutes} 分钟前"),
            (false, 0) => "Moments ago".into(),
            (false, 1) => "A minute ago".into(),
            (false, 60..) => "1 hour ago".into(),
            (false, _) => format!("{minutes} minutes ago"),
        };
    }
    let day = date.date_naive();
    let time = date.time().format("%H:%M");
    if day == now.date_naive() {
        format!("{} {time}", if chinese { "今天" } else { "Today" })
    } else if Some(day) == now.date_naive().pred_opt() {
        format!("{} {time}", if chinese { "昨天" } else { "Yesterday" })
    } else {
        format!("{} {time}", day.format("%Y/%m/%d"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, FixedOffset};
    #[test]
    fn local_calendar_year_boundary_and_time_are_preserved() {
        let zone = FixedOffset::east_opt(8 * 3600).unwrap();
        let now = zone.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        assert_eq!(pretty(now - Duration::hours(3), now, false), "Today 09:00");
        assert_eq!(pretty(now - Duration::days(1), now, true), "昨天 12:00");
        assert_eq!(
            pretty(now - Duration::days(2), now, false),
            "2025/12/30 12:00"
        );
        assert_eq!(pretty(now + Duration::hours(1), now, true), "今天 13:00");
    }
    #[test]
    fn recent_dates_use_rebased_minutes_rounding_and_cutoff() {
        let now = Local::now();
        assert_eq!(
            pretty(now - Duration::seconds(90), now, false),
            "2 minutes ago"
        );
        assert_eq!(pretty(now - Duration::seconds(30), now, true), "片刻前");
        assert_eq!(
            pretty(now - Duration::seconds(3660), now, false),
            "1 hour ago"
        );
    }
    #[test]
    fn utc_input_uses_viewers_calendar_and_unknown_values_survive() {
        let zone = FixedOffset::east_opt(8 * 3600).unwrap();
        let date = DateTime::parse_from_rfc3339("2025-12-31T20:00:00Z")
            .unwrap()
            .with_timezone(&zone);
        let now = zone.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        assert_eq!(pretty(date, now, false), "Today 04:00");
        assert_eq!(format("unknown"), "unknown");
    }
}
