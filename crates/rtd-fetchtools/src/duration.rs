//! Go-style duration parsing (`12m`, `4h`, `90s`, `1h30m`).

use std::time::Duration;

/// Parses a Go-style duration string into a [`Duration`].
///
/// Supports `h`, `m` and `s` units, combined (`1h30m`) and repeated. Returns
/// `None` when the input is empty or malformed.
#[must_use]
pub fn parse(raw: &str) -> Option<Duration> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let mut total = Duration::ZERO;
    let mut number = String::new();
    let mut saw_unit = false;
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }
        let value: u64 = number.parse().ok()?;
        let seconds = match ch {
            'h' => value.checked_mul(3600)?,
            'm' => value.checked_mul(60)?,
            's' => value,
            _ => return None,
        };
        total = total.checked_add(Duration::from_secs(seconds))?;
        number.clear();
        saw_unit = true;
    }
    if !number.is_empty() || !saw_unit {
        return None;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::parse;
    use std::time::Duration;

    #[test]
    fn parses_go_durations() {
        assert_eq!(parse("90s"), Some(Duration::from_secs(90)));
        assert_eq!(parse("12m"), Some(Duration::from_secs(720)));
        assert_eq!(parse("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse("bad"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("5"), None);
    }
}
