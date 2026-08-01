//! gizza-ai/ics-merge-dedupe core — merge several iCalendar (.ics) documents into
//! one VCALENDAR and drop duplicate VEVENTs. Pure-Rust, no dependencies: RFC 5545
//! line unfolding, component scanning, TEXT unescaping and a self-contained date
//! parser are all done by hand — shared by the chat block, the CLI and the page.
//!
//! Input may contain any number of concatenated .ics files (each with its own
//! BEGIN:VCALENDAR…END:VCALENDAR wrapper). Every VEVENT is preserved verbatim
//! (line endings normalized to CRLF); duplicates are collapsed per `DedupeBy`.
//! Unique VTIMEZONE blocks (by TZID) are emitted once, and any other top-level
//! components (VTODO / VJOURNAL / VFREEBUSY) are passed through unchanged.

use std::collections::HashMap;

/// How two VEVENTs are judged to be "the same event".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DedupeBy {
    /// Match on UID when the event has one, otherwise fall back to normalized
    /// start + title. The default — handles both same-source re-exports (shared
    /// UID) and cross-publisher copies (no shared UID).
    Smart,
    /// Match on UID + normalized start time (events with no UID fall back to
    /// start + title so they are still deduped rather than force-merged).
    UidStart,
    /// Match on normalized start + title only, ignoring UID — collapses the same
    /// public event exported by different apps that each assigned their own UID.
    StartTitle,
}

impl DedupeBy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "smart" => Ok(DedupeBy::Smart),
            "uid_start" | "uid-start" | "uid" => Ok(DedupeBy::UidStart),
            "start_title" | "start-title" | "title" => Ok(DedupeBy::StartTitle),
            other => Err(format!(
                "invalid dedupe_by '{other}': expected one of smart, uid_start, start_title"
            )),
        }
    }
}

/// Which member of a duplicate group to keep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Keep {
    /// Keep the first occurrence in document order. The default.
    First,
    /// Keep the occurrence with the newest LAST-MODIFIED (falling back to
    /// DTSTAMP); ties and unparseable timestamps keep the earlier occurrence.
    LastModified,
}

impl Keep {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "first" => Ok(Keep::First),
            "last_modified" | "last-modified" | "newest" | "latest" => Ok(Keep::LastModified),
            other => Err(format!(
                "invalid keep '{other}': expected one of first, last_modified"
            )),
        }
    }
}

/// One captured VEVENT: the raw physical lines (re-emitted verbatim) plus the
/// key fields used for matching, ordering and keep-resolution.
#[derive(Debug, Clone)]
struct Event {
    lines: Vec<String>,
    uid: String,
    start_norm: String,
    title_norm: String,
    /// Sort key: epoch seconds of DTSTART, or i64::MAX when unparseable.
    start_epoch: i64,
    /// Recency rank: epoch of LAST-MODIFIED (else DTSTAMP), or i64::MIN.
    mod_rank: i64,
}

/// Unfold RFC 5545 content lines: a line beginning with a space or tab is a
/// continuation of the previous line (the leading whitespace is dropped).
fn unfold(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            if let Some(last) = out.last_mut() {
                last.push_str(&line[1..]);
                continue;
            }
        }
        out.push(line.clone());
    }
    out
}

/// Split a content line into its property NAME (upper-cased) and value, skipping
/// any `;`-separated parameters. A `:` inside a quoted parameter value does not
/// terminate the parameter list.
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

/// Unescape an RFC 5545 TEXT value: `\n`/`\N` → newline, `\,` → `,`, `\;` → `;`,
/// `\\` → `\`. Any other backslash escape keeps the escaped char.
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

/// Escape a string for use as an RFC 5545 TEXT value.
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Collapse internal whitespace and lowercase, for tolerant title matching.
fn normalize_title(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

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

/// Parse `YYYYMMDD`, `YYYYMMDDTHHMMSS` (floating/TZID) or `YYYYMMDDTHHMMSSZ`
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

/// Epoch seconds (civil-from-days, Howard Hinnant's algorithm). Floating / TZID
/// values are treated as wall-clock UTC (best effort — no timezone db is shipped).
fn to_epoch(p: &DateParts) -> i64 {
    let y = if p.month <= 2 { p.year - 1 } else { p.year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = if p.month > 2 { p.month - 3 } else { p.month + 9 };
    let doy = (153 * mp + 2) / 5 + p.day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + p.hour * 3600 + p.min * 60 + p.sec
}

/// Normalize a DTSTART value for matching: parseable dates collapse to ISO so a
/// `Z` time and its basic form compare equal; otherwise trim + lowercase.
fn normalize_start(raw: &str) -> String {
    match parse_ics_date(raw) {
        Some(p) => to_iso(&p),
        None => raw.trim().to_ascii_lowercase(),
    }
}

/// True for a physical delimiter line (not a folded continuation).
fn delimiter(line: &str) -> Option<(bool, String)> {
    if line.starts_with(' ') || line.starts_with('\t') {
        return None;
    }
    let t = line.trim();
    let up = t.to_ascii_uppercase();
    if let Some(rest) = up.strip_prefix("BEGIN:") {
        Some((true, rest.trim().to_string()))
    } else {
        up.strip_prefix("END:").map(|rest| (false, rest.trim().to_string()))
    }
}

enum Component {
    Event(Event),
    Timezone { tzid: String, lines: Vec<String> },
    Other(Vec<String>),
}

/// Build an Event from its captured physical lines.
fn build_event(lines: Vec<String>) -> Event {
    let unfolded = unfold(&lines);
    let mut uid = String::new();
    let mut start_raw = String::new();
    let mut summary = String::new();
    let mut last_mod = String::new();
    let mut dtstamp = String::new();
    // Only read the event's OWN properties (depth 1); skip nested VALARM etc.
    let mut depth = 0i32;
    for l in &unfolded {
        if let Some((is_begin, _)) = delimiter(l) {
            depth += if is_begin { 1 } else { -1 };
            continue;
        }
        if depth != 1 {
            continue;
        }
        if let Some((name, value)) = split_prop(l) {
            match name.as_str() {
                "UID" => uid = value.trim().to_string(),
                "DTSTART" => start_raw = value.trim().to_string(),
                "SUMMARY" => summary = unescape_text(value),
                "LAST-MODIFIED" => last_mod = value.trim().to_string(),
                "DTSTAMP" => dtstamp = value.trim().to_string(),
                _ => {}
            }
        }
    }
    let start_epoch = parse_ics_date(&start_raw).map(|p| to_epoch(&p)).unwrap_or(i64::MAX);
    let mod_rank = parse_ics_date(&last_mod)
        .or_else(|| parse_ics_date(&dtstamp))
        .map(|p| to_epoch(&p))
        .unwrap_or(i64::MIN);
    Event {
        lines,
        uid,
        start_norm: normalize_start(&start_raw),
        title_norm: normalize_title(&summary),
        start_epoch,
        mod_rank,
    }
}

fn build_timezone(lines: Vec<String>) -> Component {
    let unfolded = unfold(&lines);
    let mut tzid = String::new();
    let mut depth = 0i32;
    for l in &unfolded {
        if let Some((is_begin, _)) = delimiter(l) {
            depth += if is_begin { 1 } else { -1 };
            continue;
        }
        if depth != 1 {
            continue;
        }
        if let Some((name, value)) = split_prop(l) {
            if name == "TZID" {
                tzid = value.trim().to_string();
            }
        }
    }
    Component::Timezone { tzid, lines }
}

/// Scan the (possibly multi-file) input into its top-level components, in order.
fn scan(ics: &str) -> Vec<Component> {
    let phys: Vec<String> = ics
        .split('\n')
        .map(|r| r.strip_suffix('\r').unwrap_or(r).to_string())
        .collect();
    let mut stack: Vec<String> = Vec::new();
    let mut buf: Option<(String, Vec<String>)> = None;
    let mut out: Vec<Component> = Vec::new();

    for line in phys {
        if let Some((is_begin, comp)) = delimiter(&line) {
            if is_begin {
                let inside_vcal = stack.len() == 1 && stack[0] == "VCALENDAR";
                let bare_top = stack.is_empty() && comp != "VCALENDAR";
                stack.push(comp.clone());
                if buf.is_none() && (inside_vcal || bare_top) {
                    buf = Some((comp, vec![line]));
                    continue;
                }
                if let Some((_, lines)) = buf.as_mut() {
                    lines.push(line);
                }
                continue;
            }
            // END:
            if let Some((_, lines)) = buf.as_mut() {
                lines.push(line);
            }
            stack.pop();
            if buf.is_some() {
                let closed = (stack.len() == 1 && stack[0] == "VCALENDAR") || stack.is_empty();
                if closed {
                    let (typ, lines) = buf.take().unwrap();
                    out.push(match typ.as_str() {
                        "VEVENT" => Component::Event(build_event(lines)),
                        "VTIMEZONE" => build_timezone(lines),
                        _ => Component::Other(lines),
                    });
                }
            }
            continue;
        }
        if let Some((_, lines)) = buf.as_mut() {
            lines.push(line);
        }
    }
    out
}

/// Compute the dedup key for an event under the chosen strategy. `None` means the
/// event carries no usable identity (empty UID + empty start + empty title) and
/// is always kept.
fn dedup_key(e: &Event, by: DedupeBy) -> Option<String> {
    let st = || format!("ST\u{0}{}\u{0}{}", e.start_norm, e.title_norm);
    let key = match by {
        DedupeBy::Smart => {
            if e.uid.is_empty() {
                st()
            } else {
                format!("U\u{0}{}", e.uid)
            }
        }
        DedupeBy::UidStart => {
            if e.uid.is_empty() {
                st()
            } else {
                format!("US\u{0}{}\u{0}{}", e.uid, e.start_norm)
            }
        }
        DedupeBy::StartTitle => st(),
    };
    if e.uid.is_empty() && e.start_norm.is_empty() && e.title_norm.is_empty() {
        return None;
    }
    Some(key)
}

/// Merge one or more iCalendar documents into a single VCALENDAR, dropping
/// duplicate VEVENTs.
pub fn merge(
    ics: &str,
    dedupe_by: DedupeBy,
    keep: Keep,
    sort: bool,
    calendar_name: &str,
) -> Result<String, String> {
    if ics.trim().is_empty() {
        return Err("no iCalendar data provided: paste the contents of one or more .ics files (each starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT blocks — concatenate several files to merge them)".into());
    }
    let components = scan(ics);

    let mut order: Vec<String> = Vec::new(); // dedup keys in first-seen order
    let mut kept: HashMap<String, Event> = HashMap::new();
    let mut unkeyed: Vec<Event> = Vec::new(); // events with no identity — always kept
    let mut tz_order: Vec<String> = Vec::new();
    let mut timezones: HashMap<String, Vec<String>> = HashMap::new();
    let mut passthrough: Vec<Vec<String>> = Vec::new();
    let mut total_events = 0usize;

    for c in components {
        match c {
            Component::Event(e) => {
                total_events += 1;
                match dedup_key(&e, dedupe_by) {
                    None => unkeyed.push(e),
                    Some(key) => match kept.get_mut(&key) {
                        None => {
                            order.push(key.clone());
                            kept.insert(key, e);
                        }
                        Some(existing) => {
                            if keep == Keep::LastModified && e.mod_rank > existing.mod_rank {
                                *existing = e; // replace, keep the group's position
                            }
                        }
                    },
                }
            }
            Component::Timezone { tzid, lines } => {
                let k = if tzid.is_empty() {
                    format!("\u{0}anon{}", tz_order.len())
                } else {
                    tzid
                };
                timezones.entry(k.clone()).or_insert_with(|| {
                    tz_order.push(k.clone());
                    lines
                });
            }
            Component::Other(lines) => passthrough.push(lines),
        }
    }

    if total_events == 0 {
        return Err("no events found: the input has no BEGIN:VEVENT blocks to merge (only VEVENTs are deduped; VTODO / VJOURNAL / VFREEBUSY are passed through, but at least one event is required)".into());
    }

    // Assemble the deduped event list in stable first-seen order, then optionally
    // sort chronologically by start.
    let mut events: Vec<Event> = order.into_iter().map(|k| kept.remove(&k).unwrap()).collect();
    events.extend(unkeyed);
    if sort {
        // Stable sort keeps original order among equal starts and unparseable dates.
        events.sort_by_key(|e| e.start_epoch);
    }

    let mut out: Vec<String> = Vec::new();
    out.push("BEGIN:VCALENDAR".into());
    out.push("VERSION:2.0".into());
    out.push("PRODID:-//gizza-ai//ics-merge-dedupe//EN".into());
    out.push("CALSCALE:GREGORIAN".into());
    if !calendar_name.trim().is_empty() {
        out.push(format!("X-WR-CALNAME:{}", escape_text(calendar_name.trim())));
    }
    for k in &tz_order {
        out.extend(timezones.get(k).unwrap().iter().cloned());
    }
    for e in &events {
        out.extend(e.lines.iter().cloned());
    }
    for block in &passthrough {
        out.extend(block.iter().cloned());
    }
    out.push("END:VCALENDAR".into());

    Ok(out.join("\r\n"))
}

/// String-argument convenience used by the chat block, CLI and web wrapper.
pub fn merge_str(
    ics: &str,
    dedupe_by: &str,
    keep: &str,
    sort: bool,
    calendar_name: &str,
) -> Result<String, String> {
    merge(
        ics,
        DedupeBy::parse(dedupe_by)?,
        Keep::parse(keep)?,
        sort,
        calendar_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cal(events: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{events}END:VCALENDAR\r\n")
    }

    fn ev(uid: &str, start: &str, summary: &str) -> String {
        format!("BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTART:{start}\r\nSUMMARY:{summary}\r\nEND:VEVENT\r\n")
    }

    fn m(ics: &str, by: &str, keep: &str, sort: bool, name: &str) -> String {
        merge_str(ics, by, keep, sort, name).unwrap()
    }

    fn count_events(out: &str) -> usize {
        out.lines().filter(|l| l.trim().eq_ignore_ascii_case("BEGIN:VEVENT")).count()
    }

    #[test]
    fn merges_two_files_and_drops_uid_duplicate() {
        let a = cal(&ev("evt-1@ex.com", "20240309T081530Z", "Standup"));
        let b = cal(&ev("evt-1@ex.com", "20240309T081530Z", "Standup"));
        let out = m(&format!("{a}{b}"), "smart", "first", true, "");
        // one VCALENDAR wrapper, one surviving event
        assert_eq!(out.matches("BEGIN:VCALENDAR").count(), 1);
        assert_eq!(out.matches("END:VCALENDAR").count(), 1);
        assert_eq!(count_events(&out), 1);
        assert!(out.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(out.contains("PRODID:-//gizza-ai//ics-merge-dedupe//EN"));
        // CRLF line endings throughout
        assert!(out.ends_with("END:VCALENDAR"));
    }

    #[test]
    fn distinct_uids_both_kept_and_sorted() {
        let ics = cal(&format!(
            "{}{}",
            ev("b@x", "20240310T090000Z", "Later"),
            ev("a@x", "20240309T090000Z", "Earlier"),
        ));
        let out = m(&ics, "smart", "first", true, "");
        assert_eq!(count_events(&out), 2);
        // sorted by start → Earlier first
        let earlier = out.find("Earlier").unwrap();
        let later = out.find("Later").unwrap();
        assert!(earlier < later, "events not sorted chronologically");
    }

    #[test]
    fn sort_false_keeps_document_order() {
        let ics = cal(&format!(
            "{}{}",
            ev("b@x", "20240310T090000Z", "Later"),
            ev("a@x", "20240309T090000Z", "Earlier"),
        ));
        let out = m(&ics, "smart", "first", false, "");
        assert!(out.find("Later").unwrap() < out.find("Earlier").unwrap());
    }

    #[test]
    fn start_title_collapses_cross_publisher_copies() {
        // same event, different UIDs assigned by two apps
        let ics = cal(&format!(
            "{}{}",
            ev("app-a-123", "20240704", "Independence Day"),
            ev("app-b-999", "20240704", "Independence  Day"),
        ));
        // smart keeps both (different UIDs)
        assert_eq!(count_events(&m(&ics, "smart", "first", true, "")), 2);
        // start_title collapses them (whitespace-normalized title)
        assert_eq!(count_events(&m(&ics, "start_title", "first", true, "")), 1);
    }

    #[test]
    fn keep_last_modified_wins() {
        let older = "BEGIN:VEVENT\r\nUID:e@x\r\nDTSTART:20240101T100000Z\r\nSUMMARY:Old Title\r\nLAST-MODIFIED:20240101T120000Z\r\nEND:VEVENT\r\n";
        let newer = "BEGIN:VEVENT\r\nUID:e@x\r\nDTSTART:20240101T100000Z\r\nSUMMARY:New Title\r\nLAST-MODIFIED:20240201T120000Z\r\nEND:VEVENT\r\n";
        let ics = cal(&format!("{older}{newer}"));
        let out = m(&ics, "smart", "last_modified", true, "");
        assert_eq!(count_events(&out), 1);
        assert!(out.contains("New Title"));
        assert!(!out.contains("Old Title"));
        // keep=first would retain the older one instead
        let first = m(&ics, "smart", "first", true, "");
        assert!(first.contains("Old Title") && !first.contains("New Title"));
    }

    #[test]
    fn vtimezone_deduped_by_tzid_and_events_preserved() {
        let tz = "BEGIN:VTIMEZONE\r\nTZID:Europe/London\r\nBEGIN:STANDARD\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0000\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n";
        let a = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{tz}{}END:VCALENDAR\r\n", ev("a@x", "20240101T100000", "One"));
        let b = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{tz}{}END:VCALENDAR\r\n", ev("b@x", "20240102T100000", "Two"));
        let out = m(&format!("{a}{b}"), "smart", "first", true, "");
        // the identical timezone appears once, both events survive
        assert_eq!(out.matches("BEGIN:VTIMEZONE").count(), 1);
        assert_eq!(count_events(&out), 2);
        // nested STANDARD not mistaken for a top-level component
        assert!(out.contains("TZID:Europe/London"));
    }

    #[test]
    fn valarm_dtstamp_does_not_leak_into_matching() {
        // Two events, same UID/start, each with a VALARM carrying its own props —
        // the alarm's SUMMARY/DTSTAMP must not affect dedup identity.
        let one = "BEGIN:VEVENT\r\nUID:x@x\r\nDTSTART:20240101T100000Z\r\nSUMMARY:Real\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:ring\r\nEND:VALARM\r\nEND:VEVENT\r\n";
        let ics = cal(&format!("{one}{one}"));
        let out = m(&ics, "smart", "first", true, "");
        assert_eq!(count_events(&out), 1);
        // the surviving event still carries its alarm verbatim
        assert_eq!(out.matches("BEGIN:VALARM").count(), 1);
    }

    #[test]
    fn passthrough_todo_preserved() {
        let todo = "BEGIN:VTODO\r\nUID:t@x\r\nSUMMARY:Buy milk\r\nEND:VTODO\r\n";
        let ics = cal(&format!("{}{todo}", ev("e@x", "20240101", "Event")));
        let out = m(&ics, "smart", "first", true, "");
        assert_eq!(count_events(&out), 1);
        assert!(out.contains("BEGIN:VTODO") && out.contains("Buy milk"));
    }

    #[test]
    fn calendar_name_sets_x_wr_calname() {
        let ics = cal(&ev("e@x", "20240101", "Event"));
        let out = m(&ics, "smart", "first", true, "My Merged, Calendar");
        // comma is TEXT-escaped
        assert!(out.contains("X-WR-CALNAME:My Merged\\, Calendar"));
    }

    #[test]
    fn folded_lines_preserved_verbatim() {
        let folded = "BEGIN:VEVENT\r\nUID:f@x\r\nDTSTART:20240101\r\nSUMMARY:Quarterly planning meeting for\r\n  the whole team\r\nEND:VEVENT\r\n";
        let ics = cal(folded);
        let out = m(&ics, "smart", "first", true, "");
        assert_eq!(count_events(&out), 1);
        // the physical fold (continuation line beginning with a space) is kept
        assert!(out.contains("SUMMARY:Quarterly planning meeting for\r\n  the whole team"));
    }

    #[test]
    fn uid_start_keeps_same_uid_different_start() {
        // A recurrence override: same UID, different start → uid_start keeps both.
        let ics = cal(&format!(
            "{}{}",
            ev("series@x", "20240101T100000Z", "Weekly"),
            ev("series@x", "20240108T100000Z", "Weekly"),
        ));
        assert_eq!(count_events(&m(&ics, "uid_start", "first", true, "")), 2);
        // smart matches on UID alone → collapses to one
        assert_eq!(count_events(&m(&ics, "smart", "first", true, "")), 1);
    }

    #[test]
    fn errors_are_helpful() {
        assert!(merge_str("", "smart", "first", true, "")
            .unwrap_err()
            .contains("no iCalendar data"));
        assert!(
            merge_str("BEGIN:VCALENDAR\r\nEND:VCALENDAR", "smart", "first", true, "")
                .unwrap_err()
                .contains("no events")
        );
        assert!(DedupeBy::parse("bogus").is_err());
        assert!(Keep::parse("bogus").is_err());
    }

    #[test]
    fn no_wrapper_bare_events_still_merge() {
        // Lenient: events pasted without a VCALENDAR wrapper still parse.
        let ics = format!("{}{}", ev("a@x", "20240101", "A"), ev("a@x", "20240101", "A"));
        let out = m(&ics, "smart", "first", true, "");
        assert_eq!(count_events(&out), 1);
        assert!(out.starts_with("BEGIN:VCALENDAR"));
    }
}
