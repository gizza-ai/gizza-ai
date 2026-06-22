//! cron-next-runs core — compute the next N fire times for a cron expression.
//!
//! Pure compute (no external deps): parses a standard 5-field cron expression
//! (`minute hour day-of-month month day-of-week`), optionally with a leading
//! 6th seconds field, plus the common `@`-shortcuts (`@hourly`, `@daily`, …),
//! and walks forward minute-by-minute (or second-by-second when a seconds field
//! is present) from an explicit base timestamp to find the next matching times.
//!
//! All time is treated as UTC and the base timestamp is supplied by the caller
//! (chat/CLI use the real clock, the page uses `Date.now()`), so the core stays
//! deterministic and unit-testable. Each surface passes the base as a Unix epoch
//! second.
//!
//! Supported field syntax: `*`, `N`, `A-B` ranges, `A-B/S` and `*/S` steps,
//! comma lists `A,B,C`, three-letter month names (`JAN`..`DEC`) and weekday
//! names (`SUN`..`SAT`). Day-of-week accepts both `0` and `7` for Sunday.
//! When BOTH day-of-month and day-of-week are restricted (neither is `*`), a
//! timestamp matches if EITHER matches (the standard Vixie-cron behavior).

/// A parsed cron schedule: the sorted allowed-values per field.
struct Schedule {
    seconds: Vec<u8>, // 0-59
    minutes: Vec<u8>, // 0-59
    hours: Vec<u8>,   // 0-23
    doms: Vec<u8>,    // 1-31
    months: Vec<u8>,  // 1-12
    dows: Vec<u8>,    // 0-6 (Sunday=0)
    dom_restricted: bool,
    dow_restricted: bool,
    has_seconds: bool,
    /// The normalized 5/6 field strings (after @-shortcut expansion), kept for
    /// the human-readable description.
    raw_fields: Vec<String>,
}

const MONTH_NAMES: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const DOW_NAMES: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

/// Compute the next `count` fire times for `expression` strictly after `base`
/// (a Unix epoch second, UTC). `format` is `"text"` (default, aligned report)
/// or `"json"` (machine-readable). Returns an error message on invalid input.
pub fn next_runs(expression: &str, base: i64, count: u32, format: &str) -> Result<String, String> {
    let expr = expression.trim();
    if expr.is_empty() {
        return Err("expression is empty: provide a cron expression like '*/15 * * * *'".into());
    }
    if count == 0 {
        return Err("count must be at least 1".into());
    }
    if count > 100 {
        return Err("count must be at most 100".into());
    }
    let fmt = {
        let f = format.trim();
        if f.is_empty() {
            "text".to_string()
        } else {
            f.to_ascii_lowercase()
        }
    };
    if fmt != "text" && fmt != "json" {
        return Err(format!("invalid format {format:?}: expected 'text' or 'json'"));
    }

    let sched = parse(expr)?;
    let runs = sched.next_n(base, count)?;

    Ok(match fmt.as_str() {
        "json" => to_json(expr, base, &sched, &runs),
        _ => to_text(expr, base, &sched, &runs),
    })
}

/// Parse an RFC-3339 / ISO-8601 UTC timestamp (e.g. `2020-01-01T00:00:00Z`,
/// `2020-01-01 00:00`, or a bare date `2020-01-01`) into a Unix epoch second.
/// A trailing `Z` is optional; an explicit numeric offset is rejected (treat
/// input as UTC). Also accepts a plain integer = Unix epoch seconds.
pub fn parse_timestamp(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("timestamp is empty".into());
    }
    // Plain integer epoch.
    if let Ok(epoch) = s.parse::<i64>() {
        return Ok(epoch);
    }
    let s = s.strip_suffix('Z').or_else(|| s.strip_suffix('z')).unwrap_or(s);
    // Split date and optional time on 'T' or a space.
    let (date_part, time_part) = match s.split_once(['T', 't', ' ']) {
        Some((d, t)) => (d, t),
        None => (s, ""),
    };
    let dp: Vec<&str> = date_part.split('-').collect();
    if dp.len() != 3 {
        return Err(format!(
            "invalid timestamp {s:?}: expected YYYY-MM-DD[THH:MM[:SS]][Z]"
        ));
    }
    let year: i64 = dp[0]
        .parse()
        .map_err(|_| format!("invalid year in timestamp {s:?}"))?;
    let month: u8 = dp[1]
        .parse()
        .map_err(|_| format!("invalid month in timestamp {s:?}"))?;
    let day: u8 = dp[2]
        .parse()
        .map_err(|_| format!("invalid day in timestamp {s:?}"))?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("month/day out of range in timestamp {s:?}"));
    }
    let (mut hour, mut minute, mut second) = (0u8, 0u8, 0u8);
    if !time_part.is_empty() {
        let tp: Vec<&str> = time_part.split(':').collect();
        if tp.is_empty() || tp.len() > 3 {
            return Err(format!("invalid time in timestamp {s:?}"));
        }
        hour = tp[0]
            .parse()
            .map_err(|_| format!("invalid hour in timestamp {s:?}"))?;
        if tp.len() >= 2 {
            minute = tp[1]
                .parse()
                .map_err(|_| format!("invalid minute in timestamp {s:?}"))?;
        }
        if tp.len() == 3 {
            // Drop any fractional seconds.
            let sec = tp[2].split('.').next().unwrap_or(tp[2]);
            second = sec
                .parse()
                .map_err(|_| format!("invalid second in timestamp {s:?}"))?;
        }
        if hour > 23 || minute > 59 || second > 60 {
            return Err(format!("time out of range in timestamp {s:?}"));
        }
    }
    Ok(epoch_from_civil(year, month, day, hour, minute, second.min(59)))
}

/// Inverse of `civil_from_epoch`: days-from-civil (Howard Hinnant) + time.
fn epoch_from_civil(year: i64, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m = month as i64;
    let d = day as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64
}

fn parse(expr: &str) -> Result<Schedule, String> {
    // Expand @-shortcuts to a 5-field expression.
    let normalized = match expr.to_ascii_lowercase().as_str() {
        "@yearly" | "@annually" => "0 0 1 1 *".to_string(),
        "@monthly" => "0 0 1 * *".to_string(),
        "@weekly" => "0 0 * * 0".to_string(),
        "@daily" | "@midnight" => "0 0 * * *".to_string(),
        "@hourly" => "0 * * * *".to_string(),
        s if s.starts_with('@') => {
            return Err(format!(
                "unknown shortcut {expr:?}: supported shortcuts are @yearly, @annually, \
@monthly, @weekly, @daily, @midnight, @hourly"
            ))
        }
        _ => expr.to_string(),
    };

    let fields: Vec<&str> = normalized.split_whitespace().collect();
    let (has_seconds, sec_field, rest) = match fields.len() {
        5 => (false, None, &fields[..]),
        6 => (true, Some(fields[0]), &fields[1..]),
        n => {
            return Err(format!(
                "expected 5 fields (minute hour day-of-month month day-of-week), or 6 with a \
leading seconds field; got {n}: {expr:?}"
            ))
        }
    };

    let seconds = match sec_field {
        Some(f) => parse_field(f, 0, 59, "seconds", None)?,
        None => vec![0],
    };
    let minutes = parse_field(rest[0], 0, 59, "minute", None)?;
    let hours = parse_field(rest[1], 0, 23, "hour", None)?;
    let doms = parse_field(rest[2], 1, 31, "day-of-month", None)?;
    let months = parse_field(rest[3], 1, 12, "month", Some(&MONTH_NAMES))?;
    let dows_raw = parse_field(rest[4], 0, 7, "day-of-week", Some(&DOW_NAMES))?;
    // Normalize 7 -> 0 (both are Sunday) and dedup.
    let mut dows: Vec<u8> = dows_raw
        .into_iter()
        .map(|d| if d == 7 { 0 } else { d })
        .collect();
    dows.sort_unstable();
    dows.dedup();

    let dom_restricted = rest[2].trim() != "*";
    let dow_restricted = rest[4].trim() != "*";

    Ok(Schedule {
        seconds,
        minutes,
        hours,
        doms,
        months,
        dows,
        dom_restricted,
        dow_restricted,
        has_seconds,
        raw_fields: fields.iter().map(|f| f.to_string()).collect(),
    })
}

/// Parse one cron field into a sorted, deduped vec of allowed values.
fn parse_field(
    field: &str,
    min: u8,
    max: u8,
    name: &str,
    names: Option<&[&str]>,
) -> Result<Vec<u8>, String> {
    let field = field.trim();
    if field.is_empty() {
        return Err(format!("{name} field is empty"));
    }
    let mut out: Vec<u8> = Vec::new();
    for part in field.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("{name}: empty list element in {field:?}"));
        }
        // Split optional /step.
        let (range_spec, step) = match part.split_once('/') {
            Some((r, s)) => {
                let step: u32 = s
                    .trim()
                    .parse()
                    .map_err(|_| format!("{name}: invalid step {s:?} in {part:?}"))?;
                if step == 0 {
                    return Err(format!("{name}: step must be > 0 in {part:?}"));
                }
                (r, step)
            }
            None => (part, 1),
        };

        let (lo, hi) = if range_spec == "*" {
            (min, max)
        } else if let Some((a, b)) = range_spec.split_once('-') {
            let a = resolve_value(a, min, max, name, names)?;
            let b = resolve_value(b, min, max, name, names)?;
            if a > b {
                return Err(format!(
                    "{name}: range start {a} is greater than end {b} in {part:?}"
                ));
            }
            (a, b)
        } else {
            let v = resolve_value(range_spec, min, max, name, names)?;
            // A bare value with a step means "from v to max, stepping".
            if step > 1 {
                (v, max)
            } else {
                (v, v)
            }
        };

        let mut v = lo as u32;
        while v <= hi as u32 {
            out.push(v as u8);
            v += step;
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err(format!("{name}: no values matched in {field:?}"));
    }
    Ok(out)
}

/// Resolve a single token to a numeric value, accepting 3-letter names if given.
fn resolve_value(
    token: &str,
    min: u8,
    max: u8,
    name: &str,
    names: Option<&[&str]>,
) -> Result<u8, String> {
    let token = token.trim();
    if let Some(names) = names {
        let lc = token.to_ascii_lowercase();
        if let Some(idx) = names.iter().position(|n| *n == lc) {
            // For months names[0]=jan -> value 1; for dow names[0]=sun -> value 0.
            let val = if min == 1 { idx as u8 + 1 } else { idx as u8 };
            return Ok(val);
        }
    }
    let v: u32 = token
        .parse()
        .map_err(|_| format!("{name}: invalid value {token:?}"))?;
    if v < min as u32 || v > max as u32 {
        return Err(format!("{name}: value {v} out of range {min}-{max}"));
    }
    Ok(v as u8)
}

impl Schedule {
    /// Walk forward from `base` (exclusive) and collect the next `count` fire
    /// times as Unix epoch seconds.
    fn next_n(&self, base: i64, count: u32) -> Result<Vec<i64>, String> {
        let mut out = Vec::with_capacity(count as usize);
        let step = if self.has_seconds { 1 } else { 60 };
        let mut t = if self.has_seconds {
            base + 1
        } else {
            // Align up to the next whole minute boundary strictly after base.
            (base - base.rem_euclid(60)) + 60
        };

        // Bound the search so a never-matching expression (e.g. Feb 30) can't
        // loop forever. Feb-29 fires only on leap years, and the gap between two
        // consecutive Feb-29 occurrences measured from an arbitrary base can be
        // up to ~8 years (e.g. base just after a Feb 29 -> next is 4 years on,
        // and the second is another 4 years), so use an 8-year window with slack.
        let limit = base + 8 * 366 * 86_400 + 86_400;
        while out.len() < count as usize {
            if t > limit {
                if out.is_empty() {
                    return Err(
                        "this cron expression never fires (e.g. an impossible date like Feb 30)"
                            .into(),
                    );
                }
                break;
            }
            if self.matches(t) {
                out.push(t);
            }
            t += step;
        }
        Ok(out)
    }

    fn matches(&self, epoch: i64) -> bool {
        let dt = civil_from_epoch(epoch);
        if self.has_seconds && !self.seconds.contains(&dt.second) {
            return false;
        }
        if !self.minutes.contains(&dt.minute) {
            return false;
        }
        if !self.hours.contains(&dt.hour) {
            return false;
        }
        if !self.months.contains(&dt.month) {
            return false;
        }
        // Day-of-month / day-of-week combination (Vixie rule).
        let dom_ok = self.doms.contains(&dt.day);
        let dow_ok = self.dows.contains(&dt.weekday);
        match (self.dom_restricted, self.dow_restricted) {
            (true, true) => dom_ok || dow_ok,
            (true, false) => dom_ok,
            (false, true) => dow_ok,
            (false, false) => true,
        }
    }

    /// A plain-English summary of the schedule, e.g. "Every 15 minutes" or
    /// "At 09:00, Monday through Friday". Best-effort: it covers the common
    /// shapes and otherwise falls back to listing each field.
    fn describe(&self) -> String {
        // Field index offset when a seconds field is present.
        let off = if self.has_seconds { 1 } else { 0 };
        let min_f = self.raw_fields[off].as_str();
        let hour_f = self.raw_fields[off + 1].as_str();

        let mut parts: Vec<String> = Vec::new();

        // --- Time-of-day phrase ---
        if let Some(step) = step_of(min_f) {
            if hour_f == "*" {
                parts.push(format!(
                    "Every {} minute{}",
                    step,
                    if step == 1 { "" } else { "s" }
                ));
            } else {
                parts.push(format!(
                    "Every {} minute{} past {}",
                    step,
                    if step == 1 { "" } else { "s" },
                    describe_hours(hour_f, &self.hours)
                ));
            }
        } else if min_f == "*" && hour_f == "*" {
            parts.push(if self.has_seconds && self.raw_fields[0] != "*" {
                "Every minute".to_string()
            } else {
                "Every minute".to_string()
            });
        } else if self.minutes.len() == 1 && self.hours.len() == 1 {
            parts.push(format!(
                "At {:02}:{:02}",
                self.hours[0], self.minutes[0]
            ));
        } else if hour_f == "*" {
            parts.push(format!(
                "At minute {} of every hour",
                join_list(&self.minutes)
            ));
        } else {
            parts.push(format!(
                "At minute {} past {}",
                join_list(&self.minutes),
                describe_hours(hour_f, &self.hours)
            ));
        }

        // --- Day phrase ---
        if self.dow_restricted {
            parts.push(describe_dow(&self.dows));
        }
        if self.dom_restricted {
            let conj = if self.dow_restricted { "and on" } else { "on" };
            parts.push(format!("{conj} day-of-month {}", join_list(&self.doms)));
        }

        // --- Month phrase ---
        if self.raw_fields[off + 3] != "*" {
            parts.push(format!("in {}", describe_months(&self.months)));
        }

        format!("{} (UTC)", parts.join(", "))
    }
}

/// If `field` is a pure `*/N` step, return N.
fn step_of(field: &str) -> Option<u32> {
    field.strip_prefix("*/").and_then(|s| s.parse::<u32>().ok())
}

fn describe_hours(raw: &str, hours: &[u8]) -> String {
    if raw == "*" {
        "every hour".to_string()
    } else if hours.len() == 1 {
        format!("{:02}:00", hours[0])
    } else {
        format!("hours {}", join_list(hours))
    }
}

fn join_list(vals: &[u8]) -> String {
    let s: Vec<String> = vals.iter().map(|v| v.to_string()).collect();
    match s.len() {
        0 => String::new(),
        1 => s[0].clone(),
        2 => format!("{} and {}", s[0], s[1]),
        _ => format!("{} and {}", s[..s.len() - 1].join(", "), s[s.len() - 1]),
    }
}

const WEEKDAY_SHORT: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTH_LONG: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];

fn describe_dow(dows: &[u8]) -> String {
    // Detect a contiguous run (e.g. Mon-Fri).
    if dows.len() >= 2 {
        let contiguous = dows.windows(2).all(|w| w[1] == w[0] + 1);
        if contiguous {
            return format!(
                "{} through {}",
                WEEKDAY_SHORT[dows[0] as usize],
                WEEKDAY_SHORT[dows[dows.len() - 1] as usize]
            );
        }
    }
    let names: Vec<String> = dows.iter().map(|d| WEEKDAY_SHORT[*d as usize].to_string()).collect();
    format!("on {}", join_names(&names))
}

fn describe_months(months: &[u8]) -> String {
    if months.len() >= 2 {
        let contiguous = months.windows(2).all(|w| w[1] == w[0] + 1);
        if contiguous {
            return format!(
                "{} through {}",
                MONTH_LONG[(months[0] - 1) as usize],
                MONTH_LONG[(months[months.len() - 1] - 1) as usize]
            );
        }
    }
    let names: Vec<String> = months.iter().map(|m| MONTH_LONG[(*m - 1) as usize].to_string()).collect();
    join_names(&names)
}

fn join_names(names: &[String]) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        2 => format!("{} and {}", names[0], names[1]),
        _ => format!("{} and {}", names[..names.len() - 1].join(", "), names[names.len() - 1]),
    }
}

/// A civil UTC datetime.
struct Civil {
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    weekday: u8, // 0=Sunday
}

/// Convert a Unix epoch second to a civil UTC datetime (proleptic Gregorian).
/// Based on Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_epoch(epoch: i64) -> Civil {
    let days = epoch.div_euclid(86_400);
    let secs_of_day = epoch.rem_euclid(86_400);
    let hour = (secs_of_day / 3600) as u8;
    let minute = ((secs_of_day % 3600) / 60) as u8;
    let second = (secs_of_day % 60) as u8;

    // weekday: 1970-01-01 was a Thursday. 0=Sunday.
    let weekday = (((days % 7) + 4) % 7 + 7) % 7;
    let weekday = weekday as u8;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8; // [1, 12]
    let year = if month <= 2 { y + 1 } else { y };

    Civil {
        year,
        month,
        day,
        hour,
        minute,
        second,
        weekday,
    }
}

fn fmt_iso(epoch: i64) -> String {
    let c = civil_from_epoch(epoch);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

const WEEKDAY_LONG: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

fn fmt_human(epoch: i64) -> String {
    let c = civil_from_epoch(epoch);
    format!("{} {}", WEEKDAY_LONG[c.weekday as usize], fmt_iso(epoch))
}

fn to_text(expr: &str, base: i64, sched: &Schedule, runs: &[i64]) -> String {
    let mut s = String::new();
    s.push_str(&format!("Expression:  {expr}\n"));
    s.push_str(&format!("Schedule:    {}\n", sched.describe()));
    s.push_str(&format!("After (UTC): {}\n", fmt_human(base)));
    s.push_str(&format!("Next {} run(s):\n", runs.len()));
    for (i, r) in runs.iter().enumerate() {
        s.push_str(&format!("  {:>2}. {}\n", i + 1, fmt_human(*r)));
    }
    s.trim_end().to_string()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn to_json(expr: &str, base: i64, sched: &Schedule, runs: &[i64]) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"expression\": \"{}\",\n", json_escape(expr)));
    s.push_str(&format!("  \"description\": \"{}\",\n", json_escape(&sched.describe())));
    s.push_str(&format!("  \"has_seconds\": {},\n", sched.has_seconds));
    s.push_str(&format!("  \"after\": \"{}\",\n", fmt_iso(base)));
    s.push_str(&format!("  \"after_epoch\": {base},\n"));
    s.push_str("  \"next\": [\n");
    for (i, r) in runs.iter().enumerate() {
        let c = civil_from_epoch(*r);
        s.push_str("    {\n");
        s.push_str(&format!("      \"iso\": \"{}\",\n", fmt_iso(*r)));
        s.push_str(&format!("      \"epoch\": {r},\n"));
        s.push_str(&format!(
            "      \"weekday\": \"{}\"\n",
            WEEKDAY_LONG[c.weekday as usize]
        ));
        s.push_str(if i + 1 == runs.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    s.push_str("  ]\n");
    s.push('}');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2020-01-01T00:00:00Z = 1577836800. A Wednesday.
    const Y2020: i64 = 1_577_836_800;

    fn iso_runs(expr: &str, base: i64, n: u32) -> Vec<String> {
        let out = next_runs(expr, base, n, "json").unwrap();
        out.lines()
            .filter_map(|l| {
                let l = l.trim();
                l.strip_prefix("\"iso\": \"")
                    .map(|r| r.trim_end_matches("\",").trim_end_matches('"').to_string())
            })
            .collect()
    }

    #[test]
    fn every_15_minutes() {
        let runs = iso_runs("*/15 * * * *", Y2020, 4);
        assert_eq!(
            runs,
            vec![
                "2020-01-01T00:15:00Z",
                "2020-01-01T00:30:00Z",
                "2020-01-01T00:45:00Z",
                "2020-01-01T01:00:00Z",
            ]
        );
    }

    #[test]
    fn daily_at_midnight() {
        let runs = iso_runs("0 0 * * *", Y2020, 2);
        assert_eq!(runs, vec!["2020-01-02T00:00:00Z", "2020-01-03T00:00:00Z"]);
    }

    #[test]
    fn at_specific_minute_hour() {
        // 30 9 * * * -> 09:30 daily. Base is midnight, so first is same day.
        let runs = iso_runs("30 9 * * *", Y2020, 1);
        assert_eq!(runs, vec!["2020-01-01T09:30:00Z"]);
    }

    #[test]
    fn shortcut_daily_equals_midnight() {
        let a = iso_runs("@daily", Y2020, 3);
        let b = iso_runs("0 0 * * *", Y2020, 3);
        assert_eq!(a, b);
    }

    #[test]
    fn shortcut_hourly() {
        let runs = iso_runs("@hourly", Y2020, 2);
        assert_eq!(runs, vec!["2020-01-01T01:00:00Z", "2020-01-01T02:00:00Z"]);
    }

    #[test]
    fn weekday_by_name() {
        // Every Monday at 00:00. 2020-01-06 is the first Monday after 2020-01-01.
        let runs = iso_runs("0 0 * * MON", Y2020, 1);
        assert_eq!(runs, vec!["2020-01-06T00:00:00Z"]);
    }

    #[test]
    fn weekday_sunday_zero_and_seven_equivalent() {
        let a = iso_runs("0 0 * * 0", Y2020, 2);
        let b = iso_runs("0 0 * * 7", Y2020, 2);
        assert_eq!(a, b);
        // First Sunday after 2020-01-01 (Wed) is 2020-01-05.
        assert_eq!(a[0], "2020-01-05T00:00:00Z");
    }

    #[test]
    fn month_range_and_names() {
        // At 00:00 on the 1st of Jun, Jul, Aug.
        let runs = iso_runs("0 0 1 JUN-AUG *", Y2020, 3);
        assert_eq!(
            runs,
            vec![
                "2020-06-01T00:00:00Z",
                "2020-07-01T00:00:00Z",
                "2020-08-01T00:00:00Z",
            ]
        );
    }

    #[test]
    fn dom_or_dow_union() {
        // 0 0 13 * FRI -> midnight on the 13th OR any Friday (Vixie union).
        // After 2020-01-01 (Wed): first Friday is 2020-01-03, then 2020-01-10.
        let runs = iso_runs("0 0 13 * FRI", Y2020, 2);
        assert_eq!(runs, vec!["2020-01-03T00:00:00Z", "2020-01-10T00:00:00Z"]);
    }

    #[test]
    fn seconds_field() {
        // Every 30 seconds.
        let runs = iso_runs("*/30 * * * * *", Y2020, 3);
        assert_eq!(
            runs,
            vec![
                "2020-01-01T00:00:30Z",
                "2020-01-01T00:01:00Z",
                "2020-01-01T00:01:30Z",
            ]
        );
    }

    #[test]
    fn list_of_minutes() {
        let runs = iso_runs("0,15,45 * * * *", Y2020, 3);
        assert_eq!(
            runs,
            vec![
                "2020-01-01T00:15:00Z",
                "2020-01-01T00:45:00Z",
                "2020-01-01T01:00:00Z",
            ]
        );
    }

    #[test]
    fn leap_day_feb_29() {
        // 2020 is a leap year. Next 2020-02-29 00:00 after Y2020.
        let runs = iso_runs("0 0 29 2 *", Y2020, 2);
        assert_eq!(runs[0], "2020-02-29T00:00:00Z");
        // 2021, 2022, 2023 have no Feb 29; next is 2024.
        assert_eq!(runs[1], "2024-02-29T00:00:00Z");
    }

    #[test]
    fn never_fires_errors() {
        // Feb 30 never exists.
        let err = next_runs("0 0 30 2 *", Y2020, 1, "text").unwrap_err();
        assert!(err.contains("never fires"), "{err}");
    }

    #[test]
    fn text_output_shape() {
        let out = next_runs("0 0 * * *", Y2020, 1, "text").unwrap();
        assert!(out.contains("Expression:  0 0 * * *"));
        assert!(out.contains("Schedule:    At 00:00 (UTC)"), "{out}");
        assert!(out.contains("Next 1 run(s):"));
        assert!(out.contains("2020-01-02T00:00:00Z"));
    }

    fn describe(expr: &str) -> String {
        let s = parse(expr).unwrap();
        s.describe()
    }

    #[test]
    fn descriptions() {
        assert_eq!(describe("*/15 * * * *"), "Every 15 minutes (UTC)");
        assert_eq!(describe("0 0 * * *"), "At 00:00 (UTC)");
        assert_eq!(describe("30 9 * * *"), "At 09:30 (UTC)");
        assert_eq!(
            describe("0 9 * * MON-FRI"),
            "At 09:00, Monday through Friday (UTC)"
        );
        assert_eq!(describe("@hourly"), "At minute 0 of every hour (UTC)");
        assert_eq!(
            describe("0 0 1 * *"),
            "At 00:00, on day-of-month 1 (UTC)"
        );
        assert_eq!(
            describe("0 0 1 JUN-AUG *"),
            "At 00:00, on day-of-month 1, in June through August (UTC)"
        );
        assert_eq!(
            describe("0 0 13 * FRI"),
            "At 00:00, on Friday, and on day-of-month 13 (UTC)"
        );
    }

    #[test]
    fn json_includes_description() {
        let out = next_runs("*/15 * * * *", Y2020, 1, "json").unwrap();
        assert!(out.contains("\"description\": \"Every 15 minutes (UTC)\""), "{out}");
    }

    #[test]
    fn rejects_empty_and_bad_format() {
        assert!(next_runs("", Y2020, 1, "text").is_err());
        assert!(next_runs("0 0 * * *", Y2020, 1, "xml").is_err());
        assert!(next_runs("0 0 * * *", Y2020, 0, "text").is_err());
        assert!(next_runs("0 0 * * *", Y2020, 101, "text").is_err());
    }

    #[test]
    fn rejects_wrong_field_count() {
        assert!(next_runs("* * *", Y2020, 1, "text").is_err());
        assert!(next_runs("* * * * * * *", Y2020, 1, "text").is_err());
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(next_runs("99 * * * *", Y2020, 1, "text").is_err());
        assert!(next_runs("* 25 * * *", Y2020, 1, "text").is_err());
        assert!(next_runs("0 0 * 13 *", Y2020, 1, "text").is_err());
    }

    #[test]
    fn range_with_step() {
        // 0-30/10 in minute field -> 0,10,20,30.
        let runs = iso_runs("0-30/10 0 * * *", Y2020, 4);
        // After midnight base: 00:10, 00:20, 00:30, then next day 00:00.
        assert_eq!(
            runs,
            vec![
                "2020-01-01T00:10:00Z",
                "2020-01-01T00:20:00Z",
                "2020-01-01T00:30:00Z",
                "2020-01-02T00:00:00Z",
            ]
        );
    }

    #[test]
    fn timestamp_parsing() {
        assert_eq!(parse_timestamp("2020-01-01T00:00:00Z").unwrap(), Y2020);
        assert_eq!(parse_timestamp("2020-01-01 00:00").unwrap(), Y2020);
        assert_eq!(parse_timestamp("2020-01-01").unwrap(), Y2020);
        assert_eq!(parse_timestamp("1577836800").unwrap(), Y2020);
        assert_eq!(
            parse_timestamp("2020-06-15T13:45:30Z").unwrap(),
            epoch_from_civil(2020, 6, 15, 13, 45, 30)
        );
        // Roundtrip through civil_from_epoch.
        let t = epoch_from_civil(2027, 3, 14, 9, 26, 53);
        let c = civil_from_epoch(t);
        assert_eq!((c.year, c.month, c.day, c.hour, c.minute, c.second), (2027, 3, 14, 9, 26, 53));
        assert!(parse_timestamp("not-a-date").is_err());
        assert!(parse_timestamp("2020-13-01").is_err());
    }

    #[test]
    fn weekday_civil_calc_correct() {
        // 2020-01-01 is a Wednesday.
        let c = civil_from_epoch(Y2020);
        assert_eq!(c.year, 2020);
        assert_eq!(c.month, 1);
        assert_eq!(c.day, 1);
        assert_eq!(c.weekday, 3); // Wednesday
    }
}
