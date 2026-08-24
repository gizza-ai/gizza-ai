//! tasks-to-ical core — pure compute, shared by the chat skill block and the web
//! page. Turns a todo.txt task list into one iCalendar (`.ics`) document: a
//! `VCALENDAR` wrapper and one `VTODO` (or `VEVENT`) per task, with RFC 5545
//! CRLF line endings, 75-octet line folding and escaped TEXT values, ready to
//! import into Apple Reminders, Nextcloud Tasks, Thunderbird, Google Calendar,
//! Outlook or anything else that speaks iCalendar.
//!
//! todo.txt syntax understood: a leading `x` completion marker, the completion
//! and creation dates that follow it, an `(A)`–`(Z)` priority, `+project` and
//! `@context` words, and the `due:`, `t:`, `rec:`, `pri:`, `h:` and `uid:`/`id:`
//! key/value tags. Unknown `key:value` words are left alone in the summary so
//! nothing a user wrote disappears.
//!
//! No clock and no I/O: `DTSTAMP` is a fixed constant and UIDs are derived from
//! the task itself, so the same task list always produces the same bytes.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

/// Hard cap on tasks converted in one call, so a huge paste can't blow up
/// memory. The chat/CLI schema and the page copy advertise this same bound.
pub const MAX_TASKS: usize = 2000;

/// Product identifier written into `PRODID`.
const PRODID: &str = "-//gizza-ai//tasks-to-ical//EN";

/// `DTSTAMP` is fixed rather than read from a clock: the value is only required
/// to exist, and pinning it keeps the output deterministic (re-running the same
/// list produces the same bytes, so re-imports and diffs stay clean).
const DTSTAMP: &str = "19700101T000000Z";

/// Longest reminder lead time accepted, in minutes: one week.
const MAX_REMINDER_MINUTES: i64 = 10_080;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Which iCalendar component each task becomes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Component {
    /// A to-do item: carries `DUE`, `STATUS` and `PERCENT-COMPLETE`, and is what
    /// task apps (Apple Reminders, Nextcloud Tasks, Thunderbird) import.
    Vtodo,
    /// A calendar event: lands on the grid in every calendar app, including the
    /// ones that ignore `VTODO` entirely.
    Vevent,
}

fn parse_component(s: &str) -> Result<Component, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "vtodo" | "todo" | "task" => Ok(Component::Vtodo),
        "vevent" | "event" => Ok(Component::Vevent),
        _ => Err(format!(
            "unknown component '{}' — use \"vtodo\" for to-do entries or \"vevent\" for calendar events",
            s.trim()
        )),
    }
}

/// Which tasks are exported.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Include {
    /// Only tasks carrying a `due:` or `t:` date.
    Dated,
    /// Every task, dated or not.
    All,
}

fn parse_include(s: &str) -> Result<Include, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "dated" => Ok(Include::Dated),
        "all" => Ok(Include::All),
        _ => Err(format!(
            "unknown include '{}' — use \"dated\" for tasks with a due: or t: date or \"all\" for every task",
            s.trim()
        )),
    }
}

/// How wall-clock times are anchored in the output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Zone {
    /// No zone marker: floating local time (`DUE:20260825T140000`), which every
    /// client reads as "whatever the local clock says".
    Floating,
    /// The written times are already UTC, so they get the `Z` suffix.
    Utc,
}

fn parse_zone(s: &str) -> Result<Zone, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "floating" | "local" => Ok(Zone::Floating),
        "utc" | "z" | "gmt" => Ok(Zone::Utc),
        _ => Err(format!(
            "unknown timezone '{}' — use \"floating\" for local wall-clock times or \"utc\" for times that are already UTC",
            s.trim()
        )),
    }
}

// ---------------------------------------------------------------------------
// todo.txt parsing
// ---------------------------------------------------------------------------

/// One parsed todo.txt line.
#[derive(Debug, Default)]
struct Task {
    /// The line's own 1-based number in the pasted text, for error messages.
    line_no: usize,
    /// The original line, verbatim — kept for `DESCRIPTION`.
    raw: String,
    /// Task text with the priority, leading dates and known tags removed.
    summary: String,
    completed: bool,
    priority: Option<char>,
    creation_date: Option<NaiveDate>,
    completion_date: Option<NaiveDate>,
    due: Option<(NaiveDate, Option<NaiveTime>)>,
    threshold: Option<(NaiveDate, Option<NaiveTime>)>,
    /// `+project` and `@context` words, in the order written, deduped.
    categories: Vec<String>,
    /// A `uid:` / `id:` tag, when the user pinned one.
    uid: Option<String>,
    /// A `rec:` tag, recognised so it can be reported rather than expanded.
    recurrence: Option<String>,
}

/// A bare `YYYY-MM-DD` word, as todo.txt writes creation/completion dates.
fn bare_date(word: &str) -> Option<NaiveDate> {
    let parts: Vec<&str> = word.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return None;
    }
    if !parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    NaiveDate::from_ymd_opt(
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    )
}

/// Parse a `due:` / `t:` value into a date and, when the value carries one, a
/// time. Accepted: `2026-08-25`, `2026-08-25T14:00` and `2026-08-25T14:00:30`.
fn tag_date(value: &str, tag: &str) -> Result<(NaiveDate, Option<NaiveTime>), String> {
    let text = value.trim();
    let bad = || {
        format!(
            "could not read '{tag}:{text}' as a date — use {tag}:2026-08-25 or {tag}:2026-08-25T14:00"
        )
    };
    let body = text.strip_suffix(['Z', 'z']).unwrap_or(text);
    let (date_part, time_part) = match body.split_once(['T', 't']) {
        Some((d, t)) => (d, t.trim()),
        None => (body, ""),
    };
    let date = bare_date(date_part).ok_or_else(|| bad())?;
    if time_part.is_empty() {
        return Ok((date, None));
    }
    let hms: Vec<&str> = time_part.split(':').collect();
    if !(2..=3).contains(&hms.len()) || !hms.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        return Err(format!(
            "could not read '{time_part}' in '{tag}:{text}' as a time — use HH:MM or HH:MM:SS on a 24-hour clock"
        ));
    }
    let num = |p: &str| p.parse::<u32>().map_err(|_| bad());
    let (h, mi) = (num(hms[0])?, num(hms[1])?);
    let s = if hms.len() == 3 { num(hms[2])? } else { 0 };
    let time = NaiveTime::from_hms_opt(h, mi, s).ok_or_else(|| {
        format!("'{time_part}' in '{tag}:{text}' is not a valid clock time (hour 0–23, minute and second 0–59)")
    })?;
    Ok((date, Some(time)))
}

/// Parse one todo.txt line.
///
/// The leading section is read positionally, the way todo.txt writes it — an
/// optional `x`, then (for a completed task) up to two bare dates, an optional
/// `(A)` priority, then an optional creation date — and each of those pieces is
/// accepted in either order, because real files disagree about whether the
/// priority comes before or after the completion date.
fn parse_task(line_no: usize, raw: &str) -> Result<Task, String> {
    let mut task = Task {
        line_no,
        raw: raw.to_string(),
        ..Task::default()
    };

    let words: Vec<&str> = raw.split_whitespace().collect();
    let mut i = 0usize;

    if words.first().is_some_and(|w| *w == "x" || *w == "X") {
        task.completed = true;
        i += 1;
    }

    // Priority and up to two leading bare dates, in whatever order they appear.
    // For a completed task the first bare date is the completion date and the
    // second is the creation date; for an open task the only one is creation.
    let mut dates: Vec<NaiveDate> = Vec::new();
    while i < words.len() {
        let w = words[i];
        if task.priority.is_none()
            && w.len() == 3
            && w.starts_with('(')
            && w.ends_with(')')
            && w.as_bytes()[1].is_ascii_uppercase()
        {
            task.priority = Some(w.as_bytes()[1] as char);
            i += 1;
        } else if dates.len() < if task.completed { 2 } else { 1 } {
            match bare_date(w) {
                Some(d) => {
                    dates.push(d);
                    i += 1;
                }
                None => break,
            }
        } else {
            break;
        }
    }
    if task.completed {
        task.completion_date = dates.first().copied();
        task.creation_date = dates.get(1).copied();
    } else {
        task.creation_date = dates.first().copied();
    }

    // Everything left is the task text: projects, contexts and known tags come
    // out of it; unknown key:value words stay where the user put them.
    let mut summary_words: Vec<&str> = Vec::new();
    for w in &words[i..] {
        if w.len() > 1 && (w.starts_with('+') || w.starts_with('@')) {
            let name = &w[1..];
            if !task.categories.iter().any(|c| c == name) {
                task.categories.push(name.to_string());
            }
            continue;
        }
        match w.split_once(':') {
            Some((key, value)) if !value.is_empty() => {
                match key.to_ascii_lowercase().as_str() {
                    "due" => {
                        task.due = Some(tag_date(value, "due")?);
                        continue;
                    }
                    "t" => {
                        task.threshold = Some(tag_date(value, "t")?);
                        continue;
                    }
                    "uid" | "id" => {
                        task.uid = Some(value.to_string());
                        continue;
                    }
                    "pri" => {
                        let c = value.chars().next().unwrap().to_ascii_uppercase();
                        if value.chars().count() == 1 && c.is_ascii_uppercase() {
                            task.priority = Some(c);
                            continue;
                        }
                    }
                    "rec" => {
                        task.recurrence = Some(value.to_string());
                        continue;
                    }
                    "h" => continue, // the "hidden" flag: metadata, not text
                    _ => {}
                }
                summary_words.push(w);
            }
            _ => summary_words.push(w),
        }
    }
    task.summary = summary_words.join(" ");
    Ok(task)
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

/// Write a date-or-datetime property: `VALUE=DATE` when the tag carried no
/// time, a full timestamp when it did.
fn fmt_stamp(name: &str, value: (NaiveDate, Option<NaiveTime>), zone: Zone, out: &mut String) {
    match value.1 {
        Some(t) => fold(
            &format!("{name}:{}", fmt_datetime(value.0.and_time(t), zone)),
            out,
        ),
        None => fold(&format!("{name};VALUE=DATE:{}", fmt_date(value.0)), out),
    }
}

/// todo.txt priority letter → the RFC 5545 numeric scale, where 1 is the
/// highest and 9 the lowest. `(A)` is high, `(B)` normal, everything else low.
fn priority_number(c: char) -> u8 {
    match c {
        'A' => 1,
        'B' => 5,
        _ => 9,
    }
}

/// Lowercase, hyphen-joined slug of a summary, for the generated UID.
fn slug(summary: &str) -> String {
    let mut out = String::new();
    for c in summary.chars() {
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
        "task".to_string()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// Convert a todo.txt task list into an iCalendar (`.ics`) document.
///
/// * `tasks_text` — the task list, one task per line.
/// * `component` — `vtodo` (to-do entries) or `vevent` (calendar events).
/// * `include` — `dated` (only tasks with `due:`/`t:`) or `all`.
/// * `skip_completed` — drop lines marked done with a leading `x`.
/// * `duration_minutes` — 1–1440, the length of a timed `VEVENT`.
/// * `reminder_minutes` — 0–10080; 0 writes no `VALARM`.
/// * `timezone` — `floating` (local wall-clock) or `utc`.
/// * `calendar_name` — optional `X-WR-CALNAME` for the whole calendar.
#[allow(clippy::too_many_arguments)]
pub fn run(
    tasks_text: &str,
    component: &str,
    include: &str,
    skip_completed: bool,
    duration_minutes: i64,
    reminder_minutes: i64,
    timezone: &str,
    calendar_name: &str,
) -> Result<String, String> {
    let component = parse_component(component)?;
    let include = parse_include(include)?;
    let zone = parse_zone(timezone)?;
    if !(1..=1440).contains(&duration_minutes) {
        return Err(format!(
            "duration_minutes {duration_minutes} is out of range — use a whole number of minutes from 1 to 1440"
        ));
    }
    if !(0..=MAX_REMINDER_MINUTES).contains(&reminder_minutes) {
        return Err(format!(
            "reminder_minutes {reminder_minutes} is out of range — use 0 for no reminder, or up to {MAX_REMINDER_MINUTES} minutes (one week) of lead time"
        ));
    }

    let trimmed = tasks_text.trim_start_matches('\u{feff}');
    if trimmed.trim().is_empty() {
        return Err("no tasks — paste a todo.txt list, one task per line, e.g. '(A) Pay invoice +work due:2026-08-25'".to_string());
    }

    let mut body = String::new();
    let mut count = 0usize;
    let mut skipped_undated = 0usize;
    let mut skipped_done = 0usize;

    for (n, line) in trimmed.lines().enumerate() {
        let line_no = n + 1;
        let text = line.trim();
        if text.is_empty() || text.starts_with('#') {
            continue; // blank spacer line or a comment
        }

        let task = parse_task(line_no, text)?;
        if task.summary.trim().is_empty() {
            return Err(format!(
                "line {}: this line has metadata but no task text — '{text}'",
                task.line_no
            ));
        }
        if skip_completed && task.completed {
            skipped_done += 1;
            continue;
        }
        let dated = task.due.is_some() || task.threshold.is_some();
        if include == Include::Dated && !dated {
            skipped_undated += 1;
            continue;
        }
        if component == Component::Vevent && !dated {
            return Err(format!(
                "line {}: '{}' has no due: or t: date, and a calendar event needs one — add due:2026-08-25, switch component to \"vtodo\", or leave include on \"dated\"",
                task.line_no, task.summary
            ));
        }
        if let (Some((ds, ts)), Some((de, te))) = (task.threshold, task.due) {
            if ds.and_time(ts.unwrap_or(NaiveTime::MIN)) > de.and_time(te.unwrap_or(NaiveTime::MIN))
            {
                return Err(format!(
                    "line {}: '{}' starts (t:{ds}) after it is due (due:{de})",
                    task.line_no, task.summary
                ));
            }
        }

        count += 1;
        if count > MAX_TASKS {
            return Err(format!(
                "too many tasks: this converts at most {MAX_TASKS} tasks in one run — split the list and convert it in parts"
            ));
        }

        body.push_str(&emit(
            &task,
            count,
            component,
            zone,
            duration_minutes,
            reminder_minutes,
        ));
    }

    if count == 0 {
        return Err(match (skipped_undated, skipped_done) {
            (0, 0) => "no tasks found — every line was blank or a # comment".to_string(),
            (u, 0) => format!(
                "no dated tasks found — {u} task(s) carry neither a due: nor a t: tag. Add due:2026-08-25 to one, or set include to \"all\"."
            ),
            (0, d) => format!(
                "no tasks left — all {d} task(s) are marked done and skip_completed is on."
            ),
            (u, d) => format!(
                "no tasks left — {d} task(s) were skipped as done and {u} carry no due: or t: date. Turn skip_completed off, or set include to \"all\"."
            ),
        });
    }

    let mut out = String::with_capacity(body.len() + 160);
    out.push_str("BEGIN:VCALENDAR\r\n");
    out.push_str("VERSION:2.0\r\n");
    fold(&format!("PRODID:{PRODID}"), &mut out);
    out.push_str("CALSCALE:GREGORIAN\r\n");
    let name = calendar_name.trim();
    if !name.is_empty() {
        fold(&format!("X-WR-CALNAME:{}", esc(name)), &mut out);
    }
    out.push_str(&body);
    out.push_str("END:VCALENDAR\r\n");
    Ok(out)
}

/// Render one parsed task as a `VTODO` or `VEVENT`.
fn emit(
    task: &Task,
    index: usize,
    component: Component,
    zone: Zone,
    duration_minutes: i64,
    reminder_minutes: i64,
) -> String {
    let tag = match component {
        Component::Vtodo => "VTODO",
        Component::Vevent => "VEVENT",
    };
    let mut out = String::new();
    out.push_str(&format!("BEGIN:{tag}\r\n"));

    let uid = task
        .uid
        .clone()
        .unwrap_or_else(|| format!("{}@{index}.local", slug(&task.summary)));
    fold(&format!("UID:{}", esc(&uid)), &mut out);
    fold(&format!("DTSTAMP:{DTSTAMP}"), &mut out);

    match component {
        Component::Vtodo => {
            if let Some(start) = task.threshold {
                fmt_stamp("DTSTART", start, zone, &mut out);
            }
            if let Some(due) = task.due {
                fmt_stamp("DUE", due, zone, &mut out);
            }
        }
        Component::Vevent => {
            // A calendar event starts at t: when there is one, else at due:.
            let start = task.threshold.or(task.due).expect("checked by the caller");
            fmt_stamp("DTSTART", start, zone, &mut out);
            match start.1 {
                // Timed: run for duration_minutes from the start.
                Some(t) => {
                    let end = start.0.and_time(t) + Duration::minutes(duration_minutes);
                    fold(&format!("DTEND:{}", fmt_datetime(end, zone)), &mut out);
                }
                // All-day: run through the due date. iCalendar all-day ends are
                // exclusive, so the written DTEND is the day after the last one.
                None => {
                    let last = match task.due {
                        Some((d, _)) => d.max(start.0),
                        None => start.0,
                    };
                    fold(
                        &format!("DTEND;VALUE=DATE:{}", fmt_date(last + Duration::days(1))),
                        &mut out,
                    );
                }
            }
        }
    }

    fold(&format!("SUMMARY:{}", esc(&task.summary)), &mut out);
    fold(&format!("DESCRIPTION:{}", esc(&task.raw)), &mut out);
    if !task.categories.is_empty() {
        let joined = task
            .categories
            .iter()
            .map(|c| esc(c))
            .collect::<Vec<_>>()
            .join(",");
        fold(&format!("CATEGORIES:{joined}"), &mut out);
    }
    if let Some(p) = task.priority {
        fold(&format!("PRIORITY:{}", priority_number(p)), &mut out);
    }
    if let Some(created) = task.creation_date {
        fold(&format!("CREATED:{}T000000Z", fmt_date(created)), &mut out);
    }
    if component == Component::Vtodo {
        if task.completed {
            out.push_str("STATUS:COMPLETED\r\n");
            out.push_str("PERCENT-COMPLETE:100\r\n");
            if let Some(done) = task.completion_date {
                fold(&format!("COMPLETED:{}T000000Z", fmt_date(done)), &mut out);
            }
        } else {
            out.push_str("STATUS:NEEDS-ACTION\r\n");
        }
    }

    if reminder_minutes > 0 {
        out.push_str("BEGIN:VALARM\r\n");
        out.push_str("ACTION:DISPLAY\r\n");
        // For a VTODO, RFC 5545 anchors a RELATED=END trigger to DUE, which is
        // the date the user cares about; a VEVENT triggers before its start.
        let trigger = match component {
            Component::Vtodo => format!("TRIGGER;RELATED=END:-PT{reminder_minutes}M"),
            Component::Vevent => format!("TRIGGER:-PT{reminder_minutes}M"),
        };
        out.push_str(&trigger);
        out.push_str("\r\n");
        fold(&format!("DESCRIPTION:{}", esc(&task.summary)), &mut out);
        out.push_str("END:VALARM\r\n");
    }

    out.push_str(&format!("END:{tag}\r\n"));
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(tasks: &str) -> Result<String, String> {
        run(tasks, "vtodo", "dated", false, 60, 0, "floating", "")
    }

    #[test]
    fn happy_path_dated_task_becomes_a_vtodo() {
        let ics = conv("(A) Pay the hosting invoice +admin @office due:2026-08-25").unwrap();
        assert_eq!(
            ics,
            "BEGIN:VCALENDAR\r\n\
             VERSION:2.0\r\n\
             PRODID:-//gizza-ai//tasks-to-ical//EN\r\n\
             CALSCALE:GREGORIAN\r\n\
             BEGIN:VTODO\r\n\
             UID:pay-the-hosting-invoice@1.local\r\n\
             DTSTAMP:19700101T000000Z\r\n\
             DUE;VALUE=DATE:20260825\r\n\
             SUMMARY:Pay the hosting invoice\r\n\
             DESCRIPTION:(A) Pay the hosting invoice +admin @office due:2026-08-25\r\n\
             CATEGORIES:admin,office\r\n\
             PRIORITY:1\r\n\
             STATUS:NEEDS-ACTION\r\n\
             END:VTODO\r\n\
             END:VCALENDAR\r\n"
        );
    }

    #[test]
    fn error_on_an_unreadable_due_date() {
        let err = conv("Ship the thing due:next-friday").unwrap_err();
        assert!(err.contains("due:next-friday"), "{err}");
        assert!(err.contains("2026-08-25"), "{err}");
    }

    #[test]
    fn empty_input_is_rejected() {
        assert!(conv("   \n\n").unwrap_err().contains("no tasks"));
    }

    #[test]
    fn threshold_becomes_dtstart_and_due_becomes_due() {
        let ics = conv("Draft the report t:2026-08-20 due:2026-08-25").unwrap();
        assert!(ics.contains("DTSTART;VALUE=DATE:20260820\r\n"), "{ics}");
        assert!(ics.contains("DUE;VALUE=DATE:20260825\r\n"), "{ics}");
    }

    #[test]
    fn a_threshold_after_the_due_date_is_reported_by_line() {
        let err = conv("Backwards t:2026-08-30 due:2026-08-25").unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("after it is due"), "{err}");
    }

    #[test]
    fn completed_tasks_carry_status_percent_and_dates() {
        let ics = conv("x 2026-08-20 2026-08-01 File the tax return due:2026-08-19").unwrap();
        assert!(ics.contains("STATUS:COMPLETED\r\n"), "{ics}");
        assert!(ics.contains("PERCENT-COMPLETE:100\r\n"), "{ics}");
        assert!(ics.contains("COMPLETED:20260820T000000Z\r\n"), "{ics}");
        assert!(ics.contains("CREATED:20260801T000000Z\r\n"), "{ics}");
        assert!(ics.contains("SUMMARY:File the tax return\r\n"), "{ics}");
    }

    #[test]
    fn skip_completed_drops_done_tasks() {
        let list = "x 2026-08-20 Done thing due:2026-08-19\nOpen thing due:2026-08-25";
        let kept = run(list, "vtodo", "dated", true, 60, 0, "floating", "").unwrap();
        assert!(!kept.contains("Done thing"), "{kept}");
        assert!(kept.contains("SUMMARY:Open thing"), "{kept}");

        let err = run(
            "x 2026-08-20 Done thing due:2026-08-19",
            "vtodo",
            "dated",
            true,
            60,
            0,
            "floating",
            "",
        )
        .unwrap_err();
        assert!(err.contains("marked done"), "{err}");
    }

    #[test]
    fn include_dated_skips_undated_tasks_and_all_keeps_them() {
        let list = "Buy milk\nRenew passport due:2026-09-01";
        let dated = conv(list).unwrap();
        assert!(!dated.contains("Buy milk"), "{dated}");
        assert!(dated.matches("BEGIN:VTODO").count() == 1, "{dated}");

        let all = run(list, "vtodo", "all", false, 60, 0, "floating", "").unwrap();
        assert!(all.contains("SUMMARY:Buy milk"), "{all}");
        assert_eq!(all.matches("BEGIN:VTODO").count(), 2, "{all}");
    }

    #[test]
    fn no_dated_tasks_gives_an_actionable_error() {
        let err = conv("Buy milk\nCall the dentist").unwrap_err();
        assert!(err.contains("no dated tasks"), "{err}");
        assert!(err.contains("include"), "{err}");
    }

    #[test]
    fn vevent_all_day_spans_threshold_through_due_exclusively() {
        let ics = run(
            "Conference prep t:2026-08-20 due:2026-08-22",
            "vevent",
            "dated",
            false,
            60,
            0,
            "floating",
            "",
        )
        .unwrap();
        assert!(ics.contains("BEGIN:VEVENT\r\n"), "{ics}");
        assert!(ics.contains("DTSTART;VALUE=DATE:20260820\r\n"), "{ics}");
        // Inclusive 20th–22nd → exclusive DTEND on the 23rd.
        assert!(ics.contains("DTEND;VALUE=DATE:20260823\r\n"), "{ics}");
        assert!(!ics.contains("STATUS:"), "{ics}");
    }

    #[test]
    fn vevent_from_a_due_date_alone_is_a_single_all_day_event() {
        let ics = run(
            "Renew passport due:2026-09-01",
            "vevent",
            "dated",
            false,
            60,
            0,
            "floating",
            "",
        )
        .unwrap();
        assert!(ics.contains("DTSTART;VALUE=DATE:20260901\r\n"), "{ics}");
        assert!(ics.contains("DTEND;VALUE=DATE:20260902\r\n"), "{ics}");
    }

    #[test]
    fn an_undated_task_cannot_become_an_event() {
        let err = run(
            "Buy milk",
            "vevent",
            "all",
            false,
            60,
            0,
            "floating",
            "",
        )
        .unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("calendar event needs one"), "{err}");
    }

    #[test]
    fn a_timed_due_tag_makes_a_timed_entry() {
        let ics = conv("Submit the filing due:2026-08-25T14:30").unwrap();
        assert!(ics.contains("DUE:20260825T143000\r\n"), "{ics}");

        let event = run(
            "Submit the filing due:2026-08-25T14:30",
            "vevent",
            "dated",
            false,
            45,
            0,
            "floating",
            "",
        )
        .unwrap();
        assert!(event.contains("DTSTART:20260825T143000\r\n"), "{event}");
        assert!(event.contains("DTEND:20260825T151500\r\n"), "{event}");
    }

    #[test]
    fn utc_timezone_adds_the_z_suffix_to_timed_values_only() {
        let ics = run(
            "Submit the filing due:2026-08-25T14:30",
            "vtodo",
            "dated",
            false,
            60,
            0,
            "utc",
            "",
        )
        .unwrap();
        assert!(ics.contains("DUE:20260825T143000Z\r\n"), "{ics}");

        let all_day = run(
            "Renew passport due:2026-09-01",
            "vtodo",
            "dated",
            false,
            60,
            0,
            "utc",
            "",
        )
        .unwrap();
        assert!(all_day.contains("DUE;VALUE=DATE:20260901\r\n"), "{all_day}");
    }

    #[test]
    fn reminders_anchor_to_due_for_todos_and_to_the_start_for_events() {
        let todo = run(
            "Renew passport due:2026-09-01",
            "vtodo",
            "dated",
            false,
            60,
            30,
            "floating",
            "",
        )
        .unwrap();
        assert!(todo.contains("BEGIN:VALARM\r\n"), "{todo}");
        assert!(
            todo.contains("TRIGGER;RELATED=END:-PT30M\r\n"),
            "{todo}"
        );
        assert!(todo.contains("DESCRIPTION:Renew passport\r\n"), "{todo}");

        let event = run(
            "Renew passport due:2026-09-01",
            "vevent",
            "dated",
            false,
            60,
            30,
            "floating",
            "",
        )
        .unwrap();
        assert!(event.contains("TRIGGER:-PT30M\r\n"), "{event}");
    }

    #[test]
    fn zero_reminder_writes_no_alarm() {
        assert!(!conv("Renew passport due:2026-09-01")
            .unwrap()
            .contains("VALARM"));
    }

    #[test]
    fn calendar_name_becomes_x_wr_calname() {
        let ics = run(
            "Renew passport due:2026-09-01",
            "vtodo",
            "dated",
            false,
            60,
            0,
            "floating",
            "Work deadlines",
        )
        .unwrap();
        assert!(ics.contains("X-WR-CALNAME:Work deadlines\r\n"), "{ics}");
    }

    #[test]
    fn a_pinned_uid_tag_wins_over_the_generated_one() {
        let ics = conv("Renew passport due:2026-09-01 uid:passport-2026").unwrap();
        assert!(ics.contains("UID:passport-2026\r\n"), "{ics}");
        assert!(!ics.contains("uid:passport-2026 "), "{ics}");
        let alias = conv("Renew passport due:2026-09-01 id:passport-2026").unwrap();
        assert!(alias.contains("UID:passport-2026\r\n"), "{alias}");
    }

    #[test]
    fn rec_and_h_tags_are_stripped_but_unknown_tags_are_kept() {
        let ics = conv("Water the plants rec:1w h:1 url:example.com due:2026-08-25").unwrap();
        assert!(
            ics.contains("SUMMARY:Water the plants url:example.com\r\n"),
            "{ics}"
        );
        // The raw line still holds everything, so nothing is actually lost.
        assert!(ics.contains("rec:1w"), "{ics}");
    }

    #[test]
    fn pri_tag_sets_the_priority_for_a_completed_task() {
        let ics = conv("x 2026-08-20 Ship it pri:B due:2026-08-19").unwrap();
        assert!(ics.contains("PRIORITY:5\r\n"), "{ics}");
        assert!(ics.contains("SUMMARY:Ship it\r\n"), "{ics}");
    }

    #[test]
    fn priority_letters_map_onto_the_rfc_scale() {
        let high = conv("(A) One due:2026-08-25").unwrap();
        assert!(high.contains("PRIORITY:1\r\n"), "{high}");
        let normal = conv("(B) Two due:2026-08-25").unwrap();
        assert!(normal.contains("PRIORITY:5\r\n"), "{normal}");
        let low = conv("(F) Three due:2026-08-25").unwrap();
        assert!(low.contains("PRIORITY:9\r\n"), "{low}");
        let none = conv("Four due:2026-08-25").unwrap();
        assert!(!none.contains("PRIORITY:"), "{none}");
    }

    #[test]
    fn priority_is_accepted_before_or_after_the_completion_date() {
        let after = conv("x 2026-08-20 (A) Ship it due:2026-08-19").unwrap();
        assert!(after.contains("PRIORITY:1\r\n"), "{after}");
        assert!(after.contains("COMPLETED:20260820T000000Z\r\n"), "{after}");
        let before = conv("x (A) 2026-08-20 Ship it due:2026-08-19").unwrap();
        assert!(before.contains("PRIORITY:1\r\n"), "{before}");
        assert!(before.contains("COMPLETED:20260820T000000Z\r\n"), "{before}");
    }

    #[test]
    fn blank_lines_and_comments_are_ignored() {
        let ics = conv("# my backlog\n\nRenew passport due:2026-09-01\n\n").unwrap();
        assert_eq!(ics.matches("BEGIN:VTODO").count(), 1, "{ics}");
        assert!(!ics.contains("my backlog"), "{ics}");
    }

    #[test]
    fn text_values_are_escaped_and_long_lines_folded() {
        let ics = conv("Buy milk, eggs; and bread due:2026-08-25").unwrap();
        assert!(
            ics.contains("SUMMARY:Buy milk\\, eggs\\; and bread\r\n"),
            "{ics}"
        );
        let long = conv(&format!("{} due:2026-08-25", "word ".repeat(40))).unwrap();
        assert!(long.contains("\r\n "), "expected a folded continuation line");
        assert!(long.lines().all(|l| l.trim_end().len() <= 75), "{long}");
    }

    #[test]
    fn duration_and_reminder_ranges_are_enforced() {
        let d = run("A due:2026-08-25", "vtodo", "dated", false, 0, 0, "floating", "").unwrap_err();
        assert!(d.contains("duration_minutes"), "{d}");
        let r = run(
            "A due:2026-08-25",
            "vtodo",
            "dated",
            false,
            60,
            10_081,
            "floating",
            "",
        )
        .unwrap_err();
        assert!(r.contains("reminder_minutes"), "{r}");
    }

    #[test]
    fn unknown_option_values_are_rejected_by_name() {
        assert!(run("A due:2026-08-25", "vjournal", "dated", false, 60, 0, "floating", "")
            .unwrap_err()
            .contains("vjournal"));
        assert!(run("A due:2026-08-25", "vtodo", "everything", false, 60, 0, "floating", "")
            .unwrap_err()
            .contains("everything"));
        assert!(run("A due:2026-08-25", "vtodo", "dated", false, 60, 0, "CET", "")
            .unwrap_err()
            .contains("CET"));
    }

    #[test]
    fn the_task_cap_is_enforced_at_the_boundary() {
        let list = (1..=MAX_TASKS)
            .map(|i| format!("Task {i} due:2026-08-25"))
            .collect::<Vec<_>>()
            .join("\n");
        let ics = conv(&list).unwrap();
        assert_eq!(ics.matches("BEGIN:VTODO").count(), MAX_TASKS, "at the cap");
        let over = conv(&format!("{list}\nTask {} due:2026-08-25", MAX_TASKS + 1)).unwrap_err();
        assert!(over.contains("at most 2000"), "{over}");
    }

    #[test]
    fn a_line_with_only_metadata_is_reported() {
        let err = conv("x 2026-08-20 due:2026-08-19").unwrap_err();
        assert!(err.contains("no task text"), "{err}");
    }
}
