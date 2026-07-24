//! gizza-ai/ics-to-csv core — flatten the VEVENTs of an iCalendar (.ics) document
//! into a CSV table, one row per event (SUMMARY, DTSTART, DTEND, LOCATION,
//! DESCRIPTION, plus STATUS / CATEGORIES / UID when present). Pure-Rust, no
//! dependencies: RFC 5545 line unfolding, property parsing, TEXT unescaping and a
//! self-contained date parser are all done by hand — shared by the chat block,
//! the CLI, and the page.

/// Output field separator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Delimiter {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "comma" | "," => Ok(Delimiter::Comma),
            "semicolon" | ";" => Ok(Delimiter::Semicolon),
            "tab" | "\t" => Ok(Delimiter::Tab),
            "pipe" | "|" => Ok(Delimiter::Pipe),
            other => Err(format!(
                "invalid delimiter '{other}': expected one of comma, semicolon, tab, pipe"
            )),
        }
    }
    fn ch(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Semicolon => ';',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
        }
    }
}

/// How the DTSTART / DTEND date columns are written.
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

/// One parsed VEVENT. TEXT fields are RFC 5545-unescaped; the date fields keep
/// their raw value text and are formatted per `DateFormat` at output time.
#[derive(Debug, Clone, Default)]
struct Event {
    summary: String,
    start_raw: String,
    end_raw: String,
    location: String,
    description: String,
    status: String,
    categories: String,
    uid: String,
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

/// Split a content line into its property NAME (upper-cased) and its value,
/// skipping any `;`-separated parameters. A `:` inside a quoted parameter value
/// (e.g. `ALTREP="mailto:x"`) does not terminate the parameter list.
fn split_prop(line: &str) -> Option<(String, &str)> {
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
    if bytes.get(i) == Some(&b';') {
        let mut in_quote = false;
        while j < bytes.len() {
            match bytes[j] {
                b'"' => in_quote = !in_quote,
                b':' if !in_quote => break,
                _ => {}
            }
            j += 1;
        }
    }
    if bytes.get(j) != Some(&b':') {
        return None;
    }
    Some((name, &line[j + 1..]))
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

/// Render one date cell. Unparseable values pass through as their raw text so a
/// single odd timestamp never fails the whole conversion.
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

/// Parse an iCalendar document into its VEVENTs, in document order. Nested
/// components (VALARM, VTIMEZONE, …) inside a VEVENT are skipped so their
/// properties never leak into the event row.
fn parse(ics: &str) -> Result<Vec<Event>, String> {
    if ics.trim().is_empty() {
        return Err("no iCalendar data provided: paste the contents of an .ics file (it starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT blocks)".into());
    }
    let lines = unfold(ics);
    let mut events: Vec<Event> = Vec::new();
    let mut cur: Option<Event> = None;
    // Depth of nested components below the current VEVENT (e.g. VALARM).
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
        let (name, value) = match split_prop(line) {
            Some(nv) => nv,
            None => continue,
        };
        match name.as_str() {
            "SUMMARY" => ev.summary = unescape_text(value),
            "LOCATION" => ev.location = unescape_text(value),
            "DESCRIPTION" => ev.description = unescape_text(value),
            "STATUS" => ev.status = value.trim().to_string(),
            "UID" => ev.uid = value.trim().to_string(),
            "DTSTART" => ev.start_raw = value.trim().to_string(),
            "DTEND" => ev.end_raw = value.trim().to_string(),
            "CATEGORIES" => {
                let cats = unescape_text(value);
                let cats = cats.trim();
                if cats.is_empty() {
                    // nothing to add
                } else if ev.categories.is_empty() {
                    ev.categories = cats.to_string();
                } else {
                    ev.categories.push_str(", ");
                    ev.categories.push_str(cats);
                }
            }
            _ => {}
        }
    }

    Ok(events)
}

/// RFC-4180 escape: quote a field containing the delimiter, a double-quote, CR,
/// or LF, doubling any interior quote.
fn escape(field: &str, delim: char) -> String {
    let needs = field.contains(delim)
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r');
    if needs {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Convert an iCalendar document into CSV, one row per VEVENT.
pub fn convert(
    ics: &str,
    delimiter: Delimiter,
    header: bool,
    date_format: DateFormat,
    include_location: bool,
    include_description: bool,
) -> Result<String, String> {
    let events = parse(ics)?;
    if events.is_empty() {
        return Err("no events found: this iCalendar data has no BEGIN:VEVENT blocks (only VEVENTs are exported; VTODO / VJOURNAL / VFREEBUSY are ignored)".into());
    }

    // STATUS / CATEGORIES / UID columns appear only when at least one event
    // carries them, so the CSV never has a column of blanks.
    let has_status = events.iter().any(|e| !e.status.is_empty());
    let has_categories = events.iter().any(|e| !e.categories.is_empty());
    let has_uid = events.iter().any(|e| !e.uid.is_empty());

    let delim = delimiter.ch();
    let mut cols: Vec<&str> = vec!["summary", "start", "end"];
    if include_location {
        cols.push("location");
    }
    if include_description {
        cols.push("description");
    }
    if has_status {
        cols.push("status");
    }
    if has_categories {
        cols.push("categories");
    }
    if has_uid {
        cols.push("uid");
    }

    let mut out = String::new();
    if header {
        let line: Vec<String> = cols.iter().map(|c| escape(c, delim)).collect();
        out.push_str(&line.join(&delim.to_string()));
        out.push('\n');
    }

    for e in &events {
        let mut fields: Vec<String> = Vec::with_capacity(cols.len());
        fields.push(e.summary.clone());
        fields.push(format_date(&e.start_raw, date_format));
        fields.push(format_date(&e.end_raw, date_format));
        if include_location {
            fields.push(e.location.clone());
        }
        if include_description {
            fields.push(e.description.clone());
        }
        if has_status {
            fields.push(e.status.clone());
        }
        if has_categories {
            fields.push(e.categories.clone());
        }
        if has_uid {
            fields.push(e.uid.clone());
        }
        let line: Vec<String> = fields.iter().map(|f| escape(f, delim)).collect();
        out.push_str(&line.join(&delim.to_string()));
        out.push('\n');
    }

    Ok(out.trim_end_matches('\n').to_string())
}

/// String-argument convenience used by the chat block, CLI, and web wrapper.
pub fn convert_str(
    ics: &str,
    delimiter: &str,
    header: bool,
    date_format: &str,
    include_location: bool,
    include_description: bool,
) -> Result<String, String> {
    convert(
        ics,
        Delimiter::parse(delimiter)?,
        header,
        DateFormat::parse(date_format)?,
        include_location,
        include_description,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:evt-1@example.com\r\nSUMMARY:Team Standup\r\nDTSTART:20240309T081530Z\r\nDTEND:20240309T083000Z\r\nLOCATION:Room 4\r\nDESCRIPTION:Daily sync\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    fn conv(ics: &str, d: &str, h: bool, df: &str, loc: bool, desc: bool) -> String {
        convert_str(ics, d, h, df, loc, desc).unwrap()
    }

    #[test]
    fn basic_event_iso_header() {
        let out = conv(SAMPLE, "comma", true, "iso", true, true);
        assert_eq!(
            out,
            "summary,start,end,location,description,uid\n\
             Team Standup,2024-03-09T08:15:30Z,2024-03-09T08:30:00Z,Room 4,Daily sync,evt-1@example.com"
        );
    }

    #[test]
    fn header_false_drops_header_row() {
        let out = conv(SAMPLE, "comma", false, "iso", true, true);
        assert!(!out.starts_with("summary"));
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn multiple_events_one_row_each() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:A\nDTSTART:20240101\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:B\nDTSTART:20240102\nEND:VEVENT\nEND:VCALENDAR";
        let out = conv(ics, "comma", true, "iso", false, false);
        assert_eq!(out, "summary,start,end\nA,2024-01-01,\nB,2024-01-02,");
    }

    #[test]
    fn all_day_date_only() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Holiday\nDTSTART;VALUE=DATE:20240704\nDTEND;VALUE=DATE:20240705\nEND:VEVENT";
        let out = conv(ics, "comma", false, "iso", false, false);
        assert_eq!(out, "Holiday,2024-07-04,2024-07-05");
    }

    #[test]
    fn date_format_raw_keeps_original() {
        let out = conv(SAMPLE, "comma", false, "raw", false, false);
        assert_eq!(
            out,
            "Team Standup,20240309T081530Z,20240309T083000Z,evt-1@example.com"
        );
    }

    #[test]
    fn date_format_unix_epoch() {
        let out = conv(SAMPLE, "comma", false, "unix", false, false);
        // 2024-03-09T08:15:30Z == 1709971200 + 930 == 1709972130
        assert_eq!(out, "Team Standup,1709972130,1709973000,evt-1@example.com");
    }

    #[test]
    fn delimiters_semicolon_tab_pipe() {
        assert!(
            conv(SAMPLE, "semicolon", true, "raw", false, false).starts_with("summary;start;end")
        );
        assert!(conv(SAMPLE, "tab", true, "raw", false, false).starts_with("summary\tstart\tend"));
        assert!(conv(SAMPLE, "pipe", true, "raw", false, false).starts_with("summary|start|end"));
    }

    #[test]
    fn include_toggles() {
        // location off, description on
        let out = conv(SAMPLE, "comma", true, "iso", false, true);
        assert_eq!(
            out.lines().next().unwrap(),
            "summary,start,end,description,uid"
        );
        assert!(!out.contains("Room 4"));
        // both off
        let out2 = conv(SAMPLE, "comma", true, "iso", false, false);
        assert_eq!(out2.lines().next().unwrap(), "summary,start,end,uid");
    }

    #[test]
    fn status_categories_uid_only_when_present() {
        let ics = "BEGIN:VEVENT\nSUMMARY:A\nDTSTART:20240101\nEND:VEVENT";
        let out = conv(ics, "comma", true, "iso", false, false);
        // no status/categories/uid anywhere → no such columns
        assert_eq!(out, "summary,start,end\nA,2024-01-01,");
        let ics2 = "BEGIN:VEVENT\nSUMMARY:A\nDTSTART:20240101\nSTATUS:CONFIRMED\nCATEGORIES:Work,Personal\nEND:VEVENT";
        let out2 = conv(ics2, "comma", true, "iso", false, false);
        assert_eq!(
            out2.lines().next().unwrap(),
            "summary,start,end,status,categories"
        );
        assert!(out2.contains("CONFIRMED"));
        // categories cell keeps its comma → RFC-4180 quoted
        assert!(out2.contains("\"Work,Personal\""));
    }

    #[test]
    fn line_unfolding() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Quarterly planning meeting for\n  the whole team\nDTSTART:20240101\nEND:VEVENT";
        let out = conv(ics, "comma", false, "iso", false, false);
        assert_eq!(
            out,
            "Quarterly planning meeting for the whole team,2024-01-01,"
        );
    }

    #[test]
    fn text_unescaping_and_multiline_quoting() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Lunch\nDTSTART:20240101\nDESCRIPTION:Line one\\nLine two\\, with comma\nEND:VEVENT";
        let out = conv(ics, "comma", false, "iso", false, true);
        // \n → real newline, \, → comma; the multi-line cell is RFC-4180 quoted
        assert_eq!(out, "Lunch,2024-01-01,,\"Line one\nLine two, with comma\"");
    }

    #[test]
    fn valarm_nested_component_is_skipped() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Real Summary\nDTSTART:20240101\nBEGIN:VALARM\nACTION:DISPLAY\nDESCRIPTION:Reminder popup\nEND:VALARM\nEND:VEVENT";
        let out = conv(ics, "comma", true, "iso", false, true);
        // the VALARM's DESCRIPTION must NOT overwrite the event's (empty) one
        assert_eq!(
            out,
            "summary,start,end,description\nReal Summary,2024-01-01,,"
        );
    }

    #[test]
    fn quoted_param_colon_not_treated_as_value_sep() {
        let ics = "BEGIN:VEVENT\nSUMMARY;ALTREP=\"mailto:desc@example.com\":Design review\nDTSTART:20240101\nEND:VEVENT";
        let out = conv(ics, "comma", false, "iso", false, false);
        assert_eq!(out, "Design review,2024-01-01,");
    }

    #[test]
    fn errors_are_helpful() {
        assert!(convert_str("", "comma", true, "iso", true, true)
            .unwrap_err()
            .contains("no iCalendar data"));
        assert!(convert_str(
            "BEGIN:VCALENDAR\nEND:VCALENDAR",
            "comma",
            true,
            "iso",
            true,
            true
        )
        .unwrap_err()
        .contains("no events"));
        assert!(Delimiter::parse("colon").is_err());
        assert!(DateFormat::parse("rfc").is_err());
    }

    #[test]
    fn unparseable_date_passes_through() {
        let ics = "BEGIN:VEVENT\nSUMMARY:Odd\nDTSTART:not-a-date\nEND:VEVENT";
        let iso = conv(ics, "comma", false, "iso", false, false);
        assert_eq!(iso, "Odd,not-a-date,");
        let unix = conv(ics, "comma", false, "unix", false, false);
        assert_eq!(unix, "Odd,not-a-date,");
    }
}
