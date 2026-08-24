//! gizza-ai/ics-agenda-view core — turn one pasted iCalendar (.ics) calendar
//! into a day-by-day agenda, and report the free gaps between the meetings of
//! each day.
//!
//! Pure compute, shared by the chat skill block, the web page and the CLI. No
//! clock and no I/O: the caller picks the window (`start_date` + `days`), or
//! leaves `start_date` empty and the agenda starts at the calendar's earliest
//! event — so every surface produces the same output for the same input.
//!
//! RFC 5545 handling is done by hand (line unfolding, property + parameter
//! parsing, TEXT unescaping, DATE/DATE-TIME parsing, a bounded RRULE subset
//! with EXDATE). Timezone/DST math uses chrono-tz (IANA db baked in, proven
//! wasm-safe in blocks/timezone-convert).

use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Weekday};
use chrono_tz::Tz;
use serde::Serialize;
use std::collections::HashSet;

/// Input cap (bytes). A multi-year export of a busy calendar fits well under
/// this; the cap keeps the wasm sandbox comfortable.
pub const MAX_ICS_BYTES: usize = 1_048_576; // 1 MiB
/// Largest agenda window, in days.
pub const MAX_DAYS: i64 = 90;
/// Cap on event occurrences after recurrence expansion.
pub const MAX_OCCURRENCES: usize = 5_000;
/// Minimum-gap bounds (minutes).
pub const MIN_GAP_LO: i64 = 5;
pub const MIN_GAP_HI: i64 = 480;
/// Global recurrence-expansion iteration budget (all rules combined).
const RRULE_ITER_BUDGET: i64 = 200_000;
/// Longest description rendered in `details = full` before truncation.
const MAX_DESC_CHARS: usize = 200;

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
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    for c in head.chars() {
        match c {
            '"' => {
                q = !q;
                cur.push(c);
            }
            ';' if !q => parts.push(std::mem::take(&mut cur)),
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

/// Decode RFC 5545 TEXT escapes (`\n`, `\,`, `\;`, `\\`).
fn unescape_text(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') | Some('N') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Collapse a multi-line TEXT value onto one line for the agenda listing.
fn one_line(v: &str) -> String {
    let mut s = String::with_capacity(v.len());
    let mut last_space = false;
    for c in v.chars() {
        let c = if c == '\n' || c == '\r' || c == '\t' {
            ' '
        } else {
            c
        };
        if c == ' ' {
            if last_space {
                continue;
            }
            last_space = true;
        } else {
            last_space = false;
        }
        s.push(c);
    }
    s.trim().to_string()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{}…", cut.trim_end())
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

/// Resolve a TZID onto a chrono-tz zone. None for unknown ids (the caller then
/// reads the time in the display timezone and records a warning).
fn resolve_tzid(tzid: &str) -> Option<Tz> {
    let t = tzid.trim().trim_start_matches('/');
    t.parse::<Tz>().ok().or_else(|| windows_tz(t))
}

/// Convert a wall-clock time in `tz` to a UTC epoch, resolving DST folds to the
/// earlier instant and DST gaps by shifting forward one hour.
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

/// A parsed DTSTART/DTEND/EXDATE value: wall time plus the zone it lives in.
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
/// times, all-day dates and unknown TZIDs (the latter sets `*tz_fallback`).
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

/// Parse an ISO-8601 duration as used by ICS (`P1D`, `PT1H30M`, `P1W`, …) into
/// seconds. Negative durations clamp to 0.
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
                secs += n * match c.to_ascii_uppercase() {
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
// RRULE (bounded subset)
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
    /// MONTHLY: a single ordinal weekday (e.g. `3MO` = third Monday).
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

/// Parse an RRULE value into the supported subset. Ok(None) means "outside the
/// subset" — the caller lists the event once, at its start, and warns.
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
            "WKST" => {}
            "BYMONTH" => {} // YEARLY from DTSTART already fixes the month
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

/// The `ord`th (1-based; negative = counted from the end) `w` weekday of a month.
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
// Calendar parsing → events → occurrences
// ---------------------------------------------------------------------------

/// One VEVENT, as raw property values plus the display metadata the agenda
/// prints. Times stay unparsed here so the display timezone can be applied
/// afterwards.
#[derive(Default, Clone)]
struct Event {
    dtstart: Option<(String, Vec<(String, String)>)>,
    dtend: Option<(String, Vec<(String, String)>)>,
    duration: Option<String>,
    rrule: Option<String>,
    exdates: Vec<(String, Vec<(String, String)>)>,
    uid: String,
    summary: String,
    location: String,
    description: String,
    organizer: String,
    status: String,
}

impl Event {
    fn title(&self) -> &str {
        if self.summary.is_empty() {
            "(untitled event)"
        } else {
            &self.summary
        }
    }

    fn cancelled(&self) -> bool {
        self.status.eq_ignore_ascii_case("CANCELLED")
    }

    fn matches(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let hay = format!(
            "{} {} {} {}",
            self.summary, self.location, self.description, self.organizer
        )
        .to_lowercase();
        hay.contains(needle)
    }
}

/// A single expanded occurrence of an event, as UTC epochs.
#[derive(Clone, Copy, Debug)]
struct Occ {
    start: i64,
    end: i64,
    all_day: bool,
    idx: usize,
}

/// Parse an ORGANIZER / ATTENDEE value into a readable "Name <mail>" string.
fn person(value: &str, params: &[(String, String)]) -> String {
    let mail = value
        .trim()
        .trim_start_matches("mailto:")
        .trim_start_matches("MAILTO:")
        .trim()
        .to_string();
    let cn = param(params, "CN").map(unescape_text).unwrap_or_default();
    match (cn.trim().is_empty(), mail.is_empty()) {
        (true, true) => String::new(),
        (true, false) => mail,
        (false, true) => cn.trim().to_string(),
        (false, false) => format!("{} <{}>", cn.trim(), mail),
    }
}

/// Read every VEVENT out of the calendar text.
fn parse_calendar(text: &str) -> Result<Vec<Event>, String> {
    if text.trim().is_empty() {
        return Err(
            "the calendar is empty — paste the full .ics text (it starts with BEGIN:VCALENDAR)"
                .into(),
        );
    }
    if text.len() > MAX_ICS_BYTES {
        return Err(format!(
            "calendar is too large ({} bytes; the limit is {MAX_ICS_BYTES} bytes = 1 MiB)",
            text.len()
        ));
    }
    // Shell-friendliness: a calendar pasted as ONE line with literal `\n`
    // escape sequences (what quoting does to the copyable CLI example) is
    // unescaped first. Only kicks in when the text has no real newlines, so
    // genuine ICS content is never rewritten.
    let unescaped;
    let text = if !text.contains('\n') && text.contains("\\n") {
        unescaped = text.replace("\\r\\n", "\n").replace("\\n", "\n");
        unescaped.as_str()
    } else {
        text
    };

    let lines = unfold(text);
    let mut stack: Vec<String> = Vec::new();
    let mut cur: Option<Event> = None;
    let mut events: Vec<Event> = Vec::new();
    let mut saw_vevent = false;

    for line in &lines {
        let Some((name, params, value)) = parse_prop(line) else {
            continue;
        };
        match name.as_str() {
            "BEGIN" => {
                let comp = value.trim().to_ascii_uppercase();
                if comp == "VEVENT" && !stack.iter().any(|c| c == "VEVENT") {
                    cur = Some(Event::default());
                    saw_vevent = true;
                }
                stack.push(comp);
            }
            "END" => {
                let comp = value.trim().to_ascii_uppercase();
                if comp == "VEVENT" {
                    if let Some(ev) = cur.take() {
                        events.push(ev);
                    }
                }
                while let Some(top) = stack.pop() {
                    if top == comp {
                        break;
                    }
                }
            }
            // Only properties directly inside VEVENT count — VALARM and
            // VTIMEZONE sub-components must not leak into the event.
            _ if stack.last().map(String::as_str) == Some("VEVENT") => {
                if let Some(ev) = cur.as_mut() {
                    match name.as_str() {
                        "DTSTART" => ev.dtstart = Some((value, params)),
                        "DTEND" => ev.dtend = Some((value, params)),
                        "DURATION" => ev.duration = Some(value),
                        "RRULE" => ev.rrule = Some(value),
                        "EXDATE" => ev.exdates.push((value, params)),
                        "UID" => ev.uid = value.trim().to_string(),
                        "SUMMARY" => ev.summary = one_line(&unescape_text(&value)),
                        "LOCATION" => ev.location = one_line(&unescape_text(&value)),
                        "DESCRIPTION" => ev.description = one_line(&unescape_text(&value)),
                        "ORGANIZER" => ev.organizer = person(&value, &params),
                        "STATUS" => ev.status = value.trim().to_ascii_uppercase(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if !saw_vevent {
        return Err(
            "this doesn't look like iCalendar data — no BEGIN:VEVENT block was found \
                    (paste the raw .ics text exported from your calendar app)"
                .into(),
        );
    }
    Ok(events)
}

/// Occurrence length for an event: (whole days, seconds). All-day events count
/// in wall days so DST can't shorten them.
fn event_length(
    ev: &Event,
    start: &IcsDt,
    tz: Tz,
    unknown: &mut bool,
) -> Result<(i64, i64), String> {
    if let Some((v, p)) = &ev.dtend {
        let end = parse_ics_dt(v, p, tz, unknown)?;
        if start.all_day {
            Ok(((end.naive.date() - start.naive.date()).num_days().max(1), 0))
        } else {
            Ok((0, (end.epoch() - start.epoch()).max(0)))
        }
    } else if let Some(d) = &ev.duration {
        let secs = parse_ics_duration(d)?;
        if start.all_day {
            Ok(((secs / 86_400).max(1), 0))
        } else {
            Ok((0, secs))
        }
    } else if start.all_day {
        Ok((1, 0))
    } else {
        // RFC 5545: a DATE-TIME start with neither DTEND nor DURATION takes no
        // time — it still shows on the agenda as a point in the day.
        Ok((0, 0))
    }
}

/// Expand one event into the occurrences overlapping `win`, appending to `out`.
#[allow(clippy::too_many_arguments)]
fn expand_event(
    ev: &Event,
    idx: usize,
    tz: Tz,
    win: (i64, i64),
    expand_recurring: bool,
    out: &mut Vec<Occ>,
    budget: &mut i64,
    unsupported: &mut usize,
    unknown_tzids: &mut bool,
) -> Result<(), String> {
    let Some((sv, sp)) = &ev.dtstart else {
        return Ok(()); // no DTSTART → not placeable on an agenda
    };
    let start = parse_ics_dt(sv, sp, tz, unknown_tzids)?;
    let (dur_days, dur_secs) = event_length(ev, &start, tz, unknown_tzids)?;

    // EXDATE set: epochs for timed events, dates for all-day ones.
    let mut ex_epochs: HashSet<i64> = HashSet::new();
    let mut ex_dates: HashSet<NaiveDate> = HashSet::new();
    for (v, p) in &ev.exdates {
        for one in v.split(',') {
            if one.trim().is_empty() {
                continue;
            }
            let dt = parse_ics_dt(one, p, tz, unknown_tzids)?;
            if dt.all_day {
                ex_dates.insert(dt.naive.date());
            } else {
                ex_epochs.insert(dt.epoch());
            }
        }
    }

    let push = |occ: IcsDt, out: &mut Vec<Occ>| {
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
        // Zero-length events still belong on the day they start.
        let overlaps = s < win.1 && (e > win.0 || (e == s && s >= win.0));
        if overlaps {
            out.push(Occ {
                start: s,
                end: e,
                all_day: occ.all_day,
                idx,
            });
        }
    };

    let rule = match &ev.rrule {
        None => {
            push(start, out);
            return Ok(());
        }
        Some(_) if !expand_recurring => {
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
                "recurrence expansion is too large — shorten the agenda window (days) or turn expand_recurring off"
                    .into(),
            );
        }
        let mut occs: Vec<NaiveDateTime> = Vec::new();
        match rule.freq {
            Freq::Daily => occs.push(base + Duration::days(k * rule.interval)),
            Freq::Weekly => {
                if rule.byday.is_empty() {
                    occs.push(base + Duration::weeks(k * rule.interval));
                } else {
                    let week0_mon =
                        base.date() - Duration::days(base.weekday().num_days_from_monday() as i64);
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
            if s + occ_len > win.0 || (occ_len == 0 && s >= win.0) {
                push(occ, out);
                if out.len() > MAX_OCCURRENCES {
                    return Err(format!(
                        "too many event occurrences after recurrence expansion (limit {MAX_OCCURRENCES}) — shorten the agenda window (days)"
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

/// Segments of `window` not covered by (merged, sorted) `busy`.
fn subtract(window: (i64, i64), busy: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let mut cursor = window.0;
    for &(s, e) in busy {
        if e <= cursor {
            continue;
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
    out.retain(|(s, e)| e > s);
    out
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

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

/// "HH:MM" in tz; midnight closing a window renders as 24:00.
fn fmt_hm(tz: Tz, epoch: i64, end_of_window: bool) -> String {
    let s = in_tz(tz, epoch).format("%H:%M").to_string();
    if end_of_window && s == "00:00" {
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
        return Err(format!(
            "{name} must be between 00:00 and 24:00 — got {t:?}"
        ));
    }
    Ok((h, m))
}

/// How much per-event detail each line carries.
#[derive(Clone, Copy, PartialEq)]
enum Details {
    Compact,
    Normal,
    Full,
}

impl Details {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "normal" => Ok(Details::Normal),
            "compact" => Ok(Details::Compact),
            "full" => Ok(Details::Full),
            other => Err(format!(
                "details must be one of compact, normal, full — got {other:?}"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// JSON shape
// ---------------------------------------------------------------------------

/// serde predicate: leave a false flag out of the JSON entirely.
fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize)]
struct JsonEvent {
    summary: String,
    start: String,
    end: String,
    start_iso: String,
    end_iso: String,
    minutes: i64,
    all_day: bool,
    #[serde(skip_serializing_if = "is_false")]
    recurring: bool,
    #[serde(skip_serializing_if = "is_false")]
    cancelled: bool,
    #[serde(skip_serializing_if = "is_false")]
    continued: bool,
    #[serde(skip_serializing_if = "is_false")]
    continues: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    location: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    organizer: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    uid: String,
}

#[derive(Serialize)]
struct JsonGap {
    start: String,
    end: String,
    minutes: i64,
}

#[derive(Serialize)]
struct JsonDay {
    date: String,
    weekday: String,
    events: Vec<JsonEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gaps: Option<Vec<JsonGap>>,
    busy_minutes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    free_minutes: Option<i64>,
}

#[derive(Serialize)]
struct JsonRange {
    start_date: String,
    end_date: String,
    days: i64,
}

#[derive(Serialize)]
struct JsonTotals {
    events: i64,
    busy_minutes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    gaps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    free_minutes: Option<i64>,
}

#[derive(Serialize)]
struct JsonOut {
    timezone: String,
    range: JsonRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    gap_window: Option<JsonGapWindow>,
    days: Vec<JsonDay>,
    totals: JsonTotals,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct JsonGapWindow {
    day_start: String,
    day_end: String,
    min_gap_minutes: i64,
}

/// One rendered agenda line for an event, pre-clipped to its day.
struct DayEvent {
    occ: Occ,
    /// Times clipped to the day the line appears on.
    disp_start: i64,
    disp_end: i64,
    continued: bool,
    continues: bool,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Build the agenda. See the block descriptor for parameter semantics.
#[allow(clippy::too_many_arguments)]
pub fn run(
    ics: &str,
    start_date: &str,
    days: i64,
    timezone: &str,
    day_start: &str,
    day_end: &str,
    min_gap_minutes: i64,
    show_gaps: bool,
    filter: &str,
    expand_recurring: bool,
    include_cancelled: bool,
    details: &str,
    output: &str,
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
        return Err(format!(
            "days must be between 1 and {MAX_DAYS} — got {days}"
        ));
    }
    if !(MIN_GAP_LO..=MIN_GAP_HI).contains(&min_gap_minutes) {
        return Err(format!(
            "min_gap_minutes must be between {MIN_GAP_LO} and {MIN_GAP_HI} — got {min_gap_minutes}"
        ));
    }
    let details = Details::parse(details)?;
    let output = if output.trim().is_empty() {
        "text"
    } else {
        output.trim()
    };
    if !matches!(output, "text" | "markdown" | "json") {
        return Err(format!(
            "output must be one of text, markdown, json — got {output:?}"
        ));
    }
    let (sh, sm) = parse_hm(day_start, "day_start", (9, 0))?;
    let (eh, em) = parse_hm(day_end, "day_end", (18, 0))?;
    let start_min = (sh * 60 + sm) as i64;
    let end_min = (eh * 60 + em) as i64;
    if end_min <= start_min {
        return Err(format!(
            "day_end ({eh:02}:{em:02}) must be after day_start ({sh:02}:{sm:02})"
        ));
    }
    let needle = filter.trim().to_lowercase();

    // ---- parse the calendar ----
    let all_events = parse_calendar(ics)?;
    let mut warnings: Vec<String> = Vec::new();
    let mut unknown_tzids = false;

    // Keep only the events this agenda should list.
    let mut skipped_cancelled = 0usize;
    let events: Vec<Event> = all_events
        .into_iter()
        .filter(|e| {
            if e.cancelled() && !include_cancelled {
                skipped_cancelled += 1;
                return false;
            }
            e.matches(&needle)
        })
        .collect();

    // ---- pick the window ----
    // Empty start_date = the day of the calendar's earliest event. A recurrence
    // never starts before its DTSTART, so the minimum DTSTART is the earliest
    // occurrence in the whole calendar.
    let first_day: NaiveDate = if start_date.trim().is_empty() {
        let mut earliest: Option<i64> = None;
        for ev in &events {
            if let Some((v, p)) = &ev.dtstart {
                let dt = parse_ics_dt(v, p, tz, &mut unknown_tzids)?;
                let e = dt.epoch();
                earliest = Some(earliest.map_or(e, |cur: i64| cur.min(e)));
            }
        }
        match earliest {
            Some(e) => in_tz(tz, e).date_naive(),
            None if needle.is_empty() => {
                return Err("no events with a start time were found in this calendar".into())
            }
            None => {
                return Err(format!(
                    "no events match the filter {:?} — clear the filter or try another word",
                    filter.trim()
                ))
            }
        }
    } else {
        let s = start_date.trim().replace('/', "-");
        NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|_| {
            format!("start_date must be YYYY-MM-DD, e.g. 2026-03-09 — got {start_date:?}")
        })?
    };
    let last_day = first_day + Duration::days(days - 1);
    let win_start = localize(tz, first_day.and_hms_opt(0, 0, 0).expect("midnight"));
    let win_end = localize(
        tz,
        (last_day + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .expect("midnight"),
    );

    // ---- expand occurrences ----
    let mut budget = RRULE_ITER_BUDGET;
    let mut unsupported = 0usize;
    let mut occs: Vec<Occ> = Vec::new();
    for (idx, ev) in events.iter().enumerate() {
        expand_event(
            ev,
            idx,
            tz,
            (win_start, win_end),
            expand_recurring,
            &mut occs,
            &mut budget,
            &mut unsupported,
            &mut unknown_tzids,
        )?;
    }
    if unsupported > 0 {
        warnings.push(format!(
            "{unsupported} recurring event(s) use a recurrence rule outside the supported subset — each is listed once, at its start date"
        ));
    }
    if unknown_tzids {
        warnings.push(format!(
            "some events use a TZID this tool doesn't know — those times were read as {tz_name}"
        ));
    }
    if skipped_cancelled > 0 {
        warnings.push(format!(
            "{skipped_cancelled} cancelled event(s) were hidden — set include_cancelled to list them"
        ));
    }

    // ---- lay the occurrences out day by day ----
    let mut day_rows: Vec<(NaiveDate, Vec<DayEvent>, Vec<(i64, i64)>)> = Vec::new();
    let mut total_events: i64 = 0;
    let mut total_busy: i64 = 0;
    let mut total_gaps: i64 = 0;
    let mut total_free: i64 = 0;

    for d in 0..days {
        let date = first_day + Duration::days(d);
        let midnight = date.and_hms_opt(0, 0, 0).expect("midnight");
        let d0 = localize(tz, midnight);
        let d1 = localize(tz, midnight + Duration::days(1));

        let mut rows: Vec<DayEvent> = Vec::new();
        for occ in &occs {
            let overlaps =
                occ.start < d1 && (occ.end > d0 || (occ.end == occ.start && occ.start >= d0));
            if !overlaps {
                continue;
            }
            rows.push(DayEvent {
                occ: *occ,
                disp_start: occ.start.max(d0),
                disp_end: occ.end.min(d1),
                continued: occ.start < d0,
                continues: occ.end > d1,
            });
        }
        // All-day events first, then by start time, then by title.
        rows.sort_by(|a, b| {
            b.occ
                .all_day
                .cmp(&a.occ.all_day)
                .then(a.disp_start.cmp(&b.disp_start))
                .then(a.disp_end.cmp(&b.disp_end))
                .then_with(|| events[a.occ.idx].title().cmp(events[b.occ.idx].title()))
        });
        total_events += rows.len() as i64;

        // Busy time = timed events only; all-day events don't block a working day.
        let busy_all: Vec<(i64, i64)> = rows
            .iter()
            .filter(|r| !r.occ.all_day && r.disp_end > r.disp_start)
            .map(|r| (r.disp_start, r.disp_end))
            .collect();
        total_busy += merge(busy_all.clone())
            .iter()
            .map(|(s, e)| (e - s) / 60)
            .sum::<i64>();

        let mut gaps: Vec<(i64, i64)> = Vec::new();
        if show_gaps {
            let ws = localize(tz, midnight + Duration::minutes(start_min));
            let we = localize(tz, midnight + Duration::minutes(end_min));
            if we > ws {
                let busy_in_window: Vec<(i64, i64)> = merge(busy_all)
                    .into_iter()
                    .filter_map(|(s, e)| {
                        let (s, e) = (s.max(ws), e.min(we));
                        (e > s).then_some((s, e))
                    })
                    .collect();
                for (s, e) in subtract((ws, we), &busy_in_window) {
                    if (e - s) / 60 >= min_gap_minutes {
                        gaps.push((s, e));
                    }
                }
            }
            total_gaps += gaps.len() as i64;
            total_free += gaps.iter().map(|(s, e)| (e - s) / 60).sum::<i64>();
        }

        // With gaps off an empty day carries no information, so skip it.
        if rows.is_empty() && !show_gaps {
            continue;
        }
        day_rows.push((date, rows, gaps));
    }

    // ---- render ----
    let gap_note = format!(
        "gaps {sh:02}:{sm:02}-{eh:02}:{em:02}, at least {}",
        fmt_dur(min_gap_minutes)
    );
    match output {
        "json" => {
            let out = JsonOut {
                timezone: tz_name.to_string(),
                range: JsonRange {
                    start_date: first_day.format("%Y-%m-%d").to_string(),
                    end_date: last_day.format("%Y-%m-%d").to_string(),
                    days,
                },
                gap_window: show_gaps.then(|| JsonGapWindow {
                    day_start: format!("{sh:02}:{sm:02}"),
                    day_end: format!("{eh:02}:{em:02}"),
                    min_gap_minutes,
                }),
                days: day_rows
                    .iter()
                    .map(|(date, rows, gaps)| JsonDay {
                        date: date.format("%Y-%m-%d").to_string(),
                        weekday: date.format("%a").to_string(),
                        events: rows
                            .iter()
                            .map(|r| {
                                let ev = &events[r.occ.idx];
                                JsonEvent {
                                    summary: ev.title().to_string(),
                                    start: if r.occ.all_day {
                                        "all-day".into()
                                    } else {
                                        fmt_hm(tz, r.disp_start, false)
                                    },
                                    end: if r.occ.all_day {
                                        "all-day".into()
                                    } else {
                                        fmt_hm(tz, r.disp_end, true)
                                    },
                                    start_iso: in_tz(tz, r.occ.start)
                                        .format("%Y-%m-%dT%H:%M:%S%:z")
                                        .to_string(),
                                    end_iso: in_tz(tz, r.occ.end)
                                        .format("%Y-%m-%dT%H:%M:%S%:z")
                                        .to_string(),
                                    minutes: (r.occ.end - r.occ.start) / 60,
                                    all_day: r.occ.all_day,
                                    recurring: ev.rrule.is_some(),
                                    cancelled: ev.cancelled(),
                                    continued: r.continued,
                                    continues: r.continues,
                                    location: ev.location.clone(),
                                    description: if details == Details::Full {
                                        truncate_chars(&ev.description, MAX_DESC_CHARS)
                                    } else {
                                        String::new()
                                    },
                                    organizer: ev.organizer.clone(),
                                    status: ev.status.clone(),
                                    uid: ev.uid.clone(),
                                }
                            })
                            .collect(),
                        gaps: show_gaps.then(|| {
                            gaps.iter()
                                .map(|(s, e)| JsonGap {
                                    start: fmt_hm(tz, *s, false),
                                    end: fmt_hm(tz, *e, true),
                                    minutes: (e - s) / 60,
                                })
                                .collect()
                        }),
                        busy_minutes: merge(
                            rows.iter()
                                .filter(|r| !r.occ.all_day && r.disp_end > r.disp_start)
                                .map(|r| (r.disp_start, r.disp_end))
                                .collect(),
                        )
                        .iter()
                        .map(|(s, e)| (e - s) / 60)
                        .sum(),
                        free_minutes: show_gaps
                            .then(|| gaps.iter().map(|(s, e)| (e - s) / 60).sum::<i64>()),
                    })
                    .collect(),
                totals: JsonTotals {
                    events: total_events,
                    busy_minutes: total_busy,
                    gaps: show_gaps.then_some(total_gaps),
                    free_minutes: show_gaps.then_some(total_free),
                },
                warnings,
            };
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
        }
        "markdown" => {
            let mut s = format!(
                "# Agenda: {} to {} ({tz_name})\n",
                first_day.format("%Y-%m-%d"),
                last_day.format("%Y-%m-%d")
            );
            if show_gaps {
                s.push_str(&format!("\nFree {gap_note}.\n"));
            }
            if day_rows.is_empty() {
                s.push_str("\nNo events in this window.\n");
            }
            for (date, rows, gaps) in &day_rows {
                s.push_str(&format!("\n## {}\n\n", date.format("%A, %Y-%m-%d")));
                if rows.is_empty() {
                    s.push_str("- _no events_\n");
                }
                let mut gi = 0usize;
                for r in rows {
                    while gi < gaps.len() && gaps[gi].0 <= r.disp_start && !r.occ.all_day {
                        s.push_str(&format!(
                            "- _free {} ({}-{})_\n",
                            fmt_dur((gaps[gi].1 - gaps[gi].0) / 60),
                            fmt_hm(tz, gaps[gi].0, false),
                            fmt_hm(tz, gaps[gi].1, true)
                        ));
                        gi += 1;
                    }
                    s.push_str(&format!(
                        "- **{}** — {}\n",
                        time_col(tz, r),
                        event_text(&events[r.occ.idx], r, details)
                    ));
                    if details == Details::Full {
                        for line in extra_lines(&events[r.occ.idx]) {
                            s.push_str(&format!("  - {line}\n"));
                        }
                    }
                }
                for g in &gaps[gi..] {
                    s.push_str(&format!(
                        "- _free {} ({}-{})_\n",
                        fmt_dur((g.1 - g.0) / 60),
                        fmt_hm(tz, g.0, false),
                        fmt_hm(tz, g.1, true)
                    ));
                }
            }
            s.push_str(&format!(
                "\n{}\n",
                totals_line(
                    total_events,
                    total_busy,
                    show_gaps.then_some((total_gaps, total_free))
                )
            ));
            for w in &warnings {
                s.push_str(&format!("\n> Note: {w}\n"));
            }
            Ok(s)
        }
        _ => {
            let mut s = format!(
                "Agenda {} to {} · {tz_name}\n",
                first_day.format("%Y-%m-%d"),
                last_day.format("%Y-%m-%d")
            );
            if show_gaps {
                s.push_str(&format!("Free {gap_note}\n"));
            }
            if day_rows.is_empty() {
                s.push_str("\nNo events in this window.\n");
            }
            for (date, rows, gaps) in &day_rows {
                s.push_str(&format!("\n{}\n", date.format("%a %Y-%m-%d")));
                if rows.is_empty() {
                    s.push_str("  (no events)\n");
                }
                let mut gi = 0usize;
                for r in rows {
                    while gi < gaps.len() && gaps[gi].0 <= r.disp_start && !r.occ.all_day {
                        s.push_str(&format!(
                            "    free {} ({}-{})\n",
                            fmt_dur((gaps[gi].1 - gaps[gi].0) / 60),
                            fmt_hm(tz, gaps[gi].0, false),
                            fmt_hm(tz, gaps[gi].1, true)
                        ));
                        gi += 1;
                    }
                    s.push_str(&format!(
                        "  {:<13} {}\n",
                        time_col(tz, r),
                        event_text(&events[r.occ.idx], r, details)
                    ));
                    if details == Details::Full {
                        for line in extra_lines(&events[r.occ.idx]) {
                            s.push_str(&format!("                {line}\n"));
                        }
                    }
                }
                for g in &gaps[gi..] {
                    s.push_str(&format!(
                        "    free {} ({}-{})\n",
                        fmt_dur((g.1 - g.0) / 60),
                        fmt_hm(tz, g.0, false),
                        fmt_hm(tz, g.1, true)
                    ));
                }
            }
            s.push_str(&format!(
                "\n{}\n",
                totals_line(
                    total_events,
                    total_busy,
                    show_gaps.then_some((total_gaps, total_free))
                )
            ));
            for w in &warnings {
                s.push_str(&format!("Note: {w}\n"));
            }
            Ok(s)
        }
    }
}

/// The time column for one agenda line ("all-day", "09:00-09:30", or "09:00"
/// for a zero-length event).
fn time_col(tz: Tz, r: &DayEvent) -> String {
    if r.occ.all_day {
        return "all-day".to_string();
    }
    if r.disp_end == r.disp_start {
        return fmt_hm(tz, r.disp_start, false);
    }
    format!(
        "{}-{}",
        fmt_hm(tz, r.disp_start, false),
        fmt_hm(tz, r.disp_end, true)
    )
}

/// Title plus the inline detail this `details` level shows.
fn event_text(ev: &Event, r: &DayEvent, details: Details) -> String {
    let mut s = ev.title().to_string();
    if details != Details::Compact && !ev.location.is_empty() {
        s.push_str(&format!(" · {}", ev.location));
    }
    let mut marks: Vec<&str> = Vec::new();
    if ev.rrule.is_some() {
        marks.push("repeats");
    }
    if ev.cancelled() {
        marks.push("cancelled");
    }
    if r.continued {
        marks.push("continued");
    }
    if r.continues {
        marks.push("continues");
    }
    if details != Details::Compact && !marks.is_empty() {
        s.push_str(&format!(" ({})", marks.join(", ")));
    }
    s
}

/// Extra indented lines for `details = full`.
fn extra_lines(ev: &Event) -> Vec<String> {
    let mut out = Vec::new();
    if !ev.organizer.is_empty() {
        out.push(format!("organizer: {}", ev.organizer));
    }
    if !ev.status.is_empty() {
        out.push(format!("status: {}", ev.status));
    }
    if !ev.description.is_empty() {
        out.push(format!(
            "notes: {}",
            truncate_chars(&ev.description, MAX_DESC_CHARS)
        ));
    }
    out
}

fn totals_line(events: i64, busy: i64, gaps: Option<(i64, i64)>) -> String {
    let mut s = format!(
        "Totals: {events} event{} · {} booked",
        if events == 1 { "" } else { "s" },
        fmt_dur(busy)
    );
    if let Some((n, free)) = gaps {
        s.push_str(&format!(
            " · {n} free gap{} · {} free",
            if n == 1 { "" } else { "s" },
            fmt_dur(free)
        ));
    }
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_MEETINGS: &str = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VEVENT\nUID:a@x\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Standup\nLOCATION:Room 2\nEND:VEVENT\nBEGIN:VEVENT\nUID:b@x\nDTSTART:20260309T110000Z\nDTEND:20260309T120000Z\nSUMMARY:Design review\nEND:VEVENT\nEND:VCALENDAR";

    fn agenda(ics: &str) -> String {
        run(
            ics, "", 1, "UTC", "09:00", "18:00", 30, true, "", true, false, "normal", "text",
        )
        .unwrap()
    }

    #[test]
    fn groups_events_by_day_and_finds_gaps() {
        let out = agenda(TWO_MEETINGS);
        let expected = [
            "Agenda 2026-03-09 to 2026-03-09 · UTC",
            "Free gaps 09:00-18:00, at least 30m",
            "",
            "Mon 2026-03-09",
            "  09:00-09:30   Standup · Room 2",
            "    free 1h 30m (09:30-11:00)",
            "  11:00-12:00   Design review",
            "    free 6h (12:00-18:00)",
            "",
            "Totals: 2 events · 1h 30m booked · 2 free gaps · 7h 30m free",
            "",
        ]
        .join("\n");
        assert_eq!(out, expected);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run(
            "", "", 1, "UTC", "09:00", "18:00", 30, true, "", true, false, "normal", "text",
        )
        .unwrap_err();
        assert!(err.contains("BEGIN:VCALENDAR"), "{err}");
    }

    #[test]
    fn non_calendar_text_is_an_error() {
        let err = agenda_err("hello world\nnot a calendar");
        assert!(err.contains("BEGIN:VEVENT"), "{err}");
    }

    fn agenda_err(ics: &str) -> String {
        run(
            ics, "", 1, "UTC", "09:00", "18:00", 30, true, "", true, false, "normal", "text",
        )
        .unwrap_err()
    }

    #[test]
    fn rejects_bad_params() {
        let e = run(
            TWO_MEETINGS,
            "",
            0,
            "UTC",
            "09:00",
            "18:00",
            30,
            true,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap_err();
        assert!(e.contains("days must be between 1 and 90"), "{e}");

        let e = run(
            TWO_MEETINGS,
            "",
            1,
            "Mars/Olympus",
            "09:00",
            "18:00",
            30,
            true,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap_err();
        assert!(e.contains("unknown timezone"), "{e}");

        let e = run(
            TWO_MEETINGS,
            "",
            1,
            "UTC",
            "18:00",
            "09:00",
            30,
            true,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap_err();
        assert!(e.contains("must be after day_start"), "{e}");

        let e = run(
            TWO_MEETINGS,
            "",
            1,
            "UTC",
            "09:00",
            "18:00",
            30,
            true,
            "",
            true,
            false,
            "verbose",
            "text",
        )
        .unwrap_err();
        assert!(e.contains("details must be one of"), "{e}");

        let e = run(
            TWO_MEETINGS,
            "",
            1,
            "UTC",
            "09:00",
            "18:00",
            30,
            true,
            "",
            true,
            false,
            "normal",
            "yaml",
        )
        .unwrap_err();
        assert!(e.contains("output must be one of"), "{e}");
    }

    #[test]
    fn min_gap_filters_short_gaps() {
        // The 90-minute gap survives a 120-minute minimum only as the later one.
        let out = run(
            TWO_MEETINGS,
            "",
            1,
            "UTC",
            "09:00",
            "18:00",
            120,
            true,
            "",
            true,
            false,
            "compact",
            "text",
        )
        .unwrap();
        assert!(!out.contains("free 1h 30m"), "{out}");
        assert!(out.contains("free 6h (12:00-18:00)"), "{out}");
        assert!(out.contains("1 free gap ·"), "{out}");
    }

    #[test]
    fn gaps_off_hides_gaps_and_empty_days() {
        let out = run(
            TWO_MEETINGS,
            "2026-03-09",
            3,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(!out.contains("free "), "{out}");
        assert!(!out.contains("(no events)"), "{out}");
        assert!(out.contains("Totals: 2 events · 1h 30m booked\n"), "{out}");
    }

    #[test]
    fn empty_days_show_their_whole_free_window() {
        let out = run(
            TWO_MEETINGS,
            "2026-03-10",
            1,
            "UTC",
            "09:00",
            "17:00",
            30,
            true,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(out.contains("  (no events)"), "{out}");
        assert!(out.contains("free 8h (09:00-17:00)"), "{out}");
    }

    #[test]
    fn all_day_events_come_first_and_do_not_block_time() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;VALUE=DATE:20260309\nDTEND;VALUE=DATE:20260310\nSUMMARY:Team offsite\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Standup\nEND:VEVENT\nEND:VCALENDAR";
        let out = agenda(ics);
        let offsite = out.find("Team offsite").unwrap();
        let standup = out.find("Standup").unwrap();
        assert!(offsite < standup, "all-day first:\n{out}");
        assert!(out.contains("all-day"), "{out}");
        // Only the timed event counts as booked.
        assert!(out.contains("Totals: 2 events · 30m booked"), "{out}");
    }

    #[test]
    fn expands_weekly_recurrence_with_byday() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Standup\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE\nEND:VEVENT\nEND:VCALENDAR";
        let out = run(
            ics,
            "2026-03-09",
            7,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(out.contains("Mon 2026-03-09"), "{out}");
        assert!(out.contains("Wed 2026-03-11"), "{out}");
        assert!(!out.contains("Tue 2026-03-10"), "{out}");
        assert!(out.contains("(repeats)"), "{out}");
        // Mon 9th + Wed 11th; the next Monday (16th) is past the 7-day window.
        assert!(out.contains("Totals: 2 events"), "{out}");
    }

    #[test]
    fn exdate_removes_one_occurrence() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Standup\nRRULE:FREQ=DAILY;COUNT=3\nEXDATE:20260310T090000Z\nEND:VEVENT\nEND:VCALENDAR";
        let out = run(
            ics,
            "2026-03-09",
            3,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(out.contains("Totals: 2 events"), "{out}");
        assert!(!out.contains("Tue 2026-03-10"), "{out}");
    }

    #[test]
    fn expand_recurring_off_lists_the_series_once() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Standup\nRRULE:FREQ=DAILY\nEND:VEVENT\nEND:VCALENDAR";
        let out = run(
            ics,
            "2026-03-09",
            5,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "",
            false,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(out.contains("Totals: 1 event ·"), "{out}");
    }

    #[test]
    fn timezone_converts_event_times() {
        let out = run(
            TWO_MEETINGS,
            "2026-03-09",
            1,
            "Europe/Berlin",
            "09:00",
            "18:00",
            30,
            false,
            "",
            true,
            false,
            "compact",
            "text",
        )
        .unwrap();
        // 09:00Z is 10:00 in Berlin (CET, UTC+1 in March before the DST switch).
        assert!(out.contains("10:00-10:30"), "{out}");
    }

    #[test]
    fn cancelled_events_are_hidden_unless_asked_for() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Standup\nSTATUS:CANCELLED\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:20260309T110000Z\nDTEND:20260309T120000Z\nSUMMARY:Design review\nEND:VEVENT\nEND:VCALENDAR";
        let hidden = agenda(ics);
        assert!(!hidden.contains("Standup"), "{hidden}");
        assert!(
            hidden.contains("1 cancelled event(s) were hidden"),
            "{hidden}"
        );

        let shown = run(
            ics, "", 1, "UTC", "09:00", "18:00", 30, true, "", true, true, "normal", "text",
        )
        .unwrap();
        assert!(shown.contains("Standup"), "{shown}");
        assert!(shown.contains("(cancelled)"), "{shown}");
    }

    #[test]
    fn filter_keeps_only_matching_events() {
        let out = run(
            TWO_MEETINGS,
            "",
            1,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "design",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(out.contains("Design review"), "{out}");
        assert!(!out.contains("Standup"), "{out}");
        assert!(out.contains("Totals: 1 event"), "{out}");
    }

    #[test]
    fn filter_with_no_match_explains_itself() {
        let e = run(
            TWO_MEETINGS,
            "",
            1,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "zzz",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap_err();
        assert!(e.contains("no events match the filter"), "{e}");
    }

    #[test]
    fn details_levels_change_the_line() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Standup\nLOCATION:Room 2\nDESCRIPTION:Daily sync\\nBring notes\nORGANIZER;CN=Jane Doe:mailto:jane@example.com\nSTATUS:CONFIRMED\nEND:VEVENT\nEND:VCALENDAR";
        let compact = run(
            ics, "", 1, "UTC", "09:00", "18:00", 30, false, "", true, false, "compact", "text",
        )
        .unwrap();
        assert!(compact.contains("09:00-09:30   Standup\n"), "{compact}");
        assert!(!compact.contains("Room 2"), "{compact}");

        let normal = agenda(ics);
        assert!(normal.contains("Standup · Room 2"), "{normal}");
        assert!(!normal.contains("Jane Doe"), "{normal}");

        let full = run(
            ics, "", 1, "UTC", "09:00", "18:00", 30, false, "", true, false, "full", "text",
        )
        .unwrap();
        assert!(
            full.contains("organizer: Jane Doe <jane@example.com>"),
            "{full}"
        );
        assert!(full.contains("status: CONFIRMED"), "{full}");
        assert!(full.contains("notes: Daily sync Bring notes"), "{full}");
    }

    #[test]
    fn multi_day_event_is_clipped_and_marked() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T220000Z\nDTEND:20260310T020000Z\nSUMMARY:Release window\nEND:VEVENT\nEND:VCALENDAR";
        let out = run(
            ics,
            "2026-03-09",
            2,
            "UTC",
            "00:00",
            "24:00",
            30,
            false,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(
            out.contains("22:00-24:00   Release window (continues)"),
            "{out}"
        );
        assert!(
            out.contains("00:00-02:00   Release window (continued)"),
            "{out}"
        );
    }

    #[test]
    fn folded_lines_and_escapes_are_decoded() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Weekly sync\n  with the platform team\nLOCATION:Room 2\\, floor 3\nEND:VEVENT\nEND:VCALENDAR";
        let out = agenda(ics);
        assert!(out.contains("Weekly sync with the platform team"), "{out}");
        assert!(out.contains("Room 2, floor 3"), "{out}");
    }

    #[test]
    fn valarm_properties_do_not_leak_into_the_event() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Standup\nBEGIN:VALARM\nTRIGGER:-PT15M\nDESCRIPTION:Reminder\nEND:VALARM\nEND:VEVENT\nEND:VCALENDAR";
        let out = run(
            ics, "", 1, "UTC", "09:00", "18:00", 30, false, "", true, false, "full", "text",
        )
        .unwrap();
        assert!(!out.contains("Reminder"), "{out}");
    }

    #[test]
    fn json_output_carries_days_events_and_gaps() {
        let out = run(
            TWO_MEETINGS,
            "",
            1,
            "UTC",
            "09:00",
            "18:00",
            30,
            true,
            "",
            true,
            false,
            "normal",
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["timezone"], "UTC");
        assert_eq!(v["range"]["start_date"], "2026-03-09");
        assert_eq!(v["days"][0]["events"][0]["summary"], "Standup");
        assert_eq!(v["days"][0]["events"][0]["start"], "09:00");
        assert_eq!(v["days"][0]["events"][0]["location"], "Room 2");
        assert_eq!(v["days"][0]["gaps"][0]["minutes"], 90);
        assert_eq!(v["totals"]["events"], 2);
        assert_eq!(v["totals"]["free_minutes"], 450);
    }

    #[test]
    fn markdown_output_uses_day_headings() {
        let out = run(
            TWO_MEETINGS,
            "",
            1,
            "UTC",
            "09:00",
            "18:00",
            30,
            true,
            "",
            true,
            false,
            "normal",
            "markdown",
        )
        .unwrap();
        assert!(
            out.starts_with("# Agenda: 2026-03-09 to 2026-03-09 (UTC)"),
            "{out}"
        );
        assert!(out.contains("## Monday, 2026-03-09"), "{out}");
        assert!(
            out.contains("- **09:00-09:30** — Standup · Room 2"),
            "{out}"
        );
        assert!(out.contains("- _free 1h 30m (09:30-11:00)_"), "{out}");
    }

    #[test]
    fn single_line_escaped_input_is_accepted() {
        let one_line = TWO_MEETINGS.replace('\n', "\\n");
        let out = agenda(&one_line);
        assert!(out.contains("Standup"), "{out}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_ICS_BYTES + 1);
        let e = agenda_err(&big);
        assert!(e.contains("too large"), "{e}");
    }

    #[test]
    fn unsupported_rrule_is_listed_once_with_a_warning() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDTEND:20260309T093000Z\nSUMMARY:Odd rule\nRRULE:FREQ=MINUTELY;INTERVAL=30\nEND:VEVENT\nEND:VCALENDAR";
        let out = run(
            ics,
            "2026-03-09",
            2,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(out.contains("Totals: 1 event"), "{out}");
        assert!(out.contains("outside the supported subset"), "{out}");
    }

    #[test]
    fn duration_instead_of_dtend_is_honoured() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20260309T090000Z\nDURATION:PT45M\nSUMMARY:Interview\nEND:VEVENT\nEND:VCALENDAR";
        let out = agenda(ics);
        assert!(out.contains("09:00-09:45"), "{out}");
    }

    #[test]
    fn tzid_events_convert_into_the_display_zone() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;TZID=America/New_York:20260309T090000\nDTEND;TZID=America/New_York:20260309T100000\nSUMMARY:NY sync\nEND:VEVENT\nEND:VCALENDAR";
        let out = run(
            ics,
            "2026-03-09",
            1,
            "UTC",
            "00:00",
            "24:00",
            30,
            false,
            "",
            true,
            false,
            "compact",
            "text",
        )
        .unwrap();
        // 2026-03-09 is after the US DST switch: EDT = UTC-4.
        assert!(out.contains("13:00-14:00"), "{out}");
    }

    #[test]
    fn days_boundary_is_accepted_at_the_cap() {
        let out = run(
            TWO_MEETINGS,
            "2026-03-09",
            MAX_DAYS,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap();
        assert!(out.contains("2026-03-09 to 2026-06-06"), "{out}");
        let e = run(
            TWO_MEETINGS,
            "2026-03-09",
            MAX_DAYS + 1,
            "UTC",
            "09:00",
            "18:00",
            30,
            false,
            "",
            true,
            false,
            "normal",
            "text",
        )
        .unwrap_err();
        assert!(e.contains("days must be between 1 and 90"), "{e}");
    }
}
