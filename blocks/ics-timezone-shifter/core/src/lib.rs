//! gizza-ai/ics-timezone-shifter core — rewrite every event in an iCalendar
//! (.ics) document into a different timezone.
//!
//! Two things happen to each date-time property (`DTSTART`, `DTEND`, `DUE`,
//! `RECURRENCE-ID`, `EXDATE`, `RDATE` and the `UNTIL` anchor inside `RRULE`):
//!
//! 1. an **instant** is derived from the written value — in `convert` mode from
//!    the value's own zone (`Z` → UTC, a recognized `TZID` → that zone, anything
//!    else → the `from` zone); in `relabel` mode the wall-clock digits are simply
//!    declared to be in the target zone (the fix for an export stamped with the
//!    wrong zone);
//! 2. the instant is written back in the target zone, as `TZID` + local time
//!    (default), as a UTC `Z` value, or as a floating (zone-less) value.
//!
//! Date-only (`VALUE=DATE`, all-day) values never move. `DTSTAMP`, `CREATED` and
//! `LAST-MODIFIED` are UTC sync metadata and are left untouched. Input
//! `VTIMEZONE` blocks are dropped (nothing references them afterwards) and a
//! fresh one — built from the target zone's real DST transitions over the years
//! the calendar spans — is emitted when `TZID` values are written.
//!
//! Pure compute: `chrono` + `chrono-tz` (the IANA database, no I/O, no clock),
//! shared verbatim by the chat block, the CLI and the page.

use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone, Timelike,
    Utc,
};
use chrono_tz::Tz;

/// Maximum number of `VEVENT` blocks accepted in one run.
pub const MAX_EVENTS: usize = 5000;

/// Longest span of years a generated `VTIMEZONE` covers with real transitions.
const MAX_TZ_SPAN_YEARS: i32 = 20;

/// How the instant behind a written date-time value is decided.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// Same moment in time, re-expressed in the target zone: 09:00 New York
    /// becomes 15:00 Berlin. The default.
    Convert,
    /// Keep the wall-clock digits and declare them to be in the target zone —
    /// repairs a calendar exported with the wrong timezone stamped on it.
    Relabel,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "convert" => Ok(Mode::Convert),
            "relabel" | "reinterpret" => Ok(Mode::Relabel),
            other => Err(format!(
                "invalid mode '{other}': expected one of convert, relabel"
            )),
        }
    }
}

/// How the converted values are written back into the calendar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WriteAs {
    /// `DTSTART;TZID=Europe/Berlin:20240310T150000` plus a matching
    /// `VTIMEZONE`. The default.
    Tzid,
    /// `DTSTART:20240310T140000Z` — absolute UTC instants, no `VTIMEZONE`.
    Utc,
    /// `DTSTART:20240310T150000` — zone-less wall-clock times.
    Floating,
}

impl WriteAs {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "tzid" => Ok(WriteAs::Tzid),
            "utc" | "z" => Ok(WriteAs::Utc),
            "floating" | "local" => Ok(WriteAs::Floating),
            other => Err(format!(
                "invalid write_as '{other}': expected one of tzid, utc, floating"
            )),
        }
    }
}

/// Resolve an IANA timezone name. `UTC`, `Z`, `GMT` and case-insensitive
/// spellings of any zone name are accepted.
fn parse_zone(name: &str, which: &str) -> Result<Tz, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(format!(
            "{which} timezone is empty; give an IANA name like 'America/New_York', \
             'Europe/Berlin', 'Asia/Tokyo', or 'UTC'"
        ));
    }
    if let Ok(tz) = n.parse::<Tz>() {
        return Ok(tz);
    }
    let upper = n.to_ascii_uppercase();
    if matches!(upper.as_str(), "Z" | "GMT" | "UT") {
        return Ok(Tz::UTC);
    }
    // Tolerate a differently-cased spelling (america/new_york).
    let lower = n.to_ascii_lowercase();
    if let Some(found) = chrono_tz::TZ_VARIANTS
        .iter()
        .find(|tz| tz.name().to_ascii_lowercase() == lower)
    {
        return Ok(*found);
    }
    Err(format!(
        "unknown {which} timezone '{n}'; use an IANA name like 'America/New_York', \
         'Europe/Berlin', 'Asia/Tokyo', or 'UTC'"
    ))
}

// ---------------------------------------------------------------------------
// RFC 5545 line handling
// ---------------------------------------------------------------------------

/// Unfold content lines: a physical line starting with a space or tab continues
/// the previous one (the leading whitespace is dropped).
fn unfold(ics: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in ics.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            if let Some(last) = out.last_mut() {
                last.push_str(&line[1..]);
                continue;
            }
        }
        if line.trim().is_empty() {
            continue;
        }
        out.push(line.to_string());
    }
    out
}

/// Fold one logical line back to 75-octet physical lines (continuations start
/// with a single space), never splitting a UTF-8 character.
fn fold(line: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut budget = 75usize;
    for ch in line.chars() {
        let len = ch.len_utf8();
        if cur.len() + len > budget {
            out.push(cur);
            cur = String::from(" ");
            budget = 75;
        }
        cur.push(ch);
    }
    out.push(cur);
    out
}

/// `BEGIN:`/`END:` delimiter → (is_begin, upper-cased component name).
fn delimiter(line: &str) -> Option<(bool, String)> {
    let t = line.trim();
    let up = t.to_ascii_uppercase();
    if let Some(rest) = up.strip_prefix("BEGIN:") {
        Some((true, rest.trim().to_string()))
    } else {
        up.strip_prefix("END:")
            .map(|rest| (false, rest.trim().to_string()))
    }
}

/// Split a content line into its upper-cased NAME, its `;`-separated parameters
/// (name upper-cased, value verbatim) and the value after the `:`. A `:` inside
/// a quoted parameter value does not terminate the parameter list.
fn split_prop(line: &str) -> Option<(String, Vec<(String, String)>, String)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] != b';' && bytes[i] != b':' {
        i += 1;
    }
    if i == 0 || i >= bytes.len() {
        return None;
    }
    let name = line[..i].to_ascii_uppercase();
    let mut params: Vec<(String, String)> = Vec::new();
    while bytes[i] == b';' {
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != b'=' && bytes[j] != b';' && bytes[j] != b':' {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            return None;
        }
        let pname = line[start..j].to_ascii_uppercase();
        let vstart = j + 1;
        let mut k = vstart;
        let mut quoted = false;
        while k < bytes.len() {
            match bytes[k] {
                b'"' => quoted = !quoted,
                b';' | b':' if !quoted => break,
                _ => {}
            }
            k += 1;
        }
        if k >= bytes.len() {
            return None;
        }
        params.push((pname, line[vstart..k].to_string()));
        i = k;
    }
    if bytes[i] != b':' {
        return None;
    }
    Some((name, params, line[i + 1..].to_string()))
}

/// Re-assemble a property line from its parts.
fn join_prop(name: &str, params: &[(String, String)], value: &str) -> String {
    let mut s = String::from(name);
    for (k, v) in params {
        s.push(';');
        s.push_str(k);
        s.push('=');
        s.push_str(v);
    }
    s.push(':');
    s.push_str(value);
    s
}

fn param_value<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.trim_matches('"'))
}

// ---------------------------------------------------------------------------
// Date-time values
// ---------------------------------------------------------------------------

/// A parsed `YYYYMMDDTHHMMSS[Z]` value. Date-only values are rejected here — the
/// caller passes those through verbatim.
struct DtValue {
    naive: NaiveDateTime,
    utc: bool,
}

fn parse_datetime(raw: &str) -> Option<DtValue> {
    let s = raw.trim();
    let b = s.as_bytes();
    if b.len() < 15 || b[8] != b'T' {
        return None;
    }
    if b.len() > 16 || (b.len() == 16 && b[15] != b'Z') {
        return None;
    }
    let num = |a: usize, z: usize| -> Option<i64> { s.get(a..z)?.parse::<i64>().ok() };
    let date = NaiveDate::from_ymd_opt(num(0, 4)? as i32, num(4, 6)? as u32, num(6, 8)? as u32)?;
    // A leap second (SS = 60) is clamped to :59 — chrono has no slot for it here.
    let sec = num(13, 15)?.min(59);
    let naive = date.and_hms_opt(num(9, 11)? as u32, num(11, 13)? as u32, sec as u32)?;
    Some(DtValue {
        naive,
        utc: b.len() == 16,
    })
}

fn fmt_naive(dt: &NaiveDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}",
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

/// `+HHMM` (or `+HHMMSS` for the rare sub-minute historical offsets).
fn offset_ics(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let a = seconds.abs();
    if a % 60 == 0 {
        format!("{sign}{:02}{:02}", a / 3600, (a % 3600) / 60)
    } else {
        format!("{sign}{:02}{:02}{:02}", a / 3600, (a % 3600) / 60, a % 60)
    }
}

/// Turn a local wall-clock time into an instant. A time that does not exist (the
/// spring-forward gap) rolls forward an hour; an ambiguous time (the autumn
/// repeat) takes the earlier — i.e. still-daylight — offset.
fn local_to_utc(naive: NaiveDateTime, tz: Tz) -> DateTime<Utc> {
    match tz.from_local_datetime(&naive) {
        LocalResult::Single(d) => d.with_timezone(&Utc),
        LocalResult::Ambiguous(a, _) => a.with_timezone(&Utc),
        LocalResult::None => {
            let bumped = naive + Duration::hours(1);
            match tz.from_local_datetime(&bumped) {
                LocalResult::Single(d) => d.with_timezone(&Utc),
                LocalResult::Ambiguous(a, _) => a.with_timezone(&Utc),
                LocalResult::None => Utc.from_utc_datetime(&naive),
            }
        }
    }
}

/// Everything the per-value rewrite needs.
struct Ctx {
    from: Tz,
    to: Tz,
    mode: Mode,
    write_as: WriteAs,
}

/// Running state collected while rewriting (drives the `VTIMEZONE` block).
#[derive(Default)]
struct Stats {
    wrote_tzid: bool,
    min_year: Option<i32>,
    max_year: Option<i32>,
}

impl Stats {
    fn saw(&mut self, year: i32) {
        self.min_year = Some(self.min_year.map_or(year, |y| y.min(year)));
        self.max_year = Some(self.max_year.map_or(year, |y| y.max(year)));
    }
}

impl Ctx {
    /// The instant a written value denotes, per the chosen mode.
    fn instant(&self, dt: &DtValue, tzid: Option<&str>) -> DateTime<Utc> {
        match self.mode {
            Mode::Relabel => local_to_utc(dt.naive, self.to),
            Mode::Convert => {
                if dt.utc {
                    Utc.from_utc_datetime(&dt.naive)
                } else {
                    let zone = tzid
                        .and_then(|z| parse_zone(z, "source").ok())
                        .unwrap_or(self.from);
                    local_to_utc(dt.naive, zone)
                }
            }
        }
    }

    /// Render an instant in the target zone. Returns the value text; the caller
    /// adds the `TZID` parameter when `write_as = tzid`.
    fn render(&self, instant: DateTime<Utc>, stats: &mut Stats) -> String {
        match self.write_as {
            WriteAs::Utc => {
                stats.saw(instant.year());
                format!("{}Z", fmt_naive(&instant.naive_utc()))
            }
            WriteAs::Tzid | WriteAs::Floating => {
                let local = instant.with_timezone(&self.to).naive_local();
                stats.saw(local.year());
                fmt_naive(&local)
            }
        }
    }

    /// Convert one value (possibly a `RDATE` period `start/end`). Returns `None`
    /// when the text is not a date-time and must be passed through verbatim.
    fn value(&self, raw: &str, tzid: Option<&str>, stats: &mut Stats) -> Option<String> {
        if let Some((a, b)) = raw.split_once('/') {
            let start = self.value_single(a, tzid, stats)?;
            // The second half is either an end date-time or an ISO 8601 duration.
            let end = match self.value_single(b, tzid, stats) {
                Some(v) => v,
                None => b.to_string(),
            };
            return Some(format!("{start}/{end}"));
        }
        self.value_single(raw, tzid, stats)
    }

    fn value_single(&self, raw: &str, tzid: Option<&str>, stats: &mut Stats) -> Option<String> {
        let dt = parse_datetime(raw)?;
        Some(self.render(self.instant(&dt, tzid), stats))
    }
}

/// Rewrite one property line inside an event-like component.
fn rewrite_prop(line: &str, ctx: &Ctx, stats: &mut Stats) -> String {
    let Some((name, params, value)) = split_prop(line) else {
        return line.to_string();
    };
    match name.as_str() {
        "DTSTART" | "DTEND" | "DUE" | "RECURRENCE-ID" | "EXDATE" | "RDATE" => {
            // All-day values carry no time and never move.
            if param_value(&params, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE")) {
                return line.to_string();
            }
            let tzid = param_value(&params, "TZID");
            let mut parts: Vec<String> = Vec::new();
            for piece in value.split(',') {
                match ctx.value(piece, tzid, stats) {
                    Some(v) => parts.push(v),
                    // Not a date-time we understand — leave the whole line alone.
                    None => return line.to_string(),
                }
            }
            let mut kept: Vec<(String, String)> = params
                .iter()
                .filter(|(k, _)| k != "TZID" && k != "VALUE")
                .cloned()
                .collect();
            if ctx.write_as == WriteAs::Tzid {
                kept.insert(0, ("TZID".to_string(), ctx.to.name().to_string()));
                stats.wrote_tzid = true;
            }
            join_prop(&name, &kept, &parts.join(","))
        }
        "RRULE" => {
            let mut parts: Vec<String> = Vec::new();
            for part in value.split(';') {
                let upper = part.to_ascii_uppercase();
                match upper.strip_prefix("UNTIL=") {
                    Some(_) => {
                        let raw = &part[6..];
                        match parse_datetime(raw) {
                            // UNTIL is UTC whenever the value carries a zone, and
                            // floating when the calendar's times are floating.
                            Some(dt) => {
                                let instant = ctx.instant(&dt, None);
                                let rendered = if ctx.write_as == WriteAs::Floating {
                                    fmt_naive(&instant.with_timezone(&ctx.to).naive_local())
                                } else {
                                    format!("{}Z", fmt_naive(&instant.naive_utc()))
                                };
                                parts.push(format!("UNTIL={rendered}"));
                            }
                            // Date-only UNTIL — leave it exactly as written.
                            None => parts.push(part.to_string()),
                        }
                    }
                    None => parts.push(part.to_string()),
                }
            }
            join_prop(&name, &params, &parts.join(";"))
        }
        _ => line.to_string(),
    }
}

// ---------------------------------------------------------------------------
// VTIMEZONE generation
// ---------------------------------------------------------------------------

fn utc_offset_secs(tz: Tz, t: NaiveDateTime) -> i32 {
    tz.offset_from_utc_datetime(&t).fix().local_minus_utc()
}

fn push_tz_component(
    out: &mut Vec<String>,
    daylight: bool,
    local: NaiveDateTime,
    from_off: i32,
    to_off: i32,
) {
    let tag = if daylight { "DAYLIGHT" } else { "STANDARD" };
    out.push(format!("BEGIN:{tag}"));
    out.push(format!("TZOFFSETFROM:{}", offset_ics(from_off)));
    out.push(format!("TZOFFSETTO:{}", offset_ics(to_off)));
    out.push(format!("DTSTART:{}", fmt_naive(&local)));
    out.push(format!("END:{tag}"));
}

/// Build a `VTIMEZONE` for `tz` whose sub-components carry the zone's real UTC
/// transitions across the years the calendar covers, plus an anchor component
/// that defines the offset in force before the first of them.
fn build_vtimezone(tz: Tz, min_year: i32, max_year: i32) -> Vec<String> {
    let start_y = min_year.saturating_sub(1).clamp(1970, 2100);
    let mut end_y = max_year.saturating_add(1).clamp(1970, 2100);
    if end_y < start_y {
        end_y = start_y;
    }
    if end_y - start_y > MAX_TZ_SPAN_YEARS {
        end_y = start_y + MAX_TZ_SPAN_YEARS;
    }
    let start = NaiveDate::from_ymd_opt(start_y, 1, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .expect("1 January is a valid date");
    let end = NaiveDate::from_ymd_opt(end_y, 12, 31)
        .and_then(|d| d.and_hms_opt(23, 0, 0))
        .expect("31 December is a valid date");

    let initial = utc_offset_secs(tz, start);
    let mut transitions: Vec<(NaiveDateTime, i32, i32)> = Vec::new();
    let mut prev_off = initial;
    let mut t = start;
    while t < end {
        let next = t + Duration::days(1);
        let off = utc_offset_secs(tz, next);
        if off != prev_off {
            // Narrow the day down to the exact transition second.
            let (mut lo, mut hi) = (t, next);
            while hi - lo > Duration::seconds(1) {
                let mid = lo + (hi - lo) / 2;
                if utc_offset_secs(tz, mid) == prev_off {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            transitions.push((hi, prev_off, off));
            prev_off = off;
        }
        t = next;
    }

    let std_off = transitions
        .iter()
        .map(|(_, _, to)| *to)
        .chain(core::iter::once(initial))
        .min()
        .unwrap_or(initial);

    let mut out = vec!["BEGIN:VTIMEZONE".to_string(), format!("TZID:{}", tz.name())];
    let anchor = NaiveDate::from_ymd_opt(1970, 1, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .expect("the epoch is a valid date");
    push_tz_component(&mut out, initial > std_off, anchor, initial, initial);
    for (instant, from_off, to_off) in transitions {
        let local = instant + Duration::seconds(to_off as i64);
        push_tz_component(&mut out, to_off > std_off, local, from_off, to_off);
    }
    out.push("END:VTIMEZONE".to_string());
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Components whose date-time properties are rewritten.
fn is_schedulable(component: &str) -> bool {
    matches!(component, "VEVENT" | "VTODO" | "VJOURNAL" | "VFREEBUSY")
}

/// Rewrite every event in `ics` into the `to` timezone.
pub fn shift(
    ics: &str,
    from: Tz,
    to: Tz,
    mode: Mode,
    write_as: WriteAs,
    include_vtimezone: bool,
) -> Result<String, String> {
    if ics.trim().is_empty() {
        return Err("no iCalendar data provided: paste the contents of an .ics file (it starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT…END:VEVENT blocks)".into());
    }
    let ctx = Ctx {
        from,
        to,
        mode,
        write_as,
    };
    let mut stats = Stats::default();

    let mut out: Vec<String> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut tz_depth: Option<usize> = None;
    let mut events = 0usize;
    let mut has_calendar = false;
    let mut insert_at: Option<usize> = None;

    for line in unfold(ics) {
        if let Some((is_begin, comp)) = delimiter(&line) {
            if is_begin {
                stack.push(comp.clone());
                if comp == "VCALENDAR" {
                    has_calendar = true;
                }
                if comp == "VEVENT" {
                    events += 1;
                    if events > MAX_EVENTS {
                        return Err(format!(
                            "too many events: this calendar has more than {MAX_EVENTS} VEVENT blocks, which is the per-run limit — split the .ics file and shift each part"
                        ));
                    }
                }
                if comp == "VTIMEZONE" && tz_depth.is_none() {
                    tz_depth = Some(stack.len());
                }
                if tz_depth.is_none() {
                    if insert_at.is_none() && stack.len() == 2 && stack[0] == "VCALENDAR" {
                        insert_at = Some(out.len());
                    }
                    out.push(line);
                }
                continue;
            }
            let closes_timezone = tz_depth == Some(stack.len());
            if tz_depth.is_none() {
                out.push(line);
            }
            stack.pop();
            if closes_timezone {
                tz_depth = None;
            }
            continue;
        }
        // Stale VTIMEZONE definitions are dropped — nothing points at them once
        // every value has been rewritten into the target zone.
        if tz_depth.is_some() {
            continue;
        }
        let current = stack.last().map(String::as_str).unwrap_or("");
        if current == "VCALENDAR" {
            // Keep the calendar-level display zone honest.
            if let Some((name, params, _)) = split_prop(&line) {
                if name == "X-WR-TIMEZONE" {
                    out.push(join_prop(&name, &params, to.name()));
                    continue;
                }
            }
            out.push(line);
        } else if is_schedulable(current) {
            out.push(rewrite_prop(&line, &ctx, &mut stats));
        } else {
            // VALARM triggers, X- components and anything else pass through.
            out.push(line);
        }
    }

    if events == 0 {
        return Err("no events found: the input has no BEGIN:VEVENT blocks to shift — paste the contents of an .ics calendar file".into());
    }

    let timezone_block = if include_vtimezone && stats.wrote_tzid {
        let min_year = stats.min_year.unwrap_or(1970);
        let max_year = stats.max_year.unwrap_or(min_year);
        build_vtimezone(to, min_year, max_year)
    } else {
        Vec::new()
    };

    let mut lines: Vec<String> = Vec::new();
    if has_calendar {
        let at = insert_at.unwrap_or(out.len()).min(out.len());
        lines.extend(out[..at].iter().cloned());
        lines.extend(timezone_block);
        lines.extend(out[at..].iter().cloned());
    } else {
        // Lenient: bare VEVENTs get a wrapper so the result imports cleanly.
        lines.push("BEGIN:VCALENDAR".to_string());
        lines.push("VERSION:2.0".to_string());
        lines.push("PRODID:-//gizza-ai//ics-timezone-shifter//EN".to_string());
        lines.push("CALSCALE:GREGORIAN".to_string());
        lines.extend(timezone_block);
        lines.extend(out);
        lines.push("END:VCALENDAR".to_string());
    }

    let folded: Vec<String> = lines.iter().flat_map(|l| fold(l)).collect();
    Ok(folded.join("\r\n"))
}

/// String-argument convenience used by the chat block, the CLI and the page.
pub fn shift_str(
    ics: &str,
    from: &str,
    to: &str,
    mode: &str,
    write_as: &str,
    include_vtimezone: bool,
) -> Result<String, String> {
    let from_tz = if from.trim().is_empty() {
        Tz::UTC
    } else {
        parse_zone(from, "source")?
    };
    shift(
        ics,
        from_tz,
        parse_zone(to, "target")?,
        Mode::parse(mode)?,
        WriteAs::parse(write_as)?,
        include_vtimezone,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cal(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{body}END:VCALENDAR\r\n")
    }

    fn ev(start: &str, end: &str) -> String {
        format!("BEGIN:VEVENT\r\nUID:e@x\r\nDTSTART:{start}\r\nDTEND:{end}\r\nSUMMARY:Standup\r\nEND:VEVENT\r\n")
    }

    fn run(ics: &str, from: &str, to: &str, mode: &str, write_as: &str) -> String {
        shift_str(ics, from, to, mode, write_as, true).unwrap()
    }

    #[test]
    fn utc_event_converts_to_local_tzid_values() {
        // 2024-03-10 14:00Z is 15:00 in Berlin (CET, before Europe's DST switch).
        let ics = cal(&ev("20240310T140000Z", "20240310T150000Z"));
        let out = run(&ics, "UTC", "Europe/Berlin", "convert", "tzid");
        assert!(
            out.contains("DTSTART;TZID=Europe/Berlin:20240310T150000"),
            "{out}"
        );
        assert!(
            out.contains("DTEND;TZID=Europe/Berlin:20240310T160000"),
            "{out}"
        );
        // a matching VTIMEZONE is emitted, before the event
        assert!(out.contains("BEGIN:VTIMEZONE\r\nTZID:Europe/Berlin"), "{out}");
        assert!(out.find("BEGIN:VTIMEZONE").unwrap() < out.find("BEGIN:VEVENT").unwrap());
        assert!(out.contains("TZOFFSETTO:+0200"), "no DST transition: {out}");
    }

    #[test]
    fn floating_times_are_read_in_the_from_zone() {
        // 09:00 floating, declared to be New York → 14:00 UTC (EDT, UTC-4).
        let ics = cal(&ev("20240710T090000", "20240710T100000"));
        let out = run(&ics, "America/New_York", "UTC", "convert", "utc");
        assert!(out.contains("DTSTART:20240710T130000Z"), "{out}");
        assert!(out.contains("DTEND:20240710T140000Z"), "{out}");
        // UTC output carries no VTIMEZONE
        assert!(!out.contains("BEGIN:VTIMEZONE"), "{out}");
    }

    #[test]
    fn existing_tzid_is_the_source_zone_in_convert_mode() {
        let ics = cal(
            "BEGIN:VEVENT\r\nUID:e@x\r\nDTSTART;TZID=America/New_York:20240710T090000\r\nDTEND;TZID=America/New_York:20240710T100000\r\nEND:VEVENT\r\n",
        );
        // 09:00 New York = 15:00 Berlin in July.
        let out = run(&ics, "UTC", "Europe/Berlin", "convert", "tzid");
        assert!(
            out.contains("DTSTART;TZID=Europe/Berlin:20240710T150000"),
            "{out}"
        );
        assert!(!out.contains("TZID=America/New_York"), "{out}");
    }

    #[test]
    fn relabel_keeps_the_wall_clock_and_swaps_the_zone() {
        let ics = cal(&ev("20240710T090000Z", "20240710T100000Z"));
        let out = run(&ics, "UTC", "America/New_York", "relabel", "tzid");
        // digits unchanged, zone replaced
        assert!(
            out.contains("DTSTART;TZID=America/New_York:20240710T090000"),
            "{out}"
        );
        // …and the same relabel expressed as UTC shifts the instant by the offset
        let utc = run(&ics, "UTC", "America/New_York", "relabel", "utc");
        assert!(utc.contains("DTSTART:20240710T130000Z"), "{utc}");
    }

    #[test]
    fn all_day_events_never_move() {
        let ics = cal(
            "BEGIN:VEVENT\r\nUID:d@x\r\nDTSTART;VALUE=DATE:20240704\r\nDTEND;VALUE=DATE:20240705\r\nSUMMARY:Holiday\r\nEND:VEVENT\r\n",
        );
        let out = run(&ics, "UTC", "Pacific/Auckland", "convert", "tzid");
        assert!(out.contains("DTSTART;VALUE=DATE:20240704"), "{out}");
        assert!(out.contains("DTEND;VALUE=DATE:20240705"), "{out}");
        // nothing was written with a TZID, so no VTIMEZONE is needed
        assert!(!out.contains("BEGIN:VTIMEZONE"), "{out}");
    }

    #[test]
    fn floating_output_drops_every_zone_marker() {
        let ics = cal(&ev("20240310T140000Z", "20240310T150000Z"));
        let out = run(&ics, "UTC", "Europe/Berlin", "convert", "floating");
        assert!(out.contains("DTSTART:20240310T150000\r\n"), "{out}");
        assert!(!out.contains("TZID="), "{out}");
        assert!(!out.contains("BEGIN:VTIMEZONE"), "{out}");
    }

    #[test]
    fn dtstamp_and_last_modified_are_left_alone() {
        let ics = cal(
            "BEGIN:VEVENT\r\nUID:e@x\r\nDTSTAMP:20240101T000000Z\r\nLAST-MODIFIED:20240102T000000Z\r\nCREATED:20240103T000000Z\r\nDTSTART:20240310T140000Z\r\nEND:VEVENT\r\n",
        );
        let out = run(&ics, "UTC", "Asia/Tokyo", "convert", "tzid");
        assert!(out.contains("DTSTAMP:20240101T000000Z"), "{out}");
        assert!(out.contains("LAST-MODIFIED:20240102T000000Z"), "{out}");
        assert!(out.contains("CREATED:20240103T000000Z"), "{out}");
        assert!(out.contains("DTSTART;TZID=Asia/Tokyo:20240310T230000"), "{out}");
    }

    #[test]
    fn recurrence_until_and_exdate_follow_the_shift() {
        let ics = cal(
            "BEGIN:VEVENT\r\nUID:r@x\r\nDTSTART:20240701T120000Z\r\nRRULE:FREQ=WEEKLY;UNTIL=20240729T120000Z;BYDAY=MO\r\nEXDATE:20240708T120000Z,20240715T120000Z\r\nEND:VEVENT\r\n",
        );
        let out = run(&ics, "UTC", "Europe/Berlin", "convert", "tzid");
        // UNTIL stays a UTC instant (unchanged: only the expression of DTSTART moved)
        assert!(out.contains("UNTIL=20240729T120000Z"), "{out}");
        assert!(out.contains("BYDAY=MO"), "{out}");
        // EXDATEs are re-expressed in the target zone, both of them
        assert!(
            out.contains("EXDATE;TZID=Europe/Berlin:20240708T140000,20240715T140000"),
            "{out}"
        );
        // floating output rewrites UNTIL as a local wall-clock value
        let floating = run(&ics, "UTC", "Europe/Berlin", "convert", "floating");
        assert!(floating.contains("UNTIL=20240729T140000"), "{floating}");
    }

    #[test]
    fn stale_vtimezone_blocks_are_replaced() {
        let ics = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-WR-TIMEZONE:America/Chicago\r\nBEGIN:VTIMEZONE\r\nTZID:America/Chicago\r\nBEGIN:STANDARD\r\nTZOFFSETFROM:-0500\r\nTZOFFSETTO:-0600\r\nDTSTART:19701101T020000\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n{}END:VCALENDAR\r\n",
            ev("20240310T140000Z", "20240310T150000Z")
        );
        let out = run(&ics, "UTC", "Europe/Berlin", "convert", "tzid");
        assert!(!out.contains("America/Chicago"), "{out}");
        assert_eq!(out.matches("BEGIN:VTIMEZONE").count(), 1, "{out}");
        assert!(out.contains("X-WR-TIMEZONE:Europe/Berlin"), "{out}");
    }

    #[test]
    fn dst_gap_time_rolls_forward_an_hour() {
        // 02:30 on 2024-03-10 does not exist in New York (clocks jump 02:00→03:00).
        let ics = cal(&ev("20240310T023000", "20240310T033000"));
        let out = run(&ics, "America/New_York", "UTC", "convert", "utc");
        // 03:30 EDT = 07:30 UTC
        assert!(out.contains("DTSTART:20240310T073000Z"), "{out}");
    }

    #[test]
    fn other_components_and_alarms_pass_through() {
        let ics = cal(
            "BEGIN:VEVENT\r\nUID:a@x\r\nDTSTART:20240310T140000Z\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\nBEGIN:VTODO\r\nUID:t@x\r\nDUE:20240310T140000Z\r\nEND:VTODO\r\n",
        );
        let out = run(&ics, "UTC", "Europe/Berlin", "convert", "tzid");
        assert!(out.contains("TRIGGER:-PT15M"), "{out}");
        assert!(out.contains("DUE;TZID=Europe/Berlin:20240310T150000"), "{out}");
    }

    #[test]
    fn bare_events_get_a_calendar_wrapper() {
        let out = run(&ev("20240310T140000Z", "20240310T150000Z"), "UTC", "UTC", "convert", "utc");
        assert!(out.starts_with("BEGIN:VCALENDAR\r\n"), "{out}");
        assert!(out.contains("PRODID:-//gizza-ai//ics-timezone-shifter//EN"), "{out}");
        assert!(out.ends_with("END:VCALENDAR"), "{out}");
    }

    #[test]
    fn long_lines_are_refolded_at_75_octets() {
        let long = "x".repeat(200);
        let ics = cal(&format!(
            "BEGIN:VEVENT\r\nUID:l@x\r\nDTSTART:20240310T140000Z\r\nSUMMARY:{long}\r\nEND:VEVENT\r\n"
        ));
        let out = run(&ics, "UTC", "UTC", "convert", "utc");
        assert!(out.contains("\r\n x"), "continuation lines missing: {out}");
        for line in out.split("\r\n") {
            assert!(line.len() <= 75, "line over 75 octets: {line}");
        }
    }

    #[test]
    fn folded_input_is_unfolded_before_rewriting() {
        let ics = cal(
            "BEGIN:VEVENT\r\nUID:f@x\r\nDTSTART;TZID=America/New_Yo\r\n rk:20240710T090000\r\nEND:VEVENT\r\n",
        );
        let out = run(&ics, "UTC", "UTC", "convert", "utc");
        assert!(out.contains("DTSTART:20240710T130000Z"), "{out}");
    }

    #[test]
    fn vtimezone_can_be_suppressed() {
        let ics = cal(&ev("20240310T140000Z", "20240310T150000Z"));
        let out = shift_str(&ics, "UTC", "Europe/Berlin", "convert", "tzid", false).unwrap();
        assert!(out.contains("DTSTART;TZID=Europe/Berlin:20240310T150000"), "{out}");
        assert!(!out.contains("BEGIN:VTIMEZONE"), "{out}");
    }

    #[test]
    fn fixed_offset_zone_gets_a_single_standard_block() {
        let ics = cal(&ev("20240310T140000Z", "20240310T150000Z"));
        let out = run(&ics, "UTC", "Asia/Tokyo", "convert", "tzid");
        assert_eq!(out.matches("BEGIN:STANDARD").count(), 1, "{out}");
        assert!(!out.contains("BEGIN:DAYLIGHT"), "{out}");
        assert!(out.contains("TZOFFSETTO:+0900"), "{out}");
    }

    #[test]
    fn zone_names_are_case_insensitive() {
        let ics = cal(&ev("20240310T140000Z", "20240310T150000Z"));
        let out = run(&ics, "utc", "europe/berlin", "convert", "tzid");
        assert!(out.contains("TZID=Europe/Berlin"), "{out}");
    }

    #[test]
    fn event_cap_is_enforced() {
        let body = ev("20240310T140000Z", "20240310T150000Z").repeat(MAX_EVENTS + 1);
        let err = shift_str(&cal(&body), "UTC", "UTC", "convert", "utc", true).unwrap_err();
        assert!(err.contains("too many events"), "{err}");
        // exactly at the cap still works
        let ok = shift_str(
            &cal(&ev("20240310T140000Z", "20240310T150000Z").repeat(MAX_EVENTS)),
            "UTC",
            "UTC",
            "convert",
            "utc",
            true,
        )
        .unwrap();
        assert_eq!(ok.matches("BEGIN:VEVENT").count(), MAX_EVENTS);
    }

    #[test]
    fn errors_are_helpful() {
        let ics = cal(&ev("20240310T140000Z", "20240310T150000Z"));
        assert!(shift_str("", "UTC", "UTC", "convert", "utc", true)
            .unwrap_err()
            .contains("no iCalendar data"));
        assert!(
            shift_str("BEGIN:VCALENDAR\r\nEND:VCALENDAR", "UTC", "UTC", "convert", "utc", true)
                .unwrap_err()
                .contains("no events")
        );
        assert!(shift_str(&ics, "UTC", "Mars/Olympus", "convert", "utc", true)
            .unwrap_err()
            .contains("unknown target timezone"));
        assert!(shift_str(&ics, "Nowhere/Special", "UTC", "convert", "utc", true)
            .unwrap_err()
            .contains("unknown source timezone"));
        assert!(shift_str(&ics, "UTC", "", "convert", "utc", true)
            .unwrap_err()
            .contains("target timezone is empty"));
        assert!(shift_str(&ics, "UTC", "UTC", "bogus", "utc", true)
            .unwrap_err()
            .contains("invalid mode"));
        assert!(shift_str(&ics, "UTC", "UTC", "convert", "bogus", true)
            .unwrap_err()
            .contains("invalid write_as"));
    }
}
