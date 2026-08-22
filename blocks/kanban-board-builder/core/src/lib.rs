//! kanban-board-builder core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Turns a task list or freeform notes into a **column-shaped** board. Every
//! non-blank line becomes one card; the card is routed to a column by, in
//! order of precedence:
//!
//! 1. an explicit status tag on the line — `status:doing`, `status=done`, or a
//!    bracketed `[In Progress]` whose contents name a column;
//! 2. a `#tag` that names a column (`#done`, `#blocked`);
//! 3. the section heading currently in effect (`## In Progress`, or a bare
//!    `In Progress:` line) — this is what makes a board round-trip;
//! 4. a ticked checkbox (`- [x]`) → the done-like column;
//! 5. a workflow marker in the text (see [`MULTI_WORD_MARKERS`] /
//!    [`SINGLE_WORD_MARKERS`]);
//! 6. the fallback column (`default_column`, else the first column).
//!
//! Column names are matched loosely: case, punctuation and spacing are ignored,
//! and a synonym table maps names onto six canonical workflow groups (backlog,
//! todo, doing, blocked, review, done) so `Doing` matches `In Progress`.
//!
//! Card metadata (`@assignee`, `#tags`, `[tags]`, `!priority` / `P1` /
//! `priority:high`, `due:YYYY-MM-DD`) is parsed off the line, so it can be
//! re-emitted in a canonical order in Markdown and broken out into fields in
//! JSON.

use serde_json::{json, Value};

/// Hard cap on cards, so a pasted log file fails loudly instead of producing a
/// megabyte of Markdown.
pub const MAX_CARDS: usize = 500;
/// Hard cap on columns — beyond this a board stops being a board.
pub const MAX_COLUMNS: usize = 12;
/// Column set used when `columns` is left empty.
pub const DEFAULT_COLUMNS: &str = "To Do, In Progress, Done";

/// Card priority, highest first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

impl Priority {
    fn parse(s: &str) -> Option<Priority> {
        match s.trim().to_ascii_lowercase().as_str() {
            "critical" | "crit" | "urgent" | "blocker" | "p0" => Some(Priority::Critical),
            "high" | "hi" | "p1" => Some(Priority::High),
            "medium" | "med" | "normal" | "p2" => Some(Priority::Medium),
            "low" | "minor" | "p3" | "p4" => Some(Priority::Low),
            _ => None,
        }
    }

    /// Canonical lowercase label used in both Markdown (`!high`) and JSON.
    pub fn label(self) -> &'static str {
        match self {
            Priority::Critical => "critical",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Priority::Critical => 0,
            Priority::High => 1,
            Priority::Medium => 2,
            Priority::Low => 3,
        }
    }
}

/// One card on the board.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card {
    /// The task text with all recognised metadata removed.
    pub text: String,
    /// True when the line was ticked (`- [x]`) or the card landed in a done-like column.
    pub done: bool,
    /// `@name`, without the `@`.
    pub assignee: Option<String>,
    /// `#tag` and `[tag]` labels, without the marker, in first-seen order.
    pub tags: Vec<String>,
    /// `!high`, `P1`, `priority:high`.
    pub priority: Option<Priority>,
    /// `due:YYYY-MM-DD`, `due=YYYY-MM-DD` or `due YYYY-MM-DD`.
    pub due: Option<String>,
}

/// One column (lane) of the finished board.
#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    pub cards: Vec<Card>,
    /// Canonical workflow group, when the name maps onto one.
    pub group: Option<&'static str>,
}

// ---------------------------------------------------------------------------
// Column-name matching
// ---------------------------------------------------------------------------

/// Lowercase, turn every non-alphanumeric character into a space, collapse runs.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else {
            pending_space = true;
        }
    }
    out
}

/// Canonical workflow group → the column names that mean it.
const GROUPS: &[(&str, &[&str])] = &[
    (
        "backlog",
        &["backlog", "icebox", "someday", "ideas", "inbox", "later", "parking lot", "wishlist"],
    ),
    (
        "todo",
        &[
            "to do", "todo", "to dos", "todos", "open", "next", "planned", "not started", "new",
            "ready", "up next", "queue", "pending", "candidate",
        ],
    ),
    (
        "doing",
        &[
            "in progress", "doing", "wip", "work in progress", "started", "active", "working",
            "ongoing", "current", "in flight", "in development", "dev", "underway",
        ],
    ),
    (
        "blocked",
        &["blocked", "waiting", "on hold", "stuck", "waiting on", "paused", "impeded", "hold", "at risk"],
    ),
    (
        "review",
        &[
            "review", "in review", "code review", "qa", "testing", "needs review",
            "awaiting review", "verify", "validation", "in qa", "test",
        ],
    ),
    (
        "done",
        &[
            "done", "complete", "completed", "finished", "shipped", "closed", "resolved",
            "delivered", "merged", "live", "released",
        ],
    ),
];

fn group_of(normalized: &str) -> Option<&'static str> {
    GROUPS
        .iter()
        .find(|(_, names)| names.contains(&normalized))
        .map(|(g, _)| *g)
}

/// Workflow phrases containing a space. These are specific enough to match
/// anywhere in a card's text.
pub const MULTI_WORD_MARKERS: &[(&str, &str)] = &[
    ("work in progress", "doing"),
    ("in progress", "doing"),
    ("in flight", "doing"),
    ("not started", "todo"),
    ("to do", "todo"),
    ("on hold", "blocked"),
    ("waiting on", "blocked"),
    ("waiting for", "blocked"),
    ("blocked by", "blocked"),
    ("needs review", "review"),
    ("awaiting review", "review"),
    ("code review", "review"),
    ("in review", "review"),
    ("in qa", "review"),
    ("parking lot", "backlog"),
];

/// One-word workflow markers. Far too common inside ordinary prose to match
/// mid-sentence, so they only count at the START or END of a line — the way a
/// status marker is actually written in notes (`fix login — blocked`).
pub const SINGLE_WORD_MARKERS: &[(&str, &str)] = &[
    ("doing", "doing"),
    ("wip", "doing"),
    ("started", "doing"),
    ("blocked", "blocked"),
    ("stuck", "blocked"),
    ("waiting", "blocked"),
    ("paused", "blocked"),
    ("review", "review"),
    ("qa", "review"),
    ("testing", "review"),
    ("done", "done"),
    ("complete", "done"),
    ("completed", "done"),
    ("finished", "done"),
    ("shipped", "done"),
    ("merged", "done"),
    ("todo", "todo"),
    ("backlog", "backlog"),
    ("someday", "backlog"),
    ("icebox", "backlog"),
];

fn single_marker(normalized_word: &str) -> Option<&'static str> {
    SINGLE_WORD_MARKERS
        .iter()
        .find(|(w, _)| *w == normalized_word)
        .map(|(_, g)| *g)
}

/// Resolve a free-text name (`doing`, `In-Progress`, `DONE`) to a column index.
fn resolve_column(name: &str, columns: &[Column]) -> Option<usize> {
    let n = normalize(name);
    if n.is_empty() {
        return None;
    }
    if let Some(i) = columns.iter().position(|c| normalize(&c.name) == n) {
        return Some(i);
    }
    let g = group_of(&n)?;
    route_group(columns, g)
}

// ---------------------------------------------------------------------------
// Line parsing
// ---------------------------------------------------------------------------

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let month: u32 = s[5..7].parse().unwrap_or(0);
    let day: u32 = s[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Pull `[bracketed]` labels out of a line. A `[...]` immediately followed by
/// `(` is a Markdown link and is left alone.
fn extract_brackets(text: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = text.chars().collect();
    let mut kept = String::with_capacity(text.len());
    let mut labels = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            if let Some(close) = (i + 1..chars.len()).find(|&j| chars[j] == ']') {
                let inner: String = chars[i + 1..close].iter().collect();
                let is_link = chars.get(close + 1) == Some(&'(');
                if !is_link && !inner.trim().is_empty() && !inner.contains('[') {
                    labels.push(inner.trim().to_string());
                    i = close + 1;
                    continue;
                }
            }
        }
        kept.push(chars[i]);
        i += 1;
    }
    (kept, labels)
}

fn strip_wrappers(tok: &str) -> &str {
    tok.trim_matches(|c: char| "()[]{}\"'“”‘’".contains(c))
}

/// Trailing punctuation that never belongs to a metadata token.
fn strip_tail_punct(tok: &str) -> &str {
    tok.trim_end_matches(|c: char| ".,;:!?".contains(c))
}

struct ParsedLine {
    card: Card,
    /// Explicit status from `status:` / `status=` / a bracketed column name.
    status: Option<String>,
    /// Checkbox present and ticked.
    checked: bool,
    /// Checkbox present at all (ticked or not).
    has_checkbox: bool,
}

/// Strip the bullet/number marker and the GFM checkbox off a line.
fn strip_bullet(line: &str) -> (String, bool, bool) {
    let mut t = line.trim().to_string();
    // "- ", "* ", "+ ", "• "
    let bullets = ['-', '*', '+', '\u{2022}'];
    if let Some(first) = t.chars().next() {
        if bullets.contains(&first) {
            let rest: String = t.chars().skip(1).collect();
            if rest.starts_with(' ') || rest.starts_with('\t') || rest.is_empty() {
                t = rest.trim_start().to_string();
            }
        } else if first.is_ascii_digit() {
            // "1. " / "1) "
            let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
            let rest: String = t.chars().skip(digits.len()).collect();
            if (rest.starts_with(". ") || rest.starts_with(") ")) && digits.len() <= 4 {
                t = rest[2..].trim_start().to_string();
            }
        }
    }
    let lower: String = t.chars().take(4).collect::<String>().to_ascii_lowercase();
    if lower.starts_with("[ ]") || lower.starts_with("[x]") {
        let checked = lower.starts_with("[x]");
        t = t.chars().skip(3).collect::<String>().trim_start().to_string();
        return (t, checked, true);
    }
    (t, false, false)
}

fn parse_line(line: &str) -> ParsedLine {
    let (body, checked, has_checkbox) = strip_bullet(line);
    let (body, labels) = extract_brackets(&body);

    let mut card = Card {
        done: checked,
        ..Card::default()
    };
    let mut status: Option<String> = None;

    for label in labels {
        // A bracketed label that names a workflow state is a status; anything
        // else is a plain tag. Resolution against the real column list happens
        // later, so here we only recognise the canonical groups.
        if status.is_none() && group_of(&normalize(&label)).is_some() {
            status = Some(label);
        } else {
            card.tags.push(label);
        }
    }

    let tokens: Vec<&str> = body.split_whitespace().collect();
    let mut kept: Vec<String> = Vec::with_capacity(tokens.len());
    let mut skip_next = false;
    for (i, raw) in tokens.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        let tok = strip_tail_punct(strip_wrappers(raw));
        let lower = tok.to_ascii_lowercase();

        if let Some(rest) = tok.strip_prefix('@') {
            if card.assignee.is_none()
                && !rest.is_empty()
                && rest.chars().next().is_some_and(|c| c.is_alphanumeric())
            {
                card.assignee = Some(rest.to_string());
                continue;
            }
        }
        if let Some(rest) = tok.strip_prefix('#') {
            if !rest.is_empty() && rest.chars().next().is_some_and(|c| c.is_alphabetic()) {
                card.tags.push(rest.to_string());
                continue;
            }
        }
        if let Some(rest) = tok.strip_prefix('!') {
            if let Some(p) = Priority::parse(rest) {
                if card.priority.is_none() {
                    card.priority = Some(p);
                }
                continue;
            }
        }
        if card.priority.is_none() && matches!(lower.as_str(), "p0" | "p1" | "p2" | "p3" | "p4") {
            card.priority = Priority::parse(&lower);
            continue;
        }
        if let Some(rest) = lower
            .strip_prefix("priority:")
            .or_else(|| lower.strip_prefix("priority="))
        {
            if let Some(p) = Priority::parse(rest) {
                if card.priority.is_none() {
                    card.priority = Some(p);
                }
                continue;
            }
        }
        if let Some(rest) = tok
            .strip_prefix("status:")
            .or_else(|| tok.strip_prefix("status="))
            .or_else(|| tok.strip_prefix("Status:"))
            .or_else(|| tok.strip_prefix("Status="))
        {
            if !rest.is_empty() {
                if status.is_none() {
                    status = Some(rest.to_string());
                }
                continue;
            }
        }
        if let Some(rest) = lower
            .strip_prefix("due:")
            .or_else(|| lower.strip_prefix("due="))
        {
            if is_iso_date(rest) {
                if card.due.is_none() {
                    card.due = Some(rest.to_string());
                }
                continue;
            }
        }
        if lower == "due" {
            if let Some(next) = tokens.get(i + 1) {
                let next = strip_tail_punct(strip_wrappers(next));
                if is_iso_date(next) {
                    if card.due.is_none() {
                        card.due = Some(next.to_string());
                    }
                    skip_next = true;
                    continue;
                }
            }
        }
        kept.push((*raw).to_string());
    }

    card.text = tidy(&kept.join(" "));
    ParsedLine {
        card,
        status,
        checked,
        has_checkbox,
    }
}

/// Clean up the residue metadata removal leaves behind: empty brackets, dangling
/// separators, doubled spaces.
fn tidy(s: &str) -> String {
    let mut t = s.replace("()", " ").replace("( )", " ").replace("[]", " ");
    while t.contains("  ") {
        t = t.replace("  ", " ");
    }
    t.trim()
        .trim_end_matches(|c: char| " -\u{2013}\u{2014}|:,;(".contains(c))
        .trim_start_matches(|c: char| " -\u{2013}\u{2014}|:,;)".contains(c))
        .trim()
        .to_string()
}

/// Is this line a section heading, and if so what does it name?
/// `## In Progress` → `Some("In Progress")`; `Doing:` → `Some("Doing")`.
fn heading_name(line: &str) -> Option<String> {
    let t = line.trim();
    if let Some(rest) = t.strip_prefix('#') {
        let name = rest.trim_start_matches('#').trim();
        // Drop a trailing "(3)" / "(2 / limit 3 — over WIP limit)" count suffix
        // so a board this tool emitted re-imports cleanly.
        return Some(strip_count_suffix(name));
    }
    // A bare "In Progress:" line — only a heading if it has no bullet and no
    // trailing content after the colon.
    if let Some(name) = t.strip_suffix(':') {
        let name = name.trim();
        if !name.is_empty()
            && name.chars().count() <= 40
            && !name.contains(':')
            && !name.starts_with(['-', '*', '+'])
        {
            return Some(strip_count_suffix(name));
        }
    }
    None
}

fn strip_count_suffix(name: &str) -> String {
    match (name.rfind('('), name.ends_with(')')) {
        (Some(open), true) if open > 0 => name[..open].trim().to_string(),
        _ => name.trim().to_string(),
    }
}

/// Lines that carry no card: blank, rules, table separators, and the empty-lane
/// placeholder this tool itself emits (so a board round-trips).
fn is_noise(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t == "_No cards._" {
        return true;
    }
    let stripped: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    if stripped.len() >= 3 && stripped.chars().all(|c| c == '-' || c == '*' || c == '_') {
        return true;
    }
    // Markdown table separator row: | --- | :--- |
    if stripped.starts_with('|') && stripped.chars().all(|c| "|-:".contains(c)) {
        return true;
    }
    false
}

/// Find a workflow marker at the edge of the text. Returns the group plus the
/// text with the marker removed (the marker is a status label, not part of the
/// task), or `None`.
fn edge_phrase(words: &[&str]) -> Option<&'static str> {
    let joined = normalize(&words.join(" "));
    if joined.is_empty() {
        return None;
    }
    if words.len() == 1 {
        return single_marker(&joined);
    }
    MULTI_WORD_MARKERS
        .iter()
        .find(|(phrase, _)| *phrase == joined)
        .map(|(_, g)| *g)
}

fn edge_marker(text: &str) -> Option<(&'static str, String)> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    // Longest edge phrase wins, so "needs review" beats a bare trailing "review".
    let tail_trimmed = t
        .trim_end_matches(|c: char| "()[]{}.,;:!*_~".contains(c))
        .trim_end();
    let tail_words: Vec<&str> = tail_trimmed.split_whitespace().collect();
    for k in (1..=3.min(tail_words.len())).rev() {
        let start = tail_words.len() - k;
        if let Some(g) = edge_phrase(&tail_words[start..]) {
            let head = tidy(&tail_words[..start].join(" "));
            if !head.is_empty() {
                return Some((g, head));
            }
        }
    }
    // Leading marker: "Blocked: fix login", "In progress — deploy"
    let head_src = t.trim_start_matches(|c: char| "([{".contains(c));
    let head_words: Vec<&str> = head_src.split_whitespace().collect();
    for k in (1..=3.min(head_words.len())).rev() {
        let cleaned: Vec<&str> = head_words[..k]
            .iter()
            .map(|w| w.trim_matches(|c: char| ":-\u{2013}\u{2014}|)]},".contains(c)))
            .collect();
        if let Some(g) = edge_phrase(&cleaned) {
            let rest = tidy(&head_words[k..].join(" "));
            if !rest.is_empty() {
                return Some((g, rest));
            }
        }
    }
    // The whole line is the marker — route it, but keep the text.
    if let Some(g) = single_marker(&normalize(t)) {
        return Some((g, t.to_string()));
    }
    None
}

fn phrase_marker(text: &str) -> Option<&'static str> {
    let padded = format!(" {} ", normalize(text));
    MULTI_WORD_MARKERS
        .iter()
        .find(|(phrase, _)| padded.contains(&format!(" {phrase} ")))
        .map(|(_, g)| *g)
}

// ---------------------------------------------------------------------------
// Board assembly
// ---------------------------------------------------------------------------

fn parse_columns(spec: &str) -> Result<Vec<Column>, String> {
    let spec = if spec.trim().is_empty() {
        DEFAULT_COLUMNS
    } else {
        spec
    };
    let names: Vec<String> = spec
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return Err(format!(
            "columns is empty: list at least one column name, comma-separated (e.g. \"{DEFAULT_COLUMNS}\")"
        ));
    }
    if names.len() > MAX_COLUMNS {
        return Err(format!(
            "too many columns: {} (max {MAX_COLUMNS})",
            names.len()
        ));
    }
    let mut seen: Vec<String> = Vec::new();
    for n in &names {
        let norm = normalize(n);
        if seen.contains(&norm) {
            return Err(format!("duplicate column name: \"{n}\""));
        }
        seen.push(norm);
    }
    Ok(names
        .into_iter()
        .map(|name| {
            let group = group_of(&normalize(&name));
            Column {
                name,
                cards: Vec::new(),
                group,
            }
        })
        .collect())
}

/// The column ticked checkboxes fall into: the done-group column if there is
/// one, else the last column.
fn done_column(columns: &[Column]) -> usize {
    columns
        .iter()
        .position(|c| c.group == Some("done"))
        .unwrap_or(columns.len() - 1)
}

fn group_column(columns: &[Column], group: &str) -> Option<usize> {
    columns.iter().position(|c| c.group == Some(group))
}

/// Where a detected workflow group lands when the board has no column for it.
/// A `blocked` card on a To Do / In Progress / Done board is still started work,
/// so it belongs in the doing lane rather than back in To Do.
const GROUP_FALLBACKS: &[(&str, &[&str])] = &[
    ("backlog", &["todo"]),
    ("todo", &[]),
    ("doing", &["todo"]),
    ("blocked", &["doing", "todo"]),
    ("review", &["doing", "todo"]),
    ("done", &[]),
];

/// Resolve a workflow group to a column, walking the fallback chain.
fn route_group(columns: &[Column], group: &str) -> Option<usize> {
    if let Some(i) = group_column(columns, group) {
        return Some(i);
    }
    let chain = GROUP_FALLBACKS
        .iter()
        .find(|(g, _)| *g == group)
        .map(|(_, c)| *c)
        .unwrap_or(&[]);
    chain.iter().find_map(|g| group_column(columns, g))
}

fn sort_cards(cards: &mut [Card], sort_by: &str) {
    match sort_by {
        "priority" => cards.sort_by_key(|c| c.priority.map_or(4, |p| p.rank())),
        "due" => cards.sort_by(|a, b| match (&a.due, &b.due) {
            (Some(x), Some(y)) => x.cmp(y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }),
        "text" => cards.sort_by_key(|c| c.text.to_lowercase()),
        _ => {}
    }
}

/// Build the board. Returns the rendered board in the requested `format`.
///
/// * `tasks` — the task list / freeform notes, one task per line.
/// * `columns` — comma-separated column names, in board order.
/// * `format` — `markdown`, `table` or `json`.
/// * `title` — board title (empty = no title line).
/// * `default_column` — where unrouted cards land (empty = the first column).
/// * `wip_limit` — per-column card limit, 0 = none.
/// * `sort_by` — `none`, `priority`, `due` or `text`.
/// * `show_counts` — append `(n)` card counts to column headings.
#[allow(clippy::too_many_arguments)]
pub fn build(
    tasks: &str,
    columns: &str,
    format: &str,
    title: &str,
    default_column: &str,
    wip_limit: i64,
    sort_by: &str,
    show_counts: bool,
) -> Result<String, String> {
    let format = if format.trim().is_empty() {
        "markdown"
    } else {
        format.trim()
    };
    if !matches!(format, "markdown" | "table" | "json") {
        return Err(format!(
            "invalid format {format:?}: expected \"markdown\", \"table\" or \"json\""
        ));
    }
    let sort_by = if sort_by.trim().is_empty() {
        "none"
    } else {
        sort_by.trim()
    };
    if !matches!(sort_by, "none" | "priority" | "due" | "text") {
        return Err(format!(
            "invalid sort_by {sort_by:?}: expected \"none\", \"priority\", \"due\" or \"text\""
        ));
    }
    if wip_limit < 0 {
        return Err(format!(
            "invalid wip_limit {wip_limit}: expected 0 (no limit) or a positive whole number"
        ));
    }

    let mut cols = parse_columns(columns)?;

    let fallback = if default_column.trim().is_empty() {
        0
    } else {
        resolve_column(default_column, &cols).ok_or_else(|| {
            format!(
                "default_column {:?} is not one of the columns: {}",
                default_column.trim(),
                cols.iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?
    };
    let done_idx = done_column(&cols);

    let mut section: Option<usize> = None;
    let mut total = 0usize;
    for line in tasks.lines() {
        if is_noise(line) {
            continue;
        }
        if let Some(name) = heading_name(line) {
            section = resolve_column(&name, &cols);
            continue;
        }

        let parsed = parse_line(line);
        if parsed.card.text.is_empty()
            && parsed.card.assignee.is_none()
            && parsed.card.tags.is_empty()
        {
            continue;
        }

        total += 1;
        if total > MAX_CARDS {
            return Err(format!(
                "too many tasks: more than {MAX_CARDS} lines. Split the notes into smaller boards."
            ));
        }

        let mut card = parsed.card;
        let target = if let Some(idx) = parsed.status.as_deref().and_then(|s| resolve_column(s, &cols)) {
            Some(idx)
        } else if let Some(pos) = card.tags.iter().position(|t| resolve_column(t, &cols).is_some()) {
            let tag = card.tags.remove(pos);
            resolve_column(&tag, &cols)
        } else {
            None
        };

        let idx = match target {
            Some(i) => i,
            None => match section {
                Some(i) => i,
                None if parsed.has_checkbox && parsed.checked => done_idx,
                None => {
                    let by_edge = edge_marker(&card.text).and_then(|(g, rest)| {
                        route_group(&cols, g).map(|i| {
                            card.text = rest;
                            i
                        })
                    });
                    match by_edge {
                        Some(i) => i,
                        None => phrase_marker(&card.text)
                            .and_then(|g| route_group(&cols, g))
                            .unwrap_or(fallback),
                    }
                }
            },
        };

        if cols[idx].group == Some("done") {
            card.done = true;
        }
        cols[idx].cards.push(card);
    }

    if total == 0 {
        return Err(
            "no tasks found: paste at least one task, one per line (bullets and checkboxes optional)"
                .to_string(),
        );
    }

    for col in &mut cols {
        sort_cards(&mut col.cards, sort_by);
    }

    Ok(match format {
        "json" => render_json(&cols, title, wip_limit),
        "table" => render_table(&cols, title, wip_limit, show_counts),
        _ => render_markdown(&cols, title, wip_limit, show_counts),
    })
}

/// Parse the `wip_limit` field as it arrives from the page (a string).
pub fn parse_wip_limit(raw: &str) -> Result<i64, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i64>().map_err(|_| {
        format!("invalid wip_limit {t:?}: expected a whole number (0 = no limit)")
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn heading_suffix(count: usize, wip_limit: i64, show_counts: bool) -> String {
    let limit = wip_limit.max(0) as usize;
    let over = limit > 0 && count > limit;
    match (show_counts, limit > 0) {
        (true, true) if over => format!(" ({count} / limit {limit} — over WIP limit)"),
        (true, true) => format!(" ({count} / limit {limit})"),
        (true, false) => format!(" ({count})"),
        (false, true) if over => " (over WIP limit)".to_string(),
        _ => String::new(),
    }
}

/// Re-emit a card's text with its metadata in a canonical order.
pub fn card_line(card: &Card) -> String {
    let mut s = card.text.clone();
    if let Some(a) = &card.assignee {
        s.push_str(&format!(" @{a}"));
    }
    for t in &card.tags {
        s.push_str(&format!(" #{t}"));
    }
    if let Some(p) = card.priority {
        s.push_str(&format!(" !{}", p.label()));
    }
    if let Some(d) = &card.due {
        s.push_str(&format!(" due:{d}"));
    }
    s.trim().to_string()
}

fn render_markdown(cols: &[Column], title: &str, wip_limit: i64, show_counts: bool) -> String {
    let mut out = String::new();
    if !title.trim().is_empty() {
        out.push_str(&format!("# {}\n\n", title.trim()));
    }
    for col in cols {
        out.push_str(&format!(
            "## {}{}\n\n",
            col.name,
            heading_suffix(col.cards.len(), wip_limit, show_counts)
        ));
        if col.cards.is_empty() {
            out.push_str("_No cards._\n\n");
        } else {
            for card in &col.cards {
                out.push_str(&format!(
                    "- [{}] {}\n",
                    if card.done { "x" } else { " " },
                    card_line(card)
                ));
            }
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

fn render_table(cols: &[Column], title: &str, wip_limit: i64, show_counts: bool) -> String {
    let mut out = String::new();
    if !title.trim().is_empty() {
        out.push_str(&format!("# {}\n\n", title.trim()));
    }
    let headers: Vec<String> = cols
        .iter()
        .map(|c| {
            escape_cell(&format!(
                "{}{}",
                c.name,
                heading_suffix(c.cards.len(), wip_limit, show_counts)
            ))
        })
        .collect();
    out.push_str(&format!("| {} |\n", headers.join(" | ")));
    out.push_str(&format!(
        "| {} |\n",
        cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    ));
    let rows = cols.iter().map(|c| c.cards.len()).max().unwrap_or(0);
    for r in 0..rows {
        let cells: Vec<String> = cols
            .iter()
            .map(|c| match c.cards.get(r) {
                Some(card) => {
                    let text = escape_cell(&card_line(card));
                    if card.done {
                        format!("~~{text}~~")
                    } else {
                        text
                    }
                }
                None => String::new(),
            })
            .collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out.trim_end().to_string()
}

fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|")
}

fn render_json(cols: &[Column], title: &str, wip_limit: i64) -> String {
    let limit = wip_limit.max(0) as usize;
    let columns: Vec<Value> = cols
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "count": c.cards.len(),
                "wip_limit": if limit > 0 { json!(limit) } else { Value::Null },
                "over_limit": limit > 0 && c.cards.len() > limit,
                "cards": c.cards.iter().map(|card| json!({
                    "text": card.text,
                    "done": card.done,
                    "assignee": card.assignee.clone().map_or(Value::Null, Value::String),
                    "tags": card.tags,
                    "priority": card.priority.map_or(Value::Null, |p| Value::String(p.label().into())),
                    "due": card.due.clone().map_or(Value::Null, Value::String),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let total: usize = cols.iter().map(|c| c.cards.len()).sum();
    let done: usize = cols
        .iter()
        .flat_map(|c| c.cards.iter())
        .filter(|c| c.done)
        .count();
    let board = json!({
        "title": title.trim(),
        "columns": columns,
        "total_cards": total,
        "done_cards": done,
    });
    serde_json::to_string_pretty(&board).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn md(tasks: &str) -> String {
        build(tasks, "", "markdown", "Kanban Board", "", 0, "none", true).unwrap()
    }

    #[test]
    fn happy_path_three_columns() {
        // No Blocked column here, so the blocked card falls back to the doing
        // lane (started work), not back to To Do.
        let out = md("Write the docs\n- [ ] Fix login — blocked\n- [x] Ship v1");
        assert_eq!(
            out,
            "# Kanban Board\n\n## To Do (1)\n\n- [ ] Write the docs\n\n## In Progress (1)\n\n- [ ] Fix login\n\n## Done (1)\n\n- [x] Ship v1"
        );
    }

    #[test]
    fn group_fallback_chain() {
        // review → doing when there is no review lane; backlog → todo.
        let out = md("Add retries — needs review\nSomeday: rewrite the CLI");
        assert!(out.contains("## In Progress (1)\n\n- [ ] Add retries"), "{out}");
        assert!(out.contains("## To Do (1)\n\n- [ ] rewrite the CLI"), "{out}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = build("   \n\n", "", "markdown", "", "", 0, "none", true).unwrap_err();
        assert!(err.contains("no tasks found"), "{err}");
    }

    #[test]
    fn edge_marker_routes_and_is_stripped() {
        let out = build(
            "Fix login — blocked\nDeploy (wip)\nWIP: rewrite parser",
            "To Do, In Progress, Blocked, Done",
            "markdown",
            "",
            "",
            0,
            "none",
            false,
        )
        .unwrap();
        assert!(out.contains("## In Progress\n\n- [ ] Deploy\n- [ ] rewrite parser"), "{out}");
        assert!(out.contains("## Blocked\n\n- [ ] Fix login"), "{out}");
    }

    #[test]
    fn multi_word_marker_matches_mid_sentence() {
        let out = md("Migration is in progress on staging");
        assert!(
            out.contains("## In Progress (1)\n\n- [ ] Migration is in progress on staging"),
            "{out}"
        );
    }

    #[test]
    fn single_word_marker_does_not_match_mid_sentence() {
        // "review" mid-sentence must NOT route — only at an edge.
        let out = build(
            "Ask legal to review the contract wording",
            "To Do, Review, Done",
            "markdown",
            "",
            "",
            0,
            "none",
            true,
        )
        .unwrap();
        assert!(out.contains("## To Do (1)"), "{out}");
        assert!(out.contains("## Review (0)"), "{out}");
    }

    #[test]
    fn explicit_status_beats_everything() {
        // status: wins over the ticked checkbox for ROUTING; the tick itself is
        // preserved, because it is the user's own record of the task's state.
        let out = md("- [x] Rewrite parser status:todo");
        assert!(out.contains("## To Do (1)\n\n- [x] Rewrite parser"), "{out}");
        assert!(out.contains("## Done (0)"), "{out}");
    }

    #[test]
    fn bracketed_column_name_is_a_status() {
        let out = md("[In Progress] Fix the flaky test [flaky]");
        assert!(
            out.contains("## In Progress (1)\n\n- [ ] Fix the flaky test #flaky"),
            "{out}"
        );
    }

    #[test]
    fn section_headings_route_cards() {
        let out = md("## In Progress\n- Fix login\n\n## Done\n- Ship v1");
        assert!(out.contains("## In Progress (1)\n\n- [ ] Fix login"), "{out}");
        assert!(out.contains("## Done (1)\n\n- [x] Ship v1"), "{out}");
    }

    #[test]
    fn markdown_board_round_trips() {
        let first = md("Write the docs\n- [ ] Fix login — blocked\n- [x] Ship v1");
        let second = md(&first);
        assert_eq!(first, second, "re-importing a board must be a no-op");
    }

    #[test]
    fn metadata_is_parsed_and_recanonicalised() {
        let out = md("Write the docs @alice #docs !high due:2026-09-01");
        assert!(
            out.contains("- [ ] Write the docs @alice #docs !high due:2026-09-01"),
            "{out}"
        );
        let out2 = md("P1 Write the docs (due 2026-09-01) @alice");
        assert!(
            out2.contains("- [ ] Write the docs @alice !high due:2026-09-01"),
            "{out2}"
        );
    }

    #[test]
    fn markdown_link_is_not_a_tag() {
        let out = md("Read [the RFC](https://example.com/rfc)");
        assert!(out.contains("- [ ] Read [the RFC](https://example.com/rfc)"), "{out}");
    }

    #[test]
    fn wip_limit_annotates_over_limit_columns() {
        let out = build("a\nb\nc", "To Do, Done", "markdown", "", "", 2, "none", true).unwrap();
        assert!(out.contains("## To Do (3 / limit 2 — over WIP limit)"), "{out}");
        assert!(out.contains("## Done (0 / limit 2)"), "{out}");
    }

    #[test]
    fn wip_warning_survives_counts_off() {
        let out = build("a\nb\nc", "To Do, Done", "markdown", "", "", 2, "none", false).unwrap();
        assert!(out.contains("## To Do (over WIP limit)"), "{out}");
        assert!(out.contains("## Done\n"), "{out}");
    }

    #[test]
    fn sort_by_priority_then_due_then_text() {
        let tasks = "zeta !low\nalpha !critical due:2026-01-02\nmid due:2026-01-01";
        let by_prio = build(tasks, "To Do", "markdown", "", "", 0, "priority", false).unwrap();
        assert!(by_prio.starts_with("## To Do\n\n- [ ] alpha !critical due:2026-01-02"), "{by_prio}");
        let by_due = build(tasks, "To Do", "markdown", "", "", 0, "due", false).unwrap();
        assert!(by_due.contains("- [ ] mid due:2026-01-01\n- [ ] alpha"), "{by_due}");
        assert!(by_due.trim_end().ends_with("- [ ] zeta !low"), "{by_due}");
        let by_text = build(tasks, "To Do", "markdown", "", "", 0, "text", false).unwrap();
        assert!(by_text.starts_with("## To Do\n\n- [ ] alpha"), "{by_text}");
    }

    #[test]
    fn table_format_lays_lanes_side_by_side() {
        let out = build(
            "Write docs\n- [x] Ship v1",
            "To Do, Done",
            "table",
            "Sprint 12",
            "",
            0,
            "none",
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "# Sprint 12\n\n| To Do (1) | Done (1) |\n| --- | --- |\n| Write docs | ~~Ship v1~~ |"
        );
    }

    #[test]
    fn json_format_exposes_every_field() {
        let out = build(
            "Write the docs @alice #docs !high due:2026-09-01",
            "To Do, Done",
            "json",
            "Board",
            "",
            3,
            "none",
            true,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["title"], "Board");
        assert_eq!(v["total_cards"], 1);
        assert_eq!(v["done_cards"], 0);
        assert_eq!(v["columns"][0]["name"], "To Do");
        assert_eq!(v["columns"][0]["wip_limit"], 3);
        assert_eq!(v["columns"][0]["over_limit"], false);
        let card = &v["columns"][0]["cards"][0];
        assert_eq!(card["text"], "Write the docs");
        assert_eq!(card["assignee"], "alice");
        assert_eq!(card["tags"][0], "docs");
        assert_eq!(card["priority"], "high");
        assert_eq!(card["due"], "2026-09-01");
        assert_eq!(card["done"], false);
    }

    #[test]
    fn custom_columns_and_default_column() {
        let out = build(
            "Idea one\nShip it — done",
            "Backlog, Doing, Shipped",
            "markdown",
            "",
            "Backlog",
            0,
            "none",
            true,
        )
        .unwrap();
        assert!(out.contains("## Backlog (1)\n\n- [ ] Idea one"), "{out}");
        assert!(out.contains("## Shipped (1)\n\n- [x] Ship it"), "{out}");
    }

    #[test]
    fn unknown_default_column_errors() {
        let err = build("a", "To Do, Done", "markdown", "", "Nowhere", 0, "none", true).unwrap_err();
        assert!(err.contains("default_column"), "{err}");
        assert!(err.contains("To Do, Done"), "{err}");
    }

    #[test]
    fn invalid_format_and_sort_error() {
        assert!(build("a", "", "yaml", "", "", 0, "none", true)
            .unwrap_err()
            .contains("invalid format"));
        assert!(build("a", "", "markdown", "", "", 0, "colour", true)
            .unwrap_err()
            .contains("invalid sort_by"));
        assert!(build("a", "", "markdown", "", "", -1, "none", true)
            .unwrap_err()
            .contains("invalid wip_limit"));
        assert!(build("a", ",", "markdown", "", "", 0, "none", true)
            .unwrap_err()
            .contains("columns is empty"));
        assert!(build("a", "a,b,c,d,e,f,g,h,i,j,k,l,m", "markdown", "", "", 0, "none", true)
            .unwrap_err()
            .contains("too many columns"));
        assert!(build("a", "To Do, TO-DO", "markdown", "", "", 0, "none", true)
            .unwrap_err()
            .contains("duplicate column"));
    }

    #[test]
    fn card_cap_boundary() {
        let ok = (1..=MAX_CARDS).map(|i| format!("task {i}")).collect::<Vec<_>>().join("\n");
        assert!(build(&ok, "To Do", "markdown", "", "", 0, "none", true).is_ok());
        let too_many = format!("{ok}\ntask overflow");
        let err = build(&too_many, "To Do", "markdown", "", "", 0, "none", true).unwrap_err();
        assert!(err.contains("too many tasks"), "{err}");
    }

    #[test]
    fn pipes_in_card_text_are_escaped_in_tables() {
        let out = build("run a | b", "To Do", "table", "", "", 0, "none", false).unwrap();
        assert!(out.contains("| run a \\| b |"), "{out}");
    }

    #[test]
    fn numbered_and_bulleted_markers_are_stripped() {
        let out = md("1. First\n2) Second\n* Third\n+ Fourth");
        assert!(out.contains("- [ ] First\n- [ ] Second\n- [ ] Third\n- [ ] Fourth"), "{out}");
    }

    #[test]
    fn parse_wip_limit_accepts_blank() {
        assert_eq!(parse_wip_limit("  "), Ok(0));
        assert_eq!(parse_wip_limit("4"), Ok(4));
        assert!(parse_wip_limit("lots").is_err());
    }
}
