//! gizza-ai/rule-based-extractor core — apply a set of user-defined regex rules
//! to text and return the named fields as JSON, CSV, a readable listing, or a
//! rule-by-rule report.
//!
//! Rule grammar (one rule per line):
//!   `# comment` / `// comment`      ignored
//!   `@NAME = regex`                 define a reusable pattern macro, used as `%{NAME}`
//!   `field = regex`                 extract one named field (note the space before `=`)
//!   `regex`                         a bare rule: every named group becomes a field
//!
//! `%{NAME}` inserts a built-in (or user-defined) pattern non-capturing;
//! `%{NAME:field}` inserts it as a named capture group called `field`.

use regex::{Regex, RegexBuilder};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Hard cap on the input text (1 MB).
pub const MAX_TEXT_BYTES: usize = 1_000_000;
/// Hard cap on the number of rules.
pub const MAX_RULES: usize = 200;
/// Upper bound accepted for the `max_records` parameter.
pub const MAX_RECORDS_LIMIT: usize = 50_000;
/// Upper bound accepted for the `max_matches` parameter.
pub const MAX_MATCHES_LIMIT: usize = 10_000;
/// Default value of the `max_records` parameter.
pub const DEFAULT_MAX_RECORDS: usize = 5_000;
/// Default value of the `max_matches` parameter.
pub const DEFAULT_MAX_MATCHES: usize = 1_000;

/// Accepted `split` values, in descriptor/enum order.
pub const SPLITS: &[&str] = &["whole", "lines", "paragraphs", "pattern"];
/// Accepted `matches` values, in descriptor/enum order.
pub const MATCHES: &[&str] = &["first", "all"];
/// Accepted `on_missing` values, in descriptor/enum order.
pub const ON_MISSING: &[&str] = &["skip", "null", "error"];
/// Accepted `output` values, in descriptor/enum order.
pub const OUTPUTS: &[&str] = &["json", "csv", "text", "report"];

/// The built-in pattern library, referenced as `%{NAME}` / `%{NAME:field}`.
/// Every body is written with non-capturing groups so a macro never shifts
/// the caller's capture-group numbering.
pub const BUILTIN_PATTERNS: &[(&str, &str)] = &[
    ("WORD", r"\w+"),
    ("NOTSPACE", r"\S+"),
    ("SPACE", r"\s+"),
    ("GREEDYDATA", r".*"),
    ("DATA", r".*?"),
    ("INT", r"[+-]?\d+"),
    ("NUMBER", r"[+-]?\d+(?:\.\d+)?"),
    ("HEX", r"(?:0[xX])?[0-9A-Fa-f]+"),
    ("HASH", r"[0-9a-fA-F]{32,64}"),
    (
        "UUID",
        r"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}",
    ),
    ("IPV4", r"(?:\d{1,3}\.){3}\d{1,3}"),
    ("IPV6", r"(?:[0-9A-Fa-f]{0,4}:){2,7}[0-9A-Fa-f]{0,4}"),
    (
        "IP",
        r"(?:(?:\d{1,3}\.){3}\d{1,3}|(?:[0-9A-Fa-f]{0,4}:){2,7}[0-9A-Fa-f]{0,4})",
    ),
    ("MAC", r"(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}"),
    ("EMAIL", r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"),
    ("URL", r#"https?://[^\s<>"']+"#),
    (
        "HOSTNAME",
        r"[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?)+",
    ),
    ("PATH", r"(?:/[^\s/]+)+/?"),
    ("DATE_ISO", r"\d{4}-\d{2}-\d{2}"),
    ("DATE_US", r"\d{1,2}/\d{1,2}/\d{2,4}"),
    ("TIME", r"\d{1,2}:\d{2}(?::\d{2}(?:\.\d+)?)?"),
    (
        "TIMESTAMP_ISO",
        r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?",
    ),
    ("SYSLOG_TIME", r"[A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}"),
    ("YEAR", r"\d{4}"),
    (
        "LOGLEVEL",
        r"(?i:TRACE|DEBUG|INFO|NOTICE|WARN(?:ING)?|ERROR|FATAL|CRITICAL)",
    ),
    (
        "HTTP_METHOD",
        r"(?:GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS|TRACE|CONNECT)",
    ),
    ("HTTP_STATUS", r"[1-5]\d{2}"),
    ("QUOTED", r#""[^"]*""#),
    ("MONEY", r"[$£€]\s?\d[\d,]*(?:\.\d{1,2})?"),
    ("PERCENT", r"\d+(?:\.\d+)?\s?%"),
    ("PHONE", r"\+?\d[\d\s().-]{6,}\d"),
    ("ZIP_US", r"\d{5}(?:-\d{4})?"),
    ("SEMVER", r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?"),
    ("TICKET", r"[A-Z][A-Z0-9]+-\d+"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum SplitMode {
    Whole,
    Lines,
    Paragraphs,
    Pattern,
}

impl SplitMode {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "whole" => Ok(SplitMode::Whole),
            "lines" | "line" => Ok(SplitMode::Lines),
            "paragraphs" | "paragraph" => Ok(SplitMode::Paragraphs),
            "pattern" => Ok(SplitMode::Pattern),
            other => Err(format!(
                "unknown split '{other}' — use whole, lines, paragraphs, or pattern"
            )),
        }
    }
    fn word(self) -> &'static str {
        match self {
            SplitMode::Lines => "line",
            SplitMode::Paragraphs => "paragraph",
            _ => "record",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OnMissing {
    Skip,
    Null,
    Error,
}

impl OnMissing {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "skip" => Ok(OnMissing::Skip),
            "null" => Ok(OnMissing::Null),
            "error" => Ok(OnMissing::Error),
            other => Err(format!(
                "unknown on_missing '{other}' — use skip, null, or error"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    Json,
    Csv,
    Text,
    Report,
}

impl OutputKind {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(OutputKind::Json),
            "csv" => Ok(OutputKind::Csv),
            "text" => Ok(OutputKind::Text),
            "report" => Ok(OutputKind::Report),
            other => Err(format!(
                "unknown output '{other}' — use json, csv, text, or report"
            )),
        }
    }
}

struct Rule {
    /// Field name(s) this rule produces, with the capture-group index each reads.
    fields: Vec<(String, usize)>,
    re: Regex,
    line_no: usize,
}

fn macro_ref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"%\{([A-Za-z_][A-Za-z0-9_]*)(?::([A-Za-z_][A-Za-z0-9_]*))?\}").unwrap()
    })
}

fn paragraph_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\r?\n(?:[ \t]*\r?\n)+").unwrap())
}

fn lookup_pattern<'a>(name: &str, user: &'a HashMap<String, String>) -> Option<&'a str> {
    if let Some(v) = user.get(name) {
        return Some(v.as_str());
    }
    BUILTIN_PATTERNS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
}

fn available_patterns(user: &HashMap<String, String>) -> String {
    let mut names: Vec<String> = BUILTIN_PATTERNS.iter().map(|(n, _)| n.to_string()).collect();
    let mut own: Vec<String> = user.keys().cloned().collect();
    own.sort();
    names.extend(own);
    names.join(", ")
}

fn expand_macros(
    src: &str,
    user: &HashMap<String, String>,
    depth: usize,
) -> Result<String, String> {
    if depth > 5 {
        return Err("pattern macros are nested more than 5 levels deep".to_string());
    }
    let re = macro_ref_re();
    let mut out = String::with_capacity(src.len());
    let mut last = 0usize;
    for caps in re.captures_iter(src) {
        let whole = caps.get(0).unwrap();
        out.push_str(&src[last..whole.start()]);
        let name = caps.get(1).unwrap().as_str();
        let body = lookup_pattern(name, user).ok_or_else(|| {
            format!(
                "unknown pattern %{{{name}}} — available patterns: {}",
                available_patterns(user)
            )
        })?;
        let body = expand_macros(body, user, depth + 1)?;
        match caps.get(2) {
            Some(field) => {
                out.push_str("(?<");
                out.push_str(field.as_str());
                out.push('>');
                out.push_str(&body);
                out.push(')');
            }
            None => {
                out.push_str("(?:");
                out.push_str(&body);
                out.push(')');
            }
        }
        last = whole.end();
    }
    out.push_str(&src[last..]);
    Ok(out)
}

/// Split a rules block into `(line_no, field_or_none, pattern_source)` triples,
/// after applying `@NAME = regex` macro definitions.
fn parse_rules(
    rules: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
) -> Result<Vec<Rule>, String> {
    let assign_re = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)[ \t]+=[ \t]*(.+)$").unwrap();
    let define_re = Regex::new(r"^@([A-Za-z_][A-Za-z0-9_]*)[ \t]*=[ \t]*(.+)$").unwrap();
    let mut user: HashMap<String, String> = HashMap::new();
    let mut out: Vec<Rule> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for (i, raw) in rules.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if line.starts_with('@') {
            let caps = define_re.captures(line).ok_or_else(|| {
                format!("line {line_no}: a pattern definition looks like '@NAME = regex'")
            })?;
            let name = caps[1].to_string();
            let body = expand_macros(caps[2].trim(), &user, 0)?;
            user.insert(name, body);
            continue;
        }
        if out.len() >= MAX_RULES {
            return Err(format!(
                "too many rules: the limit is {MAX_RULES} extraction rules"
            ));
        }

        let (field, pattern_src) = match assign_re.captures(line) {
            Some(c) => (Some(c[1].to_string()), c[2].trim().to_string()),
            None => (None, line.to_string()),
        };
        let expanded = expand_macros(&pattern_src, &user, 0)?;
        let re = RegexBuilder::new(&expanded)
            .case_insensitive(ignore_case)
            .multi_line(multiline)
            .dot_matches_new_line(dotall)
            .size_limit(8 << 20)
            .build()
            .map_err(|e| format!("line {line_no}: invalid regular expression — {e}"))?;

        let fields: Vec<(String, usize)> = match &field {
            Some(name) => {
                let idx = re
                    .capture_names()
                    .position(|n| n == Some(name.as_str()))
                    .or(if re.captures_len() > 1 { Some(1) } else { None })
                    .unwrap_or(0);
                vec![(name.clone(), idx)]
            }
            None => {
                let named: Vec<(String, usize)> = re
                    .capture_names()
                    .enumerate()
                    .filter_map(|(i, n)| n.map(|n| (n.to_string(), i)))
                    .collect();
                if named.is_empty() {
                    let hint = if line.contains('=') {
                        " (to name a field, put a space before the '=': 'field = pattern')"
                    } else {
                        ""
                    };
                    return Err(format!(
                        "line {line_no}: this rule produces no field — write 'field = pattern', or add a named group like (?<name>…) or %{{PATTERN:name}}{hint}"
                    ));
                }
                named
            }
        };

        for (name, _) in &fields {
            if let Some(prev) = seen.get(name) {
                return Err(format!(
                    "line {line_no}: field '{name}' is already produced by the rule on line {prev} — give one of them a different name"
                ));
            }
            seen.insert(name.clone(), line_no);
        }
        out.push(Rule {
            fields,
            re,
            line_no,
        });
    }

    if out.is_empty() {
        return Err(
            "no extraction rules — add at least one rule, for example: date = %{DATE_ISO}"
                .to_string(),
        );
    }
    Ok(out)
}

fn split_records<'a>(
    text: &'a str,
    mode: SplitMode,
    split_pattern: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
) -> Result<Vec<&'a str>, String> {
    Ok(match mode {
        SplitMode::Whole => vec![text],
        SplitMode::Lines => text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect(),
        SplitMode::Paragraphs => paragraph_re().split(text.trim()).collect(),
        SplitMode::Pattern => {
            if split_pattern.trim().is_empty() {
                return Err(
                    "split = pattern needs a record separator regex in split_pattern, for example ^-{3,}$"
                        .to_string(),
                );
            }
            let re = RegexBuilder::new(split_pattern)
                .case_insensitive(ignore_case)
                .multi_line(multiline)
                .dot_matches_new_line(dotall)
                .build()
                .map_err(|e| format!("invalid split_pattern — {e}"))?;
            re.split(text).collect()
        }
    })
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

struct RecordOut {
    index: usize,
    /// One slot per field, in declaration order. `None` = the rule matched nothing.
    values: Vec<Option<Vec<String>>>,
}

/// Apply `rules` to `text` and render the extracted fields.
#[allow(clippy::too_many_arguments)]
pub fn extract(
    text: &str,
    rules: &str,
    split: &str,
    split_pattern: &str,
    matches: &str,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    trim: bool,
    unique: bool,
    on_missing: &str,
    skip_empty_records: bool,
    max_records: usize,
    max_matches: usize,
    output: &str,
    pretty: bool,
) -> Result<String, String> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(format!(
            "input text is {} bytes; the limit is {MAX_TEXT_BYTES} bytes (1 MB)",
            text.len()
        ));
    }
    let all_mode = match matches.trim().to_ascii_lowercase().as_str() {
        "" | "first" => false,
        "all" => true,
        other => return Err(format!("unknown matches '{other}' — use first or all")),
    };
    let split_mode = SplitMode::parse(split)?;
    let missing = OnMissing::parse(on_missing)?;
    let out_kind = OutputKind::parse(output)?;
    if max_records == 0 || max_records > MAX_RECORDS_LIMIT {
        return Err(format!(
            "max_records must be between 1 and {MAX_RECORDS_LIMIT}"
        ));
    }
    if max_matches == 0 || max_matches > MAX_MATCHES_LIMIT {
        return Err(format!(
            "max_matches must be between 1 and {MAX_MATCHES_LIMIT}"
        ));
    }

    let parsed = parse_rules(rules, ignore_case, multiline, dotall)?;
    let field_order: Vec<String> = parsed
        .iter()
        .flat_map(|r| r.fields.iter().map(|(n, _)| n.clone()))
        .collect();

    let records = split_records(
        text,
        split_mode,
        split_pattern,
        ignore_case,
        multiline,
        dotall,
    )?;
    if records.len() > max_records {
        return Err(format!(
            "input has {} {}s; max_records is {max_records}. Raise max_records (up to {MAX_RECORDS_LIMIT}) or shorten the input",
            records.len(),
            split_mode.word()
        ));
    }

    let word = split_mode.word();
    let mut rows: Vec<RecordOut> = Vec::new();
    // Per-field stats for the report: (records hit, values).
    let mut hits: Vec<(usize, usize)> = vec![(0, 0); field_order.len()];
    let mut total_values = 0usize;
    let mut empty_records = 0usize;

    for (ri, rec) in records.iter().enumerate() {
        let mut values: Vec<Option<Vec<String>>> = vec![None; field_order.len()];
        let mut slot = 0usize;
        for rule in &parsed {
            let width = rule.fields.len();
            let mut collected: Vec<Vec<String>> = vec![Vec::new(); width];
            if all_mode {
                let mut count = 0usize;
                for caps in rule.re.captures_iter(rec) {
                    count += 1;
                    if count > max_matches {
                        return Err(format!(
                            "the rule on line {} matched more than {max_matches} times in {word} {}; raise max_matches (up to {MAX_MATCHES_LIMIT})",
                            rule.line_no,
                            ri + 1
                        ));
                    }
                    for (fi, (_, gi)) in rule.fields.iter().enumerate() {
                        if let Some(m) = caps.get(*gi) {
                            let v = if trim { m.as_str().trim() } else { m.as_str() };
                            collected[fi].push(v.to_string());
                        }
                    }
                }
            } else if let Some(caps) = rule.re.captures(rec) {
                for (fi, (_, gi)) in rule.fields.iter().enumerate() {
                    if let Some(m) = caps.get(*gi) {
                        let v = if trim { m.as_str().trim() } else { m.as_str() };
                        collected[fi].push(v.to_string());
                    }
                }
            }
            for (fi, mut vals) in collected.into_iter().enumerate() {
                if unique && vals.len() > 1 {
                    let mut seen: Vec<String> = Vec::new();
                    vals.retain(|v| {
                        if seen.iter().any(|s| s == v) {
                            false
                        } else {
                            seen.push(v.clone());
                            true
                        }
                    });
                }
                if vals.is_empty() {
                    if missing == OnMissing::Error {
                        let where_ = if split_mode == SplitMode::Whole {
                            "the input".to_string()
                        } else {
                            format!("{word} {}", ri + 1)
                        };
                        return Err(format!(
                            "field '{}' (rule on line {}) matched nothing in {where_} — set on_missing to skip or null to allow it",
                            field_order[slot + fi],
                            rule.line_no
                        ));
                    }
                } else {
                    hits[slot + fi].0 += 1;
                    hits[slot + fi].1 += vals.len();
                    total_values += vals.len();
                    values[slot + fi] = Some(vals);
                }
            }
            slot += width;
        }

        let any = values.iter().any(|v| v.is_some());
        if !any {
            empty_records += 1;
            if skip_empty_records && split_mode != SplitMode::Whole {
                continue;
            }
        }
        rows.push(RecordOut {
            index: ri + 1,
            values,
        });
    }

    if out_kind == OutputKind::Report {
        return Ok(render_report(
            &field_order,
            &parsed,
            &hits,
            records.len(),
            empty_records,
            total_values,
            split_mode,
            all_mode,
        ));
    }

    if total_values == 0 {
        return Err(format!(
            "no rule matched anywhere in the input ({} {}{} checked) — set Output to 'report' to see which rules failed",
            records.len(),
            word,
            if records.len() == 1 { "" } else { "s" }
        ));
    }

    Ok(match out_kind {
        OutputKind::Json => render_json(&field_order, &rows, missing, all_mode, split_mode, pretty),
        OutputKind::Csv => render_csv(&field_order, &rows),
        OutputKind::Text => render_text(&field_order, &rows, missing, split_mode, word),
        OutputKind::Report => unreachable!(),
    })
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn count_arg(s: &str, name: &str, default: usize, limit: usize) -> Result<usize, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<usize>()
        .ok()
        .filter(|n| *n >= 1 && *n <= limit)
        .ok_or_else(|| format!("{name} must be a whole number between 1 and {limit}"))
}

/// String-argument façade for the browser page (every form control arrives as
/// a string) — empty strings fall back to each parameter's default.
#[allow(clippy::too_many_arguments)]
pub fn extract_text(
    text: &str,
    rules: &str,
    split: &str,
    split_pattern: &str,
    matches: &str,
    ignore_case: &str,
    multiline: &str,
    dotall: &str,
    trim: &str,
    unique: &str,
    on_missing: &str,
    skip_empty_records: &str,
    max_records: &str,
    max_matches: &str,
    output: &str,
    pretty: &str,
) -> Result<String, String> {
    extract(
        text,
        rules,
        split,
        split_pattern,
        matches,
        truthy(ignore_case),
        truthy(multiline),
        truthy(dotall),
        truthy(trim),
        truthy(unique),
        on_missing,
        truthy(skip_empty_records),
        count_arg(max_records, "max_records", DEFAULT_MAX_RECORDS, MAX_RECORDS_LIMIT)?,
        count_arg(max_matches, "max_matches", DEFAULT_MAX_MATCHES, MAX_MATCHES_LIMIT)?,
        output,
        truthy(pretty),
    )
}

fn render_json(
    field_order: &[String],
    rows: &[RecordOut],
    missing: OnMissing,
    all_mode: bool,
    split_mode: SplitMode,
    pretty: bool,
) -> String {
    let to_obj = |row: &RecordOut| -> Value {
        let mut m = Map::new();
        for (fi, name) in field_order.iter().enumerate() {
            match &row.values[fi] {
                Some(vals) => {
                    let v = if all_mode {
                        Value::Array(vals.iter().map(|s| Value::String(s.clone())).collect())
                    } else {
                        Value::String(vals[0].clone())
                    };
                    m.insert(name.clone(), v);
                }
                None => {
                    if missing == OnMissing::Null {
                        m.insert(name.clone(), Value::Null);
                    }
                }
            }
        }
        Value::Object(m)
    };
    let doc = if split_mode == SplitMode::Whole {
        rows.first().map(to_obj).unwrap_or(Value::Object(Map::new()))
    } else {
        Value::Array(rows.iter().map(to_obj).collect())
    };
    if pretty {
        serde_json::to_string_pretty(&doc).unwrap_or_default()
    } else {
        serde_json::to_string(&doc).unwrap_or_default()
    }
}

fn render_csv(field_order: &[String], rows: &[RecordOut]) -> String {
    let mut out = String::new();
    out.push_str(
        &field_order
            .iter()
            .map(|f| csv_field(f))
            .collect::<Vec<_>>()
            .join(","),
    );
    for row in rows {
        out.push('\n');
        let cells: Vec<String> = row
            .values
            .iter()
            .map(|v| match v {
                Some(vals) => csv_field(&vals.join("; ")),
                None => String::new(),
            })
            .collect();
        out.push_str(&cells.join(","));
    }
    out
}

fn render_text(
    field_order: &[String],
    rows: &[RecordOut],
    missing: OnMissing,
    split_mode: SplitMode,
    word: &str,
) -> String {
    let mut blocks: Vec<String> = Vec::new();
    for row in rows {
        let mut lines: Vec<String> = Vec::new();
        let indent = if split_mode == SplitMode::Whole {
            ""
        } else {
            lines.push(format!(
                "{}{} {}",
                word[..1].to_uppercase(),
                &word[1..],
                row.index
            ));
            "  "
        };
        for (fi, name) in field_order.iter().enumerate() {
            match &row.values[fi] {
                Some(vals) => lines.push(format!("{indent}{name}: {}", vals.join("; "))),
                None => {
                    if missing == OnMissing::Null {
                        lines.push(format!("{indent}{name}:"));
                    }
                }
            }
        }
        blocks.push(lines.join("\n"));
    }
    blocks.join("\n\n")
}

#[allow(clippy::too_many_arguments)]
fn render_report(
    field_order: &[String],
    parsed: &[Rule],
    hits: &[(usize, usize)],
    total_records: usize,
    empty_records: usize,
    total_values: usize,
    split_mode: SplitMode,
    all_mode: bool,
) -> String {
    let word = split_mode.word();
    // Field name → the rule line that produces it.
    let mut lines_for: Vec<usize> = Vec::new();
    for rule in parsed {
        for _ in &rule.fields {
            lines_for.push(rule.line_no);
        }
    }
    let width = field_order.iter().map(|f| f.len()).max().unwrap_or(5).max(5);
    let mut out = String::new();
    out.push_str(&format!(
        "Rules: {} · Fields: {} · Matches: {}\n",
        parsed.len(),
        field_order.len(),
        if all_mode { "all" } else { "first" }
    ));
    out.push_str(&format!(
        "{}s: {total_records} ({} with no field, {} with at least one)\n",
        word[..1].to_uppercase() + &word[1..],
        empty_records,
        total_records - empty_records
    ));
    out.push_str(&format!("Values extracted: {total_values}\n\n"));
    out.push_str(&format!(
        "{:<width$}  rule line  {}s hit  values\n",
        "field",
        word,
        width = width
    ));
    let mut never: Vec<&str> = Vec::new();
    for (fi, name) in field_order.iter().enumerate() {
        let (recs, vals) = hits[fi];
        if recs == 0 {
            never.push(name);
        }
        out.push_str(&format!(
            "{:<width$}  {:>9}  {:>7}  {:>6}{}\n",
            name,
            lines_for[fi],
            format!("{recs}/{total_records}"),
            vals,
            if recs == 0 { "   never matched" } else { "" },
            width = width
        ));
    }
    if never.is_empty() {
        out.push_str("\nEvery rule matched at least once.");
    } else {
        out.push_str(&format!("\nNever matched: {}", never.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, rules: &str, split: &str, output: &str) -> Result<String, String> {
        extract(
            text, rules, split, "", "first", false, false, false, true, false, "skip", true, 1000,
            1000, output, false,
        )
    }

    #[test]
    fn named_rules_extract_fields_as_json() {
        let out = run(
            "Invoice INV-2026-014 dated 2026-08-15 total $1,240.00",
            "invoice = INV-\\d{4}-\\d+\ndate = %{DATE_ISO}\ntotal = %{MONEY}",
            "whole",
            "json",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"invoice":"INV-2026-014","date":"2026-08-15","total":"$1,240.00"}"#
        );
    }

    #[test]
    fn bare_rule_emits_every_named_group() {
        let out = run(
            "2026-08-15 ERROR auth login failed",
            "%{DATE_ISO:date} %{LOGLEVEL:level} (?<module>\\w+) (?<message>.*)",
            "whole",
            "json",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"date":"2026-08-15","level":"ERROR","module":"auth","message":"login failed"}"#
        );
    }

    #[test]
    fn lines_split_gives_one_object_per_line_and_skips_empty() {
        let out = run(
            "id=1 ok\nnothing here\nid=2 ok",
            "id = id=(\\d+)",
            "lines",
            "json",
        )
        .unwrap();
        assert_eq!(out, r#"[{"id":"1"},{"id":"2"}]"#);
    }

    #[test]
    fn matches_all_returns_arrays() {
        let out = extract(
            "a@x.com, b@y.org, a@x.com",
            "email = %{EMAIL}",
            "whole",
            "",
            "all",
            false,
            false,
            false,
            true,
            true,
            "skip",
            true,
            1000,
            1000,
            "json",
            false,
        )
        .unwrap();
        assert_eq!(out, r#"{"email":["a@x.com","b@y.org"]}"#);
    }

    #[test]
    fn user_macro_overrides_and_composes() {
        let out = run(
            "sku ABC-99 shipped",
            "@SKU = [A-Z]{3}-\\d+\nsku = %{SKU}",
            "whole",
            "json",
        )
        .unwrap();
        assert_eq!(out, r#"{"sku":"ABC-99"}"#);
    }

    #[test]
    fn csv_output_quotes_and_orders_columns() {
        let out = run(
            "name: Ada, Lovelace | role: engineer",
            "name = name: ([^|]+)\nrole = role: (\\w+)",
            "whole",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "name,role\n\"Ada, Lovelace\",engineer");
    }

    #[test]
    fn text_output_labels_each_record() {
        let out = run("x=1\nx=2", "x = x=(\\d)", "lines", "text").unwrap();
        assert_eq!(out, "Line 1\n  x: 1\n\nLine 2\n  x: 2");
    }

    #[test]
    fn on_missing_null_keeps_the_key() {
        let out = extract(
            "only a date 2026-08-15",
            "date = %{DATE_ISO}\nlevel = %{LOGLEVEL}",
            "whole",
            "",
            "first",
            false,
            false,
            false,
            true,
            false,
            "null",
            true,
            1000,
            1000,
            "json",
            false,
        )
        .unwrap();
        assert_eq!(out, r#"{"date":"2026-08-15","level":null}"#);
    }

    #[test]
    fn on_missing_error_names_the_field() {
        let err = extract(
            "only a date 2026-08-15",
            "date = %{DATE_ISO}\nlevel = %{LOGLEVEL}",
            "whole",
            "",
            "first",
            false,
            false,
            false,
            true,
            false,
            "error",
            true,
            1000,
            1000,
            "json",
            false,
        )
        .unwrap_err();
        assert!(err.contains("field 'level'"), "{err}");
    }

    #[test]
    fn report_flags_a_rule_that_never_matched() {
        let out = run(
            "2026-08-15 hello",
            "date = %{DATE_ISO}\nuser = user=(\\w+)",
            "lines",
            "report",
        )
        .unwrap();
        assert!(out.contains("Never matched: user"), "{out}");
        assert!(out.contains("Values extracted: 1"), "{out}");
    }

    #[test]
    fn unknown_pattern_lists_available_names() {
        let err = run("x", "a = %{NOPE}", "whole", "json").unwrap_err();
        assert!(err.contains("unknown pattern %{NOPE}"), "{err}");
        assert!(err.contains("DATE_ISO"), "{err}");
    }

    #[test]
    fn invalid_regex_reports_the_rule_line() {
        let err = run("x", "# comment\na = (unclosed", "whole", "json").unwrap_err();
        assert!(err.contains("line 2"), "{err}");
        assert!(err.contains("invalid regular expression"), "{err}");
    }

    #[test]
    fn a_rule_with_no_field_is_an_error_with_a_hint() {
        let err = run("host=web1", "host=(\\S+)", "whole", "json").unwrap_err();
        assert!(err.contains("produces no field"), "{err}");
        assert!(err.contains("space before the '='"), "{err}");
    }

    #[test]
    fn duplicate_field_names_are_rejected() {
        let err = run("a b", "x = a\nx = b", "whole", "json").unwrap_err();
        assert!(err.contains("already produced by the rule on line 1"), "{err}");
    }

    #[test]
    fn nothing_matched_points_at_the_report() {
        let err = run("abc", "n = \\d+", "whole", "json").unwrap_err();
        assert!(err.contains("no rule matched"), "{err}");
        assert!(err.contains("report"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        let err = run(&big, "x = x", "whole", "json").unwrap_err();
        assert!(err.contains("1000000 bytes"), "{err}");
    }

    #[test]
    fn max_records_boundary_is_exact() {
        let text = "a=1\na=2\na=3";
        let ok = extract(
            text, "a = a=(\\d)", "lines", "", "first", false, false, false, true, false, "skip",
            true, 3, 1000, "json", false,
        );
        assert!(ok.is_ok(), "{ok:?}");
        let err = extract(
            text, "a = a=(\\d)", "lines", "", "first", false, false, false, true, false, "skip",
            true, 2, 1000, "json", false,
        )
        .unwrap_err();
        assert!(err.contains("max_records is 2"), "{err}");
    }

    #[test]
    fn split_pattern_needs_a_separator() {
        let err = run("a\n---\nb", "x = \\w", "pattern", "json").unwrap_err();
        assert!(err.contains("split_pattern"), "{err}");
    }

    #[test]
    fn every_published_enum_value_is_accepted() {
        for s in SPLITS {
            assert!(SplitMode::parse(s).is_ok(), "split {s}");
        }
        for m in MATCHES {
            assert!(
                run("2026-08-15", "d = %{DATE_ISO}", "whole", "json").is_ok(),
                "sanity"
            );
            assert!(
                extract(
                    "2026-08-15", "d = %{DATE_ISO}", "whole", "", m, false, false, false, true,
                    false, "skip", true, 10, 10, "json", false,
                )
                .is_ok(),
                "matches {m}"
            );
        }
        for m in ON_MISSING {
            assert!(OnMissing::parse(m).is_ok(), "on_missing {m}");
        }
        for o in OUTPUTS {
            assert!(OutputKind::parse(o).is_ok(), "output {o}");
        }
    }

    #[test]
    fn string_facade_parses_flags_and_uses_defaults_for_blanks() {
        let out = extract_text(
            "IP 10.0.0.1 hit /login and IP 10.0.0.2 hit /admin",
            "ip = %{IPV4}\npath = %{PATH}",
            "",
            "",
            "all",
            "",
            "",
            "",
            "true",
            "",
            "",
            "true",
            "",
            "",
            "json",
            "",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"ip":["10.0.0.1","10.0.0.2"],"path":["/login","/admin"]}"#
        );
    }

    #[test]
    fn string_facade_rejects_an_out_of_range_count() {
        let err = extract_text(
            "x", "a = x", "", "", "", "", "", "", "true", "", "", "", "0", "", "json", "",
        )
        .unwrap_err();
        assert!(err.contains("max_records must be"), "{err}");
    }

    #[test]
    fn pretty_json_is_indented() {
        let out = extract_text(
            "2026-08-15", "date = %{DATE_ISO}", "", "", "", "", "", "", "true", "", "", "", "", "",
            "json", "true",
        )
        .unwrap();
        assert_eq!(out, "{\n  \"date\": \"2026-08-15\"\n}");
    }

    #[test]
    fn paragraph_split_groups_blocks() {
        let out = run(
            "name: Ada\n\nname: Bob",
            "name = name: (\\w+)",
            "paragraphs",
            "json",
        )
        .unwrap();
        assert_eq!(out, r#"[{"name":"Ada"},{"name":"Bob"}]"#);
    }
}
