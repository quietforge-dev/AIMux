use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use std::time::Instant;

const DATABASE_TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";

pub fn utc_now_string() -> String {
    utc_datetime_string(Utc::now())
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
        .single()
        .expect("本地日期开始时间必须唯一")
        .with_timezone(&Utc);
    (start, start + Duration::days(1))
}

pub fn elapsed_millis(started: Instant) -> i64 {
    started.elapsed().as_millis() as i64
}
