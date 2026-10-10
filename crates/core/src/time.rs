//! Just enough calendar to read GitHub's timestamps and show them, without a date crate.

/// Seconds since the Unix epoch for an RFC 3339 UTC timestamp such as `2026-10-08T14:52:50Z`.
pub fn parse_rfc3339(text: &str) -> Option<i64> {
    let b = text.trim().as_bytes();
    if b.len() < 19 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') || !matches!(b.get(10), Some(b'T' | b't' | b' ')) {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> {
        let s = std::str::from_utf8(b.get(from..to)?).ok()?;
        if !s.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    };
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, s) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    // Offsets other than Z: GitHub always sends Z, but honour ±hh:mm if present.
    let tail = std::str::from_utf8(b.get(19..)?).ok()?;
    let tail = tail.strip_prefix('.').map_or(tail, |t| t.trim_start_matches(|c: char| c.is_ascii_digit()));
    let offset = match tail {
        "" | "Z" | "z" => 0,
        t if t.len() == 6 && (t.starts_with('+') || t.starts_with('-')) => {
            let sign = if t.starts_with('-') { -1 } else { 1 };
            let oh: i64 = t.get(1..3)?.parse().ok()?;
            let om: i64 = t.get(4..6)?.parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
        _ => return None,
    };
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s - offset)
}

/// Days since 1970-01-01 (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (year, month, day) for a Unix time.
pub fn civil_from_unix(secs: i64) -> (i64, u32, u32) {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, u32::try_from(m).unwrap_or(1), u32::try_from(d).unwrap_or(1))
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// `Oct 8, 2026`
pub fn format_date(secs: i64) -> String {
    let (y, m, d) = civil_from_unix(secs);
    let month = MONTHS.get(m.saturating_sub(1) as usize).copied().unwrap_or("?");
    format!("{month} {d}, {y}")
}

/// `just now`, `5 min ago`, `3 h ago`, `yesterday`, `4 days ago`, or the date.
pub fn format_ago(secs: i64, now: i64) -> String {
    let dt = now.saturating_sub(secs);
    match dt {
        i64::MIN..60 => "just now".into(),
        60..3600 => format!("{} min ago", dt / 60),
        3600..86_400 => format!("{} h ago", dt / 3600),
        86_400..172_800 => "yesterday".into(),
        172_800..2_592_000 => format!("{} days ago", dt / 86_400),
        _ => format_date(secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_github_timestamps() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2026-10-08T14:52:50Z"), Some(1_791_471_170));
        assert_eq!(parse_rfc3339("2026-10-08T16:52:50+02:00"), Some(1_791_471_170));
        assert_eq!(parse_rfc3339("2026-10-08T14:52:50.123Z"), Some(1_791_471_170));
        for bad in ["", "2026-10-08", "2026-13-08T00:00:00Z", "2026-10-08T14:52:50Q", "20x6-10-08T14:52:50Z"] {
            assert_eq!(parse_rfc3339(bad), None, "{bad}");
        }
    }

    #[test]
    fn shows_dates_people_read() {
        assert_eq!(format_date(1_791_471_170), "Oct 8, 2026");
        assert_eq!(civil_from_unix(951_782_400), (2000, 2, 29));
        let now = 1_791_471_170;
        assert_eq!(format_ago(now - 5, now), "just now");
        assert_eq!(format_ago(now - 600, now), "10 min ago");
        assert_eq!(format_ago(now - 7200, now), "2 h ago");
        assert_eq!(format_ago(now - 90_000, now), "yesterday");
        assert_eq!(format_ago(now - 5 * 86_400, now), "5 days ago");
        assert_eq!(format_ago(now - 90 * 86_400, now), "Jul 10, 2026");
    }
}
