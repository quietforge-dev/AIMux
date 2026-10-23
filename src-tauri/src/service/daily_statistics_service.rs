use crate::{dao::daily_statistics_dao as dao, error::AppError, utils::time::utc_datetime_string};
use chrono::{DateTime, Days, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use sqlx::SqlitePool;
use std::collections::BTreeSet;

pub fn day_range<T: TimeZone>(
    zone: &T,
    date: NaiveDate,
) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
    let next = date
        .succ_opt()
        .ok_or_else(|| AppError::Internal("统计日期超出范围".into()))?;
    let boundary = |date: NaiveDate| {
        zone.from_local_datetime(&date.and_time(NaiveTime::MIN))
            .earliest()
            .map(|value| value.with_timezone(&Utc))
            .ok_or_else(|| AppError::Internal(format!("统计日期 {date} 在当前时区没有有效的零点")))
    };
    Ok((boundary(date)?, boundary(next)?))
}

pub fn eligible_date<T: TimeZone>(now: &DateTime<T>) -> Option<NaiveDate> {
    let days = if now.time() >= NaiveTime::from_hms_opt(8, 0, 0).unwrap() {
        1
    } else {
        2
    };
    now.date_naive().checked_sub_days(Days::new(days))
}

pub async fn catch_up(pool: &SqlitePool, now: DateTime<Local>) -> Result<usize, AppError> {
    let Some(last) = eligible_date(&now) else {
        return Ok(0);
    };
    let dates: BTreeSet<String> = dao::dates(pool).await?.into_iter().collect();
    let first_raw = dao::first_usage_at(pool)
        .await?
        .map(|value| {
            DateTime::parse_from_rfc3339(&value)
                .map(|value| value.with_timezone(&Local).date_naive())
        })
        .transpose()
        .map_err(|error| AppError::Internal(format!("使用记录日期无效：{error}")))?;
    let first_archived = dates
        .first()
        .map(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .transpose()
        .map_err(|error| AppError::Internal(format!("每日统计日期无效：{error}")))?;
    let Some(mut date) = first_raw.into_iter().chain(first_archived).min() else {
        return Ok(0);
    };
    let mut count = 0;
    while date <= last {
        let key = date.to_string();
        if !dates.contains(&key) {
            let (start, end) = day_range(&Local, date)?;
            if dao::archive_day(
                pool,
                &key,
                &utc_datetime_string(start),
                &utc_datetime_string(end),
                &utc_datetime_string(now.with_timezone(&Utc)),
            )
            .await?
            {
                count += 1;
            }
        }
        date = date
            .succ_opt()
            .ok_or_else(|| AppError::Internal("统计日期超出范围".into()))?;
        tokio::task::yield_now().await;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_am_boundary_and_local_day_ranges() {
        let before = DateTime::parse_from_rfc3339("2026-10-23T07:59:59+08:00").unwrap();
        let after = DateTime::parse_from_rfc3339("2026-10-23T08:00:00+08:00").unwrap();
        assert_eq!(eligible_date(&before).unwrap().to_string(), "2026-10-21");
        assert_eq!(eligible_date(&after).unwrap().to_string(), "2026-10-22");
        let (start, end) = day_range(&after.timezone(), after.date_naive()).unwrap();
        assert_eq!(utc_datetime_string(start), "2026-10-22T16:00:00Z");
        assert_eq!(utc_datetime_string(end), "2026-10-23T16:00:00Z");
    }
}
