use aimux_lib::utils::time::utc_datetime_string;
use chrono::{TimeZone, Utc};

#[test]
fn formats_database_timestamps_in_utc() {
    let value = Utc
        .with_ymd_and_hms(2026, 8, 24, 9, 8, 7)
        .single()
        .expect("固定 UTC 时间必须有效");

    assert_eq!(utc_datetime_string(value), "2026-08-24T09:08:07Z");
}
