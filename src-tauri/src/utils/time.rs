use chrono::{DateTime, Duration, Local, NaiveTime, TimeZone, Utc, Weekday};
use std::time::Instant;

const DATABASE_TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
const MONITORING_TIME_FORMAT: &str = "%H:%M";

pub fn utc_now_string() -> String {
    utc_datetime_string(Utc::now())
}

pub fn unix_now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn utc_datetime_string(value: DateTime<Utc>) -> String {
    value.format(DATABASE_TIMESTAMP_FORMAT).to_string()
}

pub fn utc_days_ago_string(days: i64) -> String {
    utc_datetime_string(Utc::now() - Duration::days(days))
}

pub fn utc_hours_ago_string(hours: i64) -> String {
    utc_datetime_string(Utc::now() - Duration::hours(hours))
}

pub fn local_today_utc_range() -> (DateTime<Utc>, DateTime<Utc>) {
    let today = Local::now().date_naive();
    let start = Local
        .from_local_datetime(&today.and_hms_opt(0, 0, 0).expect("本地日期必须有效"))
        .earliest()
        .expect("本地日期开始时间必须有效")
        .with_timezone(&Utc);
    let end = Local
        .from_local_datetime(
            &today
                .succ_opt()
                .expect("次日日期必须有效")
                .and_hms_opt(0, 0, 0)
                .expect("本地日期必须有效"),
        )
        .earliest()
        .expect("本地日期结束时间必须有效")
        .with_timezone(&Utc);
    (start, end)
}

pub fn elapsed_millis(started: Instant) -> i64 {
    started.elapsed().as_millis() as i64
}

pub fn parse_hhmm(value: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value, MONITORING_TIME_FORMAT).ok()
}

pub fn in_monitoring_window(start: Option<&str>, end: Option<&str>, now: NaiveTime) -> bool {
    let (Some(start), Some(end)) = (start.and_then(parse_hhmm), end.and_then(parse_hhmm)) else {
        return true;
    };
    if start == end {
        return true;
    }
    if start < end {
        now >= start && now <= end
    } else {
        now >= start || now <= end
    }
}

pub fn in_monitoring_schedule(
    weekdays: &[u8],
    start: Option<&str>,
    end: Option<&str>,
    weekday: Weekday,
    now: NaiveTime,
) -> bool {
    let day = weekday.number_from_monday() as u8;
    let (Some(start), Some(end)) = (start.and_then(parse_hhmm), end.and_then(parse_hhmm)) else {
        return weekdays.contains(&day);
    };
    if start == end {
        return weekdays.contains(&day);
    }
    if start < end {
        weekdays.contains(&day) && now >= start && now <= end
    } else if now >= start {
        weekdays.contains(&day)
    } else if now <= end {
        weekdays.contains(&match weekday {
            Weekday::Mon => 7,
            _ => day - 1,
        })
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{in_monitoring_schedule, in_monitoring_window};
    use chrono::{NaiveTime, Weekday};

    fn time(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M:%S").unwrap()
    }

    #[test]
    fn unrestricted_when_window_missing() {
        let now = time("12:00:00");
        assert!(in_monitoring_window(None, None, now));
        assert!(in_monitoring_window(Some("08:00"), None, now));
        assert!(in_monitoring_window(None, Some("18:00"), now));
    }

    #[test]
    fn same_day_window_is_inclusive() {
        assert!(in_monitoring_window(
            Some("08:00"),
            Some("18:00"),
            time("08:00:00")
        ));
        assert!(in_monitoring_window(
            Some("08:00"),
            Some("18:00"),
            time("12:30:00")
        ));
        assert!(in_monitoring_window(
            Some("08:00"),
            Some("18:00"),
            time("18:00:00")
        ));
        assert!(!in_monitoring_window(
            Some("08:00"),
            Some("18:00"),
            time("07:59:59")
        ));
        assert!(!in_monitoring_window(
            Some("08:00"),
            Some("18:00"),
            time("18:00:01")
        ));
    }

    #[test]
    fn overnight_window_wraps_midnight() {
        assert!(in_monitoring_window(
            Some("22:00"),
            Some("06:00"),
            time("23:30:00")
        ));
        assert!(in_monitoring_window(
            Some("22:00"),
            Some("06:00"),
            time("05:00:00")
        ));
        assert!(!in_monitoring_window(
            Some("22:00"),
            Some("06:00"),
            time("12:00:00")
        ));
    }

    #[test]
    fn invalid_or_equal_window_stays_unrestricted() {
        let now = time("12:00:00");
        assert!(in_monitoring_window(Some("bad"), Some("18:00"), now));
        assert!(in_monitoring_window(Some("08:00"), Some("08:00"), now));
    }

    #[test]
    fn schedule_respects_selected_weekdays() {
        let weekdays = [1, 2, 3, 4, 5];
        assert!(in_monitoring_schedule(
            &weekdays,
            Some("09:20"),
            Some("18:30"),
            Weekday::Mon,
            time("12:00:00")
        ));
        assert!(!in_monitoring_schedule(
            &weekdays,
            Some("09:20"),
            Some("18:30"),
            Weekday::Sat,
            time("12:00:00")
        ));
        assert!(in_monitoring_schedule(
            &weekdays,
            None,
            None,
            Weekday::Fri,
            time("23:59:00")
        ));
        assert!(!in_monitoring_schedule(
            &weekdays,
            None,
            None,
            Weekday::Sun,
            time("23:59:00")
        ));
    }

    #[test]
    fn overnight_schedule_uses_start_day_after_midnight() {
        assert!(in_monitoring_schedule(
            &[5],
            Some("22:00"),
            Some("02:00"),
            Weekday::Sat,
            time("01:00:00")
        ));
        assert!(!in_monitoring_schedule(
            &[5],
            Some("22:00"),
            Some("02:00"),
            Weekday::Sun,
            time("01:00:00")
        ));
        assert!(in_monitoring_schedule(
            &[7],
            Some("22:00"),
            Some("02:00"),
            Weekday::Mon,
            time("01:00:00")
        ));
    }
}
