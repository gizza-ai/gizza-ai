//! csv-to-ics core — pure compute, shared by the chat skill block and the web
//! page. Turns a CSV event list with a header row into one iCalendar (`.ics`)
//! document: a `VCALENDAR` wrapper and one `VEVENT` per row, with RFC 5545 CRLF
//! line endings, 75-octet line folding and escaped TEXT values, ready to import
//! into Google Calendar, Outlook, Apple Calendar or any other iCalendar client.
//!
//! Column names are matched case- and punctuation-insensitively against a small
//! alias vocabulary (`title`/`summary`/`name`/`event`, `start`/`date`/`begins`,
//! …), so most spreadsheet exports need no renaming.
//!
//! No clock and no I/O: `DTSTAMP` is the fixed epoch value and UIDs are derived
//! from the row itself, so the same CSV always produces byte-identical output.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use std::collections::HashMap;

/// Hard cap on rows converted in one call, so a huge paste can't blow up memory.
/// The chat/CLI schema and the page copy advertise this same bound.
pub const MAX_EVENTS: usize = 5000;

/// Product identifier written into `PRODID`.
const PRODID: &str = "-//gizza-ai//csv-to-ics//EN";

/// `DTSTAMP` is fixed rather than read from a clock: the value is only required
/// to exist, and pinning it keeps the output deterministic (re-running the same
/// CSV produces the same bytes, so diffs and re-imports stay clean).
const DTSTAMP: &str = "19700101T000000Z";

/// Longest duration accepted in a per-row `duration_minutes` cell: one year.
const MAX_ROW_DURATION_MINUTES: i64 = 525_600;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How wall-clock times are anchored in the output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Zone {
    /// No zone marker: floating local time (`DTSTART:20260724T090000`), which
    /// every client reads as "whatever the local clock says".
    Floating,
    /// The pasted times are already UTC, so they get the `Z` suffix.
    Utc,
}

fn parse_zone(s: &str) -> Result<Zone, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "floating" | "local" => Ok(Zone::Floating),
        "utc" | "z" | "gmt" => Ok(Zone::Utc),
        other => Err(format!(
            "unknown timezone '{other}' — use \"floating\" for local wall-clock times or \"utc\" for times that are already UTC"
        )),
    }
}

// ---------------------------------------------------------------------------
// Column detection
// ---------------------------------------------------------------------------

/// A CSV column's meaning in the calendar.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Field {
    Title,
    Start,
    End,
    Duration,
    Description,
    Location,
    Uid,
    AllDay,
}

/// Header aliases. A header matches when it normalizes to one of these, so
/// `Start Date`, `start_date` and `START-DATE` are all the same column.
const ALIASES: &[(Field, &[&str])] = &[
    (Field::Title, &["title", "summary", "name", "event"]),
    (Field::Start, &["start", "start_date", "begins", "date"]),
    (Field::End, &["end", "end_date", "ends"]),
    (Field::Duration, &["duration_minutes", "duration"]),
    (Field::Description, &["description", "details", "notes"]),
    (Field::Location, &["location", "place"]),
    (Field::Uid, &["uid", "id"]),
    (Field::AllDay, &["all_day", "all-day"]),
];

/// Lowercase and drop every non-alphanumeric character, so `All Day`,
/// `all_day` and `all-day` all compare equal.
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Map each field to the index of the first header that names it. Earlier
/// aliases win, and a header is only ever claimed by one field.
fn detect_columns(headers: &[String]) -> HashMap<Field, usize> {
    let norm: Vec<String> = headers.iter().map(|h| normalize(h)).collect();
    let mut map: HashMap<Field, usize> = HashMap::new();
    let mut taken = vec![false; headers.len()];
    for (field, aliases) in ALIASES {
        'aliases: for alias in *aliases {
            let a = normalize(alias);
            for (i, h) in norm.iter().enumerate() {
                if !taken[i] && *h == a {
                    map.insert(*field, i);
                    taken[i] = true;
                    break 'aliases;
                }
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Date / time parsing
// ---------------------------------------------------------------------------

/// Parse a date cell into a date and, when the cell carries one, a time.
///
/// Accepted: `YYYY-MM-DD`, `YYYY-MM-DD HH:MM`, `YYYY-MM-DDTHH:MM`, each with
/// optional `:SS` and an optional trailing `Z` (which the `timezone` option,
/// not the cell, decides how to render).
fn parse_cell(cell: &str) -> Result<(NaiveDate, Option<NaiveTime>), String> {
    let text = cell.trim();
    if text.is_empty() {
        return Err("the cell is empty".to_string());
    }
    let bad = || {
        format!(
            "could not read '{text}' as a date — use 2026-07-24, 2026-07-24 09:00 or 2026-07-24T09:00:00"
        )
    };

    // Split the date part from the time part on an ISO `T` or on whitespace.
    let body = text.strip_suffix(['Z', 'z']).unwrap_or(text).trim_end();
    let (date_part, time_part) = match body.find(['T', 't']) {
        Some(i)
            if body[..i].chars().next_back().is_some_and(|c| c.is_ascii_digit())
                && body[i + 1..].chars().next().is_some_and(|c| c.is_ascii_digit()) =>
        {
            (&body[..i], body[i + 1..].trim())
        }
        _ => match body.split_once(char::is_whitespace) {
            Some((d, t)) => (d, t.trim()),
            None => (body, ""),
        },
    };

    let nums: Vec<&str> = date_part.split(['-', '/']).collect();
    if nums.len() != 3 || !nums.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        return Err(bad());
    }
    if nums[0].len() != 4 || nums[1].is_empty() || nums[2].is_empty() {
        return Err(bad());
    }
    let (y, m, d) = (
        nums[0].parse::<i32>().map_err(|_| bad())?,
        nums[1].parse::<u32>().map_err(|_| bad())?,
        nums[2].parse::<u32>().map_err(|_| bad())?,
    );
    let date = NaiveDate::from_ymd_opt(y, m, d)
        .ok_or_else(|| format!("'{text}' is not a real calendar date"))?;

    if time_part.is_empty() {
        return Ok((date, None));
    }
    let hms: Vec<&str> = time_part.split(':').collect();
    if !(2..=3).contains(&hms.len()) || !hms.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        return Err(format!(
            "could not read '{time_part}' in '{text}' as a time — use HH:MM or HH:MM:SS on a 24-hour clock"
        ));
    }
    let parse = |p: &str| p.parse::<u32>().map_err(|_| bad());
    let (h, mi) = (parse(hms[0])?, parse(hms[1])?);
    let s = if hms.len() == 3 { parse(hms[2])? } else { 0 };
    let time = NaiveTime::from_hms_opt(h, mi, s).ok_or_else(|| {
        format!("'{time_part}' in '{text}' is not a valid clock time (hour 0–23, minute and second 0–59)")
    })?;
    Ok((date, Some(time)))
}

/// Truthiness for the all-day column.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "y" | "1" | "x" | "t"
    )
}

// ---------------------------------------------------------------------------
// iCalendar emission helpers
// ---------------------------------------------------------------------------

/// Escape an iCalendar TEXT value per RFC 5545 §3.3.11.
fn esc(s: &str) -> String {
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

/// Append a content line, folded to 75 octets with a leading space on each
/// continuation (RFC 5545 §3.1) and never split inside a UTF-8 code point.
fn fold(line: &str, out: &mut String) {
    if line.len() <= 75 {
        out.push_str(line);
        out.push_str("\r\n");
        return;
    }
    let mut start = 0usize;
    let mut limit = 75usize;
    while start < line.len() {
        let mut end = (start + limit).min(line.len());
        while end > start && !line.is_char_boundary(end) {
            end -= 1;
        }
        out.push_str(&line[start..end]);
        out.push_str("\r\n");
        start = end;
        if start < line.len() {
            out.push(' ');
            limit = 74; // the continuation space counts toward the 75 octets
        }
    }
}

fn fmt_date(d: NaiveDate) -> String {
    format!("{:04}{:02}{:02}", d.year(), d.month(), d.day())
}

fn fmt_datetime(dt: NaiveDateTime, zone: Zone) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}{}",
        dt.year(),
        dt.month(),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        if zone == Zone::Utc { "Z" } else { "" }
    )
}

/// Lowercase, hyphen-joined slug of a title, for the generated UID.
fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 60 {
            break;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "event".to_string()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// Convert a CSV event list into an iCalendar (`.ics`) document.
///
/// * `csv_text` — CSV whose first row names the columns.
/// * `timezone` — `floating` (local wall-clock) or `utc` (times already in UTC).
/// * `default_duration_minutes` — 1–1440, used when a row has no end or duration.
/// * `include_alarm` — add a 15-minute display reminder to every event.
pub fn run(
    csv_text: &str,
    timezone: &str,
    default_duration_minutes: i64,
    include_alarm: bool,
) -> Result<String, String> {
    let zone = parse_zone(timezone)?;
    if !(1..=1440).contains(&default_duration_minutes) {
        return Err(format!(
            "default_duration_minutes {default_duration_minutes} is out of range — use a whole number of minutes from 1 to 1440"
        ));
    }

    let trimmed = csv_text.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err("no CSV data — paste an event list whose first row names the columns, e.g. 'title,start,end'".to_string());
    }

    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(trimmed.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("could not read the header row: {e}"))?
        .iter()
        .map(|h| h.to_string())
        .collect();
    if headers.iter().all(|h| h.trim().is_empty()) {
        return Err("the first row must name the columns, e.g. 'title,start,end'".to_string());
    }

    let cols = detect_columns(&headers);
    let named = headers.join(", ");
    let title_idx = *cols.get(&Field::Title).ok_or_else(|| {
        format!("no title column found — name one column title, summary, name or event (the header row is: {named})")
    })?;
    let start_idx = *cols.get(&Field::Start).ok_or_else(|| {
        format!("no start column found — name one column start, start_date, begins or date (the header row is: {named})")
    })?;

    let mut body = String::new();
    let mut count = 0usize;

    for (n, record) in rdr.records().enumerate() {
        let record = record.map_err(|e| format!("row {}: {e}", n + 2))?;
        let row_no = n + 2; // 1-based, and the header is row 1
        let cell = |f: Field| -> String {
            cols.get(&f)
                .and_then(|i| record.get(*i))
                .unwrap_or("")
                .trim()
                .to_string()
        };
        if record.iter().all(|v| v.trim().is_empty()) {
            continue; // blank spacer row
        }

        let title = record.get(title_idx).unwrap_or("").trim().to_string();
        let start_cell = record.get(start_idx).unwrap_or("").trim().to_string();
        if title.is_empty() {
            return Err(format!(
                "row {row_no}: no event title in column '{}'",
                headers[title_idx]
            ));
        }
        if start_cell.is_empty() {
            return Err(format!(
                "row {row_no}: '{title}' has no start date in column '{}'",
                headers[start_idx]
            ));
        }

        count += 1;
        if count > MAX_EVENTS {
            return Err(format!(
                "too many events: this converts at most {MAX_EVENTS} rows in one run — split the file and convert it in parts"
            ));
        }

        let (start_date, start_time) =
            parse_cell(&start_cell).map_err(|e| format!("row {row_no}: start — {e}"))?;
        let end_cell = cell(Field::End);
        let end = if end_cell.is_empty() {
            None
        } else {
            Some(parse_cell(&end_cell).map_err(|e| format!("row {row_no}: end — {e}"))?)
        };

        let duration_cell = cell(Field::Duration);
        let duration = if duration_cell.is_empty() {
            None
        } else {
            let minutes: i64 = duration_cell.parse().map_err(|_| {
                format!("row {row_no}: duration '{duration_cell}' is not a whole number of minutes")
            })?;
            if !(1..=MAX_ROW_DURATION_MINUTES).contains(&minutes) {
                return Err(format!(
                    "row {row_no}: duration {minutes} is out of range — use 1 to {MAX_ROW_DURATION_MINUTES} minutes"
                ));
            }
            Some(minutes)
        };

        // A row is all-day when its all-day column says so, or when the start
        // cell carries a date with no time.
        let all_day = truthy(&cell(Field::AllDay)) || start_time.is_none();

        let mut event = String::new();
        event.push_str("BEGIN:VEVENT\r\n");
        let uid_cell = cell(Field::Uid);
        let uid = if uid_cell.is_empty() {
            format!("{}@{}.local", slug(&title), count)
        } else {
            uid_cell
        };
        fold(&format!("UID:{}", esc(&uid)), &mut event);
        fold(&format!("DTSTAMP:{DTSTAMP}"), &mut event);

        if all_day {
            let last = end.map(|(d, _)| d).unwrap_or(start_date);
            if last < start_date {
                return Err(format!(
                    "row {row_no}: the end date {last} is before the start date {start_date}"
                ));
            }
            // An all-day DTEND is exclusive, so an event running through the
            // end date ends on the day after it.
            fold(
                &format!("DTSTART;VALUE=DATE:{}", fmt_date(start_date)),
                &mut event,
            );
            fold(
                &format!("DTEND;VALUE=DATE:{}", fmt_date(last + Duration::days(1))),
                &mut event,
            );
        } else {
            let start_dt = start_date.and_time(start_time.unwrap());
            let end_dt = match (end, duration) {
                // An end cell with no time of its own keeps the start's time.
                (Some((d, t)), _) => d.and_time(t.unwrap_or(start_time.unwrap())),
                (None, Some(minutes)) => start_dt + Duration::minutes(minutes),
                (None, None) => start_dt + Duration::minutes(default_duration_minutes),
            };
            if end_dt < start_dt {
                return Err(format!(
                    "row {row_no}: the event ends ({end_dt}) before it starts ({start_dt})"
                ));
            }
            fold(
                &format!("DTSTART:{}", fmt_datetime(start_dt, zone)),
                &mut event,
            );
            fold(&format!("DTEND:{}", fmt_datetime(end_dt, zone)), &mut event);
        }

        fold(&format!("SUMMARY:{}", esc(&title)), &mut event);
        let description = cell(Field::Description);
        if !description.is_empty() {
            fold(
                &format!("DESCRIPTION:{}", esc(&description)),
                &mut event,
            );
        }
        let location = cell(Field::Location);
        if !location.is_empty() {
            fold(&format!("LOCATION:{}", esc(&location)), &mut event);
        }
        if include_alarm {
            event.push_str("BEGIN:VALARM\r\n");
            event.push_str("ACTION:DISPLAY\r\n");
            event.push_str("TRIGGER:-PT15M\r\n");
            fold(&format!("DESCRIPTION:{}", esc(&title)), &mut event);
            event.push_str("END:VALARM\r\n");
        }
        event.push_str("END:VEVENT\r\n");
        body.push_str(&event);
    }

    if count == 0 {
        return Err(
            "no event rows found — the CSV has a header row but no data rows underneath it"
                .to_string(),
        );
    }

    let mut out = String::with_capacity(body.len() + 128);
    out.push_str("BEGIN:VCALENDAR\r\n");
    out.push_str("VERSION:2.0\r\n");
    fold(&format!("PRODID:{PRODID}"), &mut out);
    out.push_str("CALSCALE:GREGORIAN\r\n");
    out.push_str(&body);
    out.push_str("END:VCALENDAR\r\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(data: &str) -> Result<String, String> {
        run(data, "floating", 60, false)
    }

    #[test]
    fn happy_path_timed_event() {
        let ics =
            conv("title,start,end,location\nTeam sync,2026-07-24 09:00,2026-07-24 09:30,Room 2")
                .unwrap();
        assert_eq!(
            ics,
            "BEGIN:VCALENDAR\r\n\
             VERSION:2.0\r\n\
             PRODID:-//gizza-ai//csv-to-ics//EN\r\n\
             CALSCALE:GREGORIAN\r\n\
             BEGIN:VEVENT\r\n\
             UID:team-sync@1.local\r\n\
             DTSTAMP:19700101T000000Z\r\n\
             DTSTART:20260724T090000\r\n\
             DTEND:20260724T093000\r\n\
             SUMMARY:Team sync\r\n\
             LOCATION:Room 2\r\n\
             END:VEVENT\r\n\
             END:VCALENDAR\r\n"
        );
    }

    #[test]
    fn utc_timezone_adds_the_z_suffix() {
        let ics = run("title,start\nCall,2026-07-24T14:00", "utc", 60, false).unwrap();
        assert!(ics.contains("DTSTART:20260724T140000Z\r\n"), "{ics}");
        assert!(ics.contains("DTEND:20260724T150000Z\r\n"), "{ics}");
    }

    #[test]
    fn all_day_row_emits_value_date_with_exclusive_end() {
        let ics = conv("title,start,end\nConference,2026-07-24,2026-07-26").unwrap();
        assert!(ics.contains("DTSTART;VALUE=DATE:20260724\r\n"), "{ics}");
        // Inclusive 24th–26th → exclusive DTEND on the 27th.
        assert!(ics.contains("DTEND;VALUE=DATE:20260727\r\n"), "{ics}");
        assert!(!ics.contains("DTSTART:2026"), "{ics}");
    }

    #[test]
    fn single_day_all_day_event_ends_the_next_day() {
        let ics = conv("title,start\nHoliday,2026-07-24").unwrap();
        assert!(ics.contains("DTSTART;VALUE=DATE:20260724\r\n"), "{ics}");
        assert!(ics.contains("DTEND;VALUE=DATE:20260725\r\n"), "{ics}");
    }

    #[test]
    fn all_day_column_overrides_a_start_time() {
        let ics = conv("title,start,all_day\nOffsite,2026-07-24 09:00,yes").unwrap();
        assert!(ics.contains("DTSTART;VALUE=DATE:20260724\r\n"), "{ics}");
        let timed = conv("title,start,all-day\nOffsite,2026-07-24 09:00,no").unwrap();
        assert!(timed.contains("DTSTART:20260724T090000\r\n"), "{timed}");
    }

    #[test]
    fn duration_column_wins_over_the_default() {
        let ics = conv("title,start,duration_minutes\nStandup,2026-07-24 09:00,15").unwrap();
        assert!(ics.contains("DTEND:20260724T091500\r\n"), "{ics}");
        let alias = conv("title,start,duration\nStandup,2026-07-24 09:00,45").unwrap();
        assert!(alias.contains("DTEND:20260724T094500\r\n"), "{alias}");
    }

    #[test]
    fn default_duration_fills_a_missing_end() {
        let ics = run("title,start\nStandup,2026-07-24 09:00", "floating", 25, false).unwrap();
        assert!(ics.contains("DTEND:20260724T092500\r\n"), "{ics}");
    }

    #[test]
    fn an_end_cell_beats_the_duration_column() {
        let ics =
            conv("title,start,end,duration_minutes\nReview,2026-07-24 09:00,2026-07-24 11:00,15")
                .unwrap();
        assert!(ics.contains("DTEND:20260724T110000\r\n"), "{ics}");
    }

    #[test]
    fn a_date_only_end_keeps_the_start_time() {
        let ics = conv("title,start,end\nTrip,2026-07-24 09:00,2026-07-26").unwrap();
        assert!(ics.contains("DTSTART:20260724T090000\r\n"), "{ics}");
        assert!(ics.contains("DTEND:20260726T090000\r\n"), "{ics}");
    }

    #[test]
    fn column_aliases_are_matched_case_and_punctuation_insensitively() {
        let ics = conv(
            "Event,Start Date,End Date,Notes,Place,ID\n\
             Kickoff,2026-07-24 10:00,2026-07-24 11:00,Bring slides,HQ,kickoff-2026",
        )
        .unwrap();
        assert!(ics.contains("SUMMARY:Kickoff\r\n"), "{ics}");
        assert!(ics.contains("DTSTART:20260724T100000\r\n"), "{ics}");
        assert!(ics.contains("DTEND:20260724T110000\r\n"), "{ics}");
        assert!(ics.contains("DESCRIPTION:Bring slides\r\n"), "{ics}");
        assert!(ics.contains("LOCATION:HQ\r\n"), "{ics}");
        assert!(ics.contains("UID:kickoff-2026\r\n"), "{ics}");
    }

    #[test]
    fn accepted_date_and_time_shapes() {
        let d = NaiveDate::from_ymd_opt(2026, 7, 24).unwrap();
        assert_eq!(parse_cell("2026-07-24").unwrap(), (d, None));
        assert_eq!(
            parse_cell("2026-07-24 09:00").unwrap(),
            (d, NaiveTime::from_hms_opt(9, 0, 0))
        );
        assert_eq!(
            parse_cell("2026-07-24T09:00").unwrap(),
            (d, NaiveTime::from_hms_opt(9, 0, 0))
        );
        assert_eq!(
            parse_cell("2026-07-24T09:30:45").unwrap(),
            (d, NaiveTime::from_hms_opt(9, 30, 45))
        );
        assert_eq!(
            parse_cell("2026-07-24T09:30:45Z").unwrap(),
            (d, NaiveTime::from_hms_opt(9, 30, 45))
        );
        assert_eq!(parse_cell("2026/07/24").unwrap(), (d, None));
    }

    #[test]
    fn generated_uids_are_deterministic_and_unique() {
        let csv = "title,start\nTeam sync,2026-07-24\nTeam sync,2026-07-25";
        let first = conv(csv).unwrap();
        assert_eq!(first, conv(csv).unwrap(), "same CSV, same bytes");
        let uids: Vec<&str> = first.lines().filter(|l| l.starts_with("UID:")).collect();
        assert_eq!(uids, vec!["UID:team-sync@1.local", "UID:team-sync@2.local"]);
    }

    #[test]
    fn quoted_cells_and_text_escaping() {
        let ics = conv(
            "title,start,description\n\"Lunch, with team\",2026-07-24,\"Agenda; part one\\two\nand a second line\"",
        )
        .unwrap();
        assert!(ics.contains("SUMMARY:Lunch\\, with team\r\n"), "{ics}");
        assert!(
            ics.contains("DESCRIPTION:Agenda\\; part one\\\\two\\nand a second line\r\n"),
            "{ics}"
        );
    }

    #[test]
    fn long_lines_are_folded_to_75_octets() {
        let ics = conv(&format!("title,start\n{},2026-07-24", "a".repeat(200))).unwrap();
        for line in ics.split("\r\n") {
            assert!(line.len() <= 75, "line over 75 octets: {line}");
        }
        assert!(ics.contains("\r\n a"), "continuations start with a space");
    }

    #[test]
    fn alarm_is_opt_in() {
        let ics = run("title,start\nCall,2026-07-24 09:00", "floating", 60, true).unwrap();
        assert!(
            ics.contains(
                "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nDESCRIPTION:Call\r\nEND:VALARM\r\n"
            ),
            "{ics}"
        );
        assert!(!conv("title,start\nCall,2026-07-24 09:00")
            .unwrap()
            .contains("VALARM"));
    }

    #[test]
    fn blank_rows_are_skipped() {
        let ics = conv("title,start\nA,2026-07-24\n\nB,2026-07-25\n").unwrap();
        assert_eq!(ics.matches("BEGIN:VEVENT").count(), 2, "{ics}");
    }

    #[test]
    fn err_empty_input_and_headers_only() {
        assert!(conv("   ").unwrap_err().contains("no CSV data"));
        assert!(conv("title,start")
            .unwrap_err()
            .contains("no event rows found"));
    }

    #[test]
    fn err_missing_title_or_start_column() {
        let e = conv("when,where\n2026-07-24,Room 2").unwrap_err();
        assert!(e.contains("no title column found"), "{e}");
        let e = conv("title,where\nStandup,Room 2").unwrap_err();
        assert!(e.contains("no start column found"), "{e}");
    }

    #[test]
    fn err_missing_title_or_start_value_names_the_row() {
        let e = conv("title,start\nA,2026-07-24\n,2026-07-25").unwrap_err();
        assert!(e.starts_with("row 3: no event title"), "{e}");
        let e = conv("title,start\nStandup,").unwrap_err();
        assert!(e.starts_with("row 2: 'Standup' has no start date"), "{e}");
    }

    #[test]
    fn err_invalid_dates() {
        let e = conv("title,start\nA,not-a-date").unwrap_err();
        assert!(e.starts_with("row 2: start — could not read"), "{e}");
        let e = conv("title,start\nA,2026-02-30").unwrap_err();
        assert!(e.contains("not a real calendar date"), "{e}");
        let e = conv("title,start\nA,2026-07-24 25:00").unwrap_err();
        assert!(e.contains("not a valid clock time"), "{e}");
        let e = conv("title,start,end\nA,2026-07-24,24/07/2026").unwrap_err();
        assert!(e.starts_with("row 2: end — could not read"), "{e}");
    }

    #[test]
    fn err_invalid_durations() {
        let e = conv("title,start,duration_minutes\nA,2026-07-24 09:00,half an hour").unwrap_err();
        assert!(e.contains("is not a whole number of minutes"), "{e}");
        let e = conv("title,start,duration_minutes\nA,2026-07-24 09:00,0").unwrap_err();
        assert!(e.contains("out of range"), "{e}");
        let e = run("title,start\nA,2026-07-24 09:00", "floating", 0, false).unwrap_err();
        assert!(e.contains("default_duration_minutes 0 is out of range"), "{e}");
        let e = run("title,start\nA,2026-07-24 09:00", "floating", 1441, false).unwrap_err();
        assert!(e.contains("out of range"), "{e}");
    }

    #[test]
    fn err_end_before_start() {
        let e = conv("title,start,end\nA,2026-07-24 10:00,2026-07-24 09:00").unwrap_err();
        assert!(e.contains("ends"), "{e}");
        let e = conv("title,start,end\nA,2026-07-24,2026-07-20").unwrap_err();
        assert!(e.contains("before the start date"), "{e}");
    }

    #[test]
    fn err_unknown_timezone() {
        let e = run("title,start\nA,2026-07-24", "Europe/Berlin", 60, false).unwrap_err();
        assert!(e.contains("unknown timezone"), "{e}");
    }

    #[test]
    fn max_events_boundary() {
        let mut csv = String::from("title,start\n");
        for i in 0..MAX_EVENTS {
            csv.push_str(&format!("E{i},2026-07-24\n"));
        }
        assert!(conv(&csv).is_ok(), "exactly MAX_EVENTS must convert");
        csv.push_str("one over,2026-07-24\n");
        assert!(conv(&csv).unwrap_err().contains("at most 5000 rows"));
    }
}
