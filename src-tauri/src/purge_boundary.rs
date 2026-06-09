use chrono::{Duration, Local};

/// Local calendar date `today - retention_days` as `YYYY-MM-DD` for purge comparisons.
pub fn retention_cutoff_date(retention_days: u32) -> String {
    let today = Local::now().date_naive();
    let cutoff = today - Duration::days(i64::from(retention_days));
    cutoff.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
