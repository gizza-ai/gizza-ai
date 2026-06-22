//! gizza-ai/timezone-convert core — convert a naive date/time from one IANA
//! timezone to another, correctly handling daylight-saving-time (DST) rules.
//! Pure-Rust (`chrono` + `chrono-tz`), no clock, no I/O.
//!
//! The input wall-clock time (e.g. `2024-03-10 14:30`) is interpreted in the
//! `from` timezone, then re-expressed in the `to` timezone. Because both zones'
//! DST rules come from the bundled IANA tz database, the conversion is correct
//! across spring-forward / fall-back boundaries (e.g. the day the US "springs
//! forward" the offset to UTC changes by an hour, which this accounts for).

use chrono::{Datelike, LocalResult, NaiveDateTime, TimeZone, Weekday};
use chrono_tz::Tz;
use serde::Serialize;

/// The structured result of a timezone conversion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Converted {
    /// The source instant rendered in the `from` zone, ISO 8601 with offset
    /// (e.g. `2024-03-10T14:30:00-05:00`).
    pub from: String,
    /// The same instant rendered in the `to` zone, ISO 8601 with offset.
    pub to: String,
    /// Canonical IANA name of the source zone (e.g. `America/New_York`).
    pub from_zone: String,
    /// Canonical IANA name of the target zone.
    pub to_zone: String,
    /// UTC offset of the source instant, e.g. `-05:00`.
    pub from_offset: String,
    /// UTC offset of the target instant, e.g. `+09:00`.
    pub to_offset: String,
    /// Human-friendly target rendering, e.g. `Mon, 11 Mar 2024 04:30:00`.
    pub to_pretty: String,
    /// English weekday of the target instant (e.g. `Monday`).
    pub to_weekday: String,
    /// Whether the target instant falls in DST (summer time) for its zone.
    pub to_is_dst: bool,
    /// Difference target − source in whole hours (may be fractional zones; see
    /// `offset_diff_minutes` for exactness). Positive = target is ahead.
    pub offset_diff_hours: f64,
    /// Same difference expressed in minutes (handles 30/45-minute zones).
    pub offset_diff_minutes: i64,
    /// The instant as a Unix timestamp (seconds since the epoch, UTC).
    pub unix: i64,
}

/// Parse a wall-clock date/time string into a `NaiveDateTime`.
///
/// Accepts (whitespace-trimmed):
///   * `2024-03-10T14:30:00`, `2024-03-10 14:30:00`
///   * `2024-03-10T14:30`, `2024-03-10 14:30`
///   * `2024-03-10` (midnight assumed)
///
/// A trailing `Z` or numeric offset is rejected — the zone is given by the
/// `from` argument, not the string, to keep the meaning unambiguous.
fn parse_naive(s: &str) -> Result<NaiveDateTime, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("datetime is empty".into());
    }
    // Reject an embedded offset/zone: the from-zone is authoritative.
    let after_date = t.get(10..).unwrap_or("");
    if t.ends_with('Z')
        || after_date.contains('+')
        // a '-' in the time portion (position >= 11) signals an offset
        || after_date.trim_start().contains('-')
    {
        return Err(format!(
            "datetime {t:?} carries a timezone/offset; give a plain wall-clock \
             time (e.g. 2024-03-10 14:30) and set the 'from' zone instead"
        ));
    }

    let normalized = t.replacen('T', " ", 1);
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&normalized, fmt) {
            return Ok(dt);
        }
    }
    // Date-only -> midnight.
    if let Ok(d) = chrono::NaiveDate::parse_from_str(&normalized, "%Y-%m-%d") {
        return Ok(d.and_hms_opt(0, 0, 0).expect("midnight is valid"));
    }
    Err(format!(
        "could not parse datetime {t:?}; use ISO form like 2024-03-10 14:30 \
         or 2024-03-10T14:30:00"
    ))
}

/// Resolve an IANA timezone name (case-sensitive canonical form, e.g.
/// `America/New_York`, `Europe/London`, `UTC`). Returns a helpful error
/// listing a few examples on a miss.
fn parse_zone(name: &str, which: &str) -> Result<Tz, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(format!("{which} timezone is empty"));
    }
    n.parse::<Tz>().map_err(|_| {
        format!(
            "unknown {which} timezone {n:?}; use an IANA name like \
             'America/New_York', 'Europe/London', 'Asia/Tokyo', or 'UTC'"
        )
    })
}

fn offset_string(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let abs = seconds.abs();
    format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
}

fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

/// Convert `datetime` (a wall-clock time in `from`) into the `to` timezone.
///
/// DST is handled by `chrono-tz`. When a local time is *ambiguous* (the fall-back
/// hour that occurs twice) the earlier occurrence is used; when it is *skipped*
/// (the spring-forward gap that never happens) an error is returned naming the
/// gap, since no valid instant exists.
pub fn convert(datetime: &str, from: &str, to: &str) -> Result<Converted, String> {
    let naive = parse_naive(datetime)?;
    let from_tz = parse_zone(from, "source")?;
    let to_tz = parse_zone(to, "target")?;

    let from_dt = match from_tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        // Ambiguous (fall-back): pick the earlier offset, the common convention.
        LocalResult::Ambiguous(earlier, _later) => earlier,
        LocalResult::None => {
            return Err(format!(
                "{} does not exist in {from} — it falls in a daylight-saving \
                 spring-forward gap (the clock skips that hour)",
                naive.format("%Y-%m-%d %H:%M:%S")
            ));
        }
    };

    let to_dt = from_dt.with_timezone(&to_tz);

    use chrono::Offset;
    let from_off = from_dt.offset().fix().local_minus_utc();
    let to_off = to_dt.offset().fix().local_minus_utc();
    let diff_secs = (to_off - from_off) as i64;

    // DST detection: a zone is in DST when its current offset differs from its
    // standard (January-or-July, whichever is smaller) offset. chrono-tz exposes
    // the DST flag via the offset's `Display`, but the robust check is comparing
    // the instant's offset to the zone's standard offset.
    let to_is_dst = is_dst(&to_tz, &to_dt);

    Ok(Converted {
        from: from_dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        to: to_dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        from_zone: from_tz.name().to_string(),
        to_zone: to_tz.name().to_string(),
        from_offset: offset_string(from_off),
        to_offset: offset_string(to_off),
        to_pretty: to_dt.format("%a, %d %b %Y %H:%M:%S").to_string(),
        to_weekday: weekday_name(to_dt.weekday()).to_string(),
        to_is_dst,
        offset_diff_hours: diff_secs as f64 / 3600.0,
        offset_diff_minutes: diff_secs / 60,
        unix: to_dt.timestamp(),
    })
}

/// Whether `dt` (in zone `tz`) is in daylight-saving time. Compares the
/// instant's UTC offset to the zone's standard-time offset, which we take as the
/// smaller (in summer-DST zones) / matching offset observed in deep winter and
/// summer. We sample the zone in January and July of the same year and treat the
/// instant as DST when its offset equals the *larger* of the two sampled offsets
/// (and the two differ). For zones without DST the two samples are equal -> false.
fn is_dst(tz: &Tz, dt: &chrono::DateTime<Tz>) -> bool {
    use chrono::Offset;
    let year = dt.year();
    let off_at = |month: u32| -> i32 {
        let n = chrono::NaiveDate::from_ymd_opt(year, month, 15)
            .and_then(|d| d.and_hms_opt(12, 0, 0))
            .expect("mid-month noon is valid");
        match tz.from_local_datetime(&n) {
            LocalResult::Single(x) | LocalResult::Ambiguous(x, _) => {
                x.offset().fix().local_minus_utc()
            }
            LocalResult::None => 0,
        }
    };
    let jan = off_at(1);
    let jul = off_at(7);
    if jan == jul {
        return false; // no DST in this zone
    }
    let dst_offset = jan.max(jul); // DST = the larger (more east / less negative) offset
    let cur = dt.offset().fix().local_minus_utc();
    cur == dst_offset
}

/// Render `convert(...)` as a pretty JSON string (the web/page surface).
pub fn render(datetime: &str, from: &str, to: &str) -> Result<String, String> {
    let c = convert(datetime, from, to)?;
    serde_json::to_string_pretty(&c).map_err(|e| format!("serialize failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ny_to_tokyo_standard_time() {
        // 10 Jan 2024 14:30 EST (UTC-5) -> 11 Jan 2024 04:30 JST (UTC+9). 14h ahead.
        let c = convert("2024-01-10 14:30", "America/New_York", "Asia/Tokyo").unwrap();
        assert_eq!(c.from, "2024-01-10T14:30:00-05:00");
        assert_eq!(c.to, "2024-01-11T04:30:00+09:00");
        assert_eq!(c.to_zone, "Asia/Tokyo");
        assert_eq!(c.from_offset, "-05:00");
        assert_eq!(c.to_offset, "+09:00");
        assert_eq!(c.offset_diff_hours, 14.0);
        assert_eq!(c.offset_diff_minutes, 14 * 60);
        assert_eq!(c.to_weekday, "Thursday");
        assert!(!c.to_is_dst); // Tokyo has no DST
    }

    #[test]
    fn ny_summer_is_edt_dst() {
        // 10 Jul 2024: New York is on EDT (UTC-4), in DST.
        let c = convert("2024-07-10 09:00", "America/New_York", "UTC").unwrap();
        assert_eq!(c.from_offset, "-04:00");
        assert_eq!(c.to, "2024-07-10T13:00:00+00:00");
        // UTC is the *target*; to_is_dst reflects UTC (never DST).
        assert!(!c.to_is_dst);
        // But converting INTO New York summer flags DST:
        let c2 = convert("2024-07-10 13:00", "UTC", "America/New_York").unwrap();
        assert_eq!(c2.to_offset, "-04:00");
        assert!(c2.to_is_dst);
    }

    #[test]
    fn half_hour_zone_india() {
        // India Standard Time is UTC+5:30, no DST.
        let c = convert("2024-06-01 00:00", "UTC", "Asia/Kolkata").unwrap();
        assert_eq!(c.to, "2024-06-01T05:30:00+05:30");
        assert_eq!(c.to_offset, "+05:30");
        assert_eq!(c.offset_diff_minutes, 330);
        assert_eq!(c.offset_diff_hours, 5.5);
        assert!(!c.to_is_dst);
    }

    #[test]
    fn spring_forward_gap_is_rejected() {
        // 10 Mar 2024 02:30 does not exist in New York (clocks jump 2:00->3:00).
        let err = convert("2024-03-10 02:30", "America/New_York", "UTC").unwrap_err();
        assert!(err.contains("does not exist"), "got: {err}");
    }

    #[test]
    fn date_only_assumes_midnight() {
        let c = convert("2024-01-01", "UTC", "UTC").unwrap();
        assert_eq!(c.to, "2024-01-01T00:00:00+00:00");
    }

    #[test]
    fn rejects_input_with_embedded_offset() {
        let err = convert("2024-01-10T14:30:00Z", "UTC", "UTC").unwrap_err();
        assert!(err.contains("carries a timezone"), "got: {err}");
        let err2 = convert("2024-01-10T14:30:00+02:00", "UTC", "UTC").unwrap_err();
        assert!(err2.contains("carries a timezone"), "got: {err2}");
    }

    #[test]
    fn rejects_unknown_zone() {
        let err = convert("2024-01-10 14:30", "Mars/Olympus", "UTC").unwrap_err();
        assert!(err.contains("unknown source timezone"), "got: {err}");
        let err2 = convert("2024-01-10 14:30", "UTC", "Narnia").unwrap_err();
        assert!(err2.contains("unknown target timezone"), "got: {err2}");
    }

    #[test]
    fn rejects_garbage_datetime() {
        let err = convert("not a date", "UTC", "UTC").unwrap_err();
        assert!(err.contains("could not parse"), "got: {err}");
    }

    #[test]
    fn same_zone_is_identity() {
        let c = convert("2024-06-15 12:00:00", "Europe/London", "Europe/London").unwrap();
        assert_eq!(c.offset_diff_minutes, 0);
        assert_eq!(c.from, c.to);
    }

    #[test]
    fn render_emits_json() {
        let j = render("2024-01-10 14:30", "America/New_York", "Asia/Tokyo").unwrap();
        assert!(j.contains("\"to\""));
        assert!(j.contains("2024-01-11T04:30:00+09:00"));
    }
}
