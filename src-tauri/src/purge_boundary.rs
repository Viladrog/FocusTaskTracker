/// Local midnight today as UTC `YYYY-MM-DD HH:MM:SS` for purge comparisons.
pub fn today_local_midnight_boundary_utc() -> String {
    use chrono::{Local, TimeZone};
    let today = Local::now().date_naive();
    let local_midnight = today.and_hms_opt(0, 0, 0).unwrap();
    let dt = Local.from_local_datetime(&local_midnight).unwrap();
    dt.with_timezone(&chrono::Utc)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_local_midnight_boundary_utc_format() {
        let s = today_local_midnight_boundary_utc();
        assert_eq!(s.len(), 19);
        assert!(s.as_bytes()[4] == b'-');
        assert!(s.as_bytes()[7] == b'-');
        assert!(s.as_bytes()[10] == b' ');
        assert!(s.as_bytes()[13] == b':');
        assert!(s.as_bytes()[16] == b':');
        assert!(s.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' ' || c == ':'));
    }
}
