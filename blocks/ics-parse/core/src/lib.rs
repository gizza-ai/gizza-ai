//! gizza-ai/ics-parse core — parse an iCalendar (.ics) document into a structured
//! JSON list of events. One JSON object per VEVENT, with title, start/end times,
//! all-day flag, location, description, status, categories, organizer, attendees,
//! and a parsed recurrence (RRULE) object. Pure-Rust: RFC 5545 line unfolding,
//! property + parameter parsing, TEXT unescaping and a self-contained date parser
//! are all done by hand — shared by the chat block, the CLI, and the page. Only
//! serde/serde_json are used, to build the JSON output. Nothing is uploaded.

use serde::Serialize;
use serde_json::{Map, Value};

/// How the start / end / until date values are written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DateFormat {
    /// Normalize to ISO-8601 (`20240309T081530Z` → `2024-03-09T08:15:30Z`,
    /// `20240309` → `2024-03-09`). The default.
    Iso,
    /// Keep the original RFC 5545 value text verbatim.
    Raw,
    /// Epoch seconds. `Z` (UTC) times are exact; floating / TZID times are read
    /// as wall-clock UTC (best effort — no timezone database is shipped).
    Unix,
}

impl DateFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "iso" => Ok(DateFormat::Iso),
            "raw" | "original" => Ok(DateFormat::Raw),
            "unix" | "epoch" => Ok(DateFormat::Unix),
            other => Err(format!(
                "invalid date_format '{other}': expected one of iso, raw, unix"
            )),
        }
    }
}

/// A named person reference (ORGANIZER / ATTENDEE): the CN parameter becomes the
/// name and the `mailto:` value becomes the email. Empty halves are omitted.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct Person {
    #[serde(skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    email: String,
}

impl Person {
    fn is_empty(&self) -> bool {
        self.name.is_empty() && self.email.is_empty()
    }
}

/// One parsed VEVENT, ready to serialize. Empty / absent fields are skipped so
/// the JSON stays clean (no columns of blanks, no `null`s).
#[derive(Debug, Clone, Default, Serialize)]
struct EventOut {
    #[serde(skip_serializing_if = "String::is_empty")]
    uid: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    summary: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    start: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    end: Value,
    #[serde(skip_serializing_if = "is_false")]
    all_day: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    location: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    categories: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    organizer: Option<Person>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attendees: Vec<Person>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurrence: Option<Value>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The raw, unformatted state we accumulate while scanning one VEVENT.
#[derive(Default)]
struct Event {
    uid: String,
    summary: String,
    start_raw: String,
    end_raw: String,
    all_day: bool,
    location: String,
    description: String,
    status: String,
    categories: Vec<String>,
    organizer: Option<Person>,
    attendees: Vec<Person>,
    rrule_raw: String,
}

/// Unfold RFC 5545 content lines: a line beginning with a space or a tab is a
/// continuation of the previous line (the leading whitespace is dropped). Also
/// strips a trailing CR so both CRLF and bare-LF inputs behave identically.
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
        out.push(line.to_string());
    }
    out
}

/// Split a content line into its property NAME (upper-cased), its parameters
/// (upper-cased keys, unquoted values), and its value. A `:`/`;` inside a quoted
/// parameter value (e.g. `ALTREP="mailto:x"`) does not terminate the list.
fn parse_line(line: &str) -> Option<(String, Vec<(String, String)>, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] != b';' && bytes[i] != b':' {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let name = line[..i].to_ascii_uppercase();
    let mut j = i;
    let mut in_quote = false;
    while j < bytes.len() {
        match bytes[j] {
            b'"' => in_quote = !in_quote,
            b':' if !in_quote => break,
            _ => {}
        }
        j += 1;
    }
    if bytes.get(j) != Some(&b':') {
        return None;
    }
    let params = if bytes.get(i) == Some(&b';') {
        parse_params(&line[i + 1..j])
    } else {
        Vec::new()
    };
    Some((name, params, &line[j + 1..]))
}

/// Parse the `KEY=VALUE;KEY="quoted;value"` parameter section (no leading `;`).
fn parse_params(s: &str) -> Vec<(String, String)> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let mut in_quote = false;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => in_quote = !in_quote,
                b';' if !in_quote => break,
                _ => {}
            }
            i += 1;
        }
        let seg = &s[start..i];
        if i < bytes.len() {
            i += 1; // skip the ';'
        }
        if let Some((k, v)) = seg.split_once('=') {
            let key = k.trim().to_ascii_uppercase();
            let mut val = v.trim();
            if val.len() >= 2 && val.starts_with('"') && val.ends_with('"') {
                val = &val[1..val.len() - 1];
            }
            if !key.is_empty() {
                out.push((key, val.to_string()));
            }
        }
    }
    out
}

/// Unescape an RFC 5545 TEXT value: `\n`/`\N` → newline, `\,` → `,`,
/// `\;` → `;`, `\\` → `\`. Any other backslash escape keeps the escaped char.
fn unescape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(',') => out.push(','),
                Some(';') => out.push(';'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Split a comma-list TEXT value (CATEGORIES) on UNescaped commas, then unescape
/// each item. Blank items are dropped.
fn split_list(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            cur.push('\\');
            if let Some(n) = chars.next() {
                cur.push(n);
            }
        } else if c == ',' {
            let item = unescape_text(cur.trim());
            if !item.is_empty() {
                out.push(item);
            }
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    let item = unescape_text(cur.trim());
    if !item.is_empty() {
        out.push(item);
    }
    out
}

/// Turn an ORGANIZER / ATTENDEE line into a Person: CN param → name,
/// `mailto:addr` value → email (the `mailto:` prefix is case-insensitive).
fn parse_person(params: &[(String, String)], value: &str) -> Person {
    let v = value.trim();
    let email = if v.len() >= 7 && v[..7].eq_ignore_ascii_case("mailto:") {
        v[7..].trim()
    } else {
        v
    };
    let name = params
        .iter()
        .find(|(k, _)| k == "CN")
        .map(|(_, val)| unescape_text(val.trim()))
        .unwrap_or_default();
    Person {
        name: name.trim().to_string(),
        email: email.to_string(),
    }
}

/// The broken-down parts of an RFC 5545 DATE or DATE-TIME value.
struct DateParts {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    min: i64,
    sec: i64,
    has_time: bool,
    utc: bool,
}

/// Parse `YYYYMMDD`, `YYYYMMDDTHHMMSS` (floating/TZID), or `YYYYMMDDTHHMMSSZ`
/// (UTC). Returns None for anything that isn't one of those basic forms.
fn parse_ics_date(raw: &str) -> Option<DateParts> {
    let s = raw.trim();
    let bytes = s.as_bytes();
    if bytes.len() < 8 {
        return None;
    }
    let num = |a: usize, b: usize| -> Option<i64> { s.get(a..b)?.parse::<i64>().ok() };
    let year = num(0, 4)?;
    let month = num(4, 6)?;
    let day = num(6, 8)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if bytes.len() >= 15 && bytes[8] == b'T' {
        let hour = num(9, 11)?;
        let min = num(11, 13)?;
        let sec = num(13, 15)?;
        if hour > 23 || min > 59 || sec > 60 {
            return None;
        }
        Some(DateParts {
            year,
            month,
            day,
            hour,
            min,
            sec,
            has_time: true,
            utc: bytes.get(15) == Some(&b'Z'),
        })
    } else {
        Some(DateParts {
            year,
            month,
            day,
            hour: 0,
            min: 0,
            sec: 0,
            has_time: false,
            utc: false,
        })
    }
}

fn to_iso(p: &DateParts) -> String {
    if p.has_time {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
            p.year,
            p.month,
            p.day,
            p.hour,
            p.min,
            p.sec,
            if p.utc { "Z" } else { "" }
        )
    } else {
        format!("{:04}-{:02}-{:02}", p.year, p.month, p.day)
    }
}

/// Epoch seconds for a parsed value (civil-from-days, Howard Hinnant's
/// algorithm). Floating / TZID values are treated as wall-clock UTC.
fn to_epoch(p: &DateParts) -> i64 {
    let y = if p.month <= 2 { p.year - 1 } else { p.year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = if p.month > 2 {
        p.month - 3
    } else {
        p.month + 9
    };
    let doy = (153 * mp + 2) / 5 + p.day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + p.hour * 3600 + p.min * 60 + p.sec
}

/// Render one date value. Unparseable values pass through as their raw text so a
/// single odd timestamp never fails the whole parse.
fn format_date(raw: &str, f: DateFormat) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    match f {
        DateFormat::Raw => raw.trim().to_string(),
        DateFormat::Iso => parse_ics_date(raw)
            .map(|p| to_iso(&p))
            .unwrap_or_else(|| raw.trim().to_string()),
        DateFormat::Unix => parse_ics_date(raw)
            .map(|p| to_epoch(&p).to_string())
            .unwrap_or_else(|| raw.trim().to_string()),
    }
}

/// Render one date value as JSON. ISO/raw formats are strings; unix format is a
/// JSON number when the timestamp is parseable, falling back to the raw string for
/// odd vendor-specific values so a single unusual timestamp never fails parsing.
fn format_date_value(raw: &str, f: DateFormat) -> Value {
    if raw.trim().is_empty() {
        return Value::Null;
    }
    match f {
        DateFormat::Raw | DateFormat::Iso => Value::from(format_date(raw, f)),
        DateFormat::Unix => parse_ics_date(raw)
            .map(|p| Value::from(to_epoch(&p)))
            .unwrap_or_else(|| Value::from(raw.trim())),
    }
}

/// A bare `1,15` → JSON numbers where every part parses, else the raw string.
fn number_or_string(s: &str) -> Value {
    match s.trim().parse::<i64>() {
        Ok(n) => Value::from(n),
        Err(_) => Value::from(s.trim()),
    }
}

/// Parse an RRULE value (`FREQ=WEEKLY;INTERVAL=2;COUNT=10;BYDAY=MO,WE`) into a
/// structured object, preserving the property order. FREQ/WKST stay strings,
/// INTERVAL/COUNT become numbers, UNTIL is date-formatted, and any BY* /other
/// part becomes a number, string, or array of those (split on commas).
fn parse_recurrence(value: &str, df: DateFormat) -> Value {
    let mut map = Map::new();
    for part in value.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (raw_key, v) = match part.split_once('=') {
            Some((k, v)) => (k.trim().to_ascii_uppercase(), v.trim()),
            None => continue,
        };
        if raw_key.is_empty() {
            continue;
        }
        let val = match raw_key.as_str() {
            "INTERVAL" | "COUNT" => v
                .parse::<i64>()
                .map(Value::from)
                .unwrap_or_else(|_| Value::from(v)),
            "UNTIL" => format_date_value(v, df),
            "FREQ" | "WKST" => Value::from(v),
            _ => {
                if v.contains(',') {
                    Value::Array(v.split(',').map(number_or_string).collect())
                } else {
                    number_or_string(v)
                }
            }
        };
        map.insert(raw_key.to_ascii_lowercase(), val);
    }
    Value::Object(map)
}

/// Parse an iCalendar document into its VEVENTs, in document order. Nested
/// components (VALARM, VTIMEZONE, …) inside a VEVENT are skipped so their
/// properties never leak into the event.
fn parse(ics: &str) -> Result<Vec<Event>, String> {
    if ics.trim().is_empty() {
        return Err("no iCalendar data provided: paste the contents of an .ics file (it starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT…END:VEVENT blocks)".into());
    }
    let lines = unfold(ics);
    let mut events: Vec<Event> = Vec::new();
    let mut cur: Option<Event> = None;
    let mut nested: u32 = 0;

    for line in &lines {
        let l = line.trim();
        if cur.is_none() {
            if l.eq_ignore_ascii_case("BEGIN:VEVENT") {
                cur = Some(Event::default());
                nested = 0;
            }
            continue;
        }
        if l.get(..6)
            .map_or(false, |p| p.eq_ignore_ascii_case("BEGIN:"))
        {
            nested += 1;
            continue;
        }
        if l.get(..4).map_or(false, |p| p.eq_ignore_ascii_case("END:")) {
            if nested > 0 {
                nested -= 1;
            } else if let Some(e) = cur.take() {
                events.push(e);
            }
            continue;
        }
        if nested > 0 {
            continue;
        }
        let ev = match cur.as_mut() {
            Some(e) => e,
            None => continue,
        };
        let (name, params, value) = match parse_line(line) {
            Some(nv) => nv,
            None => continue,
        };
        match name.as_str() {
            "UID" => ev.uid = value.trim().to_string(),
            "SUMMARY" => ev.summary = unescape_text(value),
            "LOCATION" => ev.location = unescape_text(value),
            "DESCRIPTION" => ev.description = unescape_text(value),
            "STATUS" => ev.status = value.trim().to_ascii_uppercase(),
            "DTSTART" => {
                ev.start_raw = value.trim().to_string();
                let is_date_param = params
                    .iter()
                    .any(|(k, v)| k == "VALUE" && v.eq_ignore_ascii_case("DATE"));
                let is_date_time = parse_ics_date(value).map_or(false, |p| p.has_time);
                ev.all_day = is_date_param || !is_date_time;
            }
            "DTEND" => ev.end_raw = value.trim().to_string(),
            "CATEGORIES" => ev.categories.extend(split_list(value)),
            "ORGANIZER" => {
                let p = parse_person(&params, value);
                if !p.is_empty() {
                    ev.organizer = Some(p);
                }
            }
            "ATTENDEE" => {
                let p = parse_person(&params, value);
                if !p.is_empty() {
                    ev.attendees.push(p);
                }
            }
            "RRULE" => ev.rrule_raw = value.trim().to_string(),
            _ => {}
        }
    }

    Ok(events)
}

/// Parse an iCalendar document into a JSON array of event objects.
pub fn parse_ics(
    ics: &str,
    date_format: DateFormat,
    pretty: bool,
    include_description: bool,
) -> Result<String, String> {
    let events = parse(ics)?;
    if events.is_empty() {
        return Err("no events found: this iCalendar data has no BEGIN:VEVENT blocks (only VEVENTs are parsed; task / journal / free-busy components are ignored)".into());
    }

    let out: Vec<EventOut> = events
        .into_iter()
        .map(|e| EventOut {
            uid: e.uid,
            summary: e.summary,
            start: format_date_value(&e.start_raw, date_format),
            end: format_date_value(&e.end_raw, date_format),
            all_day: e.all_day && !e.start_raw.trim().is_empty(),
            location: e.location,
            description: if include_description {
                e.description
            } else {
                String::new()
            },
            status: e.status,
            categories: e.categories,
            organizer: e.organizer,
            attendees: e.attendees,
            recurrence: if e.rrule_raw.is_empty() {
                None
            } else {
                Some(parse_recurrence(&e.rrule_raw, date_format))
            },
        })
        .collect();

    let json = if pretty {
        serde_json::to_string_pretty(&out)
    } else {
        serde_json::to_string(&out)
    };
    json.map_err(|e| format!("failed to serialize events to JSON: {e}"))
}

/// String-argument convenience used by the chat block, CLI, and web wrapper.
pub fn parse_ics_str(
    ics: &str,
    date_format: &str,
    pretty: bool,
    include_description: bool,
) -> Result<String, String> {
    parse_ics(
        ics,
        DateFormat::parse(date_format)?,
        pretty,
        include_description,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:evt-1@example.com\r\nSUMMARY:Team Standup\r\nDTSTART:20240309T081530Z\r\nDTEND:20240309T083000Z\r\nLOCATION:Room 4\r\nDESCRIPTION:Daily sync\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    fn json(ics: &str, df: &str, pretty: bool, desc: bool) -> Value {
        serde_json::from_str(&parse_ics_str(ics, df, pretty, desc).unwrap()).unwrap()
    }

    #[test]
    fn basic_event_iso() {
        let v = json(SAMPLE, "iso", false, true);
        let e = &v[0];
        assert_eq!(e["uid"], "evt-1@example.com");
        assert_eq!(e["summary"], "Team Standup");
        assert_eq!(e["start"], "2024-03-09T08:15:30Z");
        assert_eq!(e["end"], "2024-03-09T08:30:00Z");
        assert_eq!(e["location"], "Room 4");
        assert_eq!(e["description"], "Daily sync");
        // a timed event is NOT all-day → the key is omitted
        assert!(e.get("all_day").is_none());
    }

    #[test]
    fn all_day_date_value() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Independence Day\nDTSTART;VALUE=DATE:20240704\nDTEND;VALUE=DATE:20240705\nCATEGORIES:Holiday,US\nEND:VEVENT";
        let v = json(ics, "iso", false, true);
        let e = &v[0];
        assert_eq!(e["start"], "2024-07-04");
        assert_eq!(e["all_day"], true);
        assert_eq!(e["categories"], serde_json::json!(["Holiday", "US"]));
    }

    #[test]
    fn recurrence_parsed_structured() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Weekly sync\nDTSTART:20240101T090000Z\nRRULE:FREQ=WEEKLY;INTERVAL=2;COUNT=10;BYDAY=MO,WE,FR;UNTIL=20241231T000000Z\nEND:VEVENT";
        let v = json(ics, "iso", false, true);
        let r = &v[0]["recurrence"];
        assert_eq!(r["freq"], "WEEKLY");
        assert_eq!(r["interval"], 2);
        assert_eq!(r["count"], 10);
        assert_eq!(r["byday"], serde_json::json!(["MO", "WE", "FR"]));
        assert_eq!(r["until"], "2024-12-31T00:00:00Z");
    }

    #[test]
    fn organizer_and_attendees() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Review\nDTSTART:20240101T090000Z\nORGANIZER;CN=Jane Doe:mailto:jane@example.com\nATTENDEE;CN=Bob:mailto:bob@example.com\nATTENDEE:MAILTO:carol@example.com\nEND:VEVENT";
        let v = json(ics, "iso", false, true);
        let e = &v[0];
        assert_eq!(e["organizer"]["name"], "Jane Doe");
        assert_eq!(e["organizer"]["email"], "jane@example.com");
        assert_eq!(e["attendees"][0]["name"], "Bob");
        assert_eq!(e["attendees"][1]["email"], "carol@example.com");
        // carol has no CN → the name key is omitted
        assert!(e["attendees"][1].get("name").is_none());
    }

    #[test]
    fn multiple_events_document_order() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:A\nDTSTART:20240101\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:B\nDTSTART:20240102\nEND:VEVENT\nEND:VCALENDAR";
        let v = json(ics, "iso", false, false);
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["summary"], "A");
        assert_eq!(v[1]["summary"], "B");
    }

    #[test]
    fn date_format_raw_and_unix() {
        let raw = json(SAMPLE, "raw", false, false);
        assert_eq!(raw[0]["start"], "20240309T081530Z");
        let unix = json(SAMPLE, "unix", false, false);
        // 2024-03-09T08:15:30Z == 1709972130
        assert_eq!(unix[0]["start"], 1709972130);
    }

    #[test]
    fn include_description_toggle_drops_it() {
        let v = json(SAMPLE, "iso", false, false);
        assert!(v[0].get("description").is_none());
    }

    #[test]
    fn line_unfolding_and_text_unescape() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Quarterly planning for\n  the whole team\nDTSTART:20240101\nDESCRIPTION:Line one\\nLine two\\, done\nEND:VEVENT";
        let v = json(ics, "iso", false, true);
        assert_eq!(v[0]["summary"], "Quarterly planning for the whole team");
        assert_eq!(v[0]["description"], "Line one\nLine two, done");
    }

    #[test]
    fn valarm_nested_component_skipped() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Real\nDTSTART:20240101\nBEGIN:VALARM\nACTION:DISPLAY\nDESCRIPTION:Reminder popup\nEND:VALARM\nEND:VEVENT";
        let v = json(ics, "iso", false, true);
        // the VALARM's DESCRIPTION must NOT become the event's
        assert_eq!(v[0]["summary"], "Real");
        assert!(v[0].get("description").is_none());
    }

    #[test]
    fn quoted_param_colon_not_a_value_sep() {
        let ics = "BEGIN:VEVENT\nSUMMARY;ALTREP=\"mailto:desc@example.com\":Design review\nDTSTART:20240101\nEND:VEVENT";
        let v = json(ics, "iso", false, false);
        assert_eq!(v[0]["summary"], "Design review");
    }

    #[test]
    fn pretty_prints_indented() {
        let s = parse_ics_str(SAMPLE, "iso", true, true).unwrap();
        assert!(s.contains("\n  {"));
        assert!(s.starts_with("[\n"));
    }

    #[test]
    fn errors_are_helpful() {
        assert!(parse_ics_str("", "iso", true, true)
            .unwrap_err()
            .contains("no iCalendar data"));
        assert!(
            parse_ics_str("BEGIN:VCALENDAR\nEND:VCALENDAR", "iso", true, true)
                .unwrap_err()
                .contains("no events")
        );
        assert!(DateFormat::parse("rfc").is_err());
    }

    #[test]
    fn unparseable_date_passes_through() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Odd\nDTSTART:not-a-date\nEND:VEVENT";
        let v = json(ics, "iso", false, false);
        assert_eq!(v[0]["start"], "not-a-date");
        // an unparseable DTSTART has no time component → treated as all-day
        assert_eq!(v[0]["all_day"], true);
    }
}
