//! recurring-task-expander core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps, no clock: every surface passes
//! the start date in explicitly so the expansion is deterministic.
//!
//! Input is a plain task list (one task per line, todo.txt-flavored). A task may
//! carry a recurrence tag `rec:<value>` and an optional anchor `due:YYYY-MM-DD`.
//! Output is the next N concrete dated instances of every recurring task.

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/// Maximum task lines accepted in one run.
pub const MAX_LINES: usize = 200;
/// Maximum instances generated per task.
pub const MAX_COUNT: u32 = 100;
/// Maximum numeric interval in a recurrence value (e.g. the 12 in `rec:12m`).
const MAX_INTERVAL: i64 = 999;
/// Safety bound on the day-by-day / step-by-step search for matching dates.
const MAX_SCAN: usize = 200_000;

// ---------------------------------------------------------------------------
// Civil-date helpers (Howard Hinnant's days_from_civil / civil_from_days)
// ---------------------------------------------------------------------------

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn last_day_of_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 0 = Sunday … 6 = Saturday (1970-01-01 was a Thursday).
fn weekday_of(days: i64) -> u8 {
    (((days % 7) + 11) % 7) as u8
}

const WEEKDAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const WEEKDAY_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn fmt_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Parse a strict `YYYY-MM-DD` calendar date into days since the Unix epoch.
pub fn parse_date(s: &str) -> Result<i64, String> {
    let s = s.trim();
    let parts: Vec<&str> = s.split('-').collect();
    let bad = || format!("invalid date '{s}' — expected YYYY-MM-DD, e.g. 2026-03-01");
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return Err(bad());
    }
    if !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())) {
        return Err(bad());
    }
    let y: i64 = parts[0].parse().map_err(|_| bad())?;
    let m: i64 = parts[1].parse().map_err(|_| bad())?;
    let d: i64 = parts[2].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&m) {
        return Err(format!("invalid date '{s}' — month must be 01-12"));
    }
    if d < 1 || d > last_day_of_month(y, m) {
        return Err(format!(
            "invalid date '{s}' — {y:04}-{m:02} has {} days",
            last_day_of_month(y, m)
        ));
    }
    Ok(days_from_civil(y, m, d))
}

/// Convert a Unix timestamp (seconds) into a `YYYY-MM-DD` UTC date string.
/// Each surface supplies its own clock (std on the CLI/chat block, `Date.now()`
/// in the browser) and passes the result in as `start`.
pub fn date_from_epoch_secs(secs: i64) -> String {
    fmt_date(secs.div_euclid(86_400))
}

/// Add `months` calendar months, clamping the day to the target month's length
/// (2026-01-31 + 1 month → 2026-02-28).
fn add_months(days: i64, months: i64) -> i64 {
    let (y, m, d) = civil_from_days(days);
    let total = (y * 12 + (m - 1)) + months;
    let ny = total.div_euclid(12);
    let nm = total.rem_euclid(12) + 1;
    let nd = d.min(last_day_of_month(ny, nm));
    days_from_civil(ny, nm, nd)
}

/// Add `n` business days (Mon-Fri). `n = 0` returns the same day unchanged.
fn add_business_days(days: i64, n: i64) -> i64 {
    let mut cur = days;
    let mut left = n;
    while left > 0 {
        cur += 1;
        if (1..=5).contains(&weekday_of(cur)) {
            left -= 1;
        }
    }
    cur
}

// ---------------------------------------------------------------------------
// Recurrence parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Step {
    Days(i64),
    BusinessDays(i64),
    Weeks(i64),
    Months(i64),
    Years(i64),
    /// Explicit weekday pattern, e.g. `mon,thu` — sorted, deduped, 0 = Sunday.
    Weekdays(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
struct Rec {
    /// `rec:+1m` — anchor the series on the task's due date (fixed schedule).
    strict: bool,
    step: Step,
    /// The recurrence value exactly as written (without the `rec:` prefix).
    raw: String,
}

fn weekday_index(name: &str) -> Option<u8> {
    Some(match name {
        "sun" | "sunday" | "su" => 0,
        "mon" | "monday" | "mo" => 1,
        "tue" | "tues" | "tuesday" | "tu" => 2,
        "wed" | "weds" | "wednesday" | "we" => 3,
        "thu" | "thur" | "thurs" | "thursday" | "th" => 4,
        "fri" | "friday" | "fr" => 5,
        "sat" | "saturday" | "sa" => 6,
        _ => return None,
    })
}

fn rec_syntax_help() -> String {
    "expected a number plus a unit — d (days), b (business days), w (weeks), m (months) or \
y (years), e.g. 1w, 3d, +2m — or weekday names, e.g. mon, mon,thu, weekdays, weekends. \
A leading + (rec:+1m) keeps the fixed due-date schedule"
        .to_string()
}

/// Parse a recurrence value such as `1w`, `+2m`, `10b`, `mon,thu`, `weekdays`.
fn parse_rec(value: &str) -> Result<Rec, String> {
    let raw = value.trim().to_string();
    if raw.is_empty() {
        return Err(format!("empty recurrence — {}", rec_syntax_help()));
    }
    let (strict, body) = match raw.strip_prefix('+') {
        Some(rest) => (true, rest.trim()),
        None => (false, raw.as_str()),
    };
    let lower = body.to_ascii_lowercase();
    if lower.is_empty() {
        return Err(format!("empty recurrence '{raw}' — {}", rec_syntax_help()));
    }

    // Weekday patterns: a single alias or a comma-separated list of weekday names.
    if lower.chars().all(|c| c.is_ascii_alphabetic() || c == ',') {
        let mut set: Vec<u8> = Vec::new();
        for part in lower.split(',') {
            let part = part.trim();
            if part.is_empty() {
                return Err(format!(
                    "invalid recurrence '{raw}' — empty weekday in the list; {}",
                    rec_syntax_help()
                ));
            }
            match part {
                "weekday" | "weekdays" => set.extend_from_slice(&[1, 2, 3, 4, 5]),
                "weekend" | "weekends" => set.extend_from_slice(&[0, 6]),
                "daily" => set.extend_from_slice(&[0, 1, 2, 3, 4, 5, 6]),
                other => match weekday_index(other) {
                    Some(w) => set.push(w),
                    None => {
                        return Err(format!(
                            "unknown recurrence '{raw}' — '{other}' is not a weekday name; {}",
                            rec_syntax_help()
                        ))
                    }
                },
            }
        }
        set.sort_unstable();
        set.dedup();
        return Ok(Rec {
            strict,
            step: Step::Weekdays(set),
            raw,
        });
    }

    // Numeric interval + unit.
    let digits: String = lower.chars().take_while(|c| c.is_ascii_digit()).collect();
    let unit = lower[digits.len()..].trim();
    if digits.is_empty() {
        return Err(format!(
            "unknown recurrence '{raw}' — {}",
            rec_syntax_help()
        ));
    }
    let n: i64 = digits
        .parse()
        .map_err(|_| format!("recurrence interval '{digits}' is too large (max {MAX_INTERVAL})"))?;
    if n < 1 {
        return Err(format!(
            "invalid recurrence '{raw}' — the interval must be at least 1"
        ));
    }
    if n > MAX_INTERVAL {
        return Err(format!(
            "invalid recurrence '{raw}' — the interval must be {MAX_INTERVAL} or less"
        ));
    }
    let step = match unit {
        "d" | "day" | "days" => Step::Days(n),
        "b" | "bd" | "business" | "businessday" | "businessdays" => Step::BusinessDays(n),
        "w" | "week" | "weeks" => Step::Weeks(n),
        "m" | "month" | "months" => Step::Months(n),
        "y" | "year" | "years" => Step::Years(n),
        "" => {
            return Err(format!(
                "recurrence '{raw}' has no unit — {}",
                rec_syntax_help()
            ))
        }
        other => {
            return Err(format!(
                "unknown recurrence unit '{other}' in '{raw}' — {}",
                rec_syntax_help()
            ))
        }
    };
    Ok(Rec { strict, step, raw })
}

/// The k-th date of the series counted from `anchor` (k = 0 is the anchor).
fn nth_from(anchor: i64, step: &Step, k: i64) -> i64 {
    match step {
        Step::Days(n) => anchor + n * k,
        Step::Weeks(n) => anchor + 7 * n * k,
        Step::BusinessDays(n) => add_business_days(anchor, n * k),
        Step::Months(n) => add_months(anchor, n * k),
        Step::Years(n) => add_months(anchor, 12 * n * k),
        Step::Weekdays(_) => anchor,
    }
}

// ---------------------------------------------------------------------------
// Task-line parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Task {
    /// 1-based line number in the input.
    line_no: usize,
    /// The task text with the `rec:`/`due:` tags removed.
    description: String,
    rec: Option<Rec>,
    /// The `due:` date, in days since the epoch.
    due: Option<i64>,
}

/// Strip a leading list marker (`- `, `* `, `- [ ] `, `1. `) from a task line.
fn strip_list_marker(line: &str) -> &str {
    let t = line.trim();
    let t = if let Some(rest) = t.strip_prefix("- ") {
        rest
    } else if let Some(rest) = t.strip_prefix("* ") {
        rest
    } else if let Some(rest) = t.strip_prefix("+ ") {
        rest
    } else {
        // "12. task" — an ordered-list marker.
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        match (digits.is_empty(), t[digits.len()..].strip_prefix(". ")) {
            (false, Some(rest)) => rest,
            _ => t,
        }
    };
    let t = t.trim_start();
    // Checkbox, with either state.
    for marker in ["[ ] ", "[] ", "[x] ", "[X] "] {
        if let Some(rest) = t.strip_prefix(marker) {
            return rest.trim_start();
        }
    }
    t
}

fn parse_task(line_no: usize, line: &str, default_rec: &Option<Rec>) -> Result<Task, String> {
    let body = strip_list_marker(line);
    let mut kept: Vec<&str> = Vec::new();
    let mut rec: Option<Rec> = None;
    let mut due: Option<i64> = None;
    for tok in body.split_whitespace() {
        if let Some(v) = tok.strip_prefix("rec:") {
            rec = Some(parse_rec(v).map_err(|e| format!("line {line_no}: {e}"))?);
            continue;
        }
        if let Some(v) = tok.strip_prefix("due:") {
            let d = parse_date(v).map_err(|e| {
                format!("line {line_no}: {e} (in the due: tag of '{}')", body.trim())
            })?;
            due = Some(d);
            continue;
        }
        kept.push(tok);
    }
    let description = kept.join(" ");
    if description.is_empty() {
        return Err(format!(
            "line {line_no}: the task has no text left after the rec:/due: tags — add a description, e.g. 'Pay rent due:2026-09-01 rec:+1m'"
        ));
    }
    Ok(Task {
        line_no,
        description,
        rec: rec.or_else(|| default_rec.clone()),
        due,
    })
}

// ---------------------------------------------------------------------------
// Expansion
// ---------------------------------------------------------------------------

fn shift_off_weekend(days: i64) -> i64 {
    match weekday_of(days) {
        6 => days + 2, // Saturday → Monday
        0 => days + 1, // Sunday → Monday
        _ => days,
    }
}

/// Generate the next `count` dates for one task, on or after `start`.
fn instances(task: &Task, start: i64, count: u32, skip_weekends: bool) -> Result<Vec<i64>, String> {
    let rec = match &task.rec {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    let anchor = task.due.unwrap_or(start);
    let mut out: Vec<i64> = Vec::new();

    match &rec.step {
        Step::Weekdays(set) => {
            // A weekday pattern is an absolute grid: walk forward day by day from
            // the later of the anchor and the start date. `+` is a no-op here.
            let mut cur = anchor.max(start);
            let mut scanned = 0usize;
            while out.len() < count as usize {
                if set.contains(&weekday_of(cur)) {
                    out.push(cur);
                }
                cur += 1;
                scanned += 1;
                if scanned > MAX_SCAN {
                    return Err(format!(
                        "line {}: recurrence '{}' produced no dates — check the weekday list",
                        task.line_no, rec.raw
                    ));
                }
            }
        }
        step => {
            // Strict (`+`) keeps the original due-date grid and simply skips the
            // occurrences already in the past. Plain recurrence is
            // completion-based: an overdue task restarts from the start date.
            let base = if rec.strict || anchor >= start {
                anchor
            } else {
                start
            };
            let mut k: i64 = 0;
            let mut scanned = 0usize;
            let mut last: Option<i64> = None;
            while out.len() < count as usize {
                let mut d = nth_from(base, step, k);
                k += 1;
                scanned += 1;
                if scanned > MAX_SCAN {
                    return Err(format!(
                        "line {}: recurrence '{}' did not reach the start date",
                        task.line_no, rec.raw
                    ));
                }
                if d < start {
                    continue;
                }
                if skip_weekends {
                    d = shift_off_weekend(d);
                }
                // Weekend shifting can collapse two occurrences onto the same
                // Monday — keep the instances distinct.
                if last == Some(d) {
                    continue;
                }
                last = Some(d);
                out.push(d);
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
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

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

struct Expanded {
    task: Task,
    dates: Vec<i64>,
}

fn render_text(rows: &[Expanded]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for r in rows {
        if r.task.rec.is_none() {
            match r.task.due {
                Some(d) => lines.push(format!("{} due:{}", r.task.description, fmt_date(d))),
                None => lines.push(r.task.description.clone()),
            }
            continue;
        }
        for d in &r.dates {
            lines.push(format!("{} due:{}", r.task.description, fmt_date(*d)));
        }
    }
    lines.join("\n")
}

fn render_markdown(rows: &[Expanded]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for r in rows {
        if r.task.rec.is_none() {
            match r.task.due {
                Some(d) => lines.push(format!(
                    "- [ ] {} — due {} ({})",
                    r.task.description,
                    fmt_date(d),
                    WEEKDAY_SHORT[weekday_of(d) as usize]
                )),
                None => lines.push(format!("- [ ] {}", r.task.description)),
            }
            continue;
        }
        for d in &r.dates {
            lines.push(format!(
                "- [ ] {} — due {} ({})",
                r.task.description,
                fmt_date(*d),
                WEEKDAY_SHORT[weekday_of(*d) as usize]
            ));
        }
    }
    lines.join("\n")
}

fn render_csv(rows: &[Expanded]) -> String {
    let mut out = String::from("task,recurrence,instance,date,weekday");
    for r in rows {
        let rec = r.task.rec.as_ref().map(|x| x.raw.clone()).unwrap_or_default();
        if r.task.rec.is_none() {
            let (date, wd) = match r.task.due {
                Some(d) => (fmt_date(d), WEEKDAY_NAMES[weekday_of(d) as usize].to_string()),
                None => (String::new(), String::new()),
            };
            out.push_str(&format!(
                "\n{},{},,{},{}",
                csv_field(&r.task.description),
                csv_field(&rec),
                csv_field(&date),
                csv_field(&wd)
            ));
            continue;
        }
        for (i, d) in r.dates.iter().enumerate() {
            out.push_str(&format!(
                "\n{},{},{},{},{}",
                csv_field(&r.task.description),
                csv_field(&rec),
                i + 1,
                fmt_date(*d),
                WEEKDAY_NAMES[weekday_of(*d) as usize]
            ));
        }
    }
    out
}

fn render_json(rows: &[Expanded], start: i64, count: u32) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"start\": \"{}\",\n", fmt_date(start)));
    out.push_str(&format!("  \"count\": {count},\n"));
    out.push_str("  \"tasks\": [");
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n    {\n");
        out.push_str(&format!("      \"line\": {},\n", r.task.line_no));
        out.push_str(&format!(
            "      \"description\": \"{}\",\n",
            json_escape(&r.task.description)
        ));
        match &r.task.rec {
            Some(rec) => {
                out.push_str(&format!(
                    "      \"recurrence\": \"{}\",\n",
                    json_escape(&rec.raw)
                ));
                out.push_str(&format!("      \"strict\": {},\n", rec.strict));
            }
            None => {
                out.push_str("      \"recurrence\": null,\n");
                out.push_str("      \"strict\": false,\n");
            }
        }
        match r.task.due {
            Some(d) => out.push_str(&format!("      \"due\": \"{}\",\n", fmt_date(d))),
            None => out.push_str("      \"due\": null,\n"),
        }
        out.push_str("      \"instances\": [");
        if r.task.rec.is_none() {
            // Non-recurring lines pass through as a single instance.
            let (date, wd) = match r.task.due {
                Some(d) => (
                    format!("\"{}\"", fmt_date(d)),
                    format!("\"{}\"", WEEKDAY_NAMES[weekday_of(d) as usize]),
                ),
                None => ("null".to_string(), "null".to_string()),
            };
            let line = match r.task.due {
                Some(d) => format!("{} due:{}", r.task.description, fmt_date(d)),
                None => r.task.description.clone(),
            };
            out.push_str(&format!(
                "\n        {{ \"date\": {date}, \"weekday\": {wd}, \"line\": \"{}\" }}",
                json_escape(&line)
            ));
        }
        for (j, d) in r.dates.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "\n        {{ \"date\": \"{}\", \"weekday\": \"{}\", \"line\": \"{}\" }}",
                fmt_date(*d),
                WEEKDAY_NAMES[weekday_of(*d) as usize],
                json_escape(&format!("{} due:{}", r.task.description, fmt_date(*d)))
            ));
        }
        out.push_str("\n      ]\n    }");
    }
    out.push_str("\n  ]\n}");
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Expand every recurring task in `tasks` into its next `count` dated instances.
///
/// * `tasks` — one task per line; `rec:<value>` sets the recurrence and
///   `due:YYYY-MM-DD` the anchor. Blank lines and `#` comments are ignored.
/// * `start` — the base date (`YYYY-MM-DD`); nothing before it is emitted.
/// * `count` — instances per recurring task (1-100).
/// * `default_rec` — recurrence applied to lines that carry no `rec:` tag
///   (blank = leave those lines alone).
/// * `skip_weekends` — move an instance that lands on Sat/Sun to the Monday.
/// * `format` — `text`, `markdown`, `json` or `csv`.
pub fn expand(
    tasks: &str,
    start: &str,
    count: u32,
    default_rec: &str,
    skip_weekends: bool,
    format: &str,
) -> Result<String, String> {
    let fmt = format.trim().to_ascii_lowercase();
    let fmt = if fmt.is_empty() { "text".to_string() } else { fmt };
    if !matches!(fmt.as_str(), "text" | "markdown" | "json" | "csv") {
        return Err(format!(
            "unknown format '{format}' — expected text, markdown, json or csv"
        ));
    }
    if !(1..=MAX_COUNT).contains(&count) {
        return Err(format!(
            "count must be between 1 and {MAX_COUNT} (got {count})"
        ));
    }
    let start_days = parse_date(start).map_err(|e| format!("start date: {e}"))?;
    let default = {
        let d = default_rec.trim();
        if d.is_empty() {
            None
        } else {
            Some(parse_rec(d).map_err(|e| format!("default recurrence: {e}"))?)
        }
    };

    let mut rows: Vec<Expanded> = Vec::new();
    let mut lines_seen = 0usize;
    for (idx, raw_line) in tasks.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        lines_seen += 1;
        if lines_seen > MAX_LINES {
            return Err(format!(
                "too many task lines — this tool expands at most {MAX_LINES} lines per run"
            ));
        }
        let task = parse_task(idx + 1, raw_line, &default)?;
        let dates = instances(&task, start_days, count, skip_weekends)?;
        rows.push(Expanded { task, dates });
    }
    if rows.is_empty() {
        return Err(
            "no tasks found — paste one task per line, e.g. 'Pay rent due:2026-09-01 rec:+1m'"
                .to_string(),
        );
    }

    Ok(match fmt.as_str() {
        "markdown" => render_markdown(&rows),
        "json" => render_json(&rows, start_days, count),
        "csv" => render_csv(&rows),
        _ => render_text(&rows),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn text(tasks: &str, start: &str, count: u32) -> String {
        expand(tasks, start, count, "", false, "text").unwrap()
    }

    #[test]
    fn happy_path_weekly_from_due_date() {
        let out = text("Water plants due:2026-08-10 rec:1w", "2026-08-08", 3);
        assert_eq!(
            out,
            "Water plants due:2026-08-10\n\
             Water plants due:2026-08-17\n\
             Water plants due:2026-08-24"
        );
    }

    #[test]
    fn error_on_unknown_recurrence_unit() {
        let err = expand("Do thing rec:2x", "2026-08-08", 3, "", false, "text").unwrap_err();
        assert!(err.starts_with("line 1: unknown recurrence unit 'x'"), "{err}");
        assert!(err.contains("business days"), "{err}");
    }

    #[test]
    fn error_on_bad_due_date() {
        let err = expand("Do thing due:2026-02-30 rec:1d", "2026-08-08", 2, "", false, "text")
            .unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("2026-02 has 28 days"), "{err}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = expand("   \n# just a comment\n", "2026-08-08", 3, "", false, "text").unwrap_err();
        assert!(err.starts_with("no tasks found"), "{err}");
    }

    #[test]
    fn count_cap_boundary() {
        assert!(expand("t rec:1d", "2026-08-08", 100, "", false, "text").is_ok());
        let err = expand("t rec:1d", "2026-08-08", 101, "", false, "text").unwrap_err();
        assert_eq!(err, "count must be between 1 and 100 (got 101)");
        let err = expand("t rec:1d", "2026-08-08", 0, "", false, "text").unwrap_err();
        assert_eq!(err, "count must be between 1 and 100 (got 0)");
    }

    #[test]
    fn strict_keeps_the_original_grid_when_overdue() {
        // Weekly report was due on a past Wednesday: the strict series stays on
        // Wednesdays and simply skips the occurrences already gone.
        let out = text("Weekly report due:2026-07-01 rec:+1w", "2026-08-08", 2);
        assert_eq!(
            out,
            "Weekly report due:2026-08-12\nWeekly report due:2026-08-19"
        );
    }

    #[test]
    fn plain_recurrence_restarts_from_start_when_overdue() {
        // Same task without the '+': completion-based, so it restarts today.
        let out = text("Weekly report due:2026-07-01 rec:1w", "2026-08-08", 2);
        assert_eq!(
            out,
            "Weekly report due:2026-08-08\nWeekly report due:2026-08-15"
        );
    }

    #[test]
    fn monthly_clamps_to_the_end_of_short_months() {
        // Jan 31 + 1 month → Feb 28; the grid stays anchored on the 31st.
        let out = text("Invoice due:2026-01-31 rec:+1m", "2026-01-01", 4);
        assert_eq!(
            out,
            "Invoice due:2026-01-31\n\
             Invoice due:2026-02-28\n\
             Invoice due:2026-03-31\n\
             Invoice due:2026-04-30"
        );
    }

    #[test]
    fn yearly_clamps_leap_day() {
        let out = text("Leap party due:2024-02-29 rec:+1y", "2024-01-01", 3);
        assert_eq!(
            out,
            "Leap party due:2024-02-29\n\
             Leap party due:2025-02-28\n\
             Leap party due:2026-02-28"
        );
    }

    #[test]
    fn business_days_skip_the_weekend() {
        // 2026-08-13 is a Thursday; +1b twice lands on Fri then Mon.
        let out = text("Standup due:2026-08-13 rec:+1b", "2026-08-13", 4);
        assert_eq!(
            out,
            "Standup due:2026-08-13\n\
             Standup due:2026-08-14\n\
             Standup due:2026-08-17\n\
             Standup due:2026-08-18"
        );
    }

    #[test]
    fn weekday_pattern_lists_each_matching_day() {
        // 2026-08-08 is a Saturday.
        let out = text("Gym rec:mon,thu", "2026-08-08", 4);
        assert_eq!(
            out,
            "Gym due:2026-08-10\n\
             Gym due:2026-08-13\n\
             Gym due:2026-08-17\n\
             Gym due:2026-08-20"
        );
    }

    #[test]
    fn weekday_alias_expands_to_monday_through_friday() {
        let out = text("Timesheet rec:weekdays", "2026-08-08", 3);
        assert_eq!(
            out,
            "Timesheet due:2026-08-10\n\
             Timesheet due:2026-08-11\n\
             Timesheet due:2026-08-12"
        );
    }

    #[test]
    fn skip_weekends_shifts_to_monday_and_dedupes() {
        // Daily from Friday: Sat and Sun both shift to Monday, collapsing into
        // one instance, so the result is consecutive business days.
        let out = expand("Check email rec:+1d", "2026-08-14", 4, "", true, "text").unwrap();
        assert_eq!(
            out,
            "Check email due:2026-08-14\n\
             Check email due:2026-08-17\n\
             Check email due:2026-08-18\n\
             Check email due:2026-08-19"
        );
    }

    #[test]
    fn no_due_date_anchors_on_the_start_date() {
        let out = text("Backup laptop rec:2w", "2026-08-08", 2);
        assert_eq!(
            out,
            "Backup laptop due:2026-08-08\nBackup laptop due:2026-08-22"
        );
    }

    #[test]
    fn future_due_date_is_kept_as_the_first_instance() {
        let out = text("Pay rent due:2026-09-01 rec:1m", "2026-08-08", 2);
        assert_eq!(out, "Pay rent due:2026-09-01\nPay rent due:2026-10-01");
    }

    #[test]
    fn default_rec_applies_only_to_untagged_lines() {
        let out = expand(
            "Water plants\nPay rent due:2026-09-01 rec:+1m",
            "2026-08-08",
            2,
            "1w",
            false,
            "text",
        )
        .unwrap();
        assert_eq!(
            out,
            "Water plants due:2026-08-08\n\
             Water plants due:2026-08-15\n\
             Pay rent due:2026-09-01\n\
             Pay rent due:2026-10-01"
        );
    }

    #[test]
    fn untagged_lines_pass_through_unchanged() {
        let out = text("Buy milk\nPay rent due:2026-09-01 rec:+1m", "2026-08-08", 1);
        assert_eq!(out, "Buy milk\nPay rent due:2026-09-01");
    }

    #[test]
    fn keeps_priority_project_and_context_tags() {
        let out = text("(A) Pay rent +home @bank due:2026-09-01 rec:+1m", "2026-08-08", 1);
        assert_eq!(out, "(A) Pay rent +home @bank due:2026-09-01");
    }

    #[test]
    fn strips_markdown_and_ordered_list_markers() {
        let out = text("- [ ] Standup rec:mon\n2. Retro rec:fri", "2026-08-08", 1);
        assert_eq!(out, "Standup due:2026-08-10\nRetro due:2026-08-14");
    }

    #[test]
    fn markdown_format_renders_a_checklist_with_weekdays() {
        let out = expand("Pay rent due:2026-09-01 rec:+1m", "2026-08-08", 2, "", false, "markdown")
            .unwrap();
        assert_eq!(
            out,
            "- [ ] Pay rent — due 2026-09-01 (Tue)\n- [ ] Pay rent — due 2026-10-01 (Thu)"
        );
    }

    #[test]
    fn csv_format_has_a_header_and_quotes_commas() {
        let out = expand(
            "Call mum, then dad due:2026-08-10 rec:+1w",
            "2026-08-08",
            2,
            "",
            false,
            "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "task,recurrence,instance,date,weekday\n\
             \"Call mum, then dad\",+1w,1,2026-08-10,Monday\n\
             \"Call mum, then dad\",+1w,2,2026-08-17,Monday"
        );
    }

    #[test]
    fn json_format_is_parseable_and_complete() {
        let out = expand("Pay rent due:2026-09-01 rec:+1m", "2026-08-08", 2, "", false, "json")
            .unwrap();
        assert!(out.contains("\"start\": \"2026-08-08\""), "{out}");
        assert!(out.contains("\"recurrence\": \"+1m\""), "{out}");
        assert!(out.contains("\"strict\": true"), "{out}");
        assert!(out.contains("\"due\": \"2026-09-01\""), "{out}");
        assert!(
            out.contains("{ \"date\": \"2026-10-01\", \"weekday\": \"Thursday\", \"line\": \"Pay rent due:2026-10-01\" }"),
            "{out}"
        );
    }

    #[test]
    fn unknown_format_is_rejected() {
        let err = expand("t rec:1d", "2026-08-08", 1, "", false, "yaml").unwrap_err();
        assert_eq!(err, "unknown format 'yaml' — expected text, markdown, json or csv");
    }

    #[test]
    fn too_many_lines_is_rejected() {
        let many = (0..=MAX_LINES).map(|i| format!("t{i} rec:1d")).collect::<Vec<_>>().join("\n");
        let err = expand(&many, "2026-08-08", 1, "", false, "text").unwrap_err();
        assert!(err.starts_with("too many task lines"), "{err}");
    }

    #[test]
    fn bad_start_date_is_rejected() {
        let err = expand("t rec:1d", "08/08/2026", 1, "", false, "text").unwrap_err();
        assert!(err.starts_with("start date: invalid date"), "{err}");
    }

    #[test]
    fn epoch_helper_matches_utc_date() {
        assert_eq!(date_from_epoch_secs(1_767_225_600), "2026-01-01");
        assert_eq!(date_from_epoch_secs(0), "1970-01-01");
    }

    #[test]
    fn weekday_names_are_correct() {
        assert_eq!(WEEKDAY_NAMES[weekday_of(parse_date("2026-08-08").unwrap()) as usize], "Saturday");
        assert_eq!(WEEKDAY_NAMES[weekday_of(parse_date("1970-01-01").unwrap()) as usize], "Thursday");
    }
}
