//! gizza-ai/timezone-convert core — convert a naive date/time from one IANA
//! timezone to another, correctly handling daylight-saving-time (DST) rules.
//! Pure-Rust (`chrono` + `chrono-tz`), no clock, no I/O.
//!
//! The input wall-clock time (e.g. `2024-03-10 14:30`) is interpreted in the
//! `from` timezone, then re-expressed in the `to` timezone. Because both zones'
//! DST rules come from the bundled IANA tz database, the conversion is correct
//! across spring-forward / fall-back boundaries (e.g. the day the US "springs
//! forward" the offset to UTC changes by an hour, which this accounts for).

use chrono::{Datelike, LocalResult, NaiveDateTime, TimeZone, Timelike, Weekday};
use chrono_tz::Tz;
use serde::Serialize;

/// Individual target conversion detail.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TargetConversion {
    /// The same instant rendered in this target zone, ISO 8601 with offset.
    pub to: String,
    /// Canonical IANA name of this target zone.
    pub to_zone: String,
    /// UTC offset of this target instant, e.g. `+09:00`.
    pub to_offset: String,
    /// Human-friendly target rendering, e.g. `Mon, 11 Mar 2024 04:30:00`.
    pub to_pretty: String,
    /// English weekday of the target instant (e.g. `Monday`).
    pub to_weekday: String,
    /// Whether the target instant falls in DST (summer time) for its zone.
    pub to_is_dst: bool,
    /// Difference target − source in whole hours. Positive = target is ahead.
    pub offset_diff_hours: f64,
    /// Same difference expressed in minutes (handles 30/45-minute zones).
    pub offset_diff_minutes: i64,
    /// The instant as a Unix timestamp (seconds since the epoch, UTC).
    pub unix: i64,
}

/// Meeting planner hourly target detail.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannerTargetSlot {
    pub to_zone: String,
    pub to_time: String,
    pub to_hour: u32,
    pub to_status: String, // "Business", "Leisure", "Rest"
}

/// Meeting planner hour slot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannerSlot {
    pub hour_index: u32,
    pub from_time: String,
    pub from_hour: u32,
    pub from_status: String, // "Business", "Leisure", "Rest"
    pub targets: Vec<PlannerTargetSlot>,
}

/// The structured result of a timezone conversion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Converted {
    /// The source instant rendered in the `from` zone, ISO 8601 with offset
    /// (e.g. `2024-03-10T14:30:00-05:00`).
    pub from: String,
    /// The same instant rendered in the `to` zone, ISO 8601 with offset.
    /// In case of multiple targets, matches the first target.
    pub to: String,
    /// Canonical IANA name of the source zone (e.g. `America/New_York`).
    pub from_zone: String,
    /// Canonical IANA name of the target zone.
    /// In case of multiple targets, matches the first target.
    pub to_zone: String,
    /// UTC offset of the source instant, e.g. `-05:00`.
    pub from_offset: String,
    /// UTC offset of the target instant, e.g. `+09:00`.
    /// In case of multiple targets, matches the first target.
    pub to_offset: String,
    /// Human-friendly target rendering, e.g. `Mon, 11 Mar 2024 04:30:00`.
    /// In case of multiple targets, matches the first target.
    pub to_pretty: String,
    /// English weekday of the target instant (e.g. `Monday`).
    /// In case of multiple targets, matches the first target.
    pub to_weekday: String,
    /// Whether the target instant falls in DST (summer time) for its zone.
    /// In case of multiple targets, matches the first target.
    pub to_is_dst: bool,
    /// Difference target − source in whole hours. Positive = target is ahead.
    /// In case of multiple targets, matches the first target.
    pub offset_diff_hours: f64,
    /// Same difference expressed in minutes (handles 30/45-minute zones).
    /// In case of multiple targets, matches the first target.
    pub offset_diff_minutes: i64,
    /// The instant as a Unix timestamp (seconds since the epoch, UTC).
    pub unix: i64,
    /// Details for all targets.
    pub targets: Vec<TargetConversion>,
    /// Hour-by-hour planner comparison.
    pub meeting_planner: Vec<PlannerSlot>,
}

fn normalize_datetime_str(s: &str) -> String {
    let mut t = s.trim().to_string();
    t = t.replace('/', "-");
    t = t.replacen('T', " ", 1);
    
    // Handle AM/PM spacing
    let mut upper = t.to_uppercase();
    if let Some(idx) = upper.find("AM") {
        if idx > 0 && upper.as_bytes()[idx - 1].is_ascii_digit() {
            upper.insert(idx, ' ');
        }
    } else if let Some(idx) = upper.find("PM") {
        if idx > 0 && upper.as_bytes()[idx - 1].is_ascii_digit() {
            upper.insert(idx, ' ');
        }
    }
    upper
}

/// Parse a wall-clock date/time string into a `NaiveDateTime`.
///
/// Accepts (whitespace-trimmed):
///   * `2024-03-10T14:30:00`, `2024-03-10 14:30:00`
///   * `2024-03-10T14:30`, `2024-03-10 14:30`
///   * `2024-03-10` (midnight assumed)
///   * Slashes: `2024/03/10 14:30`
///   * AM/PM forms: `2024-03-10 2:30 PM`, `2024-03-10 2:30PM`
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

    let normalized = normalize_datetime_str(t);
    for fmt in [
        "%Y-%m-%d %I:%M:%S %p",
        "%Y-%m-%d %I:%M %p",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
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

fn hour_status(hour: u32) -> &'static str {
    if hour >= 22 || hour < 6 {
        "Rest"
    } else if hour >= 9 && hour < 17 {
        "Business"
    } else {
        "Leisure"
    }
}

/// Convert `datetime` (a wall-clock time in `from`) into target timezones `to` (comma-separated list).
///
/// DST is handled by `chrono-tz`. When a local time is *ambiguous* (the fall-back
/// hour that occurs twice) the earlier occurrence is used; when it is *skipped*
/// (the spring-forward gap that never happens) an error is returned naming the
/// gap, since no valid instant exists.
pub fn convert(datetime: &str, from: &str, to: &str) -> Result<Converted, String> {
    let naive = parse_naive(datetime)?;
    let from_tz = parse_zone(from, "source")?;

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

    let target_zones_raw: Vec<&str> = to.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if target_zones_raw.is_empty() {
        return Err("target timezone is empty".into());
    }

    let mut targets = Vec::new();
    let mut resolved_targets = Vec::new();

    use chrono::Offset;
    let from_off = from_dt.offset().fix().local_minus_utc();

    for tz_name in &target_zones_raw {
        let to_tz = parse_zone(tz_name, "target")?;
        let to_dt = from_dt.with_timezone(&to_tz);
        let to_off = to_dt.offset().fix().local_minus_utc();
        let diff_secs = (to_off - from_off) as i64;
        let to_is_dst = is_dst(&to_tz, &to_dt);

        targets.push(TargetConversion {
            to: to_dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
            to_zone: to_tz.name().to_string(),
            to_offset: offset_string(to_off),
            to_pretty: to_dt.format("%a, %d %b %Y %H:%M:%S").to_string(),
            to_weekday: weekday_name(to_dt.weekday()).to_string(),
            to_is_dst,
            offset_diff_hours: diff_secs as f64 / 3600.0,
            offset_diff_minutes: diff_secs / 60,
            unix: to_dt.timestamp(),
        });
        resolved_targets.push(to_tz);
    }

    // Meeting planner generation
    let date = from_dt.date_naive();
    let base_naive = date.and_hms_opt(0, 0, 0).unwrap();
    let start_of_day = match from_tz.from_local_datetime(&base_naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(earlier, _) => earlier,
        LocalResult::None => {
            let alt_naive = date.and_hms_opt(1, 0, 0).unwrap();
            match from_tz.from_local_datetime(&alt_naive) {
                LocalResult::Single(dt) => dt,
                LocalResult::Ambiguous(earlier, _) => earlier,
                LocalResult::None => from_dt.clone(),
            }
        }
    };

    let mut meeting_planner = Vec::new();
    for h in 0..24 {
        let slot_dt = start_of_day + chrono::Duration::hours(h as i64);
        let from_hour = slot_dt.hour();
        let from_status = hour_status(from_hour).to_string();

        let mut planner_targets = Vec::new();
        for to_tz in &resolved_targets {
            let target_slot_dt = slot_dt.with_timezone(to_tz);
            let to_hour = target_slot_dt.hour();
            let to_status = hour_status(to_hour).to_string();
            planner_targets.push(PlannerTargetSlot {
                to_zone: to_tz.name().to_string(),
                to_time: target_slot_dt.format("%H:%M").to_string(),
                to_hour,
                to_status,
            });
        }

        meeting_planner.push(PlannerSlot {
            hour_index: h,
            from_time: slot_dt.format("%H:%M").to_string(),
            from_hour,
            from_status,
            targets: planner_targets,
        });
    }

    let first = &targets[0];

    Ok(Converted {
        from: from_dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        to: first.to.clone(),
        from_zone: from_tz.name().to_string(),
        to_zone: first.to_zone.clone(),
        from_offset: offset_string(from_off),
        to_offset: first.to_offset.clone(),
        to_pretty: first.to_pretty.clone(),
        to_weekday: first.to_weekday.clone(),
        to_is_dst: first.to_is_dst,
        offset_diff_hours: first.offset_diff_hours,
        offset_diff_minutes: first.offset_diff_minutes,
        unix: first.unix,
        targets,
        meeting_planner,
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

    #[test]
    fn lenient_parsing_am_pm_and_slashes() {
        let c1 = convert("2024/03/10 2:30 PM", "America/New_York", "UTC").unwrap();
        assert_eq!(c1.from, "2024-03-10T14:30:00-04:00");

        let c2 = convert("2024/03/10 2:30PM", "America/New_York", "UTC").unwrap();
        assert_eq!(c2.from, "2024-03-10T14:30:00-04:00");

        let c3 = convert("2024-03-10 02:30:00 pm", "America/New_York", "UTC").unwrap();
        assert_eq!(c3.from, "2024-03-10T14:30:00-04:00");
    }

    #[test]
    fn multi_target_conversion() {
        let c = convert("2024-01-10 14:30", "America/New_York", "Asia/Tokyo, Europe/London").unwrap();
        assert_eq!(c.targets.len(), 2);
        assert_eq!(c.targets[0].to_zone, "Asia/Tokyo");
        assert_eq!(c.targets[1].to_zone, "Europe/London");

        // Top level corresponds to first target (Tokyo)
        assert_eq!(c.to_zone, "Asia/Tokyo");
        assert_eq!(c.to_offset, "+09:00");

        // Check meeting planner
        assert_eq!(c.meeting_planner.len(), 24);
        assert_eq!(c.meeting_planner[0].hour_index, 0);
        assert_eq!(c.meeting_planner[0].targets.len(), 2);
    }
}
