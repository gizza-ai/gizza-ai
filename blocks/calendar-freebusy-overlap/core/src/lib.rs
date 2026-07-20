//! calendar-freebusy-overlap core — parse two iCalendar (.ics) texts, expand
//! their busy time (VEVENT incl. common RRULEs, VFREEBUSY periods), and list
//! the time windows where BOTH calendars are free inside chosen working hours.
//!
//! Pure compute, shared by the chat skill block, the web page, and the CLI.
//! No clock: callers pass `now_utc_secs` so results are deterministic.
//! Timezone/DST math uses chrono-tz (IANA db baked in; proven wasm-safe in
//! blocks/timezone-convert).

use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Weekday};
use chrono_tz::Tz;
use serde::Serialize;
use std::collections::HashSet;

/// Per-calendar input cap (bytes). A year of a busy calendar exports well
/// under this; the cap keeps the wasm sandbox comfortable.
pub const MAX_CALENDAR_BYTES: usize = 1_048_576; // 1 MiB
/// Combined busy-interval cap after recurrence expansion.
pub const MAX_BUSY_INTERVALS: usize = 20_000;
/// Scan-range cap in days.
pub const MAX_DAYS: i64 = 60;
/// Minimum-duration bounds (minutes).
pub const MIN_MINUTES_LO: i64 = 5;
pub const MIN_MINUTES_HI: i64 = 720;
/// Global recurrence-expansion iteration budget (all rules combined).
const RRULE_ITER_BUDGET: i64 = 200_000;

// ---------------------------------------------------------------------------
// ICS line + property parsing
// ---------------------------------------------------------------------------

/// Unfold RFC 5545 folded lines (a line starting with SPACE/HTAB continues the
/// previous line). Accepts LF or CRLF endings.
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else {
            out.push(line.to_string());
        }
    }
    out
}

/// Split a content line into (NAME, params, value). Params may be quoted
/// (`TZID="America/New_York"`); the NAME:VALUE colon is the first `:` outside
/// quotes. Returns None for non-property lines.
fn parse_prop(line: &str) -> Option<(String, Vec<(String, String)>, String)> {
    let mut in_quotes = false;
    let mut colon = None;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ':' if !in_quotes => {
                colon = Some(i);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    // Split head on ';' outside quotes: NAME;P1=V1;P2=V2
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    for c in head.chars() {
        match c {
            '"' => {
                q = !q;
                cur.push(c);
            }
            ';' if !q => {
                parts.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    let name = parts[0].trim().to_ascii_uppercase();
    if name.is_empty() {
        return None;
    }
    let mut params = Vec::new();
    for p in &parts[1..] {
        if let Some((k, v)) = p.split_once('=') {
            params.push((
                k.trim().to_ascii_uppercase(),
                v.trim().trim_matches('"').to_string(),
            ));
        }
    }
    Some((name, params, value.to_string()))
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

// ---------------------------------------------------------------------------
// Timezone + datetime parsing
// ---------------------------------------------------------------------------

/// Map the most common Windows/Outlook TZID names onto IANA zones.
fn windows_tz(name: &str) -> Option<Tz> {
    let iana = match name {
        "Pacific Standard Time" => "America/Los_Angeles",
        "Mountain Standard Time" => "America/Denver",
        "Central Standard Time" => "America/Chicago",
        "Eastern Standard Time" => "America/New_York",
        "Atlantic Standard Time" => "America/Halifax",
        "Alaskan Standard Time" => "America/Anchorage",
        "Hawaiian Standard Time" => "Pacific/Honolulu",
        "SA Pacific Standard Time" => "America/Bogota",
        "E. South America Standard Time" => "America/Sao_Paulo",
        "GMT Standard Time" => "Europe/London",
        "Greenwich Standard Time" => "Atlantic/Reykjavik",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Central Europe Standard Time" => "Europe/Budapest",
        "Central European Standard Time" => "Europe/Warsaw",
        "Romance Standard Time" => "Europe/Paris",
        "FLE Standard Time" => "Europe/Helsinki",
        "Russian Standard Time" => "Europe/Moscow",
        "South Africa Standard Time" => "Africa/Johannesburg",
        "Egypt Standard Time" => "Africa/Cairo",
        "Israel Standard Time" => "Asia/Jerusalem",
        "Arabian Standard Time" => "Asia/Dubai",
        "India Standard Time" => "Asia/Kolkata",
        "China Standard Time" => "Asia/Shanghai",
        "Singapore Standard Time" => "Asia/Singapore",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "New Zealand Standard Time" => "Pacific/Auckland",
        _ => return None,
    };
    iana.parse().ok()
}

/// Resolve a TZID onto a chrono-tz zone. Falls back to None for unknown ids
/// (caller uses the query timezone and records a warning).
fn resolve_tzid(tzid: &str) -> Option<Tz> {
    let t = tzid.trim().trim_start_matches('/');
    t.parse::<Tz>().ok().or_else(|| windows_tz(t))
}

/// Convert a wall-clock time in `tz` to a UTC epoch, resolving DST folds to
/// the earlier instant and DST gaps by shifting forward one hour.
fn localize(tz: Tz, naive: NaiveDateTime) -> i64 {
    match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.timestamp(),
        LocalResult::Ambiguous(a, _) => a.timestamp(),
        LocalResult::None => match tz.from_local_datetime(&(naive + Duration::hours(1))) {
            LocalResult::Single(dt) => dt.timestamp(),
            LocalResult::Ambiguous(a, _) => a.timestamp(),
            LocalResult::None => chrono::Utc.from_utc_datetime(&naive).timestamp(),
        },
    }
}

/// A parsed DTSTART/DTEND/EXDATE value: wall time + the zone it lives in.
#[derive(Clone, Copy, Debug)]
struct IcsDt {
    naive: NaiveDateTime,
    tz: Tz,
    all_day: bool,
}

impl IcsDt {
    fn epoch(&self) -> i64 {
        localize(self.tz, self.naive)
    }
}

/// Parse an ICS DATE / DATE-TIME value. `fallback_tz` is used for floating
/// times, all-day dates, and unknown TZIDs (sets `*tz_fallback` for the
/// latter so the caller can warn).
fn parse_ics_dt(
    value: &str,
    params: &[(String, String)],
    fallback_tz: Tz,
    tz_fallback: &mut bool,
) -> Result<IcsDt, String> {
    let v = value.trim();
    let is_date = param(params, "VALUE") == Some("DATE")
        || (v.len() == 8 && v.bytes().all(|b| b.is_ascii_digit()));
    if is_date {
        let d = NaiveDate::parse_from_str(v, "%Y%m%d")
            .map_err(|_| format!("invalid iCalendar DATE value {v:?} (expected YYYYMMDD)"))?;
        return Ok(IcsDt {
            naive: d.and_hms_opt(0, 0, 0).unwrap(),
            tz: fallback_tz,
            all_day: true,
        });
    }
    let (body, utc) = match v.strip_suffix('Z') {
        Some(b) => (b, true),
        None => (v, false),
    };
    let naive = NaiveDateTime::parse_from_str(body, "%Y%m%dT%H%M%S").map_err(|_| {
        format!("invalid iCalendar DATE-TIME value {v:?} (expected YYYYMMDDTHHMMSS[Z])")
    })?;
    let tz = if utc {
        chrono_tz::UTC
    } else if let Some(tzid) = param(params, "TZID") {
        match resolve_tzid(tzid) {
            Some(tz) => tz,
            None => {
                *tz_fallback = true;
                fallback_tz
            }
        }
    } else {
        fallback_tz
    };
    Ok(IcsDt {
        naive,
        tz,
        all_day: false,
    })
}

/// Parse an ISO-8601 duration as used by ICS (`P1D`, `PT1H30M`, `P1W`, …) to
/// seconds. Negative durations return 0 (they can't extend busy time).
fn parse_ics_duration(v: &str) -> Result<i64, String> {
    let s = v.trim();
    let (s, neg) = match s.strip_prefix('-') {
        Some(r) => (r, true),
        None => (s.strip_prefix('+').unwrap_or(s), false),
    };
    let s = s
        .strip_prefix(['P', 'p'])
        .ok_or_else(|| format!("invalid DURATION {v:?}"))?;
    let mut secs: i64 = 0;
    let mut num = String::new();
    let mut in_time = false;
    for c in s.chars() {
        match c {
            'T' | 't' => in_time = true,
            '0'..='9' => num.push(c),
            'W' | 'w' | 'D' | 'd' | 'H' | 'h' | 'M' | 'm' | 'S' | 's' => {
                let n: i64 = num.parse().map_err(|_| format!("invalid DURATION {v:?}"))?;
                num.clear();
                secs += n
                    * match c.to_ascii_uppercase() {
                        'W' => 604_800,
                        'D' => 86_400,
                        'H' => 3_600,
                        'M' if in_time => 60,
                        'M' => return Err(format!("invalid DURATION {v:?} (months unsupported)")),
                        _ => 1,
                    };
            }
            _ => return Err(format!("invalid DURATION {v:?}")),
        }
    }
    Ok(if neg { 0 } else { secs })
}

// ---------------------------------------------------------------------------
// RRULE
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Clone, Debug)]
struct RRule {
    freq: Freq,
    interval: i64,
    count: Option<i64>,
    /// Inclusive last-occurrence bound, as a UTC epoch.
    until: Option<i64>,
    /// WEEKLY: plain weekdays.
    byday: Vec<Weekday>,
    /// MONTHLY: single ordinal weekday (e.g. `3MO` = third Monday).
    monthly_byday: Option<(i32, Weekday)>,
    bymonthday: Option<u32>,
}

fn parse_weekday(s: &str) -> Option<Weekday> {
    Some(match s {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

/// Parse an RRULE value into the supported subset. Returns Ok(None) for rules
/// outside the subset (caller treats the event as one-time and warns).
fn parse_rrule(value: &str, event_tz: Tz) -> Result<Option<RRule>, String> {
    let mut freq = None;
    let mut interval: i64 = 1;
    let mut count = None;
    let mut until = None;
    let mut byday_raw: Vec<String> = Vec::new();
    let mut bymonthday: Option<u32> = None;
    for part in value.split(';') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        match k.trim().to_ascii_uppercase().as_str() {
            "FREQ" => {
                freq = Some(match v.trim().to_ascii_uppercase().as_str() {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    _ => return Ok(None), // HOURLY/MINUTELY/SECONDLY: out of subset
                })
            }
            "INTERVAL" => interval = v.trim().parse().unwrap_or(1).max(1),
            "COUNT" => count = v.trim().parse::<i64>().ok().filter(|n| *n > 0),
            "UNTIL" => {
                let mut fb = false;
                let dt = parse_ics_dt(v, &[], event_tz, &mut fb)?;
                // Inclusive bound: an all-day UNTIL covers its whole day.
                until = Some(if dt.all_day {
                    dt.epoch() + 86_399
                } else {
                    dt.epoch()
                });
            }
            "BYDAY" => byday_raw = v.split(',').map(|s| s.trim().to_string()).collect(),
            "BYMONTHDAY" => {
                let vals: Vec<&str> = v.split(',').collect();
                if vals.len() != 1 {
                    return Ok(None);
                }
                match vals[0].trim().parse::<i64>() {
                    Ok(d) if (1..=31).contains(&d) => bymonthday = Some(d as u32),
                    _ => return Ok(None), // negative month days: out of subset
                }
            }
            // Common no-op keys we can safely ignore:
            "WKST" => {}
            "BYMONTH" => {} // YEARLY from DTSTART already fixes the month
            // Anything else (BYSETPOS, BYWEEKNO, BYHOUR, …): out of subset.
            _ => return Ok(None),
        }
    }
    let Some(freq) = freq else { return Ok(None) };
    let mut byday = Vec::new();
    let mut monthly_byday = None;
    if !byday_raw.is_empty() {
        match freq {
            Freq::Weekly => {
                for d in &byday_raw {
                    match parse_weekday(d) {
                        Some(w) => byday.push(w),
                        None => return Ok(None), // ordinals in WEEKLY: out of subset
                    }
                }
            }
            Freq::Monthly => {
                if byday_raw.len() != 1 {
                    return Ok(None);
                }
                let raw = &byday_raw[0];
                let split = raw.len().saturating_sub(2);
                let (ord_raw, day) = raw.split_at(split);
                let (Some(w), Ok(ord)) = (parse_weekday(day), ord_raw.parse::<i32>()) else {
                    return Ok(None); // plain BYDAY in MONTHLY: out of subset
                };
                if ord == 0 || !(-5..=5).contains(&ord) {
                    return Ok(None);
                }
                monthly_byday = Some((ord, w));
            }
            _ => return Ok(None), // BYDAY with DAILY/YEARLY: out of subset
        }
    }
    Ok(Some(RRule {
        freq,
        interval,
        count,
        until,
        byday,
        monthly_byday,
        bymonthday,
    }))
}

fn add_months(date: NaiveDate, months: i64) -> Option<(i32, u32)> {
    let zero = date.year() as i64 * 12 + date.month0() as i64 + months;
    let y = zero.div_euclid(12);
    let m0 = zero.rem_euclid(12) as u32;
    Some((i32::try_from(y).ok()?, m0 + 1))
}

/// The `ord`th (1-based; negative = from the end) `w` weekday of year/month.
fn nth_weekday_of_month(y: i32, m: u32, ord: i32, w: Weekday) -> Option<NaiveDate> {
    if ord > 0 {
        let first = NaiveDate::from_ymd_opt(y, m, 1)?;
        let offset = (7 + w.num_days_from_monday() as i64
            - first.weekday().num_days_from_monday() as i64)
            % 7;
        let d = first + Duration::days(offset + 7 * (ord as i64 - 1));
        (d.month() == m).then_some(d)
    } else {
        let last = if m == 12 {
            NaiveDate::from_ymd_opt(y + 1, 1, 1)?
        } else {
            NaiveDate::from_ymd_opt(y, m + 1, 1)?
        } - Duration::days(1);
        let offset = (7 + last.weekday().num_days_from_monday() as i64
            - w.num_days_from_monday() as i64)
            % 7;
        let d = last - Duration::days(offset + 7 * ((-ord) as i64 - 1));
        (d.month() == m).then_some(d)
    }
}

// ---------------------------------------------------------------------------
// Calendar parsing → busy intervals
// ---------------------------------------------------------------------------

#[derive(Default)]
struct EventProps {
    dtstart: Option<(String, Vec<(String, String)>)>,
    dtend: Option<(String, Vec<(String, String)>)>,
    duration: Option<String>,
    rrule: Option<String>,
    exdates: Vec<(String, Vec<(String, String)>)>,
    status: Option<String>,
    transp: Option<String>,
}

struct CalendarBusy {
    intervals: Vec<(i64, i64)>,
    unsupported_rrules: usize,
    unknown_tzids: bool,
}

/// One event's busy occurrences clipped to `win`, appended onto `out`.
fn expand_event(
    ev: &EventProps,
    query_tz: Tz,
    win: (i64, i64),
    out: &mut Vec<(i64, i64)>,
    budget: &mut i64,
    unsupported: &mut usize,
    unknown_tzids: &mut bool,
) -> Result<(), String> {
    // Cancelled events and transparent (free-marked) events don't block time.
    let upper = |v: &Option<String>| v.as_deref().map(str::trim).map(str::to_ascii_uppercase);
    if upper(&ev.status).as_deref() == Some("CANCELLED")
        || upper(&ev.transp).as_deref() == Some("TRANSPARENT")
    {
        return Ok(());
    }
    let Some((sv, sp)) = &ev.dtstart else {
        return Ok(()); // no DTSTART → nothing to block
    };
    let start = parse_ics_dt(sv, sp, query_tz, unknown_tzids)?;

    // Occurrence length: wall days for all-day events, seconds otherwise.
    let mut dur_days: i64 = 0;
    let mut dur_secs: i64 = 0;
    if let Some((ev_v, ev_p)) = &ev.dtend {
        let end = parse_ics_dt(ev_v, ev_p, query_tz, unknown_tzids)?;
        if start.all_day {
            dur_days = (end.naive.date() - start.naive.date()).num_days().max(1);
        } else {
            dur_secs = (end.epoch() - start.epoch()).max(0);
        }
    } else if let Some(d) = &ev.duration {
        let secs = parse_ics_duration(d)?;
        if start.all_day {
            dur_days = (secs / 86_400).max(1);
        } else {
            dur_secs = secs;
        }
    } else if start.all_day {
        dur_days = 1;
    } else {
        dur_secs = 0; // RFC 5545: DATE-TIME start with no end takes no time
    }
    if !start.all_day && dur_secs <= 0 {
        return Ok(());
    }

    // EXDATE set: epochs for timed events, dates for all-day ones.
    let mut ex_epochs: HashSet<i64> = HashSet::new();
    let mut ex_dates: HashSet<NaiveDate> = HashSet::new();
    for (v, p) in &ev.exdates {
        for one in v.split(',') {
            if one.trim().is_empty() {
                continue;
            }
            let dt = parse_ics_dt(one, p, query_tz, unknown_tzids)?;
            if dt.all_day {
                ex_dates.insert(dt.naive.date());
            } else {
                ex_epochs.insert(dt.epoch());
            }
        }
    }

    let push = |occ: IcsDt, out: &mut Vec<(i64, i64)>| {
        if occ.all_day {
            if ex_dates.contains(&occ.naive.date()) {
                return;
            }
        } else if ex_epochs.contains(&occ.epoch()) {
            return;
        }
        let s = occ.epoch();
        let e = if occ.all_day {
            localize(occ.tz, occ.naive + Duration::days(dur_days))
        } else {
            s + dur_secs
        };
        if e > win.0 && s < win.1 {
            out.push((s.max(win.0), e.min(win.1)));
        }
    };

    let rule = match &ev.rrule {
        None => {
            push(start, out);
            return Ok(());
        }
        Some(r) => match parse_rrule(r, start.tz)? {
            Some(rule) => rule,
            None => {
                *unsupported += 1;
                push(start, out);
                return Ok(());
            }
        },
    };

    // Expand: iterate occurrence wall-times from DTSTART; stop past the window
    // end, COUNT, UNTIL, or the global budget.
    let occ_len = if start.all_day {
        dur_days * 86_400
    } else {
        dur_secs
    };
    let base = start.naive;
    let mut occ_index: i64 = 0; // occurrence counter for COUNT
    let mut k: i64 = 0; // period index (day/week/month/year steps)
    loop {
        *budget -= 1;
        if *budget <= 0 {
            return Err(
                "recurrence expansion is too large — reduce the scan range (days) or simplify the calendars"
                    .into(),
            );
        }
        // The wall-clock occurrence start(s) for period k, in chronological order.
        let mut occs: Vec<NaiveDateTime> = Vec::new();
        match rule.freq {
            Freq::Daily => occs.push(base + Duration::days(k * rule.interval)),
            Freq::Weekly => {
                if rule.byday.is_empty() {
                    occs.push(base + Duration::weeks(k * rule.interval));
                } else {
                    // Week block k (weeks start Monday), each matching weekday ≥ DTSTART.
                    let week0_mon = base.date()
                        - Duration::days(base.weekday().num_days_from_monday() as i64);
                    let mon = week0_mon + Duration::weeks(k * rule.interval);
                    for d in 0..7 {
                        let date = mon + Duration::days(d);
                        if date < base.date() {
                            continue;
                        }
                        if rule.byday.contains(&date.weekday()) {
                            occs.push(date.and_time(base.time()));
                        }
                    }
                }
            }
            Freq::Monthly => {
                if let Some((ym, m)) = add_months(base.date(), k * rule.interval) {
                    if let Some((ord, w)) = rule.monthly_byday {
                        if let Some(date) = nth_weekday_of_month(ym, m, ord, w) {
                            if date >= base.date() {
                                occs.push(date.and_time(base.time()));
                            }
                        }
                    } else {
                        let day = rule.bymonthday.unwrap_or(base.day());
                        if let Some(date) = NaiveDate::from_ymd_opt(ym, m, day) {
                            occs.push(date.and_time(base.time()));
                        }
                    }
                }
            }
            Freq::Yearly => {
                let y = base.year() + (k * rule.interval) as i32;
                if let Some(date) = NaiveDate::from_ymd_opt(y, base.month(), base.day()) {
                    occs.push(date.and_time(base.time()));
                }
            }
        }
        let mut past_window = false;
        for naive in occs {
            if let Some(c) = rule.count {
                if occ_index >= c {
                    return Ok(());
                }
            }
            occ_index += 1;
            let occ = IcsDt {
                naive,
                tz: start.tz,
                all_day: start.all_day,
            };
            let s = occ.epoch();
            if let Some(u) = rule.until {
                if s > u {
                    return Ok(());
                }
            }
            if s >= win.1 {
                past_window = true;
                break;
            }
            if s + occ_len > win.0 {
                push(occ, out);
                if out.len() > MAX_BUSY_INTERVALS {
                    return Err(format!(
                        "too many busy intervals after recurrence expansion (limit {MAX_BUSY_INTERVALS})"
                    ));
                }
            }
        }
        if past_window {
            return Ok(());
        }
        if let Some(c) = rule.count {
            if occ_index >= c {
                return Ok(());
            }
        }
        k += 1;
    }
}

fn parse_calendar(
    text: &str,
    label: &str,
    query_tz: Tz,
    win: (i64, i64),
    budget: &mut i64,
) -> Result<CalendarBusy, String> {
    if text.trim().is_empty() {
        return Err(format!(
            "calendar {label} is empty — paste the full .ics text (it starts with BEGIN:VCALENDAR)"
        ));
    }
    if text.len() > MAX_CALENDAR_BYTES {
        return Err(format!(
            "calendar {label} is too large ({} bytes; the limit is {MAX_CALENDAR_BYTES} bytes = 1 MiB)",
            text.len()
        ));
    }
    // Shell-friendliness: a calendar pasted as ONE line with literal `\n`
    // escape sequences (e.g. the copyable CLI example, where quoting keeps
    // backslash-n literal) is unescaped before parsing. Only kicks in when the
    // text has no real newlines, so genuine ICS content is never rewritten.
    let unescaped;
    let text = if !text.contains('\n') && text.contains("\\n") {
        unescaped = text.replace("\\r\\n", "\n").replace("\\n", "\n");
        unescaped.as_str()
    } else {
        text
    };
    let lines = unfold(text);
    let mut stack: Vec<String> = Vec::new();
    let mut cur_event: Option<EventProps> = None;
    let mut fb_periods: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut out = CalendarBusy {
        intervals: Vec::new(),
        unsupported_rrules: 0,
        unknown_tzids: false,
    };
    let mut saw_component = false;

    for line in &lines {
        let Some((name, params, value)) = parse_prop(line) else {
            continue;
        };
        match name.as_str() {
            "BEGIN" => {
                let comp = value.trim().to_ascii_uppercase();
                if comp == "VEVENT" && !stack.iter().any(|c| c == "VEVENT") {
                    cur_event = Some(EventProps::default());
                    saw_component = true;
                } else if comp == "VFREEBUSY" {
                    saw_component = true;
                }
                stack.push(comp);
            }
            "END" => {
                let comp = value.trim().to_ascii_uppercase();
                if comp == "VEVENT" {
                    if let Some(ev) = cur_event.take() {
                        expand_event(
                            &ev,
                            query_tz,
                            win,
                            &mut out.intervals,
                            budget,
                            &mut out.unsupported_rrules,
                            &mut out.unknown_tzids,
                        )
                        .map_err(|e| format!("calendar {label}: {e}"))?;
                        if out.intervals.len() > MAX_BUSY_INTERVALS {
                            return Err(format!(
                                "calendar {label}: too many busy intervals (limit {MAX_BUSY_INTERVALS})"
                            ));
                        }
                    }
                }
                while let Some(top) = stack.pop() {
                    if top == comp {
                        break;
                    }
                }
            }
            _ => {
                let top = stack.last().map(String::as_str);
                if top == Some("VEVENT") {
                    if let Some(ev) = cur_event.as_mut() {
                        match name.as_str() {
                            "DTSTART" => ev.dtstart = Some((value, params)),
                            "DTEND" => ev.dtend = Some((value, params)),
                            "DURATION" => ev.duration = Some(value),
                            "RRULE" => ev.rrule = Some(value),
                            "EXDATE" => ev.exdates.push((value, params)),
                            "STATUS" => ev.status = Some(value),
                            "TRANSP" => ev.transp = Some(value),
                            _ => {}
                        }
                    }
                } else if top == Some("VFREEBUSY") && name == "FREEBUSY" {
                    fb_periods.push((value, params));
                }
            }
        }
    }

    if !saw_component {
        return Err(format!(
            "calendar {label} doesn't look like iCalendar data — no VEVENT or VFREEBUSY found \
             (paste the raw .ics text exported from your calendar app)"
        ));
    }

    // VFREEBUSY periods: BUSY / BUSY-UNAVAILABLE / BUSY-TENTATIVE block time.
    for (value, params) in fb_periods {
        let fbtype = param(&params, "FBTYPE")
            .unwrap_or("BUSY")
            .to_ascii_uppercase();
        if fbtype == "FREE" {
            continue;
        }
        for period in value.split(',') {
            let p = period.trim();
            if p.is_empty() {
                continue;
            }
            let Some((s, e)) = p.split_once('/') else {
                return Err(format!("calendar {label}: invalid FREEBUSY period {p:?}"));
            };
            let mut fb = false;
            let start = parse_ics_dt(s, &[], query_tz, &mut fb)
                .map_err(|e| format!("calendar {label}: {e}"))?;
            let start_epoch = start.epoch();
            let end_epoch = if e.trim_start().starts_with(['P', 'p', '+', '-']) {
                start_epoch + parse_ics_duration(e).map_err(|e| format!("calendar {label}: {e}"))?
            } else {
                parse_ics_dt(e, &[], query_tz, &mut fb)
                    .map_err(|e| format!("calendar {label}: {e}"))?
                    .epoch()
            };
            if end_epoch > win.0 && start_epoch < win.1 && end_epoch > start_epoch {
                out.intervals
                    .push((start_epoch.max(win.0), end_epoch.min(win.1)));
            }
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Interval math
// ---------------------------------------------------------------------------

fn merge(mut v: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    v.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(v.len());
    for (s, e) in v {
        if let Some(last) = out.last_mut() {
            if s <= last.1 {
                last.1 = last.1.max(e);
                continue;
            }
        }
        out.push((s, e));
    }
    out
}

/// Free segments of `window` not covered by (merged, sorted) `busy`.
fn subtract(window: (i64, i64), busy: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let mut cursor = window.0;
    for &(s, e) in busy {
        if e <= cursor {
            continue;
        }
        if s >= window.1 {
            break;
        }
        if s > cursor {
            out.push((cursor, s.min(window.1)));
        }
        cursor = cursor.max(e);
        if cursor >= window.1 {
            break;
        }
    }
    if cursor < window.1 {
        out.push((cursor, window.1));
    }
    out
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonSlot {
    date: String,
    weekday: String,
    start: String,
    end: String,
    start_iso: String,
    end_iso: String,
    minutes: i64,
}

#[derive(Serialize)]
struct JsonRange {
    start_date: String,
    days: i64,
    day_start: String,
    day_end: String,
    weekends: bool,
}

#[derive(Serialize)]
struct JsonBusy {
    calendar_a: usize,
    calendar_b: usize,
}

#[derive(Serialize)]
struct JsonOut {
    timezone: String,
    range: JsonRange,
    min_minutes: i64,
    busy_intervals: JsonBusy,
    slots: Vec<JsonSlot>,
    total_minutes: i64,
    warnings: Vec<String>,
}

fn fmt_dur(minutes: i64) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

fn in_tz(tz: Tz, epoch: i64) -> chrono::DateTime<Tz> {
    tz.from_utc_datetime(
        &chrono::DateTime::from_timestamp(epoch, 0)
            .expect("epoch in range")
            .naive_utc(),
    )
}

/// "HH:MM" in tz; midnight at the end of a slot renders as 24:00.
fn fmt_hm(tz: Tz, epoch: i64, end_of_slot: bool) -> String {
    let s = in_tz(tz, epoch).format("%H:%M").to_string();
    if end_of_slot && s == "00:00" {
        "24:00".to_string()
    } else {
        s
    }
}

fn parse_hm(s: &str, name: &str, default: (u32, u32)) -> Result<(u32, u32), String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    let (h, m) = t
        .split_once(':')
        .ok_or_else(|| format!("{name} must be HH:MM (24-hour), e.g. \"09:00\" — got {t:?}"))?;
    let h: u32 = h
        .trim()
        .parse()
        .map_err(|_| format!("{name}: invalid hour in {t:?}"))?;
    let m: u32 = m
        .trim()
        .parse()
        .map_err(|_| format!("{name}: invalid minutes in {t:?}"))?;
    if m > 59 || h > 24 || (h == 24 && m != 0) {
        return Err(format!("{name} must be between 00:00 and 24:00 — got {t:?}"));
    }
    Ok((h, m))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Find the free windows common to both calendars. See the block descriptor
/// for parameter semantics. `now_utc_secs` supplies "today" when `start_date`
/// is empty (each surface passes its own clock; the core stays deterministic).
#[allow(clippy::too_many_arguments)]
pub fn run(
    calendar_a: &str,
    calendar_b: &str,
    start_date: &str,
    days: i64,
    day_start: &str,
    day_end: &str,
    min_minutes: i64,
    timezone: &str,
    weekends: bool,
    output: &str,
    now_utc_secs: i64,
) -> Result<String, String> {
    // ---- validate params ----
    let tz_name = if timezone.trim().is_empty() {
        "UTC"
    } else {
        timezone.trim()
    };
    let tz: Tz = tz_name.parse().map_err(|_| {
        format!("unknown timezone {tz_name:?} — use an IANA name like Europe/Berlin, America/New_York, or UTC")
    })?;
    if !(1..=MAX_DAYS).contains(&days) {
        return Err(format!("days must be between 1 and {MAX_DAYS} — got {days}"));
    }
    if !(MIN_MINUTES_LO..=MIN_MINUTES_HI).contains(&min_minutes) {
        return Err(format!(
            "min_minutes must be between {MIN_MINUTES_LO} and {MIN_MINUTES_HI} — got {min_minutes}"
        ));
    }
    let output = if output.trim().is_empty() {
        "text"
    } else {
        output.trim()
    };
    if !matches!(output, "text" | "json" | "ics") {
        return Err(format!(
            "output must be one of text, json, ics — got {output:?}"
        ));
    }
    let (sh, sm) = parse_hm(day_start, "day_start", (9, 0))?;
    let (eh, em) = parse_hm(day_end, "day_end", (17, 0))?;
    let start_min = (sh * 60 + sm) as i64;
    let end_min = (eh * 60 + em) as i64;
    if end_min <= start_min {
        return Err(format!(
            "day_end ({eh:02}:{em:02}) must be after day_start ({sh:02}:{sm:02})"
        ));
    }
    let first_day: NaiveDate = if start_date.trim().is_empty() {
        let now = chrono::DateTime::from_timestamp(now_utc_secs, 0)
            .ok_or_else(|| "invalid clock value".to_string())?;
        tz.from_utc_datetime(&now.naive_utc()).date_naive()
    } else {
        let s = start_date.trim().replace('/', "-");
        NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|_| {
            format!("start_date must be YYYY-MM-DD, e.g. 2026-07-20 — got {start_date:?}")
        })?
    };
    let last_day = first_day + Duration::days(days - 1);

    // ---- day windows (each day's working hours, localized with DST) ----
    let mut windows: Vec<(NaiveDate, i64, i64)> = Vec::new();
    for d in 0..days {
        let date = first_day + Duration::days(d);
        let wd = date.weekday();
        if !weekends && (wd == Weekday::Sat || wd == Weekday::Sun) {
            continue;
        }
        let midnight = date.and_hms_opt(0, 0, 0).expect("midnight");
        let ws = localize(tz, midnight + Duration::minutes(start_min));
        let we = localize(tz, midnight + Duration::minutes(end_min));
        if we > ws {
            windows.push((date, ws, we));
        }
    }
    if windows.is_empty() {
        return Err(
            "the scan range contains no working days — enable weekends or extend days".into(),
        );
    }
    let scan_win = (windows[0].1, windows[windows.len() - 1].2);

    // ---- parse calendars ----
    let mut budget = RRULE_ITER_BUDGET;
    let a = parse_calendar(calendar_a, "A", tz, scan_win, &mut budget)?;
    let b = parse_calendar(calendar_b, "B", tz, scan_win, &mut budget)?;
    let mut warnings: Vec<String> = Vec::new();
    for (cal, label) in [(&a, "A"), (&b, "B")] {
        if cal.unsupported_rrules > 0 {
            warnings.push(format!(
                "calendar {label}: {} recurring event(s) use recurrence rules outside the supported subset and were treated as one-time events",
                cal.unsupported_rrules
            ));
        }
        if cal.unknown_tzids {
            warnings.push(format!(
                "calendar {label}: some events use a TZID this tool doesn't know — those times were read in {tz_name}"
            ));
        }
    }

    let mut all_busy = a.intervals.clone();
    all_busy.extend_from_slice(&b.intervals);
    let busy = merge(all_busy);

    // ---- free slots ----
    let mut slots: Vec<(NaiveDate, i64, i64)> = Vec::new();
    for (date, ws, we) in &windows {
        for (s, e) in subtract((*ws, *we), &busy) {
            if e - s >= min_minutes * 60 {
                slots.push((*date, s, e));
            }
        }
    }
    let total_minutes: i64 = slots.iter().map(|(_, s, e)| (e - s) / 60).sum();

    // ---- render ----
    match output {
        "json" => {
            let out = JsonOut {
                timezone: tz_name.to_string(),
                range: JsonRange {
                    start_date: first_day.format("%Y-%m-%d").to_string(),
                    days,
                    day_start: format!("{sh:02}:{sm:02}"),
                    day_end: format!("{eh:02}:{em:02}"),
                    weekends,
                },
                min_minutes,
                busy_intervals: JsonBusy {
                    calendar_a: a.intervals.len(),
                    calendar_b: b.intervals.len(),
                },
                slots: slots
                    .iter()
                    .map(|(date, s, e)| JsonSlot {
                        date: date.format("%Y-%m-%d").to_string(),
                        weekday: date.format("%a").to_string(),
                        start: fmt_hm(tz, *s, false),
                        end: fmt_hm(tz, *e, true),
                        start_iso: in_tz(tz, *s).format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
                        end_iso: in_tz(tz, *e).format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
                        minutes: (e - s) / 60,
                    })
                    .collect(),
                total_minutes,
                warnings,
            };
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
        }
        "ics" => {
            let fmt_utc = |epoch: i64| {
                chrono::DateTime::from_timestamp(epoch, 0)
                    .expect("epoch in range")
                    .format("%Y%m%dT%H%M%SZ")
                    .to_string()
            };
            let mut s = String::new();
            s.push_str("BEGIN:VCALENDAR\r\n");
            s.push_str("VERSION:2.0\r\n");
            s.push_str("PRODID:-//calendar-freebusy-overlap//EN\r\n");
            s.push_str("CALSCALE:GREGORIAN\r\n");
            s.push_str("BEGIN:VFREEBUSY\r\n");
            s.push_str(&format!("DTSTAMP:{}\r\n", fmt_utc(now_utc_secs)));
            s.push_str(&format!("DTSTART:{}\r\n", fmt_utc(scan_win.0)));
            s.push_str(&format!("DTEND:{}\r\n", fmt_utc(scan_win.1)));
            for (_, st, en) in &slots {
                s.push_str(&format!(
                    "FREEBUSY;FBTYPE=FREE:{}/{}\r\n",
                    fmt_utc(*st),
                    fmt_utc(*en)
                ));
            }
            s.push_str("END:VFREEBUSY\r\n");
            s.push_str("END:VCALENDAR\r\n");
            Ok(s)
        }
        _ => {
            let daylabel = if weekends { "Mon–Sun" } else { "Mon–Fri" };
            let mut s = format!(
                "Common free time — {} to {} · {daylabel} {sh:02}:{sm:02}–{eh:02}:{em:02} · {tz_name} · ≥{}\n",
                first_day.format("%Y-%m-%d"),
                last_day.format("%Y-%m-%d"),
                fmt_dur(min_minutes)
            );
            s.push_str(&format!(
                "Calendar A: {} busy interval{} · Calendar B: {} busy interval{}\n",
                a.intervals.len(),
                if a.intervals.len() == 1 { "" } else { "s" },
                b.intervals.len(),
                if b.intervals.len() == 1 { "" } else { "s" },
            ));
            s.push('\n');
            if slots.is_empty() {
                s.push_str(
                    "No common free time found. Try a longer range (days), wider working hours, a smaller minimum duration, or including weekends.\n",
                );
            } else {
                for (date, st, en) in &slots {
                    s.push_str(&format!(
                        "{} {}  {}–{}  ({})\n",
                        date.format("%a"),
                        date.format("%Y-%m-%d"),
                        fmt_hm(tz, *st, false),
                        fmt_hm(tz, *en, true),
                        fmt_dur((en - st) / 60)
                    ));
                }
                s.push('\n');
                s.push_str(&format!(
                    "{} free slot{} · {} total\n",
                    slots.len(),
                    if slots.len() == 1 { "" } else { "s" },
                    fmt_dur(total_minutes)
                ));
            }
            for w in &warnings {
                s.push_str(&format!("Note: {w}\n"));
            }
            Ok(s)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_768_000_000; // fixed clock for tests (2026-01-09T22:26:40Z)

    fn cal(events: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n{events}END:VCALENDAR\r\n")
    }

    fn event(body: &str) -> String {
        format!("BEGIN:VEVENT\r\nUID:x@test\r\n{body}END:VEVENT\r\n")
    }

    #[test]
    fn happy_path_two_calendars_text() {
        // Mon 2026-07-20, UTC. A busy 09-10, B busy 12-13 → free 10-12 and 13-17.
        let a = cal(&event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"));
        let b = cal(&event("DTSTART:20260720T120000Z\r\nDTEND:20260720T130000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "text", NOW).unwrap();
        assert_eq!(
            out,
            "Common free time — 2026-07-20 to 2026-07-20 · Mon–Fri 09:00–17:00 · UTC · ≥30m\n\
             Calendar A: 1 busy interval · Calendar B: 1 busy interval\n\
             \n\
             Mon 2026-07-20  10:00–12:00  (2h)\n\
             Mon 2026-07-20  13:00–17:00  (4h)\n\
             \n\
             2 free slots · 6h total\n"
        );
    }

    #[test]
    fn empty_calendar_is_an_error() {
        let a = cal(&event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"));
        let err = run(&a, "  ", "2026-07-20", 1, "", "", 30, "UTC", false, "text", NOW).unwrap_err();
        assert!(err.contains("calendar B is empty"), "{err}");
    }

    #[test]
    fn non_ics_input_is_an_error() {
        let a = cal(&event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"));
        let err = run("hello world", &a, "2026-07-20", 1, "", "", 30, "UTC", false, "text", NOW)
            .unwrap_err();
        assert!(err.contains("doesn't look like iCalendar"), "{err}");
    }

    #[test]
    fn weekly_rrule_with_byday_blocks_the_right_days() {
        // Weekly Mon+Wed 09:00-10:00 UTC starting well before the window.
        let a = cal(&event(
            "DTSTART:20260601T090000Z\r\nDTEND:20260601T100000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE\r\n",
        ));
        let b = cal("BEGIN:VFREEBUSY\r\nEND:VFREEBUSY\r\n"); // no busy time
        let out =
            run(&a, &b, "2026-07-20", 3, "09:00", "11:00", 30, "UTC", false, "text", NOW).unwrap();
        // Mon 20th + Wed 22nd are blocked 09-10; Tue 21st fully free.
        assert!(out.contains("Mon 2026-07-20  10:00–11:00  (1h)"), "{out}");
        assert!(out.contains("Tue 2026-07-21  09:00–11:00  (2h)"), "{out}");
        assert!(out.contains("Wed 2026-07-22  10:00–11:00  (1h)"), "{out}");
    }

    #[test]
    fn rrule_count_and_exdate_are_honored() {
        // Daily 09:00-10:00 for 3 days (20,21,22) but the 21st is EXDATEd.
        let a = cal(&event(
            "DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEXDATE:20260721T090000Z\r\n",
        ));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 4, "09:00", "10:00", 30, "UTC", false, "text", NOW).unwrap();
        // Free 09-10 only on the EXDATEd 21st and on the 23rd (past COUNT).
        assert!(!out.contains("Mon 2026-07-20"), "{out}");
        assert!(out.contains("Tue 2026-07-21  09:00–10:00  (1h)"), "{out}");
        assert!(!out.contains("Wed 2026-07-22"), "{out}");
        assert!(out.contains("Thu 2026-07-23  09:00–10:00  (1h)"), "{out}");
    }

    #[test]
    fn all_day_event_blocks_whole_working_day() {
        let a = cal(&event("DTSTART;VALUE=DATE:20260721\r\n"));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 2, "09:00", "17:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("Mon 2026-07-20  09:00–17:00  (8h)"), "{out}");
        assert!(!out.contains("Tue 2026-07-21"), "{out}");
    }

    #[test]
    fn cancelled_and_transparent_events_do_not_block() {
        let a = cal(&format!(
            "{}{}",
            event("DTSTART:20260720T090000Z\r\nDTEND:20260720T170000Z\r\nSTATUS:CANCELLED\r\n"),
            event("DTSTART:20260720T090000Z\r\nDTEND:20260720T170000Z\r\nTRANSP:TRANSPARENT\r\n"),
        ));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("Mon 2026-07-20  09:00–17:00  (8h)"), "{out}");
    }

    #[test]
    fn vfreebusy_periods_block_time() {
        let a = cal(
            "BEGIN:VFREEBUSY\r\nFREEBUSY;FBTYPE=BUSY:20260720T090000Z/20260720T120000Z\r\n\
             FREEBUSY;FBTYPE=FREE:20260720T130000Z/20260720T140000Z\r\n\
             FREEBUSY:20260720T150000Z/PT1H\r\nEND:VFREEBUSY\r\n",
        );
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("Mon 2026-07-20  12:00–15:00  (3h)"), "{out}");
        assert!(out.contains("Mon 2026-07-20  16:00–17:00  (1h)"), "{out}");
    }

    #[test]
    fn tzid_events_convert_into_query_timezone() {
        // 09:00–12:00 in New York = 15:00–18:00 in Berlin (July, EDT→CEST).
        let a = cal(&event(
            "DTSTART;TZID=America/New_York:20260720T090000\r\nDTEND;TZID=America/New_York:20260720T120000\r\n",
        ));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out = run(
            &a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "Europe/Berlin", false, "text", NOW,
        )
        .unwrap();
        assert!(out.contains("Mon 2026-07-20  09:00–15:00  (6h)"), "{out}");
        assert!(!out.contains("16:00"), "{out}");
    }

    #[test]
    fn unknown_tzid_falls_back_with_warning() {
        let a = cal(&event(
            "DTSTART;TZID=Mars/Olympus_Mons:20260720T090000\r\nDTEND;TZID=Mars/Olympus_Mons:20260720T100000\r\n",
        ));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("Note: calendar A: some events use a TZID"), "{out}");
        assert!(out.contains("Mon 2026-07-20  10:00–17:00  (7h)"), "{out}");
    }

    #[test]
    fn windows_tzid_maps_to_iana() {
        // 09:00 "W. Europe Standard Time" = Europe/Berlin wall time.
        let a = cal(&event(
            "DTSTART;TZID=W. Europe Standard Time:20260720T090000\r\nDTEND;TZID=W. Europe Standard Time:20260720T120000\r\n",
        ));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out = run(
            &a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "Europe/Berlin", false, "text", NOW,
        )
        .unwrap();
        assert!(out.contains("Mon 2026-07-20  12:00–17:00  (5h)"), "{out}");
    }

    #[test]
    fn weekends_flag_includes_saturday_sunday() {
        let a = cal(&event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"));
        let b = cal(&event("DTSTART:20260720T120000Z\r\nDTEND:20260720T130000Z\r\n"));
        // 2026-07-25 is a Saturday.
        let excl = run(&a, &b, "2026-07-25", 1, "09:00", "17:00", 30, "UTC", false, "text", NOW);
        assert!(excl.unwrap_err().contains("no working days"));
        let incl =
            run(&a, &b, "2026-07-25", 1, "09:00", "17:00", 30, "UTC", true, "text", NOW).unwrap();
        assert!(incl.contains("Sat 2026-07-25  09:00–17:00  (8h)"), "{incl}");
        assert!(incl.contains("Mon–Sun"), "{incl}");
    }

    #[test]
    fn min_duration_filters_short_gaps() {
        // Busy 09:00-10:00 and 10:20-17:00 → the 20m gap disappears at min 30.
        let a = cal(&format!(
            "{}{}",
            event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"),
            event("DTSTART:20260720T102000Z\r\nDTEND:20260720T170000Z\r\n"),
        ));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("No common free time found"), "{out}");
        let out20 =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 20, "UTC", false, "text", NOW).unwrap();
        assert!(out20.contains("Mon 2026-07-20  10:00–10:20  (20m)"), "{out20}");
    }

    #[test]
    fn json_output_shape() {
        let a = cal(&event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"));
        let b = cal(&event("DTSTART:20260720T120000Z\r\nDTEND:20260720T130000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "json", NOW).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["timezone"], "UTC");
        assert_eq!(v["slots"][0]["start"], "10:00");
        assert_eq!(v["slots"][0]["end"], "12:00");
        assert_eq!(v["slots"][0]["minutes"], 120);
        assert_eq!(v["slots"][0]["start_iso"], "2026-07-20T10:00:00+00:00");
        assert_eq!(v["total_minutes"], 360);
        assert_eq!(v["busy_intervals"]["calendar_a"], 1);
    }

    #[test]
    fn ics_output_is_a_vfreebusy_with_free_periods() {
        let a = cal(&event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"));
        let b = cal(&event("DTSTART:20260720T120000Z\r\nDTEND:20260720T130000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "ics", NOW).unwrap();
        assert!(out.starts_with("BEGIN:VCALENDAR\r\n"), "{out}");
        assert!(
            out.contains("FREEBUSY;FBTYPE=FREE:20260720T100000Z/20260720T120000Z\r\n"),
            "{out}"
        );
        assert!(
            out.contains("FREEBUSY;FBTYPE=FREE:20260720T130000Z/20260720T170000Z\r\n"),
            "{out}"
        );
        assert!(out.ends_with("END:VCALENDAR\r\n"), "{out}");
    }

    #[test]
    fn day_end_24_00_and_folded_lines() {
        // A folded DTSTART line must unfold; day_end 24:00 renders as 24:00.
        let a = cal(
            "BEGIN:VEVENT\r\nUID:f@test\r\nDTSTART:20260720T2\r\n 30000Z\r\nDTEND:20260721T000000Z\r\nEND:VEVENT\r\n",
        );
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "20:00", "24:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("Mon 2026-07-20  20:00–23:00  (3h)"), "{out}");
    }

    #[test]
    fn size_cap_boundary() {
        let b = cal(&event("DTSTART:20260720T120000Z\r\nDTEND:20260720T130000Z\r\n"));
        let base = cal(&event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"));
        // Pad calendar A to exactly the cap with an X- property.
        let pad_needed = MAX_CALENDAR_BYTES - base.len();
        let mut a = base.clone();
        let insert_at = a.len() - "END:VCALENDAR\r\n".len();
        a.insert_str(insert_at, &format!("X-PAD:{}\r\n", "a".repeat(pad_needed - 8)));
        assert_eq!(a.len(), MAX_CALENDAR_BYTES);
        assert!(run(&a, &b, "2026-07-20", 1, "", "", 30, "UTC", false, "text", NOW).is_ok());
        // One byte over fails.
        a.push(' ');
        let err = run(&a, &b, "2026-07-20", 1, "", "", 30, "UTC", false, "text", NOW).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn param_bounds_are_enforced() {
        let a = cal(&event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n"));
        let e = run(&a, &a, "2026-07-20", 61, "", "", 30, "UTC", false, "text", NOW).unwrap_err();
        assert!(e.contains("days must be between 1 and 60"), "{e}");
        assert!(run(&a, &a, "2026-07-20", 60, "", "", 30, "UTC", true, "text", NOW).is_ok());
        let e = run(&a, &a, "2026-07-20", 1, "", "", 4, "UTC", false, "text", NOW).unwrap_err();
        assert!(e.contains("min_minutes"), "{e}");
        let e = run(&a, &a, "2026-07-20", 1, "", "", 30, "Nope/Nowhere", false, "text", NOW)
            .unwrap_err();
        assert!(e.contains("unknown timezone"), "{e}");
        let e = run(&a, &a, "2026-07-20", 1, "17:00", "09:00", 30, "UTC", false, "text", NOW)
            .unwrap_err();
        assert!(e.contains("day_end"), "{e}");
        let e = run(&a, &a, "2026-07-20", 1, "", "", 30, "UTC", false, "xml", NOW).unwrap_err();
        assert!(e.contains("output must be one of"), "{e}");
        let e = run(&a, &a, "someday", 1, "", "", 30, "UTC", false, "text", NOW).unwrap_err();
        assert!(e.contains("start_date must be YYYY-MM-DD"), "{e}");
    }

    #[test]
    fn empty_start_date_uses_today_in_timezone() {
        // NOW = 2026-01-09T22:26:40Z → already Jan 10 in Asia/Tokyo (+9).
        let a = cal(&event("DTSTART:20260110T010000Z\r\nDTEND:20260110T020000Z\r\n")); // 10:00–11:00 JST
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "", 1, "09:00", "12:00", 30, "Asia/Tokyo", true, "text", NOW).unwrap();
        assert!(out.contains("2026-01-10 to 2026-01-10"), "{out}");
        assert!(out.contains("Sat 2026-01-10  09:00–10:00  (1h)"), "{out}");
        assert!(out.contains("Sat 2026-01-10  11:00–12:00  (1h)"), "{out}");
    }

    #[test]
    fn monthly_ordinal_byday_rrule() {
        // 3rd Monday monthly 09:00–10:00 UTC — 2026-07-20 IS the 3rd Monday.
        let a = cal(&event(
            "DTSTART:20260119T090000Z\r\nDTEND:20260119T100000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=3MO\r\n",
        ));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "11:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("Mon 2026-07-20  10:00–11:00  (1h)"), "{out}");
    }

    #[test]
    fn unsupported_rrule_warns_and_blocks_dtstart_only() {
        let a = cal(&event(
            "DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\nRRULE:FREQ=DAILY;BYSETPOS=2\r\n",
        ));
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 2, "09:00", "10:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("recurrence rules outside the supported subset"), "{out}");
        assert!(out.contains("Tue 2026-07-21  09:00–10:00  (1h)"), "{out}");
        assert!(!out.contains("Mon 2026-07-20  09:00"), "{out}");
    }

    #[test]
    fn vtimezone_dtstart_lines_are_ignored() {
        // A VTIMEZONE block contains DTSTART lines that must NOT become events.
        let a = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/Berlin\r\n\
             BEGIN:STANDARD\r\nDTSTART:19701025T030000\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nEND:STANDARD\r\n\
             END:VTIMEZONE\r\n{}END:VCALENDAR\r\n",
            event("DTSTART:20260720T090000Z\r\nDTEND:20260720T100000Z\r\n")
        );
        let b = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out =
            run(&a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "json", NOW).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["busy_intervals"]["calendar_a"], 1);
    }

    #[test]
    fn single_line_escaped_newlines_are_unescaped() {
        // The copy-paste CLI example carries literal \n sequences — accept them.
        let a = "BEGIN:VCALENDAR\\nVERSION:2.0\\nBEGIN:VEVENT\\nUID:e@a\\nDTSTART:20260720T090000Z\\nDTEND:20260720T100000Z\\nEND:VEVENT\\nEND:VCALENDAR";
        let b = cal(&event("DTSTART:20260720T120000Z\r\nDTEND:20260720T130000Z\r\n"));
        let out =
            run(a, &b, "2026-07-20", 1, "09:00", "17:00", 30, "UTC", false, "text", NOW).unwrap();
        assert!(out.contains("Mon 2026-07-20  10:00–12:00  (2h)"), "{out}");
    }

    #[test]
    fn dst_spring_forward_window_is_handled() {
        // Europe/Berlin 2026-03-29: 02:00→03:00 gap. A window that starts at
        // 02:30 must not panic and lands at the shifted instant.
        let a = cal(&event("DTSTART:20300101T000000Z\r\nDTEND:20300101T010000Z\r\n"));
        let out = run(
            &a, &a, "2026-03-29", 1, "02:30", "05:00", 30, "Europe/Berlin", true, "text", NOW,
        )
        .unwrap();
        assert!(out.contains("Sun 2026-03-29"), "{out}");
    }
}
