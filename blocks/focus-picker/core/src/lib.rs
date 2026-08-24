//! focus-picker core — pure compute, shared by the chat skill block and the web
//! page. No deps, no I/O, no clock: the caller resolves "today" and passes it in
//! as a day number, so every surface is deterministic and testable.
//!
//! Given a list of tasks (one per line, with optional `!p1` / `due:` / `est:`
//! annotations or `|`/tab-delimited columns), it scores every task under one of
//! five published methods and returns the single task to work on next plus a
//! one-sentence justification.

pub const MAX_TASKS: usize = 500;

/// Hours in one "day" of effort (`est:2d` = two working days).
const HOURS_PER_DAY: f64 = 8.0;

/// Due dates further out than this contribute no urgency.
const URGENCY_HORIZON_DAYS: f64 = 14.0;

/// Urgency assumed for a task that carries no due date at all.
const NO_DUE_URGENCY: f64 = 0.30;

/// A due date this many days out (or nearer) counts as "urgent" for Eisenhower.
const EISENHOWER_URGENT_DAYS: i64 = 2;

/// Priorities at or above this level (numerically at or below) count as
/// "important" for Eisenhower.
const EISENHOWER_IMPORTANT_MAX_P: u8 = 2;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Everything `pick` needs. `today_days` is days since 1970-01-01 — get it from
/// [`resolve_today`].
pub struct Options<'a> {
    pub tasks: &'a str,
    pub method: &'a str,
    pub today_days: i64,
    pub default_priority: &'a str,
    pub default_effort: f64,
    pub overdue_boost: bool,
    pub format: &'a str,
    pub show_ranking: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Method {
    Balanced,
    Deadline,
    Wsjf,
    QuickWins,
    Eisenhower,
}

impl Method {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "balanced" => Ok(Method::Balanced),
            "deadline" => Ok(Method::Deadline),
            "wsjf" => Ok(Method::Wsjf),
            "quick-wins" | "quick_wins" | "quickwins" => Ok(Method::QuickWins),
            "eisenhower" => Ok(Method::Eisenhower),
            other => Err(format!(
                "unknown method {other:?} — expected balanced, deadline, wsjf, quick-wins or eisenhower"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Method::Balanced => "balanced",
            Method::Deadline => "deadline",
            Method::Wsjf => "wsjf",
            Method::QuickWins => "quick-wins",
            Method::Eisenhower => "eisenhower",
        }
    }

    /// The formula, printed with every result so the ranking is auditable.
    fn formula(self) -> &'static str {
        match self {
            Method::Balanced => "0.45 x priority + 0.35 x urgency + 0.20 x effort-ease, scaled to 100",
            Method::Deadline => "0.80 x urgency + 0.20 x priority, scaled to 100",
            Method::Wsjf => "(10 x priority + 10 x urgency) / effort hours",
            Method::QuickWins => "0.35 x priority + 0.25 x urgency + 0.40 x effort-ease, scaled to 100",
            Method::Eisenhower => {
                "quadrant base (Do first 75 / Schedule 50 / Delegate 25 / Drop 0) + a quarter of the balanced score"
            }
        }
    }

    fn lead(self) -> &'static str {
        match self {
            Method::Balanced => "highest balanced score",
            Method::Deadline => "highest deadline score",
            Method::Wsjf => "highest WSJF",
            Method::QuickWins => "highest quick-wins score",
            Method::Eisenhower => "highest Eisenhower score",
        }
    }

    fn decimals(self) -> usize {
        if self == Method::Wsjf {
            2
        } else {
            1
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quadrant {
    DoFirst,
    Schedule,
    Delegate,
    Drop,
}

impl Quadrant {
    fn label(self) -> &'static str {
        match self {
            Quadrant::DoFirst => "Do first",
            Quadrant::Schedule => "Schedule",
            Quadrant::Delegate => "Delegate",
            Quadrant::Drop => "Drop",
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Quadrant::DoFirst => "urgent + important",
            Quadrant::Schedule => "important, not urgent",
            Quadrant::Delegate => "urgent, not important",
            Quadrant::Drop => "neither urgent nor important",
        }
    }

    fn base(self) -> f64 {
        match self {
            Quadrant::DoFirst => 75.0,
            Quadrant::Schedule => 50.0,
            Quadrant::Delegate => 25.0,
            Quadrant::Drop => 0.0,
        }
    }
}

struct Task {
    index: usize,
    line_no: usize,
    title: String,
    priority: u8,
    due: Option<i64>,
    hours: f64,
    score: f64,
    quadrant: Quadrant,
}

impl Task {
    fn days_until(&self, today: i64) -> Option<i64> {
        self.due.map(|d| d - today)
    }

    fn is_overdue(&self, today: i64) -> bool {
        matches!(self.days_until(today), Some(d) if d < 0)
    }
}

// ---------------------------------------------------------------------------
// Calendar helpers (Howard Hinnant's civil-date algorithms — no deps)
// ---------------------------------------------------------------------------

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m as i64) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + (d as i64) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

/// 0 = Sunday … 6 = Saturday. 1970-01-01 was a Thursday.
fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

fn iso(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Parse a strict `YYYY-MM-DD` date into a day number.
fn parse_iso(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return None;
    }
    let y: i64 = s[0..4].parse().ok()?;
    let m: u32 = s[5..7].parse().ok()?;
    let d: u32 = s[8..10].parse().ok()?;
    if m == 0 || m > 12 || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Resolve the `today` parameter: an explicit ISO date, or the caller's clock
/// (`now_unix_secs`) when it is blank.
pub fn resolve_today(today: &str, now_unix_secs: f64) -> Result<i64, String> {
    let t = today.trim();
    if t.is_empty() {
        return Ok((now_unix_secs / 86_400.0).floor() as i64);
    }
    parse_iso(t).ok_or_else(|| {
        format!("today must be an ISO date (YYYY-MM-DD), got {t:?} — leave it blank to use the current date")
    })
}

const WEEKDAY_NAMES: [(&str, i64); 14] = [
    ("sunday", 0),
    ("sun", 0),
    ("monday", 1),
    ("mon", 1),
    ("tuesday", 2),
    ("tue", 2),
    ("wednesday", 3),
    ("wed", 3),
    ("thursday", 4),
    ("thu", 4),
    ("friday", 5),
    ("fri", 5),
    ("saturday", 6),
    ("sat", 6),
];

/// Parse a due-date value: ISO date, `today`/`tomorrow`/`yesterday`, `eod`/`eow`,
/// a weekday name (the next occurrence, today included), or a `+Nd`/`+Nw` offset.
fn parse_due(value: &str, today: i64) -> Option<i64> {
    let v = value.trim().trim_matches(',').to_ascii_lowercase();
    if v.is_empty() {
        return None;
    }
    if let Some(d) = parse_iso(&v) {
        return Some(d);
    }
    match v.as_str() {
        "today" | "eod" | "now" => return Some(today),
        "tomorrow" => return Some(today + 1),
        "yesterday" => return Some(today - 1),
        "eow" => {
            // The upcoming Friday (today if today is a Friday).
            let delta = (5 - weekday(today)).rem_euclid(7);
            return Some(today + delta);
        }
        _ => {}
    }
    if let Some((_, wd)) = WEEKDAY_NAMES.iter().find(|(n, _)| *n == v) {
        let delta = (wd - weekday(today)).rem_euclid(7);
        return Some(today + delta);
    }
    // +Nd / +Nw / +N (days)
    if let Some(rest) = v.strip_prefix('+') {
        let (digits, unit) = split_number(rest);
        let n: i64 = digits.parse().ok()?;
        return match unit {
            "" | "d" | "day" | "days" => Some(today + n),
            "w" | "week" | "weeks" => Some(today + n * 7),
            _ => None,
        };
    }
    None
}

/// Split a leading numeric run from its trailing unit.
fn split_number(s: &str) -> (&str, &str) {
    let cut = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    (&s[..cut], &s[cut..])
}

/// Parse an effort value into hours: `90m`, `1.5h`, `2d`, or a plain number of hours.
fn parse_effort(value: &str) -> Option<f64> {
    let v = value.trim().trim_matches(',').to_ascii_lowercase();
    if v.is_empty() {
        return None;
    }
    let (digits, unit) = split_number(&v);
    let n: f64 = digits.parse().ok()?;
    if !n.is_finite() || n <= 0.0 {
        return None;
    }
    match unit {
        "" | "h" | "hr" | "hrs" | "hour" | "hours" => Some(n),
        "m" | "min" | "mins" | "minute" | "minutes" => Some(n / 60.0),
        "d" | "day" | "days" => Some(n * HOURS_PER_DAY),
        "w" | "week" | "weeks" => Some(n * HOURS_PER_DAY * 5.0),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Priority
// ---------------------------------------------------------------------------

const HIGH_WORDS: &[&str] = &[
    "urgent",
    "asap",
    "critical",
    "emergency",
    "blocker",
    "blocking",
    "important",
    "overdue",
    "deadline",
    "must",
];

const LOW_WORDS: &[&str] = &[
    "someday",
    "eventually",
    "whenever",
    "maybe",
    "optional",
    "backlog",
    "later",
    "nice-to-have",
];

fn parse_priority_token(tok: &str) -> Option<u8> {
    let t = tok.trim().trim_start_matches('!').to_ascii_lowercase();
    let t = t.trim_matches(|c: char| c == ',' || c == ';');
    if let Some(digit) = t.strip_prefix('p') {
        if digit.len() == 1 {
            if let Some(n) = digit.chars().next().and_then(|c| c.to_digit(10)) {
                if n <= 4 {
                    return Some(n as u8);
                }
            }
        }
    }
    // Bare integer 0-4 (delimited columns): "3" means p3.
    if t.len() == 1 {
        if let Some(n) = t.chars().next().and_then(|c| c.to_digit(10)) {
            if n <= 4 {
                return Some(n as u8);
            }
        }
    }
    match t {
        "high" => Some(1),
        "medium" | "normal" | "med" => Some(2),
        "low" => Some(4),
        _ => None,
    }
}

/// todo.txt-style `(A)`-`(Z)` marker: A -> p1, B -> p2, C -> p3, D and beyond -> p4.
fn parse_letter_priority(tok: &str) -> Option<u8> {
    let b = tok.as_bytes();
    if b.len() == 3 && b[0] == b'(' && b[2] == b')' && b[1].is_ascii_alphabetic() {
        let idx = b[1].to_ascii_uppercase() - b'A';
        return Some(match idx {
            0 => 1,
            1 => 2,
            2 => 3,
            _ => 4,
        });
    }
    None
}

fn priority_weight(p: u8) -> f64 {
    match p {
        0 => 1.0,
        1 => 0.8,
        2 => 0.6,
        3 => 0.4,
        _ => 0.2,
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whole-word-ish containment: the needle must start at a word boundary but may
/// be followed by more letters, so `urgent` also matches `urgently`.
fn contains_stem(haystack_lower: &str, needle: &str) -> bool {
    let hb = haystack_lower.as_bytes();
    let nb = needle.as_bytes();
    if nb.is_empty() || nb.len() > hb.len() {
        return false;
    }
    (0..=hb.len() - nb.len())
        .any(|i| &hb[i..i + nb.len()] == nb && (i == 0 || !is_word_char(hb[i - 1] as char)))
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Strip list chrome (`-`, `*`, `1.`, `- [ ]`). Returns `None` for a line that
/// is an already-completed checklist item.
fn strip_chrome(line: &str) -> Option<String> {
    let mut s = line.trim().to_string();
    loop {
        let t = s.trim_start();
        if let Some(rest) = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| t.strip_prefix("+ "))
            .or_else(|| t.strip_prefix("• "))
        {
            s = rest.to_string();
            continue;
        }
        // "1." / "1)" numbering
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let after = &t[digits.len()..];
            if let Some(rest) = after
                .strip_prefix(". ")
                .or_else(|| after.strip_prefix(") "))
            {
                s = rest.to_string();
                continue;
            }
        }
        break;
    }
    let t = s.trim_start();
    if let Some(rest) = t.strip_prefix('[') {
        if rest.len() >= 2 && rest.as_bytes()[1] == b']' {
            let mark = rest.as_bytes()[0];
            if mark == b'x' || mark == b'X' {
                return None; // already done
            }
            if mark == b' ' {
                s = rest[2..].trim_start().to_string();
            }
        }
    }
    Some(s.trim().to_string())
}

struct Parsed {
    priority: Option<u8>,
    due: Option<i64>,
    hours: Option<f64>,
    title: String,
}

/// Pull the inline annotations out of one text segment, returning the leftover
/// words as the (cleaned) title.
fn parse_segment(seg: &str, today: i64, line_no: usize, keep_text: bool) -> Result<Parsed, String> {
    let mut out = Parsed {
        priority: None,
        due: None,
        hours: None,
        title: String::new(),
    };
    let mut kept: Vec<&str> = Vec::new();

    for (i, tok) in seg.split_whitespace().enumerate() {
        let lower = tok.to_ascii_lowercase();

        if let Some(v) = lower
            .strip_prefix("due:")
            .or_else(|| lower.strip_prefix("by:"))
            .or_else(|| lower.strip_prefix("deadline:"))
        {
            match parse_due(v, today) {
                Some(d) => {
                    out.due.get_or_insert(d);
                    continue;
                }
                None => {
                    return Err(format!(
                        "line {line_no}: unrecognised due date {v:?} — expected YYYY-MM-DD, today, tomorrow, yesterday, eod, eow, a weekday name, or +Nd/+Nw"
                    ))
                }
            }
        }

        if let Some(v) = lower
            .strip_prefix("est:")
            .or_else(|| lower.strip_prefix("effort:"))
            .or_else(|| lower.strip_prefix("takes:"))
            .or_else(|| lower.strip_prefix('~'))
        {
            match parse_effort(v) {
                Some(h) => {
                    out.hours.get_or_insert(h);
                    continue;
                }
                None => {
                    return Err(format!(
                        "line {line_no}: unrecognised effort {v:?} — expected e.g. 90m, 1.5h, 2d, or a plain number of hours"
                    ))
                }
            }
        }

        if let Some(p) = parse_letter_priority(tok) {
            if i == 0 {
                out.priority.get_or_insert(p);
                continue;
            }
        }

        // Explicit p0-p4, with or without a leading "!".
        let is_p_tag = lower.starts_with('p') || lower.starts_with("!p");
        if is_p_tag {
            if let Some(p) = parse_priority_token(&lower) {
                out.priority.get_or_insert(p);
                continue;
            }
        }
        if let Some(rest) = lower.strip_prefix('!') {
            if let Some(p) = parse_priority_token(rest) {
                out.priority.get_or_insert(p);
                continue;
            }
        }

        // A bare ISO date anywhere in the line is a due date.
        if out.due.is_none() {
            let bare = lower.trim_matches(|c: char| c == ',' || c == ';' || c == '(' || c == ')');
            if let Some(d) = parse_iso(bare) {
                out.due = Some(d);
                continue;
            }
        }

        kept.push(tok);
    }

    if keep_text {
        out.title = kept.join(" ");
    } else {
        // A delimited column: whatever is left must be a bare priority/due/effort.
        let leftover = kept.join(" ");
        let bare = leftover.trim();
        if !bare.is_empty() {
            if out.priority.is_none() {
                if let Some(p) = parse_priority_token(bare) {
                    out.priority = Some(p);
                    return Ok(out);
                }
            }
            if out.due.is_none() {
                if let Some(d) = parse_due(bare, today) {
                    out.due = Some(d);
                    return Ok(out);
                }
            }
            if out.hours.is_none() {
                if let Some(h) = parse_effort(bare) {
                    out.hours = Some(h);
                    return Ok(out);
                }
            }
            return Err(format!(
                "line {line_no}: column {bare:?} is not a priority (p0-p4), a due date (YYYY-MM-DD, today, tomorrow, a weekday, +Nd) or an effort (90m, 1.5h, 2d)"
            ));
        }
    }
    Ok(out)
}

fn parse_tasks(
    text: &str,
    today: i64,
    default_priority: u8,
    default_effort: f64,
) -> Result<Vec<Task>, String> {
    let mut tasks = Vec::new();

    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(body) = strip_chrome(trimmed) else {
            continue; // completed checklist item
        };
        if body.is_empty() {
            continue;
        }

        // "|" or tab splits a row into task + hint columns.
        let sep = if body.contains('|') {
            Some('|')
        } else if body.contains('\t') {
            Some('\t')
        } else {
            None
        };
        let mut segments: Vec<&str> = match sep {
            Some(c) => body
                .split(c)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect(),
            None => vec![body.as_str()],
        };
        if segments.is_empty() {
            continue;
        }

        let head = parse_segment(segments.remove(0), today, line_no, true)?;
        let mut priority = head.priority;
        let mut due = head.due;
        let mut hours = head.hours;
        let mut title = head.title;

        for seg in segments {
            let col = parse_segment(seg, today, line_no, false)?;
            if priority.is_none() {
                priority = col.priority;
            }
            if due.is_none() {
                due = col.due;
            }
            if hours.is_none() {
                hours = col.hours;
            }
        }

        if title.trim().is_empty() {
            continue;
        }
        title = title
            .trim()
            .trim_end_matches([':', '-', '–'])
            .trim()
            .to_string();
        if title.is_empty() {
            continue;
        }

        // Fall back to urgency keywords, then to the caller's default.
        let priority = priority.unwrap_or_else(|| {
            let lower = title.to_ascii_lowercase();
            if HIGH_WORDS.iter().any(|w| contains_stem(&lower, w)) {
                1
            } else if LOW_WORDS.iter().any(|w| contains_stem(&lower, w)) {
                4
            } else {
                default_priority
            }
        });

        tasks.push(Task {
            index: tasks.len(),
            line_no,
            title,
            priority,
            due,
            hours: hours.unwrap_or(default_effort),
            score: 0.0,
            quadrant: Quadrant::Drop,
        });

        if tasks.len() > MAX_TASKS {
            return Err(format!(
                "too many tasks (over {MAX_TASKS}) — trim the list and re-run"
            ));
        }
    }

    Ok(tasks)
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

fn urgency(task: &Task, today: i64) -> f64 {
    match task.days_until(today) {
        None => NO_DUE_URGENCY,
        Some(d) if d < 0 => 1.0,
        Some(d) => (1.0 - d as f64 / URGENCY_HORIZON_DAYS).max(0.05),
    }
}

/// Smaller jobs are easier to finish; 2 h maps to 0.5, 8 h to 0.2.
fn ease(hours: f64) -> f64 {
    1.0 / (1.0 + hours.max(0.05) / 2.0)
}

fn quadrant_of(task: &Task, today: i64) -> Quadrant {
    let urgent = matches!(task.days_until(today), Some(d) if d <= EISENHOWER_URGENT_DAYS);
    let important = task.priority <= EISENHOWER_IMPORTANT_MAX_P;
    match (urgent, important) {
        (true, true) => Quadrant::DoFirst,
        (false, true) => Quadrant::Schedule,
        (true, false) => Quadrant::Delegate,
        (false, false) => Quadrant::Drop,
    }
}

fn score(task: &Task, method: Method, today: i64) -> f64 {
    let p = priority_weight(task.priority);
    let u = urgency(task, today);
    let e = ease(task.hours);
    let balanced = 100.0 * (0.45 * p + 0.35 * u + 0.20 * e);
    match method {
        Method::Balanced => balanced,
        Method::QuickWins => 100.0 * (0.35 * p + 0.25 * u + 0.40 * e),
        Method::Deadline => 100.0 * (0.80 * u + 0.20 * p),
        Method::Wsjf => (10.0 * p + 10.0 * u) / task.hours.max(0.25),
        Method::Eisenhower => task.quadrant.base() + balanced / 4.0,
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn trim_num(x: f64) -> String {
    let s = format!("{x:.2}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

fn fmt_hours(h: f64) -> String {
    format!("{} h", trim_num(h))
}

fn fmt_score(x: f64, method: Method) -> String {
    format!("{:.*}", method.decimals(), x)
}

/// "today" / "tomorrow" / "in 3 days" / "3 days overdue" / "no due date".
fn fmt_when(task: &Task, today: i64) -> String {
    match task.days_until(today) {
        None => "no due date".to_string(),
        Some(0) => "today".to_string(),
        Some(1) => "tomorrow".to_string(),
        Some(-1) => "1 day overdue".to_string(),
        Some(d) if d < 0 => format!("{} days overdue", -d),
        Some(d) => format!("in {d} days"),
    }
}

/// The due column: "due 2026-08-24 (in 3 days)" or "no due date".
fn fmt_due(task: &Task, today: i64) -> String {
    match task.due {
        None => "no due date".to_string(),
        Some(d) => format!("due {} ({})", iso(d), fmt_when(task, today)),
    }
}

fn fmt_facts(task: &Task, today: i64, method: Method) -> String {
    let mut s = format!(
        "p{} · {} · {}",
        task.priority,
        fmt_due(task, today),
        fmt_hours(task.hours)
    );
    if method == Method::Eisenhower {
        s.push_str(" · ");
        s.push_str(task.quadrant.label());
    }
    s
}

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

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Score every task and return the one to do next, with a justification.
pub fn pick(o: &Options) -> Result<String, String> {
    let method = Method::parse(o.method)?;

    let format = o.format.trim().to_ascii_lowercase();
    let format = if format.is_empty() {
        "text".to_string()
    } else {
        format
    };
    if !matches!(format.as_str(), "text" | "markdown" | "json") {
        return Err(format!(
            "unknown format {format:?} — expected text, markdown or json"
        ));
    }

    let dp = o.default_priority.trim().to_ascii_lowercase();
    let default_priority = if dp.is_empty() {
        3
    } else {
        parse_priority_token(&dp).ok_or_else(|| {
            format!("unknown default_priority {dp:?} — expected p0, p1, p2, p3 or p4")
        })?
    };

    let default_effort = if o.default_effort <= 0.0 {
        2.0
    } else {
        o.default_effort
    };
    if !default_effort.is_finite() || default_effort > 10_000.0 {
        return Err(format!(
            "default_effort must be between 0 and 10000 hours, got {}",
            trim_num(o.default_effort)
        ));
    }

    let today = o.today_days;
    let mut tasks = parse_tasks(o.tasks, today, default_priority, default_effort)?;
    if tasks.is_empty() {
        return Err(
            "no tasks found — paste one task per line, e.g. \"Fix the login redirect !p1 due:tomorrow est:90m\""
                .to_string(),
        );
    }

    for t in tasks.iter_mut() {
        t.quadrant = quadrant_of(t, today);
    }
    for i in 0..tasks.len() {
        let s = score(&tasks[i], method, today);
        tasks[i].score = s;
    }

    // Deterministic ordering: overdue pin (optional) → score → soonest due →
    // strongest priority → smallest job → original line order.
    let boost = o.overdue_boost;
    tasks.sort_by(|a, b| {
        let pin = |t: &Task| (boost && t.is_overdue(today)) as u8;
        pin(b)
            .cmp(&pin(a))
            .then(
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then_with(|| match (a.due, b.due) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then(a.priority.cmp(&b.priority))
            .then(
                a.hours
                    .partial_cmp(&b.hours)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.index.cmp(&b.index))
    });

    let count = tasks.len();
    let total_hours: f64 = tasks.iter().map(|t| t.hours).sum();
    let overdue_count = tasks.iter().filter(|t| t.is_overdue(today)).count();
    let top = &tasks[0];

    // The justification: the three facts that decided it, then the method.
    let mut reason = format!(
        "p{} priority, {}, ~{} effort — {} ({}) of {} task{}.",
        top.priority,
        fmt_due(top, today),
        fmt_hours(top.hours),
        method.lead(),
        fmt_score(top.score, method),
        count,
        if count == 1 { "" } else { "s" }
    );
    if method == Method::Eisenhower {
        reason = format!(
            "p{} priority, {}, ~{} effort — Eisenhower quadrant \"{}\" ({}), highest score ({}) of {} task{}.",
            top.priority,
            fmt_due(top, today),
            fmt_hours(top.hours),
            top.quadrant.label(),
            top.quadrant.detail(),
            fmt_score(top.score, method),
            count,
            if count == 1 { "" } else { "s" }
        );
    }
    if boost && top.is_overdue(today) && overdue_count < count {
        reason.push_str(&format!(
            " Pinned above everything not overdue ({}).",
            fmt_when(top, today)
        ));
    }

    let summary = format!(
        "{} task{} · {} total effort · {} overdue · method {} ({})",
        count,
        if count == 1 { "" } else { "s" },
        fmt_hours(total_hours),
        overdue_count,
        method.name(),
        method.formula()
    );

    Ok(match format.as_str() {
        "json" => render_json(
            &tasks,
            method,
            today,
            &reason,
            &summary,
            o,
            total_hours,
            overdue_count,
        ),
        "markdown" => render_markdown(&tasks, method, today, &reason, &summary, o.show_ranking),
        _ => render_text(&tasks, method, today, &reason, &summary, o.show_ranking),
    })
}

fn render_text(
    tasks: &[Task],
    method: Method,
    today: i64,
    reason: &str,
    summary: &str,
    show_ranking: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Focus on: {}\n", tasks[0].title));
    out.push_str(&format!("Why: {reason}\n"));

    if show_ranking && tasks.len() > 1 {
        out.push_str("\nFull ranking\n");
        for (i, t) in tasks.iter().enumerate() {
            out.push_str(&format!(
                "{:>3}. {:>6}  {} — {}\n",
                i + 1,
                fmt_score(t.score, method),
                t.title,
                fmt_facts(t, today, method)
            ));
        }
    }
    out.push_str(&format!("\n{summary}\n"));
    out
}

fn render_markdown(
    tasks: &[Task],
    method: Method,
    today: i64,
    reason: &str,
    summary: &str,
    show_ranking: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("**Focus on:** {}\n\n", tasks[0].title));
    out.push_str(&format!("{reason}\n"));

    if show_ranking && tasks.len() > 1 {
        let quad = if method == Method::Eisenhower {
            " Quadrant |"
        } else {
            ""
        };
        let quad_sep = if method == Method::Eisenhower {
            " --- |"
        } else {
            ""
        };
        out.push_str(&format!(
            "\n| # | Score | Task | Priority | Due | Effort |{quad}\n"
        ));
        out.push_str(&format!(
            "| ---: | ---: | --- | --- | --- | ---: |{quad_sep}\n"
        ));
        for (i, t) in tasks.iter().enumerate() {
            let quad_cell = if method == Method::Eisenhower {
                format!(" {} |", t.quadrant.label())
            } else {
                String::new()
            };
            out.push_str(&format!(
                "| {} | {} | {} | p{} | {} | {} |{}\n",
                i + 1,
                fmt_score(t.score, method),
                t.title.replace('|', "\\|"),
                t.priority,
                fmt_due(t, today),
                fmt_hours(t.hours),
                quad_cell
            ));
        }
    }
    out.push_str(&format!("\n_{summary}_\n"));
    out
}

#[allow(clippy::too_many_arguments)]
fn render_json(
    tasks: &[Task],
    method: Method,
    today: i64,
    reason: &str,
    summary: &str,
    o: &Options,
    total_hours: f64,
    overdue_count: usize,
) -> String {
    let entry = |i: usize, t: &Task| -> String {
        let due = match t.due {
            Some(d) => format!("\"{}\"", iso(d)),
            None => "null".to_string(),
        };
        let days = match t.days_until(today) {
            Some(d) => d.to_string(),
            None => "null".to_string(),
        };
        format!(
            "{{\"rank\":{},\"task\":\"{}\",\"score\":{},\"priority\":\"p{}\",\"due\":{},\"days_until\":{},\"effort_hours\":{},\"overdue\":{},\"quadrant\":\"{}\",\"line\":{}}}",
            i + 1,
            json_escape(&t.title),
            fmt_score(t.score, method),
            t.priority,
            due,
            days,
            trim_num(t.hours),
            t.is_overdue(today),
            t.quadrant.label(),
            t.line_no
        )
    };

    let mut out = String::from("{\n");
    out.push_str(&format!("  \"method\": \"{}\",\n", method.name()));
    out.push_str(&format!(
        "  \"formula\": \"{}\",\n",
        json_escape(method.formula())
    ));
    out.push_str(&format!("  \"today\": \"{}\",\n", iso(today)));
    out.push_str(&format!("  \"task_count\": {},\n", tasks.len()));
    out.push_str(&format!(
        "  \"total_effort_hours\": {},\n",
        trim_num(total_hours)
    ));
    out.push_str(&format!("  \"overdue_count\": {overdue_count},\n"));
    out.push_str(&format!("  \"summary\": \"{}\",\n", json_escape(summary)));
    out.push_str(&format!("  \"reason\": \"{}\",\n", json_escape(reason)));
    out.push_str(&format!("  \"pick\": {},\n", entry(0, &tasks[0])));
    if o.show_ranking {
        out.push_str("  \"ranked\": [\n");
        let rows: Vec<String> = tasks
            .iter()
            .enumerate()
            .map(|(i, t)| format!("    {}", entry(i, t)))
            .collect();
        out.push_str(&rows.join(",\n"));
        out.push_str("\n  ]\n");
    } else {
        out.push_str("  \"ranked\": null\n");
    }
    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-08-21 (a Friday).
    const TODAY: i64 = 20_686;

    fn opts<'a>(tasks: &'a str, method: &'a str) -> Options<'a> {
        Options {
            tasks,
            method,
            today_days: TODAY,
            default_priority: "p3",
            default_effort: 2.0,
            overdue_boost: true,
            format: "text",
            show_ranking: true,
        }
    }

    #[test]
    fn today_constant_is_2026_08_21_a_friday() {
        assert_eq!(iso(TODAY), "2026-08-21");
        assert_eq!(weekday(TODAY), 5);
    }

    #[test]
    fn happy_path_picks_the_urgent_high_priority_task() {
        let out = pick(&opts(
            "Fix the login redirect !p1 due:today est:90m\n\
             Rewrite the onboarding docs !p3 due:+10d est:6h\n\
             Reply to the vendor !p2 due:+1d est:15m",
            "balanced",
        ))
        .unwrap();
        assert!(
            out.starts_with("Focus on: Fix the login redirect\n"),
            "unexpected output:\n{out}"
        );
        assert!(out.contains("p1 priority, due 2026-08-21 (today), ~1.5 h effort"));
        assert!(out.contains("3 tasks"));
    }

    #[test]
    fn error_on_empty_input() {
        let err = pick(&opts("   \n\n# just a heading\n", "balanced")).unwrap_err();
        assert!(err.starts_with("no tasks found"), "{err}");
    }

    #[test]
    fn error_on_unrecognised_due_date() {
        let err = pick(&opts("Ship it due:someday-soon", "balanced")).unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("unrecognised due date"), "{err}");
        assert!(err.contains("YYYY-MM-DD"), "{err}");
    }

    #[test]
    fn error_on_unrecognised_effort() {
        let err = pick(&opts("Ship it est:ages", "balanced")).unwrap_err();
        assert!(err.contains("unrecognised effort"), "{err}");
    }

    #[test]
    fn error_on_unknown_method() {
        let err = pick(&opts("Ship it", "rice")).unwrap_err();
        assert!(
            err.contains("expected balanced, deadline, wsjf, quick-wins or eisenhower"),
            "{err}"
        );
    }

    #[test]
    fn error_on_unknown_format() {
        let mut o = opts("Ship it", "balanced");
        o.format = "csv";
        assert!(pick(&o)
            .unwrap_err()
            .contains("expected text, markdown or json"));
    }

    #[test]
    fn overdue_boost_pins_the_overdue_task() {
        let list = "Tiny overdue chore !p4 due:2026-08-18 est:8h\n\
                    Huge important thing !p0 due:+9d est:1h";
        let pinned = pick(&opts(list, "balanced")).unwrap();
        assert!(
            pinned.starts_with("Focus on: Tiny overdue chore"),
            "{pinned}"
        );
        assert!(pinned.contains("Pinned above everything not overdue (3 days overdue)."));

        let mut o = opts(list, "balanced");
        o.overdue_boost = false;
        let free = pick(&o).unwrap();
        assert!(free.starts_with("Focus on: Huge important thing"), "{free}");
        assert!(!free.contains("Pinned above"));
    }

    #[test]
    fn deadline_method_takes_the_soonest_due_date() {
        let out = pick(&opts(
            "Big deal !p0 due:+6d est:1h\nSmall thing !p4 due:tomorrow est:4h",
            "deadline",
        ))
        .unwrap();
        assert!(out.starts_with("Focus on: Small thing"), "{out}");
    }

    #[test]
    fn quick_wins_prefers_the_smaller_job() {
        let list = "Slow slog !p1 due:+3d est:20h\nFast win !p1 due:+3d est:20m";
        let quick = pick(&opts(list, "quick-wins")).unwrap();
        assert!(quick.starts_with("Focus on: Fast win"), "{quick}");
    }

    #[test]
    fn wsjf_divides_by_job_size() {
        let out = pick(&opts(
            "Cheap and valuable !p1 due:today est:2h\nSame value ten times the size !p1 due:today est:20h",
            "wsjf",
        ))
        .unwrap();
        assert!(out.starts_with("Focus on: Cheap and valuable"), "{out}");
        // (10*0.8 + 10*1.0) / 2 = 9.00
        assert!(out.contains("highest WSJF (9.00)"), "{out}");
    }

    #[test]
    fn eisenhower_labels_every_quadrant() {
        let out = pick(&opts(
            "Server on fire !p1 due:today est:1h\n\
             Plan next quarter !p2 due:+30d est:4h\n\
             Answer the survey !p4 due:tomorrow est:10m\n\
             Reorganise bookmarks !p4 due:+40d est:1h",
            "eisenhower",
        ))
        .unwrap();
        assert!(out.starts_with("Focus on: Server on fire"), "{out}");
        assert!(
            out.contains("Eisenhower quadrant \"Do first\" (urgent + important)"),
            "{out}"
        );
        for q in ["Do first", "Schedule", "Delegate", "Drop"] {
            assert!(out.contains(q), "missing quadrant {q} in:\n{out}");
        }
    }

    #[test]
    fn parses_delimited_columns_and_tabs() {
        let piped = pick(&opts(
            "Renew the domain | p1 | 2026-08-22 | 30m",
            "balanced",
        ))
        .unwrap();
        assert!(piped.contains("Focus on: Renew the domain"), "{piped}");
        assert!(
            piped.contains("p1 priority, due 2026-08-22 (tomorrow), ~0.5 h effort"),
            "{piped}"
        );

        let tabbed = pick(&opts("Renew the domain\tp1\t2026-08-22\t30m", "balanced")).unwrap();
        assert_eq!(piped, tabbed);
    }

    #[test]
    fn bare_integer_column_is_a_priority_and_bare_hours_is_effort() {
        let out = pick(&opts("Do the thing | 1 | tomorrow | 6", "balanced")).unwrap();
        assert!(
            out.contains("p1 priority, due 2026-08-22 (tomorrow), ~6 h effort"),
            "{out}"
        );
    }

    #[test]
    fn error_on_unusable_column() {
        let err = pick(&opts("Do the thing | sometime next century", "balanced")).unwrap_err();
        assert!(err.contains("is not a priority"), "{err}");
    }

    #[test]
    fn strips_bullets_numbering_and_skips_done_items() {
        let out = pick(&opts(
            "# My list\n- [x] Already shipped !p0 due:today\n- [ ] Still open !p1 due:today est:1h\n2. Numbered one !p4 est:9h",
            "balanced",
        ))
        .unwrap();
        assert!(out.starts_with("Focus on: Still open"), "{out}");
        assert!(!out.contains("Already shipped"), "{out}");
        assert!(out.contains("Numbered one"), "{out}");
        assert!(out.contains("2 tasks"), "{out}");
    }

    #[test]
    fn todo_txt_letter_priority_maps_to_p1() {
        let out = pick(&opts(
            "(A) Call the bank est:20m\n(C) Water the plants est:5m",
            "balanced",
        ))
        .unwrap();
        assert!(out.starts_with("Focus on: Call the bank"), "{out}");
        assert!(out.contains("p1 priority"), "{out}");
    }

    #[test]
    fn keywords_set_priority_when_no_tag_is_present() {
        let out = pick(&opts(
            "Buy stamps someday\nUrgent: patch the CVE",
            "balanced",
        ))
        .unwrap();
        assert!(out.starts_with("Focus on: Urgent: patch the CVE"), "{out}");
        assert!(out.contains("p1 priority"), "{out}");
        assert!(out.contains("p4 · no due date"), "{out}");
    }

    #[test]
    fn defaults_apply_to_untagged_tasks() {
        let mut o = opts("Untagged chore", "balanced");
        o.default_priority = "p0";
        o.default_effort = 3.5;
        let out = pick(&o).unwrap();
        assert!(
            out.contains("p0 priority, no due date, ~3.5 h effort"),
            "{out}"
        );
        assert!(out.contains("3.5 h total effort"), "{out}");
    }

    #[test]
    fn error_on_unknown_default_priority() {
        let mut o = opts("Untagged chore", "balanced");
        o.default_priority = "p9";
        assert!(pick(&o)
            .unwrap_err()
            .contains("expected p0, p1, p2, p3 or p4"));
    }

    #[test]
    fn show_ranking_off_omits_the_table() {
        let mut o = opts("A !p1 due:today est:1h\nB !p4 est:9h", "balanced");
        o.show_ranking = false;
        let out = pick(&o).unwrap();
        assert!(!out.contains("Full ranking"), "{out}");
        assert!(out.contains("Focus on: A"), "{out}");
    }

    #[test]
    fn markdown_format_emits_a_table() {
        let mut o = opts("Alpha !p1 due:today est:1h\nBeta !p3 est:4h", "balanced");
        o.format = "markdown";
        let out = pick(&o).unwrap();
        assert!(out.starts_with("**Focus on:** Alpha\n"), "{out}");
        assert!(
            out.contains("| # | Score | Task | Priority | Due | Effort |"),
            "{out}"
        );
        assert!(out.contains("| 1 |"), "{out}");
    }

    #[test]
    fn json_format_is_parseable_and_complete() {
        let mut o = opts("Alpha !p1 due:today est:1h\nBeta !p3 est:4h", "balanced");
        o.format = "json";
        let out = pick(&o).unwrap();
        assert!(out.contains("\"today\": \"2026-08-21\""), "{out}");
        assert!(out.contains("\"task_count\": 2"), "{out}");
        assert!(out.contains("\"total_effort_hours\": 5"), "{out}");
        assert!(out.contains("\"rank\":1,\"task\":\"Alpha\""), "{out}");
        assert!(out.contains("\"due\":\"2026-08-21\""), "{out}");
        assert!(out.contains("\"days_until\":0"), "{out}");
        assert!(out.contains("\"ranked\": ["), "{out}");
    }

    #[test]
    fn json_escapes_quotes_in_task_titles() {
        let mut o = opts("Ship the \"beta\" build !p1", "balanced");
        o.format = "json";
        let out = pick(&o).unwrap();
        assert!(out.contains(r#"\"beta\""#), "{out}");
    }

    #[test]
    fn relative_and_weekday_due_dates_resolve() {
        // TODAY is a Friday: "friday" is today, "monday" is +3.
        assert_eq!(parse_due("friday", TODAY), Some(TODAY));
        assert_eq!(parse_due("monday", TODAY), Some(TODAY + 3));
        assert_eq!(parse_due("eow", TODAY), Some(TODAY));
        assert_eq!(parse_due("+2w", TODAY), Some(TODAY + 14));
        assert_eq!(parse_due("+3", TODAY), Some(TODAY + 3));
        assert_eq!(parse_due("yesterday", TODAY), Some(TODAY - 1));
        assert_eq!(
            parse_due("2026-02-29", TODAY),
            None,
            "2026 is not a leap year"
        );
        assert_eq!(
            parse_due("2024-02-29", TODAY),
            Some(days_from_civil(2024, 2, 29))
        );
    }

    #[test]
    fn effort_units_convert_to_hours() {
        assert_eq!(parse_effort("90m"), Some(1.5));
        assert_eq!(parse_effort("1.5h"), Some(1.5));
        assert_eq!(parse_effort("2d"), Some(16.0));
        assert_eq!(parse_effort("1w"), Some(40.0));
        assert_eq!(parse_effort("3"), Some(3.0));
        assert_eq!(parse_effort("0"), None);
        assert_eq!(parse_effort("-2h"), None);
    }

    #[test]
    fn resolve_today_uses_the_clock_when_blank() {
        assert_eq!(resolve_today("", 1_787_356_800.0).unwrap(), 20_687);
        assert_eq!(resolve_today("2026-08-21", 0.0).unwrap(), TODAY);
        assert!(resolve_today("21/08/2026", 0.0).is_err());
    }

    #[test]
    fn single_task_reads_naturally() {
        let out = pick(&opts("Just this one !p2 est:1h", "balanced")).unwrap();
        assert!(out.contains("of 1 task."), "{out}");
        assert!(!out.contains("Full ranking"), "{out}");
    }

    #[test]
    fn ties_break_deterministically_by_input_order() {
        let out = pick(&opts(
            "First one !p2 est:2h\nSecond one !p2 est:2h",
            "balanced",
        ))
        .unwrap();
        assert!(out.starts_with("Focus on: First one"), "{out}");
    }

    #[test]
    fn task_cap_is_enforced() {
        let many = (0..=MAX_TASKS)
            .map(|i| format!("Task {i} !p3"))
            .collect::<Vec<_>>()
            .join("\n");
        let err = pick(&opts(&many, "balanced")).unwrap_err();
        assert!(err.contains("too many tasks"), "{err}");
    }
}
