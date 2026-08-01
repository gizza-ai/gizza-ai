//! deadline-countdown core — parse tasks with due dates, compute urgency from a
//! deterministic `now`, and render the sorted countdown in table/Markdown/JSON/CSV.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

const MAX_TASKS: usize = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountdownRow {
    pub task: String,
    pub due: String,
    pub status: String,
    pub remaining: String,
    pub total_minutes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Table,
    Markdown,
    Json,
    Csv,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "table" => Ok(Self::Table),
            "markdown" | "md" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            other => Err(format!(
                "unknown format '{other}' (use table, markdown, json, or csv)"
            )),
        }
    }
}

/// Main entry point used by CLI/chat/web.
pub fn run(
    tasks: &str,
    now: &str,
    format: &str,
    include_completed: bool,
    soon_days: i64,
) -> Result<String, String> {
    if tasks.trim().is_empty() {
        return Err("tasks is empty — paste one task per line with a due date".into());
    }
    if soon_days < 0 {
        return Err("soon_days must be 0 or greater".into());
    }
    let now = parse_datetime(now).map_err(|e| format!("invalid now: {e}"))?;
    let fmt = Format::parse(format)?;
    let mut rows = Vec::new();
    let mut skipped_completed = 0usize;
    let mut skipped_no_date = Vec::new();

    for (idx, raw) in tasks.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if !include_completed && is_completed(line) {
            skipped_completed += 1;
            continue;
        }
        match parse_task_line(line) {
            Ok((task, due)) => rows.push(build_row(task, due, now, soon_days)),
            Err(_) => skipped_no_date.push(idx + 1),
        }
        if rows.len() > MAX_TASKS {
            return Err(format!("too many tasks — maximum is {MAX_TASKS}"));
        }
    }

    if rows.is_empty() {
        let mut msg = "No dated tasks found.".to_string();
        if skipped_completed > 0 {
            msg.push_str(&format!(" Skipped {skipped_completed} completed task(s)."));
        }
        if !skipped_no_date.is_empty() {
            msg.push_str(&format!(
                " Lines without dates: {}.",
                join_usize(&skipped_no_date)
            ));
        }
        return Ok(msg);
    }

    rows.sort_by(|a, b| {
        sort_key(a)
            .cmp(&sort_key(b))
            .then_with(|| a.task.cmp(&b.task))
    });
    Ok(render(&rows, fmt, skipped_completed, &skipped_no_date))
}

fn is_completed(line: &str) -> bool {
    let l = line.trim_start().to_ascii_lowercase();
    l.starts_with("x ") || l.starts_with("done:") || l.starts_with("[x]") || l.starts_with("✓")
}

fn sort_key(row: &CountdownRow) -> (i8, i64) {
    if row.total_minutes < 0 {
        (0, row.total_minutes) // most overdue first
    } else {
        (1, row.total_minutes) // nearest upcoming first
    }
}

fn parse_task_line(line: &str) -> Result<(String, NaiveDateTime), String> {
    let line = strip_completion_marker(line);
    let lower = line.to_ascii_lowercase();
    for marker in ["due:", "deadline:"] {
        if let Some(pos) = lower.find(marker) {
            let after = line[pos + marker.len()..].trim_start();
            if let Some((dt, consumed)) = parse_datetime_prefix(after) {
                let mut task = format!("{} {}", line[..pos].trim(), after[consumed..].trim())
                    .trim()
                    .to_string();
                task = trim_task_punctuation(&task);
                if task.is_empty() {
                    task = line.to_string();
                }
                return Ok((task, dt));
            }
        }
    }
    if let Some((dt, start, end)) = find_any_date(line) {
        let task =
            trim_task_punctuation(&format!("{} {}", line[..start].trim(), line[end..].trim()));
        return Ok((
            if task.is_empty() {
                line.to_string()
            } else {
                task
            },
            dt,
        ));
    }
    Err("no due date found".into())
}

fn strip_completion_marker(line: &str) -> &str {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("x ") {
        trimmed[2..].trim_start()
    } else if lower.starts_with("done:") {
        trimmed[5..].trim_start()
    } else if lower.starts_with("[x]") {
        trimmed[3..].trim_start()
    } else if trimmed.starts_with('✓') {
        trimmed['✓'.len_utf8()..].trim_start()
    } else {
        line
    }
}

fn find_any_date(line: &str) -> Option<(NaiveDateTime, usize, usize)> {
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        if !bytes[i].is_ascii_digit() {
            continue;
        }
        let s = &line[i..];
        if let Some((dt, consumed)) = parse_datetime_prefix(s) {
            return Some((dt, i, i + consumed));
        }
    }
    None
}

fn parse_datetime_prefix(s: &str) -> Option<(NaiveDateTime, usize)> {
    let t = s.trim_start();
    let trim_offset = s.len() - t.len();
    let max = t.len().min(32);
    for len in (10..=max).rev() {
        if !t.is_char_boundary(len) {
            continue;
        }
        let cand = t[..len].trim_end_matches(|c: char| matches!(c, ',' | ';' | ')' | ']'));
        if cand.len() < 10 {
            continue;
        }
        if let Ok(dt) = parse_datetime(cand) {
            return Some((dt, trim_offset + cand.len()));
        }
    }
    None
}

fn parse_datetime(s: &str) -> Result<NaiveDateTime, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty date".into());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(t) {
        return Ok(dt.naive_local());
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(t, fmt) {
            return Ok(dt);
        }
    }
    for fmt in ["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%d.%m.%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Ok(d.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()));
        }
    }
    Err(format!(
        "could not parse '{t}' as a date/datetime — use 2026-08-15 or 2026-08-15 17:30"
    ))
}

fn trim_task_punctuation(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| matches!(c, '-' | '–' | '—' | ':' | ';' | ',' | '|'))
        .trim()
        .to_string()
}

fn build_row(task: String, due: NaiveDateTime, now: NaiveDateTime, soon_days: i64) -> CountdownRow {
    let mins = (due - now).num_minutes();
    let status = if mins < 0 {
        "OVERDUE".to_string()
    } else if due.date() == now.date() {
        "DUE TODAY".to_string()
    } else if mins <= soon_days * 24 * 60 {
        "DUE SOON".to_string()
    } else {
        "LATER".to_string()
    };
    CountdownRow {
        task,
        due: fmt_dt(due),
        status,
        remaining: human_delta(mins),
        total_minutes: mins,
    }
}

fn fmt_dt(dt: NaiveDateTime) -> String {
    if dt.time() == NaiveTime::from_hms_opt(0, 0, 0).unwrap() {
        dt.format("%Y-%m-%d").to_string()
    } else {
        dt.format("%Y-%m-%d %H:%M").to_string()
    }
}

fn human_delta(total_minutes: i64) -> String {
    if total_minutes == 0 {
        return "due now".to_string();
    }
    let overdue = total_minutes < 0;
    let mut mins = total_minutes.abs();
    let days = mins / (24 * 60);
    mins %= 24 * 60;
    let hours = mins / 60;
    let minutes = mins % 60;
    let body = if days > 0 && hours > 0 {
        format!("{days}d {hours}h")
    } else if days > 0 {
        format!("{days}d")
    } else if hours > 0 && minutes > 0 {
        format!("{hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h")
    } else {
        format!("{minutes}m")
    };
    if overdue {
        format!("{body} overdue")
    } else {
        format!("in {body}")
    }
}

fn render(
    rows: &[CountdownRow],
    fmt: Format,
    skipped_completed: usize,
    skipped_no_date: &[usize],
) -> String {
    let mut out = match fmt {
        Format::Table => render_table(rows),
        Format::Markdown => render_markdown(rows),
        Format::Json => serde_json::to_string_pretty(rows).unwrap_or_else(|_| "[]".to_string()),
        Format::Csv => render_csv(rows),
    };
    let mut notes = Vec::new();
    if skipped_completed > 0 {
        notes.push(format!("skipped {skipped_completed} completed task(s)"));
    }
    if !skipped_no_date.is_empty() {
        notes.push(format!(
            "lines without dates: {}",
            join_usize(skipped_no_date)
        ));
    }
    if !notes.is_empty() && !matches!(fmt, Format::Json | Format::Csv) {
        out.push_str("\n\nNote: ");
        out.push_str(&notes.join("; "));
        out.push('.');
    }
    out
}

fn render_table(rows: &[CountdownRow]) -> String {
    let mut lines = vec![
        "Status       Due              Remaining       Task".to_string(),
        "-----------  ---------------  --------------  ----".to_string(),
    ];
    for r in rows {
        lines.push(format!(
            "{:<11}  {:<15}  {:<14}  {}",
            r.status, r.due, r.remaining, r.task
        ));
    }
    lines.join("\n")
}

fn render_markdown(rows: &[CountdownRow]) -> String {
    let mut lines = vec![
        "| Status | Due | Remaining | Task |".to_string(),
        "|---|---:|---:|---|".to_string(),
    ];
    for r in rows {
        lines.push(format!(
            "| {} | {} | {} | {} |",
            r.status,
            r.due,
            r.remaining,
            escape_md(&r.task)
        ));
    }
    lines.join("\n")
}

fn render_csv(rows: &[CountdownRow]) -> String {
    let mut lines = vec!["status,due,remaining,total_minutes,task".to_string()];
    for r in rows {
        lines.push(format!(
            "{},{},{},{},{}",
            r.status,
            r.due,
            r.remaining,
            r.total_minutes,
            csv(&r.task)
        ));
    }
    lines.join("\n")
}

fn csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn escape_md(s: &str) -> String {
    s.replace('|', "\\|")
}

fn join_usize(v: &[usize]) -> String {
    v.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Submit taxes due: 2026-07-30\nShip launch due: 2026-07-31 16:00\nRenew cert due: 2026-08-05\nx Done item due: 2026-07-01";

    #[test]
    fn sorts_overdue_then_upcoming() {
        let out = run(SAMPLE, "2026-07-31 12:00", "table", false, 7).unwrap();
        let tax = out.find("Submit taxes").unwrap();
        let launch = out.find("Ship launch").unwrap();
        let cert = out.find("Renew cert").unwrap();
        assert!(tax < launch && launch < cert, "{out}");
        assert!(out.contains("OVERDUE"));
        assert!(out.contains("DUE TODAY"));
        assert!(out.contains("4h"));
        assert!(out.contains("skipped 1 completed"));
    }

    #[test]
    fn supports_json_and_completed_items() {
        let out = run(SAMPLE, "2026-07-31 12:00", "json", true, 3).unwrap();
        let rows: Vec<CountdownRow> = serde_json::from_str(&out).unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].task, "Done item");
        assert_eq!(rows[0].status, "OVERDUE");
    }

    #[test]
    fn parses_inline_date_without_marker() {
        let out = run("Call vendor 2026-08-01", "2026-07-31", "csv", false, 2).unwrap();
        assert!(
            out.contains("DUE SOON,2026-08-01,in 1d,1440,\"Call vendor\""),
            "{out}"
        );
    }

    #[test]
    fn reports_missing_input() {
        let err = run("", "2026-07-31", "table", false, 7).unwrap_err();
        assert!(err.contains("tasks is empty"));
    }

    #[test]
    fn validates_now_and_format() {
        assert!(run("Task due: 2026-08-01", "soon", "table", false, 7)
            .unwrap_err()
            .contains("invalid now"));
        assert!(run("Task due: 2026-08-01", "2026-07-31", "xml", false, 7)
            .unwrap_err()
            .contains("unknown format"));
    }
}
