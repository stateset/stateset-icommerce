//! RFC 3339 timestamps in the one form every ICP SDK emits.
//!
//! `JavaScript` builds every wire timestamp as `new Date(x).toISOString()`:
//! UTC, with exactly three fractional digits. The Python SDK re-emits parsed
//! strings in that form for the same reason (`codec._binding_expiry`). An SDK
//! that passed an operator-supplied RFC 3339 string through untouched would
//! sign different canonical bytes for the same instant, and no other
//! implementation could reproduce the signature — so parse, then re-emit.

use crate::Error;

/// Milliseconds since the Unix epoch, from the system clock.
pub(crate) fn now_millis() -> i64 {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before the Unix epoch");
    // u128 → i64 cannot overflow before the year 292 million.
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

/// Format epoch milliseconds as `YYYY-MM-DDTHH:MM:SS.mmmZ`.
///
/// Byte-identical to `new Date(ms).toISOString()` for every instant these
/// SDKs deal in.
pub(crate) fn format_rfc3339_millis(epoch_ms: i64) -> String {
    let secs = epoch_ms.div_euclid(1000);
    let millis = epoch_ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}.{millis:03}Z")
}

/// Parse an RFC 3339 timestamp into epoch milliseconds.
///
/// Accepts `Z`, a numeric `±HH:MM` / `±HHMM` offset, or no offset at all
/// (read as UTC, matching the Python SDK's naive-datetime handling).
/// Fractional seconds beyond milliseconds are truncated, exactly as
/// `Date.prototype.toISOString` and `isoformat(timespec="milliseconds")` do.
///
/// Deliberately strict: an expiry no handler can parse is a binding no
/// handler can accept, and rejecting it here beats surfacing it hours later
/// as an unexplained signature failure.
pub(crate) fn parse_rfc3339_millis(input: &str) -> Result<i64, Error> {
    let bad = || Error::InvalidInput(format!("expiry is not RFC 3339: {input:?}"));
    let b = input.as_bytes();
    if b.len() < 19 {
        return Err(bad());
    }
    let num = |range: std::ops::Range<usize>| -> Option<i64> {
        let slice = input.get(range)?;
        if slice.bytes().all(|c| c.is_ascii_digit()) { slice.parse().ok() } else { None }
    };
    if b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ') {
        return Err(bad());
    }
    if b[13] != b':' || b[16] != b':' {
        return Err(bad());
    }
    let (year, month, day) =
        (num(0..4).ok_or_else(bad)?, num(5..7).ok_or_else(bad)?, num(8..10).ok_or_else(bad)?);
    let (hour, minute, second) =
        (num(11..13).ok_or_else(bad)?, num(14..16).ok_or_else(bad)?, num(17..19).ok_or_else(bad)?);
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return Err(bad());
    }
    if hour > 23 || minute > 59 || second > 59 {
        return Err(bad());
    }

    let mut rest = &input[19..];
    let mut millis = 0i64;
    if let Some(frac) = rest.strip_prefix('.').or_else(|| rest.strip_prefix(',')) {
        let digits: String = frac.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return Err(bad());
        }
        // Truncate (never round) to milliseconds, zero-padding a short fraction.
        let mut ms: String = digits.chars().take(3).collect();
        while ms.len() < 3 {
            ms.push('0');
        }
        millis = ms.parse().map_err(|_| bad())?;
        rest = &rest[1 + digits.len()..];
    }

    let offset_secs = match rest.as_bytes() {
        [] | [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), ..] => {
            let body = &rest[1..];
            let (oh, om) = match body.len() {
                5 if body.as_bytes()[2] == b':' => (&body[0..2], &body[3..5]),
                4 => (&body[0..2], &body[2..4]),
                _ => return Err(bad()),
            };
            let parse2 = |s: &str| -> Option<i64> {
                if s.bytes().all(|c| c.is_ascii_digit()) { s.parse().ok() } else { None }
            };
            let (oh, om) = (parse2(oh).ok_or_else(bad)?, parse2(om).ok_or_else(bad)?);
            if oh > 23 || om > 59 {
                return Err(bad());
            }
            let magnitude = oh * 3600 + om * 60;
            if *sign == b'-' { -magnitude } else { magnitude }
        }
        _ => return Err(bad()),
    };

    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second - offset_secs;
    Ok(secs * 1000 + millis)
}

/// Days since 1970-01-01 for a proleptic-Gregorian date.
/// Howard Hinnant's `days_from_civil`.
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`] — Hinnant's `civil_from_days`.
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

const fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_the_javascript_iso_form() {
        assert_eq!(format_rfc3339_millis(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(format_rfc3339_millis(1_767_225_600_000), "2026-01-01T00:00:00.000Z");
        assert_eq!(format_rfc3339_millis(1_767_225_600_123), "2026-01-01T00:00:00.123Z");
    }

    #[test]
    fn round_trips_every_accepted_spelling_of_one_instant() {
        for s in [
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00.000Z",
            "2026-01-01T00:00:00",
            "2026-01-01t00:00:00z",
            "2026-01-01T02:00:00+02:00",
            "2026-01-01T02:00:00+0200",
            "2025-12-31T19:00:00-05:00",
        ] {
            assert_eq!(
                format_rfc3339_millis(parse_rfc3339_millis(s).unwrap()),
                "2026-01-01T00:00:00.000Z",
                "input {s}"
            );
        }
    }

    #[test]
    fn truncates_sub_millisecond_precision_like_the_other_sdks() {
        assert_eq!(parse_rfc3339_millis("2026-01-01T00:00:00.123456Z").unwrap() % 1000, 123);
        assert_eq!(parse_rfc3339_millis("2026-01-01T00:00:00.1Z").unwrap() % 1000, 100);
    }

    #[test]
    fn rejects_what_no_handler_could_parse() {
        for s in [
            "last tuesday",
            "",
            "2026-13-01T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:60:00Z",
            "2026-01-01T00:00:00.Z",
            "2026-01-01T00:00:00+99:00",
            "2026-01-01T00:00:00 UTC",
            "20260101T000000Z",
        ] {
            assert!(parse_rfc3339_millis(s).is_err(), "should reject {s:?}");
        }
    }

    #[test]
    fn leap_days_and_epoch_edges_survive() {
        assert_eq!(
            format_rfc3339_millis(parse_rfc3339_millis("2024-02-29T12:34:56.789Z").unwrap()),
            "2024-02-29T12:34:56.789Z"
        );
        assert_eq!(parse_rfc3339_millis("1970-01-01T00:00:00Z").unwrap(), 0);
        assert_eq!(
            format_rfc3339_millis(parse_rfc3339_millis("1969-12-31T23:59:59.500Z").unwrap()),
            "1969-12-31T23:59:59.500Z"
        );
    }
}
