//! gantt-mermaid-generator core — turn a plain task table into Mermaid `gantt`
//! diagram source.
//!
//! The input is one task per line with up to five columns separated by `|`, a
//! tab or a comma:
//!
//! ```text
//! Wireframes | 2026-03-02 | 5d  | done
//! Visual design | after Wireframes | 1w | active
//! Design signed off | after Visual design | | milestone
//! ```
//!
//! `section <Name>` (or `## <Name>`) lines group the tasks that follow them.
//! Task ids are derived from the task name, so dependencies can be written as
//! `after <task name>` instead of forcing the author to invent ids.
//!
//! Nothing here computes dates: Mermaid itself resolves `after`/`until` chains
//! and durations at render time. This crate's job is to validate the table and
//! emit syntactically correct, readable `gantt` source.

use std::collections::HashMap;

/// Largest accepted task table, in bytes.
pub const MAX_INPUT_BYTES: usize = 2_000_000;
/// Largest accepted number of task rows.
pub const MAX_TASKS: usize = 500;
/// Largest accepted number of `section` groups.
pub const MAX_SECTIONS: usize = 100;
/// Longest task title that still gets padded for column alignment.
const MAX_PAD: usize = 40;

/// Accepted `dateFormat` values (also the set the CLI/chat schema advertises).
pub const DATE_FORMATS: [&str; 9] = [
    "YYYY-MM-DD",
    "YYYY/MM/DD",
    "DD-MM-YYYY",
    "MM-DD-YYYY",
    "DD/MM/YYYY",
    "MM/DD/YYYY",
    "YYYY-MM-DD HH:mm",
    "YYYY-MM-DDTHH:mm:ss",
    "X",
];
/// Accepted delimiter names.
pub const DELIMITERS: [&str; 4] = ["auto", "pipe", "comma", "tab"];
/// Accepted `weekend` values (Mermaid 11+ moves the weekend for exclusions).
pub const WEEKENDS: [&str; 2] = ["saturday", "friday"];
/// Task tags Mermaid understands, in the order it wants them.
const TAGS: [&str; 4] = ["done", "active", "crit", "milestone"];
/// Weekday names accepted inside `excludes`.
const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];
/// Duration suffixes Mermaid understands (`m` is minutes, `M` is months).
const DURATION_UNITS: [&str; 8] = ["ms", "s", "m", "h", "d", "w", "M", "y"];
/// Words that cannot be used as a task id (they are Mermaid gantt keywords).
const RESERVED_IDS: [&str; 8] = [
    "after",
    "until",
    "section",
    "gantt",
    "done",
    "active",
    "crit",
    "milestone",
];

/// One parsed task row.
struct Task {
    /// Display title, already stripped of characters Mermaid can't hold.
    name: String,
    /// Unique Mermaid task id.
    id: String,
    /// Raw `start` column (a date, `after …`, or empty for "chain").
    start: String,
    /// Raw duration/end column (`5d`, an end date, `until …`, or empty).
    duration: String,
    /// Tags in Mermaid's canonical order.
    tags: Vec<String>,
    /// Index into the section list, or `None` for tasks before any section.
    section: Option<usize>,
    /// 1-based source line, used in every error message.
    lineno: usize,
}

/// Build Mermaid `gantt` source from a task table.
///
/// * `tasks` — the task table, one task per line (see the module docs).
/// * `title` — optional chart title; empty means no `title` line.
/// * `delimiter` — `auto` | `pipe` | `comma` | `tab`.
/// * `date_format` — one of [`DATE_FORMATS`]; also validates every date typed.
/// * `axis_format` — optional `axisFormat` strftime pattern, e.g. `%b %d`.
/// * `tick_interval` — optional `tickInterval`, e.g. `1week`.
/// * `exclude_weekends` — add `weekends` to the `excludes` line.
/// * `weekend` — which day starts the weekend (Mermaid 11+).
/// * `excludes` — extra excluded dates/weekdays, comma or space separated.
/// * `today_marker` — false emits `todayMarker off`.
/// * `compact` — emit the `displayMode: compact` front matter.
/// * `fence` — wrap the output in a ```` ```mermaid ```` fenced block.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    tasks: &str,
    title: &str,
    delimiter: &str,
    date_format: &str,
    axis_format: &str,
    tick_interval: &str,
    exclude_weekends: bool,
    weekend: &str,
    excludes: &str,
    today_marker: bool,
    compact: bool,
    fence: bool,
) -> Result<String, String> {
    if tasks.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "task table is too large: {} bytes, the maximum is {MAX_INPUT_BYTES}",
            tasks.len()
        ));
    }
    let delimiter = pick(delimiter, "auto", &DELIMITERS, "delimiter")?;
    let date_format = pick(date_format, "YYYY-MM-DD", &DATE_FORMATS, "date_format")?;
    let weekend = pick(weekend, "saturday", &WEEKENDS, "weekend")?;
    let axis_format = one_line(axis_format, "axis_format")?;
    let tick_interval = check_tick_interval(tick_interval)?;
    let title = one_line(title, "title")?;

    let sep = resolve_delimiter(&delimiter, tasks);
    let (parsed, sections) = parse_tasks(tasks, sep)?;
    if parsed.is_empty() {
        return Err(
            "no tasks found: add at least one task line, e.g. `Design | 2026-03-02 | 5d`".into(),
        );
    }

    let by_id: HashMap<&str, usize> = parsed
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for (i, t) in parsed.iter().enumerate() {
        by_name.entry(t.name.to_lowercase()).or_insert(i);
    }

    let exclude_line = build_excludes(excludes, exclude_weekends, &date_format)?;

    let mut lines: Vec<String> = Vec::new();
    if compact {
        lines.push("---".into());
        lines.push("displayMode: compact".into());
        lines.push("---".into());
    }
    lines.push("gantt".into());
    if !title.is_empty() {
        lines.push(format!("    title {title}"));
    }
    lines.push(format!("    dateFormat {date_format}"));
    if !axis_format.is_empty() {
        lines.push(format!("    axisFormat {axis_format}"));
    }
    if !tick_interval.is_empty() {
        lines.push(format!("    tickInterval {tick_interval}"));
    }
    if let Some(ex) = exclude_line {
        lines.push(format!("    excludes {ex}"));
    }
    if weekend != "saturday" {
        lines.push(format!("    weekend {weekend}"));
    }
    if !today_marker {
        lines.push("    todayMarker off".into());
    }

    let pad = parsed
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(MAX_PAD);

    let mut current: Option<usize> = None;
    let mut first = true;
    for (idx, task) in parsed.iter().enumerate() {
        if first || task.section != current {
            if let Some(s) = task.section {
                lines.push(format!("    section {}", sections[s]));
            }
            current = task.section;
        }
        first = false;
        let spec = render_spec(task, idx, &parsed, &by_id, &by_name, &date_format)?;
        let width = task.name.chars().count();
        let padding = " ".repeat(pad.saturating_sub(width));
        lines.push(format!("        {}{} :{}", task.name, padding, spec));
    }

    let body = lines.join("\n");
    Ok(if fence {
        format!("```mermaid\n{body}\n```")
    } else {
        body
    })
}

/// Validate an option against its allowed set, falling back to `default` when blank.
fn pick(value: &str, default: &str, allowed: &[&str], field: &str) -> Result<String, String> {
    let t = value.trim();
    if t.is_empty() {
        return Ok(default.to_string());
    }
    for a in allowed {
        if a.eq_ignore_ascii_case(t) {
            return Ok((*a).to_string());
        }
    }
    Err(format!(
        "unknown {field} '{t}': expected one of {}",
        allowed.join(", ")
    ))
}

/// Trim a single-line option and reject embedded newlines.
fn one_line(value: &str, field: &str) -> Result<String, String> {
    let t = value.trim();
    if t.contains('\n') || t.contains('\r') {
        return Err(format!("{field} must be a single line, but it contains a line break"));
    }
    Ok(t.to_string())
}

/// Validate `tickInterval` against Mermaid's `<count><unit>` grammar.
fn check_tick_interval(value: &str) -> Result<String, String> {
    let t = value.trim();
    if t.is_empty() {
        return Ok(String::new());
    }
    const UNITS: [&str; 7] = [
        "millisecond",
        "second",
        "minute",
        "hour",
        "day",
        "week",
        "month",
    ];
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let unit = &t[digits.len()..];
    let ok = !digits.is_empty() && !digits.starts_with('0') && UNITS.contains(&unit);
    if ok {
        Ok(t.to_string())
    } else {
        Err(format!(
            "invalid tick_interval '{t}': expected a count then a unit, e.g. 1week, 2day or 6hour (units: {})",
            UNITS.join(", ")
        ))
    }
}

/// Merge the `exclude_weekends` toggle and the free-form `excludes` list into
/// one validated `excludes` value, or `None` when nothing is excluded.
fn build_excludes(
    excludes: &str,
    exclude_weekends: bool,
    date_format: &str,
) -> Result<Option<String>, String> {
    let mut out: Vec<String> = Vec::new();
    if exclude_weekends {
        out.push("weekends".into());
    }
    for raw in excludes.split([',', ' ', '\t', '\n', '\r']) {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        let lower = t.to_lowercase();
        let token = if lower == "weekends" || WEEKDAYS.contains(&lower.as_str()) {
            lower
        } else if matches_date(t, date_format) {
            t.to_string()
        } else {
            return Err(format!(
                "invalid excludes entry '{t}': expected `weekends`, a weekday name such as friday, or a date in {date_format}"
            ));
        };
        if !out.contains(&token) {
            out.push(token);
        }
    }
    Ok(if out.is_empty() {
        None
    } else {
        Some(out.join(", "))
    })
}

/// Choose the column separator, sniffing the first task row for `auto`.
fn resolve_delimiter(delimiter: &str, tasks: &str) -> char {
    match delimiter {
        "pipe" => '|',
        "tab" => '\t',
        "comma" => ',',
        _ => {
            for line in tasks.lines() {
                let t = line.trim();
                if t.is_empty() || is_comment(t) || section_name(t).is_some() {
                    continue;
                }
                if t.contains('|') {
                    return '|';
                }
                if t.contains('\t') {
                    return '\t';
                }
                return ',';
            }
            ','
        }
    }
}

/// True for blank-ish lines the parser ignores.
fn is_comment(line: &str) -> bool {
    line.starts_with("%%") || line.starts_with("//") || (line.starts_with('#') && !line.starts_with("##"))
}

/// Extract a section name from `section Name` or `## Name`, if this is one.
fn section_name(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("##") {
        let name = rest.trim_start_matches('#').trim();
        return Some(name.to_string());
    }
    let lower = line.to_lowercase();
    if lower.starts_with("section ") || lower == "section" {
        return Some(line[7..].trim().to_string());
    }
    None
}

/// Parse the table into tasks plus the ordered section names.
fn parse_tasks(tasks: &str, sep: char) -> Result<(Vec<Task>, Vec<String>), String> {
    let mut out: Vec<Task> = Vec::new();
    let mut sections: Vec<String> = Vec::new();
    let mut current: Option<usize> = None;
    let mut used: HashMap<String, usize> = HashMap::new();

    for (i, raw) in tasks.lines().enumerate() {
        let lineno = i + 1;
        let line = raw.trim();
        if line.is_empty() || is_comment(line) {
            continue;
        }
        if let Some(name) = section_name(line) {
            if name.is_empty() {
                return Err(format!("line {lineno}: a section needs a name, e.g. `section Design`"));
            }
            if sections.len() >= MAX_SECTIONS {
                return Err(format!(
                    "too many sections: the maximum is {MAX_SECTIONS}"
                ));
            }
            sections.push(name);
            current = Some(sections.len() - 1);
            continue;
        }
        if out.len() >= MAX_TASKS {
            return Err(format!("too many tasks: the maximum is {MAX_TASKS}"));
        }

        let cols: Vec<&str> = line.split(sep).map(str::trim).collect();
        if cols.len() > 5 {
            return Err(format!(
                "line {lineno} has {} columns but at most 5 are allowed (name, start, duration, tags, id) — if a task name contains a '{sep}', switch the delimiter to pipe",
                cols.len()
            ));
        }
        let name = clean_title(cols[0]);
        if name.is_empty() {
            return Err(format!(
                "line {lineno} has an empty task name: the first column is the task title"
            ));
        }
        let tags = parse_tags(cols.get(3).copied().unwrap_or(""), lineno)?;
        let id = assign_id(cols.get(4).copied().unwrap_or(""), &name, lineno, &mut used)?;

        out.push(Task {
            name,
            id,
            start: cols.get(1).copied().unwrap_or("").to_string(),
            duration: cols.get(2).copied().unwrap_or("").to_string(),
            tags,
            section: current,
            lineno,
        });
    }
    Ok((out, sections))
}

/// Strip the characters a Mermaid task title cannot hold.
fn clean_title(name: &str) -> String {
    name.replace(':', "-").trim().to_string()
}

/// Parse and canonically order a task's tags.
fn parse_tags(raw: &str, lineno: usize) -> Result<Vec<String>, String> {
    let mut found: Vec<String> = Vec::new();
    for part in raw.split([' ', '+', ';', '\t']) {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        let lower = t.to_lowercase();
        if !TAGS.contains(&lower.as_str()) {
            return Err(format!(
                "line {lineno}: unknown tag '{t}': expected any of {}",
                TAGS.join(", ")
            ));
        }
        if !found.contains(&lower) {
            found.push(lower);
        }
    }
    // Mermaid wants the tags in its documented order.
    Ok(TAGS
        .iter()
        .filter(|t| found.iter().any(|f| f == *t))
        .map(|t| (*t).to_string())
        .collect())
}

/// Derive (or validate) a unique Mermaid id for a task.
fn assign_id(
    explicit: &str,
    name: &str,
    lineno: usize,
    used: &mut HashMap<String, usize>,
) -> Result<String, String> {
    let base = if explicit.trim().is_empty() {
        slug_id(name)
    } else {
        let e = explicit.trim().to_string();
        let valid = e
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            && e.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
        if !valid {
            return Err(format!(
                "line {lineno}: invalid task id '{e}': ids start with a letter or underscore and use only letters, digits, '_' and '-'"
            ));
        }
        if RESERVED_IDS.contains(&e.to_lowercase().as_str()) {
            return Err(format!(
                "line {lineno}: '{e}' is a Mermaid gantt keyword and cannot be a task id"
            ));
        }
        if used.contains_key(&e) {
            return Err(format!("line {lineno}: duplicate task id '{e}'"));
        }
        e
    };
    let count = used.entry(base.clone()).or_insert(0);
    *count += 1;
    Ok(if *count == 1 {
        base
    } else {
        format!("{base}_{}", *count)
    })
}

/// Turn a task title into a safe Mermaid identifier.
fn slug_id(name: &str) -> String {
    let mut out = String::new();
    let mut prev_us = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_us = false;
        } else if !prev_us && !out.is_empty() {
            out.push('_');
            prev_us = true;
        }
    }
    let trimmed = out.trim_end_matches('_').to_string();
    if trimmed.is_empty() {
        return "task".into();
    }
    let starts_bad = trimmed.chars().next().is_some_and(|c| c.is_ascii_digit());
    if starts_bad || RESERVED_IDS.contains(&trimmed.as_str()) {
        format!("t_{trimmed}")
    } else {
        trimmed
    }
}

/// Render the `:tags, id, start, duration` spec that follows a task title.
fn render_spec(
    task: &Task,
    idx: usize,
    all: &[Task],
    by_id: &HashMap<&str, usize>,
    by_name: &HashMap<String, usize>,
    date_format: &str,
) -> Result<String, String> {
    let is_milestone = task.tags.iter().any(|t| t == "milestone");

    let start = if task.start.is_empty() {
        if idx == 0 {
            return Err(format!(
                "line {}: task '{}' has no start — give it a date in {date_format}, an `after <task>` reference, or list it after the task it follows",
                task.lineno, task.name
            ));
        }
        format!("after {}", all[idx - 1].id)
    } else if let Some(refs) = keyword_args(&task.start, "after") {
        let mut ids: Vec<String> = Vec::new();
        for r in refs {
            ids.push(resolve_ref(&r, idx, all, by_id, by_name, task, "after")?);
        }
        format!("after {}", ids.join(" "))
    } else if matches_date(&task.start, date_format) {
        task.start.clone()
    } else {
        return Err(format!(
            "line {}: invalid start '{}' for task '{}': expected a date in {date_format}, `after <task>`, or an empty column to follow the previous task",
            task.lineno, task.start, task.name
        ));
    };

    let duration = if task.duration.is_empty() {
        if is_milestone {
            "0d".to_string()
        } else {
            return Err(format!(
                "line {}: task '{}' has no duration — give a length such as 5d or 2w, an end date in {date_format}, or `until <task>`",
                task.lineno, task.name
            ));
        }
    } else if let Some(refs) = keyword_args(&task.duration, "until") {
        if refs.len() != 1 {
            return Err(format!(
                "line {}: `until` takes exactly one task, but got {}",
                task.lineno,
                refs.len()
            ));
        }
        format!(
            "until {}",
            resolve_ref(&refs[0], idx, all, by_id, by_name, task, "until")?
        )
    } else if date_format == "X" && matches_date(&task.duration, date_format) {
        // With `dateFormat X` every bare number is a timestamp, so an end
        // timestamp wins over the "bare number means days" shorthand.
        task.duration.clone()
    } else if let Some(d) = normalize_duration(&task.duration) {
        d
    } else if matches_date(&task.duration, date_format) {
        task.duration.clone()
    } else {
        return Err(format!(
            "line {}: invalid duration '{}' for task '{}': expected a length such as 5d, 2w or 36h, an end date in {date_format}, or `until <task>`",
            task.lineno, task.duration, task.name
        ));
    };

    let mut parts: Vec<String> = task.tags.clone();
    parts.push(task.id.clone());
    parts.push(start);
    parts.push(duration);
    Ok(parts.join(", "))
}

/// Split `after A, B` / `until C` into its referenced task names.
fn keyword_args(value: &str, keyword: &str) -> Option<Vec<String>> {
    let trimmed = value.trim();
    let lower = trimmed.to_lowercase();
    let prefix = format!("{keyword} ");
    if !lower.starts_with(&prefix) {
        return None;
    }
    let rest = &trimmed[prefix.len()..];
    let refs: Vec<String> = rest
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some(refs)
}

/// Resolve an `after`/`until` reference to a task id, by id then by name.
fn resolve_ref(
    reference: &str,
    idx: usize,
    all: &[Task],
    by_id: &HashMap<&str, usize>,
    by_name: &HashMap<String, usize>,
    task: &Task,
    keyword: &str,
) -> Result<String, String> {
    let target = by_id
        .get(reference)
        .or_else(|| by_name.get(&reference.to_lowercase()))
        .copied();
    let Some(target) = target else {
        return Err(format!(
            "line {}: unknown `{keyword}` reference '{reference}' — no task has that name or id",
            task.lineno
        ));
    };
    if target == idx {
        return Err(format!(
            "line {}: task '{}' cannot depend on itself",
            task.lineno, task.name
        ));
    }
    if target > idx {
        return Err(format!(
            "line {}: task '{}' refers to '{reference}' from line {}, which comes later — move the task it depends on above it",
            task.lineno, task.name, all[target].lineno
        ));
    }
    Ok(all[target].id.clone())
}

/// Normalize a duration cell: a bare number means days, a unit suffix is kept.
fn normalize_duration(value: &str) -> Option<String> {
    let t = value.trim();
    if t.is_empty() {
        return None;
    }
    let digits: String = t
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if digits.is_empty() || digits.parse::<f64>().is_err() {
        return None;
    }
    let unit = t[digits.len()..].trim();
    if unit.is_empty() {
        return Some(format!("{digits}d"));
    }
    if DURATION_UNITS.contains(&unit) {
        return Some(format!("{digits}{unit}"));
    }
    None
}

/// Check a value against a Mermaid `dateFormat` pattern (digit tokens + literals).
fn matches_date(value: &str, format: &str) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return false;
    }
    if format == "X" {
        return v.chars().all(|c| c.is_ascii_digit());
    }
    let vb: Vec<char> = v.chars().collect();
    let fb: Vec<char> = format.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    while j < fb.len() {
        let rest: String = fb[j..].iter().collect();
        let width = if rest.starts_with("YYYY") {
            Some((4usize, 4usize))
        } else if rest.starts_with("YY") {
            Some((2, 2))
        } else if ["MM", "DD", "HH", "mm", "ss"]
            .iter()
            .any(|t| rest.starts_with(t))
        {
            Some((2, 2))
        } else {
            None
        };
        match width {
            Some((need, adv)) => {
                if i + need > vb.len() || !vb[i..i + need].iter().all(|c| c.is_ascii_digit()) {
                    return false;
                }
                i += need;
                j += adv;
            }
            None => {
                if i >= vb.len() || vb[i] != fb[j] {
                    return false;
                }
                i += 1;
                j += 1;
            }
        }
    }
    i == vb.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(tasks: &str) -> Result<String, String> {
        generate(
            tasks,
            "",
            "auto",
            "YYYY-MM-DD",
            "",
            "",
            false,
            "saturday",
            "",
            true,
            false,
            false,
        )
    }

    #[test]
    fn simple_table_becomes_gantt_source() {
        let out = gen("Design | 2026-03-02 | 5d\nBuild | after Design | 2w").unwrap();
        assert_eq!(
            out,
            "gantt\n    dateFormat YYYY-MM-DD\n        Design :design, 2026-03-02, 5d\n        Build  :build, after design, 2w"
        );
    }

    #[test]
    fn blank_start_chains_after_the_previous_task() {
        let out = gen("Design | 2026-03-02 | 5d\nBuild | | 3d").unwrap();
        assert!(out.contains("Build  :build, after design, 3d"), "{out}");
    }

    #[test]
    fn bare_number_duration_means_days() {
        let out = gen("Design | 2026-03-02 | 5").unwrap();
        assert!(out.ends_with("Design :design, 2026-03-02, 5d"), "{out}");
    }

    #[test]
    fn end_date_is_accepted_instead_of_a_duration() {
        let out = gen("Design | 2026-03-02 | 2026-03-09").unwrap();
        assert!(out.contains(":design, 2026-03-02, 2026-03-09"), "{out}");
    }

    #[test]
    fn sections_group_the_tasks_that_follow() {
        let out = gen("section Design\nWireframes | 2026-03-02 | 5d\n## Build\nAPI | after Wireframes | 1w").unwrap();
        assert!(out.contains("    section Design\n"), "{out}");
        assert!(out.contains("    section Build\n"), "{out}");
        assert!(out.contains("API        :api, after wireframes, 1w"), "{out}");
    }

    #[test]
    fn tags_are_emitted_in_mermaid_order() {
        let out = gen("Design | 2026-03-02 | 5d | crit done").unwrap();
        assert!(out.contains(":done, crit, design,"), "{out}");
    }

    #[test]
    fn milestone_without_a_duration_gets_zero_days() {
        let out = gen("Design | 2026-03-02 | 5d\nSigned off | after Design | | milestone").unwrap();
        assert!(out.contains(":milestone, signed_off, after design, 0d"), "{out}");
    }

    #[test]
    fn until_resolves_to_a_task_id() {
        let out = gen("Design | 2026-03-02 | 5d\nSupport | 2026-03-02 | until Design").unwrap();
        assert!(out.contains(":support, 2026-03-02, until design"), "{out}");
    }

    #[test]
    fn multiple_dependencies_are_space_joined() {
        let out = gen("A | 2026-03-02 | 1d\nB | 2026-03-02 | 1d\nC | after A, B | 1d").unwrap();
        assert!(out.contains(":c, after a b, 1d"), "{out}");
    }

    #[test]
    fn explicit_ids_are_used_verbatim() {
        let out = gen("Design work | 2026-03-02 | 5d | | des\nBuild | after des | 1d").unwrap();
        assert!(out.contains(":des, 2026-03-02, 5d"), "{out}");
        assert!(out.contains(":build, after des, 1d"), "{out}");
    }

    #[test]
    fn duplicate_names_get_distinct_ids() {
        let out = gen("Review | 2026-03-02 | 1d\nReview | 2026-03-03 | 1d").unwrap();
        assert!(out.contains(":review, 2026-03-02"), "{out}");
        assert!(out.contains(":review_2, 2026-03-03"), "{out}");
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let out = gen("# plan\n\n%% note\n// aside\nDesign | 2026-03-02 | 5d").unwrap();
        assert_eq!(
            out,
            "gantt\n    dateFormat YYYY-MM-DD\n        Design :design, 2026-03-02, 5d"
        );
    }

    #[test]
    fn colons_in_titles_are_replaced() {
        let out = gen("Phase 1: design | 2026-03-02 | 5d").unwrap();
        assert!(out.contains("Phase 1- design :"), "{out}");
    }

    #[test]
    fn csv_and_tab_tables_parse_too() {
        let csv = gen("Design,2026-03-02,5d").unwrap();
        assert!(csv.contains(":design, 2026-03-02, 5d"), "{csv}");
        let tsv = gen("Design\t2026-03-02\t5d").unwrap();
        assert!(tsv.contains(":design, 2026-03-02, 5d"), "{tsv}");
    }

    #[test]
    fn header_options_render_in_order() {
        let out = generate(
            "Design | 2026-03-02 | 5d",
            "Launch plan",
            "pipe",
            "YYYY-MM-DD",
            "%b %d",
            "1week",
            true,
            "friday",
            "2026-03-05, sunday",
            false,
            true,
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "```mermaid\n---\ndisplayMode: compact\n---\ngantt\n    title Launch plan\n    dateFormat YYYY-MM-DD\n    axisFormat %b %d\n    tickInterval 1week\n    excludes weekends, 2026-03-05, sunday\n    weekend friday\n    todayMarker off\n        Design :design, 2026-03-02, 5d\n```"
        );
    }

    #[test]
    fn alternative_date_formats_validate_against_the_chosen_one() {
        let out = generate(
            "Design | 02/03/2026 | 5d",
            "",
            "pipe",
            "DD/MM/YYYY",
            "",
            "",
            false,
            "saturday",
            "",
            true,
            false,
            false,
        )
        .unwrap();
        assert!(out.contains("dateFormat DD/MM/YYYY"), "{out}");
        assert!(out.contains(":design, 02/03/2026, 5d"), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = gen("   \n# only a comment\n").unwrap_err();
        assert!(err.contains("no tasks found"), "{err}");
    }

    #[test]
    fn a_date_in_the_wrong_format_is_rejected() {
        let err = gen("Design | 03/02/2026 | 5d").unwrap_err();
        assert!(err.contains("invalid start"), "{err}");
        assert!(err.contains("YYYY-MM-DD"), "{err}");
    }

    #[test]
    fn a_missing_duration_is_rejected() {
        let err = gen("Design | 2026-03-02 |").unwrap_err();
        assert!(err.contains("has no duration"), "{err}");
    }

    #[test]
    fn the_first_task_needs_a_start() {
        let err = gen("Design | | 5d").unwrap_err();
        assert!(err.contains("has no start"), "{err}");
    }

    #[test]
    fn unknown_tags_are_rejected() {
        let err = gen("Design | 2026-03-02 | 5d | urgent").unwrap_err();
        assert!(err.contains("unknown tag 'urgent'"), "{err}");
        assert!(err.contains("milestone"), "{err}");
    }

    #[test]
    fn unknown_dependencies_are_rejected() {
        let err = gen("Design | 2026-03-02 | 5d\nBuild | after Research | 1d").unwrap_err();
        assert!(err.contains("unknown `after` reference 'Research'"), "{err}");
    }

    #[test]
    fn forward_dependencies_are_rejected_with_a_fix() {
        let err = gen("Build | after Design | 1d\nDesign | 2026-03-02 | 5d").unwrap_err();
        assert!(err.contains("comes later"), "{err}");
    }

    #[test]
    fn self_dependencies_are_rejected() {
        let err = gen("Design | 2026-03-02 | 5d\nBuild | after Build | 1d").unwrap_err();
        assert!(err.contains("cannot depend on itself"), "{err}");
    }

    #[test]
    fn too_many_columns_names_the_delimiter_fix() {
        let err = gen("Design, phase one, 2026-03-02, 5d, done, des").unwrap_err();
        assert!(err.contains("at most 5"), "{err}");
        assert!(err.contains("pipe"), "{err}");
    }

    #[test]
    fn a_bad_tick_interval_lists_the_units() {
        let err = generate(
            "Design | 2026-03-02 | 5d",
            "",
            "pipe",
            "YYYY-MM-DD",
            "",
            "fortnight",
            false,
            "saturday",
            "",
            true,
            false,
            false,
        )
        .unwrap_err();
        assert!(err.contains("invalid tick_interval"), "{err}");
        assert!(err.contains("1week"), "{err}");
    }

    #[test]
    fn a_bad_excludes_entry_is_rejected() {
        let err = generate(
            "Design | 2026-03-02 | 5d",
            "",
            "pipe",
            "YYYY-MM-DD",
            "",
            "",
            false,
            "saturday",
            "holidays",
            true,
            false,
            false,
        )
        .unwrap_err();
        assert!(err.contains("invalid excludes entry 'holidays'"), "{err}");
    }

    #[test]
    fn an_unknown_date_format_lists_the_choices() {
        let err = generate(
            "Design | 2026-03-02 | 5d",
            "",
            "pipe",
            "RFC3339",
            "",
            "",
            false,
            "saturday",
            "",
            true,
            false,
            false,
        )
        .unwrap_err();
        assert!(err.contains("unknown date_format"), "{err}");
        assert!(err.contains("YYYY-MM-DD"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = gen(&big).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn too_many_tasks_is_rejected() {
        let mut s = String::from("Kick off | 2026-03-02 | 1d\n");
        for i in 0..MAX_TASKS {
            s.push_str(&format!("Task {i} | | 1d\n"));
        }
        let err = gen(&s).unwrap_err();
        assert!(err.contains("too many tasks"), "{err}");
    }

    #[test]
    fn reserved_ids_are_rejected() {
        let err = gen("Design | 2026-03-02 | 5d | | after").unwrap_err();
        assert!(err.contains("keyword"), "{err}");
    }

    #[test]
    fn unix_timestamps_work_with_the_x_format() {
        let out = generate(
            "Design | 1772409600 | 5d",
            "",
            "pipe",
            "X",
            "",
            "",
            false,
            "saturday",
            "",
            true,
            false,
            false,
        )
        .unwrap();
        assert!(out.contains("dateFormat X"), "{out}");
        assert!(out.contains(":design, 1772409600, 5d"), "{out}");
    }

    #[test]
    fn with_the_x_format_a_bare_number_end_stays_a_timestamp() {
        let out = generate(
            "Design | 1772409600 | 1772841600",
            "",
            "pipe",
            "X",
            "",
            "",
            false,
            "saturday",
            "",
            true,
            false,
            false,
        )
        .unwrap();
        assert!(out.contains(":design, 1772409600, 1772841600"), "{out}");
    }
}
