//! RFC 3339 (UTC) formatting matching Go's `time.RFC3339`.
//!
//! Go's tools stamp timestamps with `time.Unix(secs, 0).UTC().Format(time.RFC3339)`,
//! which renders whole seconds as `YYYY-MM-DDTHH:MM:SSZ`. No calendar or timezone
//! crate is pulled in; the civil date is derived from the day count.

/// Formats Unix `secs` as a UTC RFC 3339 timestamp (`2006-01-02T15:04:05Z`).
#[must_use]
pub fn rfc3339_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Converts a day count since the Unix epoch into a civil `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days` algorithm, valid across the whole `i64`
/// day range.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1); // [1, 31]
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (year + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::rfc3339_utc;

    #[test]
    fn epoch_and_known_instants() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn leap_day() {
        assert_eq!(rfc3339_utc(1_582_934_400), "2020-02-29T00:00:00Z");
    }

    #[test]
    fn before_epoch() {
        assert_eq!(rfc3339_utc(-1), "1969-12-31T23:59:59Z");
    }
}
