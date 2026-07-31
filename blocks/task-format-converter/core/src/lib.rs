//! task-format-converter core — pure compute, shared by the chat skill block and the web page.
//!
//! Converts a task list between four formats in any direction:
//!   * `todotxt`  — the todo.txt plain-text spec (http://todotxt.org)
//!   * `markdown` — GitHub-flavored Markdown checklist (`- [ ] task`)
//!   * `json`     — an array of task objects
//!   * `csv`      — one task per row with a fixed header
//!
//! Every format is parsed into a common in-memory [`Task`] model that captures
//! the task text, completion state, priority, `+projects`, `@contexts`,
//! creation / due / completion dates, and any leftover `key:value` metadata
//! tags — so those attributes survive a round trip through any pair of formats.

use serde_json::{Map, Value};
use std::fmt;

/// The common task model. Every supported format parses into this and every
/// format is written back out of it, so attributes are preserved across a
/// conversion instead of being flattened into free text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Task {
    /// The task description with all inline metadata (priority, projects,
    /// contexts, dates, key:value tags) stripped out.
    pub text: String,
    /// Whether the task is completed.
    pub done: bool,
    /// Single uppercase priority letter `A`..=`Z`, if any.
    pub priority: Option<char>,
    /// `+project` tags, without the leading `+`.
    pub projects: Vec<String>,
    /// `@context` tags, without the leading `@`.
    pub contexts: Vec<String>,
    /// Creation date, `YYYY-MM-DD`.
    pub created: Option<String>,
    /// Due date, `YYYY-MM-DD` (from the `due:` key).
    pub due: Option<String>,
    /// Completion date, `YYYY-MM-DD`.
    pub completed: Option<String>,
    /// Any other `key:value` metadata, in first-seen order.
    pub tags: Vec<(String, String)>,
}

/// The four supported formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Todotxt,
    Markdown,
    Json,
    Csv,
}

impl Format {
    /// Parse a format name. `"auto"` is NOT accepted here — resolve auto to a
    /// concrete format with [`detect`] first.
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "todotxt" | "todo.txt" | "todo" => Ok(Format::Todotxt),
            "markdown" | "md" | "checklist" => Ok(Format::Markdown),
            "json" => Ok(Format::Json),
            "csv" => Ok(Format::Csv),
            other => Err(format!(
                "unknown format '{other}' (expected todotxt, markdown, json, or csv)"
            )),
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Format::Todotxt => "todotxt",
            Format::Markdown => "markdown",
            Format::Json => "json",
            Format::Csv => "csv",
        })
    }
}

/// The fixed CSV / JSON column order.
const COLUMNS: [&str; 9] = [
    "text",
    "done",
    "priority",
    "projects",
    "contexts",
    "created",
    "due",
    "completed",
    "tags",
];

/// Sniff the source format from the raw text. Returns an error for empty input.
pub fn detect(input: &str) -> Result<Format, String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return Err("input is empty — nothing to convert".into());
    }
    let first = trimmed.chars().next().unwrap();
    if first == '[' || first == '{' {
        return Ok(Format::Json);
    }
    // Any Markdown checklist line ("- [ ]", "* [x]", "1. [ ]") ⇒ markdown.
    if input.lines().any(|l| is_markdown_item(l).is_some()) {
        return Ok(Format::Markdown);
    }
    // A header row naming a known column + at least one comma ⇒ csv.
    if let Some(header) = input.lines().find(|l| !l.trim().is_empty()) {
        if header.contains(',') {
            let cols: Vec<String> = header
                .split(',')
                .map(|c| c.trim().trim_matches('"').to_ascii_lowercase())
                .collect();
            if cols
                .iter()
                .any(|c| c == "text" || c == "task" || c == "title")
            {
                return Ok(Format::Csv);
            }
        }
    }
    Ok(Format::Todotxt)
}

/// Public entry point shared by the chat block, CLI, and web page. Resolves
/// `from == "auto"` (any case) by sniffing the source with [`detect`], parses
/// the named `from` / `to` format names, and converts. `pretty` only affects
/// JSON output (indented vs single-line).
pub fn run(input: &str, from: &str, to: &str, pretty: bool) -> Result<String, String> {
    let from = if from.trim().eq_ignore_ascii_case("auto") {
        detect(input)?
    } else {
        Format::parse(from)?
    };
    let to = Format::parse(to)?;
    convert(input, from, to, pretty)
}

/// Top-level conversion entry point used by every surface.
pub fn convert(input: &str, from: Format, to: Format, pretty: bool) -> Result<String, String> {
    let tasks = parse(input, from)?;
    if tasks.is_empty() {
        return Err("no tasks found in the input".into());
    }
    Ok(write(&tasks, to, pretty))
}

/// Parse `input` from `from` into the task model.
pub fn parse(input: &str, from: Format) -> Result<Vec<Task>, String> {
    match from {
        Format::Todotxt => Ok(parse_todotxt(input)),
        Format::Markdown => Ok(parse_markdown(input)),
        Format::Json => parse_json(input),
        Format::Csv => parse_csv(input),
    }
}

/// Serialize the task model into `to`.
pub fn write(tasks: &[Task], to: Format, pretty: bool) -> String {
    match to {
        Format::Todotxt => write_todotxt(tasks),
        Format::Markdown => write_markdown(tasks),
        Format::Json => write_json(tasks, pretty),
        Format::Csv => write_csv(tasks),
    }
}

// ---------------------------------------------------------------------------
// Shared token helpers
// ---------------------------------------------------------------------------

fn is_date(tok: &str) -> bool {
    let b = tok.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..].iter().all(u8::is_ascii_digit)
}

/// Consume `+project`, `@context`, and `key:value` tokens out of a description,
/// filling the task's structured fields and returning the cleaned text.
fn extract_inline_metadata(desc: &str, task: &mut Task) {
    let mut text_tokens: Vec<&str> = Vec::new();
    for tok in desc.split_whitespace() {
        if let Some(p) = tok.strip_prefix('+') {
            if !p.is_empty() {
                task.projects.push(p.to_string());
                continue;
            }
        }
        if let Some(c) = tok.strip_prefix('@') {
            if !c.is_empty() {
                task.contexts.push(c.to_string());
                continue;
            }
        }
        if let Some((key, value)) = split_key_value(tok) {
            match key {
                "due" => task.due = Some(value.to_string()),
                "created" => task.created = Some(value.to_string()),
                "completed" => {
                    task.completed = Some(value.to_string());
                    task.done = true;
                }
                "pri" => task.priority = value.chars().next().filter(|c| c.is_ascii_uppercase()),
                _ => task.tags.push((key.to_string(), value.to_string())),
            }
            continue;
        }
        text_tokens.push(tok);
    }
    task.text = text_tokens.join(" ");
}

/// A `key:value` token per the todo.txt spec: key and value are both non-empty
/// and contain no colon or whitespace. Returns `None` for anything else (e.g. a
/// bare `http://…` URL, whose "key" `http` has an empty following segment only
/// if it has no host — real URLs keep their value and are treated as tags, which
/// we avoid by requiring the value to itself be colon-free).
fn split_key_value(tok: &str) -> Option<(&str, &str)> {
    let (key, value) = tok.split_once(':')?;
    if key.is_empty() || value.is_empty() || value.contains(':') {
        return None;
    }
    if key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        Some((key, value))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// todo.txt
// ---------------------------------------------------------------------------

fn parse_todotxt(input: &str) -> Vec<Task> {
    input
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(parse_todotxt_line)
        .collect()
}

fn parse_todotxt_line(line: &str) -> Task {
    let mut task = Task::default();
    let mut rest = line.trim();

    // Completion marker: a leading lowercase `x` followed by whitespace.
    if let Some(r) = rest.strip_prefix("x ") {
        task.done = true;
        rest = r.trim_start();
    }

    // Optional priority `(A)`.
    if let Some(pri) = leading_priority(rest) {
        task.priority = Some(pri);
        rest = rest[3..].trim_start();
    }

    // Up to two leading dates. For a completed task the first is the completion
    // date and the second is the creation date; otherwise the first is the
    // creation date (spec: priority/prepended dates come before the text).
    let mut dates: Vec<String> = Vec::new();
    let max = if task.done { 2 } else { 1 };
    while dates.len() < max {
        let tok = match rest.split_whitespace().next() {
            Some(t) => t,
            None => break,
        };
        if is_date(tok) {
            dates.push(tok.to_string());
            rest = rest[tok.len()..].trim_start();
        } else {
            break;
        }
    }
    if task.done {
        let mut it = dates.into_iter();
        task.completed = it.next();
        task.created = it.next();
    } else {
        task.created = dates.into_iter().next();
    }

    extract_inline_metadata(rest, &mut task);
    task
}

fn leading_priority(s: &str) -> Option<char> {
    let b = s.as_bytes();
    if b.len() >= 3 && b[0] == b'(' && b[2] == b')' && b[1].is_ascii_uppercase() {
        // Must be followed by whitespace or end of line.
        if b.len() == 3 || b[3] == b' ' {
            return Some(b[1] as char);
        }
    }
    None
}

fn write_todotxt(tasks: &[Task]) -> String {
    tasks
        .iter()
        .map(|t| {
            let mut parts: Vec<String> = Vec::new();
            if t.done {
                parts.push("x".to_string());
            }
            if let Some(p) = t.priority {
                parts.push(format!("({p})"));
            }
            if t.done {
                if let Some(c) = &t.completed {
                    parts.push(c.clone());
                }
            }
            if let Some(c) = &t.created {
                parts.push(c.clone());
            }
            if !t.text.is_empty() {
                parts.push(t.text.clone());
            }
            for p in &t.projects {
                parts.push(format!("+{p}"));
            }
            for c in &t.contexts {
                parts.push(format!("@{c}"));
            }
            if let Some(d) = &t.due {
                parts.push(format!("due:{d}"));
            }
            for (k, v) in &t.tags {
                parts.push(format!("{k}:{v}"));
            }
            parts.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Markdown checklist
// ---------------------------------------------------------------------------

/// If `line` is a Markdown checklist item, return `(done, description)`.
fn is_markdown_item(line: &str) -> Option<(bool, &str)> {
    let t = line.trim_start();
    // Bullet: `- `, `* `, `+ `, or ordered `12. ` / `12) `.
    let after_bullet = if let Some(r) = t.strip_prefix("- ") {
        r
    } else if let Some(r) = t.strip_prefix("* ") {
        r
    } else if let Some(r) = t.strip_prefix("+ ") {
        r
    } else {
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        let after_num = &t[digits.len()..];
        let sep = after_num
            .strip_prefix(". ")
            .or_else(|| after_num.strip_prefix(") "));
        sep?
    };
    let after_bullet = after_bullet.trim_start();
    // Checkbox: `[ ]`, `[x]`, or `[X]` followed by a space (or end).
    let (mark, rest) = if let Some(r) = after_bullet.strip_prefix("[ ]") {
        (false, r)
    } else if let Some(r) = after_bullet.strip_prefix("[x]") {
        (true, r)
    } else if let Some(r) = after_bullet.strip_prefix("[X]") {
        (true, r)
    } else {
        return None;
    };
    Some((mark, rest.trim_start()))
}

fn parse_markdown(input: &str) -> Vec<Task> {
    input
        .lines()
        .filter_map(is_markdown_item)
        .map(|(done, desc)| {
            let mut task = Task {
                done,
                ..Task::default()
            };
            let mut body = desc;
            // Optional leading priority `(A)`.
            if let Some(pri) = leading_priority(body) {
                task.priority = Some(pri);
                body = body[3..].trim_start();
            }
            extract_inline_metadata(body, &mut task);
            // `completed:` in the tags may have set done; keep the checkbox as
            // the authority for the boolean but honor the recorded date.
            task.done = done || task.completed.is_some();
            task
        })
        .collect()
}

fn write_markdown(tasks: &[Task]) -> String {
    tasks
        .iter()
        .map(|t| {
            let mut parts: Vec<String> = Vec::new();
            if let Some(p) = t.priority {
                parts.push(format!("({p})"));
            }
            if !t.text.is_empty() {
                parts.push(t.text.clone());
            }
            for p in &t.projects {
                parts.push(format!("+{p}"));
            }
            for c in &t.contexts {
                parts.push(format!("@{c}"));
            }
            if let Some(d) = &t.due {
                parts.push(format!("due:{d}"));
            }
            // Markdown has no native slot for creation / completion dates, so
            // encode them as key:value tags to keep the round trip lossless.
            if let Some(c) = &t.created {
                parts.push(format!("created:{c}"));
            }
            if let Some(c) = &t.completed {
                parts.push(format!("completed:{c}"));
            }
            for (k, v) in &t.tags {
                parts.push(format!("{k}:{v}"));
            }
            let mark = if t.done { "x" } else { " " };
            format!("- [{mark}] {}", parts.join(" "))
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

fn parse_json(input: &str) -> Result<Vec<Task>, String> {
    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
    let arr = match value {
        Value::Array(a) => a,
        obj @ Value::Object(_) => vec![obj],
        _ => return Err("JSON must be an array of task objects or a single task object".into()),
    };
    arr.into_iter().map(json_to_task).collect()
}

fn json_to_task(v: Value) -> Result<Task, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "each JSON task must be an object".to_string())?;
    let mut task = Task::default();

    task.text = obj
        .get("text")
        .or_else(|| obj.get("task"))
        .or_else(|| obj.get("title"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    task.done = match obj.get("done") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => is_truthy(s),
        _ => false,
    };

    if let Some(p) = obj.get("priority").and_then(Value::as_str) {
        task.priority = p.chars().next().filter(|c| c.is_ascii_uppercase());
    }

    task.projects = string_list(obj.get("projects"));
    task.contexts = string_list(obj.get("contexts"));
    task.created = opt_date_str(obj.get("created"));
    task.due = opt_date_str(obj.get("due"));
    task.completed = opt_date_str(obj.get("completed"));
    if task.completed.is_some() {
        task.done = true;
    }

    if let Some(Value::Object(tags)) = obj.get("tags") {
        for (k, v) in tags {
            if let Some(s) = value_to_plain_string(v) {
                task.tags.push((k.clone(), s));
            }
        }
    }
    Ok(task)
}

fn is_truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "x" | "done" | "completed" | "complete"
    )
}

fn string_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().filter_map(value_to_plain_string).collect(),
        Some(Value::String(s)) => s
            .split_whitespace()
            .map(|t| t.trim_start_matches(['+', '@']).to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn opt_date_str(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn value_to_plain_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn task_to_json(t: &Task) -> Value {
    let mut m = Map::new();
    m.insert("text".into(), Value::String(t.text.clone()));
    m.insert("done".into(), Value::Bool(t.done));
    m.insert(
        "priority".into(),
        t.priority
            .map(|c| Value::String(c.to_string()))
            .unwrap_or(Value::Null),
    );
    m.insert(
        "projects".into(),
        Value::Array(t.projects.iter().cloned().map(Value::String).collect()),
    );
    m.insert(
        "contexts".into(),
        Value::Array(t.contexts.iter().cloned().map(Value::String).collect()),
    );
    m.insert("created".into(), opt_to_value(&t.created));
    m.insert("due".into(), opt_to_value(&t.due));
    m.insert("completed".into(), opt_to_value(&t.completed));
    let mut tags = Map::new();
    for (k, v) in &t.tags {
        tags.insert(k.clone(), Value::String(v.clone()));
    }
    m.insert("tags".into(), Value::Object(tags));
    Value::Object(m)
}

fn opt_to_value(o: &Option<String>) -> Value {
    match o {
        Some(s) => Value::String(s.clone()),
        None => Value::Null,
    }
}

fn write_json(tasks: &[Task], pretty: bool) -> String {
    let arr = Value::Array(tasks.iter().map(task_to_json).collect());
    if pretty {
        serde_json::to_string_pretty(&arr).unwrap()
    } else {
        serde_json::to_string(&arr).unwrap()
    }
}

// ---------------------------------------------------------------------------
// CSV
// ---------------------------------------------------------------------------

fn parse_csv(input: &str) -> Result<Vec<Task>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(input.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("invalid CSV header: {e}"))?
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let col = |names: &[&str]| -> Option<usize> {
        headers.iter().position(|h| names.contains(&h.as_str()))
    };
    let c_text = col(&["text", "task", "title", "description"]);
    let c_done = col(&["done", "completed?", "complete", "status"]);
    let c_pri = col(&["priority", "pri"]);
    let c_proj = col(&["projects", "project"]);
    let c_ctx = col(&["contexts", "context"]);
    let c_created = col(&["created", "creation", "created_date", "start"]);
    let c_due = col(&["due", "due_date"]);
    let c_completed = col(&["completed", "completion", "completed_date", "done_date"]);
    let c_tags = col(&["tags", "metadata", "meta"]);

    if c_text.is_none() {
        return Err("CSV needs a 'text' (or 'task'/'title') column".into());
    }

    let mut tasks = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| format!("invalid CSV row {}: {e}", i + 1))?;
        let get = |idx: Option<usize>| -> String {
            idx.and_then(|j| rec.get(j))
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let mut task = Task {
            text: get(c_text),
            ..Task::default()
        };
        let done_cell = get(c_done);
        task.done = is_truthy(&done_cell);
        let pri = get(c_pri);
        task.priority = pri.chars().next().filter(|c| c.is_ascii_uppercase());
        task.projects = split_multi(&get(c_proj));
        task.contexts = split_multi(&get(c_ctx));
        task.created = opt_nonempty(get(c_created));
        task.due = opt_nonempty(get(c_due));
        task.completed = opt_nonempty(get(c_completed));
        if task.completed.is_some() {
            task.done = true;
        }
        for tok in get(c_tags).split_whitespace() {
            if let Some((k, v)) = split_key_value(tok) {
                task.tags.push((k.to_string(), v.to_string()));
            }
        }
        tasks.push(task);
    }
    Ok(tasks)
}

fn split_multi(s: &str) -> Vec<String> {
    s.split([' ', ',', ';'])
        .map(|t| t.trim().trim_start_matches(['+', '@']).to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn opt_nonempty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn write_csv(tasks: &[Task]) -> String {
    let mut wtr = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    wtr.write_record(COLUMNS).unwrap();
    for t in tasks {
        let tags = t
            .tags
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(" ");
        wtr.write_record([
            t.text.clone(),
            t.done.to_string(),
            t.priority.map(|c| c.to_string()).unwrap_or_default(),
            t.projects.join(" "),
            t.contexts.join(" "),
            t.created.clone().unwrap_or_default(),
            t.due.clone().unwrap_or_default(),
            t.completed.clone().unwrap_or_default(),
            tags,
        ])
        .unwrap();
    }
    String::from_utf8(wtr.into_inner().unwrap())
        .unwrap()
        .trim_end_matches('\n')
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todotxt_to_json_roundtrips_attributes() {
        let input = "(A) 2025-01-10 Write the report +work @office due:2025-01-20 est:2h";
        let out = convert(input, Format::Todotxt, Format::Json, false).unwrap();
        assert_eq!(
            out,
            r#"[{"text":"Write the report","done":false,"priority":"A","projects":["work"],"contexts":["office"],"created":"2025-01-10","due":"2025-01-20","completed":null,"tags":{"est":"2h"}}]"#
        );
    }

    #[test]
    fn completed_todotxt_parses_both_dates() {
        let input = "x (A) 2025-01-15 2025-01-10 Call the bank +finance";
        let tasks = parse(input, Format::Todotxt).unwrap();
        assert_eq!(tasks[0].done, true);
        assert_eq!(tasks[0].completed.as_deref(), Some("2025-01-15"));
        assert_eq!(tasks[0].created.as_deref(), Some("2025-01-10"));
        assert_eq!(tasks[0].priority, Some('A'));
        assert_eq!(tasks[0].text, "Call the bank");
        assert_eq!(tasks[0].projects, vec!["finance".to_string()]);
    }

    #[test]
    fn markdown_checkbox_maps_to_done() {
        let input = "- [x] Ship it\n- [ ] Write tests";
        let tasks = parse(input, Format::Markdown).unwrap();
        assert_eq!(tasks[0].done, true);
        assert_eq!(tasks[0].text, "Ship it");
        assert_eq!(tasks[1].done, false);
        assert_eq!(tasks[1].text, "Write tests");
    }

    #[test]
    fn json_to_todotxt() {
        let input = r#"[{"text":"Buy milk","done":false,"priority":"B","projects":["errands"],"contexts":["store"],"due":"2025-02-01"}]"#;
        let out = convert(input, Format::Json, Format::Todotxt, false).unwrap();
        assert_eq!(out, "(B) Buy milk +errands @store due:2025-02-01");
    }

    #[test]
    fn markdown_to_csv_and_back() {
        let md = "- [ ] (A) Draft spec +proj @home due:2025-03-01";
        let csv = convert(md, Format::Markdown, Format::Csv, false).unwrap();
        let expected_csv = "text,done,priority,projects,contexts,created,due,completed,tags\nDraft spec,false,A,proj,home,,2025-03-01,,";
        assert_eq!(csv, expected_csv);
        // Round trip CSV -> markdown reproduces the item.
        let back = convert(&csv, Format::Csv, Format::Markdown, false).unwrap();
        assert_eq!(back, "- [ ] (A) Draft spec +proj @home due:2025-03-01");
    }

    #[test]
    fn markdown_preserves_dates_via_tags() {
        let md = "- [x] Finish +proj created:2025-01-01 completed:2025-01-05";
        let tasks = parse(md, Format::Markdown).unwrap();
        assert_eq!(tasks[0].created.as_deref(), Some("2025-01-01"));
        assert_eq!(tasks[0].completed.as_deref(), Some("2025-01-05"));
        // And the todo.txt form uses native leading dates.
        let tt = write(&tasks, Format::Todotxt, false);
        assert_eq!(tt, "x 2025-01-05 2025-01-01 Finish +proj");
    }

    #[test]
    fn detect_formats() {
        assert_eq!(detect("- [ ] a").unwrap(), Format::Markdown);
        assert_eq!(detect("[{\"text\":\"a\"}]").unwrap(), Format::Json);
        assert_eq!(detect("text,done\na,true").unwrap(), Format::Csv);
        assert_eq!(detect("(A) plain task +p").unwrap(), Format::Todotxt);
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   \n  ", Format::Todotxt, Format::Json, false).is_err());
        assert!(detect("").is_err());
    }

    #[test]
    fn invalid_json_errors() {
        let err = convert("{not json", Format::Json, Format::Csv, false).unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
    }

    #[test]
    fn unknown_format_name_errors() {
        assert!(Format::parse("yaml").is_err());
    }

    #[test]
    fn csv_without_text_column_errors() {
        let err = convert("a,b\n1,2", Format::Csv, Format::Json, false).unwrap_err();
        assert!(err.contains("text"), "got: {err}");
    }

    #[test]
    fn run_auto_detects_source_format() {
        // Markdown checklist auto-detected, converted to todo.txt.
        let out = run("- [x] Ship it +proj", "auto", "todotxt", false).unwrap();
        assert_eq!(out, "x Ship it +proj");
        // JSON auto-detected, converted to csv.
        let out = run(r#"[{"text":"Buy milk"}]"#, "auto", "csv", false).unwrap();
        assert_eq!(
            out,
            "text,done,priority,projects,contexts,created,due,completed,tags\nBuy milk,false,,,,,,,"
        );
    }

    #[test]
    fn run_rejects_unknown_format_names() {
        assert!(run("a", "auto", "yaml", true).is_err());
        assert!(run("a", "xml", "json", true).is_err());
    }

    #[test]
    fn pretty_json_is_indented() {
        let out = convert("Buy milk", Format::Todotxt, Format::Json, true).unwrap();
        assert!(out.contains("\"text\": \"Buy milk\""), "got: {out}");
        assert!(out.contains('\n'), "got: {out}");
    }
}
