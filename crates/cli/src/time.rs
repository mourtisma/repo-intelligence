//! RFC 3339 formatting for `analyzed_at`, in UTC with second precision.
//! Hand-written (about 20 lines) to avoid a time dependency for one value.

use std::time::{SystemTime, UNIX_EPOCH};

/// Formats `t` as `YYYY-MM-DDTHH:MM:SSZ`. Times before the Unix epoch are clamped to it.
pub fn rfc3339_utc(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let (days, secs_of_day) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs_of_day / 3600,
        secs_of_day % 3600 / 60,
        secs_of_day % 60
    )
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian.
/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe as i64 + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(secs: u64) -> String {
        rfc3339_utc(UNIX_EPOCH + Duration::from_secs(secs))
    }

    #[test]
    fn epoch() {
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn leap_day_2024() {
        assert_eq!(at(1_709_210_096), "2024-02-29T12:34:56Z");
    }

    #[test]
    fn first_of_november_2026() {
        assert_eq!(at(1_793_491_200), "2026-11-01T00:00:00Z");
    }

    #[test]
    fn end_of_year_rolls_over() {
        // 2000-12-31T23:59:59Z
        assert_eq!(at(978_307_199), "2000-12-31T23:59:59Z");
    }

    #[test]
    fn shape_is_fixed_width() {
        assert_eq!(at(1).len(), "YYYY-MM-DDTHH:MM:SSZ".len());
    }
}
