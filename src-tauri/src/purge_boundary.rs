use chrono::{Datelike, Duration, Local, NaiveDate};

/// Local calendar date `today - retention_days` as `YYYY-MM-DD` for purge comparisons.
pub fn retention_cutoff_date(retention_days: u32) -> String {
    let today = Local::now().date_naive();
    let cutoff = today - Duration::days(i64::from(retention_days));
    cutoff.format("%Y-%m-%d").to_string()
}

/// Local calendar today as `YYYY-MM-DD`.
pub fn local_today() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

/// Monday (ISO) of the week containing `date`.
pub fn iso_week_start(date: NaiveDate) -> NaiveDate {
    let days_from_monday = date.weekday().num_days_from_monday();
    date - Duration::days(i64::from(days_from_monday))
}

/// Local Monday of the current week as `YYYY-MM-DD`.
pub fn local_week_start() -> String {
    iso_week_start(Local::now().date_naive())
        .format("%Y-%m-%d")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn retention_cutoff_date_format() {
        let s = retention_cutoff_date(0);
        assert_eq!(s.len(), 10);
        assert_eq!(s.as_bytes()[4], b'-');
        assert_eq!(s.as_bytes()[7], b'-');
    }

    #[test]
    fn retention_cutoff_date_decreases_with_n() {
        let n0 = retention_cutoff_date(0);
        let n1 = retention_cutoff_date(1);
        assert!(n1 < n0);
    }

    #[test]
    fn local_today_format() {
        let s = local_today();
        assert_eq!(s.len(), 10);
    }

    #[test]
    fn iso_week_start_is_monday() {
        // 2026-06-10 is Wednesday
        let wed = NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let start = iso_week_start(wed);
        assert_eq!(start.weekday(), chrono::Weekday::Mon);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 6, 8).unwrap());
    }

    #[test]
    fn iso_week_start_monday_unchanged() {
        let mon = NaiveDate::from_ymd_opt(2026, 6, 8).unwrap();
        assert_eq!(iso_week_start(mon), mon);
    }
}
